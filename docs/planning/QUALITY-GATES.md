# Phase verification and release gates

Every implementation ticket has design references, acceptance tests, a verification plan and migration/rollback guidance. Every phase adds an integrated evidence gate. This document defines those obligations; the [live board](../../Tickets/TRACKER.md) and each ticket’s retained evidence record which checks have actually passed. Completed isolated qualifications do not establish a managed release trial or phase acceptance.

The controlling references are the [delivered platform design](../../design/forma-generative-app-platform-v2/deliverables/v1/md/design.md), [shared contracts](../../design/forma-generative-app-platform-v2/SHARED-CONTRACTS.md), [validation plan](../../design/forma-generative-app-platform-v2/VALIDATION.md), [delivered demo design](../../design/forma-northstar-demo/deliverables/v1/md/design.md), and [S01–S07 scenarios](../../design/forma-northstar-demo/SCENARIOS.md). [Coverage](../../Tickets/COVERAGE.md) maps all 80 requirement/scenario IDs to owning work. A coverage link is a test obligation, not a passing result.

## Required checks by phase

| Phase | Checks and tests | Trial / integrated demonstration | Owning tickets and design obligations |
|---|---|---|---|
| [PH01](../../Tickets/phases/PH01.md) Foundations | Published-pin build; native admission/adoption; parser/query confinement; tenant/auth denial; invite-first registration and verified-mailbox activation; atomic code redemption, expiry/revocation, request limits; command atomicity/replay/conflict and both race orders | Public invite entry → tenant-bound signup → admitted session → authorized query → receipt → explicit command → restart/reopen on the live invite-first beta at beta.getforma.dev in installed Chrome with real mail, with accepted upstream evidence and direct API denials; native Safari/Firefox, deployed negatives and the RDS lock-observation repeat are deferred to APP-079/080 and named in the gate record ([ADR-008](../architecture/ADR-008-YC-DEMO-AND-BETA-MVP.md)); until APP-080 exercises it, the pre-migration snapshot restore is unproven and the only beta rollback for a committed migration is a reviewed forward fix | APP-001–014 plus APP-073–078; [ADR-003](../architecture/ADR-003-PUBLIC-BETA-ACCESS.md), [ADR-005](../architecture/ADR-005-PHASE-1-SEQUENCING.md), [ADR-008](../architecture/ADR-008-YC-DEMO-AND-BETA-MVP.md); C01/C04/C06/C07/C11/C15, N4/N5/N7/N9, R15/R16, SR1/SR2/SR3/SR5/SR7/SR8/SR10/SR12; design §§5–10 |
| [PH02](../../Tickets/phases/PH02.md) Data/workspace | Actual CSV/XLSX and malicious-file cases; mappings/units; exact arithmetic; pagination/snapshots; stale-response rejection; resource limits; automatic accessibility states, keyboard and screen reader | Drop and keyboard import, linked views, real form submission, personal layout and new/continued work survive reload | APP-015–031; R1/R2/R3/R6/R7/R8, N3/N6/N12; S01/S02/S05/S06 |
| [PH03](../../Tickets/phases/PH03.md) Creation/governance/AI | Whole-definition admission; patch conflicts/undo; preview isolation; publication and adoption races; provider disclosure; bounded tools; 10 supported + 5 adversarial intents; provider outage | Author two app shapes through ordinary controls, review exact bytes, publish, explicitly adopt/roll back, and keep deterministic work usable during model outage | APP-032–047; R4/R5/R9/R10/R12/R13/R14, C08/C09/C10/C12/C13, SR4/SR6/SR13; S03 |
| [PH04](../../Tickets/phases/PH04.md) Two-app proof | Current grants across raw/derived/cached/export surfaces; revocation; independent reuse; actual backup restore with chained/anchored audit, deletion ledger, quotas; full S06 and accessibility suite | Six-act rehearsal on one unchanged build; real Spend Review creation; guarded reset, pre/post-take canaries and assertions, sanitized/anchored evidence packet; record all seeded/simulated/manual steps | APP-048–057; C11/C15/C16, N4/N7/N12; S01–S07; APP-056 records SR1–SR13 demo applicability and demo-SR1–SR6 verification |
| [PH05](../../Tickets/phases/PH05.md) Managed controls | Service-role denial; signatures/provenance/SBOM; previous/current API compatibility; audit chains/anchors; support/breakglass; retention/erasure and deletion hold | Upgrade → downgrade → upgrade; callee-first canary deployment and rollback on an exact candidate digest, in the intended isolated environment | APP-058–066 and APP-080, especially APP-061; SR3/SR8/SR9/SR10/SR11; design §§9–11 |
| [PH06](../../Tickets/phases/PH06.md) Pilot qualification | Payload-safe telemetry; measured RPO/RTO; process/role/zone/provider/key-store failures; cold/warm million-row and six-view loads; hot-record concurrency; 10x/noisy-neighbor/budgets; independent security and accessibility review | Restore and failure drills, exercised release/rollback/runbooks, final live-data readiness review, then recurring customer workflow and second-person reuse | APP-067–072 and APP-079; N1–N12 as applicable, SR1–SR13; design §§10–11; S06/S07 |

Checks run as the owning behavior is introduced, and are rerun when changes invalidate the evidence. Security, isolation, exact arithmetic and command consistency are required throughout; their later whole-system review does not defer those controls until the end.

## Demo-specific applicability

The delivered demo §10.7 explicitly retains platform SR1, SR2, SR4–SR7 and SR10–SR13. It substitutes typed in-process principals for platform SR3, a pinned image digest plus per-build tests for SR8, and accepts the one-operator/no-breakglass reduction for SR9 under demo AR1. APP-056 must record this matrix and verify the applicable or substituted controls; a PH04 pass is not a claim that production controls already exist. Full service tokens, signed releases and managed support controls belong to APP-060/061/063 in PH05.

Demo-SR1–SR6 are a separate namespace in the demo §4. APP-057 verifies that protocol: reset guards, pre-take membership/policy/build/dry-sign-in assertions, paired canary denial and owner-success checks before and after a take, sanitized packet/credential scan, and P10's anchored manifest digest. Per demo §11.1, failed assertions/checks/canaries stop the take or roll back to the previous task definition; there are no demo canary waves or upgrade/downgrade trials. Those full rollout trials remain PH05 obligations.

The demo does retain chained insert-only audit and 15-minute Object Lock anchors (TH7/M13–M14), restored-chain verification, and a packet-manifest audit event under an anchor (M31). APP-053 and APP-057 own these before PH04 completion; APP-062 carries them into managed operations. These are not deferred until after the demo.

## Faithfulness to the design

- Retain exact fixture semantics: 1,140,000 total bytes, 900,000 unmatched, 114,000 bytes/minute; independent reuse 210,000 bytes and 21,000 bytes/minute; Spend Review 64,000 cents total and a 45,000-cent queue. Recompute from imported sources and receipts.
- Treat design capacity/latency/recovery figures as targets to qualify. Use the adopted limits and explicit workload conditions; record the result. A missed target requires an explicit design disposition, never a silent relaxed assertion.
- Preserve immutable publication, source/work binding, record snapshot, current authorization, idempotency and atomic audit semantics. A mocked boundary cannot pass its integrated check.
- Forma 3.2's published route limits differ from the design's historical 3.0 figures. APP-001/003 and the FormaStack handoff resolve the difference with measured evidence. Neither an application build nor this phase plan qualifies a framework release.
- Source changes that affect a tested contract require review and fresh affected checks. Keep the original failed trial as well as the repaired run.

## Completion evidence

Use the [phase gate record](../../Tickets/_tools/PHASE-GATE.md) to produce `evidence/phases/PHxx/GATE.md` at actual phase review. Record:

1. Exact commit/build/package/image/model/source identities and environment.
2. Each required design obligation and its owning ticket/test result: command or reproducible manual steps, expected versus actual behavior, and evidence location.
3. Relevant negative, concurrency, isolation, accessibility, failure and release/rollback results. Where a trial is not applicable to the phase, state why; later assigned trials remain required.
4. Accepted FormaStack dependency receipts and GenUI integration results.
5. Remaining failures, limits, design amendments and independent reviewer disposition. An unresolved hard-gate failure keeps the phase open.
6. The explicit decision to accept the phase and what it permits next. A phase acceptance does not authorize an unrelated framework publication or production deployment.

The board marks a phase complete only when all its tickets are Done, its predecessor phase gates passed, and its own gate is marked passed with existing evidence-file references in `Tickets/plan.json`. The checker validates those relationships; an independent reviewer evaluates the evidence itself. A checkbox or file's existence alone proves no runtime result.

For PH06, APP-067–071 provide technical readiness. The live-data entry decision occurs at APP-072 entry using APP-058/070/071/079 evidence. PH06 completes after APP-072 observations; requiring completed customer learning before admitting the customer would be circular.
