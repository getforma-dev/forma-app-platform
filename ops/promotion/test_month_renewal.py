"""Offline qualification of the pinned December ledger month: create, re-assess, never lower, never after Dec 1."""
import copy
from datetime import datetime, timedelta, timezone
import json
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch

from ops.promotion import month_renewal
from ops.promotion.runner import Runner, preflight
from ops.promotion.state import Refused, Store, digest, encoded
from ops.promotion.test_runner import FakeAWS, fixture
from ops.promotion.test_state import MemoryS3

CLOCK = datetime(2026, 11, 21, 12, tzinfo=timezone.utc)


class MonthRenewalTests(unittest.TestCase):
    def setUp(self):
        environment = patch.dict('os.environ', {'GITHUB_ACTIONS': 'false'})
        environment.start(); self.addCleanup(environment.stop)
        manifest = {'source': {'schema_manifest_sha256': 'a' * 64, 'deployment_contract_sha256': 'b' * 64}}
        self.state = fixture(manifest)
        self.state['policy'].update(budget_genui_cents=18000, budget_total_cents=21800, shorten_expiry_hours=18)
        self.state['ledger']['months']['2026-11']['reconciled_at_utc'] = (CLOCK - timedelta(hours=1)).strftime('%Y-%m-%dT%H:%M:%SZ')
        self.s3 = MemoryS3(self.state)
        self.store = Store(self.s3, 'bucket', 'beta')
        self.aws = FakeAWS(self.state, self.s3, manifest)
        self.aws.phase = 'operator'
        self.runner = Runner(self.aws, self.store, clock=lambda: CLOCK)
        self.before = digest(self.state)
        self.assessment = {'month': '2026-12', 'ledger_sha256': digest(self.state['ledger']),
                           'rationale': 'December forward reservation for the real-user trial through the Dec 16 stop; covers probes, logs and billing lag.',
                           'amounts': {'prior_extra_spend_cents': 0, 'variable_reserve_cents': 1500, 'auth_spent_or_reserved_cents': 3800, 'unallocated_cents': 0}}

    def replace_state(self):
        self.s3.objects['beta/state.json'] = encoded(self.state)
        self.aws.state = copy.deepcopy(self.state)
        self.before = digest(self.state)
        self.assessment['ledger_sha256'] = digest(self.state['ledger'])

    def run_month(self, apply=False, assessment=None, expected=None):
        return month_renewal.run(self.runner, expected or self.before, assessment or self.assessment, apply)

    def assert_unchanged(self):
        self.assertEqual(self.s3.objects['beta/state.json'], encoded(self.state))
        self.assertFalse(any(k.startswith('beta/promotions/') for k in self.s3.objects))

    def test_plan_is_read_only_and_shows_decembers_full_month_projection_beside_the_trial_ceiling(self):
        result = self.run_month()
        self.assert_unchanged()
        public = result['public']
        self.assertEqual((public['status'], public['month'], public['created']), ('planned', '2026-12', True))
        budget = preflight()
        retained = budget.cents(budget.FIXED_HOURLY * 31 * 24 + budget.FIXED_STORAGE_KEYS)
        self.assertEqual(public['december_projection'], {'retained_core_full_month_cents': retained, 'genui_total_cents': retained + 1500,
                                                         'project_total_cents': retained + 1500 + 3800})
        self.assertEqual(public['trial_ceiling_cents'], {'genui': 8000, 'project': 11800})
        self.assertEqual((public['genui_cap_cents'], public['project_cap_cents']), (18000, 21800))
        self.assertEqual(self.aws.calls, [])  # no boundary read: the month is a ledger fact, as in reconcile
        self.assertNotIn(self.state['live']['window_id'], json.dumps(public))

    def test_apply_adds_the_month_with_one_intent_and_one_cas_and_keeps_every_window(self):
        result = self.run_month(apply=True)
        actual = self.store.load()
        entry = actual['ledger']['months']['2026-12']
        self.assertEqual({k: entry[k] for k in month_renewal.AMOUNTS}, self.assessment['amounts'])
        self.assertEqual(entry['reconciled_at_utc'], CLOCK.strftime('%Y-%m-%dT%H:%M:%SZ'))
        self.assertTrue(entry['basis'].startswith('Operator forward reservation (ADR-019 amendment 2026-11-14'))
        self.assertIn(self.assessment['rationale'], entry['basis'])
        self.assertEqual(actual['ledger']['windows'], self.state['ledger']['windows'])
        self.assertEqual(actual['ledger']['months']['2026-11'], self.state['ledger']['months']['2026-11'])
        self.assertEqual((actual['live'], actual['policy'], actual['revision']), (self.state['live'], self.state['policy'], self.state['revision'] + 1))
        receipts = [k for k in self.s3.objects if k.startswith('beta/promotions/month-renewal-')]
        self.assertEqual(len(receipts), 1)
        receipt = json.loads(self.s3.objects[receipts[0]])
        self.assertEqual((receipt['month'], receipt['previous_entry'], receipt['expected_after_state_sha256']), ('2026-12', None, digest(actual)))
        self.assertEqual(result['public']['status'], 'applied')
        # The month now satisfies preflight for a window reaching into December.
        config = copy.deepcopy(self.state['live']['config'])
        config.update(window_id='11111111-2222-4333-8444-555555555555', supersedes_window_id=self.state['live']['window_id'],
                      deployment_expires_at_utc='2026-12-15T14:00:00Z', service_desired_count=0, extra_task_hours=2)
        _, report = preflight().reserve(config, actual['ledger'], CLOCK)
        self.assertEqual(set(report['cost_by_month']), {'2026-11', '2026-12'})

    def test_reassessment_moves_the_timestamp_but_never_lowers_an_amount(self):
        self.run_month(apply=True)
        # Two days later, after November was reconciled again (reconcile's job; here written in place).
        renewed = self.store.load()
        renewed['ledger']['months']['2026-11']['reconciled_at_utc'] = (CLOCK + timedelta(days=2) - timedelta(hours=1)).strftime('%Y-%m-%dT%H:%M:%SZ')
        self.s3.objects['beta/state.json'] = encoded(renewed)
        later = Runner(self.aws, self.store, clock=lambda: CLOCK + timedelta(days=2))
        higher = copy.deepcopy(self.assessment)
        higher.update(ledger_sha256=digest(renewed['ledger']), rationale='Re-assessed before release B: a higher variable reserve for the class weekend logs and probes.')
        higher['amounts']['variable_reserve_cents'] = 2000
        result = month_renewal.run(later, digest(renewed), higher, apply=True)
        entry = self.store.load()['ledger']['months']['2026-12']
        self.assertEqual((entry['variable_reserve_cents'], entry['reconciled_at_utc']), (2000, (CLOCK + timedelta(days=2)).strftime('%Y-%m-%dT%H:%M:%SZ')))
        self.assertFalse(result['public']['created'])
        lower = copy.deepcopy(higher)
        lower.update(ledger_sha256=digest(self.store.load()['ledger']))
        lower['amounts']['variable_reserve_cents'] = 1500
        with self.assertRaises(Refused):
            month_renewal.run(later, digest(self.store.load()), lower, apply=True)
        self.assertEqual(self.store.load()['ledger']['months']['2026-12']['variable_reserve_cents'], 2000)

    def test_refusals_before_any_write(self):
        def held(): self.state['active'] = {'owner': 'x'}; self.replace_state()
        def stale(): self.before = '0' * 64
        def workflow(): self.aws.phase = 'prepare'
        def other_month(): self.assessment['month'] = '2026-11'
        def january(): self.assessment['month'] = '2027-01'
        def other_ledger(): self.assessment['ledger_sha256'] = '0' * 64
        def short_rationale(): self.assessment['rationale'] = 'too short'
        def control_character(): self.assessment['rationale'] = 'x' * 40 + '\n'
        def low_reserve(): self.assessment['amounts']['variable_reserve_cents'] = 999
        def auth_low(): self.assessment['amounts']['auth_spent_or_reserved_cents'] = 3299
        def auth_high(): self.assessment['amounts']['auth_spent_or_reserved_cents'] = 3801
        def negative(): self.assessment['amounts']['prior_extra_spend_cents'] = -1
        def float_cents(): self.assessment['amounts']['unallocated_cents'] = 10.0
        def extra_field(): self.assessment['amounts']['extra'] = 1
        def missing_field(): del self.assessment['amounts']['unallocated_cents']
        def over_cap(): self.assessment['amounts']['unallocated_cents'] = 20000
        def december_begun(): self.runner = Runner(self.aws, self.store, clock=lambda: datetime(2026, 12, 1, 0, 0, 1, tzinfo=timezone.utc))
        def stale_november():
            self.state['ledger']['months']['2026-11']['reconciled_at_utc'] = '2026-11-19T00:00:00Z'; self.replace_state()
        def github(): import os; os.environ['GITHUB_ACTIONS'] = 'true'
        for name, prepare in [('held lock', held), ('stale hash', stale), ('workflow identity', workflow), ('other month', other_month), ('january', january),
                              ('other ledger', other_ledger), ('short rationale', short_rationale), ('control character', control_character),
                              ('low variable reserve', low_reserve), ('auth below', auth_low), ('auth above', auth_high), ('negative', negative),
                              ('float cents', float_cents), ('extra amount field', extra_field), ('missing amount field', missing_field),
                              ('over the cap', over_cap), ('december begun', december_begun), ('stale november', stale_november), ('github actions', github)]:
            with self.subTest(case=name):
                self.setUp(); prepare()
                with self.assertRaises(Refused):
                    self.run_month(apply=True)
                self.assert_unchanged()

    def test_once_december_begins_reconcile_owns_the_month(self):
        """A live window reaching into December, both months fresh, clock Dec 1 00:00:01: every other check
        would pass (the month exists, amounts do not drop, the live window validates), so only the
        month-begun rule stops a forward reservation from overwriting a month reconcile now owns."""
        clock = datetime(2026, 12, 1, 0, 0, 1, tzinfo=timezone.utc)
        reserved_at = datetime(2026, 11, 23, 12, tzinfo=timezone.utc)
        config = copy.deepcopy(self.state['live']['config'])
        config['deployment_expires_at_utc'] = '2026-12-15T14:00:00Z'
        ledger = copy.deepcopy(self.state['ledger'])
        ledger['windows'] = {}
        for month in ('2026-11', '2026-12'):
            ledger['months'][month] = dict(ledger['months']['2026-11'], reconciled_at_utc=(reserved_at - timedelta(hours=1)).strftime('%Y-%m-%dT%H:%M:%SZ'))
        ledger, reserved = preflight().reserve(config, ledger, reserved_at, activating=True)
        for month in ('2026-11', '2026-12'):  # reconciled again just before the month boundary
            ledger['months'][month]['reconciled_at_utc'] = (clock - timedelta(hours=1)).strftime('%Y-%m-%dT%H:%M:%SZ')
        self.state['ledger'] = ledger
        self.state['live']['config'] = config
        self.state['live']['parameters'] = reserved['cloudformation_parameters']
        self.replace_state()
        self.assessment['ledger_sha256'] = digest(self.state['ledger'])
        self.assessment['amounts']['variable_reserve_cents'] = 1600  # not lower than the existing 1500
        self.runner = Runner(self.aws, self.store, clock=lambda: clock)
        with self.assertRaisesRegex(Refused, 'the month has begun'):
            self.run_month(apply=True)
        self.assert_unchanged()
        # Half an hour before the boundary, the same assessment re-assesses the month.
        self.runner = Runner(self.aws, self.store, clock=lambda: clock - timedelta(minutes=30))
        self.assertEqual(self.run_month(apply=True)['public']['status'], 'applied')

    def test_clean_trial_windows_add_the_retained_trial_figure(self):
        window = self.state['ledger']['windows'][self.state['live']['window_id']]
        window['clean_trial_organization_id'] = '7f2a9c4e-1b3d-4a5e-8f60-123456789abc'
        self.replace_state()
        budget = preflight()
        projection = self.run_month()['public']['december_projection']
        self.assertEqual(projection['retained_core_full_month_cents'], budget.cents(budget.FIXED_HOURLY * 31 * 24 + budget.FIXED_STORAGE_KEYS + budget.CLEAN_RETAINED_MONTHLY))

    def test_cli_reserves_a_private_output_and_never_overwrites(self):
        with tempfile.TemporaryDirectory() as directory:
            path, assessment = Path(directory) / 'plan.json', Path(directory) / 'assessment.json'
            assessment.write_text(json.dumps(self.assessment))
            with patch.object(month_renewal, 'AWS', side_effect=AssertionError('no AWS')):
                code = month_renewal.main(['--state-bucket', 'b', '--state-prefix', 'beta', '--expected-state-sha256', self.before,
                                          '--assessment', str(assessment), '--output', str(path)])
            self.assertEqual(code, 1)
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
            self.assertEqual(json.loads(path.read_bytes())['status'], 'refused-or-outcome-unknown')
            code = month_renewal.main(['--state-bucket', 'b', '--state-prefix', 'beta', '--expected-state-sha256', self.before,
                                      '--assessment', str(assessment), '--output', str(path)])
            self.assertEqual(code, 1)


if __name__ == '__main__':
    unittest.main()
