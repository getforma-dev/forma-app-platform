import assert from 'node:assert/strict';
import { test } from 'node:test';
import { App } from 'aws-cdk-lib';
import { Match, Template } from 'aws-cdk-lib/assertions';
import { readFileSync } from 'node:fs';
import { ALB_IDLE_TIMEOUT_SECONDS, AUTH_CONFIGURATION_ENV, DEMO_MODE_ENV, DEMO_ORGANIZATIONS_ENV, DEMO_ORGANIZATION_ID, AUTH_CONFIGURATION_SECRET_ID_ENV, CANVAS_EVENTS_URL, CANVAS_EVENTS_URL_ENV, CAPABILITY_SECRET_ENV, CAPABILITY_SECRET_ID_ENV, COMPOSE_DEADLINE_SECONDS, COMPOSER_KEY_ENV, GenuiFoundation, SLACK_APP_PLACEHOLDER, SLACK_BOT_TOKEN_ENV, SLACK_SIGNING_SECRET_ENV, IMAGE_DIGEST_PATTERN, INVITATION_INPUT_ENV, OPERATOR_CIDR_PATTERN, OPERATOR_IMAGE_PATTERN, PROVIDER_MANAGEMENT_KEY_ENV, PROVIDER_MANAGEMENT_SECRET_ARN_PATTERN, PROVIDER_MANAGEMENT_TENANT_ENV, PROVIDER_MANAGEMENT_URL_ENV, PROVIDER_MANAGEMENT_URL_PATTERN, QUALIFICATION_IMAGE_PATTERN } from '../src/stack.js';

const template = Template.fromStack(new GenuiFoundation(new App(), 'SecurityQualification'));
const document = template.toJSON();
const entries = (type: string): Array<[string, any]> => Object.entries(template.findResources(type));
const one = (type: string, prefix = ''): [string, any] => {
  const found = entries(type).filter(([id]) => id.startsWith(prefix));
  assert.equal(found.length, 1, `${type} ${prefix}`);
  return found[0];
};
const role = (prefix: string) => one('AWS::IAM::Role', prefix)[0];
const policy = (roleId: string): any[] => entries('AWS::IAM::Policy')
  .filter(([, resource]) => resource.Properties.Roles.some((value: any) => value.Ref === roleId))
  .flatMap(([, resource]) => resource.Properties.PolicyDocument.Statement);
const array = (value: any): any[] => Array.isArray(value) ? value : [value];
const actions = (statement: any): string[] => array(statement.Action);
const reference = (id: string) => ({ 'Fn::GetAtt': [id, 'GroupId'] });
const environment_names = (container: any): string[] => (container.Environment ?? []).map((value: any) => value.Name);

// Independent evaluation checks derived repository names, including nested paths.
function resolve(value: any, parameters: Record<string, string>): any {
  if (Array.isArray(value)) return value.map(item => resolve(item, parameters)).filter(item => item !== undefined);
  if (value === null || typeof value !== 'object') return value;
  if (value.Ref === 'AWS::NoValue') return undefined;
  if (value.Ref) return parameters[value.Ref] ?? `reference:${value.Ref}`;
  if (value['Fn::GetAtt']) return `arn:fixture:${value['Fn::GetAtt'].join(':')}`;
  if (value['Fn::Join']) return resolve(value['Fn::Join'][1], parameters).join(value['Fn::Join'][0]);
  if (value['Fn::Split']) return resolve(value['Fn::Split'][1], parameters).split(value['Fn::Split'][0]);
  if (value['Fn::Select']) return resolve(value['Fn::Select'][1], parameters)[Number(value['Fn::Select'][0])];
  if (value['Fn::Equals']) return resolve(value['Fn::Equals'][0], parameters) === resolve(value['Fn::Equals'][1], parameters);
  if (value['Fn::Not']) return !resolve(value['Fn::Not'][0], parameters);
  if (value['Fn::And']) return value['Fn::And'].every((item: any) => resolve(item, parameters));
  if (value['Fn::Or']) return value['Fn::Or'].some((item: any) => resolve(item, parameters));
  if (typeof value.Condition === 'string' && Object.keys(value).length === 1) return resolve(document.Conditions[value.Condition], parameters);
  if (value['Fn::If']) return resolve(value['Fn::If'][resolve(document.Conditions[value['Fn::If'][0]], parameters) ? 1 : 2], parameters);
  return Object.fromEntries(Object.entries(value).map(([key, child]) => [key, resolve(child, parameters)]).filter(([, child]) => child !== undefined));
}

test('managed data stays private, encrypted, retained and versioned', () => {
  template.hasResourceProperties('AWS::RDS::DBInstance', { PubliclyAccessible: false, StorageEncrypted: true, EnableIAMDatabaseAuthentication: true, DeletionProtection: true, BackupRetentionPeriod: 7, DBInstanceClass: 'db.t4g.small', AllocatedStorage: '20', MultiAZ: false, AutoMinorVersionUpgrade: false });
  // A maximum equal to allocated storage is an invalid autoscaling request.
  // Omission keeps the intended fixed 20-GiB allocation without enabling it.
  assert.equal(one('AWS::RDS::DBInstance')[1].Properties.MaxAllocatedStorage, undefined);
  template.hasResourceProperties('AWS::RDS::DBParameterGroup', { Parameters: { 'rds.force_ssl': '1' } });
  // Two tenant keys, the capability secret's key, the auth configuration's key (APP-078 B1), and the operator
  // invitation input's key (ADR-003 amendment 2026-10-30, managed invitations). The fAuth management secret is
  // FormaStack's, attached by ARN: the stack creates no copy and no key for it. Plus the composer model key's and
  // (APP-100) the Slack app's.
  template.resourceCountIs('AWS::KMS::Key', 7);
  for (const [, key] of entries('AWS::KMS::Key')) assert.equal(key.Properties.EnableKeyRotation, true);
  template.hasResourceProperties('AWS::S3::Bucket', { VersioningConfiguration: { Status: 'Enabled' }, PublicAccessBlockConfiguration: { BlockPublicAcls: true, BlockPublicPolicy: true, IgnorePublicAcls: true, RestrictPublicBuckets: true }, OwnershipControls: { Rules: [{ ObjectOwnership: 'BucketOwnerEnforced' }] } });
  for (const type of ['AWS::RDS::DBInstance', 'AWS::S3::Bucket', 'AWS::KMS::Key', 'AWS::Logs::LogGroup', 'AWS::SecretsManager::Secret']) for (const [, item] of entries(type)) {
    assert.equal(item.DeletionPolicy, 'Retain'); assert.equal(item.UpdateReplacePolicy, 'Retain');
  }
  for (const subnet of one('AWS::RDS::DBSubnetGroup')[1].Properties.SubnetIds) {
    assert.match(subnet.Ref, /databaseSubnet/);
    assert.equal(document.Resources[subnet.Ref].Properties.MapPublicIpOnLaunch, false);
  }
});

test('only HTTPS switches public/operator; ALB-to-task and private database boundaries stay exact', () => {
  const task = one('AWS::EC2::SecurityGroup', 'TaskSecurity')[0], alb = one('AWS::EC2::SecurityGroup', 'AlbSecurity')[0], database = one('AWS::EC2::SecurityGroup', 'DatabaseSecurity')[0];
  const ingress = entries('AWS::EC2::SecurityGroupIngress').map(([, item]) => item.Properties);
  for (const [id, group] of entries('AWS::EC2::SecurityGroup')) for (const rule of group.Properties.SecurityGroupIngress ?? []) ingress.push({ GroupId: reference(id), ...rule });
  assert.equal(ingress.length, 3);
  const byPort = (port: number) => ingress.find(rule => rule.FromPort === port);
  assert.deepEqual(byPort(443).GroupId, reference(alb));
  for (const mode of ['operator', 'public']) assert.equal(resolve(byPort(443).CidrIp, { IngressMode: mode, OperatorIpv4Cidr: '8.8.8.8/32' }), mode === 'public' ? '0.0.0.0/0' : '8.8.8.8/32');
  assert.deepEqual(document.Parameters.IngressMode.AllowedValues, ['operator', 'public']);
  assert.equal(document.Parameters.IngressMode.Default, 'public');
  assert.deepEqual(byPort(3000).SourceSecurityGroupId, reference(alb)); assert.deepEqual(byPort(3000).GroupId, reference(task));
  assert.deepEqual(byPort(5432).SourceSecurityGroupId, reference(task)); assert.deepEqual(byPort(5432).GroupId, reference(database));
  for (const rule of ingress) {
    assert.equal(rule.CidrIpv6, undefined); assert.equal(rule.FromPort, rule.ToPort); assert.equal(rule.IpProtocol, 'tcp');
    if (rule.FromPort !== 443) assert.equal(rule.CidrIp, undefined);
  }
  const albEgress = entries('AWS::EC2::SecurityGroupEgress').map(([, x]) => x.Properties).filter(x => JSON.stringify(x.GroupId) === JSON.stringify(reference(alb)));
  assert.equal(albEgress.length, 1); assert.equal(albEgress[0].FromPort, 3000); assert.deepEqual(albEgress[0].DestinationSecurityGroupId, reference(task));
});

test('operator and certificate are required; broad/private/placeholder networks fail', () => {
  const pattern = new RegExp(OPERATOR_CIDR_PATTERN);
  for (const value of ['8.8.8.8/32', '1.1.1.1/32', '9.9.9.8/32']) assert.equal(pattern.test(value), true, value);
  for (const value of ['0.0.0.0/0', '8.8.0.0/16', '1.1.0.0/23', '1.1.1.0/24', '9.9.9.8/30', '10.1.2.3/32', '172.16.0.1/32', '172.31.255.1/24', '192.168.1.1/32', '127.0.0.1/32', '169.254.169.254/32', '224.1.2.3/32', '203.0.113.1/32', '198.51.100.1/32', '192.0.2.1/32', '100.64.1.1/32', '198.18.0.1/32', '256.1.1.1/32', '1.1.1.1/33', '8.8.8.8/32\n', '::/0']) assert.equal(pattern.test(value), false, value);
  for (const name of ['OperatorIpv4Cidr', 'TlsCertificateArn', 'Operator', 'ImageArchitecture', 'DeploymentExpiresAtUtc']) assert.equal(document.Parameters[name].Default, undefined);
  assert.equal(document.Parameters.MonthlyBudgetUsd.Default, 100);
  // '100' is ADR-003's bounded window; '170' is the always-on beta figure Victor approved (ADR-008, 2026-10-29).
  assert.deepEqual(document.Parameters.MonthlyBudgetUsd.AllowedValues, ['100', '170']);
  const cert = new RegExp(document.Parameters.TlsCertificateArn.AllowedPattern);
  assert.equal(cert.test('arn:aws:acm:us-east-1:123456789012:certificate/00000000-0000-4000-8000-000000000001'), true);
  assert.equal(cert.test('invented-certificate'), false);
  assert.deepEqual(document.Parameters.ImageArchitecture.AllowedValues, ['ARM64', 'X86_64']);
});

test('expiry is one UTC execution that can update only this service in this cluster', () => {
  const group = one('AWS::Scheduler::ScheduleGroup')[0], schedule = one('AWS::Scheduler::Schedule')[1].Properties;
  const cluster = one('AWS::ECS::Cluster')[0], service = one('AWS::ECS::Service')[0], expiryRole = role('ExpiryRole');
  assert.equal(schedule.State, 'ENABLED'); assert.equal(schedule.ActionAfterCompletion, undefined);
  assert.equal(schedule.ScheduleExpressionTimezone, 'UTC'); assert.deepEqual(schedule.FlexibleTimeWindow, { Mode: 'OFF' });
  assert.equal(resolve(schedule.ScheduleExpression, { DeploymentExpiresAtUtc: '2026-10-30T12:34:56Z' }), 'at(2026-10-30T12:34:56)');
  assert.deepEqual(schedule.GroupName, { Ref: group });
  assert.equal(resolve(schedule.Target.Arn, { 'AWS::Partition': 'aws' }), 'arn:aws:scheduler:::aws-sdk:ecs:updateService');
  const payload = JSON.parse(resolve(schedule.Target.Input, {}));
  assert.deepEqual(Object.keys(payload).sort(), ['Cluster', 'DesiredCount', 'Service']);
  assert.equal(payload.DesiredCount, 0); assert.equal(payload.Cluster, `arn:fixture:${cluster}:Arn`); assert.equal(payload.Service, `reference:${service}`);
  assert.deepEqual(schedule.Target.RetryPolicy, { MaximumEventAgeInSeconds: 900, MaximumRetryAttempts: 3 });
  assert.deepEqual(schedule.Target.RoleArn, { 'Fn::GetAtt': [expiryRole, 'Arn'] });
  assert.deepEqual(policy(expiryRole), [{ Action: 'ecs:UpdateService', Effect: 'Allow', Resource: { Ref: service }, Condition: { ArnEquals: { 'ecs:cluster': { 'Fn::GetAtt': [cluster, 'Arn'] } } } }]);
  assert.deepEqual(document.Resources[expiryRole].Properties.AssumeRolePolicyDocument.Statement, [{ Action: 'sts:AssumeRole', Effect: 'Allow', Principal: { Service: 'scheduler.amazonaws.com' }, Condition: { StringEquals: { 'aws:SourceAccount': { Ref: 'AWS::AccountId' } }, ArnEquals: { 'aws:SourceArn': { 'Fn::GetAtt': [group, 'Arn'] } } } }]);
});

test('expiry schedule ARN output uses the actual group and schedule names without GetAtt', () => {
  const group = one('AWS::Scheduler::ScheduleGroup')[0], schedule = one('AWS::Scheduler::Schedule')[0];
  const output = document.Outputs.DeploymentExpiryScheduleArn.Value;
  assert.equal(JSON.stringify(output).includes('Fn::GetAtt'), false);
  for (const [partition, region, account, groupName, scheduleName] of [
    ['aws', 'us-east-1', '123456789012', 'generated-group_1', 'generated-schedule.2'],
    ['aws-us-gov', 'us-gov-west-1', '210987654321', 'another-group', 'another-schedule'],
  ]) {
    assert.equal(resolve(output, { 'AWS::Partition': partition, 'AWS::Region': region, 'AWS::AccountId': account, [group]: groupName, [schedule]: scheduleName }),
      `arn:${partition}:scheduler:${region}:${account}:schedule/${groupName}/${scheduleName}`);
  }
  assert.deepEqual(document.Outputs.DeploymentExpiryGroup.Value, { Ref: group });
});

test('digest-pinned Fargate service has HTTPS and no default live task, exec, NAT or endpoints', () => {
  for (const type of ['AWS::EC2::NatGateway', 'AWS::EC2::VPCEndpoint', 'AWS::EC2::Instance', 'AWS::AppRunner::Service', 'AWS::Route53::RecordSet', 'AWS::CertificateManager::Certificate']) template.resourceCountIs(type, 0);
  template.resourceCountIs('AWS::ECS::Service', 1); template.resourceCountIs('AWS::ElasticLoadBalancingV2::Listener', 1);
  assert.equal(document.Parameters.ServiceDesiredCount.Default, 0); assert.deepEqual(document.Parameters.ServiceDesiredCount.AllowedValues, ['0']);
  template.hasResourceProperties('AWS::ECS::Service', { LaunchType: 'FARGATE', DesiredCount: { Ref: 'ServiceDesiredCount' }, EnableExecuteCommand: false, AvailabilityZoneRebalancing: 'DISABLED', DeploymentConfiguration: Match.objectLike({ DeploymentCircuitBreaker: { Enable: true, Rollback: true }, MinimumHealthyPercent: 0, MaximumPercent: 100 }), NetworkConfiguration: { AwsvpcConfiguration: Match.objectLike({ AssignPublicIp: 'ENABLED' }) } });
  template.hasResourceProperties('AWS::ElasticLoadBalancingV2::Listener', { Port: 443, Protocol: 'HTTPS', Certificates: [{ CertificateArn: { Ref: 'TlsCertificateArn' } }], SslPolicy: 'ELBSecurityPolicy-TLS13-1-2-2021-06' });
  template.hasResourceProperties('AWS::ElasticLoadBalancingV2::TargetGroup', { Port: 3000, Protocol: 'HTTP', TargetType: 'ip', HealthCheckPath: '/health/ready', Matcher: { HttpCode: '200' } });
  const imagePattern = new RegExp(document.Parameters.ImageDigest.AllowedPattern);
  assert.equal(imagePattern.test('123456789012.dkr.ecr.us-east-1.amazonaws.com/team/genui@sha256:' + 'a'.repeat(64)), true);
  for (const invalid of ['123456789012.dkr.ecr.us-east-1.amazonaws.com/team/genui:latest', ...['/genui', 'team//genui', 'team/../genui', 'genui/'].map(repo => `123456789012.dkr.ecr.us-east-1.amazonaws.com/${repo}@sha256:${'a'.repeat(64)}`), `123456789012.dkr.ecr.us-east-1.amazonaws.com/genui@sha256:${'a'.repeat(64)}\n`]) assert.equal(imagePattern.test(invalid), false, invalid);
  assert.equal(document.Parameters.ImageDigest.AllowedPattern, IMAGE_DIGEST_PATTERN);
  for (const [id, task] of entries('AWS::ECS::TaskDefinition')) {
    // The optional APP-075 qualification and APP-077 operator tasks are smaller and run their own images.
    const qualification = id.startsWith('QualificationTask'), operatorTask = id.startsWith('OperatorTask');
    const small = qualification || operatorTask;
    assert.equal(task.Properties.Cpu, small ? '1024' : '2048'); assert.equal(task.Properties.Memory, small ? '2048' : '8192');
    assert.deepEqual(task.Properties.RuntimePlatform, { CpuArchitecture: { Ref: 'ImageArchitecture' }, OperatingSystemFamily: 'LINUX' });
    for (const container of task.Properties.ContainerDefinitions) {
      assert.deepEqual(container.Image, { Ref: qualification ? 'QualificationImageDigest' : operatorTask ? 'OperatorImageDigest' : 'ImageDigest' }); assert.equal(container.ReadonlyRootFilesystem, true); assert.equal(container.User, '65532:65532');
    }
  }
});

test('task identity assumes exactly four DB roles and two tenant roles; trusts bind back to it', () => {
  const taskRole = role('TaskRole'), statements = policy(taskRole);
  const allowed = [...['workspace', 'control', 'data', 'worker'].map(name => role(`DatabaseAccess${name}`)), role('TenantAccess0'), role('TenantAccess1')];
  assert.equal(statements.length, 6);
  assert.deepEqual(statements.map(statement => {
    assert.equal(statement.Action, 'sts:AssumeRole'); assert.equal(statement.Effect, 'Allow');
    return statement.Resource['Fn::GetAtt'][0];
  }).sort(), allowed.sort());
  for (const id of allowed) {
    const trust = document.Resources[id].Properties.AssumeRolePolicyDocument.Statement;
    assert.equal(trust.length, 1); assert.deepEqual(trust[0].Principal, { AWS: { 'Fn::GetAtt': [taskRole, 'Arn'] } }); assert.equal(trust[0].Action, 'sts:AssumeRole');
  }
});

test('each DB identity connects only as its fixed username on this database', () => {
  for (const name of ['workspace', 'control', 'data', 'worker']) {
    const statements = policy(role(`DatabaseAccess${name}`));
    assert.equal(statements.length, 1); assert.equal(statements[0].Action, 'rds-db:connect');
    const resource = resolve(statements[0].Resource, { 'AWS::Partition': 'aws', 'AWS::Region': 'us-east-1', 'AWS::AccountId': '123456789012' });
    assert.ok(resource.endsWith(`/genui_${name}`)); assert.ok(resource.includes(one('AWS::RDS::DBInstance')[0])); assert.equal(resource.includes('*'), false);
  }
});

test('tenant roles isolate object prefixes and keys; both explicit matching KMS headers are required', () => {
  const bucket = one('AWS::S3::Bucket')[0], bucketPolicy = one('AWS::S3::BucketPolicy')[1].Properties.PolicyDocument.Statement;
  for (const [index, keyPrefix] of ['northstarKey', 'canaryKey'].entries()) {
    const prefix = `o/00000000-0000-4000-8000-00000000000${index + 1}`, statements = policy(role(`TenantAccess${index}`));
    assert.equal(statements.length, 3);
    const objects = statements.find(statement => actions(statement).includes('s3:GetObject'));
    assert.deepEqual(actions(objects).sort(), ['s3:GetObject', 's3:GetObjectVersion', 's3:PutObject']);
    assert.deepEqual(objects.Resource, { 'Fn::Join': ['', [{ 'Fn::GetAtt': [bucket, 'Arn'] }, `/${prefix}/*`]] });
    const listing = statements.find(statement => statement.Action === 's3:ListBucket');
    assert.deepEqual(listing.Condition, { StringLike: { 's3:prefix': [prefix, `${prefix}/*`] } });
    const crypto = statements.find(statement => actions(statement).includes('kms:Decrypt'));
    assert.deepEqual(crypto.Resource, { 'Fn::GetAtt': [one('AWS::KMS::Key', keyPrefix)[0], 'Arn'] });
    for (const header of ['s3:x-amz-server-side-encryption', 's3:x-amz-server-side-encryption-aws-kms-key-id']) {
      const deny = bucketPolicy.find((statement: any) => statement.Effect === 'Deny' && statement.Action === 's3:PutObject' && statement.Condition?.StringNotEquals?.[header] && JSON.stringify(statement.Resource).includes(prefix));
      assert.ok(deny, `${prefix} ${header}`); assert.deepEqual(deny.Resource, objects.Resource);
      if (header.endsWith('key-id')) assert.deepEqual(deny.Condition.StringNotEquals[header], crypto.Resource); else assert.equal(deny.Condition.StringNotEquals[header], 'aws:kms');
    }
  }
});

test('execution roles pull exactly the selected ECR repository; no exec, pass-role or broad data grants', () => {
  for (const [prefix, parameter] of [['ExecutionRole', 'ImageDigest'], ['MigrationExecutionRole', 'ImageDigest'], ['QualificationExecutionRole', 'QualificationImageDigest']]) {
    const statements = policy(role(prefix));
    assert.equal(statements.find(statement => statement.Action === 'ecr:GetAuthorizationToken').Resource, '*');
    const pull = statements.find(statement => actions(statement).includes('ecr:BatchGetImage'));
    assert.deepEqual(actions(pull).sort(), ['ecr:BatchCheckLayerAvailability', 'ecr:BatchGetImage', 'ecr:GetDownloadUrlForLayer']);
    for (const repo of ['genui', 'team/nested/genui']) assert.equal(resolve(pull.Resource, { 'AWS::Partition': 'aws', [parameter]: `123456789012.dkr.ecr.us-east-1.amazonaws.com/${repo}@sha256:${'b'.repeat(64)}` }), `arn:aws:ecr:us-east-1:123456789012:repository/${repo}`);
  }
  // APP-077: the migration execution role also pulls exactly the operator image's repository, only when that task exists.
  const [, operatorPull] = one('AWS::IAM::Policy', 'OperatorImagePull');
  assert.equal(operatorPull.Condition, 'OperatorTaskEnabled');
  assert.deepEqual(operatorPull.Properties.Roles, [{ Ref: role('MigrationExecutionRole') }]);
  const [operatorStatement] = operatorPull.Properties.PolicyDocument.Statement;
  assert.equal(operatorPull.Properties.PolicyDocument.Statement.length, 1);
  assert.deepEqual(actions(operatorStatement).sort(), ['ecr:BatchCheckLayerAvailability', 'ecr:BatchGetImage', 'ecr:GetDownloadUrlForLayer']);
  for (const repo of ['genui-operator', 'team/nested/genui-operator']) assert.equal(resolve(operatorStatement.Resource, { 'AWS::Partition': 'aws', OperatorImageDigest: `123456789012.dkr.ecr.us-east-1.amazonaws.com/${repo}@sha256:${'e'.repeat(64)}` }), `arn:aws:ecr:us-east-1:123456789012:repository/${repo}`);
  for (const [, item] of entries('AWS::IAM::Policy')) for (const statement of item.Properties.PolicyDocument.Statement) {
    assert.equal(actions(statement).some(action => action.startsWith('ssmmessages:') || action === 'ecs:ExecuteCommand' || action === 'iam:PassRole' || action === '*'), false);
    if (statement.Resource === '*') assert.equal(statement.Action, 'ecr:GetAuthorizationToken');
  }
});

test('only migration execution role receives admin secret; migration has no tenant AWS authority', () => {
  const readers = entries('AWS::IAM::Policy').filter(([, value]) => !value.Condition && value.Properties.PolicyDocument.Statement.some((statement: any) => actions(statement).includes('secretsmanager:GetSecretValue')));
  // The migration execution role reads the database secret; the service's execution role reads only the capability secret.
  assert.deepEqual(readers.map(([, value]) => value.Properties.Roles).sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b))),
    [[{ Ref: role('ExecutionRole') }], [{ Ref: role('MigrationExecutionRole') }]]);
  const secretReads = (roleId: string) => policy(roleId).filter(statement => actions(statement).some(action => action.startsWith('secretsmanager:')));
  // The migration execution role (also the operator task's) resolves the database secret and, for the operator
  // container alone, the invitation input and (only when attached, by its own conditional policy) FormaStack's
  // management secret (ADR-003 amendment 2026-10-30, managed invitations).
  assert.deepEqual(secretReads(role('MigrationExecutionRole')).map(statement => statement.Resource), [
    { Ref: one('AWS::SecretsManager::SecretTargetAttachment')[0] },
    { Ref: one('AWS::SecretsManager::Secret', 'OperatorInvitationInput')[0] },
    { Ref: 'ProviderManagementSecretArn' },
  ]);
  assert.deepEqual(secretReads(role('ExecutionRole')).map(statement => statement.Resource), [{ Ref: one('AWS::SecretsManager::Secret', 'ProviderCapabilities')[0] }, { Ref: one('AWS::SecretsManager::Secret', 'AuthConfiguration')[0] }, { Ref: one('AWS::SecretsManager::Secret', 'ComposerModel')[0] }, { Ref: one('AWS::SecretsManager::Secret', 'SlackApp')[0] }]);
  const migration = one('AWS::ECS::TaskDefinition', 'MigrationTask')[1].Properties;
  assert.deepEqual(document.Outputs.MigrationTaskRoleArn.Value, { 'Fn::GetAtt': [role('MigrationRole'), 'Arn'] });
  assert.deepEqual(document.Outputs.MigrationExecutionRoleArn.Value, { 'Fn::GetAtt': [role('MigrationExecutionRole'), 'Arn'] });
  assert.deepEqual(migration.TaskRoleArn, { 'Fn::GetAtt': [role('MigrationRole'), 'Arn'] }); assert.equal(policy(role('MigrationRole')).length, 0);
  const container = migration.ContainerDefinitions[0];
  assert.deepEqual(container.EntryPoint, ['python3', '/app/runtime.py']); assert.deepEqual(container.Command, ['migrate']); assert.deepEqual(container.Secrets.map((value: any) => value.Name), ['PGPASSWORD']);
  const environment = Object.fromEntries(container.Environment.map((value: any) => [value.Name, value.Value]));
  assert.equal(environment.PGUSER, 'genui_migration'); assert.equal(environment.PGDATABASE, 'genui'); assert.equal(environment.PGSSLMODE, 'verify-full'); assert.equal(environment.PGSSLROOTCERT, '/etc/ssl/certs/rds-global-bundle.pem');
});

test('app environment contains identities only and one private ephemeral credential mount', () => {
  const task = one('AWS::ECS::TaskDefinition', 'Task')[1].Properties;
  assert.deepEqual(task.Volumes, [{ Name: 'runtime-config' }]);
  const container = task.ContainerDefinitions[0];
  // ADR-003 amendment 2026-10-30, APP-078 B1 and APP-093: the three secrets are the capability map, the auth
  // configuration (runtime.py serve() consumes both) and the composer model key (the Rust service reads it from
  // its environment; runtime.py passes it through).
  assert.deepEqual(container.Secrets, [
    { Name: CAPABILITY_SECRET_ENV, ValueFrom: { Ref: one('AWS::SecretsManager::Secret', 'ProviderCapabilities')[0] } },
    { Name: AUTH_CONFIGURATION_ENV, ValueFrom: { Ref: one('AWS::SecretsManager::Secret', 'AuthConfiguration')[0] } },
    { Name: COMPOSER_KEY_ENV, ValueFrom: { Ref: one('AWS::SecretsManager::Secret', 'ComposerModel')[0] } },
    // APP-100: the Slack app secret's two JSON keys, each as the one variable the service reads.
    { Name: SLACK_BOT_TOKEN_ENV, ValueFrom: { 'Fn::Join': ['', [{ Ref: one('AWS::SecretsManager::Secret', 'SlackApp')[0] }, ':bot_token::']] } },
    { Name: SLACK_SIGNING_SECRET_ENV, ValueFrom: { 'Fn::Join': ['', [{ Ref: one('AWS::SecretsManager::Secret', 'SlackApp')[0] }, ':signing_secret::']] } },
  ]);
  assert.equal(COMPOSER_KEY_ENV in environment_names(container), false, 'the key is a secret, never a plain environment entry');
  assert.deepEqual(container.MountPoints, [{ ContainerPath: '/run/genui', ReadOnly: false, SourceVolume: 'runtime-config' }]);
  assert.equal(container.EntryPoint, undefined); assert.equal(container.Command, undefined);
  const environment = Object.fromEntries(container.Environment.map((value: any) => [value.Name, value.Value]));
  // The files the service reads come from the secret alone, never from a plain environment entry.
  assert.equal(Object.keys(environment).some(name => name.startsWith('GENUI_AUTH_CONFIG') || name.startsWith('GENUI_IDENTITY_REGISTRY')), false);
  assert.equal(environment.GENUI_RUNTIME_CONFIG_FILE, '/run/genui/credentials.json'); assert.equal(environment.GENUI_RDS_CA_FILE, '/etc/ssl/certs/rds-global-bundle.pem'); assert.equal(environment.FORMA_BIND_ADDRESS, '0.0.0.0'); assert.equal(environment.FORMA_PORT, '3000');
  // D7.14: the trusted proxies are exactly the ALB's public subnets.
  assert.equal(environment.GENUI_TRUSTED_PROXY_CIDRS, '10.72.0.0/24,10.72.1.0/24');
  assert.equal(Object.keys(environment).some(name => /PASSWORD|TOKEN|CREDENTIAL$/.test(name)), false);
  const database = JSON.parse(resolve(environment.GENUI_DATABASE_ROLES_JSON, {}));
  assert.deepEqual(Object.keys(database).sort(), ['control', 'data', 'worker', 'workspace']);
  for (const [name, arn] of Object.entries(database)) assert.equal(arn, `arn:fixture:${role(`DatabaseAccess${name}`)}:Arn`);
  const tenants = JSON.parse(resolve(environment.GENUI_TENANTS_JSON, {})); assert.equal(tenants.length, 2);
  for (const tenant of tenants) {
    assert.deepEqual(Object.keys(tenant).sort(), ['kms_key_arn', 'organization_id', 'prefix', 'role_arn']); assert.equal(tenant.prefix, `o/${tenant.organization_id}/`);
  }
});

test('the retired qualification secret has no parameter, condition, read policy or injection (APP-074 review m11)', () => {
  assert.equal(document.Parameters.QualificationConfigSecretArn, undefined);
  assert.equal((document.Conditions ?? {}).QualificationConfigured, undefined);
  assert.equal(entries('AWS::IAM::Policy').some(([id]) => id.startsWith('QualificationConfigRead')), false);
  assert.equal(/QualificationConfigSecretArn|GENUI_AUTH_QUALIFICATION_/.test(JSON.stringify(document)), false);
  assert.equal(document.Parameters.ServiceDesiredCount.Default, 0);
  // The migration secret, the capability secret, the auth configuration secret (APP-078 B1) and the operator
  // invitation input (ADR-003 amendment 2026-10-30, managed invitations) only; FormaStack's management secret is
  // attached by ARN, never created here.
  // Plus (APP-100) the Slack app's credentials.
  template.resourceCountIs('AWS::SecretsManager::Secret', 6);
});

test('the capability secret is KMS-encrypted, retained, starts empty and is injected into the service task only (ADR-003 amendment 2026-10-30)', () => {
  const [secretId, secret] = one('AWS::SecretsManager::Secret', 'ProviderCapabilities');
  const [keyId, key] = one('AWS::KMS::Key', 'ProviderCapabilityKey');
  assert.deepEqual(secret.Properties.KmsKeyId, { 'Fn::GetAtt': [keyId, 'Arn'] });
  assert.equal(secret.Properties.SecretString, '{}', 'the empty capability map, never a generated password the runtime would refuse');
  assert.equal(secret.Properties.GenerateSecretString, undefined);
  for (const item of [secret, key]) { assert.equal(item.DeletionPolicy, 'Retain'); assert.equal(item.UpdateReplacePolicy, 'Retain'); }
  assert.equal(key.Properties.EnableKeyRotation, true);
  assert.deepEqual(document.Outputs.ProviderCapabilitySecretArn.Value, { Ref: secretId });
  // Injected into the service container alone; the task role never reads it.
  for (const [id, task] of entries('AWS::ECS::TaskDefinition')) {
    const body = JSON.stringify(task.Properties.ContainerDefinitions.map((container: any) => container.Secrets ?? []));
    assert.equal(body.includes(secretId), id.startsWith('Task'), id);
    assert.equal(body.includes(CAPABILITY_SECRET_ENV), id.startsWith('Task'), id);
  }
  assert.equal(policy(role('TaskRole')).some(statement => actions(statement).some(action => action.startsWith('secretsmanager:') || action.startsWith('kms:'))), false);
  // Its key decrypts for the service's execution role only through Secrets Manager.
  const decrypts = key.Properties.KeyPolicy.Statement.filter((statement: any) => statement.Principal?.AWS?.['Fn::GetAtt']);
  assert.deepEqual(decrypts.map((statement: any) => [statement.Principal.AWS['Fn::GetAtt'][0], statement.Action]), [[role('ExecutionRole'), 'kms:Decrypt']]);
  for (const statement of decrypts) assert.deepEqual(Object.keys(statement.Condition.StringEquals), ['kms:ViaService']);
  // Every IAM grant on the secret: the execution role reads it; the operator role reads and writes it, nothing else.
  const grants = entries('AWS::IAM::Policy').flatMap(([, item]) => item.Properties.PolicyDocument.Statement
    .filter((statement: any) => JSON.stringify(statement.Resource).includes(secretId))
    .map((statement: any) => [item.Properties.Roles.map((value: any) => value.Ref), actions(statement).sort(), item.Condition ?? null]));
  assert.deepEqual(grants.sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b))), [
    [[role('ExecutionRole')], ['secretsmanager:DescribeSecret', 'secretsmanager:GetSecretValue'], null],
    [[role('OperatorRole')], ['secretsmanager:GetSecretValue', 'secretsmanager:PutSecretValue'], 'OperatorTaskEnabled'],
  ].sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b))));
});

test('the auth configuration secret is KMS-encrypted, retained, starts unconfigured and is injected into the service task only (APP-078 B1)', () => {
  const [secretId, secret] = one('AWS::SecretsManager::Secret', 'AuthConfiguration');
  const [keyId, key] = one('AWS::KMS::Key', 'AuthConfigurationKey');
  assert.deepEqual(secret.Properties.KmsKeyId, { 'Fn::GetAtt': [keyId, 'Arn'] });
  assert.equal(secret.Properties.SecretString, '{}', 'not configured: runtime.py serve() starts the deny-all service, never a generated value it would refuse');
  assert.equal(secret.Properties.GenerateSecretString, undefined);
  for (const item of [secret, key]) { assert.equal(item.DeletionPolicy, 'Retain'); assert.equal(item.UpdateReplacePolicy, 'Retain'); }
  assert.equal(key.Properties.EnableKeyRotation, true);
  assert.deepEqual(document.Outputs.AuthConfigurationSecretArn.Value, { Ref: secretId });
  // Injected into the service container alone; the migration, qualification and operator tasks never see it.
  for (const [id, task] of entries('AWS::ECS::TaskDefinition')) {
    const body = JSON.stringify(task.Properties.ContainerDefinitions.map((container: any) => container.Secrets ?? []));
    assert.equal(body.includes(secretId), id.startsWith('Task'), id);
    assert.equal(body.includes(AUTH_CONFIGURATION_ENV), id.startsWith('Task'), id);
  }
  assert.equal(policy(role('TaskRole')).some(statement => actions(statement).some(action => action.startsWith('secretsmanager:') || action.startsWith('kms:'))), false);
  const decrypts = key.Properties.KeyPolicy.Statement.filter((statement: any) => statement.Principal?.AWS?.['Fn::GetAtt']);
  assert.deepEqual(decrypts.map((statement: any) => [statement.Principal.AWS['Fn::GetAtt'][0], statement.Action]), [[role('ExecutionRole'), 'kms:Decrypt']]);
  for (const statement of decrypts) assert.deepEqual(Object.keys(statement.Condition.StringEquals), ['kms:ViaService']);
  // Every IAM grant on the secret: the execution role reads it; the operator role reads and writes it, nothing else.
  const grants = entries('AWS::IAM::Policy').flatMap(([, item]) => item.Properties.PolicyDocument.Statement
    .filter((statement: any) => JSON.stringify(statement.Resource).includes(secretId))
    .map((statement: any) => [item.Properties.Roles.map((value: any) => value.Ref), actions(statement).sort(), item.Condition ?? null]));
  assert.deepEqual(grants.sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b))), [
    [[role('ExecutionRole')], ['secretsmanager:DescribeSecret', 'secretsmanager:GetSecretValue'], null],
    [[role('OperatorRole')], ['secretsmanager:GetSecretValue', 'secretsmanager:PutSecretValue'], 'OperatorTaskEnabled'],
  ].sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b))));
});

test('the invitation input secret is KMS-encrypted, retained, starts empty and reaches the operator task only (ADR-003 amendment 2026-10-30)', () => {
  const executionRole = role('ExecutionRole'), taskRole = role('TaskRole'), operatorRole = role('OperatorRole'), migrationExecution = role('MigrationExecutionRole');
  const [secretId, secret] = one('AWS::SecretsManager::Secret', 'OperatorInvitationInput');
  const [keyId, key] = one('AWS::KMS::Key', 'OperatorInvitationInputKey');
  assert.deepEqual(secret.Properties.KmsKeyId, { 'Fn::GetAtt': [keyId, 'Arn'] });
  assert.equal(secret.Properties.SecretString, '{}', 'nothing to issue, never a generated value the operator would refuse');
  assert.equal(secret.Properties.GenerateSecretString, undefined);
  for (const item of [secret, key]) { assert.equal(item.DeletionPolicy, 'Retain'); assert.equal(item.UpdateReplacePolicy, 'Retain'); }
  assert.equal(key.Properties.EnableKeyRotation, true);
  assert.deepEqual(document.Outputs.OperatorInvitationInputSecretArn.Value, { Ref: secretId });
  // Injected whole into the operator container alone: never the service, its rollback revision, the migration or the qualification task.
  for (const [id, task] of entries('AWS::ECS::TaskDefinition')) {
    const body = JSON.stringify(task.Properties.ContainerDefinitions.map((container: any) => container.Secrets ?? []));
    assert.equal(body.includes(secretId), id.startsWith('OperatorTask'), id);
    assert.equal(body.includes(INVITATION_INPUT_ENV), id.startsWith('OperatorTask'), id);
    assert.equal(JSON.stringify(task.Properties.ContainerDefinitions.map((container: any) => container.Environment ?? [])).includes(INVITATION_INPUT_ENV), false, id);
  }
  // Its key decrypts for the operator task's execution role only, through Secrets Manager.
  const decrypts = key.Properties.KeyPolicy.Statement.filter((statement: any) => statement.Principal?.AWS?.['Fn::GetAtt']);
  assert.deepEqual(decrypts.map((statement: any) => [statement.Principal.AWS['Fn::GetAtt'][0], statement.Action]), [[migrationExecution, 'kms:Decrypt']]);
  for (const statement of decrypts) assert.deepEqual(Object.keys(statement.Condition.StringEquals), ['kms:ViaService']);
  // Every IAM grant on the secret: the operator task's execution role resolves it at task start; no task role,
  // not the service's execution role and not the operator role itself reads or writes it.
  const grants = entries('AWS::IAM::Policy').flatMap(([, item]) => item.Properties.PolicyDocument.Statement
    .filter((statement: any) => JSON.stringify(statement.Resource).includes(secretId))
    .map((statement: any) => [item.Properties.Roles.map((value: any) => value.Ref), actions(statement).sort(), item.Condition ?? null]));
  assert.deepEqual(grants, [[[migrationExecution], ['secretsmanager:DescribeSecret', 'secretsmanager:GetSecretValue'], null]]);
  for (const roleId of [executionRole, taskRole, operatorRole]) {
    assert.equal(policy(roleId).some(statement => JSON.stringify(statement.Resource).includes(secretId) || JSON.stringify(statement.Resource).includes(keyId)), false, roleId);
  }
  // The service's execution role still resolves exactly its three secrets, none of the operator's.
  const serviceReads = policy(executionRole).filter(statement => actions(statement).some(action => action.startsWith('secretsmanager:'))).map(statement => statement.Resource);
  assert.deepEqual(serviceReads, [{ Ref: one('AWS::SecretsManager::Secret', 'ProviderCapabilities')[0] }, { Ref: one('AWS::SecretsManager::Secret', 'AuthConfiguration')[0] }, { Ref: one('AWS::SecretsManager::Secret', 'ComposerModel')[0] }, { Ref: one('AWS::SecretsManager::Secret', 'SlackApp')[0] }]);
});

test('the Slack app secret is KMS-encrypted, retained, starts as no app and is injected into the service task only as the two Slack variables (APP-100, ADR-011)', () => {
  const [secretId, secret] = one('AWS::SecretsManager::Secret', 'SlackApp');
  const [keyId, key] = one('AWS::KMS::Key', 'SlackAppKey');
  assert.deepEqual(secret.Properties.KmsKeyId, { 'Fn::GetAtt': [keyId, 'Arn'] });
  assert.equal(secret.Properties.SecretString, SLACK_APP_PLACEHOLDER);
  assert.deepEqual(JSON.parse(SLACK_APP_PLACEHOLDER), { bot_token: '', signing_secret: '' }, 'both keys present and empty: no Slack app, and ECS can resolve both keys');
  assert.equal(secret.Properties.GenerateSecretString, undefined);
  for (const item of [secret, key]) { assert.equal(item.DeletionPolicy, 'Retain'); assert.equal(item.UpdateReplacePolicy, 'Retain'); }
  assert.equal(key.Properties.EnableKeyRotation, true);
  assert.deepEqual(document.Outputs.SlackAppSecretArn.Value, { Ref: secretId });
  for (const [id, task] of entries('AWS::ECS::TaskDefinition')) {
    const body = JSON.stringify(task.Properties.ContainerDefinitions.map((container: any) => container.Secrets ?? []));
    assert.equal(body.includes(secretId), id.startsWith('Task'), id);
    for (const name of [SLACK_BOT_TOKEN_ENV, SLACK_SIGNING_SECRET_ENV]) {
      assert.equal(body.includes(name), id.startsWith('Task'), id);
      for (const container of task.Properties.ContainerDefinitions) assert.equal(environment_names(container).includes(name), false, id);
    }
  }
  assert.equal(policy(role('TaskRole')).some(statement => actions(statement).some(action => action.startsWith('secretsmanager:') || action.startsWith('kms:'))), false);
  const decrypts = key.Properties.KeyPolicy.Statement.filter((statement: any) => statement.Principal?.AWS?.['Fn::GetAtt']);
  assert.deepEqual(decrypts.map((statement: any) => [statement.Principal.AWS['Fn::GetAtt'][0], statement.Action]), [[role('ExecutionRole'), 'kms:Decrypt']]);
  for (const statement of decrypts) assert.deepEqual(Object.keys(statement.Condition.StringEquals), ['kms:ViaService']);
  // Every IAM grant on the secret: the execution role reads it; the operator role reads and writes it, nothing else.
  const grants = entries('AWS::IAM::Policy').flatMap(([, item]) => item.Properties.PolicyDocument.Statement
    .filter((statement: any) => JSON.stringify(statement.Resource).includes(secretId))
    .map((statement: any) => [item.Properties.Roles.map((value: any) => value.Ref), actions(statement).sort(), item.Condition ?? null]));
  assert.deepEqual(grants.sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b))), [
    [[role('ExecutionRole')], ['secretsmanager:DescribeSecret', 'secretsmanager:GetSecretValue'], null],
    [[role('OperatorRole')], ['secretsmanager:GetSecretValue', 'secretsmanager:PutSecretValue'], 'OperatorTaskEnabled'],
  ].sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b))));
});

test('the composer model key secret is KMS-encrypted, retained, starts as no key and is injected into the service task only (APP-093, ADR-011)', () => {
  const [secretId, secret] = one('AWS::SecretsManager::Secret', 'ComposerModel');
  const [keyId, key] = one('AWS::KMS::Key', 'ComposerModelKey');
  assert.deepEqual(secret.Properties.KmsKeyId, { 'Fn::GetAtt': [keyId, 'Arn'] });
  assert.equal(secret.Properties.SecretString, '{}', 'no key: the service treats the placeholder as disabled (composer_unavailable), never as a key');
  assert.equal(secret.Properties.GenerateSecretString, undefined);
  for (const item of [secret, key]) { assert.equal(item.DeletionPolicy, 'Retain'); assert.equal(item.UpdateReplacePolicy, 'Retain'); }
  assert.equal(key.Properties.EnableKeyRotation, true);
  assert.deepEqual(document.Outputs.ComposerModelSecretArn.Value, { Ref: secretId });
  // Injected into the service container alone, as the plain variable the service reads; the rollback,
  // migration, qualification and operator tasks never see it.
  for (const [id, task] of entries('AWS::ECS::TaskDefinition')) {
    const body = JSON.stringify(task.Properties.ContainerDefinitions.map((container: any) => container.Secrets ?? []));
    assert.equal(body.includes(secretId), id.startsWith('Task'), id);
    assert.equal(body.includes(COMPOSER_KEY_ENV), id.startsWith('Task'), id);
    for (const container of task.Properties.ContainerDefinitions) assert.equal(environment_names(container).includes(COMPOSER_KEY_ENV), false, id);
  }
  assert.equal(policy(role('TaskRole')).some(statement => actions(statement).some(action => action.startsWith('secretsmanager:') || action.startsWith('kms:'))), false);
  const decrypts = key.Properties.KeyPolicy.Statement.filter((statement: any) => statement.Principal?.AWS?.['Fn::GetAtt']);
  assert.deepEqual(decrypts.map((statement: any) => [statement.Principal.AWS['Fn::GetAtt'][0], statement.Action]), [[role('ExecutionRole'), 'kms:Decrypt']]);
  for (const statement of decrypts) assert.deepEqual(Object.keys(statement.Condition.StringEquals), ['kms:ViaService']);
  // Every IAM grant on the secret: the execution role reads it; the operator role reads and writes it, nothing else.
  const grants = entries('AWS::IAM::Policy').flatMap(([, item]) => item.Properties.PolicyDocument.Statement
    .filter((statement: any) => JSON.stringify(statement.Resource).includes(secretId))
    .map((statement: any) => [item.Properties.Roles.map((value: any) => value.Ref), actions(statement).sort(), item.Condition ?? null]));
  assert.deepEqual(grants.sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b))), [
    [[role('ExecutionRole')], ['secretsmanager:DescribeSecret', 'secretsmanager:GetSecretValue'], null],
    [[role('OperatorRole')], ['secretsmanager:GetSecretValue', 'secretsmanager:PutSecretValue'], 'OperatorTaskEnabled'],
  ].sort((a, b) => JSON.stringify(a).localeCompare(JSON.stringify(b))));
});

test('FormaStack\'s management secret is attached by ARN to the operator task only, never copied (ADR-003 amendment 2026-10-30, A12)', () => {
  const NORTHSTAR_SECRET = 'arn:aws:secretsmanager:us-east-1:877573132739:secret:fauth-beta/management/northstar-jAkFuz';
  const URL = 'https://auth-staging.getforma.dev';
  const digest = `877573132739.dkr.ecr.us-east-1.amazonaws.com/genui-operator@sha256:${'d'.repeat(64)}`;
  // The parameters: empty by default (not attached), ARN-shaped or an https origin otherwise.
  const parameter = (name: string) => document.Parameters[name];
  assert.deepEqual([parameter('ProviderManagementSecretArn').Default, parameter('ProviderManagementSecretArn').AllowedPattern], ['', PROVIDER_MANAGEMENT_SECRET_ARN_PATTERN]);
  assert.deepEqual([parameter('ProviderManagementUrl').Default, parameter('ProviderManagementUrl').AllowedPattern], ['', PROVIDER_MANAGEMENT_URL_PATTERN]);
  assert.equal(parameter('ProviderManagementSecretArn').NoEcho, undefined, 'an identifier: preflight compares it in describe-stacks');
  const arnPattern = new RegExp(PROVIDER_MANAGEMENT_SECRET_ARN_PATTERN), urlPattern = new RegExp(PROVIDER_MANAGEMENT_URL_PATTERN);
  for (const good of ['', NORTHSTAR_SECRET]) assert.equal(arnPattern.test(good), true, good);
  for (const bad of ['fauth-beta/management/northstar', 'arn:aws:secretsmanager:us-east-1:877573132739:secret:fauth-beta/management/northstar',
    `${NORTHSTAR_SECRET}:management_key::`, `${NORTHSTAR_SECRET}\n`, 'arn:aws:s3:::bucket', 'arn:aws:secretsmanager:us-east-1:87757313273:secret:x-AbCdEf']) {
    assert.equal(arnPattern.test(bad), false, bad);
  }
  for (const good of ['', URL, 'https://auth.example.test:8443']) assert.equal(urlPattern.test(good), true, good);
  for (const bad of ['http://auth-staging.getforma.dev', `${URL}/`, `${URL}/manage`, 'https://user@auth.example.test', `${URL}\n`]) assert.equal(urlPattern.test(bad), false, bad);
  // Both or neither (a template rule, checked by CloudFormation before any change).
  const rule = document.Rules.ProviderManagementComplete.Assertions[0].Assert;
  for (const [arn, url, ok] of [['', '', true], [NORTHSTAR_SECRET, URL, true], [NORTHSTAR_SECRET, '', false], ['', URL, false]] as const) {
    assert.equal(resolve(rule, { ProviderManagementSecretArn: arn, ProviderManagementUrl: url }), ok, `${arn} ${url}`);
  }
  // No copy: no management secret, key or output in the stack.
  assert.equal(entries('AWS::SecretsManager::Secret').some(([id]) => id.startsWith('ProviderManagementKey')), false);
  assert.equal(entries('AWS::KMS::Key').some(([id]) => id.startsWith('ProviderManagementKey')), false);
  assert.equal(Object.keys(document.Outputs).some(name => name.startsWith('ProviderManagement')), false);
  assert.equal(JSON.stringify(document).includes('GENUI_PROVIDER_MANAGEMENT_JSON'), false);
  // The operator container: empty parameter -> exactly the unattached lists; set -> the two JSON-key injections and the URL.
  const operator = one('AWS::ECS::TaskDefinition', 'OperatorTask')[1];
  const lists = (parameters: Record<string, string>) => {
    const [container] = resolve(operator.Properties.ContainerDefinitions, parameters);
    return { secrets: container.Secrets.map((entry: any) => [entry.Name, entry.ValueFrom]), environment: Object.fromEntries(container.Environment.map((entry: any) => [entry.Name, entry.Value])) };
  };
  const off = lists({ OperatorImageDigest: digest, ProviderManagementSecretArn: '', ProviderManagementUrl: '' });
  assert.deepEqual(off.secrets.map(([name]: [string]) => name), ['PGPASSWORD', INVITATION_INPUT_ENV]);
  for (const name of [PROVIDER_MANAGEMENT_URL_ENV, PROVIDER_MANAGEMENT_KEY_ENV, PROVIDER_MANAGEMENT_TENANT_ENV]) assert.equal(name in off.environment, false, name);
  const on = lists({ OperatorImageDigest: digest, ProviderManagementSecretArn: NORTHSTAR_SECRET, ProviderManagementUrl: URL });
  assert.deepEqual(on.secrets.slice(2), [[PROVIDER_MANAGEMENT_KEY_ENV, `${NORTHSTAR_SECRET}:management_key::`], [PROVIDER_MANAGEMENT_TENANT_ENV, `${NORTHSTAR_SECRET}:tenant_id::`]]);
  assert.deepEqual(on.secrets.slice(0, 2), off.secrets);
  assert.equal(on.environment[PROVIDER_MANAGEMENT_URL_ENV], URL);
  assert.deepEqual(Object.keys(on.environment).filter(name => name !== PROVIDER_MANAGEMENT_URL_ENV), Object.keys(off.environment));
  // The secret ARN and its injected names appear in no other task definition (service, rollback, migration, qualification).
  for (const [id, task] of entries('AWS::ECS::TaskDefinition')) {
    const body = JSON.stringify(task.Properties);
    for (const needle of ['ProviderManagementSecretArn', 'ProviderManagementUrl', PROVIDER_MANAGEMENT_KEY_ENV, PROVIDER_MANAGEMENT_TENANT_ENV, PROVIDER_MANAGEMENT_URL_ENV]) {
      assert.equal(body.includes(needle), id.startsWith('OperatorTask'), `${needle} ${id}`);
    }
  }
  // The only grant: GetSecretValue on exactly that ARN, to the operator task's execution role, only when the
  // operator task exists and the secret is attached. No KMS statement (AWS-managed key), no other role.
  const readers = entries('AWS::IAM::Policy').filter(([, item]) => JSON.stringify(item.Properties.PolicyDocument).includes('ProviderManagementSecretArn'));
  assert.equal(readers.length, 1);
  const [readerId, reader] = readers[0];
  assert.match(readerId, /^OperatorProviderManagementRead/);
  assert.deepEqual(reader.Properties.Roles, [{ Ref: role('MigrationExecutionRole') }]);
  assert.deepEqual(reader.Properties.PolicyDocument.Statement, [{ Effect: 'Allow', Action: 'secretsmanager:GetSecretValue', Resource: { Ref: 'ProviderManagementSecretArn' } }]);
  for (const [image, arn, enabled] of [['', '', false], ['', NORTHSTAR_SECRET, false], [digest, '', false], [digest, NORTHSTAR_SECRET, true]] as const) {
    assert.equal(resolve({ Condition: reader.Condition }, { OperatorImageDigest: image, ProviderManagementSecretArn: arn }), enabled, `${image} ${arn}`);
  }
  for (const [, item] of entries('AWS::IAM::Policy')) for (const statement of item.Properties.PolicyDocument.Statement) {
    if (actions(statement).some(action => action.startsWith('kms:'))) assert.equal(JSON.stringify(statement).includes('ProviderManagement'), false);
  }
});

test('the rollback revision runs the same image with no secret injected, so it starts deny-all (APP-076 G11)', () => {
  const [serviceTaskId, serviceTask] = one('AWS::ECS::TaskDefinition', 'Task');
  const [rollbackId, rollbackTask] = one('AWS::ECS::TaskDefinition', 'RollbackTask');
  assert.notEqual(rollbackId, serviceTaskId);
  assert.deepEqual(document.Outputs.RollbackTaskDefinition.Value, { Ref: rollbackId });
  assert.equal(rollbackTask.Condition, undefined);
  const live = serviceTask.Properties, fallback = rollbackTask.Properties;
  for (const field of ['Cpu', 'Memory', 'NetworkMode', 'RequiresCompatibilities', 'RuntimePlatform', 'TaskRoleArn', 'ExecutionRoleArn', 'Volumes']) {
    assert.deepEqual(fallback[field], live[field], field);
  }
  assert.equal(fallback.ContainerDefinitions.length, 1); assert.equal(live.ContainerDefinitions.length, 1);
  const [container] = fallback.ContainerDefinitions, [current] = live.ContainerDefinitions;
  // Identical container except the secrets (none) and the log stream prefix.
  for (const field of ['Name', 'Image', 'ReadonlyRootFilesystem', 'User', 'Environment', 'MountPoints', 'PortMappings', 'HealthCheck', 'Essential', 'EntryPoint', 'Command']) {
    assert.deepEqual(container[field], current[field], field);
  }
  assert.equal(container.Secrets, undefined);
  assert.equal(current.Secrets.length, 5);
  assert.equal(container.LogConfiguration.Options['awslogs-stream-prefix'], 'genui-rollback');
  const environment = Object.fromEntries(container.Environment.map((value: any) => [value.Name, value.Value]));
  assert.equal(Object.keys(environment).some(name => name.startsWith('GENUI_AUTH_CONFIG') || name.startsWith('GENUI_IDENTITY_REGISTRY') || name.startsWith('GENUI_PROVIDER_CAPABILIT')), false);
  // The service never deploys it by itself; the operator's update-service does (runbook §8).
  for (const [, service] of entries('AWS::ECS::Service')) assert.deepEqual(service.Properties.TaskDefinition, { Ref: serviceTaskId });
  assert.equal(JSON.stringify(document.Resources).split(`"${rollbackId}"`).length - 1, 1);
});

test('the load balancer appends the client address without a port (APP-074 review m3)', () => {
  const [, alb] = one('AWS::ElasticLoadBalancingV2::LoadBalancer');
  const attributes = Object.fromEntries(alb.Properties.LoadBalancerAttributes.map((item: any) => [item.Key, item.Value]));
  assert.equal(attributes['routing.http.xff_header_processing.mode'], 'append');
  assert.equal(attributes['routing.http.xff_client_port.enabled'], 'false');
  assert.equal(attributes['routing.http.drop_invalid_header_fields.enabled'], 'true');
});

test('the load balancer idles longer than the composer route and the live stream subscribes at /v1/events (ADR-011 APP-093/095/096)', () => {
  const [, alb] = one('AWS::ElasticLoadBalancingV2::LoadBalancer');
  const attributes = Object.fromEntries(alb.Properties.LoadBalancerAttributes.map((item: any) => [item.Key, item.Value]));
  // POST /v1/compose runs up to 120 s; SSE heartbeats every 15 s keep the stream inside the timeout.
  assert.equal(ALB_IDLE_TIMEOUT_SECONDS >= COMPOSE_DEADLINE_SECONDS + 10, true);
  assert.equal(attributes['idle_timeout.timeout_seconds'], String(ALB_IDLE_TIMEOUT_SECONDS));
  assert.equal(Number(attributes['idle_timeout.timeout_seconds']) >= 130, true);
  const [, task] = one('AWS::ECS::TaskDefinition', 'Task');
  const [, rollbackTask] = one('AWS::ECS::TaskDefinition', 'RollbackTask');
  for (const definition of [task, rollbackTask]) {
    const environment = Object.fromEntries(definition.Properties.ContainerDefinitions[0].Environment.map((value: any) => [value.Name, value.Value]));
    assert.equal(environment[CANVAS_EVENTS_URL_ENV], CANVAS_EVENTS_URL);
    assert.equal(CANVAS_EVENTS_URL.startsWith('/v1/'), true, 'main.rs accepts a /v1/ path only');
  }
  // The other task definitions (migration, operator, qualification) do not run the canvas.
  for (const [id, definition] of entries('AWS::ECS::TaskDefinition')) {
    if (id === one('AWS::ECS::TaskDefinition', 'Task')[0] || id === one('AWS::ECS::TaskDefinition', 'RollbackTask')[0]) continue;
    const environment = (definition.Properties.ContainerDefinitions[0].Environment ?? []).map((value: any) => value.Name);
    assert.equal(environment.includes(CANVAS_EVENTS_URL_ENV), false, id);
    assert.equal(environment.includes(DEMO_ORGANIZATIONS_ENV), false, id);
    assert.equal(environment.includes(DEMO_MODE_ENV), false, id);
  }
});

test('demo mode names the synthetic canary organization only, never the beta organization (ADR-011 APP-098)', () => {
  const [, task] = one('AWS::ECS::TaskDefinition', 'Task');
  const environment = Object.fromEntries(task.Properties.ContainerDefinitions[0].Environment.map((value: any) => [value.Name, value.Value]));
  assert.equal(environment[DEMO_ORGANIZATIONS_ENV], DEMO_ORGANIZATION_ID);
  assert.equal(environment[DEMO_MODE_ENV], '1');
  const listed = environment[DEMO_ORGANIZATIONS_ENV].split(',');
  assert.deepEqual(listed, ['00000000-0000-4000-8000-000000000002']);
  assert.equal(listed.includes('00000000-0000-4000-8000-000000000001'), false, 'the beta organization behaves exactly as without demo mode');
  for (const id of listed) assert.match(id, /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/, 'lowercase UUIDs, as the service parses them');
  // The demo organization is a configured tenant (role, key, prefix), so its library sources are reachable.
  const tenants = JSON.parse(resolve(environment.GENUI_TENANTS_JSON, {}));
  assert.equal(tenants.some((tenant: any) => tenant.organization_id === DEMO_ORGANIZATION_ID), true);
});

test('qualification image parameter accepts only empty or a reviewed digest', () => {
  const parameter = document.Parameters.QualificationImageDigest;
  assert.equal(parameter.Default, ''); assert.equal(parameter.AllowedPattern, QUALIFICATION_IMAGE_PATTERN);
  const pattern = new RegExp(parameter.AllowedPattern);
  const digest = `877573132739.dkr.ecr.us-east-1.amazonaws.com/genui-synthetic@sha256:${'c'.repeat(64)}`;
  for (const valid of ['', digest]) assert.equal(pattern.test(valid), true, valid);
  for (const invalid of [' ', '\n', digest + '\n', digest.replace(/@sha256:.*/, ':qualification'), digest.replace('c'.repeat(64), 'C'.repeat(64)), digest.replace('c'.repeat(64), 'c'.repeat(63))]) assert.equal(pattern.test(invalid), false, invalid);
});

test('qualification task is absent by default; when supplied it is a count-zero standalone task on the runtime role only', () => {
  const [taskId, task] = one('AWS::ECS::TaskDefinition', 'QualificationTask');
  const executionId = role('QualificationExecutionRole');
  const [policyId, executionPolicy] = one('AWS::IAM::Policy', 'QualificationExecutionRoleDefaultPolicy');
  // Every qualification resource and output exists only under the condition.
  for (const resource of [task, document.Resources[executionId], executionPolicy]) assert.equal(resource.Condition, 'QualificationTaskEnabled');
  assert.equal(document.Outputs.QualificationTaskDefinition.Condition, 'QualificationTaskEnabled');
  const condition = document.Conditions.QualificationTaskEnabled;
  assert.equal(resolve(condition, { QualificationImageDigest: '' }), false);
  assert.equal(resolve(condition, { QualificationImageDigest: `877573132739.dkr.ecr.us-east-1.amazonaws.com/genui@sha256:${'d'.repeat(64)}` }), true);
  for (const [id, resource] of Object.entries(document.Resources) as Array<[string, any]>) {
    const body = JSON.stringify(resource.Properties ?? {});
    if ([taskId, executionId, policyId].some(name => body.includes(`"${name}"`)) || body.includes('QualificationImageDigest')) assert.equal(resource.Condition, 'QualificationTaskEnabled', id);
    assert.equal(array(resource.DependsOn ?? []).some((name: string) => [taskId, executionId, policyId].includes(name)), false, id);
  }
  // Count zero: no service, schedule or target references it.
  for (const [, service] of entries('AWS::ECS::Service')) assert.deepEqual(service.Properties.TaskDefinition, { Ref: one('AWS::ECS::TaskDefinition', 'Task')[0] });
  assert.equal(JSON.stringify(document.Resources).split(`"${taskId}"`).length - 1, 1); // its own key only: no resource references it
  // Runtime task role only: the four DB identities and two tenant roles; no migration identity or secret.
  assert.deepEqual(task.Properties.TaskRoleArn, { 'Fn::GetAtt': [role('TaskRole'), 'Arn'] });
  assert.deepEqual(task.Properties.ExecutionRoleArn, { 'Fn::GetAtt': [executionId, 'Arn'] });
  const statements = policy(executionId);
  assert.deepEqual(statements.flatMap(actions).sort(), ['ecr:BatchCheckLayerAvailability', 'ecr:BatchGetImage', 'ecr:GetAuthorizationToken', 'ecr:GetDownloadUrlForLayer', 'logs:CreateLogStream', 'logs:PutLogEvents']);
  assert.equal(JSON.stringify(task).includes('MigrationSecret'), false);
  assert.equal(JSON.stringify(task).includes(role('MigrationRole')), false);
  const [container] = task.Properties.ContainerDefinitions;
  assert.equal(task.Properties.ContainerDefinitions.length, 1);
  assert.deepEqual(container.EntryPoint, ['python3', '/app/qualification_supervisor.py']); assert.deepEqual(container.Command, ['run']);
  assert.equal(container.Secrets, undefined); assert.equal(container.PortMappings, undefined); assert.equal(container.HealthCheck, undefined);
  assert.deepEqual(container.MountPoints, [{ ContainerPath: '/run/genui', ReadOnly: false, SourceVolume: 'qualification-run' }]);
  assert.deepEqual(task.Properties.Volumes, [{ Name: 'qualification-run' }]);
  const environment = Object.fromEntries(container.Environment.map((value: any) => [value.Name, value.Value]));
  assert.deepEqual(Object.keys(environment).sort(), ['AWS_REGION', 'GENUI_DATABASE_HOST', 'GENUI_DATABASE_NAME', 'GENUI_DATABASE_PORT', 'GENUI_DATABASE_ROLES_JSON', 'GENUI_EVIDENCE_BUCKET', 'GENUI_TENANTS_JSON', 'PYTHONDONTWRITEBYTECODE']);
  // Arming value and run manifest arrive only as per-run overrides.
  assert.equal(Object.keys(environment).some(name => name.startsWith('PG') || /PASSWORD|TOKEN|SECRET|APP008|MANIFEST/.test(name)), false);
  assert.deepEqual(environment.GENUI_DATABASE_HOST, { 'Fn::GetAtt': [one('AWS::RDS::DBInstance')[0], 'Endpoint.Address'] });
  assert.equal(container.LogConfiguration.Options['awslogs-stream-prefix'], 'genui-qualification');
  // Its execution role reads no secret, even when the rehearsal secret is enabled.
  for (const [, item] of entries('AWS::IAM::Policy')) {
    if (item.Properties.PolicyDocument.Statement.some((statement: any) => actions(statement).includes('secretsmanager:GetSecretValue'))) {
      assert.equal(item.Properties.Roles.some((value: any) => value.Ref === executionId), false);
    }
  }
});

// APP-077: the operator package's optional task. Its entry point and documented
// command overrides come from app/operator/overrides.json, which the operator's
// own tests and the image check bind to operator.py and the image.
const overrides = JSON.parse(readFileSync(new URL('../../app/operator/overrides.json', import.meta.url), 'utf8'));

test('operator image parameter accepts only empty or a reviewed digest', () => {
  const parameter = document.Parameters.OperatorImageDigest;
  assert.equal(parameter.Default, ''); assert.equal(parameter.AllowedPattern, OPERATOR_IMAGE_PATTERN);
  const pattern = new RegExp(parameter.AllowedPattern);
  const digest = `877573132739.dkr.ecr.us-east-1.amazonaws.com/genui-operator@sha256:${'f'.repeat(64)}`;
  for (const valid of ['', digest]) assert.equal(pattern.test(valid), true, valid);
  for (const invalid of [' ', '\n', digest + '\n', digest.replace(/@sha256:.*/, ':operator'), digest.replace('f'.repeat(64), 'F'.repeat(64)), digest.replace('f'.repeat(64), 'f'.repeat(63))]) assert.equal(pattern.test(invalid), false, invalid);
});

test('operator task is absent by default; when supplied it is a count-zero standalone task with the migration channel only', () => {
  const [taskId, task] = one('AWS::ECS::TaskDefinition', 'OperatorTask');
  const operatorRole = role('OperatorRole');
  const [policyId, operatorPolicy] = one('AWS::IAM::Policy', 'OperatorRoleDefaultPolicy');
  const [pullId] = one('AWS::IAM::Policy', 'OperatorImagePull');
  const owned = [taskId, operatorRole, policyId, pullId];
  for (const resource of [task, document.Resources[operatorRole], operatorPolicy, document.Resources[pullId]]) assert.equal(resource.Condition, 'OperatorTaskEnabled');
  for (const output of ['OperatorTaskDefinition', 'OperatorTaskRoleArn']) assert.equal(document.Outputs[output].Condition, 'OperatorTaskEnabled');
  const condition = document.Conditions.OperatorTaskEnabled;
  assert.equal(resolve(condition, { OperatorImageDigest: '' }), false);
  assert.equal(resolve(condition, { OperatorImageDigest: `877573132739.dkr.ecr.us-east-1.amazonaws.com/genui-operator@sha256:${'d'.repeat(64)}` }), true);
  for (const [id, resource] of Object.entries(document.Resources) as Array<[string, any]>) {
    const body = JSON.stringify(resource.Properties ?? {});
    if (owned.some(name => body.includes(`"${name}"`)) || body.includes('OperatorImageDigest')) assert.equal(resource.Condition, 'OperatorTaskEnabled', id);
    assert.equal(array(resource.DependsOn ?? []).some((name: string) => owned.includes(name)), false, id);
  }
  // Count zero: no service, schedule or target references it.
  for (const [, service] of entries('AWS::ECS::Service')) assert.deepEqual(service.Properties.TaskDefinition, { Ref: one('AWS::ECS::TaskDefinition', 'Task')[0] });
  assert.equal(JSON.stringify(document.Resources).split(`"${taskId}"`).length - 1, 1);
  // The migration execution role (its PGPASSWORD secret) and the operator's own task role.
  assert.deepEqual(task.Properties.ExecutionRoleArn, { 'Fn::GetAtt': [role('MigrationExecutionRole'), 'Arn'] });
  assert.deepEqual(task.Properties.TaskRoleArn, { 'Fn::GetAtt': [operatorRole, 'Arn'] });
  assert.deepEqual(document.Resources[operatorRole].Properties.AssumeRolePolicyDocument.Statement, [{ Action: 'sts:AssumeRole', Effect: 'Allow', Principal: { Service: 'ecs-tasks.amazonaws.com' } }]);
  const [container] = task.Properties.ContainerDefinitions;
  assert.equal(task.Properties.ContainerDefinitions.length, 1);
  assert.equal(container.Name, overrides.container);
  assert.deepEqual(container.EntryPoint, overrides.entryPoint); assert.deepEqual(container.Command, overrides.default_command);
  // PGPASSWORD and the invitation input, whole, plus (only when attached) the management secret's two JSON keys
  // (ADR-003 amendment 2026-10-30, managed invitations; asserted in detail by the attached-secret test).
  assert.deepEqual(container.Secrets.slice(0, 2), [
    { Name: 'PGPASSWORD', ValueFrom: { 'Fn::Join': ['', [{ Ref: one('AWS::SecretsManager::SecretTargetAttachment')[0] }, ':password::']] } },
    { Name: INVITATION_INPUT_ENV, ValueFrom: { Ref: one('AWS::SecretsManager::Secret', 'OperatorInvitationInput')[0] } },
  ]);
  assert.deepEqual(container.Secrets.slice(2).map((entry: any) => [entry['Fn::If'][0], entry['Fn::If'][1].Name, entry['Fn::If'][2]]),
    [['ProviderManagementAttached', PROVIDER_MANAGEMENT_KEY_ENV, { Ref: 'AWS::NoValue' }], ['ProviderManagementAttached', PROVIDER_MANAGEMENT_TENANT_ENV, { Ref: 'AWS::NoValue' }]]);
  assert.equal(container.PortMappings, undefined); assert.equal(container.HealthCheck, undefined);
  assert.deepEqual(container.MountPoints, [{ ContainerPath: '/run/operator', ReadOnly: false, SourceVolume: 'operator-scratch' }]);
  assert.deepEqual(task.Properties.Volumes, [{ Name: 'operator-scratch' }]);
  assert.equal(container.LogConfiguration.Options['awslogs-stream-prefix'], 'genui-operator');
  // The attached-only URL entry is the conditional last one (asserted by the attached-secret test).
  const environment = Object.fromEntries(container.Environment.filter((value: any) => value.Name).map((value: any) => [value.Name, value.Value]));
  assert.equal(container.Environment.length, Object.keys(environment).length + 1);
  assert.deepEqual(Object.keys(environment).sort(), ['AWS_REGION', AUTH_CONFIGURATION_SECRET_ID_ENV, 'GENUI_EVIDENCE_BUCKET', 'GENUI_OPERATOR_TENANTS_JSON', CAPABILITY_SECRET_ID_ENV, 'PGDATABASE', 'PGHOST', 'PGPORT', 'PGSSLMODE', 'PGSSLROOTCERT', 'PGUSER'].sort());
  // ADR-003 amendment 2026-10-30: the capability secret's identifier (never its value) for `revoke`;
  // B1 review m5: the auth configuration secret's identifier for `bind-provider-tenant --from-auth-configuration`.
  assert.deepEqual(environment[CAPABILITY_SECRET_ID_ENV], { Ref: one('AWS::SecretsManager::Secret', 'ProviderCapabilities')[0] });
  assert.deepEqual(environment[AUTH_CONFIGURATION_SECRET_ID_ENV], { Ref: one('AWS::SecretsManager::Secret', 'AuthConfiguration')[0] });
  assert.equal(container.Secrets.some((entry: any) => entry.Name === CAPABILITY_SECRET_ENV), false);
  assert.equal(environment.PGUSER, 'genui_migration'); assert.equal(environment.PGDATABASE, 'genui'); assert.equal(environment.PGSSLMODE, 'verify-full'); assert.equal(environment.PGSSLROOTCERT, '/etc/ssl/certs/rds-global-bundle.pem');
  assert.deepEqual(environment.PGHOST, { 'Fn::GetAtt': [one('AWS::RDS::DBInstance')[0], 'Endpoint.Address'] });
  // Tenant keys and prefixes only: no role ARN to assume, no application database identity.
  const tenants = JSON.parse(resolve(environment.GENUI_OPERATOR_TENANTS_JSON, {}));
  assert.deepEqual(tenants.map((tenant: any) => Object.keys(tenant).sort()), [['kms_key_arn', 'organization_id', 'prefix'], ['kms_key_arn', 'organization_id', 'prefix']]);
  for (const [index, tenant] of tenants.entries()) {
    assert.equal(tenant.prefix, `o/${tenant.organization_id}/`);
    assert.equal(tenant.kms_key_arn, `arn:fixture:${one('AWS::KMS::Key', ['northstarKey', 'canaryKey'][index])[0]}:Arn`);
  }
  assert.equal(/DatabaseAccess|TenantAccess|GENUI_DATABASE_ROLES_JSON|GENUI_TENANTS_JSON/.test(JSON.stringify(container)), false);
});

test('operator task role holds no role assumption and reaches only B1 source objects under each tenant key', () => {
  const all = policy(role('OperatorRole'));
  // ADR-003 amendment 2026-10-30 and APP-078 B1: plus exactly Get/PutSecretValue on the capability and auth
  // configuration secrets and their keys via Secrets Manager.
  const secretRefs = ['ProviderCapabilities', 'AuthConfiguration', 'ComposerModel', 'SlackApp'].map(prefix => ({ Ref: one('AWS::SecretsManager::Secret', prefix)[0] }));
  const keyArns = ['ProviderCapabilityKey', 'AuthConfigurationKey', 'ComposerModelKey', 'SlackAppKey'].map(prefix => ({ 'Fn::GetAtt': [one('AWS::KMS::Key', prefix)[0], 'Arn'] }));
  const custody = all.filter(statement => JSON.stringify(statement.Resource) === JSON.stringify(secretRefs)
    || JSON.stringify(statement.Resource) === JSON.stringify(keyArns));
  assert.deepEqual(custody.map(statement => [actions(statement).sort(), statement.Condition ?? null]), [
    [['secretsmanager:GetSecretValue', 'secretsmanager:PutSecretValue'], null],
    [['kms:Decrypt', 'kms:GenerateDataKey'], { StringEquals: { 'kms:ViaService': { 'Fn::Join': ['', ['secretsmanager.', { Ref: 'AWS::Region' }, '.amazonaws.com']] } } }],
  ]);
  const statements = all.filter(statement => !custody.includes(statement));
  assert.equal(statements.some(statement => actions(statement).some(action => action.startsWith('sts:') || action.startsWith('rds-db:') || action.startsWith('secretsmanager:') || action === 's3:DeleteObject' || action === 's3:ListBucket')), false);
  const bucket = one('AWS::S3::Bucket')[0];
  assert.equal(statements.length, 6);
  for (const statement of statements) assert.equal(statement.Effect, 'Allow');
  for (const [index, keyPrefix] of ['northstarKey', 'canaryKey'].entries()) {
    const prefix = `o/00000000-0000-4000-8000-00000000000${index + 1}/s/*`;
    const objectArn = { 'Fn::Join': ['', [{ 'Fn::GetAtt': [bucket, 'Arn'] }, `/${prefix}`]] };
    const objects = statements.filter(statement => JSON.stringify(statement.Resource).includes(prefix));
    // Review m3: HeadObject reads through s3:GetObject; PutObject is allowed only as a conditional create.
    assert.equal(objects.length, 2);
    const [get] = objects.filter(statement => actions(statement).includes('s3:GetObject'));
    const [put] = objects.filter(statement => actions(statement).includes('s3:PutObject'));
    assert.deepEqual(actions(get), ['s3:GetObject']); assert.equal(get.Condition, undefined); assert.deepEqual(get.Resource, objectArn);
    assert.deepEqual(actions(put), ['s3:PutObject']); assert.deepEqual(put.Resource, objectArn);
    assert.deepEqual(put.Condition, { StringEquals: { 's3:if-none-match': '*' } });
    // Review M1: a checksum-mode HeadObject of an SSE-KMS object needs kms:Decrypt, so a rerun can compare
    // an existing object. Both KMS actions are usable only through S3 in this region and only for this
    // tenant's B1 source objects (bucket keys are off, so S3's encryption context is the object ARN).
    const key = statements.filter(statement => JSON.stringify(statement.Resource) === JSON.stringify({ 'Fn::GetAtt': [one('AWS::KMS::Key', keyPrefix)[0], 'Arn'] }));
    assert.equal(key.length, 1); assert.deepEqual(actions(key[0]).sort(), ['kms:Decrypt', 'kms:GenerateDataKey']);
    assert.deepEqual(key[0].Condition, {
      StringEquals: { 'kms:ViaService': { 'Fn::Join': ['', ['s3.', { Ref: 'AWS::Region' }, '.amazonaws.com']] } },
      StringLike: { 'kms:EncryptionContext:aws:s3:arn': objectArn },
    });
    assert.equal(resolve(key[0].Condition.StringEquals['kms:ViaService'], { 'AWS::Region': 'us-east-1' }), 's3.us-east-1.amazonaws.com');
  }
  assert.equal(one('AWS::S3::Bucket')[1].Properties.BucketEncryption.ServerSideEncryptionConfiguration[0].BucketKeyEnabled, false);
  // The task role is the operator's own: not the runtime task role, not the migration task role.
  assert.notEqual(role('OperatorRole'), role('TaskRole')); assert.equal(policy(role('MigrationRole')).length, 0);
  // The bucket policy still denies any PutObject without SSE-KMS under that prefix's tenant key.
  const bucketPolicy = one('AWS::S3::BucketPolicy')[1].Properties.PolicyDocument.Statement;
  for (const index of [0, 1]) {
    const prefix = `/o/00000000-0000-4000-8000-00000000000${index + 1}/*`;
    assert.equal(bucketPolicy.filter((statement: any) => statement.Effect === 'Deny' && statement.Action === 's3:PutObject' && JSON.stringify(statement.Resource).includes(prefix)).length, 2);
  }
});

test('documented operator overrides fit the 8,192-byte ECS limit and match the task entry point', () => {
  assert.equal(overrides.max_override_bytes, 8192);
  const container = one('AWS::ECS::TaskDefinition', 'OperatorTask')[1].Properties.ContainerDefinitions[0];
  assert.deepEqual(container.EntryPoint, overrides.entryPoint);
  assert.ok(overrides.overrides.length >= 5);
  for (const item of overrides.overrides) {
    assert.ok(Buffer.byteLength(JSON.stringify(item.overrides)) <= 8192, item.name);
    assert.deepEqual(Object.keys(item.overrides), ['containerOverrides']);
    assert.equal(item.overrides.containerOverrides.length, 1);
    assert.deepEqual(Object.keys(item.overrides.containerOverrides[0]).sort(), ['command', 'name']);
    assert.equal(item.overrides.containerOverrides[0].name, container.Name);
  }
});

test('the service, migration and qualification task definitions do not reference the operator', () => {
  for (const prefix of ['Task', 'MigrationTask', 'QualificationTask']) {
    const body = JSON.stringify(one('AWS::ECS::TaskDefinition', prefix)[1]);
    assert.equal(/Operator(Image|Role|Task)|GENUI_OPERATOR_/.test(body), false, prefix);
  }
  assert.deepEqual(one('AWS::ECS::TaskDefinition', 'MigrationTask')[1].Properties.ContainerDefinitions[0].EntryPoint, ['python3', '/app/runtime.py']);
});
