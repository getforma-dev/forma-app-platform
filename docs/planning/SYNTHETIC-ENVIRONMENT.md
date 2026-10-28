# Synthetic environment preparation — APP-007

The delivered demo's D2 and D10 control this environment. This is a concrete setup brief, not an existing deployment or permission to reuse a production database. Local unit tests can proceed independently; managed integration acceptance needs the resource evidence below.

## Inputs still needed

| Input | State |
|---|---|
| Dedicated account or authorized access profile | User directed discovery of the existing GateWasm AWS setup; verified: existing account 877573132739 / us-east-1; GateWasm application resources subsequently removed |
| Region | Existing configured account is us-east-1; proposed GenUI region in the current cost/setup record |
| Monthly spending cap and budget notifications owner | Awaiting Victor’s selection; [current estimate](SYNTHETIC-ENVIRONMENT-COST.md) is $150–170/month for the proposed always-on baseline |
| Named operator and access/cleanup responsibility | Victor proposed as owner; GenUI lead executes only the reviewed synthetic setup |
| Public app origin and registered fAuth callback | Still needed. Read-only discovery found no Route 53 hosted zone and only an issued `cdn.getforma.dev` certificate in us-east-1; that CDN identity does not cover a GenUI hostname and will not be repurposed. Coordinate the chosen app hostname/certificate and callback with FormaStack FORMA-247 |
| Operator ingress network | A real operator public IPv4 CIDR (/24–/32) is required by the CDK deployment; no broad placeholder/default ingress |

The user asked us to find and assess the existing GateWasm AWS instance/account before requesting new access. Discovery used read-only AWS calls; the subsequently authorized GateWasm teardown is separately recorded. Independent Phase 1 work continues. Region, spending cap and operator will be reconciled against the actual existing setup. Never place access tokens, database passwords or signing keys in this file, tickets, fixtures or command evidence.

## Design-shaped resource plan

- One isolated synthetic PostgreSQL/RDS instance, initially single-AZ as the demo allows; migration owner distinct from non-owner workspace/control/data/worker login roles. No application role gets superuser or BYPASSRLS. APP-008 adds tenant schema, policies and tests.
- Versioned private S3 storage with per-organization prefixes, explicit encryption/key context, denied public access and two synthetic organization keys. APP-007 proves tenant access separation; import/query tickets establish actual revision writes and reads.
- The eventual single ECS task uses the design's 2 vCPU / 8 GiB envelope with distinct in-process role configuration; the VM alternative must be recorded explicitly. A local development server can exercise the same role interfaces against the selected managed stores without claiming deployment trials.
- Secrets supplied by the authorized runtime/operator channel. Separate database credentials per logical role even in one process; no credentials embedded in client assets or user/model inputs. This app does not own fAuth signing keys.
- Restricted inbound access for the operator and recording/test machine, TLS at the approved ingress, explicit database/security-group connectivity and bounded egress. No broad public database rule or production ingress change.
- Resource tags identify this app, synthetic environment, owner and cleanup date. Review a current cost estimate against the chosen cap before applying resource creation; see the linked current estimate and confirm the monthly budget before deployment.

## Reviewable setup and acceptance record

Before applying provisioning, produce the exact resource plan for the selected account/region, IAM permissions, network rules, estimated recurring cost and cleanup steps. An unknown outcome must be reconciled by resource identity, not by creating duplicates.

Retain sanitized account/resource identifiers, deployment/configuration revision, migration version, role connectivity results, and paired tenant tests: authorized own-object/own-row access and denied other-tenant access. Record negative missing-credential, wrong-region/key and unreachable-service behavior. A successful local fake or schema check cannot replace these results.

Cleanup targets only the named synthetic resources. Preserve required evidence, snapshots, versioned-object/deletion records and audit anchors according to the design before cleanup; destruction or key deletion has its own explicit operational decision. Failed provisioning rolls back only resources owned by this setup, never shared auth or another team's environment.

## Ownership and blockers

GenUI owns application infrastructure configuration, migrations and consumer checks. FormaStack owns FORMA-247 hosted fAuth and FORMA-248 signing-key custody. Coordinate app origin/callback and tenant/client identifiers through the handoff; neither team changes the other's repository or services implicitly.

APP-007 stays incomplete until the managed environment checks pass. Missing account/access/budget/operator details block that acceptance specifically; they do not block APP-002/004/005 or close the FormaStack gates by implication.

## Infrastructure implementation choice

2026-10-28: GenUI will use AWS CDK in TypeScript, matching the existing AWS tooling. The target remains the design’s ECS/Fargate, RDS, S3 and KMS arrangement. App Runner is closed to new customers and receives no new features; existing service availability does not make it the new app baseline. GateWasm application resources were torn down at Victor’s request, so only account access/region can be reused automatically, not those deleted resources. CDK source and shared bootstrap remain.

GateWasm teardown completed 2026-10-28 19:29:59 UTC. Account access remains, but no old GateWasm database/network can be reused. See [teardown report](../../evidence/APP-007/gatewasm-teardown.md).

The user asked whether existing App Runner eligibility would make it easier. Existing customers can still create App Runner resources, but the selected baseline remains ECS/Fargate with CDK: the design includes background import/generation/query workers and explicit resource controls, while App Runner throttles idle request-processing CPU. This is a GenUI architecture choice, not a claim that existing App Runner customers have lost access. See [AWS availability notice](https://docs.aws.amazon.com/apprunner/latest/dg/apprunner-availability-change.html) and [App Runner FAQ](https://aws.amazon.com/apprunner/faqs/).

2026-10-28 follow-up discovery: `acm list-certificates` (issued, us-east-1) and `route53 list-hosted-zones` were read-only. No certificate, hosted zone or DNS record was created. The CDK template remains parameterized so deployment cannot invent a hostname/certificate or unrestricted ingress.
