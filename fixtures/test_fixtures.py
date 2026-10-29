"""Focused checks for fixture integrity and independently calculated semantics."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

from generate import build_package
from verify import DEFAULT, Refused, event_oracle, integer, inventory_rows, load_package, member, spend_oracle, table, timestamp, verify


class Fixtures(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = load_package()
        cls.mapping = json.loads((DEFAULT / "mappings.json").read_text())
        cls.rows = {entry["dataset"]: table(DEFAULT, entry) for entry in cls.manifest["sources"] if entry["format"] == "csv"}

    def test_scenario_arithmetic_from_original_bytes(self):
        result = verify()
        self.assertEqual(result["source_count"], 28)
        self.assertEqual(len(result["paired_csv_xlsx_datasets"]), 13)
        self.assertNotEqual(result["northstar"]["total_bytes"], result["canary"]["total_bytes"])
        self.assertEqual(result["partial-coverage"]["minute_bytes"][4], None)

    def test_two_separate_generations_are_byte_exact(self):
        with tempfile.TemporaryDirectory() as first, tempfile.TemporaryDirectory() as second:
            for root in (Path(first), Path(second)):
                for relative, data in build_package().items():
                    path = root / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(data)
            for relative in build_package():
                self.assertEqual((Path(first) / relative).read_bytes(), (Path(second) / relative).read_bytes())
                self.assertEqual((Path(first) / relative).read_bytes(), (DEFAULT / relative).read_bytes())
            for path in Path(first).rglob("*.xlsx"):
                with zipfile.ZipFile(path) as archive:
                    self.assertEqual(archive.namelist(), sorted(archive.namelist()))
                    for entry in archive.infolist():
                        self.assertEqual(entry.date_time, (2026, 10, 22, 0, 0, 0))
                        self.assertEqual(entry.compress_type, zipfile.ZIP_STORED)
                        self.assertEqual(entry.extra, b"")

    def test_changed_original_bytes_and_unlisted_files_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for relative, data in build_package().items():
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            original = root / "sources/northstar-events.csv"
            data = original.read_bytes()
            original.write_bytes(data.replace(b"60000", b"60001", 1))
            with self.assertRaisesRegex(Refused, "fixture_digest_mismatch"):
                load_package(root)
            original.write_bytes(data)
            (root / "unlisted.csv").write_text("unlisted")
            with self.assertRaisesRegex(Refused, "fixture_inventory_mismatch"):
                load_package(root)

    def test_manifest_duplicate_and_escape_refused(self):
        for mutation, reason in ((lambda m: m["sources"].append(m["sources"][0]), "duplicate_fixture_path"),
                                 (lambda m: m["sources"][0].update(path="../outside.csv"), "unsafe_fixture_path")):
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                for relative, data in build_package().items():
                    path = root / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(data)
                m = copy.deepcopy(self.manifest)
                mutation(m)
                (root / "manifest.json").write_text(json.dumps(m))
                with self.assertRaisesRegex(Refused, reason):
                    load_package(root)

    def test_symlink_within_selected_root_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "real.csv").write_text("synthetic")
            (root / "link.csv").symlink_to("real.csv")
            with self.assertRaisesRegex(Refused, "symlink_fixture_path"):
                member(root, "link.csv")

    def test_duplicate_inventory_and_events_not_double_counted(self):
        inventory = copy.deepcopy(self.rows["northstar-inventory"])
        inventory.append({**inventory[0], "inventory_id": "other-id"})
        with self.assertRaisesRegex(Refused, "inventory_ambiguous"):
            inventory_rows(inventory)
        rows = self.rows["northstar-events"]
        with self.assertRaisesRegex(Refused, "event_identity_refused"):
            event_oracle(rows + [rows[0]], self.rows["northstar-inventory"], self.mapping["events"], "build-worker-17")

    def test_timestamp_and_integer_contract_boundaries(self):
        for value in ("2026-10-22T10:00:00+00:00", "2026-10-22T10:00:00.1Z", "2026-12-31T23:59:60Z", "2026-02-30T00:00:00Z"):
            with self.assertRaisesRegex(Refused, "timestamp_refused"):
                timestamp(value)
        for value in ("1.0", "-1", "01", "1e3", str(10**38)):
            with self.assertRaisesRegex(Refused, "integer_refused"):
                integer(value)
        self.assertEqual(integer("9007199254740993"), 9007199254740993)

    def test_threshold_and_exact_large_integer_arithmetic(self):
        rows = [{"expense_id": "LOW", "amount_cents": "19999", "currency": "USD"},
                {"expense_id": "AT", "amount_cents": "20000", "currency": "USD"},
                {"expense_id": "BIG", "amount_cents": "9007199254740993", "currency": "USD"}]
        result = spend_oracle(rows, self.mapping["spend"])
        self.assertEqual(result["queue_ids"], ["AT", "BIG"])
        self.assertEqual(result["total_cents"], "9007199254780992")
        rows[0]["currency"] = ""
        with self.assertRaisesRegex(Refused, "currency_refused"):
            spend_oracle(rows, self.mapping["spend"])

    def test_inventory_order_cannot_change_precedence(self):
        rows = self.rows["northstar-events"]
        inventory = self.rows["inventory-overlap"]
        first = event_oracle(rows, inventory, self.mapping["events"], "build-worker-17")
        second = event_oracle(rows, list(reversed(inventory)), self.mapping["events"], "build-worker-17")
        self.assertEqual(first, second)
        self.assertEqual(first["matches"]["E1"], "INV-EXACT")
        self.assertEqual(first["matches"]["E2"], "INV-NET25")


if __name__ == "__main__":
    unittest.main()
