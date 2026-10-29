"""Reset targeting/failure guards; actual PostgreSQL evidence is a separate trial."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import reset_trial


class ResetGuards(unittest.TestCase):
    def test_ambient_database_and_docker_targets_are_refused_before_docker(self):
        for name in ("PGHOST", "PGHOSTADDR", "PGSERVICE", "PGPASSWORD", "DOCKER_HOST", "DOCKER_CONTEXT"):
            with self.subTest(name=name), patch.dict(os.environ, {name: "synthetic-disallowed"}, clear=True), patch.object(reset_trial, "docker") as docker:
                with self.assertRaises(reset_trial.TrialError):
                    reset_trial.local_environment()
                docker.assert_not_called()

    def test_remote_docker_context_is_refused(self):
        result = subprocess.CompletedProcess([], 0, stdout=json.dumps([{"Endpoints": {"docker": {"Host": "ssh://synthetic-remote"}}}]))
        with patch.dict(os.environ, {}, clear=True), patch.object(reset_trial, "docker", return_value=result):
            with self.assertRaisesRegex(reset_trial.TrialError, "local Unix-socket"):
                reset_trial.local_environment()

    def test_no_target_selector_is_exposed_by_cli(self):
        for flag in ("--database-url", "--profile", "--container", "--image"):
            result = subprocess.run([sys.executable, str(Path(reset_trial.__file__)), "--output", "unused", flag, "synthetic"], capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, 2)
            self.assertIn("unrecognized arguments", result.stderr)

    def test_existing_evidence_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / "preserve.txt"
            marker.write_text("earlier failed evidence")
            with patch.object(reset_trial, "local_environment") as guard:
                with self.assertRaises(FileExistsError):
                    reset_trial.run_trial(root)
                guard.assert_not_called()
            self.assertEqual(marker.read_text(), "earlier failed evidence")

    def test_early_failure_retains_receipt_without_loading_database_code(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "new"
            with patch.object(reset_trial, "local_environment", side_effect=reset_trial.TrialError("synthetic guard failure")), patch.object(reset_trial, "dependencies") as deps:
                with self.assertRaisesRegex(reset_trial.TrialError, "synthetic guard failure"):
                    reset_trial.run_trial(output)
                deps.assert_not_called()
            report = json.loads((output / "summary.json").read_text())
            self.assertFalse(report["passed"])
            self.assertEqual(report["workspaces"], [])
            self.assertEqual(report["failure"]["type"], "TrialError")


if __name__ == "__main__":
    unittest.main()
