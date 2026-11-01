"""APP-154 additive guest receipt schema and cross-language digest parity."""
import copy
import hashlib
import json
from pathlib import Path
import unittest
from jsonschema import Draft202012Validator
import validate

ROOT = Path(__file__).resolve().parent

class SharedReceiptTests(unittest.TestCase):
    def setUp(self):
        self.receipt = json.loads((ROOT / 'fixtures/shared-v1/query-receipt.json').read_text())
        schema = json.loads((ROOT / 'shared-query-receipt-v1.schema.json').read_text())
        schema['$defs'] = json.loads((ROOT / 'schema.json').read_text())['$defs']
        def local(value):
            if isinstance(value, dict):
                for key, item in value.items():
                    if key == '$ref' and item.startswith('schema.json#'):
                        value[key] = item[len('schema.json'):]
                    else:
                        local(item)
            elif isinstance(value, list):
                for item in value:
                    local(item)
        local(schema)
        self.validator = Draft202012Validator(schema, format_checker=validate.FORMATS)
    def test_valid_shared_receipt_has_explicit_account_and_grant_identity(self):
        self.validator.validate(self.receipt)
        body = {k: v for k, v in self.receipt.items() if k != 'digest'}
        digest = 'sha256:' + hashlib.sha256(b'forma:shared-query-receipt:v1\0' + validate.canonical(body)).hexdigest()
        self.assertEqual(self.receipt['digest'], digest)
        self.assertNotIn('principal_id', self.receipt)
        self.assertNotIn('membership_epoch', self.receipt)
    def test_missing_and_member_authority_fields_are_rejected(self):
        for key in ('publication_id', 'account_id', 'share_id', 'share_version'):
            changed = copy.deepcopy(self.receipt)
            del changed[key]
            self.assertFalse(self.validator.is_valid(changed), key)
        for key, value in (('principal_id', self.receipt['account_id']), ('membership_epoch', 1), ('share_version', 0), ('mode', 'synthetic_preview'), ('account_id', 'invalid'), ('share_id', 'invalid')):
            changed = dict(self.receipt, **{key: value})
            self.assertFalse(self.validator.is_valid(changed), key)

if __name__ == '__main__': unittest.main()
