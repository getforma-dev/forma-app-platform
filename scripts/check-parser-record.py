#!/usr/bin/env python3
"""Refuse a release whose parser guest digest has no passing image-bytes record.

ADR-009 D9.1: CI extracts parser_guest.wasm from the built image, runs the
APP-004 hostile suite and fixtures/qualify.py against those exact bytes, and
records the digest. A release refuses an image whose guest digest has no
passing record. The image's parser-manifest.json check at startup is only an
integrity check; this record is the allowlist.

  check-parser-record.py --wasm-sha256 HEX [--records DIR]
      exit 0 only if DIR/HEX.json is a passing record made by GitHub Actions.
  check-parser-record.py compose --wasm F --manifest F --suite-log F
      --qualify-summary F --output-dir DIR [--run-url URL --commit SHA]
      write DIR/<sha256 of F>.json; exit 0 only if it passes.

A record composed outside GitHub Actions is retained as local evidence and is
never a release pass.
"""
from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
RECORDS = ROOT / "evidence/APP-017/parser-records"
SCHEMA = "genui-parser-record/1"
HEX = re.compile(r"[0-9a-f]{64}")
RESULT = re.compile(r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed", re.MULTILINE)
RUN_URL = re.compile(r"https://github\.com/[A-Za-z0-9._-]+/[A-Za-z0-9._-]+/actions/runs/\d+(/attempts/\d+)?")
COMMIT = re.compile(r"[0-9a-f]{40}")


def suite(log: str, digest: str) -> dict:
    results = RESULT.findall(log)
    passed = sum(int(p) for _, p, _ in results)
    failed = sum(int(f) for _, _, f in results)
    return {"passed": bool(results) and all(state == "ok" for state, _, _ in results) and failed == 0 and passed > 0,
            "tests_passed": passed, "tests_failed": failed, "result_lines": len(results),
            "bound_to_bytes": f"QUALIFIED_WASM sha256:{digest} " in log}


def failures(record: dict) -> list[str]:
    digest = record.get("wasm_sha256")
    found = []
    hostile = record.get("hostile_suite") or {}
    if not (hostile.get("passed") is True and hostile.get("tests_failed") == 0 and isinstance(hostile.get("tests_passed"), int) and hostile["tests_passed"] > 0):
        found.append("hostile_suite_failed")
    if hostile.get("bound_to_bytes") is not True:
        found.append("hostile_suite_not_bound_to_bytes")
    fixture = record.get("fixture_qualification") or {}
    if not (fixture.get("passed") is True and isinstance(fixture.get("parser_cases"), int) and fixture["parser_cases"] > 0):
        found.append("fixture_qualification_failed")
    if fixture.get("wasm_sha256") != f"sha256:{digest}":
        found.append("fixture_qualification_not_bound_to_bytes")
    if ((record.get("manifest") or {}).get("guest") or {}).get("sha256") != digest:
        found.append("manifest_not_bound_to_bytes")
    return found


def origin_is_ci(origin: dict) -> bool:
    return (origin.get("kind") == "github-actions" and isinstance(origin.get("run_url"), str) and RUN_URL.fullmatch(origin["run_url"]) is not None
            and isinstance(origin.get("commit"), str) and COMMIT.fullmatch(origin["commit"]) is not None)


def check(digest: str, records: Path) -> int:
    if not HEX.fullmatch(digest):
        print(json.dumps({"admitted": False, "reasons": ["usage: --wasm-sha256 takes 64 lowercase hex characters"]}))
        return 2
    path = records / f"{digest}.json"
    reasons = []
    try:
        record = json.loads(path.read_text())
    except FileNotFoundError:
        record, reasons = None, ["no_record"]
    except (OSError, ValueError):
        record, reasons = None, ["record_unreadable"]
    if record is not None:
        if not isinstance(record, dict) or record.get("schema") != SCHEMA:
            reasons.append("record_schema")
            record = {}
        if record.get("wasm_sha256") != digest:
            reasons.append("record_digest_mismatch")
        found = failures(record)
        if record.get("passed") is not True or found:
            reasons += ["record_not_passing", *found]
        if not origin_is_ci(record.get("origin") or {}):
            reasons.append("origin_not_ci")
    shown = path.resolve().relative_to(ROOT) if path.resolve().is_relative_to(ROOT) else path
    print(json.dumps({"admitted": not reasons, "wasm_sha256": digest, "record": str(shown), "reasons": reasons}))
    return 0 if not reasons else 1


def compose(args: argparse.Namespace) -> int:
    wasm = args.wasm.read_bytes()
    digest = hashlib.sha256(wasm).hexdigest()
    manifest = json.loads(args.manifest.read_text())
    summary = json.loads(args.qualify_summary.read_text())
    hostile = suite(args.suite_log.read_text(), digest)
    ci = os.environ.get("GITHUB_ACTIONS") == "true"
    origin = {"kind": "github-actions" if ci else "local", "run_url": args.run_url, "commit": args.commit}
    record = {
        "schema": SCHEMA,
        "wasm_sha256": digest,
        "wasm_bytes": len(wasm),
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
    print(json.dumps({"passed": record["passed"], "wasm_sha256": digest, "origin": origin["kind"], "output": str(output), "failures": record["failures"]}))
    return 0 if record["passed"] else 1


def main(argv: list[str]) -> int:
    if argv[:1] == ["compose"]:
        parser = argparse.ArgumentParser(prog="check-parser-record.py compose")
        for name in ("--wasm", "--manifest", "--suite-log", "--qualify-summary", "--output-dir"):
            parser.add_argument(name, type=Path, required=True)
        parser.add_argument("--run-url")
        parser.add_argument("--commit")
        return compose(parser.parse_args(argv[1:]))
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--wasm-sha256", required=True)
    parser.add_argument("--records", type=Path, default=RECORDS)
    args = parser.parse_args(argv)
    return check(args.wasm_sha256, args.records)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
