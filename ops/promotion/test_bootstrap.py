import unittest
from ops.promotion.bootstrap import template, SUBJECT
from ops.promotion.guards import ACCOUNT, REGION, ROLE_KEYS


def inventory():
    return {'account': ACCOUNT, 'region': REGION,
            'stack_id': f'arn:aws:cloudformation:{REGION}:{ACCOUNT}:stack/GenuiSyntheticFoundation/example',
            'cluster_arn': f'arn:aws:ecs:{REGION}:{ACCOUNT}:cluster/beta',
            'services': [{'serviceArn': f'arn:aws:ecs:{REGION}:{ACCOUNT}:service/beta/{n}'} for n in ['main','broker']],
            'schedules': [{'Arn': f'arn:aws:scheduler:{REGION}:{ACCOUNT}:schedule/default/{n}',
                           'Target': {'RoleArn': f'arn:aws:iam::{ACCOUNT}:role/{n}-expiry'}} for n in ['main','broker']],
            'definitions': {k: {'taskDefinitionArn': f'arn:aws:ecs:{REGION}:{ACCOUNT}:task-definition/{k}:1',
                               'taskRoleArn': f'arn:aws:iam::{ACCOUNT}:role/{k}',
                               'executionRoleArn': f'arn:aws:iam::{ACCOUNT}:role/{k}-execution'} for k in ROLE_KEYS},
            'target_groups': [f'arn:aws:elasticloadbalancing:{REGION}:{ACCOUNT}:targetgroup/main/a'],
            'database_arn': f'arn:aws:rds:{REGION}:{ACCOUNT}:db:genui',
            'attribute_role_arns': [f'arn:aws:iam::{ACCOUNT}:role/db-access', f'arn:aws:iam::{ACCOUNT}:role/tenant-access'],
            'describe_key_arns': [f'arn:aws:kms:{REGION}:{ACCOUNT}:key/11111111-1111-1111-1111-111111111111'],
            'log_group_arn': f'arn:aws:logs:{REGION}:{ACCOUNT}:log-group:beta',
            'schedule_group_arn': f'arn:aws:scheduler:{REGION}:{ACCOUNT}:schedule-group/beta',
            'evidence_bucket_arn': 'arn:aws:s3:::genui-evidence'}


def statements(document, logical):
    return document['Resources'][logical]['Properties']['Policies'][0]['PolicyDocument']['Statement']


def schema_release_statements():
    migration = inventory()['definitions']['MigrationTaskDefinition']
    snapshots = f'arn:aws:rds:{REGION}:{ACCOUNT}:snapshot:forma-schema-release-*'
    database = inventory()['database_arn']
    return [{'Effect': 'Allow', 'Action': ['ecs:RunTask'],
             'Resource': f'arn:aws:ecs:{REGION}:{ACCOUNT}:task-definition/MigrationTaskDefinition:*',
             'Condition': {'ArnEquals': {'ecs:cluster': inventory()['cluster_arn']}}},
            {'Effect': 'Allow', 'Action': ['iam:PassRole'],
             'Resource': [migration['taskRoleArn'], migration['executionRoleArn']],
             'Condition': {'StringEquals': {'iam:PassedToService': 'ecs-tasks.amazonaws.com'}}},
            {'Effect': 'Allow', 'Action': ['rds:CreateDBSnapshot'], 'Resource': [database, snapshots]},
            {'Effect': 'Allow', 'Action': ['rds:DescribeDBSnapshots'], 'Resource': [database, snapshots]},
            {'Effect': 'Allow', 'Action': ['rds:AddTagsToResource'], 'Resource': snapshots}]


class BootstrapTests(unittest.TestCase):
    def test_cdk_parameter_read_is_exact_and_only_on_cloudformation_role(self):
        doc = template(inventory())
        cf = statements(doc, 'RuntimeExecution')
        reads = [row for row in cf if any(action.startswith('ssm:') for action in row['Action'])]
        self.assertEqual(reads, [{'Effect': 'Allow', 'Action': ['ssm:GetParameters'],
            'Resource': f'arn:aws:ssm:{REGION}:{ACCOUNT}:parameter/cdk-bootstrap/hnb659fds/version'}])
        for role in ['Prepare', 'Execute']:
            self.assertFalse(any(action.startswith('ssm:') for row in statements(doc, role) for action in row['Action']))

    def test_roles_have_disjoint_mutation_authority(self):
        doc=template(inventory())
        prep=statements(doc,'Prepare');execute=statements(doc,'Execute');cf=statements(doc,'RuntimeExecution')
        actions=lambda rows:{a for r in rows for a in r['Action']}
        self.assertNotIn('ecs:UpdateService',actions(prep));self.assertNotIn('cloudformation:ExecuteChangeSet',actions(prep))
        self.assertNotIn('cloudformation:CreateChangeSet',actions(execute))
        # Execute passes only the migration task's two roles, to ECS (APP-175).
        self.assertEqual([r for r in execute if 'iam:PassRole' in r['Action']], [schema_release_statements()[1]])
        schema_rds = {'rds:CreateDBSnapshot', 'rds:DescribeDBSnapshots', 'rds:AddTagsToResource'}
        forbidden = {'kms:Decrypt','kms:GetKeyPolicy','kms:CreateGrant','secretsmanager:GetSecretValue','secretsmanager:DescribeSecret','iam:CreateRole','iam:PutRolePolicy','iam:AttachRolePolicy','iam:ListRolePolicies','iam:ListAttachedRolePolicies','rds:Describe*','ec2:Describe*','kms:Describe*'}
        for rows in [prep,execute,cf]:
            self.assertNotIn('*',actions(rows))
            allowed_rds = schema_rds if rows is execute else {'rds:DescribeDBInstances'}
            self.assertFalse(any(a in forbidden or a.startswith('rds:') and a not in allowed_rds or a.startswith('kms:') and a != 'kms:DescribeKey' for a in actions(rows)))
            self.assertNotIn('s3:DeleteObject',actions(rows));self.assertNotIn('iam:CreateRole',actions(rows))
        self.assertEqual(next(s for s in cf if s['Action']==['rds:DescribeDBInstances'])['Resource'], inventory()['database_arn'])
        self.assertEqual(next(s for s in cf if s['Action']==['kms:DescribeKey'])['Resource'], inventory()['describe_key_arns'])
        self.assertEqual(next(s for s in cf if s['Action']==['ecs:DescribeClusters'])['Resource'], inventory()['cluster_arn'])
        self.assertEqual(next(s for s in cf if s['Action']==['ec2:DescribeSecurityGroups'])['Resource'], '*')
        self.assertEqual(next(s for s in cf if s['Action']==['elasticloadbalancing:DescribeLoadBalancers'])['Resource'], '*')
        self.assertEqual(next(s for s in cf if s['Action']==['logs:DescribeLogGroups'])['Resource'], inventory()['log_group_arn'])
        self.assertNotIn('elasticloadbalancing:CreateLoadBalancer', actions(cf))
        self.assertNotIn('logs:GetLogEvents', actions(cf))
        self.assertNotIn('s3:GetObject', [a for s in cf if s.get('Resource')==inventory()['evidence_bucket_arn'] for a in s['Action']])
        self.assertFalse(any(a.startswith(('rds:','kms:','ec2:')) for a in actions(prep)))
        self.assertFalse(any(a.startswith(('kms:','ec2:')) for a in actions(execute)))
        self.assertEqual({a for a in actions(execute) if a.startswith('rds:')}, schema_rds)
        for bad in ('*', 'arn:aws:rds:us-east-1:123:db:*', 'arn:aws:iam::123:role/*'):
            broken = inventory()
            if bad.startswith('arn:aws:rds'): broken['database_arn'] = bad
            elif 'role' in bad: broken['attribute_role_arns'] = [bad]
            else: broken['cluster_arn'] = bad
            with self.assertRaises(Exception):
                template(broken)
        create=next(r for r in prep if 'cloudformation:CreateChangeSet' in r['Action'])
        self.assertEqual(create['Condition']['StringEquals']['cloudformation:RoleArn'],f'arn:aws:iam::{ACCOUNT}:role/forma-beta-runtime-cloudformation')

    def test_schema_release_adds_exactly_the_migration_task_and_snapshot_scope(self):
        doc=template(inventory()); execute=statements(doc,'Execute')
        schema_actions={'ecs:RunTask','iam:PassRole','rds:CreateDBSnapshot','rds:DescribeDBSnapshots','rds:AddTagsToResource'}
        added=[r for r in execute if set(r['Action']) & schema_actions]
        self.assertEqual(added, schema_release_statements())
        # Tagging only ever reaches the release snapshots, never the database.
        self.assertEqual([r['Resource'] for r in execute if 'rds:AddTagsToResource' in r['Action']],
                         [f'arn:aws:rds:{REGION}:{ACCOUNT}:snapshot:forma-schema-release-*'])
        for rows in [statements(doc,'Prepare'), statements(doc,'RuntimeExecution')]:
            self.assertFalse((schema_actions - {'iam:PassRole'}) & {a for r in rows for a in r['Action']})
        self.assertFalse(any(a in {'ecs:TagResource','ecs:StopTask','rds:DeleteDBSnapshot','rds:RestoreDBInstanceFromDBSnapshot','rds:RemoveTagsFromResource'}
            for r in execute for a in r['Action']))
        for key, bad in [('taskDefinitionArn', f'arn:aws:ecs:{REGION}:{ACCOUNT}:task-definition/*'),
                         ('taskRoleArn', f'arn:aws:iam::{ACCOUNT}:role/*'), ('executionRoleArn', '*')]:
            with self.subTest(key=key):
                broken = inventory(); broken['definitions']['MigrationTaskDefinition'][key] = bad
                with self.assertRaises(Exception): template(broken)

    def test_ecs_supported_resource_and_condition_shapes(self):
        doc=template(inventory());cf=statements(doc,'RuntimeExecution')
        register=next(r for r in cf if 'ecs:RegisterTaskDefinition' in r['Action'])
        self.assertEqual(len(register['Resource']),6);self.assertTrue(all('/' in r and r.endswith(':*') for r in register['Resource']))
        self.assertEqual(next(r for r in cf if 'ecs:DescribeTaskDefinition' in r['Action'])['Resource'], register['Resource'])
        self.assertEqual(next(r for r in cf if r['Action']==['ecs:DeregisterTaskDefinition'])['Resource'], '*')
        target_health=next(r for r in statements(doc,'Execute') if 'elasticloadbalancing:DescribeTargetHealth' in r['Action'])
        self.assertEqual(target_health['Resource'],'*')
        update=next(r for r in statements(doc,'Execute') if 'ecs:UpdateService' in r['Action'])
        self.assertNotIn('Condition',update);self.assertEqual(update['Resource'],[s['serviceArn'] for s in inventory()['services']])

    def test_current_ecs_handler_dependencies_are_scoped(self):
        rules=statements(template(inventory()),'RuntimeExecution')
        services=[s['serviceArn'] for s in inventory()['services']]
        deployments=[s.replace(':service/',':service-deployment/')+'/*' for s in services]
        for action in ['ecs:DescribeServiceDeployments','ecs:StopServiceDeployment']:
            self.assertEqual(next(r for r in rules if action in r['Action'])['Resource'], services+deployments)
        self.assertEqual(next(r for r in rules if 'ecs:ListServiceDeployments' in r['Action'])['Resource'],services)
        self.assertEqual(len(next(r for r in rules if 'ecs:ListTagsForResource' in r['Action'])['Resource']),8)

    def test_changeset_permissions_bind_supported_stack_and_name_condition(self):
        import fnmatch
        doc=template(inventory())
        for role,action in [('Prepare','cloudformation:CreateChangeSet'),('Execute','cloudformation:ExecuteChangeSet')]:
            rule=next(r for r in statements(doc,role) if action in r['Action'])
            self.assertEqual(rule['Resource'],inventory()['stack_id'])
            patterns=rule['Condition']['StringLike']['cloudformation:ChangeSetName']
            for value,allowed in [('forma-promotion-reviewed',True),('genui-app078-old-admin',False),
                (f'arn:aws:cloudformation:{REGION}:{ACCOUNT}:changeSet/forma-promotion-reviewed/id',True),
                (f'arn:aws:cloudformation:{REGION}:{ACCOUNT}:changeSet/admin-other/id',False)]:
                self.assertEqual(any(fnmatch.fnmatchcase(value,p) for p in patterns),allowed)

    def test_oidc_is_exact_immutable_repository_and_environment(self):
        doc=template(inventory())
        for role,env in [('Prepare','beta-plan'),('Execute','beta')]:
            condition=doc['Resources'][role]['Properties']['AssumeRolePolicyDocument']['Statement'][0]['Condition']['StringEquals']
            self.assertEqual(condition['token.actions.githubusercontent.com:sub'],SUBJECT+env)
            self.assertEqual(condition['token.actions.githubusercontent.com:aud'],'sts.amazonaws.com')

    def test_private_versioned_state_requires_conditional_writes(self):
        resources=template(inventory())['Resources']
        self.assertEqual(resources['State']['Properties']['VersioningConfiguration']['Status'],'Enabled')
        self.assertTrue(all(resources['State']['Properties']['PublicAccessBlockConfiguration'].values()))
        rules=resources['StatePolicy']['Properties']['PolicyDocument']['Statement']
        self.assertEqual(rules[0]['Action'], ['s3:DeleteObject', 's3:DeleteObjectVersion'])
        self.assertEqual(rules[2]['Condition']['Null'],{'s3:if-match':'true','s3:if-none-match':'true'})
        self.assertEqual(rules[3]['Condition']['Null'],{'s3:if-none-match':'true'})


if __name__=='__main__':unittest.main()
