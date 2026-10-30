# GenUI infrastructure — AWS CDK / TypeScript

This is the APP-007 synthetic deployment model, revised 2026-10-28 for Victor's `beta.getforma.dev`, public HTTPS and $100/month decision. It preserves the full 2-vCPU/8-GiB Fargate envelope and private RDS/S3/KMS for a bounded Phase 1 window. **Local synthesis/tests establish no GenUI deployment or managed acceptance.** The old GateWasm CDK source stays in its owning repository. Always-on Fargate beta does not fit $100; this plan reserves retained resources and a short trial rather than promising continuous service at that price.

Actual [managed foundation results](../evidence/APP-007/managed/README.md) are now retained separately. The [final operating receipt](../evidence/APP-007/managed/final-operating-state.json) records all ten observed tasks stopped, seven standalone tasks totaling 325.86 seconds against 7,200 reserved seconds, and the unchanged $97.49 project reservation. Retained resources remain billable. Final acceptance, landing and exact-source CI remain pending; current documentation does not relabel the older deployed image or the separate APP-006 registry artifact.

## Reproduce the local checks

From `infra/`, using Node 22 or later:

```sh
npm ci
npm run typecheck
npm test
npm run synth -- --quiet
```

Dependencies and CLI versions are exact and locked. Synthesis is account-agnostic and disables lookups; these commands do not create AWS resources. Fifteen template tests inspect both ingress modes, fixed private boundaries, exact scheduler authority, role grants, storage, runtime settings and optional qualification-secret isolation. A separate Node test invokes the 22 Python preflight tests, including concurrent budget reservations, selected-secret binding and activation refusal. Run those directly with `python3 -m unittest discover -s test -p preflight_test.py -v`. Another Node wrapper runs the 14 local runtime-negative boundary tests with a 30-second process bound; their synthetic helper/native fixtures and local filesystem/process/HTTP checks make no managed calls. Run them directly with `python3 -B -m unittest discover -s test -p managed_runtime_negatives_test.py -v`. Both wrappers are included in `npm test`. `cdk.out/` is disposable. Local checks cannot prove live AWS connectivity, scheduled delivery, SQL grants or tenant authorization.

## Required deployment inputs

| Input | Required value and check |
| --- | --- |
| Account and region | The approved account/region recorded in the environment plan; verify deployed RDS engine availability, service quotas and current costs. |
| `Operator` | Named person responsible for the environment, migration, validation and cleanup. No default. |
| `MonthlyBudgetUsd` | Fixed at the approved `100`, recorded as a parameter/tag. **This creates no AWS Budget alarm and no spending hard stop.** The ledger reserves $70 for GenUI, at least $5 for separate auth and $25 unallocated. Model spending remains zero. |
| `IngressMode` | `public` by default; `operator` only for the temporary `/32` network trial. Public HTTPS does not authorize application usage; signup/invite/trial-code enforcement belongs to the app and must pass before expensive routes are exposed. |
| `OperatorIpv4Cidr` | Actual public operator IPv4 `/32`, still required for the short ingress trial. Broad/private/loopback/link-local/multicast/placeholder ranges are rejected. No default. |
| `DeploymentExpiresAtUtc` | Required real whole-second UTC time, `YYYY-MM-DDTHH:MM:SSZ`, 10 minutes to 48 hours in the future. Preflight checks the calendar and bound; CloudFormation's pattern alone cannot check real time. |
| `TlsCertificateArn` | Existing, issued ACM certificate in this account/region, covering the operator-owned application hostname. Validate issuance and hostname ownership before deployment. The ARN pattern checks shape only. No default. |
| `ImageDigest` | Reviewed immutable ECR image URI in account `877573132739`, `us-east-1`, ending in `@sha256:` and 64 lowercase hex digits. Verify the digest exists and is pullable; the repository is not created here. The preflight supports only this account/region even though the raw template's shape pattern is broader. |
| `ImageArchitecture` | `ARM64` or `X86_64`, matching that exact image artifact. No default. Locally qualified ARM64 and CI AMD64 images are distinct artifacts. Both app and migration tasks use this selection. |
| `QualificationConfigSecretArn` | Optional existing Secrets Manager ARN in this account/region under the bounded `genui-qualification-` name prefix. Empty disables both injection and read authority. Supply only an ARN, never JSON or credentials. Enabling it requires the separately reviewed runtime handoff and an operator-only synthetic configuration; it does not grant application admission. |
| `QualificationImageDigest` | Optional reviewed immutable ECR digest of the separate APP-075 qualification image built from `app/qualification/Dockerfile`, never the service image. Empty (the default) creates no `QualificationTask` definition, execution role or output. When supplied, the standalone task definition (1 vCPU/2 GiB, count zero: no service, schedule or target) runs `python3 /app/qualification_supervisor.py run` as UID 65532 on the runtime task role only, with an execution role that can pull only that repository and write logs; it receives no migration secret and exposes no port. The arming value and run manifest are supplied per run as environment overrides; nothing here authorizes or schedules a run (APP-078 owns the window). |
| `ServiceDesiredCount` | The supported template value is `0`, so initial creation starts no app task. An unrelated stack update may preserve an actual service count drifted to `1`; the template is not a stop command. Observe service/tasks stopped before replacing its image, window or stop schedule. Explicit `preflight.py --activate` checks the completed live stack, task revision and future stop schedule immediately before an ECS count-one request. |
| Hostname routing | Arrange the reviewed `beta.getforma.dev` DNS CNAME to `LoadBalancerDnsName`. This stack creates no DNS record or certificate. Do not bypass TLS verification by calling the raw ALB hostname. |

Desired count zero still provisions a billable ALB, RDS and retained resources; it is not a free preview or teardown. The [environment plan](../docs/planning/SYNTHETIC-ENVIRONMENT.md) owns the current operating decision and cost record.

## Bounded window and cumulative budget preflight

Run the following immediately before provisioning/updating, with the reviewed non-secret inputs and the existing private ledger **outside Git**:

```sh
python3 scripts/preflight.py --input /private/path/deployment.json --ledger /private/path/budget-ledger.json
```

Default mode makes no AWS calls. It atomically reserves the window in the owned mode-0600 ledger under an exclusive lock, then prints a receipt and exact CloudFormation parameter map. Missing ledgers are refused: the operator creates the first ledger only after reconciling existing resources/charges, records that evidence in `basis`, and never resets it to obtain another window. Reuse the same canonical UUID `window_id` after a lost response or failed deployment. Changing its immutable inputs or expiry is refused; only switching ingress mode and desired count within the already reserved window is permitted. Every new UUID adds a reservation. Concurrent invocations cannot consume the same remaining allowance. No reservation is automatically released; retain unknown outcomes until resource/run evidence supports reconciliation.

The input is a closed JSON object with these fields:

```json
{
  "window_id": "00000000-0000-4000-8000-000000000001",
  "account": "877573132739",
  "region": "us-east-1",
  "hostname": "beta.getforma.dev",
  "operator": "REPLACE_WITH_NAMED_OPERATOR",
  "ingress_mode": "operator",
  "operator_ipv4_cidr": "REPLACE_WITH_CURRENT_PUBLIC_IPV4/32",
  "deployment_expires_at_utc": "REPLACE_WITH_REAL_FUTURE_UTC_TIMESTAMP",
  "tls_certificate_arn": "REPLACE_WITH_ISSUED_BETA_CERTIFICATE_ARN",
  "image_digest": "REPLACE_WITH_REVIEWED_SAME_ACCOUNT_ECR_DIGEST",
  "image_architecture": "ARM64",
  "service_desired_count": 0,
  "extra_task_hours": 2
}
```

Use a fresh UUID for an actually new window; the example values are not deployment evidence. `extra_task_hours` reserves at least two aggregate task-hours for standalone migration/qualification and overlapping deployments. More hours need a larger explicit reservation. Each standalone task must have an observed exit/stop; the service expiry cannot stop it. The ledger contract is:

```json
{
  "schema_version": 1,
  "scope": "genui-beta-incremental",
  "months": {
    "YYYY-MM": {
      "reconciled_at_utc": "REPLACE_WITH_CURRENT_UTC_TIMESTAMP",
      "basis": "REPLACE_WITH_RESOURCE/BILLING/PRIOR_RUN_RECONCILIATION_REFERENCE",
      "prior_extra_spend_cents": 0,
      "variable_reserve_cents": 1000,
      "auth_spent_or_reserved_cents": 500,
      "unallocated_cents": 2500
    }
  },
  "windows": {}
}
```

The zero/empty values are valid only for a verified first environment with no previous relevant charges/reservations. For an existing environment, carry forward all `windows` and known extra spending. `prior_extra_spend_cents` covers relevant spending beyond the full retained-core baseline that is not already covered by a retained window reservation or the variable allowance; avoid omitting charges or counting the same reservation twice. Every affected month needs a reconciliation within 24 hours; crossing a month boundary reserves both months independently, including the entire extra-task allowance in each month because those tasks could run on either side. Record previous actual spend and unused-reservation reconciliation with its run/resource evidence before editing ledger entries. This script validates and persists the operator's accounting; it does not read AWS billing or independently prove the ledger truthful.

The conservative model reserves a **full calendar month** of RDS, ALB, both ALB IPv4 addresses, 20 GiB database storage and two tenant keys, even for a short window. At the checked rates this is $51.39 for a 730-hour illustration and **$52.29 for a 31-day month**. It adds the reserved active window, a one-hour stop/delivery margin, extra task-hours, all earlier window reservations and at least $10 for probes/requests/logs/secrets/image/object storage. Compute uses the x86 2-vCPU/8-GiB rate plus its IPv4 even for ARM images. A 48-hour window with two extra task-hours totals $68.49 for GenUI in a 31-day month before additional known spending; a shorter initial window leaves more room. GenUI must stay within $70 and the aggregate with the auth reservation and $25 unallocated within $100. These are operating estimates, not an AWS-enforced cap. Public traffic, billing lag, excess retention, uncontrolled standalone tasks or repeated deployments can exceed estimates; retain observed usage and stop/reconcile before exceeding the allowance. No budget notification address, active budget tag or AWS alert is invented.

After initial count-zero creation and migration are complete, set the **input** `service_desired_count` to `1` and use the explicit mutation path:

```sh
python3 scripts/preflight.py --input /private/path/deployment.json --ledger /private/path/budget-ledger.json --activate
```

Activation checks the actual account, completed `GenuiSyntheticFoundation` stack and parameter identity, exact enabled UTC stop schedule/group/role, zero-count target, retry bounds, current cluster/service/task revision and future expiry. Only then does it request desired count one, and it verifies the API result plus the current service count. The AWS CLI has one attempt, 5-second connect, 10-second read and a 20-second process limit per call. A late response after expiry triggers one bounded compensating stop. Any failure/unknown outcome retains its reservation and needs reconciliation; there is no automatic retry/resume. This starts neither migrations nor a new schedule and establishes no readiness. Never directly set count one through another command or deploy a stale preflight receipt. Refresh the preflight on every update and explicitly activate only after that update completes.

**Updating an existing stack:** an activation call changes ECS outside CloudFormation. The stored template still says zero; changing an unrelated ingress/schedule property does not necessarily reconcile that drift. Before changing the image, window/expiry or sole stop schedule, explicitly request desired count zero on the inventoried service, wait for service convergence, then inspect the actual service and tasks. Require `desiredCount=0`, `runningCount=0`, `pendingCount=0`, and no service-associated RUNNING task; inspect previously observed task ARNs until they are STOPPED. Retain those results before updating the template or reconciling unused compute reservations. Never replace the only future stop while service compute remains active. Do not infer stopped state from the CloudFormation parameter, a successful update response, or an empty health response. An ingress-only switch within the same image/window may preserve actual count one; verify its unchanged future stop and current count explicitly. The default trial procedure below uses an observed stop before that update too.

The one-time EventBridge Scheduler target requests **only this ECS service's desired count zero**, with a dedicated role allowed `ecs:UpdateService` on the exact service ARN and cluster, trusted only by the exact schedule group in this account. IAM does not constrain the UpdateService request's desired-count field; the reviewed target supplies zero. No Lambda, RDS control, data permissions, PassRole or wildcard resource authority is granted. Delivery has minute precision, up to three retries and a 900-second event-age limit; it is not guaranteed execution at an exact second. The scheduler API does not validate target business behavior at creation. Managed acceptance must actually observe a short scheduled stop, its service/task outcome and sanitized CloudTrail/metrics, then retain any failure. The schedule remains for review/cleanup and never resumes the service. RDS/ALB/IPv4/storage/keys continue charging after service compute stops; reconcile them at the next month and execute the scoped cleanup/retention plan.

### Database replacement guard for stack updates

Before updating the exact reviewed `GenuiSyntheticFoundation` stack identity, inspect and retain its existing stack policy. Preserve any existing protections; never overwrite an unrelated or stronger policy blindly. The reviewed [stack-policy.json](stack-policy.json) permits ordinary updates while explicitly denying `Update:Replace` and `Update:Delete` for `AWS::RDS::DBInstance`. Install the reviewed policy only on that verified stack, read it back, and execute the reviewed change set without a temporary policy override. A refused replacement is a failure to investigate, not a reason to weaken the guard. Retain the policy and verify the source database identity after the update. The [actual applied correction](../evidence/APP-007/managed/auto-minor-applied.json) records the matching policy hash, no override, unchanged source DB identity and PostgreSQL 16.13 with automatic minor upgrades disabled. This policy guards CloudFormation updates; it does not replace IAM, block direct RDS deletion or protect a whole-stack delete operation.

### Optional operator qualification configuration

The preflight input additionally accepts `qualification_config_secret_arn`. Omission or an empty string means disabled and preserves earlier disabled-window ledger hashes. Enabling, changing or removing a nonempty ARN changes an immutable window input; never overwrite a retained reservation to do that. The parameter report explicitly sets `QualificationConfigSecretArn`, including an empty value when disabled. Older deployed templates without this parameter are accepted for activation only as disabled.

Activation compares the actual stack parameter and task-definition revision, image digest, architecture and exact secret entry. Plain `GENUI_AUTH_QUALIFICATION_*` environment entries, environment files and task startup overrides are refused, so they cannot bypass that selection. The optional secret is injected only as `GENUI_AUTH_QUALIFICATION_CONFIG_JSON`. The reviewed runtime consumes it into an owned 0600 file, removes it from the child environment and cleans up only that file. The Rust service still validates the bounded rehearsal configuration and denies application admission. See [runtime interface, checks and limitations](../evidence/APP-006/hosted-callback/secret-wiring/README.md).

Observe the service and all its tasks stopped before changing image/configuration. Finish the complete stack update, including its conditional execution-role policy, before activation. The secret must already exist in the approved account/region and use the reviewed Secrets Manager default-key path; this template creates no secret or new KMS authority. Actual ECS delivery/private-file checks and hosted identity acceptance remain separate gates.

## Network and credential boundaries

The NAT-free VPC has two public subnets for the HTTPS ALB/Fargate ENIs and private isolated database subnets. Only ALB `443` switches between the recorded operator `/32` and public `0.0.0.0/0`; ALB security group → task `3000` and task security group → RDS `5432` remain the only other inbound rules. There is no HTTP listener, SSH, bastion, ECS Exec, NAT gateway or paid interface endpoint. The single task receives a public IPv4 address for outbound AWS/provider HTTPS; its security group still accepts inbound application traffic only from the ALB. HTTPS egress is not an application data-disclosure policy. The ALB forwards HTTP inside this VPC; database clients require TLS verification with the packaged RDS CA. Network reachability is not signup/invite authorization or tenant isolation.

RDS PostgreSQL 16.13 uses an encrypted 20-GiB `db.t4g.small`, single AZ, IAM authentication, enforced SSL, seven-day backups and deletion protection. Storage autoscaling is disabled by omitting its maximum parameter; setting the maximum equal to 20 GiB is an invalid AWS request. S3 is private, versioned and encrypted with organization keys. Both synthetic tenant UUIDs have their own assumable role, object prefix and KMS key; uploads must supply the matching key ARN **and** `aws:kms` algorithm. The tenant IDs are fixture configuration, never branches in platform business logic.

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

This sequence is the reusable operating procedure. The [retained actual migration receipt](../evidence/APP-007/managed/managed-migrations.json) is the evidence of the completed campaign; following these instructions again requires a reviewed window and fresh results:

1. Review the inputs, image/architecture, CDK diff, current estimate, cleanup plan and real certificate hostname. Reserve the short window with preflight and provision with `ServiceDesiredCount=0`; confirm the required stop schedule actually exists before activation.
2. Use stack outputs `ClusterName`, `MigrationTaskDefinition`, `PublicTaskSubnets` and `TaskSecurityGroup` to run exactly one migration task on Fargate Linux platform `1.4.0`, in those subnets, with that security group and public IP enabled for ECR/logs/Secrets Manager HTTPS. No public database endpoint or inbound migration port is added.
3. The task runs `python3 /app/runtime.py migrate` using the bundled `/app/migrations/0001_role_foundation.sql`. Its fixed environment is `PGHOST` from RDS, `PGPORT=5432`, `PGDATABASE=genui`, `PGUSER=genui_migration`, `PGSSLROOTCERT=/etc/ssl/certs/rds-global-bundle.pem`, and `PGSSLMODE=verify-full`. ECS injects only `PGPASSWORD` from the generated secret. Do not paste the secret into overrides or shell arguments. The migration task role has no application AWS grants.
4. Wait for that task to stop; inspect its exit code and sanitized `genui-migration` log stream. Stop on nonzero exit. Preserve migration version, task-definition revision and image digest in managed evidence without credentials. Verify actual SQL role attributes/grants and forbidden-role operations before starting the service.
5. Configure the actual certificate hostname's DNS and explicitly activate through the live-checked preflight path. In temporary `operator` mode, test TLS/health from the recorded `/32` and denial from another source. Immediately request/observe the service stopped as described above, return the template to `public` mode, and activate the same reserved window after the update completes. The template alone does not establish a stopped service. Verify public HTTPS separately from application refusals: uninvited, expired/revoked-code and unauthorized requests must not enqueue work or call a provider once those app features exist. No scaffold-only test passes that application gate. Collect managed per-role database, paired tenant S3/KMS, credential expiry/refresh, actual scheduled stop and cleanup/rollback evidence.

The operator needs a separate reviewed deployment identity and narrowly scoped migration invocation policy: `ecs:RunTask` on the exact migration task-definition revision, restricted to the output cluster; `iam:PassRole` only on `MigrationTaskRoleArn` and `MigrationExecutionRoleArn`, with `iam:PassedToService=ecs-tasks.amazonaws.com`; and the minimum task-status/log-read permissions needed to inspect that run. The migration invocation identity needs no `secretsmanager:GetSecretValue`. This stack does not silently grant an operator or add IAM policy to an existing identity. ECS Exec is disabled; enabling it would require a separately reviewed task configuration and policy.

## Rollback and remaining acceptance

Set desired count to zero to stop app compute. The service uses a deployment circuit breaker; its first deployment still needs explicit failure handling because no prior successful revision may exist. Subsequent image rollbacks must select a previously qualified digest **and matching architecture**. Stop/inspect failed migration tasks; do not infer that rolling back the image reverses database mutations. The initial migration's idempotency and role checks belong to the app harness, and future data migrations need an explicit restore/forward-fix plan.

Database, generated migration secret, S3 objects/versions, KMS keys and logs are retained on stack removal. Deletion protection also prevents accidental database removal. Plan snapshots/restore checks, secret retention, version cleanup and eventual key deletion explicitly; deleting the stack or setting count zero does not terminate all charges. Do not destroy keys while retained objects or database backups still require them.

The managed index retains actual account connectivity, foundation PostgreSQL grants, tenant storage denials, readiness/negative paths, snapshot restore and cleanup. [Initial-baseline recovery](../evidence/APP-007/managed/initial-baseline-recovery.json) is the selected first-deployment fallback; no earlier-image rollback or PH05 release qualification is claimed. Final acceptance and exact-source CI remain pending. Future image/configuration changes require the applicable fresh checks; template assertions cannot establish managed outcomes. No customer data or credentials belong in this synthetic environment's source or evidence.

References: [ECS execution-role separation and permissions](https://docs.aws.amazon.com/AmazonECS/latest/developerguide/task_execution_IAM_role.html), [Fargate task networking](https://docs.aws.amazon.com/AmazonECS/latest/developerguide/fargate-task-networking.html), [ECS bind mounts and ownership](https://docs.aws.amazon.com/AmazonECS/latest/developerguide/bind-mounts.html).

Scheduling references: [universal targets](https://docs.aws.amazon.com/scheduler/latest/UserGuide/managing-targets-universal.html), [one-time precision](https://docs.aws.amazon.com/scheduler/latest/UserGuide/schedule-types.html), [schedule-group trust](https://docs.aws.amazon.com/scheduler/latest/UserGuide/cross-service-confused-deputy-prevention.html), [ECS action/resource scope](https://docs.aws.amazon.com/service-authorization/latest/reference/list_ecs.html). Cost sources and exclusions remain in the [cost record](../docs/planning/SYNTHETIC-ENVIRONMENT-COST.md); the prior evidence is retained as historical qualification, not silently relabeled for these changes.
