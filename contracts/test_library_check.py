"""The library check names a stale bundle, catalog or CSV and does not rewrite it."""
import json
import tempfile
import unittest
from pathlib import Path

import make_library


class LibraryCheckTests(unittest.TestCase):
    def test_a_changed_bundle_catalog_or_csv_is_named_and_not_rewritten(self):
        files = {
            "catalog.json": b'{"apps":[]}\n',
            "expense-claims.json": b'{"definition":{}}\n',
            "data/expense_claims.csv": b"title,author\nThe Dispossessed,Ursula K. Le Guin\n",
        }
        with tempfile.TemporaryDirectory() as temporary:
            library = Path(temporary)
            for relative, data in files.items():
                path = library / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            self.assertEqual(make_library.differences(files, library), [])
            mutated = {
                "catalog.json": b'{"apps":["stale"]}\n',
                "expense-claims.json": b'{"definition":{"stale":true}}\n',
                "data/expense_claims.csv": b"title,author\nstale,row\n",
            }
            for relative, data in mutated.items():
                (library / relative).write_bytes(data)
            (library / "data" / "extra.csv").write_bytes(b"not,generated\n")
            stale = make_library.differences(files, library)
            self.assertEqual(
                stale,
                ["catalog.json", "expense-claims.json", "data/expense_claims.csv", "data/extra.csv"],
            )
            for relative, data in mutated.items():
                self.assertEqual((library / relative).read_bytes(), data)
            self.assertEqual((library / "data" / "extra.csv").read_bytes(), b"not,generated\n")
        report = json.dumps({"stale": stale})
        for relative in stale:
            self.assertIn(relative, report)
