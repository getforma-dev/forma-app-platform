import unittest
from ops.promotion.guards import STACK, outputs, retained_outputs, service
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

    def test_unsettled_predecessor_keeps_desired_count_and_task(self):
        value = {'serviceArn': 'svc', 'clusterArn': 'cluster', 'taskDefinition': 'task:1', 'status': 'ACTIVE',
                 'desiredCount': 1, 'runningCount': 0, 'pendingCount': 1,
                 'deployments': [{'rolloutState': 'IN_PROGRESS'}]}
        service(value, 'svc', 'cluster', 'task:1', 1, settled=False)
        with self.assertRaises(Refused):
            service(value, 'svc', 'cluster', 'task:1', 1)
        value['desiredCount'] = 0
        with self.assertRaises(Refused):
            service(value, 'svc', 'cluster', 'task:1', 1, settled=False)
