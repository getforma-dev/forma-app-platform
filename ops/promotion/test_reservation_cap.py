import copy
import json
import unittest

from ops.promotion.reservation_cap import PREVIOUS, TARGET, plan_caps, raise_caps
from ops.promotion.state import Refused, digest, encoded
from ops.promotion.test_state import MemoryS3, Store


def document(caps=PREVIOUS):
    return {'schema_version': 1, 'revision': 4, 'active': None, 'completed': {},
            'ledger': {'months': {'2026-11': {'prior_extra_spend_cents': 0}}, 'windows': {'kept': 1}},
            'policy': {'budget_genui_cents': caps[0], 'budget_total_cents': caps[1], 'shorten_expiry_hours': 18}}


class ReservationCapTests(unittest.TestCase):
    def test_the_raise_is_the_adr_019_amendment_pair(self):
        self.assertEqual((PREVIOUS, TARGET), ((18000, 21800), (21000, 24800)))
        from ops.promotion.runner import preflight
        plans = preflight().PLANS['always-on-beta']
        self.assertEqual((plans['genui_cap_cents'], plans['project_cap_cents'], plans['monthly_budget_usd']), (21000, 24800, '170'))

    def test_plan_is_read_only(self):
        original = document()
        s3 = MemoryS3(original)
        result = plan_caps(Store(s3, 'bucket', 'beta'), digest(original))
        self.assertEqual((result['status'], result['from'], result['to'], result['revision']), ('planned', [18000, 21800], [21000, 24800], 4))
        self.assertEqual(json.loads(s3.objects['beta/state.json']), original)
        self.assertEqual(s3.etag, 1)

    def test_idle_raise_changes_only_the_two_caps(self):
        original = document()
        s3 = MemoryS3(original)
        store = Store(s3, 'bucket', 'beta')
        result = raise_caps(store, digest(original))
        self.assertEqual(result['status'], 'caps-raised')
        self.assertEqual((result['genui_cap_cents'], result['project_cap_cents']), (21000, 24800))
        saved = json.loads(s3.objects['beta/state.json'])
        self.assertEqual(saved['ledger'], original['ledger'])
        self.assertEqual(saved['policy']['shorten_expiry_hours'], 18)
        self.assertIsNone(saved['active'])
        self.assertEqual(saved['revision'], 5)
        self.assertEqual(result['state_sha256'], digest(saved))

    def test_lock_drift_old_pair_and_repeat_refuse_before_a_second_write(self):
        original = document()
        s3 = MemoryS3(original)
        store = Store(s3, 'bucket', 'beta')
        held = copy.deepcopy(original)
        held['active'] = {'promotion_id': 'held'}
        s3.objects['beta/state.json'] = encoded(held)
        with self.assertRaises(Refused):
            raise_caps(store, digest(original))
        self.assertEqual(json.loads(s3.objects['beta/state.json'])['active']['promotion_id'], 'held')
        # The first raise's pair is no longer what this command replaces, and the target pair refuses a repeat.
        for caps in ((17000, 20800), (21000, 24800)):
            with self.subTest(caps=caps):
                fresh = document(caps)
                s3.objects['beta/state.json'] = encoded(fresh)
                with self.assertRaises(Refused):
                    raise_caps(Store(s3, 'bucket', 'beta'), digest(fresh))
                self.assertEqual(json.loads(s3.objects['beta/state.json'])['revision'], 4)
