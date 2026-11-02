import copy
from datetime import datetime, timedelta, timezone
import unittest

from ops.promotion.guards import ACCOUNT
from ops.promotion.reconcile import apply, assessed_ledger, observe, validate_observation
from ops.promotion.state import Refused, Store, digest, encoded
from ops.promotion.test_state import MemoryS3

NOW = datetime(2026, 11, 4, 12, tzinfo=timezone.utc)


def fixtures():
    query = {'TimePeriod': {'Start': '2026-11-01', 'End': '2026-11-04'},
             'Granularity': 'MONTHLY', 'Metrics': ['UnblendedCost'],
             'Filter': {'Dimensions': {'Key': 'LINKED_ACCOUNT', 'Values': [ACCOUNT]}}}
    observation = {'schema_version': 1, 'account': ACCOUNT, 'observed_at_utc': '2026-11-04T12:00:00Z',
        'query': query, 'response': {'ResponseMetadata': {'HTTPStatusCode': 200, 'RequestId': 'synthetic'},
            'ResultsByTime': [{'TimePeriod': query['TimePeriod'], 'Estimated': True,
                              'Total': {'UnblendedCost': {'Unit': 'USD', 'Amount': '12.34'}}}]}}
    amounts = dict(prior_extra_spend_cents=5, variable_reserve_cents=1000,
                   auth_spent_or_reserved_cents=3800, unallocated_cents=0)
    ledger = {'months': {'2026-11': dict(amounts, basis='prior actual basis', reconciled_at_utc='2026-11-02T12:00:00Z')},
              'windows': {'prior': {'activation_requested_at_utc': '2026-11-01T12:00:00Z', 'reserved': 500}}}
    state = {'schema_version': 1, 'revision': 5, 'active': None, 'ledger': ledger,
             'live': {'config': {'window_id': 'prior'}}, 'policy': {'budget': 20800}}
    assessment = {'month': '2026-11', 'ledger_sha256': digest(ledger), 'observation_sha256': digest(observation),
        'account_total_usd': '12.34', 'billing_lag_and_query_cost_covered': True, 'amounts': amounts,
        'rationale': 'Observed account cost is covered by existing fixed and variable reserves, including billing lag and query cost.'}
    return state, assessment, observation


class ReconciliationTests(unittest.TestCase):
    def test_observe_calls_actual_scoped_billing_read(self):
        _, _, observation = fixtures()
        class Billing:
            def identity(self): return {'Account': ACCOUNT}
            def call(inner, service, operation, **query):
                self.assertEqual((service, operation), ('ce', 'get_cost_and_usage'))
                self.assertEqual(query, observation['query'])
                return observation['response']
        self.assertEqual(observe(Billing(), NOW), observation)

    def test_monotonic_update_preserves_every_window_and_other_state(self):
        state, assessment, observation = fixtures(); s3 = MemoryS3(state)
        store = Store(s3, 'bucket', 'beta')
        seen = []
        def validator(config, ledger, clock):
            seen.append((config, clock)); return ledger, {}
        result = apply(store, assessment, observation, NOW, observation, validator)
        after = store.load()
        self.assertEqual(result['status'], 'reconciled')
        self.assertEqual(after['ledger']['windows'], state['ledger']['windows'])
        self.assertEqual(after['live'], state['live']); self.assertEqual(after['policy'], state['policy'])
        self.assertEqual(seen, [(state['live']['config'], NOW)])
        self.assertIn('beta/' + after['last_reconciliation']['receipt'], s3.objects)

    def test_refuses_lower_money_for_each_category(self):
        for field in ('prior_extra_spend_cents', 'variable_reserve_cents', 'auth_spent_or_reserved_cents', 'unallocated_cents'):
            state, assessment, observation = fixtures(); assessment['amounts'][field] -= 1
            with self.subTest(field=field), self.assertRaises(Refused):
                assessed_ledger(state, assessment, observation, NOW)

    def test_requires_operator_assessment_not_just_new_timestamp(self):
        for key, value in [('rationale', ''), ('billing_lag_and_query_cost_covered', False), ('account_total_usd', '0')]:
            state, assessment, observation = fixtures(); assessment[key] = value
            with self.subTest(key=key), self.assertRaises(Refused):
                assessed_ledger(state, assessment, observation, NOW)

    def test_rejects_stale_future_foreign_partial_nan_and_missing_actual_response(self):
        for change in [lambda x: x.update(observed_at_utc='2026-11-04T10:00:00Z'),
                       lambda x: x.update(observed_at_utc='2026-11-04T13:00:00Z'),
                       lambda x: x.update(account='000000000000'),
                       lambda x: x['response'].update(NextPageToken='next'),
                       lambda x: x['response']['ResponseMetadata'].pop('RequestId'),
                       lambda x: x['response']['ResultsByTime'][0]['Total']['UnblendedCost'].update(Amount='NaN')]:
            _, _, observation = fixtures(); change(observation)
            with self.assertRaises(Refused): validate_observation(observation, NOW)

    def test_stale_assessment_and_missing_month_refuse(self):
        for key, value in [('ledger_sha256', '0'*64), ('observation_sha256', '0'*64), ('month', '2026-12')]:
            state, assessment, observation = fixtures(); assessment[key] = value
            with self.assertRaises(Refused): assessed_ledger(state, assessment, observation, NOW)

    def test_active_or_racing_release_cannot_be_overwritten(self):
        state, assessment, observation = fixtures(); state['active'] = {'owner': 'deployment'}
        s3 = MemoryS3(state); store = Store(s3, 'bucket', 'beta')
        with self.assertRaises(Refused): apply(store, assessment, observation, NOW, observation, lambda *a: None)
        state['active'] = None; s3 = MemoryS3(state); store = Store(s3, 'bucket', 'beta')
        def race(config, ledger, clock):
            other = Store(s3, 'bucket', 'beta'); other.load()
            other.change(lambda d: d.update(active={'owner': 'new-deployment'}))
            return ledger, {}
        with self.assertRaises(RuntimeError): apply(store, assessment, observation, NOW, observation, race)
        self.assertEqual(store.load()['active'], {'owner': 'new-deployment'})
        self.assertEqual(store.load()['ledger'], state['ledger'])

    def test_actual_budget_validator_denial_never_changes_state(self):
        state, assessment, observation = fixtures(); s3 = MemoryS3(state); store = Store(s3, 'bucket', 'beta')
        def refuse(*args): raise ValueError('budget exceeded')
        with self.assertRaises(ValueError): apply(store, assessment, observation, NOW, observation, refuse)
        self.assertEqual(s3.objects['beta/state.json'], encoded(state))

    def test_edited_or_changed_bill_cannot_refresh_basis(self):
        state, assessment, observation = fixtures(); verified = copy.deepcopy(observation)
        verified['response']['ResultsByTime'][0]['Total']['UnblendedCost']['Amount'] = '20.00'
        s3 = MemoryS3(state); store = Store(s3, 'bucket', 'beta')
        with self.assertRaises(Refused):
            apply(store, assessment, observation, NOW, verified, lambda c, l, n: (l, {}))
        self.assertEqual(s3.objects['beta/state.json'], encoded(state))

    def test_new_month_does_not_invent_a_bill(self):
        class Billing:
            def identity(self): return {'Account': ACCOUNT}
            def call(self, *args, **kwargs): raise AssertionError('no query allowed')
        with self.assertRaises(Refused): observe(Billing(), NOW.replace(day=1))

    def test_unknown_cas_write_does_not_retry(self):
        state, assessment, observation = fixtures(); s3 = MemoryS3(state); store = Store(s3, 'bucket', 'beta')
        original = store.save
        def lose_response(value):
            s3.fail_after_write = True; original(value)
        store.save = lose_response
        with self.assertRaises(TimeoutError): apply(store, assessment, observation, NOW, observation, lambda c, l, n: (l, {}))
        self.assertEqual(store.load()['revision'], state['revision'] + 1)
        with self.assertRaises(Refused): apply(store, assessment, observation, NOW, observation, lambda c, l, n: (l, {}))


if __name__ == '__main__': unittest.main()
