# GenUI infrastructure — AWS CDK / TypeScript

This is the locally reviewable APP-007 synthetic deployment model, selected 2026-10-28. It uses separately owned GenUI resources in the agreed existing AWS account. **No GenUI deployment or managed acceptance is established by this package.** The paid environment remains blocked on the recorded operator, spending decision and deployment inputs. The old GateWasm CDK source stays in its owning repository.

## Reproduce the local checks

From `infra/`, using Node 22 or later:

```sh
npm ci
npm run typecheck
npm test
npm run synth -- --quiet
```

Dependencies and CLI versions are exact and locked. Synthesis is account-agnostic and disables lookups; these commands do not create AWS resources. The ten template tests inspect network boundaries, role grants, identity configuration, storage policies and deployment settings. `cdk.out/` is disposable. Local image/credential-volume qualification belongs to the app's APP-007 checks; neither those checks nor synthesis prove live AWS connectivity, SQL grants or tenant authorization.

## Required deployment inputs

| Input | Required value and check |
| --- | --- |
| Account and region | The approved account/region recorded in the environment plan; verify deployed RDS engine availability, service quotas and current costs. |
| `Operator` | Named person responsible for the environment, migration, validation and cleanup. No default. |
| `MonthlyBudgetUsd` | Approved budget recorded as a parameter/tag. **This creates no AWS Budget alarm and no spending hard stop.** Monitoring and stop decisions remain an operator responsibility. No default. |
| `OperatorIpv4Cidr` | Actual public operator IPv4 network, preferably `/32`. The template accepts only `/24` through `/32` and rejects broad, private, loopback, link-local, multicast and placeholder ranges. No default; refresh deliberately if the operator's public address changes. |
| `TlsCertificateArn` | Existing, issued ACM certificate in this account/region, covering the operator-owned application hostname. Validate issuance and hostname ownership before deployment. The ARN pattern checks shape only. No default. |
| `ImageDigest` | Reviewed immutable ECR image URI ending in `@sha256:` and 64 lowercase hex digits. Verify the digest exists and is pullable; the repository is not created here. Cross-account repositories also need their owner-approved repository policy. |
| `ImageArchitecture` | `ARM64` or `X86_64`, matching that exact image artifact. No default. Locally qualified ARM64 and CI AMD64 images are distinct artifacts. Both app and migration tasks use this selection. |
| `ServiceDesiredCount` | Defaults to `0`; only `0` or `1` is accepted. Keep zero through initial migration, then set one for approved managed validation. |
| Hostname routing | Arrange a reviewed DNS alias/CNAME from the actual certificate hostname to `LoadBalancerDnsName` after provisioning. This stack creates no hostname, DNS record or certificate. Do not bypass TLS verification by calling the raw ALB hostname. |

Desired count zero still provisions a billable ALB, RDS and retained resources; it is not a free preview or teardown. No deploy command should run solely because synthesis succeeds. The [environment plan](../docs/planning/SYNTHETIC-ENVIRONMENT.md) owns approval and the current cost estimate.

## Network and credential boundaries

The NAT-free VPC has two public subnets for the HTTPS ALB/Fargate ENIs and private isolated database subnets. Operator CIDR → ALB `443`, ALB security group → task `3000`, and task security group → RDS `5432` are the only inbound rules. There is no HTTP listener, SSH, bastion, ECS Exec, NAT gateway or paid interface endpoint. The single task receives a public IPv4 address for outbound AWS/provider HTTPS; its security group still accepts inbound application traffic only from the ALB. HTTPS egress is not an application data-disclosure policy. The ALB forwards HTTP inside this VPC; database clients require TLS verification with the packaged RDS CA.

RDS PostgreSQL 16.13 uses an encrypted 20-GiB `db.t4g.small`, single AZ, IAM authentication, enforced SSL, seven-day backups and deletion protection. S3 is private, versioned and encrypted with organization keys. Both synthetic tenant UUIDs have their own assumable role, object prefix and KMS key; uploads must supply the matching key ARN **and** `aws:kms` algorithm. The tenant IDs are fixture configuration, never branches in platform business logic.

The runtime task role has only six role-assumption grants: four database roles and two tenant storage roles. Each database role can connect to this database only as its fixed `genui_workspace`, `genui_control`, `genui_data` or `genui_worker` user. Tenant roles permit only their own UUID prefix and key. The task has no direct database, S3, KMS or migration-secret grant. All six roles trust the runtime task role. A compromise of this one process can therefore assume all six roles: these are constrained logical credentials, not process isolation or proof of application authorization. SQL grants are supplied by the separate migration and must be checked on the managed database; later APP-008 owns the complete tenant/RLS foundation.

The application and migration execution roles are separate. Each may obtain an ECR authorization token (AWS requires `Resource: *` for that action), pull layers only from the repository derived from `ImageDigest`, and write the stack's log group. Only the migration execution role may retrieve the generated RDS migration secret. No execution role has ECS Exec, PassRole or application data authority. The runtime task never receives the migration password.

## Runtime interface

The reviewed image runs `python3 /app/runtime.py` with default command `serve`, then `/app/genui`, as UID/GID `65532:65532`. Both task definitions use 2 vCPUs and 8 GiB memory. The root filesystem is read-only. One task-local ephemeral volume is mounted writable at `/run/genui`; the image must initialize that path with owner 65532 and mode `0700`. The runtime helper atomically writes a mode-`0600` credential file there and refreshes assumed credentials. No host credential directory is mounted. ECS volume initialization and local Docker behavior must be qualified against the actual image; image paths alone are not evidence.

The template supplies only endpoints and identities:

- `GENUI_DATABASE_HOST`, `GENUI_DATABASE_PORT=5432`, `GENUI_DATABASE_NAME=genui`, `GENUI_EVIDENCE_BUCKET`, `AWS_REGION`.
- `GENUI_DATABASE_ROLES_JSON`: map of `workspace`, `control`, `data`, `worker` to their IAM role ARNs.
- `GENUI_TENANTS_JSON`: two entries with `organization_id`, `role_arn`, `kms_key_arn`, and `prefix` (`o/<organization UUID>/`).
- `GENUI_RUNTIME_CONFIG_FILE=/run/genui/credentials.json`, `GENUI_RDS_CA_FILE=/etc/ssl/certs/rds-global-bundle.pem`, `FORMA_BIND_ADDRESS=0.0.0.0`, `FORMA_PORT=3000`.

The helper obtains ECS task identity, assumes database and tenant roles for 900 seconds, and generates each IAM database token with its matching assumed credentials. Its published expiry is no later than ten minutes from issuance or sixty seconds before the STS expiration. The file contains temporary credentials and must never enter logs, stack parameters, context JSON, Git or evidence. The local helper tests own refresh/failure behavior. The container probes `/health/live`; the ALB uses `/health/ready`, so AWS readiness remains a separate managed validation.

## Initial migration and operator access

This sequence is a handoff plan, not a record that deployment ran:

1. Review the inputs, image/architecture, CDK diff, current estimate, cleanup plan and real certificate hostname. Provision only after the owning environment decision is complete, with `ServiceDesiredCount=0`.
2. Use stack outputs `ClusterName`, `MigrationTaskDefinition`, `PublicTaskSubnets` and `TaskSecurityGroup` to run exactly one migration task on Fargate Linux platform `1.4.0`, in those subnets, with that security group and public IP enabled for ECR/logs/Secrets Manager HTTPS. No public database endpoint or inbound migration port is added.
3. The task runs `python3 /app/runtime.py migrate` using the bundled `/app/migrations/0001_role_foundation.sql`. Its fixed environment is `PGHOST` from RDS, `PGPORT=5432`, `PGDATABASE=genui`, `PGUSER=genui_migration`, `PGSSLROOTCERT=/etc/ssl/certs/rds-global-bundle.pem`, and `PGSSLMODE=verify-full`. ECS injects only `PGPASSWORD` from the generated secret. Do not paste the secret into overrides or shell arguments. The migration task role has no application AWS grants.
4. Wait for that task to stop; inspect its exit code and sanitized `genui-migration` log stream. Stop on nonzero exit. Preserve migration version, task-definition revision and image digest in managed evidence without credentials. Verify actual SQL role attributes/grants and forbidden-role operations before starting the service.
5. Set the service to one task, configure the actual certificate hostname's DNS, wait for service/target readiness, and test TLS and both health endpoints from the recorded operator network. Verify forbidden public ingress from a different source. Collect managed per-role database, two-tenant S3/KMS, expiry/refusal and cleanup/rollback evidence before claiming integrated acceptance.

The operator needs a separate reviewed deployment identity and narrowly scoped migration invocation policy: `ecs:RunTask` on the exact migration task-definition revision, restricted to the output cluster; `iam:PassRole` only on `MigrationTaskRoleArn` and `MigrationExecutionRoleArn`, with `iam:PassedToService=ecs-tasks.amazonaws.com`; and the minimum task-status/log-read permissions needed to inspect that run. The migration invocation identity needs no `secretsmanager:GetSecretValue`. This stack does not silently grant an operator or add IAM policy to an existing identity. ECS Exec is disabled; enabling it would require a separately reviewed task configuration and policy.

## Rollback and remaining acceptance

Set desired count to zero to stop app compute. The service uses a deployment circuit breaker; its first deployment still needs explicit failure handling because no prior successful revision may exist. Subsequent image rollbacks must select a previously qualified digest **and matching architecture**. Stop/inspect failed migration tasks; do not infer that rolling back the image reverses database mutations. The initial migration's idempotency and role checks belong to the app harness, and future data migrations need an explicit restore/forward-fix plan.

Database, generated migration secret, S3 objects/versions, KMS keys and logs are retained on stack removal. Deletion protection also prevents accidental database removal. Plan snapshots/restore checks, secret retention, version cleanup and eventual key deletion explicitly; deleting the stack or setting count zero does not terminate all charges. Do not destroy keys while retained objects or database backups still require them.

Managed account connectivity, exact PostgreSQL grants, cross-tenant denials, readiness, snapshot/restore and cleanup/rollback are still required. Template assertions cannot establish these outcomes. No customer data or credentials belong in this synthetic environment's source or evidence.

References: [ECS execution-role separation and permissions](https://docs.aws.amazon.com/AmazonECS/latest/developerguide/task_execution_IAM_role.html), [Fargate task networking](https://docs.aws.amazon.com/AmazonECS/latest/developerguide/fargate-task-networking.html), [ECS bind mounts and ownership](https://docs.aws.amazon.com/AmazonECS/latest/developerguide/bind-mounts.html).
