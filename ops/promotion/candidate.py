"""Closed GitHub candidate gate. Read-only API calls; never starts/retries a run."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time

REPOSITORY = "getforma-dev/forma-app-platform"
REPOSITORY_ID = 1437433309
OWNER_ID = 268000170
BRANCH = "pivot/pitch-mvp"
WORKFLOW = ".github/workflows/beta-promote.yml"
REQUIRED = frozenset({"auth-contract.yml", "baseline.yml", "canvas.yml", "contracts.yml",
    "fixtures.yml", "foundation.yml", "parser-release.yml", "parser.yml", "pitch.yml",
    "planning.yml", "query.yml", "tenant-foundation.yml"})
SHA = re.compile(r"[0-9a-f]{40}\Z")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def git(*args):
    return subprocess.check_output(["git", *args], text=True).strip()


def api(path):
    # gh owns token handling; never interpolate a token or API body in logs.
    return json.loads(subprocess.check_output(["gh", "api", path], text=True))


def context(event, sha):
    require(SHA.fullmatch(sha), "full candidate SHA required")
    repo = event.get("repository", {})
    require(repo.get("full_name") == REPOSITORY and repo.get("id") == REPOSITORY_ID
        and repo.get("owner", {}).get("id") == OWNER_ID, "foreign repository")
    require(event.get("ref") == "refs/heads/" + BRANCH and event.get("after") == sha
        and event.get("deleted") is False and event.get("forced") is False,
        "candidate must be an ordinary trusted-branch push")
    require(os.environ.get("GITHUB_EVENT_NAME", "push") == "push", "push event required")
    return sha


def runtime_path(path):
    if path.endswith(".md"):
        return False
    return path in {".dockerignore", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
        ".node-version", ".python-version", WORKFLOW} or path.startswith((
        "app/", "parser/", "contracts/", "spikes/APP-006/", "infra/", "ops/", "scripts/"))


def merged_pr(prs, sha):
    matches = [p for p in prs if p.get("merged_at") and p.get("merge_commit_sha") == sha
        and p.get("base", {}).get("ref") == BRANCH
        and p.get("base", {}).get("repo", {}).get("id") == REPOSITORY_ID]
    require(len(matches) == 1, "candidate is not one exact merged pull request on the active trunk")
    return matches[0]["number"]


def ci_status(runs, sha):
    """Latest push run for EACH required source workflow must succeed at this SHA.

    A PR's synthetic merge SHA, workflow display name, or older successful attempt
    cannot satisfy this gate. The promotion workflow is deliberately not required.
    """
    selected = {}
    for run in runs:
        path = run.get("path", "")
        name = path.removeprefix(".github/workflows/")
        if name not in REQUIRED or path != ".github/workflows/" + name:
            continue
        if run.get("head_sha") != sha or run.get("event") != "push":
            continue
        require(run.get("repository", {}).get("id") == REPOSITORY_ID
            and run.get("head_branch") == BRANCH, "CI repository/ref mismatch")
        if name not in selected or (run["run_number"], run["run_attempt"]) > (
                selected[name]["run_number"], selected[name]["run_attempt"]):
            selected[name] = run
    for name, run in selected.items():
        if run.get("status") == "completed":
            require(run.get("conclusion") == "success", "required CI failed: " + name)
    ready = set(selected) == REQUIRED and all(r.get("status") == "completed"
        and r.get("conclusion") == "success" for r in selected.values())
    return ready, [{"path": ".github/workflows/" + name, "id": r["id"],
        "attempt": r["run_attempt"], "status": r["status"], "conclusion": r["conclusion"],
        "head_sha": r["head_sha"], "event": r["event"]}
        for name, r in sorted(selected.items())]


def current_ci(sha):
    require(SHA.fullmatch(sha), "full candidate required")
    require(api("repos/" + REPOSITORY + "/git/ref/heads/" + BRANCH)["object"]["sha"] == sha,
        "candidate superseded on trunk; no promotion")
    runs = []
    for page in range(1, 11):
        batch = api(f"repos/{REPOSITORY}/actions/runs?head_sha={sha}&per_page=100&page={page}")["workflow_runs"]
        runs.extend(batch)
        if len(batch) < 100:
            break
    else:
        raise ValueError("CI result limit exceeded")
    ready, checks = ci_status(runs, sha)
    # Refuse a merge racing the CI read too; the AWS state lease separately
    # prevents two deployments from mutating the existing beta concurrently.
    require(api("repos/" + REPOSITORY + "/git/ref/heads/" + BRANCH)["object"]["sha"] == sha,
        "candidate superseded while checking CI")
    return ready, checks


def check_current(candidate_sha):
    ready, checks = current_ci(candidate_sha)
    require(ready, "current exact push CI is not completely successful")
    return {"candidate_sha": candidate_sha, "ready": True, "checks": checks}


def wait_ci(sha, output, seconds=3600):
    end = time.monotonic() + seconds
    while True:
        ready, checks = current_ci(sha)
        output.write_text(json.dumps({"candidate_sha": sha, "ready": ready, "checks": checks}, indent=2) + "\n")
        if ready:
            return
        require(time.monotonic() < end, "timed out waiting for exact push CI")
        print(f"Waiting for exact push CI: {sum(c['conclusion'] == 'success' for c in checks)}/{len(REQUIRED)}", flush=True)
        time.sleep(30)


def binding(paths):
    rows = [(p, hashlib.sha256(Path(p).read_bytes()).hexdigest()) for p in sorted(paths)]
    return hashlib.sha256(b"".join(p.encode() + b"\0" + h.encode() + b"\n" for p, h in rows)).hexdigest()


def source_binding():
    paths = git("ls-files").splitlines()
    manifests = [p for p in paths if p.startswith("app/migrations/") and p.endswith("manifest.json")]
    require(len(manifests) == 1, "one schema manifest required")
    return {"tree": git("rev-parse", "HEAD^{tree}"), "runtime_sha256": binding(p for p in paths if runtime_path(p)),
        "deployment_contract_sha256": deployment_contract_digest(Path.cwd()),
        "schema_manifest_sha256": hashlib.sha256(Path(manifests[0]).read_bytes()).hexdigest()}


def deployment_contract_digest(repo, revision=None):
    revision = revision or "HEAD"
    require(revision == "HEAD" or SHA.fullmatch(revision), "full deployment baseline SHA required")
    def read(*args):
        return subprocess.check_output(["git", "-C", str(repo), *args])
    paths = read("ls-tree", "-r", "--name-only", revision).decode().splitlines()
    contract = {p for p in paths if p.startswith(("infra/src/", "app/migrations/")) or p in {
        "infra/package.json", "infra/package-lock.json", "infra/cdk.json", "infra/tsconfig.json",
        "app/runtime/runtime.py", "app/Dockerfile", "app/qualification/Dockerfile",
        "app/operator/Dockerfile", ".dockerignore", "app/qualification/Dockerfile.dockerignore",
        "app/operator/Dockerfile.dockerignore"}}
    require(contract, "deployment contract is empty")
    rows = [(p, hashlib.sha256(read("show", revision + ":" + p)).hexdigest()) for p in sorted(contract)]
    return hashlib.sha256(b"".join(p.encode() + b"\0" + h.encode() + b"\n" for p, h in rows)).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("select", "wait-ci", "source"))
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    require(SHA.fullmatch(args.candidate), "full SHA required")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    if args.command == "select":
        event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
        context(event, args.candidate)
        require(git("rev-parse", "HEAD") == args.candidate, "checkout differs from candidate")
        pr = merged_pr(api(f"repos/{REPOSITORY}/commits/{args.candidate}/pulls"), args.candidate)
        changed = git("diff-tree", "--no-commit-id", "--name-only", "-r", args.candidate + "^1", args.candidate).splitlines()
        selected = any(runtime_path(p) for p in changed)
        args.output.write_text(json.dumps({"candidate_sha": args.candidate, "merged_pr": pr,
            "runtime_changed": selected, "source": source_binding()}, indent=2) + "\n")
        with open(os.environ["GITHUB_OUTPUT"], "a") as out:
            out.write("runtime_changed=" + str(selected).lower() + "\n")
    elif args.command == "wait-ci":
        wait_ci(args.candidate, args.output)
    else:
        args.output.write_text(json.dumps(source_binding(), indent=2) + "\n")


if __name__ == "__main__":
    main()
