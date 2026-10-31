#!/usr/bin/env python3
"""The beta release record (ADR-013 D13.5): one `release.json` per window.

`release.json` lives in `$PRIV/windows/<window-id>/` (private, 0600). It is
the identity of a deploy: the candidate commit, the three ECR digests, the
stack's task definition, the database version, the approvals that let each
mutation run and the times. `docs/pitch/RELEASES.md` carries one non-secret
row per activated window, derived from it (`row`, `append`), and the window's
`beta/<yyyymmdd>-<sha7>` tag is named from it (`tag`).

Stdlib only; run as `python3 -I ops/beta/release.py <command> ...`. Every
other field is set by `ops/beta/deploy.sh` with jq as the steps run.

  release.py init --window <id> --supersedes <id> --candidate <sha40> --branch <name>
                  --database-version <n> [--build <build.json>]          > release.json
  release.py check <release.json> [--activated]
  release.py row <release.json>                  the RELEASES.md row
  release.py append <release.json> <RELEASES.md> append the row once (idempotent)
  release.py tag <release.json>                  beta/<yyyymmdd>-<sha7>
  release.py import-build <release.json> <build.json>   (re)read the local image record
"""
import argparse
import datetime
import json
import re
import sys
from pathlib import Path

SCHEMA_VERSION = 1
# name -> ECR repository (DEPLOY-RUNBOOK 2.1; the stack's three image parameters)
IMAGES = {"service": "genui-synthetic", "qualification": "genui-qualification", "operator": "genui-operator"}
UUID4 = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")
SHA40 = re.compile(r"^[0-9a-f]{40}$")
DIGEST = re.compile(r"^sha256:[0-9a-f]{64}$")
UTC = re.compile(r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$")
TABLE_HEADER = "| Activated (UTC) | Candidate | Tag | Window | Task definition | Schema | Service digest | Qualification digest | Operator digest | Notes |"


def now():
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def init(window, supersedes, candidate, branch, database_version, build=None):
    """A new window's record, before any image is pushed."""
    record = {
        "schema_version": SCHEMA_VERSION,
        "window_id": window,
        "supersedes_window_id": supersedes,
        "candidate_sha": candidate,
        "branch": branch,
        "tag": "pitch-%s-arm64" % candidate[:7],
        "built_at_utc": None,
        "images": {name: {"repository": repository, "local": "genui-%s:pitch-%s" % (name, candidate[:7]),
                          "local_id": None, "local_size_bytes": None, "tag": "pitch-%s-arm64" % candidate[:7],
                          "digest": None, "size_bytes": None}
                   for name, repository in IMAGES.items()},
        "parser_manifest_sha256": None,
        "stack": {"name": None, "template_sha256": None, "task_definition": None, "task_definition_revision": None,
                  "rollback_task_definition": None},
        "database_version": database_version,
        "migrations": {"snapshot": None, "migrated": False},
        "operator_tasks": [],
        "approvals": [],
        "created_at_utc": now(),
        "pushed_at_utc": None,
        "executed_at_utc": None,
        "activation_requested_at_utc": None,
        "activated_at_utc": None,
        "verified": {"at_utc": None, "origin": None, "live": None, "canvas": None, "mcp": None, "slack": None},
        "git_tag": None,
        "releases_row": None,
        "updated_at_utc": now(),
    }
    if build is not None:
        import_build(record, build)
    return record


def import_build(record, build):
    """Copy the local image record `just images` wrote (.cache/images/<sha7>/build.json)."""
    if build.get("candidate_sha") != record["candidate_sha"]:
        raise ValueError("build.json is for %s, not the candidate %s" % (build.get("candidate_sha"), record["candidate_sha"]))
    if not build.get("passed"):
        raise ValueError("build.json records a build whose checks did not pass")
    for name in IMAGES:
        image = build["images"][name]
        record["images"][name].update(local_id=image["id"], local_size_bytes=image["size_bytes"])
    record["built_at_utc"] = build["built_at_utc"]
    record["parser_manifest_sha256"] = build["parser"]["service_manifest_sha256"]
    return record


def problems(record, activated=False):
    """Every way `record` departs from the schema; activated=True also requires
    what `just deploy activate` needs before it writes the RELEASES.md row."""
    out = []

    def need(condition, message):
        if not condition:
            out.append(message)

    need(record.get("schema_version") == SCHEMA_VERSION, "schema_version must be %d" % SCHEMA_VERSION)
    need(bool(UUID4.match(str(record.get("window_id")))), "window_id must be a lowercase UUIDv4")
    need(bool(UUID4.match(str(record.get("supersedes_window_id")))), "supersedes_window_id must be a lowercase UUIDv4")
    need(bool(SHA40.match(str(record.get("candidate_sha")))), "candidate_sha must be a full commit sha")
    need(isinstance(record.get("database_version"), int), "database_version must be an integer")
    images = record.get("images") or {}
    need(sorted(images) == sorted(IMAGES), "images must be exactly service, qualification, operator")
    for name, repository in IMAGES.items():
        image = images.get(name) or {}
        need(image.get("repository") == repository, "images.%s.repository must be %s" % (name, repository))
        digest = image.get("digest")
        need(digest is None or bool(DIGEST.match(digest)), "images.%s.digest must be sha256:<64 hex>" % name)
        if activated:
            need(digest is not None, "images.%s.digest is not recorded (just deploy push)" % name)
    for field in ("created_at_utc", "pushed_at_utc", "executed_at_utc", "activation_requested_at_utc", "activated_at_utc",
                  "built_at_utc", "updated_at_utc"):
        value = record.get(field)
        need(value is None or bool(UTC.match(value)), "%s must be whole-second UTC with Z" % field)
    for approval in record.get("approvals", []):
        need(set(approval) == {"step", "at_utc", "approvals_md_line"}, "an approval ref is {step, at_utc, approvals_md_line}")
    stack = record.get("stack") or {}
    if activated:
        need(record.get("activated_at_utc") is not None, "activated_at_utc is not recorded")
        need(isinstance(stack.get("task_definition_revision"), int), "stack.task_definition_revision is not recorded")
        need(any(str(a.get("step", "")).startswith("activate") for a in record.get("approvals", [])), "no approval recorded for activate")
    return out


def check(record, activated=False):
    found = problems(record, activated)
    if found:
        raise ValueError("release.json: " + "; ".join(found))
    return record


def git_tag(record):
    """beta/<yyyymmdd>-<sha7>, the date of the activation (D13.2)."""
    when = record["activated_at_utc"]
    return "beta/%s-%s" % (when[:10].replace("-", ""), record["candidate_sha"][:7])


def row(record):
    """One non-secret RELEASES.md row: no account, ARN, host other than the
    public origin, secret or personal name; the window by its first 8 hex,
    the task definition by its revision number only."""
    check(record, activated=True)
    when = record["activated_at_utc"].replace("T", " ").replace("Z", "")
    digests = ["`%s`" % record["images"][name]["digest"] for name in IMAGES]
    return "| %s | `%s` | `%s` | `%s` | TD:%d | %d | %s | via `just deploy` |" % (
        when, record["candidate_sha"][:7], git_tag(record), record["window_id"][:8],
        record["stack"]["task_definition_revision"], record["database_version"], " | ".join(digests))


def append(record, releases):
    """Append the row under the table once; True when it was added."""
    line = row(record)
    text = releases.read_text()
    if TABLE_HEADER not in text:
        raise ValueError("%s has no release table header" % releases)
    if line in text.splitlines():
        return False
    if not text.endswith("\n"):
        text += "\n"
    releases.write_text(text + line + "\n")
    return True


def main(argv=None):
    parser = argparse.ArgumentParser(description=(__doc__ or "").split("\n\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("init")
    p.add_argument("--window", required=True)
    p.add_argument("--supersedes", required=True)
    p.add_argument("--candidate", required=True)
    p.add_argument("--branch", required=True)
    p.add_argument("--database-version", type=int, required=True)
    p.add_argument("--build", type=Path)
    p = sub.add_parser("check")
    p.add_argument("release", type=Path)
    p.add_argument("--activated", action="store_true")
    for name in ("row", "tag"):
        sub.add_parser(name).add_argument("release", type=Path)
    p = sub.add_parser("append")
    p.add_argument("release", type=Path)
    p.add_argument("releases", type=Path)
    p = sub.add_parser("import-build")
    p.add_argument("release", type=Path)
    p.add_argument("build", type=Path)
    args = parser.parse_args(argv)
    try:
        if args.command == "init":
            build = json.loads(args.build.read_text()) if args.build else None
            record = check(init(args.window, args.supersedes, args.candidate, args.branch, args.database_version, build))
            print(json.dumps(record, indent=2))
        elif args.command == "check":
            check(json.loads(args.release.read_text()), args.activated)
            print("release.json ok")
        elif args.command == "row":
            print(row(json.loads(args.release.read_text())))
        elif args.command == "tag":
            print(git_tag(check(json.loads(args.release.read_text()), activated=True)))
        elif args.command == "append":
            added = append(json.loads(args.release.read_text()), args.releases)
            print("RELEASES.md: row %s" % ("appended" if added else "already present"))
        elif args.command == "import-build":
            record = import_build(json.loads(args.release.read_text()), json.loads(args.build.read_text()))
            record["updated_at_utc"] = now()
            args.release.write_text(json.dumps(check(record), indent=2) + "\n")
            print("release.json: local image record imported")
    except (ValueError, KeyError) as error:
        print("error: %s" % error, file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
