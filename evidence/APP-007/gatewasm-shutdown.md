# GateWASM compute shutdown — 2026-10-28

Historical intermediate record: superseded by Victor’s subsequent [authorized teardown](gatewasm-teardown.md). The pause-only preservation rules below describe the earlier authorization, not the current teardown scope.

Victor explicitly requested: “stop or turn off the gatewasm resources etc so we don't get charged for that. Since we are not using it right now”. This authorizes a reversible shutdown of the identified GateWASM compute. It does not authorize deletion of databases, backups, images, buckets, keys, secrets or shared DNS.

Account `877573132739`, region `us-east-1`, existing AWS profile `default`. The prior [discovery](aws-discovery.md) describes the live environment before this shutdown. No GenUI cloud environment was provisioned.

## Ownership and before state

- App Runner `gatewasm-server`: RUNNING; exact ARN is the `AWS::AppRunner::Service` resource in `GatewasmServiceStack`.
- RDS `gatewasmstack-databaseinstanceaa8a5fde-vv99npqcxrxz`: available, owned by `GatewasmDatabaseStack`; no replicas; seven-day retained backups; latest restorable time before shutdown `2026-10-28T19:08:28Z`.
- NAT EC2 `i-02393d887b3bacdbc`: running, GateWASM fck-nat. The only EC2 instance in the VPC. Private default routes point to this instance; the VPC has the GateWASM App Runner connector and RDS interface. No other ECS cluster or VPC Lambda workload was found. The sole listed Lambda is a non-VPC CDK bucket-deployment helper.
- NAT Auto Scaling group `GatewasmStack-NetworkingVpcPublicSubnet1FckNatAsgASG05589C7A-qznP667LvRay`: owned by `GatewasmNetworkStack`; min/max/desired all 1; instance InService and Healthy; no suspended processes before this operation.
- Attached NAT root volume `vol-04c8627af76b62b2d`: 4 GiB gp3, retained. No allocated Elastic IP was returned by the regional address inventory; no address was released.

## Executed actions and current status

1. `aws apprunner pause-service` on the exact GateWASM ARN. Operation `6caa5ddafc214791b80f9bf8ad96f241` is **SUCCEEDED**, started `2026-10-28T19:12:36Z`, ended `2026-10-28T19:12:40Z`. Follow-up describe confirms **PAUSED**.
2. `aws rds stop-db-instance` on the exact GateWASM RDS identifier. Accepted, status **stopping**. Completion and automatic restart timestamp are pending; the API does not return an App Runner-style operation ID.
3. `aws autoscaling suspend-processes` on the NAT's exact group, explicitly suspending Launch, Terminate, AddToLoadBalancer, AlarmNotification, AZRebalance, HealthCheck, InstanceRefresh, ReplaceUnhealthy and ScheduledActions. Follow-up describe confirms all nine; min/max/desired stay 1. This prevents automated replacement/relaunch while the existing instance is stopped; no scaling-to-zero or termination occurred.
4. NAT instance stop is pending the database's completed shutdown. No force or skip-OS-shutdown option will be used.

Last database observation at `2026-10-28T19:14:29Z`: stopping, automatic restart time not yet returned. This record will be updated with final states before claiming shutdown complete.

## Billing and restart limits

Pausing App Runner reduces its compute to zero; the service keeps its identity and deployable source, while ephemeral instance state is lost. AWS bills its runtime only while running. [Pause behavior](https://docs.aws.amazon.com/apprunner/latest/dg/manage-pause.html), [App Runner pricing](https://aws.amazon.com/apprunner/pricing/).

Stopped RDS avoids DB instance-hour charges, but storage/backups remain billable. **RDS automatically restarts after seven consecutive stopped days.** No permanent stop is claimed and no restart-prevention automation was created in this operation. [RDS stop and billing behavior](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/USER_StopInstance.html).

Stopped EC2 avoids instance-hour charges; its EBS volume remains billable. [EC2 lifecycle billing](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/ec2-instance-lifecycle.html). Images, S3 objects, secrets, DNS and retained logs/backups were preserved and may continue costing money. [ECR storage pricing](https://aws.amazon.com/ecr/pricing/), [Secrets Manager pricing](https://aws.amazon.com/secrets-manager/pricing/). This shutdown does not promise a zero account bill and does not estimate a new monthly total.

## Restoration and team coordination

The fAuth environment is intentionally unavailable to save costs. FORMA-247/248 must distinguish this from an unexplained deployment outage and coordinate an authorized resumption window with the GenUI lead. Do not automatically resume it just because a test requires sign-in.

When a resumption is authorized:

1. Start the retained NAT EC2 instance; wait for running/system health and verify the original interface/routes and internet egress. Restore only the nine Auto Scaling process suspensions added here, after the instance is healthy. Prior state had no suspensions; capacities remain 1/1/1.
2. Start the retained RDS instance and wait until available. Verify its original private endpoint/security groups and perform the owning auth service's connectivity checks.
3. Resume the retained App Runner service; it resumes its last deployed version, not a newly pushed image. Wait for running and perform FORMA-247's health/sign-in/canary checks. A newer auth image needs its separately authorized deployment workflow.

No restore command has been run. No data reset, migration, deployment, secret read, credential copy, volume termination or resource deletion occurred.
