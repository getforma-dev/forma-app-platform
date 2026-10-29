"""Release refusal for parser bytes without a passing image-bytes record (ADR-009 D9.1).

Run: python3 -m unittest discover -s scripts -p 'test_*.py' -v

`check` verifies a record's CI origin through the GitHub API. These tests use
the offline path, `--github-api-fixture DIR`, whose files are API responses
keyed by request path. The response bodies start from responses recorded from
run 40963687082 of this repository's parser-release.yml
(scripts/fixtures/parser-record-api/) and are edited per case.
"""
import copy
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "check-parser-record.py"
RECORDED = ROOT / "scripts" / "fixtures" / "parser-record-api"
REPO = "getforma-dev/forma-app-platform"
WASM = b"\0asm\x01\0\0\0synthetic-guest"
DIGEST = hashlib.sha256(WASM).hexdigest()
HOST = b"\x7fELF synthetic parser-host"
HOST_DIGEST = hashlib.sha256(HOST).hexdigest()
RUN_ID = 40963687082
RUN_URL = f"https://github.com/{REPO}/actions/runs/{RUN_ID}/attempts/1"
COMMIT = "a" * 40
ARTIFACT_ID = 12937866961


def load_module():
    spec = importlib.util.spec_from_file_location("check_parser_record", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run(*args, env=None):
    environment = {key: value for key, value in os.environ.items() if not key.startswith("GITHUB_")}
    environment.update(env or {})
    return subprocess.run([sys.executable, str(SCRIPT), *map(str, args)], capture_output=True, text=True, timeout=30, env=environment)


RESULT = "test result: {state}. {passed} passed; {failed} failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s\n"


def section(header, body, passed, failed=0):
    state = "FAILED" if failed else "ok"
    return f"     {header}\n\nrunning {passed + failed} tests\n{body}\n" + RESULT.format(state=state, passed=passed, failed=failed) + "\n"


def suite_log(digest=DIGEST, host=HOST_DIGEST, job=8, qualification=17, failed=0, drop=None):
    """A log shaped like `cargo test -p parser-host -- --test-threads=1 --nocapture`."""
    sections = [
        ("lib", section("Running unittests src/lib.rs (target/debug/deps/parser_host-1)", "test tests::sha ... ok", 1)),
        ("main", section("Running unittests src/main.rs (target/debug/deps/parser_host-2)", "", 0)),
        ("job", section("Running tests/job.rs (target/debug/deps/job-3)",
                        f"test qualified_host_bytes_are_named_in_the_log ... QUALIFIED_HOST sha256:{host} bytes={len(HOST)} path=/x/parser-host\nok", job)),
        ("qualification", section("Running tests/qualification.rs (target/debug/deps/qualification-4)",
                                  f"test qualified_guest_bytes_are_named_in_the_log ... QUALIFIED_WASM sha256:{digest} bytes={len(WASM)} path=/x\nok",
                                  qualification - failed, failed)),
        ("doc", section("Doc-tests parser_host", "", 0)),
    ]
    head = "   Compiling parser-host v0.1.0 (/x/parser/host)\n    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.00s\n"
    return head + "".join(text for name, text in sections if name != drop)


def crashed_log():
    """The review's M1 reproduction: qualification.rs aborts after the binding test."""
    log = suite_log()
    cut = log.index("ok\n", log.index("QUALIFIED_WASM")) + 3
    return (log[:cut] + "test resource_profiles_have_their_caps ... \nthread 'resource_profiles_have_their_caps' has overflowed its stack\n"
            "fatal runtime error: stack overflow, aborting\nerror: test failed, to rerun pass `--test qualification`\n\nCaused by:\n"
            "  process didn't exit successfully: `/x/target/debug/deps/qualification-4 --test-threads=1 --nocapture` (signal: 6, SIGABRT: process abort signal)\n")


def manifest_document(guest=DIGEST, host=HOST_DIGEST, rustc_host="x86_64-unknown-linux-gnu"):
    return {"schema": "genui-parser-manifest/1",
            "guest": {"path": "/app/parser/parser_guest.wasm", "sha256": guest, "bytes": len(WASM)},
            "host": {"path": "/app/parser/parser-host", "sha256": host, "bytes": len(HOST)},
            "source": {"sources_sha256": "c" * 64, "cargo_lock_sha256": "d" * 64, "rust_toolchain": "1.93.1",
                       "rustc": {"release": "1.93.1", "commit-hash": "e" * 40, "LLVM version": "21.1.8", "host": rustc_host}}}


class ParserRecordTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.records = self.root / "records"
        self.records.mkdir()
        self.inputs = self.root / "inputs"
        self.inputs.mkdir()
        self.api = self.root / "api"
        (self.inputs / "parser_guest.wasm").write_bytes(WASM)
        (self.inputs / "parser-host").write_bytes(HOST)
        self.write_manifest(manifest_document())
        (self.inputs / "suite.log").write_text(suite_log())
        (self.inputs / "summary.json").write_text(json.dumps({"passed": True, "parser_cases": 12, "parser_binding": {"wasm_sha256": "sha256:" + DIGEST}}))

    def tearDown(self):
        self.directory.cleanup()

    def write_manifest(self, document, name="parser-manifest.json"):
        (self.inputs / name).write_text(json.dumps(document))
        return self.inputs / name

    # -- compose -------------------------------------------------------------

    def compose(self, env=None, **overrides):
        args = {"--wasm": self.inputs / "parser_guest.wasm", "--host": self.inputs / "parser-host", "--manifest": self.inputs / "parser-manifest.json",
                "--suite-log": self.inputs / "suite.log", "--qualify-summary": self.inputs / "summary.json", "--output-dir": self.records,
                "--platform": "linux/amd64"}
        args.update(overrides)
        return run("compose", *[item for pair in args.items() for item in pair if item is not None], env=env)

    def ci_compose(self, **overrides):
        return self.compose(env={"GITHUB_ACTIONS": "true"}, **{"--run-url": RUN_URL, "--commit": COMMIT, **overrides})

    def record_path(self):
        return self.records / (DIGEST + ".json")

    def record(self):
        return json.loads(self.record_path().read_text())

    def rewrite(self, change):
        record = self.record()
        change(record)
        self.record_path().write_text(json.dumps(record))

    # -- the recorded GitHub API, edited per case ------------------------------

    def publish(self, attempt=None, artifacts=None, compare=None, zip_record=None, digest=None):
        """Lay out API responses for RUN_ID as a passing push run that uploaded this record."""
        record_bytes = self.record_path().read_bytes() if zip_record is None else zip_record
        archive = io.BytesIO()
        with zipfile.ZipFile(archive, "w") as bundle:
            bundle.writestr(DIGEST + ".json", record_bytes)
        archive = archive.getvalue()
        run_attempt = json.loads((RECORDED / "run-attempt.json").read_text())
        run_attempt.update(conclusion="success", head_sha=COMMIT, head_branch="main")
        listing = json.loads((RECORDED / "run-artifacts.json").read_text())
        listing["artifacts"][0]["digest"] = digest or "sha256:" + hashlib.sha256(archive).hexdigest()
        listing["artifacts"][0]["workflow_run"].update(head_sha=COMMIT, head_branch="main")
        comparison = json.loads((RECORDED / "compare.json").read_text())
        comparison.update(status="ahead", ahead_by=3, behind_by=0, base_commit={"sha": COMMIT}, merge_base_commit={"sha": COMMIT})
        for document, change in ((run_attempt, attempt), (listing, artifacts), (comparison, compare)):
            if change:
                change(document)
        base = self.api / "repos" / REPO
        for relative, body in ((f"actions/runs/{RUN_ID}/attempts/1", json.dumps(run_attempt)), (f"actions/runs/{RUN_ID}/artifacts", json.dumps(listing)),
                               (f"compare/{COMMIT}...main", json.dumps(comparison)), (f"actions/artifacts/{ARTIFACT_ID}/zip", archive)):
            path = base / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            (path.write_bytes if isinstance(body, bytes) else path.write_text)(body)

    def check(self, *extra, manifest=None):
        return run("--manifest", manifest or self.inputs / "parser-manifest.json", "--records", self.records, "--github-api-fixture", self.api, *extra)

    def assert_refused(self, outcome, reason):
        self.assertEqual(outcome.returncode, 1, outcome.stdout + outcome.stderr)
        self.assertFalse(json.loads(outcome.stdout)["admitted"])
        self.assertIn(reason, json.loads(outcome.stdout)["reasons"])

    # -- the tests -------------------------------------------------------------

    def test_release_refuses_parser_digest_without_passing_record(self):
        self.assert_refused(self.check(), "no_record")
        self.assertEqual(self.ci_compose().returncode, 0)
        other = hashlib.sha256(b"other guest bytes").hexdigest()
        self.assert_refused(self.check(manifest=self.write_manifest(manifest_document(guest=other), "other.json")), "no_record")

    def test_verified_ci_record_admits_exactly_its_release_manifest(self):
        composed = self.ci_compose()
        self.assertEqual(composed.returncode, 0, composed.stdout + composed.stderr)
        record = self.record()
        self.assertTrue(record["passed"])
        self.assertEqual(record["schema"], "genui-parser-record/2")
        self.assertEqual((record["wasm_sha256"], record["host_sha256"], record["image_platform"]), (DIGEST, HOST_DIGEST, "linux/amd64"))
        self.assertEqual(record["origin"], {"kind": "github-actions", "run_url": RUN_URL, "commit": COMMIT})
        self.assertEqual(record["hostile_suite"]["sections"],
                         {"unittests src/lib.rs": 1, "unittests src/main.rs": 0, "tests/job.rs": 8, "tests/qualification.rs": 17, "Doc-tests parser_host": 0})
        self.publish()
        admitted = self.check()
        self.assertEqual(admitted.returncode, 0, admitted.stdout + admitted.stderr)
        result = json.loads(admitted.stdout)
        self.assertEqual((result["admitted"], result["reasons"]), (True, []))
        self.assertEqual(result["origin_verification"], "recorded-fixture")
        self.assertEqual(result["host_sha256"], HOST_DIGEST)
        self.assertEqual(self.check("--wasm-sha256", DIGEST).returncode, 0)
        self.assert_refused(self.check("--wasm-sha256", "e" * 64), "manifest_guest_mismatch")

    def test_record_must_match_the_release_image_manifest(self):
        self.assertEqual(self.ci_compose().returncode, 0)
        self.publish()
        base = manifest_document()
        for change, reason in (
            (lambda m: m["host"].update(sha256="e" * 64), "record_host_not_release_manifest"),
            (lambda m: m["source"].update(sources_sha256="e" * 64), "record_sources_not_release_manifest"),
            (lambda m: m["source"].update(cargo_lock_sha256="e" * 64), "record_manifest_not_release_manifest"),
        ):
            with self.subTest(reason=reason):
                document = copy.deepcopy(base)
                change(document)
                self.assert_refused(self.check(manifest=self.write_manifest(document, "release.json")), reason)
        (self.inputs / "broken.json").write_text("{")
        self.assert_refused(self.check(manifest=self.inputs / "broken.json"), "manifest_unreadable")

    def test_origin_is_verified_through_the_github_api_not_self_declared(self):
        self.assertEqual(self.ci_compose().returncode, 0)
        for publish, reason in (
            ({"attempt": lambda r: r.update(path=".github/workflows/other.yml")}, "run_not_parser_release"),
            ({"attempt": lambda r: r.update(event="pull_request")}, "run_not_push"),
            ({"attempt": lambda r: r.update(conclusion="failure")}, "run_not_successful"),
            ({"attempt": lambda r: r.update(status="in_progress", conclusion=None)}, "run_not_successful"),
            ({"attempt": lambda r: r.update(head_sha="b" * 40)}, "run_commit_mismatch"),
            ({"attempt": lambda r: r["repository"].update(full_name="someone-else/fork")}, "run_not_in_repository"),
            ({"attempt": lambda r: r["head_repository"].update(full_name="someone-else/fork")}, "run_not_in_repository"),
            ({"attempt": lambda r: r.update(id=1)}, "run_identity_mismatch"),
            ({"compare": lambda c: c.update(status="diverged")}, "commit_not_on_release_branch"),
            ({"compare": lambda c: c.update(status="behind")}, "commit_not_on_release_branch"),
            ({"artifacts": lambda a: a.update(artifacts=[], total_count=0)}, "run_artifact_missing"),
            ({"artifacts": lambda a: a["artifacts"][0].update(expired=True)}, "run_artifact_missing"),
            ({"artifacts": lambda a: a["artifacts"][0]["workflow_run"].update(id=1)}, "run_artifact_missing"),
            ({"digest": "sha256:" + "0" * 64}, "run_artifact_digest_mismatch"),
            ({"zip_record": b"{}"}, "record_differs_from_run_artifact"),
        ):
            with self.subTest(reason=reason):
                self.publish(**publish)
                self.assert_refused(self.check(), reason)
        # No response for a request is never an admission.
        self.publish()
        (self.api / "repos" / REPO / f"actions/artifacts/{ARTIFACT_ID}/zip").unlink()
        self.assert_refused(self.check(), "github_api_unavailable")

    def test_review_m2_self_declared_origin_is_refused(self):
        # A record composed on a workstation with GITHUB_ACTIONS=true and a
        # fork's run URL was admitted before; compose now refuses the URL ...
        composed = self.compose(env={"GITHUB_ACTIONS": "true"}, **{"--run-url": "https://github.com/someone-else/fork/actions/runs/42/attempts/1", "--commit": "b" * 40})
        self.assertEqual(composed.returncode, 2, composed.stdout + composed.stderr)
        self.assertFalse(self.record_path().exists())
        for url in (f"https://github.com/{REPO}/actions/runs/42", f"https://github.com/{REPO}/actions/runs/42/attempts/1/extra"):
            self.assertEqual(self.ci_compose(**{"--run-url": url}).returncode, 2, url)
        # ... and a record naming this repository's run is admitted only when
        # the API shows that run produced exactly these bytes.
        self.assertEqual(self.ci_compose().returncode, 0)
        self.publish(zip_record=b"{\"schema\": \"genui-parser-record/2\"}")
        self.assert_refused(self.check(), "record_differs_from_run_artifact")

    def test_local_record_is_evidence_never_a_release_pass(self):
        composed = self.compose()
        self.assertEqual(composed.returncode, 0, composed.stdout + composed.stderr)
        self.assertEqual(self.record()["origin"], {"kind": "local", "run_url": None, "commit": None})
        self.publish()
        self.assert_refused(self.check(), "origin_not_ci")

    def test_compose_binds_suite_and_fixture_runs_to_the_bytes(self):
        moved = suite_log().replace("QUALIFIED_WASM", "MOVED").replace("QUALIFIED_HOST sha256:" + HOST_DIGEST,
                                                                        f"QUALIFIED_HOST sha256:{HOST_DIGEST} bytes=1 path=/x\nQUALIFIED_WASM sha256:{DIGEST}")
        doubled = suite_log().replace(RESULT.format(state="ok", passed=8, failed=0), RESULT.format(state="ok", passed=8, failed=0) * 2)
        stray = RESULT.format(state="ok", passed=3, failed=0) + suite_log()
        for name, content, reason in (
            ("suite.log", suite_log(digest="e" * 64), "hostile_suite_not_bound_to_bytes"),
            ("suite.log", moved, "hostile_suite_not_bound_to_bytes"),
            ("suite.log", suite_log(host="e" * 64), "hostile_suite_not_bound_to_host"),
            ("suite.log", suite_log(failed=1), "hostile_suite_failed"),
            ("suite.log", "no tests ran\n", "hostile_suite_failed"),
            ("suite.log", crashed_log(), "hostile_suite_failed"),
            ("suite.log", suite_log() + "error: test failed, to rerun pass `--test job`\n", "hostile_suite_failed"),
            ("suite.log", suite_log() + "  process didn't exit successfully: `x` (exit status: 101)\n", "hostile_suite_failed"),
            ("suite.log", suite_log(drop="job"), "hostile_suite_failed"),
            ("suite.log", suite_log(drop="doc"), "hostile_suite_failed"),
            ("suite.log", suite_log(job=7), "hostile_suite_failed"),
            ("suite.log", suite_log(qualification=16), "hostile_suite_failed"),
            ("suite.log", doubled, "hostile_suite_failed"),
            ("suite.log", stray, "hostile_suite_failed"),
            ("summary.json", json.dumps({"passed": True, "parser_cases": 12, "parser_binding": {"wasm_sha256": "sha256:" + "e" * 64}}), "fixture_qualification_not_bound_to_bytes"),
            ("summary.json", json.dumps({"passed": False, "parser_cases": 12, "parser_binding": {"wasm_sha256": "sha256:" + DIGEST}}), "fixture_qualification_failed"),
            ("parser-manifest.json", json.dumps(manifest_document(guest="e" * 64)), "manifest_not_bound_to_bytes"),
            ("parser-manifest.json", json.dumps(manifest_document(host="e" * 64)), "manifest_not_bound_to_host"),
            ("parser-manifest.json", json.dumps(manifest_document(rustc_host="aarch64-unknown-linux-gnu")), "platform_not_bound_to_manifest"),
        ):
            release = self.write_manifest(manifest_document(), "release.json")
            with self.subTest(reason=reason, content=content[-80:]):
                original = (self.inputs / name).read_text()
                (self.inputs / name).write_text(content)
                try:
                    composed = self.ci_compose()
                    self.assertEqual(composed.returncode, 1, composed.stdout + composed.stderr)
                    self.assertFalse(self.record()["passed"])
                    self.assertIn(reason, self.record()["failures"])
                    self.publish()
                    self.assert_refused(self.check(manifest=release), "record_not_passing")
                finally:
                    (self.inputs / name).write_text(original)

    def test_suite_sections_parse_the_actual_cargo_log(self):
        # The image-bytes log from the packaging evidence (before QUALIFIED_HOST existed).
        log = (ROOT / "evidence/APP-017/parser-packaging/hostile-suite-image-bytes.log").read_text()
        digest = re.search(r"QUALIFIED_WASM sha256:([0-9a-f]{64}) ", log).group(1)
        result = load_module().suite(log, digest, HOST_DIGEST)
        self.assertEqual(result["sections"], {"unittests src/lib.rs": 1, "unittests src/main.rs": 0, "tests/job.rs": 7,
                                              "tests/qualification.rs": 17, "Doc-tests parser_host": 0})
        self.assertTrue(result["bound_to_bytes"])
        self.assertFalse(result["bound_to_host"])
        self.assertIn("section_below_minimum:tests/job.rs", result["problems"])
        self.assertFalse(result["passed"])

    def test_edited_records_are_refused(self):
        self.assertEqual(self.ci_compose().returncode, 0)
        for change, reason in (
            (lambda r: r.update(wasm_sha256="e" * 64), "record_digest_mismatch"),
            (lambda r: r["hostile_suite"].update(tests_failed=1), "record_not_passing"),
            (lambda r: r["hostile_suite"]["sections"].update({"tests/job.rs": 7}), "record_not_passing"),
            (lambda r: r["fixture_qualification"].update(wasm_sha256="sha256:" + "e" * 64), "record_not_passing"),
            (lambda r: r["manifest"]["guest"].update(sha256="e" * 64), "record_not_passing"),
            (lambda r: r.update(host_sha256="e" * 64), "record_not_passing"),
            (lambda r: r["origin"].update(run_url="https://example.invalid/run"), "origin_not_ci"),
            (lambda r: r["origin"].update(run_url=f"https://github.com/someone-else/fork/actions/runs/{RUN_ID}/attempts/1"), "origin_not_ci"),
            (lambda r: r.update(schema="genui-parser-record/1"), "record_schema"),
        ):
            with self.subTest(reason=reason):
                self.assertEqual(self.ci_compose().returncode, 0)
                self.publish()
                self.rewrite(change)
                self.assert_refused(self.check(), reason)
        self.record_path().write_text("{not json")
        self.assertEqual(self.check().returncode, 1)

    def test_digest_argument_must_be_lowercase_sha256_hex(self):
        for digest in ("", "sha256:" + DIGEST, DIGEST.upper(), DIGEST[:-1], "../" + DIGEST[3:]):
            with self.subTest(digest=digest):
                self.assertEqual(self.check("--wasm-sha256", digest).returncode, 2)
        self.assertEqual(self.check(manifest=self.write_manifest(manifest_document(guest="../" + DIGEST[3:]), "evil.json")).returncode, 2)


class ReleaseWorkflowTests(unittest.TestCase):
    """Static properties of parser-release.yml that the record relies on."""

    def setUp(self):
        self.text = (ROOT / ".github/workflows/parser-release.yml").read_text()

    def job(self, name):
        start = self.text.index(f"\n  {name}:\n")
        following = re.search(r"\n  [a-z-]+:\n", self.text[start + 1:])
        return self.text[start:start + 1 + following.start()] if following else self.text[start:]

    def test_every_job_that_pipes_runs_bash_with_pipefail(self):
        for name in re.findall(r"\n  ([a-z-]+):\n", self.text.split("\njobs:\n", 1)[1]):
            job = self.job(name)
            if "| tee" in job:
                self.assertRegex(job, r"\n    defaults:\n      run:\n(?:        .*\n)*?        shell: bash\n", name)

    def test_record_is_composed_on_push_for_this_repository_only(self):
        job = self.job("image-bytes")
        compose = job[job.index("check-parser-record.py compose") - 400:job.index("check-parser-record.py compose")]
        self.assertIn("if: github.event_name == 'push'", compose)
        self.assertIn("--platform linux/amd64", job)
        self.assertIn("docker build --platform linux/amd64", job)
        self.assertIn("name: app-017-parser-record\n          path: evidence/APP-017/parser-records\n", job)


if __name__ == "__main__":
    unittest.main()
