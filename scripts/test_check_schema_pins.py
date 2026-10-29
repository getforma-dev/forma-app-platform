"""Schema-version pin agreement check over the tree and over edited copies.

Run: python3 -m unittest discover -s scripts -p 'test_*.py' -v
"""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "check-schema-pins.py"
MANIFEST = "app/migrations/manifest.json"
RUNTIME = "app/runtime/runtime.py"
POOL = "app/src/storage/pool.rs"
CATALOG = "app/src/storage/catalog.rs"
MANAGED = "app/src/storage/managed_qualification.rs"
QUERY = "app/migrations/catalog-query.json"


def check(root):
    return subprocess.run([sys.executable, str(SCRIPT), "--root", str(root)], capture_output=True, text=True, timeout=30)


class SchemaPinTests(unittest.TestCase):
    def copy_tree(self, directory):
        catalogs = tuple("app/migrations/" + item["file"] for item in json.loads((ROOT / MANIFEST).read_text())["catalog"])
        for name in (MANIFEST, RUNTIME, POOL, CATALOG, MANAGED, *catalogs):
            target = Path(directory) / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)
        return Path(directory)

    def replace_once(self, path, old, new):
        text = path.read_text()
        self.assertEqual(text.count(old), 1, (path, old))
        path.write_text(text.replace(old, new))

    def test_current_tree_pins_agree(self):
        result = check(ROOT)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        report = json.loads(result.stdout)
        version = json.loads((ROOT / MANIFEST).read_text())["database_version"]
        self.assertTrue(report["agree"])
        self.assertEqual(len(report["pins"]), 5)
        self.assertEqual(set(report["pins"].values()), {version})
        # Second agreement group (runner review m1): the moved catalog bound.
        self.assertEqual(len(report["catalog_bound"]), 4)
        self.assertEqual(set(report["catalog_bound"].values()), {524288})
        # Runner review m3: every ledger-era catalog carries its own version bound.
        self.assertEqual(report["catalog_version_checks"], {f"catalog-v{k}.json": k for k in range(2, version + 1)})

    def test_mismatched_catalog_bound_fails(self):
        edits = {
            "catalog-query.json max_output_bytes": (QUERY, '"max_output_bytes": 524288,', '"max_output_bytes": 524289,'),
            "catalog-query.json SQL literal": (QUERY, "octet_length(body)<=524288", "octet_length(body)<=262144"),
            "catalog.rs BOUND": (CATALOG, "const BOUND: usize = 524_288;", "const BOUND: usize = 262_144;"),
            "runtime.py CATALOG_BOUND": (RUNTIME, "\nCATALOG_BOUND = 524288\n", "\nCATALOG_BOUND = 262144\n"),
        }
        for label, (name, old, new) in edits.items():
            with self.subTest(pin=label), tempfile.TemporaryDirectory() as directory:
                root = self.copy_tree(directory)
                self.assertEqual(check(root).returncode, 0)
                self.replace_once(root / name, old, new)
                result = check(root)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertFalse(json.loads(result.stdout)["agree"])

    def test_catalog_version_check_and_distinct_catalogs_are_required(self):
        version = json.loads((ROOT / MANIFEST).read_text())["database_version"]
        latest = f"app/migrations/catalog-v{version}.json"
        previous = f"app/migrations/catalog-v{version - 1}.json"
        cases = {
            "wrong schema_version bound": lambda root: self.replace_once(
                root / latest, f'"schema_version_version_check","c",true,false,false,"CHECK (version >= 1 AND version <= {version})"',
                f'"schema_version_version_check","c",true,false,false,"CHECK (version >= 1 AND version <= {version + 6})"'),
            "wrong schema_migrations bound": lambda root: self.replace_once(
                root / latest, f'"schema_migrations_version_check","c",true,false,false,"CHECK (version >= 1 AND version <= {version})"',
                f'"schema_migrations_version_check","c",true,false,false,"CHECK (version >= 1 AND version <= {version - 1})"'),
            "identical consecutive catalogs": lambda root: shutil.copyfile(root / previous, root / latest),
        }
        for label, edit in cases.items():
            with self.subTest(case=label), tempfile.TemporaryDirectory() as directory:
                root = self.copy_tree(directory)
                self.assertEqual(check(root).returncode, 0)
                edit(root)
                result = check(root)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertFalse(json.loads(result.stdout)["agree"])

    def test_mismatched_pin_fails(self):
        version = json.loads((ROOT / MANIFEST).read_text())["database_version"]
        edits = {
            "pool.rs verify_schema_version literal": (POOL, f")? != {version} {{", f")? != {version + 1} {{"),
            "runtime.py constant": (RUNTIME, f"\nDATABASE_SCHEMA_VERSION = {version}\n", f"\nDATABASE_SCHEMA_VERSION = {version + 1}\n"),
            "catalog.rs EXPECTED include": (CATALOG, f'include_str!("../../migrations/catalog-v{version}.json")',
                                            f'include_str!("../../migrations/catalog-v{version + 1}.json")'),
            "managed_qualification.rs catalog include": (MANAGED, f'include_bytes!("../../migrations/catalog-v{version}.json")',
                                                         f'include_bytes!("../../migrations/catalog-v{version + 1}.json")'),
            "manifest database_version": (MANIFEST, f'"database_version": {version},', f'"database_version": {version + 1},'),
        }
        for label, (name, old, new) in edits.items():
            with self.subTest(pin=label), tempfile.TemporaryDirectory() as directory:
                root = self.copy_tree(directory)
                self.assertEqual(check(root).returncode, 0)
                self.replace_once(root / name, old, new)
                result = check(root)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertFalse(json.loads(result.stdout)["agree"])

    def test_missing_or_duplicated_pin_refuses(self):
        version = json.loads((ROOT / MANIFEST).read_text())["database_version"]
        include = f'const EXPECTED: &str = include_str!("../../migrations/catalog-v{version}.json");\n'
        for case in ("missing", "duplicated"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as directory:
                root = self.copy_tree(directory)
                self.replace_once(root / CATALOG, include, "" if case == "missing" else include + include)
                result = check(root)
                self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
                self.assertFalse(json.loads(result.stdout)["agree"])


if __name__ == "__main__":
    unittest.main()
