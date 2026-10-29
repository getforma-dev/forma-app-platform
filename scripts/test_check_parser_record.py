"""Release refusal for parser bytes without a passing image-bytes record (ADR-009 D9.1).

Run: python3 -m unittest discover -s scripts -p 'test_*.py' -v
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "check-parser-record.py"
WASM = b"\0asm\x01\0\0\0synthetic-guest"
DIGEST = hashlib.sha256(WASM).hexdigest()
RUN_URL = "https://github.com/getforma-dev/forma-app-platform/actions/runs/1"
COMMIT = "a" * 40


def run(*args, env=None):
    environment = {key: value for key, value in os.environ.items() if not key.startswith("GITHUB_")}
    environment.update(env or {})
    return subprocess.run([sys.executable, str(SCRIPT), *map(str, args)], capture_output=True, text=True, timeout=30, env=environment)


def suite_log(digest=DIGEST, failed=0):
    return (f"running 9 tests\ntest qualified_guest_bytes_are_named_in_the_log ... QUALIFIED_WASM sha256:{digest} bytes=21 path=/x\nok\n"
            f"test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.1s\n"
            f"test result: {'FAILED' if failed else 'ok'}. 17 passed; {failed} failed; 0 ignored; 0 measured; 0 filtered out; finished in 12s\n")


class ParserRecordTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.records = self.root / "records"
        self.records.mkdir()
        self.inputs = self.root / "inputs"
        self.inputs.mkdir()
        (self.inputs / "parser_guest.wasm").write_bytes(WASM)
        manifest = {"schema": "genui-parser-manifest/1",
                    "guest": {"path": "/app/parser/parser_guest.wasm", "sha256": DIGEST, "bytes": len(WASM)},
                    "host": {"path": "/app/parser/parser-host", "sha256": "b" * 64, "bytes": 1},
                    "source": {"sources_sha256": "c" * 64, "cargo_lock_sha256": "d" * 64}}
        (self.inputs / "parser-manifest.json").write_text(json.dumps(manifest))
        (self.inputs / "suite.log").write_text(suite_log())
        (self.inputs / "summary.json").write_text(json.dumps({"passed": True, "parser_cases": 12, "parser_binding": {"wasm_sha256": "sha256:" + DIGEST}}))

    def tearDown(self):
        self.directory.cleanup()

    def check(self, digest=DIGEST):
        return run("--wasm-sha256", digest, "--records", self.records)

    def compose(self, env=None, **overrides):
        args = {"--wasm": self.inputs / "parser_guest.wasm", "--manifest": self.inputs / "parser-manifest.json", "--suite-log": self.inputs / "suite.log",
                "--qualify-summary": self.inputs / "summary.json", "--output-dir": self.records}
        args.update(overrides)
        return run("compose", *[item for pair in args.items() for item in pair if item is not None], env=env)

    def ci_compose(self):
        return self.compose(env={"GITHUB_ACTIONS": "true"}, **{"--run-url": RUN_URL, "--commit": COMMIT})

    def record(self):
        return json.loads((self.records / (DIGEST + ".json")).read_text())

    def rewrite(self, change):
        record = self.record()
        change(record)
        (self.records / (DIGEST + ".json")).write_text(json.dumps(record))

    def test_release_refuses_parser_digest_without_passing_record(self):
        refused = self.check()
        self.assertEqual(refused.returncode, 1, refused.stdout + refused.stderr)
        self.assertIn("no_record", refused.stdout)
        self.assertEqual(self.ci_compose().returncode, 0)
        other = hashlib.sha256(b"other guest bytes").hexdigest()
        refused = self.check(other)
        self.assertEqual(refused.returncode, 1)
        self.assertIn("no_record", refused.stdout)

    def test_passing_ci_record_admits_exactly_its_digest(self):
        composed = self.ci_compose()
        self.assertEqual(composed.returncode, 0, composed.stdout + composed.stderr)
        record = self.record()
        self.assertTrue(record["passed"])
        self.assertEqual(record["wasm_sha256"], DIGEST)
        self.assertEqual(record["origin"], {"kind": "github-actions", "run_url": RUN_URL, "commit": COMMIT})
        self.assertEqual(record["hostile_suite"]["tests_passed"], 18)
        admitted = self.check()
        self.assertEqual(admitted.returncode, 0, admitted.stdout + admitted.stderr)
        self.assertEqual(json.loads(admitted.stdout)["admitted"], True)

    def test_local_record_is_evidence_never_a_release_pass(self):
        composed = self.compose(**{"--run-url": RUN_URL, "--commit": COMMIT})
        self.assertEqual(composed.returncode, 0, composed.stdout + composed.stderr)
        self.assertEqual(self.record()["origin"]["kind"], "local")
        refused = self.check()
        self.assertEqual(refused.returncode, 1)
        self.assertIn("origin_not_ci", refused.stdout)

    def test_compose_binds_suite_and_fixture_runs_to_the_bytes(self):
        for name, content, reason in (
            ("suite.log", suite_log(digest="e" * 64), "hostile_suite_not_bound_to_bytes"),
            ("suite.log", suite_log(failed=1), "hostile_suite_failed"),
            ("suite.log", "no tests ran\n", "hostile_suite_failed"),
            ("summary.json", json.dumps({"passed": True, "parser_cases": 12, "parser_binding": {"wasm_sha256": "sha256:" + "e" * 64}}), "fixture_qualification_not_bound_to_bytes"),
            ("summary.json", json.dumps({"passed": False, "parser_cases": 12, "parser_binding": {"wasm_sha256": "sha256:" + DIGEST}}), "fixture_qualification_failed"),
        ):
            with self.subTest(reason=reason):
                original = (self.inputs / name).read_text()
                (self.inputs / name).write_text(content)
                try:
                    composed = self.ci_compose()
                    self.assertEqual(composed.returncode, 1, composed.stdout + composed.stderr)
                    self.assertFalse(self.record()["passed"])
                    self.assertIn(reason, self.record()["failures"])
                    refused = self.check()
                    self.assertEqual(refused.returncode, 1)
                    self.assertIn("record_not_passing", refused.stdout)
                finally:
                    (self.inputs / name).write_text(original)
        manifest = json.loads((self.inputs / "parser-manifest.json").read_text())
        manifest["guest"]["sha256"] = "e" * 64
        (self.inputs / "parser-manifest.json").write_text(json.dumps(manifest))
        self.assertEqual(self.ci_compose().returncode, 1)
        self.assertIn("manifest_not_bound_to_bytes", self.record()["failures"])

    def test_edited_records_are_refused(self):
        self.assertEqual(self.ci_compose().returncode, 0)
        for change, reason in (
            (lambda r: r.update(wasm_sha256="e" * 64), "record_digest_mismatch"),
            (lambda r: r["hostile_suite"].update(tests_failed=1), "record_not_passing"),
            (lambda r: r["fixture_qualification"].update(wasm_sha256="sha256:" + "e" * 64), "record_not_passing"),
            (lambda r: r["manifest"]["guest"].update(sha256="e" * 64), "record_not_passing"),
            (lambda r: r["origin"].update(run_url="https://example.invalid/run"), "origin_not_ci"),
            (lambda r: r.update(schema="other"), "record_schema"),
        ):
            with self.subTest(reason=reason):
                self.assertEqual(self.ci_compose().returncode, 0)
                self.rewrite(change)
                refused = self.check()
                self.assertEqual(refused.returncode, 1, refused.stdout)
                self.assertIn(reason, refused.stdout)
        (self.records / (DIGEST + ".json")).write_text("{not json")
        self.assertEqual(self.check().returncode, 1)

    def test_digest_argument_must_be_lowercase_sha256_hex(self):
        for digest in ("", "sha256:" + DIGEST, DIGEST.upper(), DIGEST[:-1], "../" + DIGEST[3:]):
            with self.subTest(digest=digest):
                self.assertEqual(self.check(digest).returncode, 2)


if __name__ == "__main__":
    unittest.main()
