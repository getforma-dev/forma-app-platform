import unittest
from ops.promotion.guards import STACK, outputs, retained_outputs
from ops.promotion.state import Refused


class GuardOutputTests(unittest.TestCase):
    def test_completed_rollback_is_ready_and_in_progress_is_not(self):
        stack = {'StackName': STACK, 'StackStatus': 'UPDATE_ROLLBACK_COMPLETE', 'Outputs': [{'OutputKey': 'A', 'OutputValue': '1'}]}
        self.assertEqual(outputs(stack), {'A': '1'})
        stack['StackStatus'] = 'UPDATE_COMPLETE'
        self.assertEqual(outputs(stack), {'A': '1'})
        stack['StackStatus'] = 'UPDATE_ROLLBACK_IN_PROGRESS'
        with self.assertRaises(Refused):
            outputs(stack)

    def test_scheduled_expiry_may_change_only_to_the_approved_parameter(self):
        previous = {'RetainedIdentity': 'same', 'ScheduledExpiryUtc': '2026-11-30T14:00:00Z'}
        approved = {'DeploymentExpiresAtUtc': '2026-11-29T20:00:00Z'}
        retained_outputs(previous, {'RetainedIdentity': 'same', 'ScheduledExpiryUtc': approved['DeploymentExpiresAtUtc']}, approved)
        with self.assertRaises(Refused):
            retained_outputs(previous, {'RetainedIdentity': 'changed', 'ScheduledExpiryUtc': approved['DeploymentExpiresAtUtc']}, approved)
        with self.assertRaises(Refused):
            retained_outputs(previous, {'RetainedIdentity': 'same', 'ScheduledExpiryUtc': '2026-01-01T00:00:00Z'}, approved)
