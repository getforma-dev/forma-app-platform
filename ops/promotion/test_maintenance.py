import copy
from datetime import timedelta
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
from ops.promotion.maintenance import abandon, terminal_proof
from ops.promotion.state import digest, encoded, Refused
from ops.promotion import test_runner
from ops.promotion.guards import ACCOUNT, REGION
NOW = test_runner.NOW
from ops.promotion.test_github import run


class MaintenanceTests(unittest.TestCase):
    def setUp(self):
        self.fixture=test_runner.LifecycleTests('test_real_prepare_packet_execute_summary_and_root_manifest_publish')
        self.fixture.setUp();self.addCleanup(self.fixture.doCleanups)
        self.fixture.prepared();self.runner=self.fixture.runner;self.aws=self.fixture.aws;self.store=self.fixture.store
        self.aws.phase='operator';self.state=self.store.load()
        self.run=dict(run(),status='completed',updated_at=(NOW-timedelta(minutes=66)).isoformat())
    def test_original_running_or_fresh_credentials_never_unlock(self):
        for r in [dict(self.run,status='in_progress'),dict(self.run,updated_at=(NOW-timedelta(minutes=64)).isoformat())]:
            with self.assertRaises(Refused):abandon(self.runner,digest(self.state),read_run=lambda _:r)
        self.assertIsNotNone(self.store.load()['active'])
        self.assertFalse(any(o=='delete_change_set' for _,o,_ in self.aws.calls))
    def test_old_state_unknown_operation_or_execution_never_unlock(self):
        with self.assertRaises(Refused):abandon(self.runner,'0'*64,read_run=lambda _:self.run)
        for phase in ['executing','activation-intended','preparing']:
            self.store.change(lambda d:d['active'].update(phase=phase));state=self.store.load()
            with self.assertRaises(Refused):abandon(self.runner,digest(state),read_run=lambda _:self.run)
        self.assertFalse(any(o=='delete_change_set' for _,o,_ in self.aws.calls))
    def test_fenced_prepared_abandon_preserves_history_and_live_state(self):
        original_call=self.aws._call
        def transport(s,o,a):
            if o=='delete_change_set':self.aws.cs=None;return {}
            return original_call(s,o,a)
        self.aws._call=transport
        result=abandon(self.runner,digest(self.state),read_run=lambda _:self.run)
        state=self.store.load();self.assertEqual(result['status'],'abandoned');self.assertIsNone(state['active'])
        self.assertEqual(state['live'],self.state['live']);self.assertEqual(set(state['ledger']['windows']),set(self.state['ledger']['windows']))
        self.assertEqual(state['ledger']['months'],self.state['ledger']['months'])
        prior=state['ledger']['windows'][state['live']['window_id']]
        self.assertNotIn('superseded_by_window_id',prior)
        self.assertTrue(state['ledger']['windows'][result['promotion_id']]['abandoned_at_utc'])
        self.assertFalse(any(o=='update_service' for _,o,_ in self.aws.calls))
    def test_lost_delete_ack_keeps_lock_and_reservation(self):
        original_call=self.aws._call
        def transport(s,o,a):
            if o=='delete_change_set':raise TimeoutError('unknown')
            return original_call(s,o,a)
        self.aws._call=transport
        with self.assertRaises(TimeoutError):abandon(self.runner,digest(self.state),read_run=lambda _:self.run)
        state=self.store.load();self.assertIsNotNone(state['active']);self.assertEqual(state['ledger'],self.state['ledger'])
        self.assertEqual(state['active']['journal'][-1]['status'],'intent')


class RegistryOnlyAbandonTests(unittest.TestCase):
    def setUp(self):
        self.fixture = test_runner.LifecycleTests('test_real_prepare_packet_execute_summary_and_root_manifest_publish')
        self.fixture.setUp(); self.addCleanup(self.fixture.doCleanups)
        self.runner, self.aws, self.store = self.fixture.runner, self.fixture.aws, self.fixture.store
        # Exercise the production preparation path through real OCI publication,
        # then fail at exactly the scan-disposition boundary, before any template.
        with patch('ops.promotion.runner.guard_scan_delta', side_effect=Refused('scan review required')):
            with self.assertRaises(Refused): self.runner.prepare(self.fixture.request, self.fixture.directory)
        self.aws.phase = 'operator'; self.state = self.store.load(); self.active = self.state['active']
        self.run = dict(run(), status='completed', updated_at=(NOW-timedelta(minutes=66)).isoformat())
        self.call_offset = len(self.aws.calls)
    def attempt(self):
        return abandon(self.runner, digest(self.store.load()), read_run=lambda _: self.run)
    def assert_no_cloud_mutations(self):
        self.assertFalse(any(o in ['put_image','initiate_layer_upload','upload_layer_part','complete_layer_upload',
            'create_change_set','delete_change_set','execute_change_set','update_service']
            for _,o,_ in self.aws.calls[self.call_offset:]))
    def test_confirmed_upload_failure_can_abandon_without_cloud_mutation(self):
        self.assertEqual(self.active['phase'], 'preparing'); self.assertIsNone(self.active['packet'])
        self.assertTrue(self.active['journal']); self.assertTrue(all(x['status']=='confirmed' for x in self.active['journal']))
        result = self.attempt(); state = self.store.load()
        self.assertIsNone(state['active']); self.assertEqual(state['live'], self.state['live'])
        self.assertEqual(set(state['ledger']['windows']), set(self.state['ledger']['windows']))
        self.assertEqual(state['ledger']['months'], self.state['ledger']['months'])
        prior = state['live']['window_id']; self.assertNotIn('superseded_by_window_id', state['ledger']['windows'][prior])
        wid = result['promotion_id']; window = state['ledger']['windows'][wid]
        self.assertEqual(window['abandoned_supersession_of_window_id'], prior)
        self.assertEqual(window['reserved_compute_cents_by_month'], self.state['ledger']['windows'][wid]['reserved_compute_cents_by_month'])
        for entry in self.active['journal']:
            self.assertIn('beta/' + entry['receipt'], self.fixture.s3.objects)
        fence = json.loads(self.fixture.s3.objects['beta/promotions/'+wid+'/abandon-fence.json'])
        receipt = json.loads(self.fixture.s3.objects['beta/promotions/'+wid+'/abandoned.json'])
        self.assertEqual(fence['registry_receipts_verified'],len(self.active['journal']))
        self.assertEqual(fence['journal'], self.active['journal']); self.assertEqual(fence['reviewed_state_sha256'], digest(self.state))
        self.assertEqual(receipt['abandoned_window'], window)
        self.assertEqual(receipt['updated_ledger_sha256'], digest(state['ledger']))
        self.assert_no_cloud_mutations()
    def test_fresh_credentials_running_workflow_wrong_operator_and_state_still_refused(self):
        for proof in [dict(self.run,status='in_progress'), dict(self.run,updated_at=(NOW-timedelta(minutes=64)).isoformat())]:
            with self.assertRaises(Refused): abandon(self.runner,digest(self.state),read_run=lambda _:proof)
        with self.assertRaises(Refused): abandon(self.runner,'0'*64,read_run=lambda _:self.run)
        for role in ['prepare','execute']:
            self.aws.phase=role
            with self.assertRaises(Refused): self.attempt()
        self.assertEqual(self.store.load()['ledger'],self.state['ledger']); self.assertIsNotNone(self.store.load()['active'])
        self.assert_no_cloud_mutations()
    def test_unknown_non_registry_or_malformed_receipt_refused(self):
        variations=[]
        active=copy.deepcopy(self.active); active['journal'][0]['status']='intent'; variations.append(active)
        for name in ['template-upload','changeset-create','changeset-execute','stop-main','activate-main','unexpected']:
            active=copy.deepcopy(self.active); active['journal'].append({'name':name,'status':'confirmed','request':{}}); variations.append(active)
        for key,value in [('receipt_sha256',''),('request_sha256','0'*64),('receipt','different-receipt')]:
            active=copy.deepcopy(self.active); active['journal'][0][key]=value; variations.append(active)
        active=copy.deepcopy(self.active); active['packet']={'unexpected':True}; variations.append(active)
        active=copy.deepcopy(self.active); active.pop('reservation'); variations.append(active)
        active=copy.deepcopy(self.active); active['journal']=[]; variations.append(active)
        for active in variations:
            with self.subTest(active=active.get('packet')):
                self.store.change(lambda d:d.update(active=active))
                with self.assertRaises((Refused,KeyError)): self.attempt()
                self.assertIsNotNone(self.store.load()['active']); self.assertEqual(self.store.load()['ledger'],self.state['ledger'])
        self.assert_no_cloud_mutations()
    def test_missing_or_altered_actual_receipt_blocks_abandonment(self):
        entry=self.active['journal'][0];key='beta/'+entry['receipt'];original=self.fixture.s3.objects[key]
        for raw in [None,original+b' ',b'{}']:
            if raw is None: self.fixture.s3.objects.pop(key,None)
            else: self.fixture.s3.objects[key]=raw
            with self.assertRaises((RuntimeError,Refused)): self.attempt()
            self.assertIsNotNone(self.store.load()['active']);self.assertEqual(self.store.load()['ledger'],self.state['ledger'])
            self.assertFalse(any(k.endswith('/abandon-fence.json') for k in self.fixture.s3.objects))
        self.fixture.s3.objects[key]=original
        self.assert_no_cloud_mutations()
    def test_rehashed_receipt_with_wrong_request_extra_field_or_nonobject_response_refuses(self):
        entry=self.active['journal'][0];key='beta/'+entry['receipt'];original=json.loads(self.fixture.s3.objects[key])
        variants=[dict(original,request={}),dict(original,unexpected=True),dict(original,response=[]),
                  dict(original,observed_at_utc='2026-02-30T00:00:00Z')]
        for receipt in variants:
            raw=encoded(receipt);self.fixture.s3.objects[key]=raw
            self.store.change(lambda d:d['active']['journal'][0].update(receipt_sha256=hashlib.sha256(raw).hexdigest()))
            with self.assertRaises((Refused,ValueError)): self.attempt()
            self.assertIsNotNone(self.store.load()['active']);self.assertEqual(self.store.load()['ledger'],self.state['ledger'])
        self.assert_no_cloud_mutations()
    def test_oversized_receipt_is_read_with_explicit_bound(self):
        entry=self.active['journal'][0];key='beta/'+entry['receipt']
        raw=b'x'*257;self.fixture.s3.objects[key]=raw
        self.store.change(lambda d:d['active']['journal'][0].update(receipt_sha256=hashlib.sha256(raw).hexdigest()))
        with patch('ops.promotion.maintenance.MAX_REGISTRY_RECEIPT_BYTES',256):
            with self.assertRaises(Refused): self.attempt()
        self.assertIsNotNone(self.store.load()['active']);self.assert_no_cloud_mutations()
    def test_complete_inventory_checks_later_page_and_repeated_tokens(self):
        original=self.aws._call
        history={'ChangeSetId':'old','Status':'CREATE_COMPLETE','ExecutionStatus':'EXECUTE_COMPLETE'}
        for final in [{'Summaries':[dict(history,ChangeSetId='pending',ExecutionStatus='AVAILABLE')]},
                      {'Summaries':[], 'NextToken':'next'}]:
            def transport(service,operation,args):
                if operation=='list_change_sets':
                    return final if args.get('NextToken') else {'Summaries':[history], 'NextToken':'next'}
                return original(service,operation,args)
            self.aws._call=transport
            with self.assertRaises(Refused): self.attempt()
        self.assertIsNotNone(self.store.load()['active']); self.assert_no_cloud_mutations()
    def test_complete_terminal_inventory_allows_abandon_and_is_retained(self):
        original=self.aws._call
        history={'ChangeSetId':'old','Status':'CREATE_COMPLETE','ExecutionStatus':'EXECUTE_COMPLETE'}
        def transport(service,operation,args):
            if operation=='list_change_sets':
                return {'Summaries':[]} if args.get('NextToken') else {'Summaries':[history],'NextToken':'next'}
            return original(service,operation,args)
        self.aws._call=transport
        result=self.attempt();wid=result['promotion_id']
        receipt=json.loads(self.fixture.s3.objects['beta/promotions/'+wid+'/abandoned.json'])
        self.assertEqual(len(receipt['final_change_set_inventory']),2)
        self.assertEqual(receipt['final_change_set_inventory'][0]['Summaries'],[history])
        self.assert_no_cloud_mutations()
    def test_registry_only_failure_retains_lock_if_live_boundary_changed(self):
        self.aws.counts['broker']=0
        with self.assertRaises(Refused): self.attempt()
        self.assertEqual(self.store.load()['ledger'], self.state['ledger']); self.assertIsNotNone(self.store.load()['active'])
        self.assert_no_cloud_mutations()
    def test_receipt_write_failure_keeps_reservation_and_lock(self):
        self.fixture.s3.fail_receipt=True
        with self.assertRaises(TimeoutError): self.attempt()
        self.assertEqual(self.store.load()['ledger'], self.state['ledger']); self.assertIsNotNone(self.store.load()['active'])
        self.assert_no_cloud_mutations()


class SyntheticApiError(Exception):
    def __init__(self, code, status):
        self.response={'Error':{'Code':code},'ResponseMetadata':{'HTTPStatusCode':status}}


class RejectedCreateRecoveryTests(unittest.TestCase):
    def setUp(self):
        self.fixture=test_runner.LifecycleTests('test_real_prepare_packet_execute_summary_and_root_manifest_publish')
        self.fixture.setUp();self.addCleanup(self.fixture.doCleanups)
        self.runner,self.aws,self.store=self.fixture.runner,self.fixture.aws,self.fixture.store
        original=self.aws._call
        def fail_create(service,operation,args):
            if operation=='create_change_set':raise SyntheticApiError('ValidationException',400)
            return original(service,operation,args)
        self.aws._call=fail_create
        with patch('ops.promotion.state.now',return_value=NOW.strftime('%Y-%m-%dT%H:%M:%SZ')):
            with self.assertRaises(SyntheticApiError):self.runner.prepare(self.fixture.request,self.fixture.directory)
        self.state=self.store.load();self.active=self.state['active'];self.aws.phase='operator'
        self.runner.clock=lambda:NOW+timedelta(hours=2)
        self.run=dict(run(),status='completed',updated_at=(NOW+timedelta(minutes=4)).isoformat())
        self.event={'eventID':'00000000-0000-4000-8000-000000000001','requestID':'00000000-0000-4000-8000-000000000002',
            'eventTime':(NOW+timedelta(seconds=1)).strftime('%Y-%m-%dT%H:%M:%SZ'),
            'eventName':'CreateChangeSet','eventSource':'cloudformation.amazonaws.com','awsRegion':REGION,
            'recipientAccountId':ACCOUNT,'eventType':'AwsApiCall','managementEvent':True,'readOnly':False,
            'userIdentity':{'arn':f'arn:aws:sts::{ACCOUNT}:assumed-role/prepare/beta-plan-42','type':'AssumedRole','accountId':ACCOUNT,
                'sessionContext':{'sessionIssuer':{'arn':self.state['authority']['prepare_role_arn']}}},
            'errorCode':'ValidationException','errorMessage':
                f'User: arn:aws:sts::{ACCOUNT}:assumed-role/restricted-cfn/AWSCloudFormation is not authorized to perform: '
                f'ssm:GetParameters on resource: arn:aws:ssm:{REGION}:{ACCOUNT}:parameter/cdk-bootstrap/hnb659fds/version '
                'because no identity-based policy allows the ssm:GetParameters action '
                '(Service: AWSSimpleSystemsManagement; Status Code: 400; Error Code: AccessDeniedException; Request ID: '
                '00000000-0000-4000-8000-000000000003; Proxy: null)',
            'requestParameters':None,'responseElements':None}
        self.events=[self.event];self.inventory=[];self.absence='ChangeSetNotFound';self.lookup_error=False
        def transport(service,operation,args):
            if operation=='lookup_events':
                if self.lookup_error:raise SyntheticApiError('AccessDenied',403)
                return {'Events':[{'EventId':e['eventID'],'CloudTrailEvent':json.dumps(e)} for e in self.events]}
            if operation=='describe_change_set':
                if self.absence is None:return {'Status':'CREATE_COMPLETE'}
                raise SyntheticApiError(self.absence,404 if self.absence=='ChangeSetNotFound' else 403)
            if operation=='list_change_sets':return {'Summaries':self.inventory}
            return original(service,operation,args)
        self.aws._call=transport;self.offset=len(self.aws.calls)
    def attempt(self, enabled=True):
        return abandon(self.runner,digest(self.store.load()),read_run=lambda _:self.run,reconcile_failed_create=enabled)
    def held(self):
        state=self.store.load();self.assertIsNotNone(state['active']);self.assertEqual(state['ledger'],self.state['ledger'])
        self.assertEqual(state['active']['journal'][-1]['status'],'intent')
        self.assertFalse(any(o in ['create_change_set','delete_change_set','execute_change_set','update_service','put_image'] for _,o,_ in self.aws.calls[self.offset:]))
    def test_definitive_rejection_preserves_original_intent_and_omitted_parameter_limit(self):
        with self.assertRaises(Refused):self.attempt(enabled=False)
        result=self.attempt();state=self.store.load();self.assertIsNone(state['active']);self.assertEqual(state['live'],self.state['live'])
        self.assertEqual(set(state['ledger']['windows']),set(self.state['ledger']['windows']))
        fence=json.loads(self.fixture.s3.objects['beta/promotions/'+result['promotion_id']+'/abandon-fence.json'])
        self.assertEqual(fence['journal'],self.active['journal']);self.assertEqual(fence['journal'][-1]['status'],'intent')
        proof=fence['rejected_create_reconciliation'];self.assertFalse(proof['cloudtrail_request_parameters_present'])
        self.assertFalse(proof['client_token_observed_in_cloudtrail'])
        self.assertEqual(proof['canonical_client_token'],self.active['journal'][-1]['request']['arguments']['ClientToken'])
        self.assertEqual(proof['cloudtrail_event'],self.event)
        self.assertEqual(sum(o=='describe_change_set' for _,o,_ in self.aws.calls[self.offset:]),2)
        self.assertFalse(any(o in ['create_change_set','delete_change_set','execute_change_set','update_service','put_image'] for _,o,_ in self.aws.calls[self.offset:]))
    def test_other_error_role_time_region_or_nonnull_mismatch_refuses(self):
        original=copy.deepcopy(self.event)
        changes=[lambda e:e.update(errorCode='AccessDenied'),lambda e:e.update(errorMessage=e['errorMessage'].replace('ssm:GetParameters','ssm:GetParameter')),
            lambda e:e.update(errorMessage=e['errorMessage'].replace('/hnb659fds/version','/different/version')),
            lambda e:e.update(responseElements={'Id':'created'}),lambda e:e.update(requestParameters={'clientToken':'different'}),
            lambda e:e.update(awsRegion='us-west-2'),lambda e:e.update(recipientAccountId='000000000000'),
            lambda e:e['userIdentity'].update(arn=e['userIdentity']['arn']+'-other'),
            lambda e:e['userIdentity']['sessionContext']['sessionIssuer'].update(arn='wrong-role'),
            lambda e:e.update(eventTime=(NOW+timedelta(minutes=3)).strftime('%Y-%m-%dT%H:%M:%SZ'))]
        for change in changes:
            self.event=copy.deepcopy(original);change(self.event);self.events=[self.event]
            with self.subTest(change=change),self.assertRaises(Refused):self.attempt()
            self.held()
    def test_ambiguous_absent_or_unreadable_cloudtrail_refuses(self):
        for events in [[],[self.event,copy.deepcopy(self.event)]]:
            self.events=events
            with self.assertRaises(Refused):self.attempt()
            self.held()
        self.events=[self.event];self.lookup_error=True
        with self.assertRaises(SyntheticApiError):self.attempt()
        self.held()
    def test_altered_intended_token_or_uploaded_template_refuses(self):
        original=copy.deepcopy(self.active)
        changed=copy.deepcopy(original);entry=changed['journal'][-1];entry['request']['arguments']['ClientToken']='different';entry['request_sha256']=digest(entry['request'])
        self.store.change(lambda d:d.update(active=changed))
        with self.assertRaises(Refused):self.attempt()
        self.held();self.store.change(lambda d:d.update(active=original))
        key=original['journal'][-2]['request']['arguments']['Key'];raw=self.fixture.s3.objects[key];self.fixture.s3.objects[key]=raw+b' '
        with self.assertRaises(Refused):self.attempt()
        self.held()
    def test_exact_existing_change_set_pending_inventory_or_wrong_absence_error_refuses(self):
        for absence in [None,'AccessDenied']:
            self.absence=absence
            with self.assertRaises((Refused,RuntimeError)):self.attempt()
            self.held()
        self.absence='ChangeSetNotFound'
        name=self.active['journal'][-1]['request']['arguments']['ChangeSetName']
        for summary in [{'ChangeSetName':name,'Status':'FAILED','ExecutionStatus':'UNAVAILABLE'},
                        {'ChangeSetName':'another','Status':'CREATE_COMPLETE','ExecutionStatus':'AVAILABLE'}]:
            self.inventory=[summary]
            with self.assertRaises(Refused):self.attempt()
            self.held()
    def test_short_fence_and_another_unknown_operation_never_bypass(self):
        self.run['updated_at']=(self.runner.clock()-timedelta(minutes=64)).isoformat()
        with self.assertRaises(Refused):self.attempt()
        self.held();self.run['updated_at']=(NOW+timedelta(minutes=4)).isoformat()
        self.store.change(lambda d:d['active']['journal'][0].update(status='intent'))
        with self.assertRaises(Refused):self.attempt()
        self.held()


class LegacyFenceTests(unittest.TestCase):
    def test_direct_preflight_and_shared_shell_env_refuse_after_marker(self):
        repo=Path(__file__).resolve().parents[2]
        with tempfile.TemporaryDirectory() as td:
            private=Path(td);private.chmod(0o700)
            ledger=private/'budget-ledger.json';ledger.write_text('{}');ledger.chmod(0o600)
            (private/'promotion-managed.json').write_text('{}');(private/'env.sh').write_text('touch "'+str(private/'must-not-run')+'"\n')
            before=ledger.read_bytes()
            result=subprocess.run([sys.executable,str(repo/'infra/scripts/preflight.py'),'--ledger',str(ledger),'--abandon','00000000-0000-4000-8000-000000000001'],capture_output=True,text=True)
            self.assertNotEqual(result.returncode,0);self.assertIn('promotion S3',result.stderr);self.assertEqual(ledger.read_bytes(),before)
            self.assertFalse(Path(str(ledger)+'.lock').exists())
            result=subprocess.run(['bash','-c','source ops/beta/lib.sh; load_env'],cwd=repo,env=dict(os.environ,GENUI_OPS_PRIV=str(private)),capture_output=True,text=True)
            self.assertNotEqual(result.returncode,0);self.assertIn('legacy operations are fenced',result.stderr);self.assertFalse((private/'must-not-run').exists())

if __name__=='__main__':unittest.main()
