import { CfnOutput, Duration, RemovalPolicy, SecretValue, Stack } from 'aws-cdk-lib';
import * as ec2 from 'aws-cdk-lib/aws-ec2';
import * as ecs from 'aws-cdk-lib/aws-ecs';
import * as elbv2 from 'aws-cdk-lib/aws-elasticloadbalancingv2';
import * as iam from 'aws-cdk-lib/aws-iam';
import * as kms from 'aws-cdk-lib/aws-kms';
import * as logs from 'aws-cdk-lib/aws-logs';
import * as scheduler from 'aws-cdk-lib/aws-scheduler';
import * as secretsmanager from 'aws-cdk-lib/aws-secretsmanager';
import { Construct } from 'constructs';
import { readFileSync } from 'node:fs';

export const CLEAN_TRIAL_CONTEXT = 'cleanTrialFile';
export const SHARED_BROKER_JSON_ENV = 'GENUI_SHARED_INVITE_BROKER_JSON';
export interface CleanTrial { organization_id: string; tenant_slug: string; provider_tenant_id: string }
export function cleanTrialConfiguration(path: unknown): CleanTrial | undefined {
  if (path === undefined) return undefined;
  if (typeof path !== 'string' || path === '') throw new Error('cleanTrialFile must name a private identifiers file');
  const text = readFileSync(path, 'utf8');
  if (Buffer.byteLength(text) > 4096) throw new Error('cleanTrialFile exceeds its bound');
  const value = JSON.parse(text);
  if (!value || Object.keys(value).sort().join(',') !== 'organization_id,provider_tenant_id,tenant_slug'
    || typeof value.organization_id !== 'string' || !/^[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(value.organization_id)
    || ['00000000-0000-4000-8000-000000000001', '00000000-0000-4000-8000-000000000002'].includes(value.organization_id)
    || typeof value.tenant_slug !== 'string' || !/^[a-z0-9][a-z0-9-]{0,63}$/.test(value.tenant_slug)
    || ['northstar', 'canary'].includes(value.tenant_slug)
    || typeof value.provider_tenant_id !== 'string' || !/^ten_[A-Za-z0-9_-]{1,250}$/.test(value.provider_tenant_id)) {
    throw new Error('cleanTrialFile must identify one fresh organization and provider tenant, with no credentials');
  }
  return value;
}

interface Props {
  vpc: ec2.IVpc; cluster: ecs.ICluster; listener: elbv2.ApplicationListener;
  albSecurity: ec2.ISecurityGroup; log: logs.ILogGroup; runtimePlatform: ecs.RuntimePlatform;
  operatorImage: string; repositoryArn: string; managementSecretArn: string; managementUrl: string;
  expiry: string; expiryGroup: scheduler.CfnScheduleGroup;
}

/** Separate task identity: no application role, database connection, object
 * store, migration password, or ordinary membership invitation capabilities. */
export class SharedBroker extends Construct {
  readonly clientConfiguration: secretsmanager.Secret;
  constructor(scope: Stack, id: string, props: Props) {
    super(scope, id);
    const key = new kms.Key(this, 'Key', { enableKeyRotation: true, removalPolicy: RemovalPolicy.RETAIN,
      description: 'Named sharing invitation broker configuration and bounded state' });
    const secret = (name: string, initial: string) => new secretsmanager.Secret(this, name, {
      encryptionKey: key, secretStringValue: SecretValue.unsafePlainText(initial), removalPolicy: RemovalPolicy.RETAIN,
      description: `Named sharing ${name}; operator initialized; no provider management credential copied`,
    });
    this.clientConfiguration = secret('ClientConfiguration', '{}');
    const configuration = secret('Configuration', '{}');
    const state = secret('State', '{"version":1,"entries":{},"write_reservations":[]}');
    const execution = new iam.Role(this, 'ExecutionRole', { assumedBy: new iam.ServicePrincipal('ecs-tasks.amazonaws.com') });
    execution.addToPolicy(new iam.PolicyStatement({ actions: ['ecr:GetAuthorizationToken'], resources: ['*'] }));
    execution.addToPolicy(new iam.PolicyStatement({ actions: ['ecr:BatchCheckLayerAvailability', 'ecr:GetDownloadUrlForLayer', 'ecr:BatchGetImage'], resources: [props.repositoryArn] }));
    const taskRole = new iam.Role(this, 'TaskRole', { assumedBy: new iam.ServicePrincipal('ecs-tasks.amazonaws.com') });
    state.grantRead(taskRole);
    taskRole.addToPolicy(new iam.PolicyStatement({ actions: ['secretsmanager:PutSecretValue'], resources: [state.secretArn] }));
    key.grantEncryptDecrypt(taskRole);
    const management = secretsmanager.Secret.fromSecretCompleteArn(this, 'ProviderManagement', props.managementSecretArn);
    const task = new ecs.FargateTaskDefinition(this, 'Task', { cpu: 256, memoryLimitMiB: 512, taskRole, executionRole: execution, runtimePlatform: props.runtimePlatform });
    task.addVolume({ name: 'broker-private' });
    const container = task.addContainer('broker', {
      image: ecs.ContainerImage.fromRegistry(props.operatorImage), readonlyRootFilesystem: true, user: '65532:65532',
      entryPoint: ['python3', '-I', '-u', '/operator/app/operator/shared_invite_broker.py'], command: [],
      environment: { AWS_REGION: scope.region, GENUI_SHARED_BROKER_BIND_ADDRESS: '0.0.0.0', GENUI_PROVIDER_MANAGEMENT_URL: props.managementUrl },
      secrets: { GENUI_SHARED_BROKER_CONFIG_JSON: ecs.Secret.fromSecretsManager(configuration),
        GENUI_PROVIDER_MANAGEMENT_SECRET_KEY: ecs.Secret.fromSecretsManager(management, 'management_key'),
        GENUI_PROVIDER_MANAGEMENT_SECRET_TENANT_ID: ecs.Secret.fromSecretsManager(management, 'tenant_id') },
      logging: ecs.LogDrivers.awsLogs({ streamPrefix: 'shared-broker', logGroup: props.log }),
      healthCheck: { command: ['CMD', 'python3', '-c', "import urllib.request; urllib.request.urlopen('http://127.0.0.1:3001/health/live', timeout=2).read()"], interval: Duration.seconds(30), timeout: Duration.seconds(5), retries: 3, startPeriod: Duration.seconds(30) },
    });
    container.addMountPoints({ sourceVolume: 'broker-private', containerPath: '/run/operator', readOnly: false });
    container.addPortMappings({ containerPort: 3001 });
    const security = new ec2.SecurityGroup(this, 'Security', { vpc: props.vpc, allowAllOutbound: false });
    security.addIngressRule(props.albSecurity, ec2.Port.tcp(3001), 'Only authenticated HTTPS routing through the ALB');
    security.addEgressRule(ec2.Peer.anyIpv4(), ec2.Port.tcp(443), 'Provider and Secrets Manager HTTPS only');
    props.albSecurity.addEgressRule(security, ec2.Port.tcp(3001), 'Invitation broker and health checks');
    const service = new ecs.FargateService(this, 'Service', { cluster: props.cluster, taskDefinition: task, desiredCount: 0,
      platformVersion: ecs.FargatePlatformVersion.VERSION1_4, assignPublicIp: true, vpcSubnets: { subnetType: ec2.SubnetType.PUBLIC },
      securityGroups: [security], enableExecuteCommand: false, circuitBreaker: { rollback: true },
      healthCheckGracePeriod: Duration.seconds(60), minHealthyPercent: 0, maxHealthyPercent: 100 });
    (service.node.defaultChild as ecs.CfnService).addPropertyOverride('AvailabilityZoneRebalancing', 'DISABLED');
    props.listener.addTargets('SharedInvitations', { priority: 20, conditions: [elbv2.ListenerCondition.pathPatterns(['/v1/shared-invitations'])],
      port: 3001, protocol: elbv2.ApplicationProtocol.HTTP, targets: [service],
      healthCheck: { path: '/health/live', healthyHttpCodes: '200', interval: Duration.seconds(30), healthyThresholdCount: 2 },
      deregistrationDelay: Duration.seconds(30) });
    const expiryRole = new iam.Role(this, 'ExpiryRole', { assumedBy: new iam.ServicePrincipal('scheduler.amazonaws.com', { conditions: {
      StringEquals: { 'aws:SourceAccount': scope.account }, ArnEquals: { 'aws:SourceArn': props.expiryGroup.attrArn },
    } }) });
    expiryRole.addToPolicy(new iam.PolicyStatement({ actions: ['ecs:UpdateService'], resources: [service.serviceArn],
      conditions: { ArnEquals: { 'ecs:cluster': props.cluster.clusterArn } } }));
    const expiry = new scheduler.CfnSchedule(this, 'Expiry', { groupName: props.expiryGroup.ref, state: 'ENABLED',
      scheduleExpression: props.expiry, scheduleExpressionTimezone: 'UTC', flexibleTimeWindow: { mode: 'OFF' },
      target: { arn: `arn:${scope.partition}:scheduler:::aws-sdk:ecs:updateService`, roleArn: expiryRole.roleArn,
        input: scope.toJsonString({ Cluster: props.cluster.clusterArn, Service: service.serviceArn, DesiredCount: 0 }),
        retryPolicy: { maximumEventAgeInSeconds: 900, maximumRetryAttempts: 3 } } });
    for (const [name, value] of Object.entries({ SharedBrokerTaskDefinition: task.taskDefinitionArn, SharedBrokerServiceName: service.serviceName,
      SharedBrokerClientConfigurationSecretArn: this.clientConfiguration.secretArn, SharedBrokerConfigurationSecretArn: configuration.secretArn,
      SharedBrokerStateSecretArn: state.secretArn, SharedBrokerExpiryScheduleName: expiry.ref, SharedBrokerExpiryRoleArn: expiryRole.roleArn })) {
      new CfnOutput(scope, name, { value });
    }
    new CfnOutput(scope, 'SharedBrokerTaskRoleArn', { value: taskRole.roleArn });
    new CfnOutput(scope, 'SharedBrokerExecutionRoleArn', { value: execution.roleArn });
  }
}
