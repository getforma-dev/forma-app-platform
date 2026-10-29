import assert from 'node:assert/strict';
import { test } from 'node:test';
import { App } from 'aws-cdk-lib';
import { Match, Template } from 'aws-cdk-lib/assertions';
import { GenuiFoundation, IMAGE_DIGEST_PATTERN, OPERATOR_CIDR_PATTERN, QUALIFICATION_IMAGE_PATTERN, QUALIFICATION_SECRET_ARN_PATTERN } from '../src/stack.js';

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
  if (value['Fn::If']) return resolve(value['Fn::If'][resolve(document.Conditions[value['Fn::If'][0]], parameters) ? 1 : 2], parameters);
  return Object.fromEntries(Object.entries(value).map(([key, child]) => [key, resolve(child, parameters)]).filter(([, child]) => child !== undefined));
}

test('managed data stays private, encrypted, retained and versioned', () => {
  template.hasResourceProperties('AWS::RDS::DBInstance', { PubliclyAccessible: false, StorageEncrypted: true, EnableIAMDatabaseAuthentication: true, DeletionProtection: true, BackupRetentionPeriod: 7, DBInstanceClass: 'db.t4g.small', AllocatedStorage: '20', MultiAZ: false, AutoMinorVersionUpgrade: false });
  // A maximum equal to allocated storage is an invalid autoscaling request.
  // Omission keeps the intended fixed 20-GiB allocation without enabling it.
  assert.equal(one('AWS::RDS::DBInstance')[1].Properties.MaxAllocatedStorage, undefined);
  template.hasResourceProperties('AWS::RDS::DBParameterGroup', { Parameters: { 'rds.force_ssl': '1' } });
  template.resourceCountIs('AWS::KMS::Key', 2);
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
  assert.deepEqual(document.Parameters.MonthlyBudgetUsd.AllowedValues, ['100']);
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
  template.hasResourceProperties('AWS::ECS::Service', { LaunchType: 'FARGATE', DesiredCount: { Ref: 'ServiceDesiredCount' }, EnableExecuteCommand: false, DeploymentConfiguration: Match.objectLike({ DeploymentCircuitBreaker: { Enable: true, Rollback: true }, MinimumHealthyPercent: 100, MaximumPercent: 200 }), NetworkConfiguration: { AwsvpcConfiguration: Match.objectLike({ AssignPublicIp: 'ENABLED' }) } });
  template.hasResourceProperties('AWS::ElasticLoadBalancingV2::Listener', { Port: 443, Protocol: 'HTTPS', Certificates: [{ CertificateArn: { Ref: 'TlsCertificateArn' } }], SslPolicy: 'ELBSecurityPolicy-TLS13-1-2-2021-06' });
  template.hasResourceProperties('AWS::ElasticLoadBalancingV2::TargetGroup', { Port: 3000, Protocol: 'HTTP', TargetType: 'ip', HealthCheckPath: '/health/ready', Matcher: { HttpCode: '200' } });
  const imagePattern = new RegExp(document.Parameters.ImageDigest.AllowedPattern);
  assert.equal(imagePattern.test('123456789012.dkr.ecr.us-east-1.amazonaws.com/team/genui@sha256:' + 'a'.repeat(64)), true);
  for (const invalid of ['123456789012.dkr.ecr.us-east-1.amazonaws.com/team/genui:latest', ...['/genui', 'team//genui', 'team/../genui', 'genui/'].map(repo => `123456789012.dkr.ecr.us-east-1.amazonaws.com/${repo}@sha256:${'a'.repeat(64)}`), `123456789012.dkr.ecr.us-east-1.amazonaws.com/genui@sha256:${'a'.repeat(64)}\n`]) assert.equal(imagePattern.test(invalid), false, invalid);
  assert.equal(document.Parameters.ImageDigest.AllowedPattern, IMAGE_DIGEST_PATTERN);
  for (const [id, task] of entries('AWS::ECS::TaskDefinition')) {
    // The optional APP-075 qualification task is smaller and runs its own image.
    const qualification = id.startsWith('QualificationTask');
    assert.equal(task.Properties.Cpu, qualification ? '1024' : '2048'); assert.equal(task.Properties.Memory, qualification ? '2048' : '8192');
    assert.deepEqual(task.Properties.RuntimePlatform, { CpuArchitecture: { Ref: 'ImageArchitecture' }, OperatingSystemFamily: 'LINUX' });
    for (const container of task.Properties.ContainerDefinitions) {
      assert.deepEqual(container.Image, { Ref: qualification ? 'QualificationImageDigest' : 'ImageDigest' }); assert.equal(container.ReadonlyRootFilesystem, true); assert.equal(container.User, '65532:65532');
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
  for (const [, item] of entries('AWS::IAM::Policy')) for (const statement of item.Properties.PolicyDocument.Statement) {
    assert.equal(actions(statement).some(action => action.startsWith('ssmmessages:') || action === 'ecs:ExecuteCommand' || action === 'iam:PassRole' || action === '*'), false);
    if (statement.Resource === '*') assert.equal(statement.Action, 'ecr:GetAuthorizationToken');
  }
});

test('only migration execution role receives admin secret; migration has no tenant AWS authority', () => {
  const readers = entries('AWS::IAM::Policy').filter(([, value]) => !value.Condition && value.Properties.PolicyDocument.Statement.some((statement: any) => actions(statement).includes('secretsmanager:GetSecretValue')));
  assert.equal(readers.length, 1); assert.deepEqual(readers[0][1].Properties.Roles, [{ Ref: role('MigrationExecutionRole') }]);
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
  assert.equal(resolve(container.Secrets, { QualificationConfigSecretArn: '' }), undefined); assert.deepEqual(container.MountPoints, [{ ContainerPath: '/run/genui', ReadOnly: false, SourceVolume: 'runtime-config' }]);
  assert.equal(container.EntryPoint, undefined); assert.equal(container.Command, undefined);
  const environment = Object.fromEntries(container.Environment.map((value: any) => [value.Name, value.Value]));
  assert.equal(environment.GENUI_RUNTIME_CONFIG_FILE, '/run/genui/credentials.json'); assert.equal(environment.GENUI_RDS_CA_FILE, '/etc/ssl/certs/rds-global-bundle.pem'); assert.equal(environment.FORMA_BIND_ADDRESS, '0.0.0.0'); assert.equal(environment.FORMA_PORT, '3000');
  assert.equal(Object.keys(environment).some(name => /PASSWORD|TOKEN|CREDENTIAL$/.test(name)), false);
  const database = JSON.parse(resolve(environment.GENUI_DATABASE_ROLES_JSON, {}));
  assert.deepEqual(Object.keys(database).sort(), ['control', 'data', 'worker', 'workspace']);
  for (const [name, arn] of Object.entries(database)) assert.equal(arn, `arn:fixture:${role(`DatabaseAccess${name}`)}:Arn`);
  const tenants = JSON.parse(resolve(environment.GENUI_TENANTS_JSON, {})); assert.equal(tenants.length, 2);
  for (const tenant of tenants) {
    assert.deepEqual(Object.keys(tenant).sort(), ['kms_key_arn', 'organization_id', 'prefix', 'role_arn']); assert.equal(tenant.prefix, `o/${tenant.organization_id}/`);
  }
});

test('qualification secret parameter accepts only empty or the exact rehearsal account/region/name scope', () => {
  const parameter = document.Parameters.QualificationConfigSecretArn;
  assert.equal(parameter.Default, '');
  assert.equal(parameter.AllowedPattern, QUALIFICATION_SECRET_ARN_PATTERN);
  const pattern = new RegExp(parameter.AllowedPattern);
  const arn = 'arn:aws:secretsmanager:us-east-1:877573132739:secret:genui-qualification-northstar-A1b2C3';
  for (const valid of ['', arn, arn.replace('northstar', 'canary-2026-10-28')]) assert.equal(pattern.test(valid), true, valid);
  for (const invalid of [
    ' ', '\n', arn + '\n', arn + ':password::', arn + ':AWSCURRENT',
    arn.replace('us-east-1', 'us-west-2'), arn.replace('877573132739', '123456789012'),
    arn.replace('genui-qualification-', 'production-'), arn.replace('northstar', '*'),
    arn.replace('northstar', '../northstar'), arn.replace('northstar', 'Northstar'),
    arn.replace('northstar', ''), arn.replace('northstar', 'a'.repeat(81)),
    arn.replace('A1b2C3', 'short'), arn.replace('A1b2C3', 'toolong'),
  ]) assert.equal(pattern.test(invalid), false, invalid);
});

test('qualification is disabled by default without an app secret or a live optional read policy', () => {
  const parameters = { QualificationConfigSecretArn: document.Parameters.QualificationConfigSecretArn.Default };
  const [optionalId, optional] = one('AWS::IAM::Policy', 'QualificationConfigRead');
  assert.equal(resolve(document.Conditions[optional.Condition], parameters), false);
  const activeReaders = entries('AWS::IAM::Policy').filter(([, resource]) =>
    (!resource.Condition || resolve(document.Conditions[resource.Condition], parameters)) &&
    resource.Properties.PolicyDocument.Statement.some((statement: any) => actions(statement).includes('secretsmanager:GetSecretValue')));
  assert.equal(activeReaders.length, 1);
  assert.deepEqual(activeReaders[0][1].Properties.Roles, [{ Ref: role('MigrationExecutionRole') }]);
  const app = resolve(one('AWS::ECS::TaskDefinition', 'Task')[1].Properties, parameters);
  assert.equal(Object.hasOwn(app.ContainerDefinitions[0], 'Secrets'), false);
  assert.equal(document.Parameters.ServiceDesiredCount.Default, 0);
  // A false condition must not suppress the default task/service through DependsOn.
  for (const resource of Object.values(document.Resources) as any[]) {
    assert.equal(array(resource.DependsOn ?? []).includes(optionalId), false);
  }
  template.resourceCountIs('AWS::SecretsManager::Secret', 1); // Existing migration secret only.
});

test('enabled qualification injects the whole exact secret only into the app through its execution role', () => {
  const arn = 'arn:aws:secretsmanager:us-east-1:877573132739:secret:genui-qualification-review-A1b2C3';
  const parameters = { QualificationConfigSecretArn: arn };
  const [, optional] = one('AWS::IAM::Policy', 'QualificationConfigRead');
  assert.equal(resolve(document.Conditions[optional.Condition], parameters), true);
  assert.deepEqual(optional.Properties.Roles, [{ Ref: role('ExecutionRole') }]);
  assert.deepEqual(resolve(optional.Properties.PolicyDocument, parameters), {
    Version: '2012-10-17', Statement: [{ Effect: 'Allow', Action: 'secretsmanager:GetSecretValue', Resource: arn }],
  });
  const app = one('AWS::ECS::TaskDefinition', 'Task')[1].Properties.ContainerDefinitions[0];
  assert.deepEqual(resolve(app.Secrets, parameters), [{ Name: 'GENUI_AUTH_QUALIFICATION_CONFIG_JSON', ValueFrom: arn }]);
  assert.equal(app.Environment.some((entry: any) => entry.Name.startsWith('GENUI_AUTH_QUALIFICATION_')), false);
  const migration = one('AWS::ECS::TaskDefinition', 'MigrationTask')[1].Properties.ContainerDefinitions[0];
  assert.deepEqual(migration.Secrets.map((entry: any) => entry.Name), ['PGPASSWORD']);
  assert.equal(JSON.stringify(migration).includes('QualificationConfigSecretArn'), false);
  assert.equal(policy(role('TaskRole')).some(statement => actions(statement).some(action => action.startsWith('secretsmanager:'))), false);
  assert.equal(policy(role('MigrationRole')).length, 0);
  assert.equal(JSON.stringify(document.Outputs).includes('QualificationConfigSecretArn'), false);
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
