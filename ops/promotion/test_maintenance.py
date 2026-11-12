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
from ops.promotion.maintenance import abandon, release_rolled_back_execution, terminal_proof
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

    def test_rolled_back_execution_releases_only_a_healthy_previous_release(self):
        self.aws.cs['ExecutionStatus'] = 'EXECUTE_FAILED'
        original = self.aws.stack
        authority = self.state['authority']['execution_role_arn']
        def stack():
            value = original()
            value['StackStatus'] = 'UPDATE_ROLLBACK_COMPLETE'
            value['RoleARN'] = authority
            return value
        self.aws.stack = stack
        self.store.change(lambda d: d['active'].update(phase='executing'))
        for name in ('stop-main', 'stop-broker', 'changeset-execute'):
            self.store.change(lambda d, name=name: d['active']['journal'].append({'name': name, 'status': 'confirmed'}))
        before = len(self.aws.calls)
        result = release_rolled_back_execution(self.runner, digest(self.store.load()), read_run=lambda _: self.run)
        state = self.store.load()
        self.assertEqual(result['status'], 'rolled-back')
        self.assertIsNone(state['active'])
        self.assertEqual(state['live']['stack_execution_role_arn'], authority)
        self.assertEqual(state['completed'][result['promotion_id']]['status'], 'rolled-back')
        self.assertTrue(state['ledger']['windows'][result['promotion_id']]['abandoned_at_utc'])
        self.assertFalse(any(o in ['update_service', 'execute_change_set', 'delete_change_set'] for _, o, _ in self.aws.calls[before:]))


class SchemaSnapshotAbandonTests(unittest.TestCase):
    """APP-175: a prepared schema release whose pre-outage snapshot is its only extra mutation."""
    def setUp(self):
        from ops.promotion import test_schema_release as schema_tests
        self.fixture = schema_tests.SchemaLifecycleTests('test_reviewed_release_snapshots_migrates_then_activates_and_admits_the_baseline')
        self.fixture.setUp(); self.addCleanup(self.fixture.doCleanups)
        self.fixture.prepared(); self.fixture.aws.snapshot_status = 'failed'
        with self.assertRaises(Refused): self.fixture.run_execute()
        self.runner, self.aws, self.store = self.fixture.runner, self.fixture.aws, self.fixture.store
        self.state = self.store.load(); self.active = self.state['active']
        self.assertEqual(self.active['phase'], 'prepared')
        self.offset = len(self.aws.calls)
    def attempt(self):
        return self.fixture.abandon()
    def rebind(self, entry, **arguments):
        # A journal whose snapshot really named something else: request, hash and receipt all agree.
        entry['request']['arguments'].update(arguments); entry['request_sha256'] = digest(entry['request'])
        key = 'beta/' + entry['receipt']; receipt = json.loads(self.fixture.s3.objects[key]); receipt['request'] = entry['request']
        self.fixture.s3.objects[key] = encoded(receipt); entry['receipt_sha256'] = hashlib.sha256(encoded(receipt)).hexdigest()
    def tamper(self, update):
        self.store.load(); self.store.change(update)
    def assert_refused_and_kept(self):
        with self.assertRaises(Refused): self.attempt()
        self.assertIsNotNone(self.store.load()['active'])
        self.assertFalse(any(o in ['delete_change_set', 'delete_db_snapshot', 'update_service'] or s == 'rds'
                             for s, o, _ in self.aws.calls[self.offset:]))
    def test_confirmed_snapshot_is_kept_and_does_not_block_abandonment(self):
        identifier = self.active['packet']['schema_release']['snapshot_identifier']
        result = self.attempt(); state = self.store.load()
        self.assertEqual(result['status'], 'abandoned'); self.assertIsNone(state['active'])
        self.assertEqual(state['live'], self.state['live']); self.assertEqual(state['baseline'], self.state['baseline'])
        self.assertEqual(state['policy'], self.state['policy'])
        self.assertEqual(set(state['ledger']['windows']), set(self.state['ledger']['windows']))
        self.assertFalse(any(s == 'rds' for s, _, _ in self.aws.calls[self.offset:]))
        self.assertIn(identifier, self.aws.snapshots)
        fence = json.loads(self.fixture.s3.objects['beta/promotions/' + result['promotion_id'] + '/abandon-fence.json'])
        self.assertEqual(fence['retained_schema_snapshot']['snapshot_identifier'], identifier)
        self.assertEqual(fence['retained_schema_snapshot']['journal_status'], 'confirmed')
    def test_unknown_duplicate_foreign_or_unbound_snapshot_refuses(self):
        def entry(d): return next(x for x in d['active']['journal'] if x['name'] == 'schema-snapshot')
        cases = [
            ('intent with a receipt', lambda d: entry(d).update(status='intent')),
            ('stop journaled', lambda d: d['active']['journal'].append({'name': 'stop-main', 'status': 'confirmed'})),
            ('duplicate', lambda d: d['active']['journal'].append(copy.deepcopy(entry(d)))),
            ('other instance', lambda d: self.rebind(entry(d), DBInstanceIdentifier='other-db')),
            ('other snapshot', lambda d: self.rebind(entry(d), DBSnapshotIdentifier='forma-schema-release-00000000-v22')),
            ('no release in packet', lambda d: d['active']['packet'].update(schema_release=None)),
            ('migration journaled', lambda d: d['active']['journal'].append(dict(copy.deepcopy(entry(d)), name='schema-migrate'))),
            ('preparing phase', lambda d: d['active'].update(phase='preparing')),
        ]
        for name, update in cases:
            with self.subTest(name=name):
                self.setUp(); self.tamper(update); self.assert_refused_and_kept()
    def snapshot_intent(self):
        self.setUp()
        def intent(d):
            e = next(x for x in d['active']['journal'] if x['name'] == 'schema-snapshot')
            for key in ['receipt', 'receipt_sha256']: e.pop(key)
            e['status'] = 'intent'
        self.tamper(intent)
    def test_snapshot_intent_is_read_back_once_and_kept(self):
        self.snapshot_intent()
        result = self.attempt(); identifier = self.active['packet']['schema_release']['snapshot_identifier']
        retained = json.loads(self.fixture.s3.objects['beta/promotions/' + result['promotion_id'] + '/abandoned.json'])['retained_schema_snapshot']
        self.assertEqual((retained['journal_status'], retained['readback']['present'], retained['readback']['status']), ('intent', True, 'failed'))
        self.assertEqual([o for s, o, _ in self.aws.calls[self.offset:] if s == 'rds'], ['describe_db_snapshots'])
        self.assertIn(identifier, self.aws.snapshots); self.assertIsNone(self.store.load()['active'])
    def test_a_snapshot_intent_beside_another_unknown_outcome_refuses(self):
        self.snapshot_intent()
        self.tamper(lambda d: next(x for x in d['active']['journal'] if x['name'] == 'changeset-create').update(status='intent'))
        self.assert_refused_and_kept()
    def test_unavailable_or_ambiguous_readback_keeps_the_lock(self):
        for error in ['AccessDenied', 'Throttling', 'two rows', 'another name']:
            with self.subTest(error=error):
                self.snapshot_intent(); original = self.aws._call
                if error in ['two rows', 'another name']:
                    def ambiguous(s, o, a, error=error):
                        value = original(s, o, a)
                        if o == 'describe_db_snapshots':
                            row = value['DBSnapshots'][0]
                            value['DBSnapshots'] = [row, row] if error == 'two rows' else [dict(row, DBSnapshotIdentifier='other')]
                        return value
                    self.aws._call = ambiguous
                else:
                    self.aws.readback_error = error
                with self.assertRaises(Refused): self.attempt()
                state = self.store.load(); self.assertIsNotNone(state['active'])
                self.assertEqual(next(x for x in state['active']['journal'] if x['name'] == 'schema-snapshot')['status'], 'intent')
                self.assertFalse(any(o == 'delete_change_set' for _, o, _ in self.aws.calls[self.offset:]))
    def test_a_read_back_snapshot_survives_a_retried_abandonment_but_not_a_changed_receipt(self):
        for tampered in [False, True, 'rebound']:
            with self.subTest(tampered=tampered):
                self.snapshot_intent(); self.aws.running_health = None
                with self.assertRaises(Refused): self.attempt()  # Fails after the readback was recorded.
                entry = next(x for x in self.store.load()['active']['journal'] if x['name'] == 'schema-snapshot')
                self.assertEqual(entry['status'], 'read-back'); self.aws.running_health = 'HEALTHY'
                reads = len([o for s, o, _ in self.aws.calls if s == 'rds'])
                if tampered:
                    key = 'beta/' + entry['readback_receipt']
                    self.fixture.s3.objects[key] = self.fixture.s3.objects[key].replace(b'failed', b'available')
                    if tampered == 'rebound':  # The hash agrees; only the recorded observation differs.
                        changed = hashlib.sha256(self.fixture.s3.objects[key]).hexdigest()
                        self.tamper(lambda d: next(x for x in d['active']['journal'] if x['name'] == 'schema-snapshot').update(readback_receipt_sha256=changed))
                    with self.assertRaises(Refused): self.attempt()
                    self.assertIsNotNone(self.store.load()['active'])
                else:
                    self.assertEqual(self.attempt()['status'], 'abandoned')
                # The second attempt reuses the recorded observation and reads nothing again.
                self.assertEqual(len([o for s, o, _ in self.aws.calls if s == 'rds']), reads)
    def test_changed_snapshot_receipt_refuses(self):
        key = 'beta/' + next(x for x in self.active['journal'] if x['name'] == 'schema-snapshot')['receipt']
        self.fixture.s3.objects[key] = self.fixture.s3.objects[key].replace(b'creating', b'available')
        self.assert_refused_and_kept()
    def test_rolled_back_release_releases_only_without_a_journaled_migration(self):
        finished = dict(run(), status='completed', updated_at=(NOW-timedelta(minutes=66)).isoformat())
        for names, released in [(('stop-main', 'stop-broker', 'changeset-execute', 'schema-migrate'), False),
                                (('stop-main', 'stop-broker', 'changeset-execute'), True)]:
            with self.subTest(released=released):
                self.setUp(); self.aws.phase = 'operator'; self.aws.cs['ExecutionStatus'] = 'EXECUTE_FAILED'
                original = self.aws.stack; authority = self.state['authority']['execution_role_arn']
                self.aws.stack = lambda: dict(original(), StackStatus='UPDATE_ROLLBACK_COMPLETE', RoleARN=authority)
                self.tamper(lambda d: (d['active'].update(phase='executing'), d['active']['journal'].extend(
                    {'name': n, 'status': 'confirmed'} for n in names)))
                if released:
                    self.assertEqual(release_rolled_back_execution(self.runner, digest(self.store.load()), read_run=lambda _: finished)['status'], 'rolled-back')
                else:
                    with self.assertRaises(Refused):
                        release_rolled_back_execution(self.runner, digest(self.store.load()), read_run=lambda _: finished)
                    self.assertIsNotNone(self.store.load()['active'])


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
