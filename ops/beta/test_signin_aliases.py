"""Per-window clean-trial aliases: real wrapper, temporary Git, closed CDK shims.

No live ledger, AWS, provider or developer environment is used. The AWS shim
fails every call; synth/diff only record their exact arguments.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
WINDOW = "11111111-2222-4333-8444-555555555555"
PREVIOUS = "22222222-2222-4333-8444-555555555555"
NEXT = "33333333-2222-4333-8444-555555555555"
TRIAL = {"organization_id": "00000000-0000-4000-8000-0000000000e1",
         "tenant_slug": "trial-test", "provider_tenant_id": "ten_trial_test"}
ALIASES = {"aliases": [{"name": "forma-trial", "handle": "se45m37vabqrfvbh3gbwn22qkm"}]}
CDK = '''#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
with open(os.environ['FAKE_CDK_LOG'],'a') as f:
    f.write(json.dumps({'tool':Path(sys.argv[0]).name,'args':sys.argv[1:]})+'\\n')
if Path(sys.argv[0]).name=='npm':
    Path('cdk.out').mkdir(exist_ok=True)
    Path('cdk.out/GenuiSyntheticFoundation.template.json').write_text('{"Resources":{}}')
'''


class SigninAliasWindows(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="forma aliases ")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.repo, self.private, self.shims = [self.root / x for x in ("repo", "private", "shims")]
        scripts = self.repo / "ops/beta"
        scripts.mkdir(parents=True)
        for name in ("deploy.sh", "lib.sh", "release.py", "signin_aliases.py"):
            shutil.copy2(HERE / name, scripts / name)
        infra = self.repo / "infra/scripts"
        infra.mkdir(parents=True)
        for name in ("window_requests.py", "common.py"):
            shutil.copy2(HERE.parents[1] / "infra/scripts" / name, infra / name)
        migrations = self.repo / "app/migrations"
        migrations.mkdir(parents=True)
        (migrations / "manifest.json").write_text('{"database_version":19}')
        self.private.mkdir(mode=0o700)
        (self.private / "env.sh").write_text("AWS_REGION=synthetic\n")
        (self.private / "signin-aliases.json").write_text(json.dumps({"aliases": [
            {"name": "northstar", "handle": ALIASES["aliases"][0]["handle"]}]}))
        previous = self.private / "windows" / PREVIOUS
        previous.mkdir(parents=True)
        (previous / "deployment-input.json").write_text(json.dumps({"clean_trial_organization_id": TRIAL["organization_id"]}))
        self.trial, self.aliases = self.root / "trial.json", self.root / "reviewed aliases.json"
        self.trial.write_text(json.dumps(TRIAL))
        self.aliases.write_text(json.dumps(ALIASES))
        self.shims.mkdir()
        for name in ("npm", "npx"):
            (self.shims / name).write_text(CDK)
            (self.shims / name).chmod(0o755)
        (self.shims / "aws").write_text('#!/bin/sh\nprintf forbidden > "$FAKE_AWS_LOG"\nexit 97\n')
        (self.shims / "aws").chmod(0o755)
        (self.shims / "uuidgen").write_text('#!/bin/sh\nprintf "%s\\n" "$FAKE_UUID"\n')
        (self.shims / "uuidgen").chmod(0o755)
        self.env = {k:v for k,v in os.environ.items() if not k.startswith(("AWS_", "GENUI_", "GIT_", "FAKE_"))}
        self.env.update(GENUI_OPS_PRIV=str(self.private), FAKE_CDK_LOG=str(self.root / "cdk.jsonl"),
                        FAKE_AWS_LOG=str(self.root / "aws.log"), FAKE_UUID=WINDOW,
                        GIT_AUTHOR_NAME="Test", GIT_AUTHOR_EMAIL="test@example.invalid",
                        GIT_COMMITTER_NAME="Test", GIT_COMMITTER_EMAIL="test@example.invalid",
                        GIT_CONFIG_GLOBAL="/dev/null", PATH=str(self.shims)+os.pathsep+os.environ["PATH"])
        self.run_cmd("git", "init", "-q", "-b", "main")
        self.run_cmd("git", "add", ".")
        self.run_cmd("git", "commit", "-qm", "synthetic alias fixture")
        self.sha = self.run_cmd("git", "rev-parse", "HEAD").stdout.strip()

    def run_cmd(self, *args, success=True):
        result = subprocess.run(args, cwd=self.repo, env=self.env, text=True, capture_output=True)
        if success:
            self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        else:
            self.assertNotEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertFalse((self.root / "aws.log").exists(), "no AWS calls are allowed")
        return result

    def new(self, *, aliases=True, predecessor=PREVIOUS, trial=True, extra=(), success=True):
        args = ["bash", str(self.repo / "ops/beta/deploy.sh"), "new", "--candidate", self.sha,
                "--supersedes", predecessor]
        if trial:
            args += ["--clean-trial-file", str(self.trial)]
        if aliases:
            args += ["--signin-aliases-file", str(self.aliases)]
        return self.run_cmd(*args, *extra, success=success)

    def test_new_copies_one_mapping_and_synth_diff_use_identical_window_context(self):
        self.new()
        w = self.private / "windows" / WINDOW
        copied = w / "signin-aliases.json"
        self.assertEqual(json.loads(copied.read_text()), ALIASES)
        self.assertEqual(copied.stat().st_mode & 0o777, 0o600)
        self.aliases.unlink()  # No dependency on the original private input.
        (w / "template-before.json").write_text('{"Resources":{}}')
        for step in ("synth", "diff"):
            self.run_cmd("bash", str(self.repo / "ops/beta/deploy.sh"), step, WINDOW)
        calls = [json.loads(x) for x in (self.root / "cdk.jsonl").read_text().splitlines()]
        contexts = [[a for a in c["args"] if a.startswith(("cleanTrialFile=", "signinAliasesFile="))] for c in calls]
        self.assertEqual(contexts, [[f"cleanTrialFile={w}/clean-trial.json", f"signinAliasesFile={copied}"]] * 2)
        self.assertNotIn("displayNamesFile", json.dumps(calls))
        self.assertNotIn(str(self.private / "signin-aliases.json"), json.dumps(calls))

    def test_successor_requires_explicit_unchanged_mapping_and_preserves_it(self):
        self.new()
        w = self.private / "windows" / WINDOW
        (w / "deployment-input.json").write_text((w / "base-input.json").read_text())
        self.env["FAKE_UUID"] = NEXT
        self.assertIn("predecessor has aliases", self.new(aliases=False, predecessor=WINDOW, success=False).stderr)
        self.assertFalse((self.private / "windows" / NEXT).exists())
        self.aliases.write_text(json.dumps({"aliases":[dict(ALIASES["aliases"][0], name="changed")]}))
        self.assertIn("preserve", self.new(predecessor=WINDOW, success=False).stderr)
        self.aliases.write_text(json.dumps(ALIASES))
        self.new(predecessor=WINDOW)
        self.assertEqual(json.loads((self.private / "windows" / NEXT / "signin-aliases.json").read_text()), ALIASES)

    def test_aliases_require_clean_trial_and_refuse_shared_legacy_source(self):
        self.assertIn("requires --clean-trial-file", self.new(trial=False, success=False).stderr)
        self.aliases = self.private / "signin-aliases.json"
        self.assertIn("shared legacy", self.new(success=False).stderr)
        self.assertFalse((self.private / "windows" / WINDOW).exists())

    def test_closed_one_mapping_bound_and_shape_refusals_create_no_window(self):
        for value in ({"aliases":[]}, {"aliases":ALIASES["aliases"]*2}, dict(ALIASES, extra=True),
                      {"aliases":[dict(ALIASES["aliases"][0], handle="invalid")]},
                      {"aliases":[dict(ALIASES["aliases"][0], handle="se45m37vabqrfvbh3gbwn22qkn")]},
                      {"aliases":[dict(ALIASES["aliases"][0], name="a"*26)]}):
            with self.subTest(value=value):
                self.aliases.write_text(json.dumps(value))
                self.new(success=False)
                self.assertFalse((self.private / "windows" / WINDOW).exists())

    def test_duplicate_keys_are_refused_but_names_have_no_tenant_special_cases(self):
        self.aliases.write_text('{"aliases":[],"aliases":' + json.dumps(ALIASES["aliases"]) + '}')
        self.assertIn("duplicate JSON keys", self.new(success=False).stderr)
        self.assertFalse((self.private / "windows" / WINDOW).exists())
        self.aliases.write_text(json.dumps({"aliases":[dict(ALIASES["aliases"][0], name="canary")]}))
        self.new()
        self.assertEqual(json.loads((self.private / "windows" / WINDOW / "signin-aliases.json").read_text())["aliases"][0]["name"], "canary")

    def test_clean_trial_without_alias_and_dry_run_create_no_alias_or_remote_work(self):
        self.new(extra=("--dry-run",))
        self.assertFalse((self.private / "windows" / WINDOW).exists())
        self.new(aliases=False)
        self.assertFalse((self.private / "windows" / WINDOW / "signin-aliases.json").exists())


if __name__ == "__main__":
    unittest.main()
