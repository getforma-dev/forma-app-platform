"""Tests of the release record (ADR-013 D13.5) and the deploy steps that write
it, with no AWS call of any kind.

`DeployFlowTests` runs ops/beta/deploy.sh in a synthetic repository (a copy of
ops/beta plus fake infra/scripts, its own bare `origin`), with a private
directory of dummy identifiers and shims first on PATH: `aws` and `docker`
log their arguments and answer canned JSON, `curl` answers the four public
checks. A real `aws`, `docker push` or `git push` to the real origin cannot be
reached from it.

  python3 -m unittest discover -s ops/beta -p 'test_*.py'
"""
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import release  # noqa: E402

SHA = "0123456789abcdef0123456789abcdef01234567"
WINDOW = "11111111-2222-4333-8444-555555555555"
LIVE = "66666666-7777-4888-9999-aaaaaaaaaaaa"
DIGESTS = {n: "sha256:" + c * 64 for n, c in (("service", "a"), ("qualification", "b"), ("operator", "c"))}


def build_json(candidate=SHA, passed=True):
    return {"schema_version": 1, "candidate_sha": candidate, "built_at_utc": "2026-10-31T12:00:00Z",
            "images": {n: {"reference": "genui-%s:pitch-%s" % (n, candidate[:7]), "id": "sha256:local-" + n,
                           "size_bytes": 1000, "architecture": "arm64", "created_at_utc": "2026-10-31T12:00:00Z"}
                       for n in release.IMAGES},
            "parser": {"service_manifest_sha256": "d" * 64, "operator_manifest_sha256": "e" * 64, "equal": True,
                       "identity": {}},
            "checks_passed": passed, "passed": passed}


def activated():
    record = release.init(WINDOW, LIVE, SHA, "origin/pivot/pitch-mvp", 15, build_json())
    for name in release.IMAGES:
        record["images"][name]["digest"] = DIGESTS[name]
    record["stack"]["task_definition_revision"] = 13
    record["activated_at_utc"] = "2026-11-01T09:30:00Z"
    record["approvals"].append({"step": "activate", "at_utc": "2026-11-01T09:20:00Z", "approvals_md_line": 7})
    return record


class ReleaseRecordTests(unittest.TestCase):
    def test_init_carries_the_build_and_passes_the_schema(self):
        record = release.check(release.init(WINDOW, LIVE, SHA, "origin/pivot/pitch-mvp", 15, build_json()))
        self.assertEqual(record["images"]["service"]["local_id"], "sha256:local-service")
        self.assertEqual(record["images"]["operator"]["repository"], "genui-operator")
        self.assertEqual(record["parser_manifest_sha256"], "d" * 64)
        self.assertEqual(record["tag"], "pitch-0123456-arm64")
        for field in ("candidate_sha", "branch", "built_at_utc", "images", "parser_manifest_sha256", "window_id",
                      "supersedes_window_id", "database_version", "activated_at_utc", "verified", "stack",
                      "migrations", "approvals"):
            self.assertIn(field, record)

    def test_a_failed_or_foreign_build_is_refused(self):
        with self.assertRaises(ValueError):
            release.init(WINDOW, LIVE, SHA, "b", 15, build_json(passed=False))
        with self.assertRaises(ValueError):
            release.init(WINDOW, LIVE, SHA, "b", 15, build_json(candidate="f" * 40))

    def test_schema_problems_are_named(self):
        record = release.init(WINDOW, LIVE, SHA, "b", 15)
        record["window_id"] = "not-a-uuid"
        record["images"]["service"]["digest"] = "sha256:short"
        found = release.problems(record)
        self.assertTrue(any("window_id" in p for p in found))
        self.assertTrue(any("images.service.digest" in p for p in found))
        self.assertTrue(release.problems(release.init(WINDOW, LIVE, SHA, "b", 15), activated=True))

    def test_row_and_tag_are_non_secret_and_stable(self):
        record = activated()
        self.assertEqual(release.git_tag(record), "beta/20261101-0123456")
        line = release.row(record)
        self.assertIn("| `0123456` | `beta/20261101-0123456` | `11111111` | TD:13 | 15 |", line)
        self.assertNotIn(WINDOW, line)
        for digest in DIGESTS.values():
            self.assertIn(digest, line)

    def test_append_is_idempotent_and_needs_the_table(self):
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "RELEASES.md"
            path.write_text("# x\n\n" + release.TABLE_HEADER + "\n| --- |\n")
            self.assertTrue(release.append(activated(), path))
            self.assertFalse(release.append(activated(), path))
            self.assertEqual(path.read_text().count("TD:13"), 1)
            path.write_text("# no table\n")
            with self.assertRaises(ValueError):
                release.append(activated(), path)

    def test_the_committed_releases_file_has_the_table(self):
        text = (HERE.parents[1] / "docs/pitch/RELEASES.md").read_text()
        self.assertIn(release.TABLE_HEADER, text)
        self.assertTrue(text.rstrip("\n").endswith("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"),
                        "rows are appended after the release table, which must end the file")


AWS = r'''#!/bin/bash
echo "aws $*" >> "$SHIM_LOG"
case "$1 $2" in
  "ecr get-login-password") echo dummy ;;
  "ecr describe-images")
    repo=$4; case $repo in genui-synthetic) c=a ;; genui-qualification) c=b ;; *) c=c ;; esac
    d=$(printf "$c%.0s" $(seq 64))
    printf '{"imageDetails":[{"imageDigest":"sha256:%s","imageSizeInBytes":123}]}\n' "$d" ;;
  "cloudformation describe-stacks")
    echo '{"Stacks":[{"Outputs":[{"OutputKey":"TaskDefinition","OutputValue":"arn:aws:ecs:r:0:task-definition/Task:13"},{"OutputKey":"RollbackTaskDefinition","OutputValue":"arn:aws:ecs:r:0:task-definition/Rollback:4"}]}]}' ;;
  "ecs describe-services") echo '{}' ;;
  *) : ;;
esac
'''
DOCKER = r'''#!/bin/bash
echo "docker $*" >> "$SHIM_LOG"
case "$1" in
  login) cat >/dev/null; exit 0 ;;
  tag|push) exit 0 ;;
  image)
    ref=${@: -1}; n=${ref#genui-}; n=${n%%:*}
    [[ -n "$SHIM_BAD_ID" && $n == service ]] && { echo "sha256:other"; exit 0; }
    echo "sha256:local-$n" ;;
esac
'''
CURL = r'''#!/bin/bash
out=/dev/null url=""
while (( $# )); do case $1 in -o) out=$2; shift 2 ;; -w|-X|-H|--data|--max-time) shift 2 ;; -*) shift ;; *) url=$1; shift ;; esac; done
case $url in
  */health/live) printf '200 ' ;;
  */canvas) echo "Sign in" > "$out"; printf '200 ' ;;
  */v1/mcp) printf '404 ' ;;
  */v1/slack/commands) printf '401 ' ;;
esac
'''
PREFLIGHT = r'''import json, sys
if "--activate" in sys.argv:
    print(json.dumps({"status": "activation_requested_not_healthy", "observed_at_utc": "2026-11-01T09:25:00Z",
                      "task_definition": "arn:aws:ecs:r:0:task-definition/Task:13"}))
else:
    print(json.dumps({"status": "reserved_local_plan_only"}))
'''


class DeployFlowTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        repo, origin, priv, shims = (self.tmp / n for n in ("repo", "origin.git", "priv", "shims"))
        self.repo, self.origin, self.priv = repo, origin, priv
        for name in ("lib.sh", "deploy.sh", "run-step.sh", "verify.sh", "release.py"):
            (repo / "ops/beta").mkdir(parents=True, exist_ok=True)
            shutil.copy2(HERE / name, repo / "ops/beta" / name)
        (repo / "infra/scripts").mkdir(parents=True)
        (repo / "infra/scripts/preflight.py").write_text(PREFLIGHT)
        (repo / "app/migrations").mkdir(parents=True)
        (repo / "app/migrations/manifest.json").write_text('{"schema_version":1,"database_version":15}\n')
        (repo / "docs/pitch").mkdir(parents=True)
        shutil.copy2(HERE.parents[1] / "docs/pitch/RELEASES.md", repo / "docs/pitch/RELEASES.md")
        self.env = dict(os.environ, GENUI_OPS_PRIV=str(priv), SHIM_LOG=str(self.tmp / "shim.log"),
                        PATH="%s:%s" % (shims, os.environ["PATH"]), GENUI_HEALTH_WAIT_SECONDS="20",
                        GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@example.invalid", GIT_COMMITTER_NAME="t",
                        GIT_COMMITTER_EMAIL="t@example.invalid", GIT_CONFIG_GLOBAL="/dev/null")
        self.env.pop("SHIM_BAD_ID", None)
        git = lambda *a, cwd=repo: subprocess.run(["git", *a], cwd=cwd, env=self.env, check=True, capture_output=True)
        git("init", "-q", "-b", "main")
        git("add", "-A")
        git("commit", "-q", "-m", "synthetic")
        git("init", "-q", "--bare", str(origin), cwd=self.tmp)
        git("remote", "add", "origin", str(origin))
        git("push", "-q", "-u", "origin", "main")
        self.sha = subprocess.run(["git", "rev-parse", "HEAD"], cwd=repo, capture_output=True, text=True).stdout.strip()
        shims.mkdir()
        for name, body in (("aws", AWS), ("docker", DOCKER), ("curl", CURL)):
            (shims / name).write_text(body)
            (shims / name).chmod(0o755)
        priv.mkdir(mode=0o700)
        (priv / "env.sh").write_text("export AWS_REGION=r\nSTACK=s\nREG=reg.invalid\nCLUSTER=c\nSERVICE=v\n")
        build = repo / ".cache/images" / self.sha[:7]
        build.mkdir(parents=True)
        (build / "build.json").write_text(json.dumps(build_json(self.sha)))
        (self.tmp / "input.json").write_text("{}")

    def deploy(self, *args, expect=0):
        result = subprocess.run(["bash", str(self.repo / "ops/beta/deploy.sh"), *args], cwd=self.repo, env=self.env,
                                stdin=subprocess.DEVNULL, capture_output=True, text=True)
        self.assertEqual(result.returncode, expect, result.stdout + result.stderr)
        return result.stdout + result.stderr

    def shim_log(self):
        path = self.tmp / "shim.log"
        return path.read_text() if path.exists() else ""

    def record(self):
        return json.loads((self.priv / "windows" / self.window / "release.json").read_text())

    def new_window(self):
        out = self.deploy("new", "--candidate", self.sha, "--supersedes", LIVE, "--input-from", str(self.tmp / "input.json"))
        self.window = next(l.split()[1] for l in out.splitlines() if l.startswith("window "))
        return out

    def test_the_release_record_through_push_execute_activate_and_verify(self):
        self.new_window()
        record = self.record()
        self.assertEqual((record["candidate_sha"], record["database_version"], record["branch"]), (self.sha, 15, "origin/main"))
        self.assertEqual(record["images"]["qualification"]["local_id"], "sha256:local-qualification")
        self.assertEqual(stat.S_IMODE((self.priv / "windows" / self.window / "release.json").stat().st_mode), 0o600)

        out = self.deploy("push", self.window, expect=3)
        self.assertIn("refused", out)
        log = self.shim_log()
        self.assertNotIn("aws ", log, "a refused step calls no AWS API")
        self.assertFalse([l for l in log.splitlines() if not l.startswith("docker image inspect")],
                         "a refused step only inspects local images")

        self.deploy("push", self.window, "--approved", "yes, push")
        record = self.record()
        self.assertEqual({n: record["images"][n]["digest"] for n in release.IMAGES}, DIGESTS)
        self.assertEqual(record["images"]["service"]["size_bytes"], 123)
        self.assertEqual(record["approvals"][0]["step"], "push")
        approvals = (self.priv / "approvals.md").read_text().splitlines()
        self.assertIn("yes, push", approvals[record["approvals"][0]["approvals_md_line"] - 1])

        (self.priv / "windows" / self.window / "change-set-review.json").write_text("{}")
        self.deploy("execute", self.window, "--approved", "execute it")
        record = self.record()
        self.assertEqual((record["stack"]["name"], record["stack"]["task_definition_revision"]), ("s", 13))

        (self.priv / "windows" / self.window / "deployment-input.json").write_text('{"service_desired_count":0}')
        out = self.deploy("activate", self.window, "--approved", "activate it")
        record = self.record()
        tag = "beta/%s-%s" % (record["activated_at_utc"][:10].replace("-", ""), self.sha[:7])
        self.assertEqual(record["git_tag"], {"name": tag, "pushed": True})
        self.assertIn(tag, subprocess.run(["git", "tag"], cwd=self.origin, capture_output=True, text=True).stdout)
        releases = (self.repo / "docs/pitch/RELEASES.md").read_text()
        self.assertEqual(releases.count("`%s`" % self.window[:8]), 1)
        self.assertTrue(releases.rstrip().endswith("via `just deploy` |"))
        self.assertNotIn(self.window, releases)
        self.assertNotIn("arn:", releases)

        self.deploy("activate", self.window, "--record-only", "--approved", "record it")
        self.assertEqual((self.repo / "docs/pitch/RELEASES.md").read_text().count("`%s`" % self.window[:8]), 1)

        out = self.deploy("verify")
        self.assertIn("release: %s %s" % (self.sha[:7], tag), out)
        verified = self.record()["verified"]
        self.assertEqual({k: verified[k] for k in ("live", "canvas", "mcp", "slack")},
                         {"live": True, "canvas": True, "mcp": True, "slack": True})
        self.assertNotIn("git push", self.shim_log())

    def test_push_refuses_images_that_were_not_qualified(self):
        self.new_window()
        self.env["SHIM_BAD_ID"] = "1"
        out = self.deploy("push", self.window, "--approved", "yes", expect=1)
        self.assertIn("not the image 'just images' qualified", out)
        self.assertNotIn("aws ecr", self.shim_log())

    def test_activate_without_approval_records_nothing(self):
        self.new_window()
        (self.priv / "windows" / self.window / "deployment-input.json").write_text("{}")
        self.deploy("activate", self.window, expect=3)
        self.assertIsNone(self.record()["activated_at_utc"])
        self.assertFalse((self.priv / "approvals.md").exists())
        self.assertEqual(subprocess.run(["git", "tag"], cwd=self.origin, capture_output=True, text=True).stdout, "")


if __name__ == "__main__":
    unittest.main()
