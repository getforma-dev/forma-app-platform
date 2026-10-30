#!/usr/bin/env python3
"""Refuse a release whose parser bytes have no verified, passing image-bytes record.

ADR-009 D9.1: CI extracts parser_guest.wasm, parser-host and
parser-manifest.json from the built image, runs the APP-004 hostile suite, the
job-mode tests and fixtures/qualify.py against those exact bytes, and records
them. A release refuses an image whose parser bytes have no passing record. The
image's parser-manifest.json check at startup is only an integrity check; this
record is the allowlist.

  check-parser-record.py --manifest F [--wasm-sha256 HEX] [--records DIR]
                         [--release-branch main]
      F is parser-manifest.json extracted from the exact release image. Exit 0
      only if DIR/<guest sha256>.json is a passing record whose manifest equals
      F (guest, host and sources), and the GitHub API shows that its run is a
      successful `push` run of .github/workflows/parser-release.yml in
      getforma-dev/forma-app-platform at the record's commit, the commit is
      reachable from the release branch, and the run's app-017-parser-record
      artifact (its zip digest checked) holds these exact record bytes.
      Requires an authenticated `gh`. `--github-api-fixture DIR` answers the
      API from files instead, for the offline unit tests only; a release
      preflight never passes it, and its output says `recorded-fixture`.
  check-parser-record.py compose --wasm F --host F --manifest F --suite-log F
      --qualify-summary F --output-dir DIR --platform linux/amd64
      [--run-url URL --commit SHA]
      write DIR/<sha256 of the guest>.json; exit 0 only if it passes.

The record's origin is a claim until `check` verifies it through the API: a
record composed anywhere else, or edited after its run, is never a release
pass.
"""
from __future__ import annotations

import argparse
import datetime
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import zipfile

ROOT = Path(__file__).resolve().parents[1]
RECORDS = ROOT / "evidence/APP-017/parser-records"
SCHEMA = "genui-parser-record/2"
REPOSITORY = "getforma-dev/forma-app-platform"
WORKFLOW = ".github/workflows/parser-release.yml"
ARTIFACT = "app-017-parser-record"
HEX = re.compile(r"[0-9a-f]{64}")
COMMIT = re.compile(r"[0-9a-f]{40}")
RUN_URL = re.compile(r"https://github\.com/getforma-dev/forma-app-platform/actions/runs/(\d+)/attempts/(\d+)")
RESULT = re.compile(r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed", re.MULTILINE)
HEADER = re.compile(r"^\s+(Running (?:unittests )?\S+|Doc-tests \S+)(?: \(.*\))?$")
# Every test binary `cargo test -p parser-host` runs, and the fewest tests each
# must pass. A binary that aborts prints no result line for its section.
SECTIONS = {"unittests src/lib.rs": 1, "unittests src/main.rs": 0, "tests/job.rs": 8, "tests/qualification.rs": 17, "Doc-tests parser_host": 0}
CRASH_MARKERS = ("error: test failed", "process didn't exit successfully")
PLATFORMS = {"linux/amd64": "x86_64-unknown-linux-gnu", "linux/arm64": "aarch64-unknown-linux-gnu"}


def sections(log: str) -> tuple[dict[str, list[str]], list[str]]:
    """Split a cargo test log at each test binary's header."""
    found: dict[str, list[str]] = {}
    problems: list[str] = []
    current = None
    for line in log.splitlines():
        header = HEADER.match(line)
        if header:
            name = header.group(1).removeprefix("Running ")
            if name in found:
                problems.append(f"section_repeated:{name}")
            current = name
            found[name] = []
        elif current is not None:
            found[current].append(line)
        elif RESULT.match(line):
            problems.append("result_outside_section")
    return found, problems


def suite(log: str, digest: str, host: str) -> dict:
    found, problems = sections(log)
    problems += [f"crash_marker:{marker}" for marker in CRASH_MARKERS if marker in log]
    counts, passed_total, failed_total, result_lines = {}, 0, 0, 0
    for name, lines in found.items():
        results = RESULT.findall("\n".join(lines))
        result_lines += len(results)
        passed_total += sum(int(p) for _, p, _ in results)
        failed_total += sum(int(f) for _, _, f in results)
        if len(results) != 1:
            problems.append(f"section_result_lines:{name}:{len(results)}")
            continue
        state, passed, failed = results[0]
        if state != "ok" or failed != "0":
            problems.append(f"section_failed:{name}")
        counts[name] = int(passed)
    for name, minimum in SECTIONS.items():
        if name not in found:
            problems.append(f"section_missing:{name}")
        elif name in counts and counts[name] < minimum:
            problems.append(f"section_below_minimum:{name}")
    text = {name: "\n".join(lines) for name, lines in found.items()}
    wasm_lines = re.findall(r"QUALIFIED_WASM sha256:(\S+) ", log)
    host_lines = re.findall(r"QUALIFIED_HOST sha256:(\S+) ", log)
    return {"passed": not problems, "problems": problems, "sections": counts,
            "tests_passed": passed_total, "tests_failed": failed_total, "result_lines": result_lines,
            "bound_to_bytes": wasm_lines == [digest] and f"QUALIFIED_WASM sha256:{digest} " in text.get("tests/qualification.rs", ""),
            "bound_to_host": host_lines == [host] and f"QUALIFIED_HOST sha256:{host} " in text.get("tests/job.rs", "")}


def failures(record: dict) -> list[str]:
    digest, host = record.get("wasm_sha256"), record.get("host_sha256")
    found = []
    hostile = record.get("hostile_suite") or {}
    counts = hostile.get("sections") if isinstance(hostile.get("sections"), dict) else {}
    if not (hostile.get("passed") is True and hostile.get("problems") == [] and hostile.get("tests_failed") == 0
            and all(isinstance(counts.get(name), int) and counts[name] >= minimum for name, minimum in SECTIONS.items())):
        found.append("hostile_suite_failed")
    if hostile.get("bound_to_bytes") is not True:
        found.append("hostile_suite_not_bound_to_bytes")
    if hostile.get("bound_to_host") is not True:
        found.append("hostile_suite_not_bound_to_host")
    fixture = record.get("fixture_qualification") or {}
    if not (fixture.get("passed") is True and isinstance(fixture.get("parser_cases"), int) and fixture["parser_cases"] > 0):
        found.append("fixture_qualification_failed")
    if fixture.get("wasm_sha256") != f"sha256:{digest}":
        found.append("fixture_qualification_not_bound_to_bytes")
    manifest = record.get("manifest") if isinstance(record.get("manifest"), dict) else {}
    if (manifest.get("guest") or {}).get("sha256") != digest:
        found.append("manifest_not_bound_to_bytes")
    if not (isinstance(host, str) and HEX.fullmatch(host)) or (manifest.get("host") or {}).get("sha256") != host:
        found.append("manifest_not_bound_to_host")
    rustc_host = ((manifest.get("source") or {}).get("rustc") or {}).get("host")
    if PLATFORMS.get(record.get("image_platform")) != rustc_host:
        found.append("platform_not_bound_to_manifest")
    return found


def run_reference(origin: dict) -> tuple[int, int] | None:
    """(run id, attempt) when the origin names this repository's run and a commit."""
    url, commit = origin.get("run_url"), origin.get("commit")
    if origin.get("kind") != "github-actions" or not isinstance(url, str) or not isinstance(commit, str) or not COMMIT.fullmatch(commit):
        return None
    match = RUN_URL.fullmatch(url)
    return (int(match.group(1)), int(match.group(2))) if match else None


class GitHub:
    """Read-only GitHub REST calls through `gh api`, or recorded responses keyed by path."""

    def __init__(self, fixture: Path | None):
        self.fixture = fixture

    def raw(self, path: str) -> bytes:
        if self.fixture is not None:
            try:
                return (self.fixture / path).read_bytes()
            except OSError as error:
                raise LookupError(path) from error
        try:
            done = subprocess.run(["gh", "api", "-H", "Accept: application/vnd.github+json", path], capture_output=True, timeout=300)
        except (OSError, subprocess.TimeoutExpired) as error:
            raise LookupError(path) from error
        if done.returncode != 0:
            raise LookupError(path)
        return done.stdout

    def json(self, path: str):
        try:
            return json.loads(self.raw(path))
        except ValueError as error:
            raise LookupError(path) from error


def verify_origin(api: GitHub, run_id: int, attempt: int, commit: str, record_name: str, record_bytes: bytes, branch: str) -> list[str]:
    base = f"repos/{REPOSITORY}"
    reasons = []
    try:
        run = api.json(f"{base}/actions/runs/{run_id}/attempts/{attempt}")
        if run.get("id") != run_id or run.get("run_attempt") != attempt:
            reasons.append("run_identity_mismatch")
        if (run.get("repository") or {}).get("full_name") != REPOSITORY or (run.get("head_repository") or {}).get("full_name") != REPOSITORY:
            reasons.append("run_not_in_repository")
        if run.get("path") != WORKFLOW:
            reasons.append("run_not_parser_release")
        if run.get("event") != "push":
            reasons.append("run_not_push")
        if run.get("status") != "completed" or run.get("conclusion") != "success":
            reasons.append("run_not_successful")
        if run.get("head_sha") != commit:
            reasons.append("run_commit_mismatch")
        comparison = api.json(f"{base}/compare/{commit}...{branch}")
        if comparison.get("status") not in ("ahead", "identical"):
            reasons.append("commit_not_on_release_branch")
        listing = api.json(f"{base}/actions/runs/{run_id}/artifacts")
        candidates = [artifact for artifact in listing.get("artifacts") or []
                      if artifact.get("name") == ARTIFACT and artifact.get("expired") is False
                      and (artifact.get("workflow_run") or {}).get("id") == run_id
                      and (artifact.get("workflow_run") or {}).get("head_sha") == commit]
        if not candidates:
            return reasons + ["run_artifact_missing"]
        artifact_reasons = []
        for artifact in candidates:
            archive = api.raw(f"{base}/actions/artifacts/{int(artifact['id'])}/zip")
            if artifact.get("digest") != "sha256:" + hashlib.sha256(archive).hexdigest():
                artifact_reasons.append("run_artifact_digest_mismatch")
                continue
            try:
                with zipfile.ZipFile(io.BytesIO(archive)) as bundle:
                    uploaded = bundle.read(record_name)
            except (KeyError, zipfile.BadZipFile):
                artifact_reasons.append("run_artifact_has_no_record")
                continue
            if uploaded == record_bytes:
                return reasons
            artifact_reasons.append("record_differs_from_run_artifact")
        return reasons + sorted(set(artifact_reasons))
    except LookupError:
        return reasons + ["github_api_unavailable"]


def check(args: argparse.Namespace) -> int:
    def finish(code: int, reasons: list[str], **fields) -> int:
        print(json.dumps({"admitted": not reasons and code == 0, **fields, "reasons": reasons}))
        return code if reasons or code else 0

    try:
        manifest = json.loads(args.manifest.read_text())
        digest = manifest["guest"]["sha256"]
        host = manifest["host"]["sha256"]
        sources = manifest["source"]["sources_sha256"]
    except (OSError, ValueError, KeyError, TypeError):
        return finish(1, ["manifest_unreadable"])
    if not (isinstance(digest, str) and HEX.fullmatch(digest)):
        return finish(2, ["usage: the manifest's guest sha256 is not 64 lowercase hex characters"])
    if args.wasm_sha256 is not None and not HEX.fullmatch(args.wasm_sha256):
        return finish(2, ["usage: --wasm-sha256 takes 64 lowercase hex characters"])
    if args.wasm_sha256 is not None and args.wasm_sha256 != digest:
        return finish(1, ["manifest_guest_mismatch"], wasm_sha256=args.wasm_sha256)
    path = args.records / f"{digest}.json"
    shown = str(path.resolve().relative_to(ROOT) if path.resolve().is_relative_to(ROOT) else path)
    fields = {"wasm_sha256": digest, "host_sha256": host, "record": shown,
              "origin_verification": "recorded-fixture" if args.github_api_fixture else "github-api"}
    try:
        raw = path.read_bytes()
        record = json.loads(raw)
    except FileNotFoundError:
        return finish(1, ["no_record"], **fields)
    except (OSError, ValueError):
        return finish(1, ["record_unreadable"], **fields)
    reasons = []
    if not isinstance(record, dict) or record.get("schema") != SCHEMA:
        reasons.append("record_schema")
        record = {}
    if record.get("wasm_sha256") != digest:
        reasons.append("record_digest_mismatch")
    found = failures(record)
    if record.get("passed") is not True or found:
        reasons += ["record_not_passing", *found]
    recorded = record.get("manifest") if isinstance(record.get("manifest"), dict) else {}
    if (recorded.get("host") or {}).get("sha256") != host:
        reasons.append("record_host_not_release_manifest")
    if (recorded.get("source") or {}).get("sources_sha256") != sources:
        reasons.append("record_sources_not_release_manifest")
    if recorded != manifest:
        reasons.append("record_manifest_not_release_manifest")
    reference = run_reference(record.get("origin") or {})
    if reference is None:
        reasons.append("origin_not_ci")
    if reasons:
        return finish(1, reasons, **fields)
    reasons = verify_origin(GitHub(args.github_api_fixture), *reference, record["origin"]["commit"], path.name, raw, args.release_branch)
    return finish(1 if reasons else 0, reasons, **fields, run_url=record["origin"]["run_url"])


def compose(args: argparse.Namespace) -> int:
    if (args.run_url is None) != (args.commit is None):
        print("compose: --run-url and --commit go together", file=sys.stderr)
        return 2
    if args.run_url is not None and not (RUN_URL.fullmatch(args.run_url) and COMMIT.fullmatch(args.commit)):
        print(f"compose: --run-url must be https://github.com/{REPOSITORY}/actions/runs/<id>/attempts/<n> and --commit a full SHA", file=sys.stderr)
        return 2
    if args.platform not in PLATFORMS:
        print(f"compose: --platform is one of {sorted(PLATFORMS)}", file=sys.stderr)
        return 2
    wasm = args.wasm.read_bytes()
    host = args.host.read_bytes()
    digest = hashlib.sha256(wasm).hexdigest()
    host_digest = hashlib.sha256(host).hexdigest()
    manifest = json.loads(args.manifest.read_text())
    summary = json.loads(args.qualify_summary.read_text())
    hostile = suite(args.suite_log.read_text(), digest, host_digest)
    ci = os.environ.get("GITHUB_ACTIONS") == "true" and args.run_url is not None
    origin = {"kind": "github-actions" if ci else "local", "run_url": args.run_url if ci else None, "commit": args.commit if ci else None}
    record = {
        "schema": SCHEMA,
        "wasm_sha256": digest,
        "wasm_bytes": len(wasm),
        "host_sha256": host_digest,
        "host_bytes": len(host),
        "image_platform": args.platform,
        "origin": origin,
        "manifest": manifest,
        "hostile_suite": {**hostile, "log_sha256": hashlib.sha256(args.suite_log.read_bytes()).hexdigest()},
        "fixture_qualification": {"passed": summary.get("passed") is True, "parser_cases": summary.get("parser_cases"),
                                  "wasm_sha256": (summary.get("parser_binding") or {}).get("wasm_sha256"),
                                  "summary_sha256": hashlib.sha256(args.qualify_summary.read_bytes()).hexdigest()},
        "recorded_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    }
    record["failures"] = failures(record)
    record["passed"] = not record["failures"]
    args.output_dir.mkdir(parents=True, exist_ok=True)
    output = args.output_dir / f"{digest}.json"
    output.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"passed": record["passed"], "wasm_sha256": digest, "host_sha256": host_digest, "origin": origin["kind"],
                      "output": str(output), "failures": record["failures"], "suite_problems": hostile["problems"]}))
    return 0 if record["passed"] else 1


def main(argv: list[str]) -> int:
    if argv[:1] == ["compose"]:
        parser = argparse.ArgumentParser(prog="check-parser-record.py compose")
        for name in ("--wasm", "--host", "--manifest", "--suite-log", "--qualify-summary", "--output-dir"):
            parser.add_argument(name, type=Path, required=True)
        parser.add_argument("--platform", required=True)
        parser.add_argument("--run-url")
        parser.add_argument("--commit")
        return compose(parser.parse_args(argv[1:]))
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--wasm-sha256")
    parser.add_argument("--records", type=Path, default=RECORDS)
    parser.add_argument("--release-branch", default="main")
    parser.add_argument("--github-api-fixture", type=Path, help=argparse.SUPPRESS)
    return check(parser.parse_args(argv))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
