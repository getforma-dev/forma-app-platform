# Synthetic environment and operating record — APP-007

The delivered demo's D2 and D10 control this isolated synthetic environment. It has now been deployed and exercised; the [managed evidence index](../../evidence/APP-007/managed/README.md) distinguishes actual results from later application gates. The [final operating receipt](../../evidence/APP-007/managed/final-operating-state.json) records all ten observed tasks stopped at 2026-10-29 01:04 UTC, with the source database and supporting infrastructure retained. APP-007 is accepted with independent review and all eight exact-source checks at `b925600`. This is not permission to reuse a production database or activate an unreviewed workload.

## Current operator decision

Victor selected `https://beta.getforma.dev`, public HTTPS from any network, and a **$100/month project limit** on 2026-10-28. His follow-up selects **invite-first signup**: public visitors may view the landing page and enter an invitation, but cannot register or use GenUI without a valid operator-issued invite. An invitation selects a preprovisioned isolated tenant on the server; signup creates the invited identity, and current membership plus active entitlement govern app access. Separate member invitations join an existing tenant. The owner controls invitation issuance, limits, expiry, revocation and suspension. Identity and application admission remain separate authorities.

The coordinated app callback is `https://beta.getforma.dev/auth/callback`; GenUI owns its implementation and FormaStack owns client registration and the hosted invite-only email/password flow. This is a selected contract, not a claim that the route is deployed. Public beta launch remains gated by actual registration, admission and endpoint-refusal evidence.

## Environment inputs

| Input | State |
|---|---|
| Dedicated account or authorized access profile | User directed discovery of the existing GateWasm AWS setup; verified: existing account 877573132739 / us-east-1; GateWasm application resources subsequently removed |
| Region | GenUI was deployed and qualified in us-east-1; exact identities are in the [configuration inventory](../../evidence/APP-007/managed/configuration-inventory.json). |
| Monthly spending cap and budget notifications owner | **$100/month approved by Victor**. The previous always-on Fargate estimate exceeds it. Use the [bounded Phase 1 operating plan](SYNTHETIC-ENVIRONMENT-COST.md); no always-on beta affordability is asserted. Victor owns the limit; the GenUI operator owns cost reconciliation and cleanup. |
| Named operator and access/cleanup responsibility | Victor is resource owner; GenUI lead executes and records the reviewed synthetic setup and cleanup. |
| Public app origin and registered fAuth callback | `https://beta.getforma.dev` and selected callback `https://beta.getforma.dev/auth/callback`. The dedicated non-exportable ACM certificate is **ISSUED**; [actual hostname TLS checks](../../evidence/APP-007/managed/network-tls.json) passed. Certificate preparation preserved the unrelated CDN certificate and all 23 existing DNS records; later beta routing is recorded separately. The deployed foundation still refuses the callback; hosted integration is not established. |
| Operator ingress network | The actual temporary operator `/32` trial allowed the execution host and denied the second network. Subsequent public HTTPS `0.0.0.0/0` passed from both networks. Private application/database boundaries stayed restricted. Application admission, not a permanent IP allowlist, must restrict future protected use. |

The user asked us to find and assess the existing GateWasm AWS instance/account before requesting new access. Discovery used read-only AWS calls; the subsequently authorized GateWasm teardown is separately recorded. Account, region, spending cap and operator were reconciled for the actual GenUI campaign. Reconcile retained resources and cumulative allowance before each further window. Never place access tokens, database passwords or signing keys in this file, tickets, fixtures or command evidence.

## Design-shaped resource plan

- One isolated synthetic PostgreSQL/RDS instance, initially single-AZ as the demo allows; migration owner distinct from non-owner workspace/control/data/worker login roles. No application role gets superuser or BYPASSRLS. APP-008 adds tenant schema, policies and tests.
- Versioned private S3 storage with per-organization prefixes, explicit encryption/key context, denied public access and two synthetic organization keys. APP-007 proves tenant access separation; import/query tickets establish actual revision writes and reads.
- The qualified ECS task uses the design's 2 vCPU / 8 GiB envelope, ARM64 and distinct in-process role configuration. The service is stopped between reviewed windows. A VM alternative would require an explicit new decision and qualification.
- Secrets supplied by the authorized runtime/operator channel. Separate database credentials per logical role even in one process; no credentials embedded in client assets or user/model inputs. This app does not own fAuth signing keys.
- Public TLS ingress for the selected beta hostname, with an explicit temporary operator-only mode for the network denial test. Preserve ALB-only application-port access and private database connectivity. Public network reachability grants no signup, tenant, query, record, import or model authority.
- Resource tags identify this app, synthetic environment, owner and cleanup date. Review a current cost estimate against the chosen cap before applying resource creation; see the linked current estimate and confirm the monthly budget before deployment.

## Reviewable setup and acceptance record

Before applying provisioning, produce the exact resource plan for the selected account/region, IAM permissions, network rules, estimated recurring cost and cleanup steps. An unknown outcome must be reconciled by resource identity, not by creating duplicates.

Retain sanitized account/resource identifiers, deployment/configuration revision, migration version, role connectivity results and paired tenant tests. APP-007 now records own-object/foreign-prefix/key results; APP-008 owns actual tenant-row/RLS tests. The [managed negative trial](../../evidence/APP-007/managed/runtime-negatives-managed.json) records missing-file, wrong-region, expired-file and unreachable-database refusals, each followed by restored positive connectivity/readiness. Region/expiry checks refuse at validation, not through an AWS expired-session response. A successful local fake or schema check cannot replace actual managed results.

The [initial-baseline recovery](../../evidence/APP-007/managed/initial-baseline-recovery.json) binds actual scheduled stop, same-qualified-image restart and another observed stop; the separate snapshot trial compared the full restored foundation before migration/reseed and removed its temporary resources. This is the selected first-deployment fallback, not a prior-image rollback or later full release campaign.

Cleanup targets only the named synthetic resources. Preserve required evidence, snapshots, versioned-object/deletion records and audit anchors according to the design before cleanup; destruction or key deletion has its own explicit operational decision. Failed provisioning rolls back only resources owned by this setup, never shared auth or another team's environment.

## Ownership and blockers

GenUI owns application infrastructure configuration, migrations and consumer checks. FormaStack owns FORMA-247 hosted fAuth and FORMA-248 signing-key custody. Coordinate app origin/callback and tenant/client identifiers through the handoff; neither team changes the other's repository or services implicitly.

FormaStack has supplied a [separate auth rehearsal proposal](../../evidence/APP-006/upstream-auth-source/hosted-rehearsal-proposal.txt): six hours of synthetic staging and cleanup. Its chat subsequently reports Victor approved that rehearsal with **$5 included inside the $100 monthly limit**, the exact origin/callback above, and an explicit readiness condition: begin only when GenUI's callback and test environment are ready for joint integration. The start signal remains pending; source preparation can continue. This is not a hard AWS billing cap or permission to recreate the deleted production environment. The received older proposal remains a historical snapshot; these later inputs control execution.

APP-007's managed proof and independent acceptance are retained; all eight [final source CI checks](../../evidence/APP-007/managed/ci.json) pass at `b925600`, landed on main. Keep the service stopped until a reviewed activation within a reconciled reservation. Hosted-auth, tenant-row authorization and invitation/admission evidence remain separate requirements; the user's choices and this foundation do not claim those implementations exist.

## Infrastructure implementation choice

2026-10-28: GenUI will use AWS CDK in TypeScript, matching the existing AWS tooling. The target remains the design’s ECS/Fargate, RDS, S3 and KMS arrangement. App Runner is closed to new customers and receives no new features; existing service availability does not make it the new app baseline. GateWasm application resources were torn down at Victor’s request, so only account access/region can be reused automatically, not those deleted resources. CDK source and shared bootstrap remain.

GateWasm teardown completed 2026-10-28 19:29:59 UTC. Account access remains, but no old GateWasm database/network can be reused. See [teardown report](../../evidence/APP-007/gatewasm-teardown.md).

The user asked whether existing App Runner eligibility would make it easier. Existing customers can still create App Runner resources, but the selected baseline remains ECS/Fargate with CDK: the design includes background import/generation/query workers and explicit resource controls, while App Runner throttles idle request-processing CPU. This is a GenUI architecture choice, not a claim that existing App Runner customers have lost access. See [AWS availability notice](https://docs.aws.amazon.com/apprunner/latest/dg/apprunner-availability-change.html) and [App Runner FAQ](https://aws.amazon.com/apprunner/faqs/).

Historical 2026-10-28 follow-up discovery: `acm list-certificates` (issued, us-east-1) and `route53 list-hosted-zones` were read-only. **During that discovery step**, no certificate, hosted zone or DNS record was created. Later authorized certificate/DNS/deployment actions have their own linked receipts above. The CDK template remains parameterized so a subsequent deployment cannot invent a hostname/certificate or unreviewed ingress.
