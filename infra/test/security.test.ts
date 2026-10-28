import assert from 'node:assert/strict';
import { test } from 'node:test';
import { App } from 'aws-cdk-lib';
import { Match, Template } from 'aws-cdk-lib/assertions';
import { GenuiFoundation, OPERATOR_CIDR_PATTERN } from '../src/stack.js';

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
  if (Array.isArray(value)) return value.map(item => resolve(item, parameters));
  if (value === null || typeof value !== 'object') return value;
  if (value.Ref) return parameters[value.Ref] ?? `reference:${value.Ref}`;
  if (value['Fn::GetAtt']) return `arn:fixture:${value['Fn::GetAtt'].join(':')}`;
  if (value['Fn::Join']) return resolve(value['Fn::Join'][1], parameters).join(value['Fn::Join'][0]);
  if (value['Fn::Split']) return resolve(value['Fn::Split'][1], parameters).split(value['Fn::Split'][0]);
  if (value['Fn::Select']) return resolve(value['Fn::Select'][1], parameters)[Number(value['Fn::Select'][0])];
  return Object.fromEntries(Object.entries(value).map(([key, child]) => [key, resolve(child, parameters)]));
}

test('managed data stays private, encrypted, retained and versioned', () => {
  template.hasResourceProperties('AWS::RDS::DBInstance', { PubliclyAccessible: false, StorageEncrypted: true, EnableIAMDatabaseAuthentication: true, DeletionProtection: true, BackupRetentionPeriod: 7, DBInstanceClass: 'db.t4g.small', AllocatedStorage: '20', MultiAZ: false });
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

test('ingress is exactly operator HTTPS, ALB-to-task and task-to-database', () => {
  const task = one('AWS::EC2::SecurityGroup', 'TaskSecurity')[0], alb = one('AWS::EC2::SecurityGroup', 'AlbSecurity')[0], database = one('AWS::EC2::SecurityGroup', 'DatabaseSecurity')[0];
  const ingress = entries('AWS::EC2::SecurityGroupIngress').map(([, item]) => item.Properties);
  for (const [id, group] of entries('AWS::EC2::SecurityGroup')) for (const rule of group.Properties.SecurityGroupIngress ?? []) ingress.push({ GroupId: reference(id), ...rule });
  assert.equal(ingress.length, 3);
  const byPort = (port: number) => ingress.find(rule => rule.FromPort === port);
  assert.deepEqual(byPort(443).GroupId, reference(alb)); assert.deepEqual(byPort(443).CidrIp, { Ref: 'OperatorIpv4Cidr' });
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
  for (const value of ['8.8.8.8/32', '1.1.1.0/24', '9.9.9.8/30']) assert.equal(pattern.test(value), true, value);
  for (const value of ['0.0.0.0/0', '8.8.0.0/16', '1.1.0.0/23', '10.1.2.3/32', '172.16.0.1/32', '172.31.255.1/24', '192.168.1.1/32', '127.0.0.1/32', '169.254.169.254/32', '224.1.2.3/32', '203.0.113.1/32', '198.51.100.1/32', '192.0.2.1/32', '100.64.1.1/32', '198.18.0.1/32', '256.1.1.1/32', '1.1.1.1/33', '8.8.8.8/32\n', '::/0']) assert.equal(pattern.test(value), false, value);
  for (const name of ['OperatorIpv4Cidr', 'TlsCertificateArn', 'Operator', 'MonthlyBudgetUsd', 'ImageArchitecture']) assert.equal(document.Parameters[name].Default, undefined);
  const cert = new RegExp(document.Parameters.TlsCertificateArn.AllowedPattern);
  assert.equal(cert.test('arn:aws:acm:us-east-1:123456789012:certificate/00000000-0000-4000-8000-000000000001'), true);
  assert.equal(cert.test('invented-certificate'), false);
  assert.deepEqual(document.Parameters.ImageArchitecture.AllowedValues, ['ARM64', 'X86_64']);
});

test('digest-pinned Fargate service has HTTPS and no default live task, exec, NAT or endpoints', () => {
  for (const type of ['AWS::EC2::NatGateway', 'AWS::EC2::VPCEndpoint', 'AWS::EC2::Instance', 'AWS::AppRunner::Service', 'AWS::Route53::RecordSet', 'AWS::CertificateManager::Certificate']) template.resourceCountIs(type, 0);
  template.resourceCountIs('AWS::ECS::Service', 1); template.resourceCountIs('AWS::ElasticLoadBalancingV2::Listener', 1);
  assert.equal(document.Parameters.ServiceDesiredCount.Default, 0); assert.deepEqual(document.Parameters.ServiceDesiredCount.AllowedValues, ['0', '1']);
  template.hasResourceProperties('AWS::ECS::Service', { LaunchType: 'FARGATE', DesiredCount: { Ref: 'ServiceDesiredCount' }, EnableExecuteCommand: false, DeploymentConfiguration: Match.objectLike({ DeploymentCircuitBreaker: { Enable: true, Rollback: true }, MinimumHealthyPercent: 100, MaximumPercent: 200 }), NetworkConfiguration: { AwsvpcConfiguration: Match.objectLike({ AssignPublicIp: 'ENABLED' }) } });
  template.hasResourceProperties('AWS::ElasticLoadBalancingV2::Listener', { Port: 443, Protocol: 'HTTPS', Certificates: [{ CertificateArn: { Ref: 'TlsCertificateArn' } }], SslPolicy: 'ELBSecurityPolicy-TLS13-1-2-2021-06' });
  template.hasResourceProperties('AWS::ElasticLoadBalancingV2::TargetGroup', { Port: 3000, Protocol: 'HTTP', TargetType: 'ip', HealthCheckPath: '/health/ready', Matcher: { HttpCode: '200' } });
  const imagePattern = new RegExp(document.Parameters.ImageDigest.AllowedPattern);
  assert.equal(imagePattern.test('123456789012.dkr.ecr.us-east-1.amazonaws.com/team/genui@sha256:' + 'a'.repeat(64)), true);
  for (const invalid of ['123456789012.dkr.ecr.us-east-1.amazonaws.com/team/genui:latest', ...['/genui', 'team//genui', 'team/../genui', 'genui/'].map(repo => `123456789012.dkr.ecr.us-east-1.amazonaws.com/${repo}@sha256:${'a'.repeat(64)}`), `123456789012.dkr.ecr.us-east-1.amazonaws.com/genui@sha256:${'a'.repeat(64)}\n`]) assert.equal(imagePattern.test(invalid), false, invalid);
  for (const [, task] of entries('AWS::ECS::TaskDefinition')) {
    assert.equal(task.Properties.Cpu, '2048'); assert.equal(task.Properties.Memory, '8192');
    assert.deepEqual(task.Properties.RuntimePlatform, { CpuArchitecture: { Ref: 'ImageArchitecture' }, OperatingSystemFamily: 'LINUX' });
    for (const container of task.Properties.ContainerDefinitions) {
      assert.deepEqual(container.Image, { Ref: 'ImageDigest' }); assert.equal(container.ReadonlyRootFilesystem, true); assert.equal(container.User, '65532:65532');
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
  for (const prefix of ['ExecutionRole', 'MigrationExecutionRole']) {
    const statements = policy(role(prefix));
    assert.equal(statements.find(statement => statement.Action === 'ecr:GetAuthorizationToken').Resource, '*');
    const pull = statements.find(statement => actions(statement).includes('ecr:BatchGetImage'));
    assert.deepEqual(actions(pull).sort(), ['ecr:BatchCheckLayerAvailability', 'ecr:BatchGetImage', 'ecr:GetDownloadUrlForLayer']);
    for (const repo of ['genui', 'team/nested/genui']) assert.equal(resolve(pull.Resource, { 'AWS::Partition': 'aws', ImageDigest: `123456789012.dkr.ecr.us-east-1.amazonaws.com/${repo}@sha256:${'b'.repeat(64)}` }), `arn:aws:ecr:us-east-1:123456789012:repository/${repo}`);
  }
  for (const [, item] of entries('AWS::IAM::Policy')) for (const statement of item.Properties.PolicyDocument.Statement) {
    assert.equal(actions(statement).some(action => action.startsWith('ssmmessages:') || action === 'ecs:ExecuteCommand' || action === 'iam:PassRole' || action === '*'), false);
    if (statement.Resource === '*') assert.equal(statement.Action, 'ecr:GetAuthorizationToken');
  }
});

test('only migration execution role receives admin secret; migration has no tenant AWS authority', () => {
  const readers = entries('AWS::IAM::Policy').filter(([, value]) => value.Properties.PolicyDocument.Statement.some((statement: any) => actions(statement).includes('secretsmanager:GetSecretValue')));
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
  assert.equal(container.Secrets, undefined); assert.deepEqual(container.MountPoints, [{ ContainerPath: '/run/genui', ReadOnly: false, SourceVolume: 'runtime-config' }]);
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
