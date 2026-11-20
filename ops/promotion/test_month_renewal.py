"""Offline qualification of the pinned ledger months (December and January): create, re-assess, never lower, never once begun."""
import copy
from datetime import datetime, timedelta, timezone
import json
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch

from ops.promotion import month_renewal, reconcile
from ops.promotion.guards import ACCOUNT
from ops.promotion.runner import Runner, preflight
from ops.promotion.state import Refused, Store, digest, encoded
from ops.promotion.test_runner import FakeAWS, RESERVES, fixture, live_window, reconciled
from ops.promotion.test_state import MemoryS3

# Inside the release-H window (Dec 2-9, 2026); the live stop is release F's, 2026-12-11 20:00 UTC.
CLOCK = datetime(2026, 12, 2, 12, tzinfo=timezone.utc)
# Release G's week, under the same live window.
NOVEMBER_CLOCK = datetime(2026, 11, 20, 12, tzinfo=timezone.utc)
LIVE_STOP = '2026-12-11T20:00:00Z'
LIVE_RESERVED_AT = datetime(2026, 11, 15, 20, tzinfo=timezone.utc)
NEXT_RELEASE = '2027-01-15T14:00:00Z'
RATIONALE = 'January forward reservation for the class continuation through the Jan 15 stop; covers probes, logs and billing lag.'
DECEMBER_RATIONALE = 'December re-assessed before release G: the whole month to the Jan 15 stop; covers probes, logs and billing lag.'


def billing_observation(clock, amount='12.34'):
    """A synthetic Cost Explorer observation of the clock's month to its last completed day, in reconcile's shape."""
    observed = clock - timedelta(minutes=30)
    query = {'TimePeriod': {'Start': observed.strftime('%Y-%m-01'), 'End': observed.strftime('%Y-%m-%d')},
             'Granularity': 'MONTHLY', 'Metrics': ['UnblendedCost'],
             'Filter': {'Dimensions': {'Key': 'LINKED_ACCOUNT', 'Values': [ACCOUNT]}}}
    return {'schema_version': 1, 'account': ACCOUNT, 'observed_at_utc': observed.strftime('%Y-%m-%dT%H:%M:%SZ'), 'query': query,
            'response': {'ResponseMetadata': {'HTTPStatusCode': 200, 'RequestId': 'synthetic'},
                         'ResultsByTime': [{'TimePeriod': query['TimePeriod'], 'Estimated': True,
                                            'Total': {'UnblendedCost': {'Unit': 'USD', 'Amount': amount}}}]}}


class MonthRenewalTests(unittest.TestCase):
    def setUp(self):
        environment = patch.dict('os.environ', {'GITHUB_ACTIONS': 'false'})
        environment.start(); self.addCleanup(environment.stop)
        manifest = {'source': {'schema_manifest_sha256': 'a' * 64, 'deployment_contract_sha256': 'b' * 64}}
        self.state = fixture(manifest)
        self.state['policy'].update(budget_genui_cents=21000, budget_total_cents=24800, shorten_expiry_hours=18)
        live_window(self.state, LIVE_STOP, LIVE_RESERVED_AT)
        reconciled(self.state['ledger'], '2026-12', CLOCK)  # December has begun: reconcile owns it, and it is fresh
        self.s3 = MemoryS3(self.state)
        self.store = Store(self.s3, 'bucket', 'beta')
        self.aws = FakeAWS(self.state, self.s3, manifest)
        self.aws.phase = 'operator'
        self.runner = Runner(self.aws, self.store, clock=lambda: CLOCK)
        self.before = digest(self.state)
        self.assessment = {'month': '2027-01', 'ledger_sha256': digest(self.state['ledger']), 'rationale': RATIONALE, 'amounts': dict(RESERVES)}

    def replace_state(self):
        self.s3.objects['beta/state.json'] = encoded(self.state)
        self.aws.state = copy.deepcopy(self.state)
        self.before = digest(self.state)
        self.assessment['ledger_sha256'] = digest(self.state['ledger'])

    def at(self, clock):
        self.runner = Runner(self.aws, self.store, clock=lambda: clock)

    def run_month(self, apply=False, assessment=None, expected=None):
        return month_renewal.run(self.runner, expected or self.before, assessment or self.assessment, apply)

    def assert_unchanged(self):
        self.assertEqual(self.s3.objects['beta/state.json'], encoded(self.state))
        self.assertFalse(any(k.startswith('beta/promotions/') for k in self.s3.objects))

    def release(self, window_id, expires_at):
        """A release's reservation input superseding the live window, as runner.prepare builds it."""
        return dict(self.state['live']['config'], window_id=window_id, supersedes_window_id=self.state['live']['window_id'],
                    service_desired_count=0, extra_task_hours=2, deployment_expires_at_utc=expires_at)

    def test_refuses_any_month_outside_the_pin_set(self):
        """MONTHS is a closed pin set with one reviewed ceiling per month (ADR-019 amendment 2), not a free argument."""
        self.assertEqual(month_renewal.MONTHS, {'2026-12': {'genui': 17500, 'project': 21300}, '2027-01': {'genui': 12000, 'project': 15800}})
        for other in ['2026-11', '2027-02', '2026-01', '2028-01', '2027-01-01', '2027-1', ' 2027-01', '', None, 202701]:
            with self.subTest(month=repr(other)):
                self.assessment['month'] = other
                with self.assertRaisesRegex(Refused, 'only a pinned month is renewed here: 2026-12, 2027-01'):
                    self.run_month(apply=True)
                self.assert_unchanged()
        # December is in the set, but under this Dec 2 clock it has begun and reconcile owns it.
        self.assessment['month'] = '2026-12'
        with self.assertRaisesRegex(Refused, 'the month has begun'):
            self.run_month(apply=True)
        self.assert_unchanged()

    def test_december_is_reassessed_under_a_november_clock_at_the_amendment_row(self):
        """Release G's week: December stays in the set and is re-assessed, as releases B to F did, until it begins.
        With the trial named, its projection is amendment 2's row, 17449 / 21249, under the 17500 / 21300 ceiling and the caps."""
        self.state['ledger']['windows'][self.state['live']['window_id']]['clean_trial_organization_id'] = '7f2a9c4e-1b3d-4a5e-8f60-123456789abc'
        for name in ('2026-11', '2026-12'):
            reconciled(self.state['ledger'], name, NOVEMBER_CLOCK)
        self.replace_state(); self.at(NOVEMBER_CLOCK)
        december = dict(self.assessment, month='2026-12', rationale=DECEMBER_RATIONALE)
        public = self.run_month(apply=True, assessment=december)['public']
        self.assertEqual((public['status'], public['month'], public['created']), ('applied', '2026-12', False))
        self.assertEqual(public['month_projection'], {'retained_core_full_month_cents': 5589, 'release_compute_cents': 10360, 'release_stop_utc': NEXT_RELEASE,
                                                      'genui_total_cents': 17449, 'project_total_cents': 21249})
        self.assertEqual(public['trial_ceiling_cents'], {'genui': 17500, 'project': 21300})
        self.assertLessEqual(public['month_projection']['genui_total_cents'], min(17500, public['genui_cap_cents']))
        self.assertLessEqual(public['month_projection']['project_total_cents'], min(21300, public['project_cap_cents']))
        entry = self.store.load()['ledger']['months']['2026-12']
        self.assertEqual(entry['reconciled_at_utc'], NOVEMBER_CLOCK.strftime('%Y-%m-%dT%H:%M:%SZ'))
        self.assertIn(DECEMBER_RATIONALE, entry['basis'])
        receipts = [k for k in self.s3.objects if k.startswith('beta/promotions/month-renewal-')]
        self.assertTrue(receipts[0].endswith('-2026-12/intent.json'))

    def test_from_a_fully_stale_ledger_reconcile_then_reassess_then_a_release_reserves(self):
        """APP-228 review: with both months more than a day stale, reconcile November refused (its validator wanted
        December fresh) and the December re-assessment refused (it wanted November fresh); nothing could be refreshed
        and no release could prepare. A refresh never spends, so its validator requires the other month to exist, not
        to be fresh; `reserve` keeps the 24-hour rule, so release G reserves only after both refreshes."""
        clock = NOVEMBER_CLOCK
        for name in ('2026-11', '2026-12'):  # last reconciled Nov 15 13:14Z, the release-F record: four days stale
            self.state['ledger']['months'][name]['reconciled_at_utc'] = '2026-11-15T13:14:00Z'
        self.replace_state(); self.at(clock)
        budget = preflight()
        release_g = self.release('22222222-2222-4222-8222-222222222222', '2026-12-11T02:00:00Z')
        with self.assertRaisesRegex(ValueError, 'reconciled within the last 24 hours'):
            budget.reserve(release_g, self.state['ledger'], clock)
        # 1. reconcile November with the default validator, December still four days stale.
        observation = billing_observation(clock)
        november = {'month': '2026-11', 'ledger_sha256': digest(self.state['ledger']), 'observation_sha256': digest(observation),
                    'account_total_usd': '12.34', 'billing_lag_and_query_cost_covered': True, 'amounts': dict(RESERVES),
                    'rationale': 'Observed account cost is covered by the fixed and variable reserves, including billing lag and query cost.'}
        self.assertEqual(reconcile.apply(self.store, november, observation, clock, observation)['status'], 'reconciled')
        after_november = self.store.load()
        self.assertEqual(after_november['ledger']['months']['2026-11']['reconciled_at_utc'], observation['observed_at_utc'])
        self.assertEqual(after_november['ledger']['months']['2026-12']['reconciled_at_utc'], '2026-11-15T13:14:00Z')
        self.assertEqual(after_november['ledger']['windows'], self.state['ledger']['windows'])
        # 2. re-assess December (month_renewal); November is fresh now, December stale until this write.
        december = {'month': '2026-12', 'ledger_sha256': digest(after_november['ledger']), 'rationale': DECEMBER_RATIONALE, 'amounts': dict(RESERVES)}
        self.assertEqual(month_renewal.run(self.runner, digest(after_november), december, apply=True)['public']['status'], 'applied')
        ledger = self.store.load()['ledger']
        self.assertEqual(ledger['months']['2026-12']['reconciled_at_utc'], clock.strftime('%Y-%m-%dT%H:%M:%SZ'))
        self.assertEqual(ledger['windows'], self.state['ledger']['windows'])
        # 3. release G reserves: both months fresh, both under the caps.
        _, report = budget.reserve(release_g, ledger, clock)
        self.assertEqual(set(report['cost_by_month']), {'2026-11', '2026-12'})
        self.assertTrue(all(c['genui_total_cents'] <= 21000 and c['project_total_cents'] <= 24800 for c in report['cost_by_month'].values()))

    def test_a_reservation_still_refuses_a_stale_month_that_a_refresh_accepted(self):
        """The refresh validator accepts a stale sibling month; `reserve` does not, for either month."""
        self.state['ledger']['months']['2026-12']['reconciled_at_utc'] = '2026-11-30T12:00:00Z'  # two days stale under the Dec 2 clock
        self.replace_state()
        self.assertEqual(self.run_month(apply=True)['public']['status'], 'applied')  # January added without December fresh
        ledger = self.store.load()['ledger']
        release_h = self.release('11111111-2222-4333-8444-555555555555', NEXT_RELEASE)
        for stale_month in ('2026-12', '2027-01'):
            with self.subTest(stale=stale_month):
                book = copy.deepcopy(ledger)
                reconciled(book, '2026-12', CLOCK)  # December reconciled now, runbook 2.1 step 1
                book['months'][stale_month]['reconciled_at_utc'] = '2026-12-01T11:00:00Z'  # 25 hours
                with self.assertRaisesRegex(ValueError, 'reconciled within the last 24 hours'):
                    preflight().reserve(release_h, book, CLOCK)
        book = copy.deepcopy(ledger); reconciled(book, '2026-12', CLOCK)
        self.assertEqual(set(preflight().reserve(release_h, book, CLOCK)[1]['cost_by_month']), {'2026-12', '2027-01'})

    def test_plan_is_read_only_and_shows_januarys_full_month_projection_beside_the_ceiling(self):
        result = self.run_month()
        self.assert_unchanged()
        public = result['public']
        self.assertEqual((public['status'], public['month'], public['created']), ('planned', '2027-01', True))
        budget = preflight()
        retained = budget.cents(budget.FIXED_HOURLY * 31 * 24 + budget.FIXED_STORAGE_KEYS)
        self.assertEqual(retained, 5229)
        # Release H's January compute: Jan 1 00:00 to the Jan 15 14:00 stop plus the one-hour margin is 351 hours, plus two task-hours.
        compute = budget.cents(353 * budget.TASK_HOURLY)
        self.assertEqual(compute, 4290)
        self.assertEqual(public['month_projection'], {'retained_core_full_month_cents': retained, 'release_compute_cents': compute,
                                                      'release_stop_utc': NEXT_RELEASE, 'genui_total_cents': retained + compute + 1500,
                                                      'project_total_cents': retained + compute + 1500 + 3800})
        self.assertEqual(public['trial_ceiling_cents'], {'genui': 12000, 'project': 15800})
        self.assertLessEqual(public['month_projection']['genui_total_cents'], 12000)
        self.assertLessEqual(public['month_projection']['project_total_cents'], 15800)
        self.assertEqual((public['genui_cap_cents'], public['project_cap_cents']), (21000, 24800))
        self.assertEqual(self.aws.calls, [])  # no boundary read: the month is a ledger fact, as in reconcile
        self.assertNotIn(self.state['live']['window_id'], json.dumps(public))

    def test_apply_adds_the_month_with_one_intent_and_one_cas_and_keeps_every_window(self):
        result = self.run_month(apply=True)
        actual = self.store.load()
        entry = actual['ledger']['months']['2027-01']
        self.assertEqual({k: entry[k] for k in month_renewal.AMOUNTS}, self.assessment['amounts'])
        self.assertEqual(entry['reconciled_at_utc'], CLOCK.strftime('%Y-%m-%dT%H:%M:%SZ'))
        self.assertTrue(entry['basis'].startswith('Operator forward reservation (ADR-019 amendment 2, 2026-11-15'))
        self.assertIn(self.assessment['rationale'], entry['basis'])
        self.assertEqual(actual['ledger']['windows'], self.state['ledger']['windows'])
        for month in ('2026-11', '2026-12'):
            self.assertEqual(actual['ledger']['months'][month], self.state['ledger']['months'][month])
        self.assertEqual((actual['live'], actual['policy'], actual['revision']), (self.state['live'], self.state['policy'], self.state['revision'] + 1))
        receipts = [k for k in self.s3.objects if k.startswith('beta/promotions/month-renewal-')]
        self.assertEqual(len(receipts), 1)
        self.assertTrue(receipts[0].endswith('-2027-01/intent.json'))
        receipt = json.loads(self.s3.objects[receipts[0]])
        self.assertEqual((receipt['month'], receipt['previous_entry'], receipt['expected_after_state_sha256']), ('2027-01', None, digest(actual)))
        self.assertEqual(result['public']['status'], 'applied')
        # The month now satisfies preflight for release H's window, which reaches into January.
        _, report = preflight().reserve(self.release('11111111-2222-4333-8444-555555555555', NEXT_RELEASE), actual['ledger'], CLOCK)
        self.assertEqual(set(report['cost_by_month']), {'2026-12', '2027-01'})
        self.assertTrue(all(c['genui_total_cents'] <= 21000 and c['project_total_cents'] <= 24800 for c in report['cost_by_month'].values()))

    def test_reassessment_moves_the_timestamp_but_never_lowers_an_amount(self):
        self.run_month(apply=True)
        # Two days later, after December was reconciled again (reconcile's job; here written in place).
        renewed = self.store.load()
        reconciled(renewed['ledger'], '2026-12', CLOCK + timedelta(days=2))
        self.s3.objects['beta/state.json'] = encoded(renewed)
        later = Runner(self.aws, self.store, clock=lambda: CLOCK + timedelta(days=2))
        higher = copy.deepcopy(self.assessment)
        higher.update(ledger_sha256=digest(renewed['ledger']), rationale='Re-assessed before release H: a higher variable reserve for the January logs and probes.')
        higher['amounts']['variable_reserve_cents'] = 2000
        result = month_renewal.run(later, digest(renewed), higher, apply=True)
        entry = self.store.load()['ledger']['months']['2027-01']
        self.assertEqual((entry['variable_reserve_cents'], entry['reconciled_at_utc']), (2000, (CLOCK + timedelta(days=2)).strftime('%Y-%m-%dT%H:%M:%SZ')))
        self.assertFalse(result['public']['created'])
        lower = copy.deepcopy(higher)
        lower.update(ledger_sha256=digest(self.store.load()['ledger']))
        lower['amounts']['variable_reserve_cents'] = 1500
        with self.assertRaises(Refused):
            month_renewal.run(later, digest(self.store.load()), lower, apply=True)
        self.assertEqual(self.store.load()['ledger']['months']['2027-01']['variable_reserve_cents'], 2000)

    def test_refusals_before_any_write(self):
        def held(): self.state['active'] = {'owner': 'x'}; self.replace_state()
        def stale(): self.before = '0' * 64
        def workflow(): self.aws.phase = 'prepare'
        def other_month(): self.assessment['month'] = '2026-11'
        def february(): self.assessment['month'] = '2027-02'
        def december_begun(): self.assessment['month'] = '2026-12'
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
        def january_begun(): self.at(datetime(2027, 1, 1, 0, 0, 1, tzinfo=timezone.utc))
        def december_in_the_future():  # a refresh accepts a stale sibling month, never a malformed one
            self.state['ledger']['months']['2026-12']['reconciled_at_utc'] = (CLOCK + timedelta(hours=1)).strftime('%Y-%m-%dT%H:%M:%SZ'); self.replace_state()
        def december_below_the_floor():
            self.state['ledger']['months']['2026-12']['variable_reserve_cents'] = 999; self.replace_state()
        def github(): import os; os.environ['GITHUB_ACTIONS'] = 'true'
        for name, prepare in [('held lock', held), ('stale hash', stale), ('workflow identity', workflow), ('other month', other_month), ('february', february),
                              ('december begun', december_begun), ('other ledger', other_ledger), ('short rationale', short_rationale),
                              ('control character', control_character), ('low variable reserve', low_reserve), ('auth below', auth_low),
                              ('auth above', auth_high), ('negative', negative), ('float cents', float_cents), ('extra amount field', extra_field),
                              ('missing amount field', missing_field), ('over the cap', over_cap), ('january begun', january_begun),
                              ('december in the future', december_in_the_future), ('december below the floor', december_below_the_floor), ('github actions', github)]:
            with self.subTest(case=name):
                self.setUp(); prepare()
                with self.assertRaises(Refused):
                    self.run_month(apply=True)
                self.assert_unchanged()

    def test_once_january_begins_reconcile_owns_the_month(self):
        """Release H's window reaches into January, both months fresh, clock Jan 1 00:00:01: every other
        check would pass (the month exists, amounts do not drop, the live window validates), so only the
        month-begun rule stops a forward reservation from overwriting a month reconcile now owns."""
        clock = datetime(2027, 1, 1, 0, 0, 1, tzinfo=timezone.utc)
        live_window(self.state, NEXT_RELEASE, datetime(2026, 12, 5, 12, tzinfo=timezone.utc))  # release H executed on Dec 5
        for name in ('2026-12', '2027-01'):  # reconciled again just before the month boundary
            reconciled(self.state['ledger'], name, clock)
        self.replace_state()
        self.assessment['amounts']['variable_reserve_cents'] = 1600  # not lower than the existing 1500
        self.at(clock)
        with self.assertRaisesRegex(Refused, 'the month has begun'):
            self.run_month(apply=True)
        self.assert_unchanged()
        # Half an hour before the boundary, the same assessment re-assesses the month.
        self.at(clock - timedelta(minutes=30))
        self.assertEqual(self.run_month(apply=True)['public']['status'], 'applied')

    def test_clean_trial_windows_add_the_retained_trial_figure(self):
        """The live state names the trial organization, so the plan's figures are these: 5589 retained,
        4903 compute for the service and the broker, 11992 GenUI and 15792 project, under the 12000 / 15800 ceiling."""
        window = self.state['ledger']['windows'][self.state['live']['window_id']]
        window['clean_trial_organization_id'] = '7f2a9c4e-1b3d-4a5e-8f60-123456789abc'
        self.replace_state()
        budget = preflight()
        projection = self.run_month()['public']['month_projection']
        self.assertEqual(projection['retained_core_full_month_cents'], budget.cents(budget.FIXED_HOURLY * 31 * 24 + budget.FIXED_STORAGE_KEYS + budget.CLEAN_RETAINED_MONTHLY))
        self.assertEqual(projection['release_compute_cents'], budget.cents(353 * (budget.TASK_HOURLY + budget.BROKER_HOURLY)))  # the broker runs beside the service
        self.assertEqual((projection['retained_core_full_month_cents'], projection['release_compute_cents']), (5589, 4903))
        self.assertEqual((projection['genui_total_cents'], projection['project_total_cents']), (11992, 15792))
        self.assertLessEqual(projection['genui_total_cents'], month_renewal.MONTHS['2027-01']['genui'])
        self.assertLessEqual(projection['project_total_cents'], month_renewal.MONTHS['2027-01']['project'])
        # December's projection on the same ledger is amendment 2's other row: 744 h + 2 at the same rate.
        december = month_renewal.month_projection(budget, self.state['ledger'], '2026-12', RESERVES, 18)
        self.assertEqual(december, {'retained_core_full_month_cents': 5589, 'release_compute_cents': budget.cents(746 * (budget.TASK_HOURLY + budget.BROKER_HOURLY)),
                                    'release_stop_utc': NEXT_RELEASE, 'genui_total_cents': 17449, 'project_total_cents': 21249})

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
