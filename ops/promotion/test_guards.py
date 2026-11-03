import copy
import unittest
from ops.promotion import guards
from ops.promotion.state import Refused


class GuardTests(unittest.TestCase):
    def definition(self):
        return {'taskDefinitionArn': 'task:1', 'revision': 1, 'status': 'ACTIVE', 'runtimePlatform': {'cpuArchitecture': 'ARM64'},
                'family': 'app', 'pidMode': 'task', 'containerDefinitions': [{'name': 'app', 'image': 'old', 'environment': [{'name': 'alias', 'value': 'original'}]}]}

    def test_only_image_and_aws_revision_metadata_may_change(self):
        old = self.definition(); new = copy.deepcopy(old); new['revision'] = 2; new['taskDefinitionArn'] = 'task:2'; new['containerDefinitions'][0]['image'] = 'new'
        guards.definition(old, new, 'new')
        for key, value in [('pidMode', 'host'), ('ephemeralStorage', {'sizeInGiB': 100}), ('enableFaultInjection', True)]:
            changed = dict(new, **{key: value})
            with self.assertRaises(Refused): guards.definition(old, changed, 'new')
        new['containerDefinitions'][0]['environment'][0]['value'] = 'changed'
        with self.assertRaises(Refused): guards.definition(old, new, 'new')

    def test_parameter_changes_are_closed(self):
        old = {'ImageDigest': 'old', 'ServiceDesiredCount': '0', 'CleanTrialOrganizationId': 'original'}
        self.assertEqual(guards.runtime_parameters(old, {'cloudformation_parameters': {'ImageDigest': 'new'}})['ImageDigest'], 'new')
        with self.assertRaises(Refused): guards.runtime_parameters(old, {'cloudformation_parameters': {'CleanTrialOrganizationId': 'other'}})

    def test_unknown_resource_change_and_admin_role_rejected(self):
        doc = {'StackName': guards.STACK, 'Status': 'CREATE_COMPLETE', 'ExecutionStatus': 'AVAILABLE',
               'ChangeSetId': f'arn:aws:cloudformation:{guards.REGION}:{guards.ACCOUNT}:changeSet/x/id',
               'Parameters': [{'ParameterKey': 'ImageDigest', 'ParameterValue': 'new'}],
               'Changes': [{'ResourceChange': {'Action': 'Modify', 'LogicalResourceId': 'Task', 'ResourceType': 'AWS::ECS::TaskDefinition', 'Replacement': 'True'}}]}
        template = {'Resources': {'Task': {'Type': 'AWS::ECS::TaskDefinition'}}}
        guards.changeset(doc, {'ImageDigest': 'new'}, 'narrow', template, {'RoleARN': 'narrow'})
        with self.assertRaises(Refused): guards.changeset(doc, {'ImageDigest': 'new'}, 'narrow', template, {'RoleARN': 'admin'})
        doc['Changes'][0]['ResourceChange']['Action'] = 'Add'
        with self.assertRaises(Refused): guards.changeset(doc, {'ImageDigest': 'new'}, 'narrow', template, {'RoleARN': 'narrow'})


if __name__ == '__main__': unittest.main()
