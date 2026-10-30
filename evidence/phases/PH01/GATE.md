# PH01 gate record — Foundations and first saved workflow

**Checklist form, not a passing record.** Prepared 2026-10-30 at `89c6ec1` per [ADR-010](../../../docs/architecture/ADR-010-NOV-2-MVP-CUT.md) §4.4 and the [gate template](../../../Tickets/_tools/PHASE-GATE.md). Every box is unchecked: nothing here is evidence until the APP-078 window (Nov 1) and the APP-076 trial (Nov 2, [runbook](../../APP-076/prep/RUNBOOK.md)) have run and been reviewed. `Tickets/plan.json` keeps `gate_status: pending` until the reviewer's decision below says Accepted.

- Phase: PH01 (`Tickets/plan.json`, order 1, no predecessor gate)
- Review date:
- Candidate commit / service image / task-definition revisions / dependency pins: `CANDIDATE` · `SERVICE_DIGEST` · `CANDIDATE_TD` / `PREVIOUS_TD` · Forma 3.2 pins per `docs/architecture/FRAMEWORK-BASELINE.md`, `duckdb==2.0.0`, contract edition `impl-2026-10-29.3`
- Test environment and workload/input identities: `beta.getforma.dev` (stack `GenuiSyntheticFoundation`, RDS resource `db-KW4LRMTPI5YXHZXF4LELQZKTAU`), fAuth `auth-staging.getforma.dev` (gatewasm `1bf5d3b`, image `sha256:d1365238…`), APP-078 window `88d1b19d-db20-4593-993c-de275d4e7318`, APP-076 trial `91de7f07-2fec-435e-b066-775e6f85068e`, fixtures `contracts/fixtures/impl-2026-10-28.1/{northstar,spend-review}.json`, Northstar source revision `9161c1e4-27c0-4cbb-addf-eb7ce504bf45`
- GenUI implementation owner: GenUI orchestrator (Claude)
- Independent reviewer:
- Decision:

## Exit criteria (one line each: criterion → owning tickets → evidence → CI run / window → limitation)

- [ ] **EC1** APP-001–014 and APP-073–078 Done with reproducible build, schema, parser/query confinement and native adoption evidence; compile refusals and numeric/resource bounds tested → the twenty tickets below → `evidence/APP-0NN/` per ticket, `evidence/APP-078/window/` → PR 2 (`699b8a9`), PR 3, PR 4, PR 5 run ids → none expected; any ticket not Done blocks the gate.
- [ ] **EC2** FORMA-246 conformance and FORMA-232 budget disposition accepted in APP-003; FORMA-247 cleared by founder disposition (ADR-008), its hosted proof FORMA-365 / APP-079 → APP-003, APP-006 → `evidence/APP-003/`, `evidence/APP-006/upstream-auth-source/` → APP-003 Done record → hosted three-browser proof deferred to APP-079.
- [ ] **EC3** Integrated query → receipt → explicit command → atomic outcome/audit → reopen; identical retries do not duplicate, stale edits conflict, both revocation/adoption race orders pass → APP-012, APP-013, APP-014 (local), APP-076 items 2 and 6 (live) → `evidence/APP-012/routes/`, `evidence/APP-013/review-fixes/command-campaign.json`, `evidence/APP-014/race-campaign*.json`, `evidence/APP-076/trial/` (matrix-member, logs-part2, operator verify) → PR 4 run ids, trial `91de7f07…` → concurrent last-slot redemption through the public API is APP-083; RDS lock-observation repeat is APP-080; in-flight revocation at delivery live not timed (runbook G10).
- [ ] **EC4** Tenant isolation and authoritative identity on the integrated path: RDS grant/RLS matrix (APP-078), identity and the private-route refusal matrix live (APP-076); synthetic substitutes identified → APP-078, APP-076 items 3, 9, 10 → `evidence/APP-078/window/` (qualification `APP008_MANAGED_RESULT`), `evidence/APP-076/trial/matrix-*.json`, `scan.json` → window `88d1b19d…`, trial `91de7f07…` → pre-migration snapshot retained but its restore unproven until APP-080; the only beta rollback for a committed migration is a reviewed forward fix (ADR-008).
- [ ] **EC5** Public HTTPS keeps invite-first registration and server-bound organization scope; anonymous/pre-access/expired/revoked cannot dispatch protected work; hosted registration, real verification delivery, mailbox activation and code revocation in installed Chrome; environment inside the approved always-on figure; no LLM call → APP-074 (local), APP-076 items 3, 4, 5, 8 → `evidence/APP-074/local/`, `evidence/APP-076/trial/` (victor-part1, logs-part1, rollback/restored matrices), ledger basis → trial `91de7f07…` → Safari/Firefox APP-079; public-API code concurrency APP-083; route-level race repeat and provider fault exercise APP-082; ledger against the approved $150–170/month figure (GenUI cap $170, project $208).

### Ticket status (EC1)

- [ ] APP-001 · [ ] APP-002 · [ ] APP-003 · [ ] APP-004 · [ ] APP-005 · [ ] APP-006 · [ ] APP-007 · [ ] APP-008 · [ ] APP-011 · [ ] APP-073 · [ ] APP-075 — Done on the board today; confirm their evidence is unchanged at the candidate.
- [ ] APP-009 · [ ] APP-010 · [ ] APP-077 — In review today; Done at PR 3's merge.
- [ ] APP-074 · [ ] APP-012 · [ ] APP-013 · [ ] APP-014 — Backlog today; Done at PR 4's merge (APP-012 review M1–M4 and the APP-013 re-check closed first).
- [ ] APP-078 · [ ] APP-076 — Backlog today; Done at PR 5's merge with their Tier A reviews.

## Integrated and release trials

- [ ] APP-078 RDS window (migrations once and again, seed, uploads, qualification, HTTPS surface, beta up) → `evidence/APP-078/window/`.
- [ ] APP-076 go-live trial (invite → verified sign-in → entitlement → query with receipt → command → outcome/audit → restart/reopen, canary denied, rollback redeploy and restore, ledger) → `evidence/APP-076/trial/`, `evidence/APP-076/README.md`.
- [ ] Open trial gaps G1–G15 of the [runbook](../../APP-076/prep/RUNBOOK.md#gaps-that-stop-or-weaken-the-trial-found-2026-10-30-at-89c6ec1) each closed or dispositioned here.

## FormaStack handoffs

- [ ] FORMA-366 (mail image, controlled inbox) → `evidence/APP-006/upstream-auth-source/fauth-mail-trial-2026-10-29.md`; GenUI integration result: APP-076 items 3–4.
- [ ] FORMA-367 (lean always-on fAuth, beta clients) → `evidence/APP-006/upstream-auth-source/fauth-beta-deployment-2026-10-29.md`; cost receipt attached to the ledger.
- [ ] FORMA-365 (hosted three-browser proof) → deferred, APP-079.

## Limits and unresolved findings

Deferred by ticket (named, not waived):

- **APP-079** (PH06, ADR-008): native Safari and Firefox runs, deployed negative campaign, rotation campaign, canary resource check of the joint rehearsal; FORMA-365.
- **APP-080** (PH05, ADR-008): recovery drill and pre-migration snapshot restore, pool lifecycle, both revocation/adoption-versus-commit orders on RDS with lock observations, drift/readiness rows.
- **APP-082** (PH07, ADR-010): APP-074 items 4 and 7, the local route-level repeat of APP-073's admission races and the provider fault exercise through the signup flow.
- **APP-083** (PH07, ADR-010): APP-076's former item 7, concurrent last-slot redemption through the public API with identical retry and revocation at the next request and at delivery.

Labels that hold for every Nov 2 claim (ADR-010 §1, demo R3/G6): sources are **"seeded by the operator"** (fixture revisions uploaded by `upload-sources`, APP-077), and the Northstar and Spend Review definitions are **"seeded definition, retrieved from the library"** (typed fixtures admitted through APP-010's path); nothing is presented as imported, authored or generated, and no model is called.

Other limits to record at review: the pre-migration snapshot restore is unproven until APP-080; Spend Review runs in organization `…0002` (runbook D1) unless an operator channel places it in Northstar; review residuals carried by APP-084–088 (ADR-010 §5).

## Acceptance and next permitted step

Reviewer conclusion:

Decision (Pending / Changes required / Accepted): ____________________  Reviewer: ____________________  Date: __________

Only after Accepted: set PH01 `gate_status: passed` with this file in `gate_evidence` in `Tickets/plan.json`, render the board, and mark `PHASE-2-HANDOFF.md` READY (MVP plan task 25).
