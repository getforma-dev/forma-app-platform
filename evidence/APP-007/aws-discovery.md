# APP-007 — Existing AWS discovery

Historical snapshot before Victor’s later [authorized GateWASM teardown](gatewasm-teardown.md). Resource states below describe discovery time; consult the teardown record for the current state.

Date: 2026-10-28. Read-only discovery authorized by Victor: inspect the AWS environment previously configured for GateWASM and determine whether it can support GenUI. No infrastructure was provisioned or changed. No secret values, database contents, object contents, private keys or credentials are included here.

## Result

Existing AWS access works. AWS CLI 2.34.0 with the existing `default` profile authenticated as `arn:aws:iam::877573132739:user/victor` in account **877573132739**. Its configured and GateWASM-documented region is **us-east-1**. No new keys are needed for the completed discovery. Read access does not establish all future deployment permissions.

The account contains a live GateWASM environment, not a pre-existing isolated GenUI environment. Reuse the account/access configuration for planning. Do not repurpose the existing authentication database, CDN bucket, NAT instance, secrets or signing resources as GenUI storage/compute. Separate GenUI resources and roles still need a concrete plan, spending cap and operator record before provisioning.

## Observed resources

| Surface | Live observation | Consequence for GenUI |
|---|---|---|
| CloudFormation | `GatewasmNetworkStack`, `GatewasmDatabaseStack`, `GatewasmServiceStack`, `GatewasmCdnStack`, `CDKToolkit`; complete/update-complete | Existing stacks belong to GateWASM/CDK; do not redeploy them as the app scaffold. |
| Auth service | App Runner `gatewasm-server`, RUNNING | This is a live auth dependency owned by FormaStack/fAuth. Historical handoff prose describing App Runner sunset is not evidence it is stopped. |
| PostgreSQL | `gatewasmstack-databaseinstanceaa8a5fde-vv99npqcxrxz`, available, PostgreSQL 16.13, `db.t4g.micro`, 20 GiB, encrypted, private, single-AZ, seven-day backup retention, deletion protection | Auth-owned database; IAM database authentication is **disabled**. This is not the demo's planned `db.t4g.small` IAM-auth baseline and must not be silently adopted. No database connection or query was attempted. |
| Compute | EC2 `i-02393d887b3bacdbc`, running `t4g.nano`, in the GateWASM VPC | This is the fck-nat network instance: source configuration and both private subnet default routes identify its purpose. It is not spare application compute. |
| ECS | No cluster returned in us-east-1 | No existing ECS application environment found in the inspected region. |
| S3 | `cdk-hnb659fds-assets-877573132739-us-east-1`; `gatewasmcdnstack-cdnassetsc2a92191-icrhemaa0yt7` | Deployment assets and auth CDN assets; no dedicated GenUI evidence bucket found. |
| CDN bucket settings | Region us-east-1, versioning enabled, default encryption AES256/SSE-S3 | Versioning alone does not supply the design's isolated organization-key storage boundary. No objects or bucket policy contents were read. |
| KMS | Five existing keys; aliases are AWS-managed ACM, Lambda, RDS, S3 and Secrets Manager. RDS/S3 keys confirmed `KeyManager=AWS`, Enabled, ENCRYPT_DECRYPT | No customer-managed GenUI organization keys found in the inspected region. |

## Network candidate and collision boundary

GateWASM VPC: `vpc-01375b2f8d7f55ac2`.

- Private subnet `subnet-0102611073df0638e`, us-east-1a, 10.0.2.0/24, public-address assignment disabled.
- Private subnet `subnet-08be61795a85f0b2f`, us-east-1b, 10.0.3.0/24, public-address assignment disabled.
- Both private subnets route internet-bound traffic through the GateWASM NAT instance above.
- Existing RDS group `sg-0779b314f84b314d2` accepts PostgreSQL only from the App Runner VPC connector group and the documented NAT/SSM tunnel group. No public PostgreSQL ingress was observed.

These are identifiable private-network candidates, not approval to modify or assume control of them. Reusing this VPC would couple GenUI egress/availability to auth infrastructure and requires an explicit ownership decision. A separate GenUI stack with dedicated resource names, IAM roles, security groups, database and organization keys keeps collision boundaries reviewable. Do not alter auth ingress or use its database credentials to bypass that separation.

## Design and remaining decisions

APP-007 and demo D2 require managed PostgreSQL/RDS, versioned S3 and organization key handling from the first integrated synthetic environment. Demo D10 defaults to one ECS task, with a VM only as an explicit alternative. The design's compute envelope is 2 vCPU/8 GiB; its database cost model uses `db.t4g.small`, 20 GiB and IAM authentication's memory allowance. The existing auth micro/NAT/App Runner resources do not establish that envelope.

Known: account, region, a working administrative identity for read-only inventory and the existing auth ownership boundary. Still required: GenUI monthly spending cap, named operational owner, exact isolated resource plan/permissions, credential delivery, migration connectivity, two-tenant access evidence and cleanup/rollback. No APP-007 managed-environment acceptance has passed.

GateWASM's source names `victor@getforma.dev` as platform owner; this is not a newly assigned GenUI operator. Its RUNBOOK gives a historical roughly $23/month estimate and an App Runner two-instance scaling cap. Those are neither a current cost quote nor a GenUI spending authorization. AWS Budgets `describe-budgets` returned an empty response; no configured budget was found through that call. Cost Explorer was not queried.

## Sources and commands

Source repository inspected read-only: `getforma-dev/gatewasm`, local revision `b750ca534ae89066714f100eb2b76ee71edf1ab1`. Relevant files: `README.md`, `RUNBOOK.md`, `docs/handoff/README.md`, `infra/bin/gatewasm-infra.ts`, `infra/lib/constructs/{networking,database}.ts`, and `infra/lib/stacks/{network,database}-stack.ts`. No applicable AGENTS.md was found in that checkout or its inspected ancestors.

AWS commands used the existing `default` profile, explicit us-east-1 for regional APIs, disabled paging and bounded connect/read timeouts. Result queries limited output to resource identity/configuration:

```text
aws configure list-profiles
aws sts get-caller-identity
aws cloudformation list-stacks
aws cloudformation list-stack-resources --stack-name GatewasmDatabaseStack
aws cloudformation list-stack-resources --stack-name GatewasmCdnStack
aws rds describe-db-instances
aws ec2 describe-instances
aws ec2 describe-security-groups --group-ids sg-0779b314f84b314d2
aws ec2 describe-subnets --filters Name=vpc-id,Values=vpc-01375b2f8d7f55ac2
aws ec2 describe-route-tables --filters Name=vpc-id,Values=vpc-01375b2f8d7f55ac2
aws apprunner list-services
aws ecs list-clusters
aws s3api list-buckets
aws s3api get-bucket-location --bucket gatewasmcdnstack-cdnassetsc2a92191-icrhemaa0yt7
aws s3api get-bucket-versioning --bucket gatewasmcdnstack-cdnassetsc2a92191-icrhemaa0yt7
aws s3api get-bucket-encryption --bucket gatewasmcdnstack-cdnassetsc2a92191-icrhemaa0yt7
aws kms list-aliases
aws kms list-keys
aws kms describe-key --key-id <observed AWS RDS/S3 key id>
aws budgets describe-budgets --account-id 877573132739
```

The existing profile's region and configured credential field names were inspected without emitting values. No Secrets Manager GetSecretValue, SSM parameter value retrieval, database login, S3 object read, deployment, tunnel, IAM change or paid resource creation ran. This inventory covers the documented GateWASM region and global bucket/budget listings; it is not an all-region account audit.
