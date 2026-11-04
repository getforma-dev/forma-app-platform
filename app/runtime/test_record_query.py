"""Full app-owned snapshots use the existing confined, typed query compiler."""
import copy
import hashlib
import json
import os
from pathlib import Path
import tempfile
import time
import unittest

import query_engine as engine


FIELDS = [{"id": "amount", "value_type": {"type": "decimal", "scale": 2, "unit": "count"}, "nullable": False},
          {"id": "name", "value_type": {"type": "text"}, "nullable": True},
          {"id": "active", "value_type": {"type": "boolean"}, "nullable": False}]
REF = {"input_id": "app_data", "field_id": "amount"}


class RecordQueryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)

    def tearDown(self):
        self.temp.cleanup()

    def request(self, rows, operations):
        root = self.root / str(len(list(self.root.iterdir())))
        root.mkdir(mode=0o700)
        path = root / "records.ndjson"
        path.write_text("".join(json.dumps(row) + "\n" for row in rows))
        os.chmod(path, 0o600)
        request = {"organization_id": "00000000-0000-4000-8000-000000000001",
                   "source": {"kind": "records", "input_id": "app_data", "fields": FIELDS,
                              "digest": "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()},
                   "query": {"operator_version": 1, "null_policy": "unknown", "inputs": ["app_data"], "operations": operations},
                   "cache_root": str(self.root), "scratch": str(root), "deadline_ms": 1800}
        return engine.parse_request(json.dumps(request).encode())

    def run_query(self, rows, operations):
        return engine.execute(self.request(rows, operations), time.monotonic())

    def test_full_snapshot_over_2000_rows_exact_decimal_filters_and_new_records(self):
        rows = [{"amount": "125", "name": "selected", "active": True} for _ in range(3001)]
        rows.append({"amount": "999", "name": "other", "active": False})
        operations = [{"type": "filter", "field": {"input_id": "app_data", "field_id": "name"},
                       "comparison": "eq", "value": {"type": "text", "value": "selected"}},
                      {"type": "aggregate", "group_by": [], "measures": [
                          {"id": "total", "operation": "sum", "field": REF},
                          {"id": "rows", "operation": "count", "field": REF}]}]
        first = self.run_query(rows, operations)
        self.assertEqual(first["rows"], [["375125", "3001"]])
        self.assertFalse(first["partial"])
        rows[0]["amount"] = "200"
        rows.append({"amount": "25", "name": "selected", "active": True})
        self.assertEqual(self.run_query(rows, operations)["rows"], [["375225", "3002"]])

    def test_null_empty_boolean_and_large_exact_scalar_survive_projection(self):
        rows = [{"amount": "9007199254740993", "name": "", "active": False},
                {"amount": "-25", "name": None, "active": True}]
        result = self.run_query(rows, [{"type": "project", "fields": [REF,
            {"input_id": "app_data", "field_id": "name"}, {"input_id": "app_data", "field_id": "active"}]}])
        self.assertEqual(result["rows"], [["9007199254740993", "", False], ["-25", None, True]])
        empty = self.run_query([], [{"type": "aggregate", "group_by": [], "measures": [{"id": "total", "operation": "sum", "field": REF}]}])
        self.assertEqual(empty["rows"], [[None]])

    def test_page_limit_does_not_change_whole_input_aggregation(self):
        result = self.run_query([{"amount": str(i), "name": "row", "active": True} for i in range(600)],
                                [{"type": "project", "fields": [REF]}, {"type": "limit", "rows": 20}])
        self.assertEqual(len(result["rows"]), 20)
        self.assertTrue(result["truncated_page"])

    def test_digest_closed_protocol_and_private_snapshot_path_are_checked(self):
        request = self.request([], [{"type": "project", "fields": [REF]}])
        bad = copy.deepcopy(request)
        bad["source"]["path"] = "/etc/passwd"
        with self.assertRaises(engine.Refused):
            engine.parse_request(json.dumps(bad).encode())
        path = Path(request["scratch"]) / "records.ndjson"
        path.write_text('{"amount":"42"}\n')
        with self.assertRaisesRegex(engine.Refused, "source_digest_mismatch"):
            engine.execute(request, time.monotonic())
        path.unlink()
        path.symlink_to(self.root / "missing")
        with self.assertRaisesRegex(engine.Refused, "scratch_refused"):
            engine.execute(request, time.monotonic())


if __name__ == "__main__":
    unittest.main()
