"""Public refusals must remain useful without exposing private exception data."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

from ops.promotion import __main__ as cli
from ops.promotion.state import LeaseHeld, Refused, Store
from ops.promotion.test_state import MemoryS3, authorization, state


class CliTests(unittest.TestCase):
    def invoke_failure(self, failure, phase='prepare'):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); request = root / 'request.json'
            request.write_text(json.dumps({'candidate_sha': 'a' * 40}))
            output = root / 'private' / 'summary.json'
            runner = Mock()
            getattr(runner, phase).side_effect = failure
            arguments = [phase, '--request', str(request), '--artifact-dir', str(root / 'qualified'),
                         '--state-bucket', 'private-bucket', '--state-prefix', 'beta', '--output', str(output)]
            if phase == 'execute':
                approval = root / 'approval.json'; approval.write_text('{}')
                arguments += ['--approval', str(approval)]
            stdout = io.StringIO(); stderr = io.StringIO()
            with patch.object(cli, 'AWS'), patch.object(cli, 'Store'), patch.object(cli, 'Runner', return_value=runner), \
                    contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                self.assertEqual(cli.main(arguments), 1)
            self.assertEqual(stdout.getvalue(), '')
            self.assertFalse(output.exists())
            return stderr.getvalue()

    def test_actual_held_lease_has_fixed_actionable_diagnostic(self):
        s3 = MemoryS3(state()); store = Store(s3, 'bucket', 'beta')
        store.acquire('existing', authorization(), 'a')
        before = dict(s3.objects)
        def acquire(*args):
            Store(s3, 'bucket', 'beta').acquire('replacement', authorization(), 'b')
        result = self.invoke_failure(acquire)
        self.assertEqual(result, 'PROMOTION_LOCK_HELD: An earlier promotion retains the deployment lock. '
                         'Use operator maintenance inspect and reconcile it before a new promotion. '
                         'Do not clear the lock or retry blindly.\n')
        self.assertEqual(s3.objects, before)

    def test_known_refusal_never_interpolates_its_exception(self):
        result = self.invoke_failure(LeaseHeld('private-token signed-url private-bucket'))
        self.assertIn('PROMOTION_LOCK_HELD:', result)
        for secret in ['private-token', 'signed-url', 'private-bucket']:
            self.assertNotIn(secret, result)

    def test_unclassified_and_uncertain_failures_stay_generic(self):
        for phase in ['prepare', 'execute']:
            for error in [Refused('another promotion or unresolved outcome holds the lease'),
                          TimeoutError('private-token signed-url private-bucket')]:
                with self.subTest(phase=phase, error=type(error).__name__):
                    result = self.invoke_failure(error, phase)
                    self.assertEqual(result, 'Promotion refused or outcome unresolved. The private lease and journal '
                                     'are retained; inspect them before any retry.\n')


if __name__ == '__main__':
    unittest.main()
