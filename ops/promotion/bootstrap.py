"""Generate reviewed bootstrap IaC from an actual private stack inventory.

Generation is local only. Applying the template and importing existing state are
separate explicit operator steps. No app data, secrets or provider are provisioned.
"""
import argparse
import json
from pathlib import Path
from .guards import ACCOUNT, REGION, REPOSITORY, ROLE_KEYS, REPOS
from .state import require

PREFIX = 'beta'
SUBJECT = 'repo:getforma-dev@268000170/forma-app-platform@1437433309:environment:'


def statement(actions, resources, **extra):
    return {'Effect': 'Allow', 'Action': actions, 'Resource': resources, **extra}


def concrete(value, prefix, label):
    require(isinstance(value, str) and value.startswith(prefix) and '*' not in value and '?' not in value, label)
    return value


def policy(statements):
    return {'Version': '2012-10-17', 'Statement': statements}


def role(name, trust, statements):
    return {'Type': 'AWS::IAM::Role', 'Properties': {'RoleName': name, 'MaxSessionDuration': 3600,
        'AssumeRolePolicyDocument': policy([trust]), 'Policies': [{'PolicyName': name, 'PolicyDocument': policy(statements)}]}}


def template(inventory):
    require(inventory['account'] == ACCOUNT and inventory['region'] == REGION, 'bootstrap account/region differs')
    require(inventory['stack_id'].startswith(f'arn:aws:cloudformation:{REGION}:{ACCOUNT}:stack/GenuiSyntheticFoundation/'), 'stack differs')
    require(set(inventory['definitions']) == set(ROLE_KEYS) and len(inventory['services']) == 2 and len(inventory['schedules']) == 2, 'incomplete runtime boundary')
    database = concrete(inventory['database_arn'], f'arn:aws:rds:{REGION}:{ACCOUNT}:db:', 'database attribute scope differs')
    attribute_roles = sorted(concrete(role, f'arn:aws:iam::{ACCOUNT}:role/', 'attribute role scope differs') for role in inventory['attribute_role_arns'])
    describe_keys = sorted(concrete(key, f'arn:aws:kms:{REGION}:{ACCOUNT}:key/', 'key attribute scope differs') for key in inventory['describe_key_arns'])
    require(attribute_roles and describe_keys, 'task-definition attribute reads are incomplete')
    cluster = concrete(inventory['cluster_arn'], f'arn:aws:ecs:{REGION}:{ACCOUNT}:cluster/', 'cluster attribute scope differs')
    log_group = concrete(inventory['log_group_arn'], f'arn:aws:logs:{REGION}:{ACCOUNT}:log-group:', 'log group attribute scope differs')
    schedule_group = concrete(inventory['schedule_group_arn'], f'arn:aws:scheduler:{REGION}:{ACCOUNT}:schedule-group/', 'schedule group attribute scope differs')
    evidence_bucket = concrete(inventory['evidence_bucket_arn'], 'arn:aws:s3:::', 'evidence bucket attribute scope differs')
    bucket_name = f'forma-beta-promotion-{ACCOUNT}-{REGION}'
    bucket = 'arn:aws:s3:::' + bucket_name
    objects = bucket + '/' + PREFIX + '/*'
    state = bucket + '/' + PREFIX + '/state.json'
    provider = {'Ref': 'GitHubOidc'}
    execution = f'arn:aws:iam::{ACCOUNT}:role/forma-beta-runtime-cloudformation'
    change_sets = f'arn:aws:cloudformation:{REGION}:{ACCOUNT}:changeSet/forma-promotion-*/*'
    stack = inventory['stack_id']
    ecs_task_roles = sorted({d[k] for d in inventory['definitions'].values() for k in ['taskRoleArn', 'executionRoleArn']})
    scheduler_roles = sorted({s['Target']['RoleArn'] for s in inventory['schedules']})
    families = sorted({d['taskDefinitionArn'].rsplit(':', 1)[0] + ':*' for d in inventory['definitions'].values()})
    service_arns = [s['serviceArn'] for s in inventory['services']]
    service_deployments = [s.replace(':service/', ':service-deployment/') + '/*' for s in service_arns]
    schedule_arns = [s['Arn'] for s in inventory['schedules']]
    common = [statement(['s3:GetObject', 's3:GetObjectVersion', 's3:PutObject'], objects),
              statement(['s3:GetBucketLocation', 's3:GetBucketVersioning'], bucket),
              statement(['sts:GetCallerIdentity'], '*'),
              statement(['cloudformation:DescribeStacks', 'cloudformation:DescribeStackEvents', 'cloudformation:GetTemplate', 'cloudformation:ListChangeSets', 'cloudformation:DescribeChangeSet'], stack),
              statement(['ecs:DescribeServices', 'ecs:DescribeTasks', 'ecs:ListTasks', 'ecs:DescribeTaskDefinition'], '*'),
              statement(['elasticloadbalancing:DescribeTargetHealth'], '*'),
              statement(['scheduler:GetSchedule'], schedule_arns)]
    ecr_read = statement(['ecr:DescribeRepositories', 'ecr:BatchGetImage', 'ecr:BatchCheckLayerAvailability', 'ecr:DescribeImageScanFindings', 'ecr:DescribeImages'],
        [f'arn:aws:ecr:{REGION}:{ACCOUNT}:repository/{r}' for r in REPOS.values()])
    prepare = common + [ecr_read,
        statement(['ecr:InitiateLayerUpload', 'ecr:UploadLayerPart', 'ecr:CompleteLayerUpload', 'ecr:PutImage'], ecr_read['Resource']),
        statement(['cloudformation:CreateChangeSet'], stack, Condition={'StringEquals': {'cloudformation:RoleArn': execution},
            'StringLike': {'cloudformation:ChangeSetName': ['forma-promotion-*', change_sets], 'cloudformation:TemplateUrl': f'https://{bucket_name}.s3.{REGION}.amazonaws.com/{PREFIX}/promotions/*/template.json'}}),
        statement(['iam:PassRole'], execution, Condition={'StringEquals': {'iam:PassedToService': 'cloudformation.amazonaws.com'}})]
    prepare.append(statement(['cloudformation:DeleteChangeSet'], stack, Condition={'StringLike': {'cloudformation:ChangeSetName': ['forma-promotion-*', change_sets]}}))
    execute = common + [ecr_read, statement(['cloudformation:ExecuteChangeSet'], stack, Condition={'StringLike': {'cloudformation:ChangeSetName': ['forma-promotion-*', change_sets]}}),
        statement(['ecs:UpdateService'], service_arns)]
    # APP-175 schema release: one snapshot and the stack's own migration task,
    # only on this cluster, only with that task definition's two roles. No tags.
    migration = inventory['definitions']['MigrationTaskDefinition']
    migration_family = concrete(migration['taskDefinitionArn'], f'arn:aws:ecs:{REGION}:{ACCOUNT}:task-definition/', 'migration task scope differs').rsplit(':', 1)[0] + ':*'
    migration_roles = [concrete(migration[key], f'arn:aws:iam::{ACCOUNT}:role/', 'migration role scope differs') for key in ['taskRoleArn', 'executionRoleArn']]
    release_snapshots = f'arn:aws:rds:{REGION}:{ACCOUNT}:snapshot:forma-schema-release-*'
    execute += [statement(['ecs:RunTask'], migration_family, Condition={'ArnEquals': {'ecs:cluster': cluster}}),
        statement(['iam:PassRole'], migration_roles, Condition={'StringEquals': {'iam:PassedToService': 'ecs-tasks.amazonaws.com'}}),
        statement(['rds:CreateDBSnapshot'], [database, release_snapshots]),
        # The service reference lists both db and snapshot resources for this read.
        statement(['rds:DescribeDBSnapshots'], [database, release_snapshots])]
    execution_rules = [statement(['ecs:RegisterTaskDefinition', 'ecs:TagResource', 'ecs:UntagResource'], families),
        statement(['ecs:DescribeTaskDefinition'], families),
        # The ECS API authorizes deregister against resource "*", not the family ARN.
        statement(['ecs:DeregisterTaskDefinition'], '*'),
        statement(['ecs:UpdateService', 'ecs:DescribeServices', 'ecs:TagResource', 'ecs:UntagResource'], service_arns),
        statement(['ecs:DescribeServiceDeployments', 'ecs:StopServiceDeployment'], service_arns + service_deployments),
        statement(['ecs:ListServiceDeployments'], service_arns),
        statement(['ecs:ListTagsForResource'], service_arns + families),
        statement(['scheduler:GetSchedule', 'scheduler:UpdateSchedule'], schedule_arns),
        statement(['iam:PassRole'], ecs_task_roles, Condition={'StringEquals': {'iam:PassedToService': 'ecs-tasks.amazonaws.com'}}),
        statement(['iam:PassRole'], scheduler_roles, Condition={'StringEquals': {'iam:PassedToService': 'scheduler.amazonaws.com'}}),
        statement(['iam:GetRole'], sorted(set(ecs_task_roles + scheduler_roles + attribute_roles))),
        # Only the attributes this stack's task definitions and services embed.
        # DescribeSecurityGroups cannot take a group ARN; every other read is one resource.
        statement(['rds:DescribeDBInstances'], database),
        statement(['kms:DescribeKey'], describe_keys),
        statement(['ecs:DescribeClusters'], cluster),
        statement(['logs:DescribeLogGroups'], log_group),
        statement(['scheduler:GetScheduleGroup'], schedule_group),
        statement(['s3:GetBucketLocation', 's3:ListBucket'], evidence_bucket),
        # These two describe calls reject a resource ARN. They cannot change the resource.
        statement(['ec2:DescribeSecurityGroups'], '*'),
        statement(['elasticloadbalancing:DescribeLoadBalancers'], '*'),
        # CDK's BootstrapVersion is an SSM-backed template parameter, resolved
        # while creating the change set (before resource handlers execute).
        statement(['ssm:GetParameters'], f'arn:aws:ssm:{REGION}:{ACCOUNT}:parameter/cdk-bootstrap/hnb659fds/version'),
        statement(['s3:GetObject'], bucket + '/' + PREFIX + '/promotions/*/template.json')]
    resources = {
        'State': {'Type': 'AWS::S3::Bucket', 'DeletionPolicy': 'Retain', 'UpdateReplacePolicy': 'Retain', 'Properties': {
            'BucketName': bucket_name, 'VersioningConfiguration': {'Status': 'Enabled'},
            'BucketEncryption': {'ServerSideEncryptionConfiguration': [{'ServerSideEncryptionByDefault': {'SSEAlgorithm': 'AES256'}}]},
            'OwnershipControls': {'Rules': [{'ObjectOwnership': 'BucketOwnerEnforced'}]},
            'PublicAccessBlockConfiguration': {'BlockPublicAcls': True, 'BlockPublicPolicy': True, 'IgnorePublicAcls': True, 'RestrictPublicBuckets': True}}},
        'StatePolicy': {'Type': 'AWS::S3::BucketPolicy', 'Properties': {'Bucket': {'Ref': 'State'}, 'PolicyDocument': policy([
            {'Effect': 'Deny', 'Principal': '*', 'Action': ['s3:DeleteObject', 's3:DeleteObjectVersion'], 'Resource': objects},
            {'Effect': 'Deny', 'Principal': '*', 'Action': 's3:*', 'Resource': [bucket, bucket + '/*'], 'Condition': {'Bool': {'aws:SecureTransport': 'false'}}},
            {'Effect': 'Deny', 'Principal': '*', 'Action': 's3:PutObject', 'Resource': state,
             'Condition': {'Null': {'s3:if-match': 'true', 's3:if-none-match': 'true'}, 'Bool': {'s3:ObjectCreationOperation': 'true'}}},
            {'Effect': 'Deny', 'Principal': '*', 'Action': 's3:PutObject', 'Resource': bucket + '/' + PREFIX + '/promotions/*',
             'Condition': {'Null': {'s3:if-none-match': 'true'}, 'Bool': {'s3:ObjectCreationOperation': 'true'}}}])}},
        'GitHubOidc': {'Type': 'AWS::IAM::OIDCProvider', 'Properties': {'Url': 'https://token.actions.githubusercontent.com', 'ClientIdList': ['sts.amazonaws.com']}},
        'RuntimeExecution': role('forma-beta-runtime-cloudformation', {'Effect': 'Allow', 'Principal': {'Service': 'cloudformation.amazonaws.com'}, 'Action': 'sts:AssumeRole'}, execution_rules),
    }
    for logical, name, environment, permissions in [('Prepare', 'forma-beta-promotion-prepare', 'beta-plan', prepare), ('Execute', 'forma-beta-promotion-execute', 'beta', execute)]:
        trust = {'Effect': 'Allow', 'Principal': {'Federated': provider}, 'Action': 'sts:AssumeRoleWithWebIdentity',
            'Condition': {'StringEquals': {'token.actions.githubusercontent.com:aud': 'sts.amazonaws.com',
                                         'token.actions.githubusercontent.com:sub': SUBJECT + environment}}}
        resources[logical] = role(name, trust, permissions)
    return {'AWSTemplateFormatVersion': '2010-09-09', 'Description': 'Runtime-only beta promotion state and least-privilege identities; no app resources',
        'Resources': resources, 'Outputs': {'StateBucket': {'Value': {'Ref': 'State'}}, 'StatePrefix': {'Value': PREFIX},
            'PrepareRoleArn': {'Value': {'Fn::GetAtt': ['Prepare', 'Arn']}}, 'ExecuteRoleArn': {'Value': {'Fn::GetAtt': ['Execute', 'Arn']}},
            'ExecutionRoleArn': {'Value': {'Fn::GetAtt': ['RuntimeExecution', 'Arn']}}}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inventory', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = template(json.loads(args.inventory.read_text()))
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print('Bootstrap template generated locally; no AWS changes made.')


if __name__ == '__main__':
    main()
