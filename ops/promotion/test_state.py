import copy
import io
import json
import unittest
from ops.promotion.state import Store, Refused, encoded


class MemoryS3:
    def __init__(self, document):
        self.objects = {'beta/state.json': encoded(document)}
        self.etag = 1; self.fail_after_write = False; self.fail_receipt = False

    def get_object(self, Bucket, Key):
        return {'Body': io.BytesIO(self.objects[Key]), 'ETag': str(self.etag)}

    def put_object(self, Bucket, Key, Body, **kwargs):
        if Key == 'beta/state.json':
            if kwargs.get('IfMatch') != str(self.etag): raise RuntimeError('412')
            self.etag += 1
        else:
            assert kwargs.get('IfNoneMatch') == '*'
            if Key in self.objects: raise RuntimeError('412')
            if self.fail_receipt: raise TimeoutError()
        self.objects[Key] = Body
        if self.fail_after_write:
            self.fail_after_write = False
            raise TimeoutError()
        return {'ETag': str(self.etag)}


def state():
    return {'schema_version': 1, 'revision': 0, 'active': None, 'ledger': {'old': {'cents': 100}}, 'completed': {}}


def authorization():
    return dict(repository='repo', workflow_ref='workflow', run_id='1', run_attempt='1')


class StateTests(unittest.TestCase):
    def setUp(self):
        self.s3 = MemoryS3(state()); self.store = Store(self.s3, 'bucket', 'beta')

    def test_two_preparers_cannot_acquire(self):
        self.store.acquire('one', authorization(), 'a')
        with self.assertRaises(Refused): Store(self.s3, 'bucket', 'beta').acquire('two', authorization(), 'b')
        self.assertEqual(json.loads(self.s3.objects['beta/state.json'])['ledger'], state()['ledger'])

    def test_stale_cas_cannot_overwrite_ledger_or_lease(self):
        other = Store(self.s3, 'bucket', 'beta'); other.load()
        self.store.acquire('one', authorization(), 'a')
        with self.assertRaises(RuntimeError): other.change(lambda d: d.update(ledger={}))
        self.assertEqual(json.loads(self.s3.objects['beta/state.json'])['active']['promotion_id'], 'one')

    def test_unknown_intent_write_does_not_call_aws(self):
        self.store.acquire('one', authorization(), 'a'); self.s3.fail_after_write = True
        called = []
        with self.assertRaises(TimeoutError): self.store.operation('stop', {}, lambda: called.append(True))
        self.assertEqual(called, [])
        with self.assertRaises(Refused): Store(self.s3, 'bucket', 'beta').resume('one', authorization())

    def test_unknown_response_does_not_replay_or_release(self):
        self.store.acquire('one', authorization(), 'a'); calls = []
        def operation():
            calls.append(1); raise TimeoutError()
        with self.assertRaises(TimeoutError): self.store.operation('activate', {'count': 1}, operation)
        with self.assertRaises(Refused): self.store.operation('activate', {'count': 1}, operation)
        self.assertEqual(calls, [1]); self.assertIsNotNone(self.store.load()['active'])

    def test_lost_receipt_after_success_holds_lease(self):
        self.store.acquire('one', authorization(), 'a'); self.s3.fail_receipt = True; calls = []
        with self.assertRaises(TimeoutError): self.store.operation('execute', {}, lambda: calls.append(1) or {})
        self.assertEqual(calls, [1]); self.assertEqual(self.store.load()['active']['journal'][0]['status'], 'intent')

    def test_success_has_intent_before_call_and_durable_result(self):
        self.store.acquire('one', authorization(), 'a')
        def operation():
            self.assertEqual(json.loads(self.s3.objects['beta/state.json'])['active']['journal'][0]['status'], 'intent')
            return {'ok': True}
        self.store.operation('readback', {'exact': 'request'}, operation)
        journal = self.store.load()['active']['journal'][0]
        self.assertEqual(journal['status'], 'confirmed')
        self.assertIn('beta/' + journal['receipt'], self.s3.objects)

    def test_rerun_attempt_cannot_reuse_old_approval_lease(self):
        self.store.acquire('one', authorization(), 'a')
        with self.assertRaises(Refused): self.store.resume('one', dict(authorization(), run_attempt='2'))


if __name__ == '__main__': unittest.main()
