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
from ops.promotion.maintenance import abandon, terminal_proof
from ops.promotion.state import digest, Refused
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
