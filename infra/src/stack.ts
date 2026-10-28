import { CfnCondition, CfnOutput, CfnParameter, Duration, Fn, RemovalPolicy, Stack, StackProps, Tags } from 'aws-cdk-lib';
import * as acm from 'aws-cdk-lib/aws-certificatemanager';
import * as ec2 from 'aws-cdk-lib/aws-ec2';
import * as ecs from 'aws-cdk-lib/aws-ecs';
import * as elbv2 from 'aws-cdk-lib/aws-elasticloadbalancingv2';
import * as iam from 'aws-cdk-lib/aws-iam';
import * as kms from 'aws-cdk-lib/aws-kms';
import * as logs from 'aws-cdk-lib/aws-logs';
import * as rds from 'aws-cdk-lib/aws-rds';
import * as s3 from 'aws-cdk-lib/aws-s3';
import * as scheduler from 'aws-cdk-lib/aws-scheduler';
import { Construct } from 'constructs';

// The temporary ingress trial is one real operator address; public mode uses /0.
// Keep the operator address separately so /0 can never masquerade as that trial.
const octet = '(?:25[0-5]|2[0-4][0-9]|1[0-9]{2}|[1-9]?[0-9])';
export const OPERATOR_CIDR_PATTERN = `^(?!(?:0|10|127)\\.)(?!100\\.(?:6[4-9]|[7-9][0-9]|1[01][0-9]|12[0-7])\\.)(?!169\\.254\\.)(?!172\\.(?:1[6-9]|2[0-9]|3[01])\\.)(?!192\\.168\\.)(?!192\\.0\\.[02]\\.)(?!192\\.88\\.99\\.)(?!198\\.(?:18|19)\\.)(?!198\\.51\\.100\\.)(?!203\\.0\\.113\\.)(?!2(?:2[4-9]|[3-5][0-9])\\.)${octet}(?:\\.${octet}){3}/32(?![\\s\\S])`;
export const EXPIRY_UTC_PATTERN = '^20[0-9]{2}-(?:0[1-9]|1[0-2])-(?:0[1-9]|[12][0-9]|3[01])T(?:[01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]Z(?![\\s\\S])';

/** Reviewable deployment model. Synth/tests never create cloud resources. */
export class GenuiFoundation extends Stack {
  constructor(scope: Construct, id: string, props?: StackProps) {
    super(scope, id, props);
    Tags.of(this).add('Application', 'forma-app-platform');
    Tags.of(this).add('Environment', 'synthetic');
    const operator = new CfnParameter(this, 'Operator', { type: 'String', minLength: 1, description: 'Named operator before provisioning' });
    const budget = new CfnParameter(this, 'MonthlyBudgetUsd', { type: 'Number', default: 100, allowedValues: ['100'], description: 'Approved $100 monthly operating limit; preflight accounts for retained resources and other spending. This is not an AWS billing hard-stop.' });
    const ingressMode = new CfnParameter(this, 'IngressMode', { type: 'String', default: 'public', allowedValues: ['operator', 'public'], description: 'operator only for the temporary /32 ingress trial; public HTTPS is the selected beta mode. Application access requires its separately tested signup/invite gate.' });
    const publicIngress = new CfnCondition(this, 'PublicIngress', { expression: Fn.conditionEquals(ingressMode.valueAsString, 'public') });
    const operatorCidr = new CfnParameter(this, 'OperatorIpv4Cidr', {
      type: 'String', allowedPattern: OPERATOR_CIDR_PATTERN,
      constraintDescription: 'Supply the real operator public IPv4 /32; broad/private/reserved placeholder ranges are not accepted.',
      description: 'Recorded operator address for the temporary ingress trial. Public mode uses 0.0.0.0/0 instead.',
    });
    const expiresAt = new CfnParameter(this, 'DeploymentExpiresAtUtc', { type: 'String', allowedPattern: EXPIRY_UTC_PATTERN, description: 'Required UTC timestamp YYYY-MM-DDTHH:MM:SSZ. Run scripts/preflight.py immediately before deployment; expiry must be real, future and at most 48 hours away.' });
    const certificateArn = new CfnParameter(this, 'TlsCertificateArn', {
      type: 'String', allowedPattern: '^arn:aws:acm:[a-z0-9-]+:[0-9]{12}:certificate/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}(?![\\s\\S])',
      description: 'Existing issued ACM certificate ARN in this account/region for the operator-owned application hostname; no certificate or DNS name is invented.',
    });
    const desiredCount = new CfnParameter(this, 'ServiceDesiredCount', {
      type: 'Number', default: 0, allowedValues: ['0'],
      description: 'Template creates service at zero; later unrelated updates may preserve actual count drift. Observe service/tasks stopped before replacing image/window/schedule. Activate only through the live-checked preflight path.',
    });
    Tags.of(this).add('Operator', operator.valueAsString, { excludeResourceTypes: ['aws:cdk:stack'] });
    Tags.of(this).add('ApprovedMonthlyBudgetUsd', budget.valueAsString, { excludeResourceTypes: ['aws:cdk:stack'] });
    const vpc = new ec2.Vpc(this, 'Vpc', {
      ipAddresses: ec2.IpAddresses.cidr('10.72.0.0/16'), maxAzs: 2, natGateways: 0,
      subnetConfiguration: [
        { name: 'public', subnetType: ec2.SubnetType.PUBLIC, cidrMask: 24 },
        { name: 'database', subnetType: ec2.SubnetType.PRIVATE_ISOLATED, cidrMask: 24 },
      ],
    });
    const taskSecurity = new ec2.SecurityGroup(this, 'TaskSecurity', { vpc, allowAllOutbound: false });
    taskSecurity.addEgressRule(ec2.Peer.anyIpv4(), ec2.Port.tcp(443), 'AWS/provider HTTPS; application disclosure policy remains separate');
    const databaseSecurity = new ec2.SecurityGroup(this, 'DatabaseSecurity', { vpc, allowAllOutbound: false });
    databaseSecurity.addIngressRule(taskSecurity, ec2.Port.tcp(5432), 'Only app task identity reaches PostgreSQL');
    taskSecurity.addEgressRule(databaseSecurity, ec2.Port.tcp(5432), 'PostgreSQL');
    const migrationSecret = new rds.DatabaseSecret(this, 'MigrationSecret', { username: 'genui_migration' });
    migrationSecret.applyRemovalPolicy(RemovalPolicy.RETAIN);
    const database = new rds.DatabaseInstance(this, 'Database', {
      engine: rds.DatabaseInstanceEngine.postgres({ version: rds.PostgresEngineVersion.VER_16_13 }),
      instanceType: ec2.InstanceType.of(ec2.InstanceClass.T4G, ec2.InstanceSize.SMALL),
      vpc, vpcSubnets: { subnetType: ec2.SubnetType.PRIVATE_ISOLATED }, securityGroups: [databaseSecurity],
      credentials: rds.Credentials.fromSecret(migrationSecret), databaseName: 'genui',
      allocatedStorage: 20, storageEncrypted: true, storageType: rds.StorageType.GP3,
      publiclyAccessible: false, multiAz: false, iamAuthentication: true, backupRetention: Duration.days(7),
      deletionProtection: true, removalPolicy: RemovalPolicy.RETAIN,
      parameters: { 'rds.force_ssl': '1' },
    });
    const tenantIds = ['00000000-0000-4000-8000-000000000001', '00000000-0000-4000-8000-000000000002'];
    const keys = ['northstar', 'canary'].map(name => new kms.Key(this, `${name}Key`, {
      description: `GenUI synthetic ${name} organization`, enableKeyRotation: true, removalPolicy: RemovalPolicy.RETAIN,
    }));
    const bucket = new s3.Bucket(this, 'Evidence', {
      versioned: true, encryption: s3.BucketEncryption.KMS, encryptionKey: keys[0], bucketKeyEnabled: false,
      blockPublicAccess: s3.BlockPublicAccess.BLOCK_ALL, enforceSSL: true, removalPolicy: RemovalPolicy.RETAIN,
      objectOwnership: s3.ObjectOwnership.BUCKET_OWNER_ENFORCED,
    });
    const taskRole = new iam.Role(this, 'TaskRole', { assumedBy: new iam.ServicePrincipal('ecs-tasks.amazonaws.com') });
    const databaseRoles: Record<string, string> = {};
    for (const role of ['workspace', 'control', 'data', 'worker']) {
      const access = new iam.Role(this, `DatabaseAccess${role}`, { assumedBy: new iam.ArnPrincipal(taskRole.roleArn), maxSessionDuration: Duration.hours(1) });
      database.grantConnect(access, `genui_${role}`);
      taskRole.addToPolicy(new iam.PolicyStatement({ actions: ['sts:AssumeRole'], resources: [access.roleArn] }));
      databaseRoles[role] = access.roleArn;
    }
    // In the demo's one process, task compromise reaches all logical roles.
    // Prefix and key policies bound configuration errors; separate deployments
    // and credentials are a later managed-pilot requirement.
    const tenants: Array<{ organization_id: string; role_arn: string; kms_key_arn: string; prefix: string }> = [];
    for (const [i, tenant] of tenantIds.entries()) {
      const prefix = `o/${tenant}/*`;
      const tenantRole = new iam.Role(this, `TenantAccess${i}`, { assumedBy: new iam.ArnPrincipal(taskRole.roleArn), maxSessionDuration: Duration.hours(1) });
      taskRole.addToPolicy(new iam.PolicyStatement({ actions: ['sts:AssumeRole'], resources: [tenantRole.roleArn] }));
      tenantRole.addToPolicy(new iam.PolicyStatement({ actions: ['s3:GetObject', 's3:GetObjectVersion', 's3:PutObject'], resources: [bucket.arnForObjects(prefix)] }));
      tenantRole.addToPolicy(new iam.PolicyStatement({ actions: ['s3:ListBucket'], resources: [bucket.bucketArn], conditions: { StringLike: { 's3:prefix': [`o/${tenant}`, `o/${tenant}/*`] } } }));
      keys[i].grantEncryptDecrypt(tenantRole);
      bucket.addToResourcePolicy(new iam.PolicyStatement({ effect: iam.Effect.DENY, principals: [new iam.AnyPrincipal()], actions: ['s3:PutObject'], resources: [bucket.arnForObjects(prefix)], conditions: { StringNotEquals: { 's3:x-amz-server-side-encryption-aws-kms-key-id': keys[i].keyArn } } }));
      bucket.addToResourcePolicy(new iam.PolicyStatement({ effect: iam.Effect.DENY, principals: [new iam.AnyPrincipal()], actions: ['s3:PutObject'], resources: [bucket.arnForObjects(prefix)], conditions: { StringNotEquals: { 's3:x-amz-server-side-encryption': 'aws:kms' } } }));
      tenants.push({ organization_id: tenant, role_arn: tenantRole.roleArn, kms_key_arn: keys[i].keyArn, prefix: `o/${tenant}/` });
    }
    const cluster = new ecs.Cluster(this, 'Cluster', { vpc });
    const image = new CfnParameter(this, 'ImageDigest', { type: 'String', allowedPattern: '^[0-9]{12}\\.dkr\\.ecr\\.[a-z0-9-]+\\.amazonaws\\.com/[a-z0-9]+(?:[._-][a-z0-9]+)*(?:/[a-z0-9]+(?:[._-][a-z0-9]+)*)*@sha256:[0-9a-f]{64}(?![\\s\\S])', description: 'Reviewed ECR image digest, not a mutable tag' });
    const imageArchitecture = new CfnParameter(this, 'ImageArchitecture', {
      type: 'String', allowedValues: ['ARM64', 'X86_64'],
      description: 'Required architecture matching the supplied immutable image; ARM64 local and X86_64 CI images are distinct artifacts.',
    });
    const runtimePlatform = { cpuArchitecture: ecs.CpuArchitecture.of(imageArchitecture.valueAsString), operatingSystemFamily: ecs.OperatingSystemFamily.LINUX };
    const imageUri = Fn.select(0, Fn.split('@sha256:', image.valueAsString));
    const imageDomain = Fn.select(0, Fn.split('/', imageUri));
    const repositoryArn = this.formatArn({ service: 'ecr', region: Fn.select(3, Fn.split('.', imageDomain)), account: Fn.select(0, Fn.split('.', imageDomain)), resource: 'repository', resourceName: Fn.select(1, Fn.split('.amazonaws.com/', imageUri)) });
    const makeExecutionRole = (name: string) => {
      const role = new iam.Role(this, name, { assumedBy: new iam.ServicePrincipal('ecs-tasks.amazonaws.com') });
      role.addToPolicy(new iam.PolicyStatement({ actions: ['ecr:GetAuthorizationToken'], resources: ['*'] }));
      role.addToPolicy(new iam.PolicyStatement({ actions: ['ecr:BatchCheckLayerAvailability', 'ecr:GetDownloadUrlForLayer', 'ecr:BatchGetImage'], resources: [repositoryArn] }));
      return role;
    };
    const task = new ecs.FargateTaskDefinition(this, 'Task', { cpu: 2048, memoryLimitMiB: 8192, taskRole, executionRole: makeExecutionRole('ExecutionRole'), runtimePlatform });
    task.addVolume({ name: 'runtime-config' });
    const log = new logs.LogGroup(this, 'Logs', { retention: logs.RetentionDays.ONE_MONTH, removalPolicy: RemovalPolicy.RETAIN });
    const runtimeEnvironment = {
      GENUI_DATABASE_HOST: database.instanceEndpoint.hostname, GENUI_DATABASE_PORT: '5432', GENUI_DATABASE_NAME: 'genui',
      GENUI_EVIDENCE_BUCKET: bucket.bucketName, GENUI_DATABASE_ROLES_JSON: this.toJsonString(databaseRoles), GENUI_TENANTS_JSON: this.toJsonString(tenants),
      GENUI_RUNTIME_CONFIG_FILE: '/run/genui/credentials.json', GENUI_RDS_CA_FILE: '/etc/ssl/certs/rds-global-bundle.pem',
      FORMA_BIND_ADDRESS: '0.0.0.0', FORMA_PORT: '3000', AWS_REGION: this.region, PYTHONDONTWRITEBYTECODE: '1',
    };
    const container = task.addContainer('workspace', {
      image: ecs.ContainerImage.fromRegistry(image.valueAsString), readonlyRootFilesystem: true,
      user: '65532:65532',
      logging: ecs.LogDrivers.awsLogs({ streamPrefix: 'genui', logGroup: log }),
      environment: runtimeEnvironment,
      healthCheck: { command: ['CMD', 'python3', '-c', "import urllib.request; urllib.request.urlopen('http://127.0.0.1:3000/health/live', timeout=2).read()"], interval: Duration.seconds(30), timeout: Duration.seconds(5), retries: 3, startPeriod: Duration.seconds(60) },
    });
    container.addMountPoints({ sourceVolume: 'runtime-config', containerPath: '/run/genui', readOnly: false });
    container.addPortMappings({ containerPort: 3000 });

    const albSecurity = new ec2.SecurityGroup(this, 'AlbSecurity', { vpc, allowAllOutbound: false });
    albSecurity.addIngressRule(ec2.Peer.ipv4(Fn.conditionIf(publicIngress.logicalId, '0.0.0.0/0', operatorCidr.valueAsString).toString()), ec2.Port.tcp(443), 'Public beta HTTPS or the temporary operator-only trial');
    albSecurity.addEgressRule(taskSecurity, ec2.Port.tcp(3000), 'Application and readiness probes');
    taskSecurity.addIngressRule(albSecurity, ec2.Port.tcp(3000), 'Only the ALB reaches the application');
    const alb = new elbv2.ApplicationLoadBalancer(this, 'LoadBalancer', { vpc, internetFacing: true, vpcSubnets: { subnetType: ec2.SubnetType.PUBLIC }, securityGroup: albSecurity, dropInvalidHeaderFields: true });
    const service = new ecs.FargateService(this, 'Service', {
      cluster, taskDefinition: task, desiredCount: desiredCount.valueAsNumber,
      platformVersion: ecs.FargatePlatformVersion.VERSION1_4,
      assignPublicIp: true, vpcSubnets: { subnetType: ec2.SubnetType.PUBLIC }, securityGroups: [taskSecurity],
      enableExecuteCommand: false, circuitBreaker: { rollback: true }, healthCheckGracePeriod: Duration.seconds(90),
      minHealthyPercent: 100, maxHealthyPercent: 200,
    });
    const certificate = acm.Certificate.fromCertificateArn(this, 'Certificate', certificateArn.valueAsString);
    const listener = alb.addListener('Https', { port: 443, protocol: elbv2.ApplicationProtocol.HTTPS, certificates: [certificate], sslPolicy: elbv2.SslPolicy.RECOMMENDED_TLS, open: false });
    listener.addTargets('Application', { port: 3000, protocol: elbv2.ApplicationProtocol.HTTP, targets: [service], healthCheck: { path: '/health/ready', healthyHttpCodes: '200', interval: Duration.seconds(30), timeout: Duration.seconds(5), healthyThresholdCount: 2, unhealthyThresholdCount: 3 }, deregistrationDelay: Duration.seconds(30) });

    // One-time safety only: this stops service compute, not the retained billable
    // database, ALB, storage or keys. There is no automatic resume or destroy.
    const expiryGroup = new scheduler.CfnScheduleGroup(this, 'ExpiryScheduleGroup');
    const expiryRole = new iam.Role(this, 'ExpiryRole', {
      assumedBy: new iam.ServicePrincipal('scheduler.amazonaws.com', { conditions: {
        StringEquals: { 'aws:SourceAccount': this.account },
        ArnEquals: { 'aws:SourceArn': expiryGroup.attrArn },
      } }),
    });
    expiryRole.addToPolicy(new iam.PolicyStatement({ actions: ['ecs:UpdateService'], resources: [service.serviceArn], conditions: { ArnEquals: { 'ecs:cluster': cluster.clusterArn } } }));
    const expiry = new scheduler.CfnSchedule(this, 'DeploymentExpiry', {
      groupName: expiryGroup.ref, state: 'ENABLED',
      scheduleExpression: Fn.join('', ['at(', Fn.select(0, Fn.split('Z', expiresAt.valueAsString)), ')']), scheduleExpressionTimezone: 'UTC',
      flexibleTimeWindow: { mode: 'OFF' },
      target: {
        arn: Fn.join('', ['arn:', this.partition, ':scheduler:::aws-sdk:ecs:updateService']),
        roleArn: expiryRole.roleArn,
        input: this.toJsonString({ Cluster: cluster.clusterArn, Service: service.serviceArn, DesiredCount: 0 }),
        retryPolicy: { maximumEventAgeInSeconds: 900, maximumRetryAttempts: 3 },
      },
    });

    const migrationRole = new iam.Role(this, 'MigrationRole', { assumedBy: new iam.ServicePrincipal('ecs-tasks.amazonaws.com') });
    const migrationExecutionRole = makeExecutionRole('MigrationExecutionRole');
    const migration = new ecs.FargateTaskDefinition(this, 'MigrationTask', { cpu: 2048, memoryLimitMiB: 8192, taskRole: migrationRole, executionRole: migrationExecutionRole, runtimePlatform });
    migration.addContainer('migration', {
      image: ecs.ContainerImage.fromRegistry(image.valueAsString), readonlyRootFilesystem: true, user: '65532:65532',
      entryPoint: ['python3', '/app/runtime.py'], command: ['migrate'],
      logging: ecs.LogDrivers.awsLogs({ streamPrefix: 'genui-migration', logGroup: log }),
      environment: { PGHOST: database.instanceEndpoint.hostname, PGPORT: '5432', PGDATABASE: 'genui', PGUSER: 'genui_migration', PGSSLROOTCERT: '/etc/ssl/certs/rds-global-bundle.pem', PGSSLMODE: 'verify-full', PYTHONDONTWRITEBYTECODE: '1' },
      secrets: { PGPASSWORD: ecs.Secret.fromSecretsManager(database.secret!, 'password') },
    });
    new CfnOutput(this, 'DatabaseEndpoint', { value: database.instanceEndpoint.hostname });
    new CfnOutput(this, 'EvidenceBucket', { value: bucket.bucketName });
    new CfnOutput(this, 'ClusterName', { value: cluster.clusterName });
    new CfnOutput(this, 'TaskDefinition', { value: task.taskDefinitionArn });
    new CfnOutput(this, 'TaskSecurityGroup', { value: taskSecurity.securityGroupId });
    new CfnOutput(this, 'ServiceName', { value: service.serviceName });
    new CfnOutput(this, 'SelectedIngressMode', { value: ingressMode.valueAsString });
    new CfnOutput(this, 'ScheduledExpiryUtc', { value: expiresAt.valueAsString });
    new CfnOutput(this, 'DeploymentExpiryScheduleArn', { value: expiry.attrArn });
    new CfnOutput(this, 'DeploymentExpiryGroup', { value: expiryGroup.ref });
    new CfnOutput(this, 'DeploymentExpiryRoleArn', { value: expiryRole.roleArn });
    new CfnOutput(this, 'LoadBalancerDnsName', { value: alb.loadBalancerDnsName });
    new CfnOutput(this, 'MigrationTaskDefinition', { value: migration.taskDefinitionArn });
    new CfnOutput(this, 'MigrationTaskRoleArn', { value: migrationRole.roleArn });
    new CfnOutput(this, 'MigrationExecutionRoleArn', { value: migrationExecutionRole.roleArn });
    new CfnOutput(this, 'PublicTaskSubnets', { value: Fn.join(',', vpc.publicSubnets.map(subnet => subnet.subnetId)) });
    new CfnOutput(this, 'DatabaseRoleConfiguration', { value: this.toJsonString(databaseRoles) });
    new CfnOutput(this, 'TenantConfiguration', { value: this.toJsonString(tenants) });
  }
}
