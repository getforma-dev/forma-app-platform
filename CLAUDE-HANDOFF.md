# GenUI handoff to Claude

## 2026-10-29 Claude takeover status (read this before the Codex text below)

Claude took over the GenUI lane on 2026-10-29 and is the single GenUI operator. The FormaStack/fAuth lane is the Claude session titled "FormaStack/fAuth prerequisites for GenUI" (`local_4691ee0e-47d3-478e-964c-a24bf5ce30a1`); coordination so far is recorded in [CROSS-TEAM-HANDOFF](docs/planning/CROSS-TEAM-HANDOFF.md). Victor will switch this session's model after planning; the session remains the owner.

What changed today:

- [ADR-005](docs/architecture/ADR-005-PHASE-1-SEQUENCING.md) restructured the dependency graph (it was circular and serialized behind cloud windows): APP-008 moved to In review on its local acceptance; **APP-075** (managed tenant qualification on RDS) and **APP-076** (integrated managed trial) were added, so PH01 now has 20 tickets (APP-075 was split the same day into APP-075/077/078 after independent review); APP-009 depends on APP-008/073 and consumes the pinned identity contract; APP-012 is local-first; the hosted items of APP-012/073 live in APP-076. Nothing was weakened; every moved sentence is verbatim in its new owner.
- [ADR-006](docs/architecture/ADR-006-SIGNIN-AND-VERIFICATION-PENDING.md) fixes the sign-in, verification-pending and backend-resend route contract FormaStack needed; FormaStack has implemented its side on an unmerged branch and supplied exact lifetimes.
- [PHASE-1-EXECUTION-PLAN](docs/planning/PHASE-1-EXECUTION-PLAN.md) is the step-by-step plan for the orchestrator, including the ADR-007 decisions (write authority, schema layout, query execution) that must be recorded before the first new migration.
- Fresh read-only AWS snapshot at 17:40:59 UTC: service 0/0/0, no tasks, RDS available 16.13, both ECR indices unchanged, ledger hash unchanged. Actual October budget headroom is $2.51 (the $25 "unallocated" is a fixed reserve preflight requires); the ledger 24-hour rule lapses 22:39:50 UTC today.
- Victor directed the repository to become **public with an all-rights-reserved notice** so GitHub Actions run free of the organization billing block; it returns to private after deployment. The "keep private" instruction below is superseded. Ten workflows ran on the pushed tip `a0a75a8`: nine passed; Synthetic fixture qualification (APP-011's) failed its parser-WASM digest check (`fixtures/qualify.py:50`, a cross-host reproducibility issue under repair), so APP-011 still waits.
- [ADR-008](docs/architecture/ADR-008-YC-DEMO-AND-BETA-MVP.md) records Victor's founder disposition of 2026-10-29 ([record](evidence/APP-006/founder-disposition-2026-10-29.md)): the joint hosted identity rehearsal is deferred, FORMA-247/248 are cleared on merged source, and the MVP is the Northstar six-act demo plus the same build left running as an invite-first beta at beta.getforma.dev. New tickets: **APP-079** (PH06, deferred hosted identity acceptance, Blocked on FORMA-365) and **APP-080** (PH05, RDS recovery/lifecycle/concurrency/drift rows). APP-006 is In progress on the hash-bound merged contract; APP-074 is the local public-entry ticket; APP-076 is the beta go-live trial, Blocked on FORMA-366 (qualified mail image). Victor still has to choose the always-on beta figure (about $185–205/month including lean fAuth) and the mail transport.

The dependency order below ("APP-009 needs APP-006/008/073…") is superseded by ADR-005; the board (`python3 Tickets/_tools/board.py next`) is authoritative.

### State at the Fable → Opus switch (2026-10-29, evening UTC)

Planning is complete; execution continues in the same session on Opus 5.5 High with ultracode off. Landed on `phase-one-spikes` and pushed: ADR-008 and the restructure (`5551421`, `ebfb8fb`), ADR-007 revised after its independent review (`5fb4766`), the migration runner generalization (plan Task 3, merged `a1d072a`), and the fixtures-workflow binding fix (merged `4931aef`). Victor approved FORMA-367 (lean always-on fAuth, about $33/month, SES SMTP) to the FormaStack session, which deploys as soon as its image is qualified; GenUI's own always-on figure (about $150–170/month, ADR-008) is still his to approve.

Next steps, in order:

1. Read `evidence/APP-006/adr-008-restructure-review.md` when the reviewer finishes; fix any finding; then move APP-006 to In review (its fifth acceptance item).
2. Watch the CI runs on the pushed tip; when the fixtures workflow passes, record the run in APP-011, move APP-011 to In review, and open the PR `phase-one-spikes` → `main` that lands APP-008 and APP-011 (plan Task 1); after merge, move both to Done and set their Done revisions.
3. Read the Task 3 runner review (`evidence/APP-073/runner-review.md`) when it arrives; fix findings before Task 4 consumes the runner.
4. The admission encoder agent (plan Task 6 part 1) is still working in `.claude/worktrees/agent-a603496511c5518f9`; review and merge its commit when it reports.
5. Task 4 (migration 0003) strictly per ADR-007 v2 (D7.1 six functions, D7.2 two indexes, D7.8 heads and policies, D7.12 cap move and A3 normalization).
6. Phase 2: write ADR-009 (parser packaging vs the Rust 1.85 floor, the new contract edition, query execution model, APP-027 split) and `docs/planning/PHASE-2-EXECUTION-PLAN.md` from the session scratchpad file `ph02-map.md` (session df0e31da).
7. FormaStack will send DNS records for App Runner, ACM validation and SES DKIM/SPF/DMARC: create them at Porkbun by API if the key is available, otherwise hand the exact records to Victor; send the APP-009 sign-in handles when they exist.



Prepared October 29, 2026, at Victor's request to wind down because Codex usage is nearly exhausted. **Phase 1 is not complete.** This is the GenUI lane. A separate FormaStack/fAuth agent is preparing its own Claude handoff; keep the two ownership boundaries below.

## Read this first

1. Use the existing local checkout at `/Users/victorvillacis/dev/FormaBuiltApps/forma-app-platform`, branch `phase-one-spikes`. It contains reviewed commits newer than GitHub. Do not start from a fresh remote clone and assume it contains this handoff or the latest work.
2. Read [AGENTS.md](AGENTS.md), [ticket instructions](Tickets/README.md), [implementation protocol](Tickets/_tools/IMPLEMENT.md), [review protocol](Tickets/_tools/REVIEW.md), and [quality gates](docs/planning/QUALITY-GATES.md). The applicable skills are `/Users/victorvillacis/dev/FormaStackDev/.agents/skills/forma-stack/SKILL.md` and `/Users/victorvillacis/.codex/skills/forma-workspace/SKILL.md`.
3. **No hosted auth rehearsal is running.** The proposed October 29 window **17:30–23:30 UTC, with stop at22:00 UTC, was withdrawn** before any reservation, provisioning, secret creation, upload, stack update or activation. Do not reuse it or interpret old readiness prose as a start signal.
4. At the latest actual AWS observation, **17:12 UTC / 1:12 PM Eastern**, GenUI service counts were0/0/0 and the cluster had no RUNNING/PENDING task. The original protected RDS16.13 database and old schema1 foundation remain. Retained RDS/ALB/storage/keys still cost money. The old stop schedule expired; it never restarts compute.
5. GitHub CI still refuses jobs before execution for an account payment/spending-limit issue. A single planning retry at17:08 UTC confirmed this. Keep the repository private; publishing it is neither required nor authorized. Fix the organization billing condition before claiming successful current-source CI.
6. There is a **new, unfinished test-only APP-008 managed-entry draft** from the final wind-down. Read its handoff/review before using it. It is not a managed-ready runner, deployed code or accepted ticket result.

The full objective is to complete Phase1, then produce an accepted Phase2 handoff. Victor's wind-down request pauses execution; this document does not authorize a new paid trial by itself. A successor should explicitly establish that it is taking over execution, inspect current state, and coordinate a fresh window with the FormaStack owner.

## Ownership and the other agent

| Lane | Owner and authoritative workspace | Boundary |
| --- | --- | --- |
| GenUI | This repository; private GitHub `getforma-dev/forma-app-platform`; Codex chat ID `01a18398-5fb1-7e22-82a6-141b013a1c3e` | APP tickets, app contracts/code, tenant schema, storage, app invitations/admission, sessions, UI, integration checks, GenUI AWS/CDK, budget ledger and beta DNS coordination. |
| FormaStack/fAuth | Codex chat **Complete Forma auth tickets**, ID `01a18408-d9ee-7501-98ba-83b062f4d121`; authoritative FORMA board under `/Users/victorvillacis/dev/Forma/Tickets`; active fAuth checkout `/Users/victorvillacis/dev/Forma/work/FORMA-247/gatewasm` | Framework fixes and FORMA-247/248 auth source, hosted identity, signing-key custody/rotation, provider invitation/PAR and mail verification. Do not edit their checkout from this lane. |

The existing [cross-team handoff](docs/planning/CROSS-TEAM-HANDOFF.md) and `/Users/victorvillacis/dev/Forma/Tickets/journeys/genui-platform-2026-10-28/README.md` explain the joint journey. Their hosted plan is `/Users/victorvillacis/dev/Forma/work/FORMA-247/HOSTED-REHEARSAL-PLAN.md`; their live preparation record is `/Users/victorvillacis/dev/Forma/work/FORMA-247/rehearsal-readiness.json`. Their authoritative Claude handoff is **`/Users/victorvillacis/dev/Forma/work/CLAUDE-HANDOFF-2026-10-29.md`**. Give that file to the separate FormaStack/fAuth Claude agent; give this file to the GenUI agent.

FORMA-232/246 consumer gates are cleared. Published Forma3.2.0 fits this app's unchanged measured budgets; FORMA-232's separate configurable-budget package is not thereby claimed released. FORMA-247 hosted signup/identity remains open. FORMA-248 owns custody/rotation and later real-organization readiness; do not accidentally make every local GenUI task depend on it.

Victor authorized coordination with that other chat. Both sides have been told to hold the joint trial during transfer. Avoid duplicate agents deploying, changing secrets, rotating keys, running schema operations or editing another team's tickets.

## User decisions that must survive transfer

- Build the designed app on Forma3.2.0. Source designs were copied from `design-doc-factory`; frozen copies and provenance are under [design](design). Follow the delivered design and [source precedence guide](docs/planning/SOURCE-GUIDE.md), not older assumptions that upstream work was already done.
- Use the existing `Tickets/` folder pattern. Ticket Markdown is authoritative; GitHub is the source host, not a second issue tracker. The board has74 tickets, six phases,12 sprints and80 mapped requirements.
- Public HTTPS at **`https://beta.getforma.dev`**, exact OAuth callback **`https://beta.getforma.dev/auth/callback`**. `getforma.dev` is managed through Porkbun and the main site is on OVH. Do not change unrelated website/DNS records.
- **Invite-first email/password registration**, not open signup. Operator-preprovisioned isolated organizations; explicit member invitations join an existing selected organization. Unrelated trial users must not share a generic beta tenant. Identity is separate from GenUI membership and trial entitlement.
- Keep fAuth as the identity provider behind a simple login UI. It is not yet fully qualified for external beta. A shared HTTP Basic Auth password does not implement the required identity/admission/session controls.
- GenUI uses TypeScript AWS CDK, ECS Fargate2vCPU/8GiB ARM64, private PostgreSQL RDS, S3 and tenant KMS keys. Existing-customer App Runner is the separate fAuth team's selected staging path.
- **$100/month total**. The approved auth trial is at most six hours with **$5 already included in that total**, not an extra allowance. Provisioning, tests and cleanup all fit inside its interval. Public ingress stays HTTPS0.0.0.0/0; any operator `/32` check is temporary, not the beta access control.
- GateWasm's old unused application resources were explicitly authorized for teardown because they held no real data. That teardown is complete; preserve shared bootstrap and source/CDK. Do not restore deleted production stacks to conduct the new staging trial.
- No model calls before APP-043. No real customer data or external-beta readiness claim from synthetic/local tests.

## Actual Phase1 status

*2026-10-29 (Claude):* ADR-005 adds APP-075/076/077/078; PH01 is now 20 tickets and the board 78; APP-008 is In review. The counts and the In-progress row below are the transfer-time snapshot.

The [Phase1 board](Tickets/phases/PH01.md) is authoritative. **Six of16 tickets are Done; ten remain.** Ticket counts are not a percentage of effort. Most end-to-end product integration is still ahead.

| Status | Tickets | What is actually established |
| --- | --- | --- |
| Done | APP-001–005, APP-007 | Published framework baseline; frozen contracts; native canvas; confined CSV/XLSX parser; bounded query spike; actual managed cloud foundation/cleanup/recovery. Each has independent evidence and accepted CI/landing. |
| Blocked | APP-006 | Reviewed/local identity verifier, callback and key qualifier exist, with previously passing CI and published images. Actual hosted identity/signup/mailbox/return and consumer acceptance remain open. |
| In progress | APP-008 | Real local schema/RLS/pools/authorization and image tests passed. Current-source CI/landing and actual managed schema2 qualification remain open. New managed-entry draft is unaccepted. |
| Blocked | APP-011 | Original source fixtures/oracles/parser/query/reset package passed local review. CI/account billing and landing remain open; do not label this Done. |
| Backlog | APP-073,009,074,010,012,013,014 | Invitations/entitlements, sessions, public admission, persisted definitions/work, authorized query/receipt, atomic saved decision, retries/conflicts/races. Their required production paths have not been implemented. |

Next dependency order: finish APP-008; then APP-073 and APP-010 can progress independently. APP-009 needs APP-006/008/073, APP-074 needs APP-006/009/073, APP-012 needs APP-003/005/009/010/011/074; APP-013 follows012 and APP-014 follows013. Keep normal predecessor waiting in Backlog; label concrete operational blockers with the responsible owner.

Phase1 must prove **invite → verified sign-in → current tenant entitlement → authorized query → visible receipt → explicit command → atomic outcome/audit → restart/reopen**, including the required denial, replay, concurrency and failure cases. No Phase1 acceptance file exists. [Phase2 handoff](docs/planning/PHASE-2-HANDOFF.md) remains **NOT READY**; do not mark the goal or phase complete before all16 tickets and the integrated exit gate pass.

## Source and evidence map

| Need | Read |
| --- | --- |
| Architecture and decisions | [Framework baseline](docs/architecture/FRAMEWORK-BASELINE.md), [ADR-002 contract edition](docs/architecture/ADR-002-CONTRACT-EDITION.md), [ADR-003 public beta access](docs/architecture/ADR-003-PUBLIC-BETA-ACCESS.md), [ADR-004 tenant transactions](docs/architecture/ADR-004-TENANT-TRANSACTIONS.md) |
| Design requirements and phases | [Source guide](docs/planning/SOURCE-GUIDE.md), [roadmap](docs/planning/ROADMAP.md), [quality gates](docs/planning/QUALITY-GATES.md), [ticket index](Tickets/TRACKER.md), [external gates](Tickets/external-dependencies.json) |
| Runtime and infrastructure | [App runbook](app/README.md), [infra runbook](infra/README.md), [environment](docs/planning/SYNTHETIC-ENVIRONMENT.md), [cost record](docs/planning/SYNTHETIC-ENVIRONMENT-COST.md) |
| Completed cloud foundation | [APP-007 managed evidence](evidence/APP-007/managed/README.md), [GateWasm teardown](evidence/APP-007/gatewasm-teardown.md) |
| Local tenant implementation | [APP-008 ticket](Tickets/3-in-review/APP-008.md), [schema evidence](evidence/APP-008/schema/README.md), [native evidence](evidence/APP-008/native/README.md), [final native R5 review](evidence/APP-008/native/r5/independent-review.md), [schema compatibility review](evidence/APP-008/schema/independent-compatibility-review.md) |
| Managed tenant work still needed | [Full managed plan](evidence/APP-008/managed-plan.md), [preparation slices](evidence/APP-008/managed-harness-preparation.md), [accepted shared-case extraction](evidence/APP-008/native/shared-cases/README.md), [unfinished entry handoff](evidence/APP-008/native/managed-entry/HANDOFF.md), [incomplete independent review](evidence/APP-008/native/managed-entry/independent-handoff-review.md) |
| Fixtures | [Fixture package](fixtures/README.md), [APP-011 independent review](evidence/APP-011/independent-review.md), [final reset receipt](evidence/APP-011/reset-independent-catalog-path-final/summary.json) |
| Auth consumer | [Verifier spike](spikes/APP-006/README.md), [callback contract](evidence/APP-006/hosted-callback/README.md), [private secret transport](evidence/APP-006/hosted-callback/secret-wiring/README.md), [schema1 sequencing](evidence/APP-006/hosted-callback/schema-one-sequence/README.md) |
| Auth live-operation preparation | [Activation runbook](evidence/APP-006/hosted-callback/schema-one-sequence/activation-runbook.md), [independent review](evidence/APP-006/hosted-callback/schema-one-sequence/activation-independent-review.md), [fresh template synthesis](evidence/APP-006/hosted-callback/schema-one-sequence/fresh-synthesis.json) |
| Actual safe SQL observation | [Managed schema1 receipt](evidence/APP-006/hosted-callback/schema-one-sequence/sql-preflight/managed-observation.json), [actual-operation review](evidence/APP-006/hosted-callback/schema-one-sequence/sql-preflight/managed-independent-review.md), [resumed read-only inventory](evidence/APP-006/hosted-callback/schema-one-sequence/resumed-prestart-20261029.json) |
| Current CI blocker | [Original ten refused jobs](evidence/APP-008/ci/677d7bd-billing-block.json), [October29 planning retry](evidence/APP-008/ci/planning-billing-recheck-20261029.json) |

## APP-008: accepted local work and unfinished managed work

The schema contains26 tenant-bearing relations, FORCE RLS, tenant-leading keys/FKs, a NOLOGIN owner and narrowly scoped `genui_lock` helpers. Business DML is not broadly granted. Native storage has four fixed role pools, two connections each/eight total, bounded waiters, five-second acquisition, three-second transactions and two-second cleanup. Real TLS, current credential generations, transaction-local context and uncertain-cleanup discard are implemented. The production API has no public `TrustedRequest` constructor; ordinary request/session integration belongs to later tickets.

Current permission checks follow ADR-004: precheck, then organization→membership→work fencing and current authority checks. Do not replace this with claims trusted from headers or a client-selected tenant. The empty production registry is deliberately deny-all.

The final local schema campaign passed156 actual PostgreSQL cases plus31 runtime tests. Native R5 passed36 TLS/pool cases,18 library tests and private-construction/compiler-floor checks. Important repaired findings include unsafe metadata before catalog admission, implicit temporary-schema precedence, replica defaults, role drift and a narrowly supported local/RDS creator-membership difference. Preserve the exact catalog normalization; do not learn expected state from RDS or grant privileges to make a comparison pass.

Commit `acbde9b` extracted six existing groups into `app/src/storage/qualification_cases.rs` under `cfg(test)` only. All36 ordered campaign checks still pass. Root reviewed the complete extraction and exported exactly37 Docker-context files. The older R5 image `sha256:160296acae0418f0ac46b71fe21e27c19f8635d4494bef785a806769c2e19e46` is historical after that source/context change; do not label it the final current-source image.

The next draft adds `app/src/storage/managed_qualification.rs` and a test-only module declaration. It is frozen at SHA256 `57478ae92f0a7fdea62365c6fae4be650a1d63e44545edd249687d43df572dd7`. Final formatting and offline `cargo check --lib --tests` pass, but zero behavior/container/managed tests ran and independent review is **INCOMPLETE**. The author and reviewer have stopped, with no owned resources left running. See [handoff state](evidence/APP-008/native/managed-entry/handoff-state.json).

Intended interface: one exact ignored test entry, fixed target/path/orgs, an owned private manifest, original≤600-second deadline including cleanup, strict production `Settings::new`, source/catalog/fixture bindings, sanitized output and abort/await→`Stores.close`. A manifest CA hash establishes self-consistency only; actual managed packaging must independently pin the reviewed RDS CA bundle and disallow CA mounts/overrides. Local synthetic-CA testing is not RDS provenance. The current production Docker allowlist does not yet package the new draft; any separate qualification image must deliberately include its required compile-time source/resource files and be independently bound.

**Remaining managed work is substantial:** independently finish the entry; build a separate immutable test executable/image and private credential-refresh supervisor; package the migration/seed operator separately; qualify a bounded two-process race coordinator; run actual RDS IAM/TLS/role/RLS/reuse/race/drift/HTTP checks and recovery. Never point the Docker-only `native_postgres_campaign` at RDS. The production service must contain no test login or authenticated fixture endpoint. The operator password and application role credentials must remain in separate actors.

The proposed race coordinator uses run-scoped advisory phase signals and actual PostgreSQL blocking/lock observations, not arbitrary sleeps or task startup as a barrier. It is still a proposal. Preserve production transaction deadlines and actual uncertainty/cleanup behavior. The managed seed package is missing from the current production image: `tenant_seed.py`, contract validators/fixtures and their dependencies must be packaged deliberately.

## Auth: exact artifacts and safe sequencing

| Artifact | Pinned identity |
| --- | --- |
| Selected schema1 HTTP callback | Source `18c33f538a19d06cb77f37d091c71b1a8d5e3c39`; ECR repository `877573132739.dkr.ecr.us-east-1.amazonaws.com/genui-synthetic`; OCI index `sha256:a4a47707e1e4028221cba7f5f0dfdf03ef663c6f46d0686a27632c4fd5fc5d08`; ARM64 leaf `sha256:08d350fd186f72886823c9b33be21b1df1a70d4f96b1a35e63d639fb7c775ac9`. |
| Separate one-shot key verifier | Source `8fca35c35b80828d4effb17e59f9bbd80ae237ea`; index `sha256:5ee5ad165bca1e37bc2efe13f2fb727912c008cbe721b18f452a98482404cedc`; [binding](evidence/APP-006/hosted-key-qualification/registry-binding.json) |
| Existing stopped foundation | Source `8f8c34ba0b5e47a55990b273fc6d555d41369791`; image index `sha256:770bcb00395b135325ebb0955edc87a84c3c86b7ec05d0711af9a7765f03eb22` |
| Selected fAuth producer | Reviewed source `dfba787295b880816fd95295b507a4527c51e62f`, PR19 merge `4d70f198546cc7a009512244c2009e814ccc5f12`; actual provider publication/deployment remains owned by FormaStack. |

Use [callback registry binding](evidence/APP-006/hosted-callback/secret-wiring/registry-platform-binding.json) and [key qualifier handoff](evidence/APP-006/hosted-key-qualification/README.md) as authoritative digest records; never confuse OCI indexes, platform leaves and local image IDs. Both GenUI images have prior independent source/image/CI qualification; neither supplies a hosted auth pass.

APP-008 schema2 migration **does not block identity-only rehearsal**. Run the already qualified callback against the quiescent schema1 foundation. Keep every APP-008 managed migration, seed, drift, restore and operator mutation outside that auth interval. Observe all auth consumers stopped before later schema2 work; changing back to a schema1 image while leaving schema2 in RDS is not rollback.

The approved consumer matrix is two deliberate processes: existing Maya in three serial Chrome/native Safari/Firefox login flows, then a complete observed stop, new version of the same private secret, and one distinct fresh invited signup in Chrome. Four configured starts total, one flow at a time, no spare/retry process and one original deadline. A consumed invitation cannot be reused for login. Unexpected ECS/supervisor replacement aborts and reconciles; in-memory counters are not a durable global budget.

Native Safari capability now passed, using an actual Safari26.1 session and complete local cleanup at17:09 UTC. The producer receipt is `/Users/victorvillacis/dev/Forma/work/FORMA-247/rehearsal/joint-callback/native-safari-confirmed-capability-20261029.json`. This was capability testing, not hosted browser acceptance. No Safari waiver is needed.

Actual provider IDs/invitations/private configuration are obtained during a mutually agreed provisioning interval, not invented before it. Secret VersionId/config/run-ID linkage is private operator evidence, not runtime attestation; status does not expose hidden expected email/subject/version before a real exchange. The first login must prove the expected subject. Preserve unknown provider cleanup, and do not equate logout acknowledgement with actual refresh revocation.

The separate key tool runs one verifier in one process, uses a private FIFO and a real61-second cooldown with bounded lifetime. Its provider bridge is [received and hash-bound here](evidence/APP-006/hosted-key-qualification/provider-bridge/README.md). Do not restart it to manufacture a passing cache/key-rotation result. Keep it inside a stable callback segment, away from the stopped configuration transition.

FormaStack's final handoff identifies an additional operator preparation gap: its `synthesize.py` intentionally handles only initial key manifestv1. The v2–v5 rotation assemblies need a concrete reviewed invocation/closure extraction from the same typed-context CDK app. That later-generation helper/invocation has not been qualified. The FormaStack successor must resolve it before starting the paid rotation trial; do not weaken the initial helper or hand-edit generated templates.

**The no-SMTP rehearsal cannot close APP-006's mailbox requirement.** FormaStack supplied a [source-only mail follow-on](evidence/APP-006/upstream-auth-source/MAIL-VERIFICATION-FOLLOW-ON-PLAN.txt) and [receipt](evidence/APP-006/upstream-auth-source/mail-follow-on-receipt.json). It still needs truthful delivery outcomes, hashed tenant/user/email-bound atomic verification, authenticated resend, and a fresh ordinary GenUI sign-in after confirmation. APP-009/074 must provide the ordinary sign-in, restricted pending-verification state and backend resend contract; those routes do not exist yet. Never reuse expired signup PKCE or substitute the qualification harness for product sessions.

## AWS, budgets and private operating files

- Account `877573132739`, region `us-east-1`, stack `GenuiSyntheticFoundation`.
- ECS cluster `GenuiSyntheticFoundation-ClusterEB0386A7-HMlpmHwGMVSN`; service `GenuiSyntheticFoundation-ServiceD69D759B-pNHHjoez1RWF`.
- RDS identifier `genuisyntheticfoundation-databaseb269d8bb-pta9s49jelmg`, resource ID `db-KW4LRMTPI5YXHZXF4LELQZKTAU`; PostgreSQL16.13, private/encrypted/deletion-protected, automatic minor upgrades disabled. The installed stack policy denies RDS replacement/deletion. Do not bypass it.
- Budget ledger: `.cache/operations/beta/budget-ledger.json`, with its lock file. Preserve it and all prior/unknown reservations. Current retained hash before this handoff is `bf5ff561052c334e17e936ffce444a90df44660cc30af8fc9689bc6a3d65f616`.
- Current recorded reservation is $97.49, including $5 auth once and $25 unallocated. The withdrawn future forecast was $98.44, not a persisted reservation or invoice. The model includes full-month retained infrastructure and a variable reserve; it is not an AWS billing cutoff.
- Ledger timestamp was `2026-10-28T22:39:50Z`. Its prose basis still describes an earlier undeployed state. Before a future reservation, honestly reconcile that basis to actual retained resources/task history while preserving all cents and windows; do not merely advance the timestamp or reset costs.
- `.cache/operations/auth-schema-preflight/` retains the completed read-only observer. Task `4afe072901f049baa53689a42b0e213f` exited0 and STOPPED after53.012 seconds; temporary `GenuiAuthSchemaOnePreflight:1` is INACTIVE. Original migration revision2 remains unchanged. Cumulative known old-window standalone time is378.872 of7,200 reserved seconds. No active operation remains there.
- `.cache/operations/auth-rehearsal-20261029/` contains **read-only** resumed inventory, discarded forecast inputs and private hashes. No RunTask, CreateSecret, upload, change-set, reservation or activation was submitted in this proposed window. Preserve the initial local HTTPS trust-store refusal; normal verified system curl succeeded without disabling TLS.
- `infra/cdk.out/GenuiSyntheticFoundation.template.json` was freshly synthesized with no lookups. Its84,297 bytes /52,793 compact bytes exceed the direct TemplateBody limit. A future update needs a reviewed TemplateURL upload, exact hash/destination and owned-object cleanup. No upload occurred. Read [synthesis receipt](evidence/APP-006/hosted-callback/schema-one-sequence/fresh-synthesis.json).
- The only count-one path is `infra/scripts/preflight.py --activate`, after actual count-zero stack completion and future schedule verification. Default preflight writes a cumulative reservation; it is not a dry run. A past schedule cannot authorize activation. Use a new immutable window UUID/input after agreement; never rewrite an expired window.

Private files remain outside Git, generally0700 directories/0600 files. Do not paste credentials, tokens, full secret documents, callback URLs containing codes, or private runtime state into handoffs/logs. The operator should never retrieve the RDS password to a transcript; ECS injects it only into the separate operator task. Preserve needed local cache/ledger state when transferring hosts through an appropriate private channel.

## Git, checks and next-session procedure

At wind-down, the reviewed local base was `20a480b551eaa1876449122e193eb5bffa01761d`. The unaccepted managed-entry draft is preserved separately at **`4ac7639748cc86cfb56fdcda206f61c1c9087fa7`**. Recent reviewed local commits include `acbde9b` (shared cases), `a5248ee` (mail follow-on), `7ef6de9` (actual SQL observation) and `06c5a89` (observer helper). Remote working branch was freshly verified as `677d7bdd7cf41e11a49e85491f50f82b886070e6`; remote main was `d39d2f911ad3e631e8ce4c2fb28defdd8e90234f`. The final transfer commit adds this document and resumed evidence; use `git log` for its actual identity. No PR was created for these latest changes.

The ten checks for677d never executed. Planning run40812964975 attempt2/job123604611378 was also refused before steps on October29. These are operational refusals, not passing checks or failing application tests. Billing belongs to the `getforma-dev` organization; its allowance is separate from AWS and a personal GitHub plan. Do not repeatedly trigger the full suite while the account remains blocked.

Recorded toolchains: Rust1.93.1, compiler floor1.85.0, Node22.21.0, npm11.6.2, Python3.11.14, locked dependencies, Docker and AWS CLI. Local runtime/contract virtual environments already exist. Keep aggregate Cargo build jobs≤3 (normally2); avoid concurrent heavyweight builds across the two agents. Do not update packages or broaden Docker context incidentally.

Start with these read-only/local checks:

```sh
git status --short
git log -8 --oneline
python3 Tickets/_tools/board.py check
python3 Tickets/_tools/board.py next
```

Then:

1. Read the final draft and independent handoff review; finish or deliberately isolate it before treating the working branch as releasable. Do not rewrite historical receipts to match new source.
2. Resolve CI availability. Once available, push the reviewed current source, run the applicable exact-source checks, inspect artifacts and land only after review. APP-011's final local reset digest is `cf22271640f77ae5c8b540e6406f5c813064886713a46c87cfb35b41695af9f2`; its underlying schema/runtime inputs remain pinned in its receipt.
3. Coordinate with the FormaStack successor. Revalidate current source/images/Safari/environment/budget and agree a **new future** interval. Repeat safe schema1 SQL observation inside its newly reserved allowance before callback activation. Prepare/review actual secret/upload/change-set/run requests with real provider IDs. Preserve the withdrawal history.
4. Continue APP-008 managed entry/package/operator/race work locally in parallel, keeping shared RDS untouched during auth. Finish its actual managed gate before using it to unlock dependent app implementation.
5. Implement the remaining dependency chain, retain all required negative/race/restart/rollback evidence, and perform independent Phase1 acceptance. Only then complete the Phase2 handoff.

All cloud calls need bounded timeouts and retained request identity. An unknown outcome is not proof that nothing ran: reconcile exact task/stack/secret/version IDs before retry, preserve reservations, and never restart an operation merely because observation timed out. Observe actual STOPPED before claiming cleanup. The service scheduler does not stop standalone tasks.

## Final transfer notes

The source owner and independent reviewer froze the managed-entry draft at the exact hash above. Its [handoff](evidence/APP-008/native/managed-entry/HANDOFF.md) and [incomplete review](evidence/APP-008/native/managed-entry/independent-handoff-review.md) list all unverified work and confirm no owned local resource needs cleanup. No GenUI cloud mutation or live operation is being handed over from this resumed turn.

The FormaStack agent saved `/Users/victorvillacis/dev/Forma/work/CLAUDE-HANDOFF-2026-10-29.md`, withdrew the proposed window, set its execution flags false and clock null, and is pausing its own goal. GenUI is likewise winding down at Victor's request. A successor must resume deliberately and revalidate evidence; neither handoff is an auth start signal.

All work is retained in the local Git history and referenced files. The remote heads were re-read during transfer and still match the two old remote hashes above. Local unpushed commits, the private operating ledger/cache and the other team's non-Git evidence workspace must not be lost if transferring to another machine. Do not upload private operating files into the repository.
