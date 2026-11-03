"""ADR-013 D13.6: real local Git worktrees share state, not environments.

Uses temporary repositories and dummy env files only. No AWS, Docker, secrets,
or existing developer instances are accessed.
"""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

HERE = Path(__file__).resolve().parent


class PrivateStateTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix="genui ops ")
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name).resolve()
        self.main = self.root / "main checkout"
        self.linked = self.root / "linked checkout"
        self.main.mkdir()
        self.env = dict(os.environ, GIT_CONFIG_GLOBAL="/dev/null",
                        GIT_AUTHOR_NAME="Test", GIT_AUTHOR_EMAIL="test@example.invalid",
                        GIT_COMMITTER_NAME="Test", GIT_COMMITTER_EMAIL="test@example.invalid")
        for name in ("GENUI_OPS_PRIV", "GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR"):
            self.env.pop(name, None)
        self.git("init", "-q", "-b", "main")
        scripts = self.main / "ops/beta"
        scripts.mkdir(parents=True)
        for name in ("lib.sh", "deploy.sh"):
            shutil.copy2(HERE / name, scripts / name)
        self.git("add", ".")
        self.git("commit", "-q", "-m", "synthetic ops scripts")
        self.git("worktree", "add", "--detach", str(self.linked))

    def git(self, *args):
        return subprocess.run(["git", *args], cwd=self.main, env=self.env,
                              check=True, capture_output=True, text=True)

    def paths(self, checkout, *, override=None, load=False):
        env = dict(self.env)
        if override:
            env["GENUI_OPS_PRIV"] = str(override)
        script = 'source "$1/ops/beta/lib.sh"; '
        if load:
            script += 'load_env; '
            # run-step.sh is a child process that sources lib.sh afresh.
            # Prove stale config cannot change its exported override.
            script += 'bash -eu -c \'source "$1/ops/beta/lib.sh"; [[ "$PRIV" == "$2" ]]\' ops-child "$REPO" "$PRIV"; '
        script += 'printf "%s\\n" "$REPO" "$PRIV" "$OPVENV" "$PY"'
        result = subprocess.run(["bash", "-eu", "-c", script, "ops-test", str(checkout)],
                                cwd=self.root, env=env, check=True, capture_output=True, text=True)
        return result.stdout.splitlines()

    def test_worktrees_share_ledger_but_keep_checkout_local_environments(self):
        for checkout in (self.main, self.linked):
            self.assertEqual(self.paths(checkout), [str(checkout), str(self.main / ".cache/ops/beta"),
                             str(checkout / "app/operator/.venv"), str(checkout / "app/runtime/.venv/bin/python")])
        self.assertFalse((self.main / ".cache").exists(), "path discovery must create nothing")

    def test_load_env_preserves_shared_path_and_explicit_override(self):
        for private in (self.main / ".cache/ops/beta", self.root / "custom private"):
            private.mkdir(parents=True, mode=0o700)
            (private / "env.sh").write_text('AWS_REGION=dummy\nREPO=/wrong\nPRIV=/wrong\nOPVENV=/wrong\nPY=/wrong\nGENUI_OPS_PRIV=/wrong\n')
            override = private if private.name == "custom private" else None
            paths = self.paths(self.linked, override=override, load=True)
            self.assertEqual(paths, [str(self.linked), str(private), str(self.linked / "app/operator/.venv"),
                                    str(self.linked / "app/runtime/.venv/bin/python")])

    def test_deploy_help_prints_the_resolved_directory_without_creating_state(self):
        result = subprocess.run(["bash", str(self.linked / "ops/beta/deploy.sh"), "help"],
                                cwd=self.root, env=self.env, check=True, capture_output=True, text=True)
        self.assertIn("Operator state: " + str(self.main / ".cache/ops/beta"), result.stderr)
        self.assertIn("--approved", result.stdout)
        self.assertFalse((self.main / ".cache").exists())


if __name__ == "__main__":
    unittest.main()
