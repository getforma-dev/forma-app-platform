"""Closed runtime-only deployment boundaries, shared by live and fault tests."""
import copy
import re
from .state import closed, digest, require

ACCOUNT = '877573132739'
REGION = 'us-east-1'
STACK = 'GenuiSyntheticFoundation'
REPOSITORY = 'getforma-dev/forma-app-platform'
WORKFLOW = REPOSITORY + '/.github/workflows/beta-promote.yml@refs/heads/main'
IMAGE_KEYS = {'service': 'ImageDigest', 'qualification': 'QualificationImageDigest', 'operator': 'OperatorImageDigest'}
REPOS = {'service': 'genui-synthetic', 'qualification': 'genui-qualification', 'operator': 'genui-operator'}
ROLE_KEYS = {'TaskDefinition': 'service', 'RollbackTaskDefinition': 'service', 'MigrationTaskDefinition': 'service',
             'QualificationTaskDefinition': 'qualification', 'OperatorTaskDefinition': 'operator', 'SharedBrokerTaskDefinition': 'operator'}


def authorization(value):
    closed(value, ['actor', 'run_id', 'run_attempt', 'repository', 'workflow_ref'], 'authorization')
    require(value['repository'] == REPOSITORY and value['workflow_ref'] == WORKFLOW, 'workflow is not trusted trunk promotion')
    require(isinstance(value['actor'], str) and re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9-]{0,99}', value['actor']), 'invalid actor')
    for field in ['run_id', 'run_attempt']:
        require(isinstance(value[field], str) and re.fullmatch(r'[1-9][0-9]{0,19}', value[field]), 'invalid run provenance')


def request(value, phase):
    keys = ['candidate_sha', 'qualified_artifact_manifest_sha256', 'authorization']
    if phase == 'execute':
        keys += ['promotion_id', 'packet_sha256']
    closed(value, keys, 'promotion request')
    require(re.fullmatch(r'[0-9a-f]{40}', value['candidate_sha']), 'invalid candidate SHA')
    require(re.fullmatch(r'[0-9a-f]{64}', value['qualified_artifact_manifest_sha256']), 'invalid artifact digest')
    if phase == 'execute':
        require(re.fullmatch(r'[0-9a-f-]{36}', value['promotion_id']) and re.fullmatch(r'[0-9a-f]{64}', value['packet_sha256']), 'invalid promotion identity')
    authorization(value['authorization'])


def outputs(stack):
    # A completed rollback is the previous template at rest. An in-progress
    # rollback is not ready for a new change set.
    require(stack['StackName'] == STACK and stack['StackStatus'] in {'UPDATE_COMPLETE', 'UPDATE_ROLLBACK_COMPLETE'}, 'stack not ready')
    return {p['OutputKey']: p['OutputValue'] for p in stack['Outputs']}


def retained_outputs(previous, current, expected):
    """Stable stack outputs must not move. The scheduled stop time is the approved parameter."""
    for key, value in previous.items():
        if key in ROLE_KEYS:
            continue
        if key == 'ScheduledExpiryUtc':
            require(current.get(key) == expected['DeploymentExpiresAtUtc'], 'scheduled expiry output does not match the approved parameter')
            continue
        require(current.get(key) == value, 'retained output changed: ' + key)


def parameters(stack):
    return {p['ParameterKey']: p['ParameterValue'] for p in stack['Parameters']}


def runtime_parameters(previous, receipt):
    result = dict(previous)
    result.update({k: str(v) for k, v in receipt['cloudformation_parameters'].items()})
    changed = {k for k in set(previous) | set(result) if previous.get(k) != result.get(k)}
    require(changed <= set(IMAGE_KEYS.values()) | {'DeploymentExpiresAtUtc'}, 'non-runtime parameter changed')
    require(result.get('ServiceDesiredCount') == '0', 'template must retain stopped desired count')
    return result


def changeset(value, expected_parameters, expected_role, template, create_request):
    require(value.get('StackName') == STACK and value.get('Status') == 'CREATE_COMPLETE'
            and value.get('ExecutionStatus') == 'AVAILABLE', 'change set not available')
    require(create_request.get('RoleARN') == expected_role, 'change-set request must use restricted execution role')
    require(parameters(value) == expected_parameters, 'change-set parameters differ')
    resources = template['Resources']
    changed = value.get('Changes', [])
    require(changed, 'empty change set')
    allowed = {'AWS::ECS::TaskDefinition', 'AWS::ECS::Service', 'AWS::Scheduler::Schedule'}
    for entry in changed:
        r = entry['ResourceChange']
        require(r['Action'] == 'Modify' and r['LogicalResourceId'] in resources, 'resource added or removed')
        require(r['ResourceType'] == resources[r['LogicalResourceId']]['Type'] and r['ResourceType'] in allowed,
                'change outside runtime task/service/schedule')
        require(r.get('Replacement') == 'False' or r['ResourceType'] == 'AWS::ECS::TaskDefinition', 'protected runtime resource replacement')
    require(value.get('ChangeSetId', '').startswith(f'arn:aws:cloudformation:{REGION}:{ACCOUNT}:changeSet/'), 'change-set ARN differs')


def definition(previous, current, image):
    require(current.get('runtimePlatform', {}).get('cpuArchitecture') == 'ARM64', 'not ARM64')
    old_task, new_task = copy.deepcopy(previous), copy.deepcopy(current)
    generated = ['taskDefinitionArn', 'revision', 'status', 'requiresAttributes', 'compatibilities',
                 'registeredAt', 'registeredBy', 'deregisteredAt', 'containerDefinitions']
    for document in [old_task, new_task]:
        for field in generated: document.pop(field, None)
    require(old_task == new_task, 'effective task-definition boundary changed')
    old = copy.deepcopy(previous['containerDefinitions']); new = copy.deepcopy(current['containerDefinitions'])
    require(len(old) == len(new) == 1 and new[0]['image'] == image, 'container image differs')
    old[0].pop('image'); new[0].pop('image')
    require(old == new, 'container configuration, alias or secret reference changed')


def service(value, expected_arn, expected_cluster, task, count, settled=True):
    require(value.get('serviceArn') == expected_arn and value.get('clusterArn') == expected_cluster,
            'service identity differs')
    require(value.get('taskDefinition') == task and value.get('status') == 'ACTIVE', 'service task/status differs')
    require(value.get('desiredCount') == count, 'service desired count differs')
    if not settled:
        return
    require(tuple(value.get(k) for k in ['desiredCount', 'runningCount', 'pendingCount']) == (count, count, 0), 'service counts not settled')
    require(len(value['deployments']) == 1 and value['deployments'][0].get('rolloutState') == 'COMPLETED', 'service rollout not complete')


def schedule(previous, current, expiry):
    old = copy.deepcopy(previous); new = copy.deepcopy(current)
    for value in [old, new]:
        for key in ['ResponseMetadata', 'CreationDate', 'LastModificationDate']: value.pop(key, None)
    require(new['State'] == 'ENABLED' and new['ScheduleExpressionTimezone'] == 'UTC', 'expiry disabled')
    require(new['ScheduleExpression'] == 'at(' + expiry[:-1] + ')', 'expiry differs')
    old['ScheduleExpression'] = new['ScheduleExpression']
    require(old == new, 'schedule target, role or retry policy changed')
