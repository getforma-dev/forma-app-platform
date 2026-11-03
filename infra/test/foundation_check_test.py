"""Bounded Foundation runner: preserve failures and stop dependent checks."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("foundation_check", ROOT / "scripts/check-foundation.py")
foundation = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(foundation)


class FoundationCheckTests(unittest.TestCase):
    def test_timeout_preserves_byte_diagnostics_and_stops(self):
        command = ['python', '-m', 'unittest', 'discover', '-s', 'app/runtime', '-v']
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            with mock.patch.object(foundation.subprocess, 'run', side_effect=subprocess.TimeoutExpired(
                    command, 600, output=b'test_database_boundary ... pending\xff')) as run, \
                    contextlib.redirect_stdout(io.StringIO()) as console:
                code = foundation.run_checks([('.', command), ('.', ['never-run'])], ROOT, output)
            self.assertEqual(code, 124)
            self.assertEqual(run.call_count, 1)
            self.assertEqual(run.call_args.kwargs['timeout'], 600)
            report = json.loads((output / 'local-preparation.json').read_text())['steps']
            self.assertEqual(len(report), 1)
            self.assertTrue(report[0]['timed_out'])
            self.assertEqual(report[0]['exit'], 124)
            self.assertIn('test_database_boundary', report[0]['output'])
            self.assertIn('test_database_boundary', console.getvalue())

    def test_nonzero_exit_keeps_failure_output_and_other_budget(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            with mock.patch.object(foundation.subprocess, 'run', return_value=subprocess.CompletedProcess(
                    ['cargo', 'test'], 7, stdout='assertion failed')) as run, \
                    contextlib.redirect_stdout(io.StringIO()) as console:
                code = foundation.run_checks([('.', ['cargo', 'test']), ('.', ['never-run'])], ROOT, output)
            self.assertEqual(code, 7)
            self.assertEqual(run.call_count, 1)
            self.assertEqual(run.call_args.kwargs['timeout'], 300)
            report = json.loads((output / 'local-preparation.json').read_text())['steps'][0]
            self.assertFalse(report['timed_out'])
            self.assertEqual(report['output'], 'assertion failed')
            self.assertIn('assertion failed', console.getvalue())

    def test_success_retains_each_receipt_and_advances(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            with mock.patch.object(foundation.subprocess, 'run', return_value=subprocess.CompletedProcess(
                    ['check'], 0, stdout='passed')) as run, contextlib.redirect_stdout(io.StringIO()):
                code = foundation.run_checks([('.', ['first']), ('infra', ['second'])], ROOT, output)
            self.assertEqual(code, 0)
            self.assertEqual(run.call_count, 2)
            report = json.loads((output / 'local-preparation.json').read_text())['steps']
            self.assertEqual([r['exit'] for r in report], [0, 0])
            self.assertEqual([r['timeout_seconds'] for r in report], [300, 300])


if __name__ == '__main__':
    unittest.main()
