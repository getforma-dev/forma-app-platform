import copy
import json
import unittest

from ops.promotion.reservation_cap import raise_caps
from ops.promotion.state import Refused, digest
from ops.promotion.test_state import MemoryS3, Store


def document():
    return {'schema_version': 1, 'revision': 4, 'active': None, 'completed': {},
            'ledger': {'months': {'2026-11': {'prior_extra_spend_cents': 0}}, 'windows': {'kept': 1}},
            'policy': {'budget_genui_cents': 17000, 'budget_total_cents': 20800, 'shorten_expiry_hours': 18}}


class ReservationCapTests(unittest.TestCase):
    def test_idle_raise_changes_only_the_two_caps(self):
        original = document()
        s3 = MemoryS3(original)
        store = Store(s3, 'bucket', 'beta')
        result = raise_caps(store, digest(original))
        self.assertEqual(result['status'], 'caps-raised')
        self.assertEqual((result['genui_cap_cents'], result['project_cap_cents']), (18000, 21800))
        saved = json.loads(s3.objects['beta/state.json'])
        self.assertEqual(saved['ledger'], original['ledger'])
        self.assertEqual(saved['policy']['shorten_expiry_hours'], 18)
        self.assertIsNone(saved['active'])
        self.assertEqual(saved['revision'], 5)

    def test_lock_drift_and_repeat_refuse_before_a_second_write(self):
        original = document()
        s3 = MemoryS3(original)
        store = Store(s3, 'bucket', 'beta')
        held = copy.deepcopy(original)
        held['active'] = {'promotion_id': 'held'}
        s3.objects['beta/state.json'] = __import__('ops.promotion.state', fromlist=['encoded']).encoded(held)
        with self.assertRaises(Refused):
            raise_caps(store, digest(original))
        self.assertEqual(json.loads(s3.objects['beta/state.json'])['active']['promotion_id'], 'held')
        fresh = document()
        fresh['policy']['budget_genui_cents'] = 18000
        fresh['policy']['budget_total_cents'] = 21800
        s3.objects['beta/state.json'] = __import__('ops.promotion.state', fromlist=['encoded']).encoded(fresh)
        with self.assertRaises(Refused):
            raise_caps(Store(s3, 'bucket', 'beta'), digest(fresh))
        self.assertEqual(json.loads(s3.objects['beta/state.json'])['revision'], 4)
