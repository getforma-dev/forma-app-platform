"""Offline qualification of the pinned expiry renewal with the actual boundary, CAS and budget code."""
import copy
from datetime import datetime, timedelta, timezone
import json
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch

from ops.promotion import expiry_renewal, month_renewal
from ops.promotion.runner import Runner
from ops.promotion.state import Refused, Store, digest, encoded
from ops.promotion.test_runner import FakeAWS, fixture
from ops.promotion.test_state import MemoryS3

CLOCK = datetime(2026, 11, 21, 12, tzinfo=timezone.utc)
RENEWED = '2026-12-16T08:00:00Z'


def december(ledger, when):
    ledger['months']['2026-12'] = {'reconciled_at_utc': when.strftime('%Y-%m-%dT%H:%M:%SZ'), 'basis': 'Synthetic December fixture',
                                   'prior_extra_spend_cents': 0, 'variable_reserve_cents': 1500, 'auth_spent_or_reserved_cents': 3800, 'unallocated_cents': 0}


class ExpiryRenewalTests(unittest.TestCase):
    def setUp(self):
        environment = patch.dict('os.environ', {'GITHUB_ACTIONS': 'false'})
        environment.start(); self.addCleanup(environment.stop)
        manifest = {'source': {'schema_manifest_sha256': 'a' * 64, 'deployment_contract_sha256': 'b' * 64}}
        self.state = fixture(manifest)
        self.state['policy'].update(budget_genui_cents=21000, budget_total_cents=24800, shorten_expiry_hours=18)
        # The fixture's live window stops 2026-11-30 23:00; the trial's clock is Nov 21 and both months are fresh.
        self.state['ledger']['months']['2026-11']['reconciled_at_utc'] = (CLOCK - timedelta(hours=1)).strftime('%Y-%m-%dT%H:%M:%SZ')
        december(self.state['ledger'], CLOCK - timedelta(hours=1))
        self.s3 = MemoryS3(self.state)
        self.store = Store(self.s3, 'bucket', 'beta')
        self.aws = FakeAWS(self.state, self.s3, manifest)
        self.aws.phase = 'operator'
        self.runner = Runner(self.aws, self.store, clock=lambda: CLOCK)
        self.before = digest(self.state)

    def replace_state(self):
        self.s3.objects['beta/state.json'] = encoded(self.state)
        self.aws.state = copy.deepcopy(self.state)
        self.before = digest(self.state)

    def run_renewal(self, apply=False, revert=False, expected=None):
        return expiry_renewal.run(self.runner, expected or self.before, apply, revert)

    def assert_unchanged(self):
        self.assertEqual(self.s3.objects['beta/state.json'], encoded(self.state))

    def test_plan_is_read_only_and_projects_the_next_release_into_both_months(self):
        result = self.run_renewal()
        self.assert_unchanged()
        self.assertFalse(any(k.startswith('beta/promotions/') for k in self.s3.objects))
        public = result['public']
        self.assertEqual((public['status'], public['before_expiry'], public['after_expiry']), ('planned', '2026-11-30T23:00:00Z', RENEWED))
        self.assertEqual(public['live_stop_unchanged'], '2026-11-30T23:00:00Z')
        self.assertEqual(public['next_release_expiry'], '2026-12-15T14:00:00Z')
        self.assertEqual(set(public['cost_by_month']), {'2026-11', '2026-12'})
        self.assertTrue(all(c['genui_total_cents'] <= 21000 and c['project_total_cents'] <= 24800 for c in public['cost_by_month'].values()))
        self.assertEqual((public['genui_cap_cents'], public['project_cap_cents']), (21000, 24800))
        self.assertFalse(public['ledger_changed']); self.assertFalse(public['live_parameters_changed'])
        self.assertEqual(result['receipt']['projection_config']['supersedes_window_id'], self.state['live']['window_id'])
        self.assertEqual(result['receipt']['window_recorded_expiry'], '2026-11-30T23:00:00Z')
        text = json.dumps(public)
        for private in ['arn:', '877573132739', 'beta.getforma.dev', self.state['live']['window_id']]:
            self.assertNotIn(private, text)
        self.assertEqual(expiry_renewal.RENEWED_EXPIRY, RENEWED)

    def test_apply_writes_one_intent_then_one_cas_changing_only_the_canonical_expiry(self):
        result = self.run_renewal(apply=True)
        expected = copy.deepcopy(self.state)
        expected['live']['config']['deployment_expires_at_utc'] = RENEWED
        expected['revision'] += 1
        actual = self.store.load()
        self.assertEqual(actual, expected)
        self.assertEqual(actual['live']['parameters']['DeploymentExpiresAtUtc'], '2026-11-30T23:00:00Z')
        self.assertEqual(actual['ledger'], self.state['ledger'])
        receipts = [k for k in self.s3.objects if k.startswith('beta/promotions/expiry-renewal-')]
        self.assertEqual(len(receipts), 1)
        receipt = json.loads(self.s3.objects[receipts[0]])
        self.assertEqual((receipt['before_expiry'], receipt['after_expiry'], receipt['expected_after_state_sha256']), ('2026-11-30T23:00:00Z', RENEWED, digest(expected)))
        self.assertEqual((result['public']['status'], result['public']['state_sha256']), ('applied', digest(expected)))
        self.assertTrue(all(op not in ['update_service', 'execute_change_set', 'create_change_set', 'put_image'] for _, op, _ in self.aws.calls))
        # A second renewal of the same state refuses: the config no longer equals its window.
        with self.assertRaises(Refused):
            expiry_renewal.run(self.runner, digest(actual), True)
        self.assertEqual(self.store.load(), expected)

    def test_revert_returns_exactly_to_the_windows_recorded_expiry(self):
        self.run_renewal(apply=True)
        renewed = self.store.load()
        with self.assertRaises(Refused):  # a forward renewal cannot run on the renewed state
            expiry_renewal.run(self.runner, digest(renewed), False, False)
        result = expiry_renewal.run(self.runner, digest(renewed), True, True)
        reverted = self.store.load()
        self.assertEqual(reverted['live']['config']['deployment_expires_at_utc'], '2026-11-30T23:00:00Z')
        self.assertEqual(reverted['revision'], self.state['revision'] + 2)
        self.assertEqual((result['public']['operation'], result['public']['after_expiry']), ('revert-expiry-renewal', '2026-11-30T23:00:00Z'))
        self.assertIsNone(result['public']['next_release_expiry'])
        with self.assertRaises(Refused):  # nothing left to revert
            expiry_renewal.run(self.runner, digest(reverted), False, True)

    def test_refusals_before_any_write(self):
        def held(): self.state['active'] = {'owner': 'x'}; self.replace_state()
        def stale(): self.before = '0' * 64
        def workflow(): self.aws.phase = 'execute'
        def parameters_disagree(): self.state['live']['parameters']['DeploymentExpiresAtUtc'] = '2026-11-29T23:00:00Z'; self.replace_state()
        def no_december(): del self.state['ledger']['months']['2026-12']; self.replace_state()
        def stale_december(): self.state['ledger']['months']['2026-12']['reconciled_at_utc'] = '2026-11-19T00:00:00Z'; self.replace_state()
        def stale_november(): self.state['ledger']['months']['2026-11']['reconciled_at_utc'] = '2026-11-19T00:00:00Z'; self.replace_state()
        def caps_differ(): self.state['policy']['budget_genui_cents'] = 18000; self.replace_state()  # the pair before the trial raise
        def pending_change_set():
            original = self.aws._call
            def call(s, o, a):
                if o == 'list_change_sets': return {'Summaries': [{'ChangeSetName': 'x', 'Status': 'CREATE_COMPLETE', 'ExecutionStatus': 'AVAILABLE'}]}
                return original(s, o, a)
            self.aws._call = call
        def unhealthy(): self.aws.running_health = 'UNHEALTHY'
        def too_far(): self.runner = Runner(self.aws, self.store, clock=lambda: datetime(2026, 11, 10, tzinfo=timezone.utc))
        def budget_exceeded(): self.state['ledger']['months']['2026-12']['unallocated_cents'] = 13000; self.replace_state()
        def already_renewed(): self.state['live']['config']['deployment_expires_at_utc'] = '2026-12-10T00:00:00Z'; self.replace_state()
        for name, prepare in [('held lock', held), ('stale hash', stale), ('workflow identity', workflow), ('parameters disagree', parameters_disagree),
                              ('no december month', no_december), ('stale december', stale_december), ('stale november', stale_november),
                              ('caps differ', caps_differ), ('pending change set', pending_change_set), ('unhealthy runtime', unhealthy),
                              ('renewal beyond 32 days', too_far), ('budget exceeded', budget_exceeded), ('config already differs', already_renewed)]:
            with self.subTest(case=name):
                self.setUp(); prepare()
                with self.assertRaises(Refused):
                    self.run_renewal(apply=True)
                self.assert_unchanged()
                self.assertFalse(any(k.startswith('beta/promotions/') for k in self.s3.objects))

    def test_a_next_release_within_24_hours_refuses_even_with_a_fresh_december(self):
        """Dec 14 20:00: the renewed expiry minus 18 hours is Dec 15 14:00, under a day away; only this
        command's own check refuses (preflight accepts a window ten minutes ahead)."""
        clock = datetime(2026, 12, 14, 20, tzinfo=timezone.utc)
        self.state['ledger']['months']['2026-12']['reconciled_at_utc'] = (clock - timedelta(hours=1)).strftime('%Y-%m-%dT%H:%M:%SZ')
        self.replace_state()
        self.runner = Runner(self.aws, self.store, clock=lambda: clock)
        with self.assertRaisesRegex(Refused, 'next release would still refuse'):
            self.run_renewal(apply=True)
        self.assert_unchanged()

    def test_cli_reserves_a_private_output_and_never_overwrites(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'plan.json'
            with patch.object(expiry_renewal, 'AWS', side_effect=AssertionError('no AWS')):
                code = expiry_renewal.main(['--state-bucket', 'b', '--state-prefix', 'beta', '--expected-state-sha256', self.before, '--output', str(path)])
            self.assertEqual(code, 1)
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
            self.assertEqual(json.loads(path.read_bytes())['status'], 'refused-or-outcome-unknown')
            code = expiry_renewal.main(['--state-bucket', 'b', '--state-prefix', 'beta', '--expected-state-sha256', self.before, '--output', str(path)])
            self.assertEqual(code, 1)

    def test_month_renewal_then_expiry_renewal_is_the_documented_order(self):
        """Without December the renewal refuses; after month_renewal applies, it plans and applies."""
        del self.state['ledger']['months']['2026-12']; self.replace_state()
        with self.assertRaises(Refused):
            self.run_renewal()
        assessment = {'month': '2026-12', 'ledger_sha256': digest(self.state['ledger']),
                      'rationale': 'December forward reservation for the real-user trial through the Dec 16 stop; covers probes, logs and lag.',
                      'amounts': {'prior_extra_spend_cents': 0, 'variable_reserve_cents': 1500, 'auth_spent_or_reserved_cents': 3800, 'unallocated_cents': 0}}
        month_renewal.run(self.runner, self.before, assessment, apply=True)
        after_month = self.store.load()
        result = expiry_renewal.run(self.runner, digest(after_month), apply=True)
        self.assertEqual(result['public']['status'], 'applied')
        self.assertEqual(self.store.load()['live']['config']['deployment_expires_at_utc'], RENEWED)


if __name__ == '__main__':
    unittest.main()
