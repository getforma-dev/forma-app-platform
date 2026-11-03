import unittest
from ops.promotion.guards import STACK, outputs
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
