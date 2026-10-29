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


def check(root):
    return subprocess.run([sys.executable, str(SCRIPT), "--root", str(root)], capture_output=True, text=True, timeout=30)


class SchemaPinTests(unittest.TestCase):
    def copy_tree(self, directory):
        for name in (MANIFEST, RUNTIME, POOL, CATALOG, MANAGED):
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
