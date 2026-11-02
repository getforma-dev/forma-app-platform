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
