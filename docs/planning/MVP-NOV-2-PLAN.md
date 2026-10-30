# November 2 MVP plan — Oct 29 evening → Nov 2

Prepared 2026-10-29 (evening UTC) by the GenUI planning lead under [ADR-010](../architecture/ADR-010-NOV-2-MVP-CUT.md). Status: plan, not evidence. The orchestrator is the GenUI owner and single GenUI operator; implementers and reviewers are separate agents in worktrees under `.claude/worktrees/`. Times are UTC. The MVP is defined in ADR-010 §1; the invariants in §2; the review tiers (A/B/C), evidence rule and batched landing in §4.

**Starting state (2026-10-29, after `0f9025d`):** PH01 Done: APP-001–008, 011, 073, 075 (PR 2 landed on `main` at `699b8a9`). In review: APP-010 (re-check clean; lands in PR 3). Ready by dependencies: APP-009 (being implemented, worktree `agent-a4241d58b67f65c4e`), APP-077 (implemented at `1c581f2` in worktree `agent-a0d2a0e69a03496cf`, awaiting review). Backlog: APP-074, 012, 013, 014, 078, 076. fAuth is live (FORMA-367) and the mail image qualified (FORMA-366); both gates cleared. Victor approved the always-on figure (about $150–170/month) and the SES sandbox with pre-verified recipients.

## Critical path

```
APP-009 (A) ──► APP-074 local (A) ──┐
APP-010 ✓  ──► APP-012 (A/B) ──► APP-013 (A) ──► APP-014 (B) ──┤
APP-077 (B) ──► APP-078 window (A, Nov 1) ───────────────────────┴──► APP-076 trial (A, Nov 2) ──► PH01 GATE.md ──► PR 5
         PR 3 (010, 009, 077, ADR-010 board)        PR 4 (074, 012, 013, 014)
```

The RDS window needs APP-077 and APP-075 Done and a reviewed candidate image; it does not need APP-012–014 merged, but the beta service that stays running after it must run the final candidate, so the window waits for PR 4's tip. The go-live trial needs everything.

## Lanes

- **L0 Orchestrator** (one agent): merges, board moves, landing PRs, CI watching, AWS operations, Victor coordination. Never implements a slice.
- **L1 Implementers** (up to three in parallel, one ticket each): APP-009 → APP-074; APP-012 → APP-013 → APP-014; APP-077 fixes → APP-078 preparation scripts.
- **L2 Reviewers** (fresh agent per slice; Tier A re-check by the same reviewer): review the worktree's reviewed head, write `evidence/APP-0NN/independent-review.md`, apply Tier B fixes themselves.
- **FormaStack lane**: nothing new is required. Keep it informed of the Nov 1 window (no fAuth deployment operation during it) and ask it to confirm the `ten_`/`usr_` ids and the beta client are unchanged.

Resource rules: `CARGO_BUILD_JOBS=2`, at most three aggregate Cargo jobs; one Docker campaign at a time; the orchestrator schedules campaigns so two implementers never run them simultaneously.

## Day by day

### Oct 29, evening (now → 24:00)

| # | Task | Lane | Parallel with | Needs |
| --- | --- | --- | --- | --- |
| 1 | Commit ADR-010 and the board (done: `0f9025d`); commit the planning docs; do not push until PR 3 is cut. | L0 | — | — |
| 2 | APP-077 Tier B review of `1c581f2` (operator image, `upload-sources` B1 layout, overrides ≤ 8,192 B, `OperatorTask` count zero, no role-assumption grant); reviewer applies small fixes; evidence under `evidence/APP-077/`. | L2 | 3, 4 | — |
| 3 | APP-009 implementation continues (plan Task 7): migration 0006, `auth/*`, `sessions.rs`, ADR-006 routes, SSR sign-in/pending pages, release router without the harness. Target: implementer reports by Oct 30 ~10:00. | L1 | 2, 4 | — |
| 4 | APP-012 storage/worker half starts in its own worktree (plan Task 9): `query_engine.py` modes, `queries.rs`, `worker.rs`, `receipt.rs`, oracle tests for `total`/`timeline`, duckdb 2.0.0 pins; the route and page wait for APP-009's `resolve_session`. | L1 | 2, 3 | APP-010 merged (yes) |
| 5 | Send Victor the ADR-010 judgment calls (§8) and the Nov 1 window request (slot, cost inside the approved figure, "no fAuth operation during it"). | L0 | — | — |

### Oct 30

| # | Task | Lane | Parallel with | Needs |
| --- | --- | --- | --- | --- |
| 6 | Merge APP-077 after its review; move APP-077 to In review. | L0 | 7 | 2 |
| 7 | APP-009 Tier A review → fixes → re-check (same reviewer). Target merged by ~18:00; APP-009 to In review. | L2, L1 | 8, 9 | 3 |
| 8 | APP-074 local implementation starts as soon as APP-009's branch builds (plan Task 8, items 1, 2, 3, 5, 6 only): landing and invite pages, `/invite`, `/invite/validate`, PAR handoff against the synthetic issuer, limits, trusted proxy, uniform 404 fallback. | L1 | 7, 9 | APP-009 branch |
| 9 | APP-012 route and page half once APP-009 merges; Tier A review of the worker confinement and authorization-at-delivery, Tier B of the page in the same review. | L1, L2 | 7, 8 | 7 |
| 10 | APP-078 preparation (no AWS mutation): reconcile the ledger basis to the approved figure; prepare the window input (`extra_task_hours` ≥ 2); teach `infra/scripts/preflight.py` `QualificationImageDigest`/`OperatorImageDigest`; draft the TemplateURL upload and change-set files; review the ten-minute campaign cap; write the window runbook as a checklist. | L1 (third implementer) | 7, 8, 9 | APP-077 merged |
| 11 | **Landing PR 3** (`phase-one-spikes` → `main`): APP-010, APP-009, APP-077, the ADR-010 board and docs. Watch the eleven workflows; fix forward in the branch. Ask Victor for the merge word; on merge set APP-010/009/077 Done with revisions. | L0 | — | 6, 7 |

### Oct 31

| # | Task | Lane | Parallel with | Needs |
| --- | --- | --- | --- | --- |
| 12 | APP-012 review fixes → re-check (Tier A surface only) → merge; APP-012 In review. | L1, L2 | 13, 14 | 9 |
| 13 | APP-013 implementation (plan Task 10): migration 0007 (`command_claims`, `command_outcomes`, `organization_sequences`, `query_receipts` per ADR-009 D9.8), `commands.rs`, payload digest, routes, record page; S05 sequence tests. Tier A review in the evening. | L1 | 12, 14 | APP-012 branch |
| 14 | APP-074 Tier A review → fixes → re-check → merge; APP-074 In review. | L2, L1 | 12, 13 | 8 |
| 15 | Build the candidate service, qualification and operator images from the branch tip after 12 and 14; run `check_image.py`, `qualify_image.py`; record identities (this is the one evidence re-recording, ADR-010 §4.2). Re-run the next day if PR 4 changes bytes. | L0 | — | 12, 14 |
| 16 | Victor confirms the Nov 1 window; FormaStack confirms no fAuth operation in it. | L0 | — | 5 |

### Nov 1

| # | Task | Lane | Parallel with | Needs |
| --- | --- | --- | --- | --- |
| 17 | APP-013 fixes → re-check → merge (morning). APP-014 campaign (plan Task 11) implemented on APP-013's checkpoints; Tier B review; merge by ~15:00. | L1, L2 | 18 | 13 |
| 18 | **Landing PR 4**: APP-074, APP-012, APP-013, APP-014. Workflows green; Victor's merge word; Done for the four at merge. | L0 | 17 | 17 |
| 19 | Rebuild the candidate images from PR 4's tip; re-record identities; push images to ECR (inside the window reservation). | L0 | — | 18 |
| 20 | **RDS window (APP-078), ~17:00–20:00 UTC, Victor-approved, no fAuth operation:** observe 0/0/0 and the consumer stopped; reserve the window UUID with the ledger; TemplateURL change set (new `ImageDigest`, optional task definitions, stack policy read back); readiness comparison read-only; pre-migration snapshot; migrations 0003–0007 once and again through the operator task; seed the two organizations; `upload-sources` for the fixture revisions; qualification task (six checks, role bindings, refusals); operator `verify`; grant/RLS isolation matrix rows; production HTTPS surface from the exact image; stop every task, verify cleanup, retain and name the snapshot; reconcile the ledger. Then (plan Task C3) service desired count 1 on the recorded digest, `beta.getforma.dev` → the load balancer (certificate already validated), issue Victor's and the demo principals' invite codes with the operator CLI (raw codes delivered privately once). | L0 | — | 19, 16, APP-077/075 Done |
| 21 | APP-078 Tier A review of the receipts (evening); APP-078 In review. Anything Fargate refused (environment names, mounts) is fixed forward in the window or recorded for APP-086. | L2 | — | 20 |

Contingency: if PR 4 slips past ~15:00, run the window on the PR 3 tip plus the reviewed APP-012–014 branch merged locally into a candidate branch, and redeploy the final digest on Nov 2 morning before the trial (a redeploy is a task start on a new digest, not a migration).

### Nov 2

| # | Task | Lane | Parallel with | Needs |
| --- | --- | --- | --- | --- |
| 22 | **Go-live trial (APP-076), ~13:00–18:00 UTC, installed Chrome, real mail (SES sandbox):** Victor, from a second network, enters his invite code at `https://beta.getforma.dev/invite`, completes fAuth signup, receives the verification mail at the SES-verified address, returns through `/signin/{handle}`, is admitted, opens the Northstar work (`total` 1,140,000; `timeline`; receipts shown), opens Spend Review's record page and runs the S05 sequence (`ik-1` v0→v1, retry, stale 409, `ik-2` v2); the orchestrator restarts the service and Victor reopens the record; scripted checks: the private-route refusal matrix and the canary replay (item 9), public-API denials and revocation at delivery (item 2), ADR-006 routes with cooldown and limits (item 4), the PAR/mail/storage/log inspection (item 5), audit `verify` on RDS (item 10), rollback as a redeploy of the previous task definition with protected routes 404 and then restore (item 8), ledger reconciled against the approved figure. | L0 + Victor | — | 20, 21 |
| 23 | APP-076 Tier A review (identity items) with the checklist; APP-076 In review. | L2 | 24 | 22 |
| 24 | `evidence/phases/PH01/GATE.md` (checklist form, ADR-010 §4.4): one line per exit criterion; limits name APP-079, APP-080, APP-082, APP-083 and the seeded-source/definition labels; independent reviewer's decision. | L0, L2 | 23 | 22 |
| 25 | **Landing PR 5**: APP-078/076 evidence, tickets to Done, `gate_status: passed` with the evidence reference in `Tickets/plan.json`, `PHASE-2-HANDOFF.md` READY, `CLAUDE-HANDOFF.md` updated. Victor's merge word. The service stays RUNNING on the recorded digest. | L0 | — | 23, 24 |
| 26 | Hand Victor the demo script for the recording (what to click, what each screen proves, the labels "seeded by the operator" and "seeded definition"), and the ADR-010 §6 pick-up order for Nov 3. | L0 | — | 25 |

## What Victor must do

1. **Oct 29–30:** confirm or veto the ADR-010 judgment calls (§8), in particular the two trims, the deferred provider-audit amendment, the custody default and the review tiers.
2. **Oct 30–31:** approve the Nov 1 RDS window slot (about $3–6 of task-hours inside the approved $150–170/month figure) and the beta go-live that follows it (the service stays running from Nov 1).
3. **Oct 31:** give the merge word on PR 3; **Nov 1:** on PR 4; **Nov 2:** on PR 5.
4. **Nov 2:** be the invited viewer: a second network (phone hotspot), installed Chrome, the SES-verified mailbox `victor@getforma.dev`; complete the signup, the verification mail, the sign-in and the clicks in task 22, and the reopen after the restart. About forty minutes.
5. **Any time:** decide when the repository returns to private (ADR-008 §7); it is not required for the MVP.

Nothing else needs his decision. If he cannot be present for task 22, the orchestrator runs it with a second SES-verified address he designates in advance; the recording of him as the viewer is then a retake after Nov 2.

## What is deliberately not done before Nov 2

Everything in ADR-010 §5–6: APP-082/083 (deferred proofs), APP-084–088 (review residuals), APP-079/080 (ADR-008 deferrals), PH02–PH06. No LLM call. No native Safari/Firefox run. No import route. No authoring UI. No sharing. No six-act rehearsal.

## Stop rules

- A Tier A finding that is a real authority defect stops the lane until fixed and re-checked; it is never dispositioned as a limitation.
- The RDS window stops at any unexpected readiness difference, any migration that does not apply once-and-again cleanly, or any qualification check that fails; the snapshot is retained and the service stays stopped until the cause is understood. The beta goes live a day late rather than on an unqualified schema.
- No window mutation without the reserved ledger entry and Victor's slot approval; no fAuth operation overlaps it.
- If the deadline is missed on a single item, the gate record names it as a limitation with its owner and Victor decides whether the recording waits.
