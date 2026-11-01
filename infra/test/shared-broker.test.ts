import assert from 'node:assert/strict';
import { test } from 'node:test';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { App } from 'aws-cdk-lib';
import { Template } from 'aws-cdk-lib/assertions';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { GenuiFoundation } from '../src/stack.js';
import { CLEAN_TRIAL_CONTEXT, cleanTrialConfiguration } from '../src/shared-broker.js';

const trial = { organization_id: '00000000-0000-4000-8000-0000000000e1', tenant_slug: 'forma-trial', provider_tenant_id: 'ten_test_trial' };
for (const suite of ['clean_trial_test.py', 'shared_broker_activation_test.py']) {
  test(`clean-trial Python qualification: ${suite}`, () => {
    execFileSync('python3', ['-m', 'unittest', 'discover', '-s', 'test', '-p', suite, '-v'], {
      cwd: fileURLToPath(new URL('../', import.meta.url)), stdio: 'pipe',
    });
  });
}
function withTrial<T>(run: (path: string) => T): T {
  const directory = mkdtempSync(join(tmpdir(), 'forma-clean-trial-'));
  const path = join(directory, 'trial.json'); writeFileSync(path, JSON.stringify(trial), { mode: 0o600 });
  try { return run(path); } finally { rmSync(directory, { recursive: true }); }
}
const document = withTrial(path => Template.fromStack(new GenuiFoundation(new App({ context: { [CLEAN_TRIAL_CONTEXT]: path } }), 'CleanTrial')).toJSON());
const resources = (type: string) => Object.entries<any>(document.Resources).filter(([, resource]) => resource.Type === type);
const one = (type: string, prefix: string): [string, any] => {
  const found = resources(type).filter(([id]) => id.startsWith(prefix)); assert.equal(found.length, 1, prefix); return found[0];
};

test('clean profile refuses fixture identity, persona names and unknown fields', () => withTrial(path => {
  assert.deepEqual(cleanTrialConfiguration(path), trial);
  for (const change of [{ organization_id: '00000000-0000-4000-8000-000000000001' }, { tenant_slug: 'northstar' }, { provider_tenant_id: 'unverified' }, { token: 'not-allowed' }]) {
    writeFileSync(path, JSON.stringify({ ...trial, ...change })); assert.throws(() => cleanTrialConfiguration(path));
  }
  writeFileSync(path, JSON.stringify(trial));
  assert.throws(() => new GenuiFoundation(new App({ context: { [CLEAN_TRIAL_CONTEXT]: path, displayNamesFile: 'anything' } }), 'BadTrial'), /persona/);
}));

test('clean service disables demo and gives only its narrow broker token to the app', () => {
  const app = one('AWS::ECS::TaskDefinition', 'Task')[1].Properties.ContainerDefinitions[0];
  const env = Object.fromEntries(app.Environment.map((e: any) => [e.Name, e.Value]));
  assert.equal(env.GENUI_DEMO_MODE, '0'); assert.equal(env.GENUI_DEMO_ORGANIZATIONS, '');
  assert.equal(env.GENUI_DISPLAY_NAMES_JSON, undefined);
  assert.equal(app.Secrets.length, 6);
  assert.ok(app.Secrets.some((s: any) => s.Name === 'GENUI_SHARED_INVITE_BROKER_JSON'));
  assert.ok(!JSON.stringify(app).includes('MANAGEMENT_SECRET'));
  assert.deepEqual(document.Outputs.CleanTrialOrganization.Value, trial.organization_id);
  const rollback = one('AWS::ECS::TaskDefinition', 'RollbackTask')[1].Properties.ContainerDefinitions[0];
  assert.equal(rollback.Secrets, undefined);
});

test('broker is a separate bounded single-writer task with no database or object-store authority', () => {
  const task = one('AWS::ECS::TaskDefinition', 'SharedBrokerTask')[1].Properties;
  assert.equal(task.Cpu, '256'); assert.equal(task.Memory, '512');
  const container = task.ContainerDefinitions[0];
  assert.equal(task.ContainerDefinitions.length, 1); assert.equal(container.Name, 'broker');
  assert.equal(container.ReadonlyRootFilesystem, true); assert.equal(container.User, '65532:65532');
  assert.deepEqual(container.Image, { Ref: 'OperatorImageDigest' });
  assert.deepEqual(container.EntryPoint, ['python3', '-I', '-u', '/operator/app/operator/shared_invite_broker.py']);
  assert.deepEqual(container.Secrets.map((x: any) => x.Name).sort(), ['GENUI_PROVIDER_MANAGEMENT_SECRET_KEY', 'GENUI_PROVIDER_MANAGEMENT_SECRET_TENANT_ID', 'GENUI_SHARED_BROKER_CONFIG_JSON']);
  assert.ok(!JSON.stringify(container).includes('PGPASSWORD'));
  const taskRole = one('AWS::IAM::Role', 'SharedBrokerTaskRole')[0];
  const policies = resources('AWS::IAM::Policy').filter(([, r]) => r.Properties.Roles.some((role: any) => role.Ref === taskRole));
  const statements = policies.flatMap(([, r]) => r.Properties.PolicyDocument.Statement);
  const actions = statements.flatMap((s: any) => Array.isArray(s.Action) ? s.Action : [s.Action]);
  assert.ok(actions.includes('secretsmanager:PutSecretValue'));
  assert.ok(!actions.some((a: string) => /^(sts:|s3:|rds|ecs:|iam:)/.test(a)));
  for (const s of statements.filter((s: any) => JSON.stringify(s.Action).includes('secretsmanager:'))) {
    assert.match(JSON.stringify(s.Resource), /SharedBrokerState/);
  }
  const service = one('AWS::ECS::Service', 'SharedBrokerService')[1].Properties;
  assert.equal(service.DesiredCount, 0); assert.equal(service.EnableExecuteCommand, false);
  assert.equal(service.DeploymentConfiguration.MinimumHealthyPercent, 0); assert.equal(service.DeploymentConfiguration.MaximumPercent, 100);
});

test('broker has one exact ALB route, dedicated ingress, retained state and its own stop schedule', () => {
  const rules = resources('AWS::ElasticLoadBalancingV2::ListenerRule');
  assert.equal(rules.length, 1);
  assert.deepEqual(rules[0][1].Properties.Conditions, [{ Field: 'path-pattern', PathPatternConfig: { Values: ['/v1/shared-invitations'] } }]);
  const security = one('AWS::EC2::SecurityGroup', 'SharedBrokerSecurity');
  const ingress = resources('AWS::EC2::SecurityGroupIngress').filter(([, r]) => JSON.stringify(r.Properties.GroupId).includes(security[0]));
  assert.equal(ingress.length, 1); assert.equal(ingress[0][1].Properties.FromPort, 3001); assert.equal(ingress[0][1].Properties.CidrIp, undefined);
  assert.match(JSON.stringify(ingress[0][1].Properties.SourceSecurityGroupId), /AlbSecurity/);
  const state = one('AWS::SecretsManager::Secret', 'SharedBrokerState')[1];
  assert.equal(state.DeletionPolicy, 'Retain'); assert.deepEqual(JSON.parse(state.Properties.SecretString), { version: 1, entries: {}, write_reservations: [] });
  const stop = one('AWS::Scheduler::Schedule', 'SharedBrokerExpiry')[1].Properties;
  assert.equal(stop.State, 'ENABLED'); assert.equal(stop.ScheduleExpressionTimezone, 'UTC');
  assert.ok(JSON.stringify(stop.Target.Input).includes('SharedBrokerService'));
  assert.ok(!JSON.stringify(stop.Target.Input).includes('ServiceD69D759B'));
  assert.ok(document.Outputs.SharedBrokerExpiryScheduleName);
});

test('fresh trial gets an additional object boundary without replacing sandbox keys or roles', () => {
  assert.ok(one('AWS::KMS::Key', 'northstarKey')); assert.ok(one('AWS::KMS::Key', 'canaryKey')); assert.ok(one('AWS::KMS::Key', 'cleanTrialKey'));
  const access = one('AWS::IAM::Role', 'TenantAccess2')[0];
  const policies = resources('AWS::IAM::Policy').filter(([, r]) => r.Properties.Roles.some((role: any) => role.Ref === access));
  const serialized = JSON.stringify(policies);
  assert.ok(serialized.includes(`o/${trial.organization_id}`));
  assert.ok(!serialized.includes('o/00000000-0000-4000-8000-000000000001'));
  assert.ok(!serialized.includes('o/00000000-0000-4000-8000-000000000002'));
});
