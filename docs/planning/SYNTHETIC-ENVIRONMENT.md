# Synthetic environment preparation — APP-007

The delivered demo's D2 and D10 control this environment. This is a concrete setup brief, not an existing deployment or permission to reuse a production database. Local unit tests can proceed independently; managed integration acceptance needs the resource evidence below.

## Current operator decision

Victor selected `https://beta.getforma.dev`, public HTTPS from any network, and a **$100/month project limit** on 2026-10-28. His follow-up selects **invite-first signup**: public visitors may view the landing page and enter an invitation, but cannot register or use GenUI without a valid operator-issued invite. An invitation selects a preprovisioned isolated tenant on the server; signup creates the invited identity, and current membership plus active entitlement govern app access. Separate member invitations join an existing tenant. The owner controls invitation issuance, limits, expiry, revocation and suspension. Identity and application admission remain separate authorities.

The coordinated app callback is `https://beta.getforma.dev/auth/callback`; GenUI owns its implementation and FormaStack owns client registration and the hosted invite-only email/password flow. This is a selected contract, not a claim that the route is deployed. Public beta launch remains gated by actual registration, admission and endpoint-refusal evidence.

## Environment inputs

| Input | State |
|---|---|
| Dedicated account or authorized access profile | User directed discovery of the existing GateWasm AWS setup; verified: existing account 877573132739 / us-east-1; GateWasm application resources subsequently removed |
| Region | Existing configured account is us-east-1; proposed GenUI region in the current cost/setup record |
| Monthly spending cap and budget notifications owner | **$100/month approved by Victor**. The previous always-on Fargate estimate exceeds it. Use the [bounded Phase 1 operating plan](SYNTHETIC-ENVIRONMENT-COST.md); no always-on beta affordability is asserted. Victor owns the limit; the GenUI operator owns cost reconciliation and cleanup. |
| Named operator and access/cleanup responsibility | Victor is resource owner; GenUI lead executes and records the reviewed synthetic setup and cleanup. |
| Public app origin and registered fAuth callback | `https://beta.getforma.dev` and exact callback `https://beta.getforma.dev/auth/callback`. Porkbun DNS access works. The dedicated non-exportable ACM certificate is **ISSUED**, with successful DNS validation; deployed TLS still needs verification. The unrelated CDN certificate and all 23 pre-existing DNS records are unchanged. [Operator input/certificate receipt](../../evidence/APP-007/operator-inputs-2026-10-28.json). |
| Operator ingress network | Temporary trial uses the host's current public IPv4 `/32`, discovered with IPv4 explicitly selected. Desired public operating mode is HTTPS `0.0.0.0/0`; application admission, not a permanent IP allowlist, restricts use. |

The user asked us to find and assess the existing GateWasm AWS instance/account before requesting new access. Discovery used read-only AWS calls; the subsequently authorized GateWasm teardown is separately recorded. Independent Phase 1 work continues. Region, spending cap and operator will be reconciled against the actual existing setup. Never place access tokens, database passwords or signing keys in this file, tickets, fixtures or command evidence.

## Design-shaped resource plan

- One isolated synthetic PostgreSQL/RDS instance, initially single-AZ as the demo allows; migration owner distinct from non-owner workspace/control/data/worker login roles. No application role gets superuser or BYPASSRLS. APP-008 adds tenant schema, policies and tests.
- Versioned private S3 storage with per-organization prefixes, explicit encryption/key context, denied public access and two synthetic organization keys. APP-007 proves tenant access separation; import/query tickets establish actual revision writes and reads.
- The eventual single ECS task uses the design's 2 vCPU / 8 GiB envelope with distinct in-process role configuration; the VM alternative must be recorded explicitly. A local development server can exercise the same role interfaces against the selected managed stores without claiming deployment trials.
- Secrets supplied by the authorized runtime/operator channel. Separate database credentials per logical role even in one process; no credentials embedded in client assets or user/model inputs. This app does not own fAuth signing keys.
- Public TLS ingress for the selected beta hostname, with an explicit temporary operator-only mode for the network denial test. Preserve ALB-only application-port access and private database connectivity. Public network reachability grants no signup, tenant, query, record, import or model authority.
- Resource tags identify this app, synthetic environment, owner and cleanup date. Review a current cost estimate against the chosen cap before applying resource creation; see the linked current estimate and confirm the monthly budget before deployment.

## Reviewable setup and acceptance record

Before applying provisioning, produce the exact resource plan for the selected account/region, IAM permissions, network rules, estimated recurring cost and cleanup steps. An unknown outcome must be reconciled by resource identity, not by creating duplicates.

Retain sanitized account/resource identifiers, deployment/configuration revision, migration version, role connectivity results, and paired tenant tests: authorized own-object/own-row access and denied other-tenant access. Record negative missing-credential, wrong-region/key and unreachable-service behavior. A successful local fake or schema check cannot replace these results.

Cleanup targets only the named synthetic resources. Preserve required evidence, snapshots, versioned-object/deletion records and audit anchors according to the design before cleanup; destruction or key deletion has its own explicit operational decision. Failed provisioning rolls back only resources owned by this setup, never shared auth or another team's environment.

## Ownership and blockers

GenUI owns application infrastructure configuration, migrations and consumer checks. FormaStack owns FORMA-247 hosted fAuth and FORMA-248 signing-key custody. Coordinate app origin/callback and tenant/client identifiers through the handoff; neither team changes the other's repository or services implicitly.

FormaStack has supplied a [separate auth rehearsal proposal](../../evidence/APP-006/upstream-auth-source/hosted-rehearsal-proposal.txt): six hours of synthetic staging and cleanup. Its chat subsequently reports Victor approved that rehearsal with **$5 included inside the $100 monthly limit**, the exact origin/callback above, and an explicit readiness condition: begin only when GenUI's callback and test environment are ready for joint integration. The start signal remains pending; source preparation can continue. This is not a hard AWS billing cap or permission to recreate the deleted production environment. The received older proposal remains a historical snapshot; these later inputs control execution.

APP-007 stays incomplete until revised infrastructure review, bounded-window preflight and the actual managed checks pass. Account, hostname, ingress policy, operator and monthly limit are now supplied. Hosted-auth and invitation/admission evidence remain separate requirements; the user's choices do not claim those implementations exist.

## Infrastructure implementation choice

2026-10-28: GenUI will use AWS CDK in TypeScript, matching the existing AWS tooling. The target remains the design’s ECS/Fargate, RDS, S3 and KMS arrangement. App Runner is closed to new customers and receives no new features; existing service availability does not make it the new app baseline. GateWasm application resources were torn down at Victor’s request, so only account access/region can be reused automatically, not those deleted resources. CDK source and shared bootstrap remain.

GateWasm teardown completed 2026-10-28 19:29:59 UTC. Account access remains, but no old GateWasm database/network can be reused. See [teardown report](../../evidence/APP-007/gatewasm-teardown.md).

The user asked whether existing App Runner eligibility would make it easier. Existing customers can still create App Runner resources, but the selected baseline remains ECS/Fargate with CDK: the design includes background import/generation/query workers and explicit resource controls, while App Runner throttles idle request-processing CPU. This is a GenUI architecture choice, not a claim that existing App Runner customers have lost access. See [AWS availability notice](https://docs.aws.amazon.com/apprunner/latest/dg/apprunner-availability-change.html) and [App Runner FAQ](https://aws.amazon.com/apprunner/faqs/).

2026-10-28 follow-up discovery: `acm list-certificates` (issued, us-east-1) and `route53 list-hosted-zones` were read-only. No certificate, hosted zone or DNS record was created. The CDK template remains parameterized so deployment cannot invent a hostname/certificate or unrestricted ingress.
