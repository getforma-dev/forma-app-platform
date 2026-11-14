"""Offline policy qualification using actual boundary, CAS and budget code."""
import copy
import contextlib
import io
import json
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch

from ops.promotion import expiry_policy
from ops.promotion.runner import Runner, preflight
from ops.promotion.state import Refused, Store, digest, encoded
from ops.promotion.test_runner import FakeAWS, NOW, fixture
from ops.promotion.test_state import MemoryS3


class TracedS3(MemoryS3):
    def __init__(self, state):
        super().__init__(state)
        self.events = []

    def put_object(self, **args):
        self.events.append(('state' if args['Key'] == 'beta/state.json' else 'receipt', copy.deepcopy(args)))
        return super().put_object(**args)


class ExpiryPolicyTests(unittest.TestCase):
    def setUp(self):
        environment = patch.dict('os.environ', {'GITHUB_ACTIONS': 'false'})
        environment.start(); self.addCleanup(environment.stop)
        manifest = {'source': {'schema_manifest_sha256': 'a' * 64, 'deployment_contract_sha256': 'b' * 64}}
        self.state = fixture(manifest)
        self.state['policy'].update(budget_genui_cents=21000, budget_total_cents=24800)
        self.s3 = TracedS3(self.state)
        self.store = Store(self.s3, 'bucket', 'beta')
        self.aws = FakeAWS(self.state, self.s3, manifest)
        self.aws.phase = 'operator'
        self.runner = Runner(self.aws, self.store, clock=lambda: NOW)
        self.before = digest(self.state)
        self.hours = 18

    def run_policy(self, apply=False, **kwargs):
        return expiry_policy.run(self.runner, kwargs.get('hours', self.hours), kwargs.get('expected', self.before), apply)

    def replace_state(self):
        self.s3.objects['beta/state.json'] = encoded(self.state)
        self.aws.state = copy.deepcopy(self.state)
        self.before = digest(self.state)

    def assert_unchanged(self):
        self.assertEqual(self.s3.objects['beta/state.json'], encoded(self.state))

    def test_read_only_plan_uses_actual_reserve_and_public_projection(self):
        result = self.run_policy()
        self.assertEqual(self.s3.events, [])
        self.assert_unchanged()
        public = result['public']
        self.assertEqual(public['status'], 'planned')
        self.assertEqual((public['genui_cap_cents'], public['project_cap_cents']), (21000, 24800))
        self.assertEqual(public['shortening_hours'], 18)
        self.assertFalse(public['ledger_changed']); self.assertFalse(public['live_changed'])
        self.assertEqual(result['receipt']['prospective_config']['supersedes_window_id'], self.state['live']['window_id'])
        self.assertEqual(result['receipt']['ledger_sha256'], digest(self.state['ledger']))
        # Nothing in the public projection carries an account, ARN, hostname,
        # operator identity, or prospective/reserved window identifier.
        text = json.dumps(public)
        for private in ['arn:', '877573132739', 'beta.getforma.dev', self.state['live']['window_id']]:
            self.assertNotIn(private, text)
        self.assertEqual(result['receipt_key'], self.run_policy()['receipt_key'])
        self.assertEqual(public, self.run_policy()['public'])

    def test_apply_retains_immutable_intent_then_one_cas_and_exact_readback(self):
        result = self.run_policy(apply=True)
        expected = copy.deepcopy(self.state)
        expected['policy']['shorten_expiry_hours'] = self.hours
        expected['revision'] += 1
        self.assertEqual(self.store.load(), expected)
        self.assertEqual([kind for kind, _ in self.s3.events], ['receipt', 'state'])
        receipt_request, state_request = [request for _, request in self.s3.events]
        self.assertTrue(receipt_request['Key'].startswith('beta/promotions/expiry-policy-'))
        self.assertEqual(receipt_request['IfNoneMatch'], '*')
        self.assertEqual(state_request['IfMatch'], '1')
        receipt = json.loads(receipt_request['Body'])
        self.assertEqual(receipt['expected_after_state_sha256'], digest(expected))
        self.assertEqual(receipt['before_state_sha256'], self.before)
        self.assertEqual(result['public']['state_sha256'], digest(expected))
        self.assertEqual(result['public']['status'], 'applied')
        import hashlib
        self.assertEqual(receipt['source_sha256'], hashlib.sha256(Path(expiry_policy.__file__).read_bytes()).hexdigest())
        self.assertEqual(result['receipt_sha256'], hashlib.sha256(receipt_request['Body']).hexdigest())
        self.assertEqual(self.store.load()['policy']['schema_version'], 19)
        self.assertTrue(all(op not in ['update_service', 'execute_change_set', 'create_change_set', 'put_image'] for _, op, _ in self.aws.calls))

    def test_active_lock_and_stale_or_invalid_hash_refuse_before_cloud_reads(self):
        for active, expected in [({'owner': 'another'}, None), (None, '0' * 64), (None, 'invalid')]:
            with self.subTest(active=active, expected=expected):
                self.setUp(); self.state['active'] = active; self.replace_state()
                with self.assertRaises(Refused): self.run_policy(apply=True, expected=expected or self.before)
                self.assertEqual(self.aws.calls, []); self.assertEqual(self.s3.events, []); self.assert_unchanged()

    def test_hours_must_strictly_shorten_and_are_bounded(self):
        for hours in [True, 0, 1, 2, 2.5, 73, '18']:
            with self.subTest(hours=hours), self.assertRaises(Refused): self.run_policy(apply=True, hours=hours)
        self.assertEqual(self.s3.events, [])

    def test_workflow_execution_foreign_and_root_identities_refuse(self):
        for role in ['prepare', 'execute', 'restricted-cfn']:
            self.aws.phase = role
            with self.subTest(role=role), self.assertRaises(Refused): self.run_policy(apply=True)
        for identity in [{'Account': '000000000000', 'Arn': 'arn:aws:iam::000000000000:user/operator'},
                         {'Account': '877573132739', 'Arn': 'arn:aws:iam::877573132739:root'}]:
            with patch.object(self.aws, 'identity', return_value=identity), self.assertRaises(Refused): self.run_policy(apply=True)
        self.aws.phase = 'operator'
        with patch.dict('os.environ', {'GITHUB_ACTIONS': 'true'}), self.assertRaises(Refused): self.run_policy(apply=True)
        self.assertEqual(self.s3.events, []); self.assert_unchanged()

    def test_stale_budget_basis_refuses_without_refresh_or_reservation(self):
        self.state['ledger']['months']['2026-11']['reconciled_at_utc'] = '2026-10-31T00:00:00Z'
        self.replace_state()
        with self.assertRaisesRegex(ValueError, 'reconciled within'): self.run_policy(apply=True)
        self.assertEqual(self.s3.events, []); self.assert_unchanged()

    def test_near_expiry_refuses_renewal_instead_of_extending(self):
        self.state['live']['config']['deployment_expires_at_utc'] = '2026-11-03T12:00:00Z'
        self.replace_state()
        with self.assertRaisesRegex(Refused, 'renewal required'): self.run_policy(apply=True)
        self.assertEqual(self.s3.events, []); self.assert_unchanged()

    def test_cap_drift_and_over_budget_cannot_be_admitted(self):
        for field in ['budget_genui_cents', 'budget_total_cents']:
            self.setUp(); self.state['policy'][field] += 1; self.replace_state()
            with self.subTest(field=field), self.assertRaisesRegex(Refused, 'canonical policy caps'):
                self.run_policy(apply=True)
            self.assertEqual(self.s3.events, []); self.assert_unchanged()
        self.setUp()
        self.state['live']['parameters']['MonthlyBudgetUsd'] = '200'; self.replace_state()
        with self.assertRaisesRegex(Refused, 'budget must remain'): self.run_policy(apply=True)
        self.setUp()
        self.state['ledger']['months']['2026-11']['prior_extra_spend_cents'] = 20000; self.replace_state()
        with self.assertRaisesRegex(ValueError, 'budget exceeded'): self.run_policy(apply=True)
        self.assertEqual(self.s3.events, []); self.assert_unchanged()
        self.setUp()
        budget = preflight(); reserve = budget.reserve
        def raised_caps(*args):
            ledger, receipt = reserve(*args); receipt['monthly_budget_cents'] += 1
            return ledger, receipt
        budget.reserve = raised_caps
        with patch.object(expiry_policy, 'preflight', return_value=budget), self.assertRaisesRegex(Refused, 'caps must remain'):
            self.run_policy(apply=True)
        self.assertEqual(self.s3.events, []); self.assert_unchanged()

    def test_unhealthy_or_pending_infrastructure_refuses(self):
        self.aws.running_health = 'UNHEALTHY'
        with self.assertRaises(Refused): self.run_policy(apply=True)
        self.aws.running_health = 'HEALTHY'; read = self.runner.read
        def pending(service, operation, **kwargs):
            if operation == 'list_change_sets':
                return {'Summaries': [{'Status': 'CREATE_COMPLETE', 'ExecutionStatus': 'AVAILABLE'}]}
            return read(service, operation, **kwargs)
        with patch.object(self.runner, 'read', side_effect=pending), self.assertRaisesRegex(Refused, 'pending change set'):
            self.run_policy(apply=True)
        self.assertEqual(self.s3.events, []); self.assert_unchanged()

    def test_state_race_during_observations_stops_before_receipt(self):
        original = self.runner.boundary; calls = 0
        def boundary(*args):
            nonlocal calls
            result = original(*args); calls += 1
            if calls == 2:
                other = Store(self.s3, 'bucket', 'beta'); other.load()
                other.change(lambda state: state.update(active={'owner': 'new-release'}))
            return result
        with patch.object(self.runner, 'boundary', side_effect=boundary), self.assertRaises(Refused): self.run_policy(apply=True)
        self.assertEqual([kind for kind, _ in self.s3.events], ['state'])
        self.assertEqual(self.store.load()['active'], {'owner': 'new-release'})

    def test_receipt_failure_or_unknown_acceptance_never_attempts_cas_or_retries(self):
        self.s3.fail_receipt = True
        with self.assertRaises(TimeoutError): self.run_policy(apply=True)
        self.assertEqual([kind for kind, _ in self.s3.events], ['receipt']); self.assert_unchanged()
        self.setUp(); self.s3.fail_after_write = True
        with self.assertRaises(TimeoutError): self.run_policy(apply=True)
        self.assert_unchanged()
        with self.assertRaisesRegex(RuntimeError, '412'): self.run_policy(apply=True)
        self.assertEqual([kind for kind, _ in self.s3.events], ['receipt', 'receipt']); self.assert_unchanged()

    def test_unknown_cas_is_not_replayed_and_old_review_cannot_be_reused(self):
        original = self.store.save
        def lose_ack(state):
            self.s3.fail_after_write = True
            original(state)
        with patch.object(self.store, 'save', side_effect=lose_ack), self.assertRaises(TimeoutError): self.run_policy(apply=True)
        after = self.store.load()
        self.assertEqual(after['policy']['shorten_expiry_hours'], 18)
        self.assertEqual(after['ledger'], self.state['ledger'])
        with self.assertRaises(Refused): self.run_policy(apply=True)
        self.assertEqual([kind for kind, _ in self.s3.events], ['receipt', 'state'])

    def test_failed_readback_leaves_intent_and_refuses_replay(self):
        original = self.store.save
        def unreadable_after_save(state):
            original(state)
            self.store.load = lambda: (_ for _ in ()).throw(TimeoutError('lost readback'))
        with patch.object(self.store, 'save', side_effect=unreadable_after_save), self.assertRaises(TimeoutError): self.run_policy(apply=True)
        self.store = Store(self.s3, 'bucket', 'beta'); self.runner.store = self.store
        self.assertEqual(self.store.load()['revision'], self.state['revision'] + 1)
        with self.assertRaises(Refused): self.run_policy(apply=True)
        self.assertEqual([kind for kind, _ in self.s3.events], ['receipt', 'state'])

    def test_racing_cas_cannot_overwrite_new_lease(self):
        original = self.store.save
        def race(state):
            other = Store(self.s3, 'bucket', 'beta'); other.load()
            other.change(lambda value: value.update(active={'owner': 'new-release'}))
            original(state)
        with patch.object(self.store, 'save', side_effect=race), self.assertRaisesRegex(RuntimeError, '412'): self.run_policy(apply=True)
        self.assertEqual(self.store.load()['active'], {'owner': 'new-release'})
        self.assertEqual(self.store.load()['policy'], self.state['policy'])
        self.assertEqual(self.store.load()['ledger'], self.state['ledger'])

    def cli(self, output, extra=()):
        return patch('sys.argv', ['expiry_policy', '--state-bucket', 'bucket', '--state-prefix', 'beta',
            '--hours', '18', '--expected-state-sha256', self.before, '--output', str(output), *extra])

    def test_output_path_errors_and_existing_or_racing_file_precede_aws(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); existing = root / 'existing.json'; existing.write_text('prior evidence')
            for output in [root / 'absent-parent' / 'out.json', existing, root]:
                with self.subTest(output=output), self.cli(output, ['--apply']), \
                     patch.object(expiry_policy, 'AWS') as aws, contextlib.redirect_stderr(io.StringIO()):
                    self.assertEqual(expiry_policy.main(), 1); aws.assert_not_called()
            self.assertEqual(existing.read_text(), 'prior evidence')
        self.assertEqual(self.s3.events, []); self.assert_unchanged()

    def test_cli_output_is_private_before_aws_and_retains_sanitized_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'failed.json'
            def unavailable():
                self.assertTrue(output.exists())
                self.assertEqual(stat.S_IMODE(output.stat().st_mode), 0o600)
                raise RuntimeError('private operator value must never reach artifact')
            with self.cli(output, ['--apply']), patch.object(expiry_policy, 'AWS', side_effect=unavailable), \
                 contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(expiry_policy.main(), 1)
            value = json.loads(output.read_text())
            self.assertEqual(value['status'], 'refused-or-outcome-unknown')
            self.assertNotIn('private operator', output.read_text())
        self.assertEqual(self.s3.events, []); self.assert_unchanged()

    def test_cli_plan_and_apply_outputs_match_the_actual_result(self):
        for apply in [False, True]:
            self.setUp()
            with tempfile.TemporaryDirectory() as directory:
                output = Path(directory) / 'result.json'; stdout = io.StringIO()
                # Keep actual Runner, Store and command dispatch; only supply
                # offline AWS transport and the deterministic campaign clock.
                self.aws.client = lambda name: self.s3
                def runner(aws, store): return Runner(aws, store, clock=lambda: NOW)
                with self.cli(output, ['--apply'] if apply else []), \
                     patch.object(expiry_policy, 'AWS', return_value=self.aws), \
                     patch.object(expiry_policy, 'Runner', side_effect=runner), contextlib.redirect_stdout(stdout):
                    self.assertEqual(expiry_policy.main(), 0)
                value = json.loads(output.read_text())
                self.assertEqual(stat.S_IMODE(output.stat().st_mode), 0o600)
                self.assertEqual(json.loads(stdout.getvalue()), value['public'])
                self.assertEqual(value['public']['status'], 'applied' if apply else 'planned')
                self.assertEqual(self.store.load()['revision'], self.state['revision'] + int(apply))


if __name__ == '__main__':
    unittest.main()
