# ADR-010 — The November 2 MVP cut

Date: 2026-10-29. Owner: GenUI (Claude, planning lead). Status: accepted. Victor accepted the scope and every §8 judgment call on 2026-10-29 ("Accept"), including the Nov 1 RDS window and the Nov 2 go-live trial with him as the viewer. This record changes no behavior contract in ADR-003, ADR-004, ADR-006, ADR-007 or ADR-009. It amends the acceptance ownership of two Phase 1 tickets (APP-074, APP-076), adds a completion phase (PH07) and seven tickets (APP-082–088), and sets the review and landing process until the deadline. Every acceptance sentence it moves is reproduced verbatim in its new owner with a "(Moved from APP-xxx by ADR-010.)" marker; nothing is deleted or softened.

Victor's words (2026-10-29): "move with the plan. However don't lose any work that needed to be done, cut tickets, mark defer for after Nov 2 and in order of pick-up and strategy. Also cut new follow-up tickets if needed, follow-ups, tests, open questions, etc. We don't want to lose sight of the main product. We will revisit all of it 100% after the Nov 2 deadline for the MVP." Earlier: "we want to show the product working. But we also need the most viable product ASAP ... Not saying to cut corners that can be dangerous. But find shortcuts that can save us time."

## Decision

1. **The November 2 MVP is Phase 1 on the live beta**, as [ADR-008](ADR-008-YC-DEMO-AND-BETA-MVP.md) defined it: invite → verified sign-in → current tenant entitlement → authorized query → visible receipt → explicit command → atomic outcome/audit → restart/reopen at `https://beta.getforma.dev`, the canary denied, the service left running. It is the demo's acts 1, 4 (run) and 6 (record) in reduced form, on seeded sources and seeded typed definitions labeled as such. Acts 2, 3 and 5 are not shown on November 2 and are not simulated; §1 says exactly what is and is not shown.
2. **Nothing a demo claim depends on is cut.** Authenticated identity, tenant isolation (RLS on RDS, the canary), receipt provenance, transactional commands with audit, idempotency and the ADR-006 verification contract stay whole (AGENTS.md: "Keep identity, tenant isolation, receipt provenance, idempotency and transactional audit intact even in a small synthetic demonstration"). §2 lists the invariants.
3. **Two tickets are trimmed, verbatim moves only.** APP-074 items 4 and 7 (the route-level repeat of APP-073's admission races; the provider fault exercise through the signup flow) move to **APP-082**. APP-076 item 7 (concurrent last-slot redemption through the public API) moves to **APP-083**. Everything else in the Nov 2 path keeps its acceptance (§3).
4. **Deferred work lives in PH07 "Post-MVP completion"** (sprint S13, order 7, depends on PH06), except two PH02 preparation tickets placed in PH02 because PH02's gate must not pass without them (§7). APP-079 (PH06) and APP-080 (PH05) stay where ADR-008 put them. Every deferred ticket's Log says "Deferred until after the 2026-11-02 MVP by ADR-010; pick-up position N".
5. **Review residuals become five follow-up tickets** (APP-084–088), grouped by subject, each citing the review file and finding ids, so no minor, declined item, limitation or open question from the APP-008, APP-010, APP-073 (schema-0003, admission, provider), APP-075, APP-017 parser-packaging, APP-015 (ADR-009, contract edition) and APP-027 reviews is lost (§5).
6. **Process accelerations** (§4): risk-tiered review, evidence re-recording only at the RDS window, batched landing PRs, a checklist-style gate record, fixed scope locks, parallel lanes. The Done standard (independent review, exact-source CI, landing on `main`) is unchanged; what changes is how many review rounds a slice gets and when evidence identities are re-recorded.

## 1. The November 2 MVP, act by act

The demo design's six acts (`design/forma-northstar-demo/design.md` §3–4; `SCENARIOS.md`) against what November 2 shows. "Shown" means live on `beta.getforma.dev` in installed Chrome with real verification mail (SES sandbox, verified recipients), with receipts retained under `evidence/APP-076/`. Labels follow demo R3 and G6: every seeded, simulated or manual step is labeled and excluded from acceptance measures; nothing is presented as generated.

| Act (design §16) | Shown on Nov 2 | Mechanism and tickets | Not shown on Nov 2, and where it lives |
| --- | --- | --- | --- |
| 1 Enter the workspace | Public landing and invitation entry; invite code validated by digest; fAuth signup with real mailbox verification; the ADR-006 sign-in, verification-pending and resend routes; an opaque workspace session minted only after membership and active entitlement; the home page listing the admitted app and saved work; sign-out. The canary organization replays Northstar identifiers and receives the uniform 404 (demo R8, G5). | APP-073 (codes, entitlement, provider invitation), APP-009 (sessions, ADR-006), APP-074 local items 1–3, 5, 6 (landing, invite entry, denials, limits, trusted proxy), APP-076 items 2–5, 9 (hosted) | Spaces, aliases, click/slash navigation, recents, thumbnails (APP-026, PH02). Native Safari and Firefox (APP-079). Concurrent last-slot redemption through the public API (APP-083). The route-level race repeat and provider fault exercise (APP-082). |
| 2 Bring data | Nothing live. The Northstar events CSV and inventory XLSX, and the Spend Review expenses table, are **operator-uploaded fixture revisions** (`upload-sources`, APP-077) normalized through the qualified APP-004/005 adapter, with real `normalized_digest`/`contract_digest` values recorded in `source_revisions`; labeled "seeded by the operator" in the UI and the evidence. The parser is packaged and qualified (APP-017 preparation) but no upload route exists. | APP-077 `upload-sources`, APP-011 fixtures, APP-004 adapter | Drop and keyboard upload, inspection, limits/progress/cancel, worksheet selection, mapping confirmation (APP-015–019, PH02). |
| 3 Form and transform an app | Nothing live. The Northstar and Spend Review definitions are **typed fixtures admitted through the real whole-output admission path** (APP-010: canonical digest, registered kinds/operators, unknown/executable/over-complex shapes refused) and published under the operator-created `owner_publish` policy; labeled "seeded definition, retrieved from the library" per R3. No model is called (no LLM before APP-043). No component is labeled generated. | APP-010, operator `create-app`/`grant-permission` | Six linked views, patches, preview, semantic diff, undo, stale-patch conflict (APP-032–036, PH03). Retrieval/adaptation/generation (APP-042–047, PH03). |
| 4 Publish and run | Run: an admitted member opens the saved work, the `total` view (1,140,000 bytes) and the `timeline` view execute in the confined query child and show their receipt (app/source/operator/parameter/policy/record-snapshot identities, result digest); loading, empty, failed and denied states are distinct; a failed execution never shows a fabricated zero; reload restores. Publish: the publication exists through the admitted path; the independent-review refusal (owner self-publish refused, non-owner reviewer accepted) is proven in the APP-010 campaign (check 13), not in the UI. | APP-012, APP-010, APP-076 items 2, 6 | The 114,000 bytes/minute ratio, CIDR inventory match, money literals (APP-020–022, PH02; APP-012's query subset is `total` and `timeline` only). Reviewer inbox and publication UI (APP-037–039, PH03). Filter epochs and six linked views (APP-025, APP-081). |
| 5 Share and reuse | Isolation only: the canary's denial of a Northstar resource; a Security-only principal cannot read Finance is proven at the storage boundary (APP-008/010 campaigns), not live. | APP-076 item 9, APP-008 | Separate grants, Jordan's reopen, bare-link denial, revocation of derived and queued results, export, `ops-runner-04` new work, version adoption (APP-048–052, APP-040, PH03–PH04). |
| 6 Build a different app | The Spend Review decision form on EXP-002 through the same typed path (no branch on an organization, space or app name): `ik-1` at version 0 commits version 1 with one record revision, one outcome and one audit event; `ik-1` retried after a lost response returns the stored outcome; a stale expected version conflicts (409 with recovery view); `reviewed` under `ik-2` commits version 2; service restart; reopen the same record/version/outcome; the audit chains verify on RDS. The imported 64,000-cent revision is unchanged. No payment, email or source update occurs. | APP-013, APP-014, APP-076 items 6, 10 | Forming Spend Review through the authoring interface (act 3's path) with no deploy between (APP-055, PH04). The `amount_cents >= 20,000` queue aggregate (money literals, APP-022, PH02); the record detail is shown from the record, not from a query. |
| Packet (S07) | APP-076's receipts, the PH01 gate record and the CI run identities; sanitized, no credentials. | APP-076 item 11, `evidence/phases/PH01/GATE.md` | The six-act rehearsal log, both app digests under one build with no deploy between acts 3 and 6, anchored manifest digest (APP-057, PH04). |

**Honest statement for the recording and the viewers.** On November 2 the product shows a working, isolated, receipted, transactional company-app runtime on a live invite-first beta with seeded sources and seeded typed definitions. It does not yet show import, authoring, generation, sharing or two apps created through one authoring path. Those are Phases 2–4 and follow the pick-up order in §6. A live AI generation (APP-043) cannot fit before November 2; the MVP therefore composes nothing and generates nothing: the seeded definitions are labeled as seeded, never as generated.

## 2. Invariants never cut

- Identity: fAuth PKCE through the backend-only PAR handoff; no provider token or Set-Cookie in the browser; opaque `__Host-` sessions; `email_verified` gate before admission (ADR-006); unknown keys fail closed.
- Tenant isolation: tenant-leading keys, FORCE RLS, four fixed roles with transaction-local scope; the grant/RLS matrix on RDS (APP-078 item 4); the canary denied over the live deployment (APP-076 item 9).
- Admission: 128-bit digest-only codes, server-fixed organization and permissions, atomic redemption with audit, pending admissions counted and bounded (APP-073).
- Receipts: every shown number equals its receipt; authorization before engine admission, at execution and at delivery (APP-012).
- Commands: one transaction for revision, outcome and audit; current authorization before replay lookup; same key different payload conflicts; stale expected version conflicts; both revocation/adoption orders (APP-013, APP-014).
- Operations: hash-chained insert-only audit verified on RDS; migrations applied once and again from the exact candidate image; pre-migration snapshot retained; rollback exercised as a redeploy that disables protected work; the ledger reconciled against the approved figure; no LLM call.

## 3. The cut, ticket by ticket

| Ticket | State today | Nov 2 disposition |
| --- | --- | --- |
| APP-001–008, 011 | Done | Unchanged. |
| APP-073 | Done (PR 2, merge `699b8a9`, source `b066247`, eleven of eleven workflows green) | Kept whole; its provider residuals go to APP-084. |
| APP-075 | Done (same PR) | Kept whole; its campaign residuals go to APP-086. |
| APP-010 | Re-check found no blocking findings | Kept whole; moved to In review by this record with items checked as supported locally (the S05 fixture goes through the operator path; the RDS application is APP-078's). Residuals r1, m3, m7 and the coordinator choices go to APP-085. |
| APP-009 | Being implemented (worktree) | Kept whole: all six items are identity. Tier A review. |
| APP-077 | Implemented and merged on the branch (`34a42eb`), Tier B review pending; ticket still Backlog until the orchestrator moves it | Kept whole; `upload-sources` is what makes APP-012's receipt truthful. Tier B review. |
| APP-074 | Backlog | **Trimmed.** Items 4 and 7 move verbatim to APP-082. Items 1, 2, 3, 5 and 6 stay (public entry, denials, replay refusals, abuse limits, trusted proxy and bounds). |
| APP-012 | Backlog | Kept whole. Scope lock: the minimal queries are the Northstar `total` and `timeline` views (plan Task 9); ratio, CIDR and money views are refused and belong to PH02. Log entry added. |
| APP-013 | Backlog | Kept whole (Tier A: transactional command). |
| APP-014 | Backlog | Kept whole (Tier B: verification campaign on APP-013's checkpoints). |
| APP-078 | Backlog | Kept whole; already reduced by ADR-008 to the beta-critical rows. One reserved window. The preflight learns `QualificationImageDigest`/`OperatorImageDigest` as part of this ticket; the `Fn::conditionIf` template doubt (APP-075 review m10) is proven by the stack update or recorded as a limitation for APP-086. |
| APP-076 | Backlog | **Trimmed.** Item 7 moves verbatim to APP-083. Items 1–6 and 8–11 stay, including the rollback redeploy and the audit-chain verification. |
| APP-079, APP-080 | Blocked / Backlog (ADR-008) | Unchanged owners; pick-up positions assigned (§6). |
| PH02–PH06 (61 tickets) | Backlog/Blocked | After November 2, in the pick-up order. |

**Why these two trims are safe.** APP-074 item 4 re-proves, through the local routes, races that APP-073's campaign proves with observed PostgreSQL lock waits and that APP-009 item 5 proves at the session boundary ("both concurrent completion orders cannot mint an unauthorized session"); the public-API functional checks of identical retry and revocation at delivery stay in APP-076 item 2. APP-074 item 7 exercises provider timeouts and reconciliation through the signup flow; the APP-073 provider slice proves the same state machine against the loopback stand-in with every fault, and the MVP's invitations are a handful of SES-verified recipients provisioned by the operator, who runs `reconcile` by hand. APP-076 item 7 needs two verified identities racing one slot on the live deployment; the storage proof exists, the RDS lock-observation repeat is already APP-080's, and the public-route layer adds no new transaction. None of the three is an authority boundary that only the moved test would have covered.

## 4. Process accelerations

These apply from this record until the PH01 gate record is accepted; the Phase 2 plan then decides whether to keep them.

1. **Risk-tiered independent review.**
   - **Tier A — full review → fix → re-check:** anything that touches authentication, sessions, PKCE, cookies or CSRF (APP-009; APP-074 items 1–3, 6; APP-076 items 3–5, 9), RLS policies, definer functions or grants (every migration: 0006, 0007), the parser sandbox or the DuckDB child confinement (APP-012's worker and authorization-at-delivery), transactional commands and idempotency (APP-013), and the RDS window receipts (APP-078, APP-076). The re-check is limited to the fixed surface and the affected acceptance cases, by the same reviewer where possible.
   - **Tier B — one review, reviewer-applied small fixes:** operator packaging and CDK task definitions at count zero (APP-077), SSR pages and route wiring without an authority effect, receipt encoding and view states (APP-012's page), the APP-014 campaign, evidence tooling. The reviewer applies fixes of about twenty lines or fewer that change no contract and records each in the review's disposition table; the implementer is not re-engaged and there is no re-check round. A finding larger than that, or any finding against an authority boundary, promotes the slice to Tier A.
   - **Tier C — self-review plus the board check:** tickets, plans, ADR text, evidence READMEs.
2. **Evidence re-recording only at the RDS window.** Per-merge re-recording of campaign hashes, image digests and README counts is skipped. The binding for each landed slice is the exact-source CI run on the landing PR's tip (`tenant-foundation.yml`, `foundation.yml` and the others), recorded by run id in the ticket Log. APP-078 rebuilds the qualification and operator images from the final candidate and re-records their identities once; APP-076 records the deployed digest. A README whose counts are stale says so in one dated line instead of being re-run.
3. **Batched landing PRs.** Three landing PRs instead of one per ticket: PR 2 landed APP-073 and APP-075 on 2026-10-29 (merge `699b8a9`); PR 3 lands APP-010, APP-009, APP-077 and the ADR-010 board; PR 4 lands APP-074, APP-012, APP-013 and APP-014; PR 5 lands APP-078 and APP-076 evidence, the tickets and the gate record. Done is set for every ticket in a batch at its merge. Victor's merge word is needed only on the PR; reviews happen on the branch before it.
4. **Checklist-style gate record.** `evidence/phases/PH01/GATE.md` follows `Tickets/_tools/PHASE-GATE.md` but each row is one line: criterion → owning ticket → evidence path → CI run id or window UUID → limitation. The ADR-008 and ADR-010 deferrals are named by ticket (APP-079, APP-080, APP-082, APP-083) in the limits section. Prose is kept to the decision paragraph.
5. **Scope locks** (no re-litigation before November 2): APP-012 serves `total` and `timeline` only; definitions and sources are seeded by the operator; UI is the SSR pages for landing, invite, sign-in, pending, home, work and record; no canvas island beyond what APP-003 qualified; no new ADR amendment except the two this record names (D7.8 provider kinds and the ADR-003 custody decision are *deferred*, not opened).
6. **Parallel lanes.** Implementers work one ticket each in worktrees under `.claude/worktrees/`; reviewers are separate agents; the orchestrator alone merges, moves tickets and renders the board. Heavy Cargo builds stay at `CARGO_BUILD_JOBS=2` and at most three aggregate jobs; Docker campaigns run one at a time.
7. **Skips that are not corners.** Docker campaigns are not re-run locally after a merge that touches no storage file (CI runs them). The native campaign's run nonce (APP-008 review F5) and the guessed-ID shared case (F3) are APP-086's, not prerequisites. The `foundation.yml` 20-minute timeout is watched on the first run that builds the parser stage and raised in the same PR if it fails (parser review m6).

What is **not** accelerated: Tier A review depth, the RDS window's STOPPED/RUNNING observations, ledger reconciliation before any reservation, the single-operator rule, the no-LLM rule, and the rule that no local result is relabeled as hosted acceptance.

## 5. Deferred items and their destinations

Acceptance sentences moved verbatim:

| From | Sentence (abbreviated here; verbatim in the destination) | To |
| --- | --- | --- |
| APP-074 item 4 | "Concurrent last-slot redemption, identical retry, code/entitlement revocation and requests already waiting in admission preserve APP-073's real transaction results. … Record both race orders and restart/reopen behavior." | APP-082 |
| APP-074 item 7 | "Exercise provider create/revoke timeout and retry, remote revocation propagation, pending reconciliation, and signup completion after local code expiry/revocation/exhaustion. …" | APP-082 |
| APP-076 item 7 | "Through the public API, concurrent last-slot redemption admits exactly one principal, an identical retry returns the prior result without consuming a second slot, and code or entitlement revocation is refused at the next request and at result delivery; a late response or existing session cannot revive expired/revoked entitlement." | APP-083 |

Review residuals (minors not fixed, declined items, limitations, open questions, missing tests), grouped:

| Source review | Findings carried | To |
| --- | --- | --- |
| `evidence/APP-073/provider/independent-review.md` | M2 at-rest custody (ADR-003 amendment of 2026-10-29, for Victor's veto; Secrets Manager move), m4 (provider/code expiry divergence, documented only), m8 and the audit recommendation (D7.8 provider kinds `provider.invitation.{created,reconciled,revoked}` + migration), n1, n2 (pinned provider host allowlist), n5, n6, n7, n8; limitations: simulated transport, `reconcile` only on operator run, 300 s clock-skew assumption | APP-084 |
| `evidence/APP-073/admission/independent-review.md` | m4 (pre-0004 numbering in ADR-007 D7.7/D7.12, plan, operator strings), m5 (no entitlement extension; D7.19 sentence), m6 (plan Task 5 seed note), nits (0004 `entitlement_id` D7.3 note, `runtime.migrate` `gssencmode`/socket host, `--provider-email` in argv → APP-074 channel), implementation-choice notes (backfill writes no audit event; reconciler treats never-sent `create` + `revoke` as "do not create") | APP-084 |
| `evidence/APP-073/schema-0003/independent-review.md` | m7 (runner review m3 runtime half, record-only), observations (several live pending admissions per identity now bounded by m3(a); `record_pending_admission` context digest rests on the flow record; `replayed` returns epoch only; compromised workspace connection inherent to ADR-004) | APP-084 |
| `evidence/APP-010/independent-review.md` | m3 (publisher chooses the policy; server-side resolution before a second policy row), m7 (expenses source revision and `independent_review` policy row remain fixture SQL; no policy subcommand), r1 (permission events do not name the permission/resource; `forma:operator-permission:v1` digest domain in the next contract edition), coordinator choices (`grant-permission` allows `discover`; `create-app` has no kind; `source_read` grant event cannot name its source; revocations advance the epoch and grants do not; `ALTER POLICY` in 0005), catalog-size note (v5 body 219,611 B against the 419,430 guard) | APP-085 |
| `evidence/APP-075/independent-review.md` | m9 (harness tmpfs versus the Fargate volume; `check_image.py` case added, harness not), m10 (`Fn::conditionIf` for the disabled-state intrinsic, left to APP-078), Fargate environment names not yet allowlisted, `declared_control_plane_resource_id` needs a control-plane receipt, `check_image.py --base-revision` trivially true at the tip, real-transport campaign still via `Settings::fixture` | APP-086 |
| `evidence/APP-008/independent-acceptance-review.md` | F2 (prepush record statuses; superseded by CI), F3 (guessed/foreign-ID shared cases so they run on RDS), F4 (Log citation of `image/r5/independent-image.json`), F5 (run nonce, timestamps and source hashes in the native report); limits (timing side channels unassessed; `Lease::finish` post-COMMIT failures ambiguous for write owners) | APP-086 |
| `evidence/APP-017/parser-packaging/independent-review.md` | M4 (fuel cap makes D9.2/B5 bounds unreachable: amend, cut per-cell cost, or raise fuel by ADR-009 amendment), m1 (stale guest from the persistent target cache before a release image), m5 (wire `check-parser-record.py --manifest` into the activation preflight), m6 (`foundation.yml` timeout), m7 (1.5 GiB line unmeasured), n1–n6, SR8 signing still open (attestation declined on a private repository), `parser-release.yml` run 40963687082 `provenance_document_drift`, plan Task A4 interface line (`--manifest`), evidence path `parser-packaging/` versus the plan's `packaging/` | APP-087 (PH02) |
| `evidence/APP-015/adr-009-review.md` re-review | R-M1 (lease reclaim stalls the queue under the unique running index), R-m1 (`genui_jobs` schema USAGE, `SELECT(kind)`, fence grant as `genui_lock`), R-m2 (unlabelled amendments: D7.14 upload deadline, `parser-host` versus design D11/§6.4, D7.8 canonical-bytes rule, 0002:85 CHECK, ADR-004:20–21; B8/B12 marks; fixture exemption), R-m3 (failed recheck outcome; epoch/fence race), R-m4 (`queue_position` under the function set), R-m5 (job read authorization), R-m6 (23505/55P03 classification; one running per deployment) | APP-088 (PH02) |
| `evidence/APP-015/contract-edition/independent-review.md` | Items 5, 6, 7 kept-and-documented (derivable identity lists; `limit_exceeded` decided here; array framing, row grammar and unknown values left to APP-016/018), item 11 (`failure_code` vocabulary open), and the limitation that the Docker-backed native campaign using the seed was not run after the edition change | APP-088 (PH02) |
| `evidence/APP-027/board-split-review.md` | Note: PH02 exit criterion 3 omitted APP-081 | Fixed in `Tickets/plan.json` by this record |
| ADR-008 deferrals | Native Safari/Firefox, deployed negatives, rotation campaign, canary resource check | APP-079 (unchanged) |
| ADR-008 deferrals | Recovery drill, pool lifecycle, two-order concurrency with lock observations, drift/readiness rows | APP-080 (unchanged) |

Obligations this record defers and names explicitly, because earlier text called them due before APP-076:

- **Provider-operation audit events** (APP-073 Log: "owed before APP-076's first live invitation"). Deferred to APP-084. For the MVP's invitations (Victor and the demo principals, SES-verified), the record is the operator's retained `list` output per invitation plus fAuth's own management-key audit. Victor confirms (§8).
- **Capability at-rest custody** (ADR-003 amendment of 2026-10-29, GenUI default for Victor's veto). The MVP issues only synthetic beta invitations under the 0600-file default; the Secrets Manager move is APP-084's. Victor confirms (§8).

## 6. Post–November 2 pick-up order and strategy

Strategy: first the small items that harden the live beta or unblock Phase 2 (positions 1–6, about a week of mostly parallel work, none of which delays Phase 2's start); then the main product in phase order (acts 2 → 3/4 → 5/6 → packet); the RDS and identity campaigns (APP-080, APP-079) ride the first Phase 2 RDS window and FORMA-365 respectively, before any real organization. "The main product" after November 2 is the six-act demo; everything in positions 1–6 is sized to fit beside Phase 2's first sprint, not in front of it.

| Position | Ticket | Why here |
| --- | --- | --- |
| 1 | APP-088 (PH02) | ADR-009 re-review R-M1 and R-m1–R-m6 must be in the ADR text before APP-015's migration 0008 is written; doc-only, a day. |
| 2 | APP-087 (PH02) | The parser fuel bound (M4) decides what beta members can import; `check-parser-record.py` must be wired before the first PH02 deployment; precedes APP-017 Task A5. |
| 2a | APP-089 (PH02) | Base-image HIGH CVEs and scan-on-push that scans the arm64 child, before the first PH02 deployment ships a new image. Added 2026-10-30. |
| 3 | APP-083 (PH07) | The one live PH01 proof the MVP skipped; two verified identities and an hour on the running beta. |
| 4 | APP-082 (PH07) | Route-level race repeat and provider fault exercise on the local harness; hardens the public entry while invitations are few. |
| 5 | APP-084 (PH07) | Victor's ADR-003 custody decision and the D7.8 provider audit kinds + migration, before invitations go beyond the demo principals; the migration can ride 0008. |
| 6 | APP-085 (PH07) | Server-side policy resolution before PH03's second policy row (APP-037); the r1 digest domain rides the next contract edition. |
| 6a | APP-090 (PH02) | MVP UI fixes (demo counter island, HTML 404/503, select, receipt, contrast and focus, CSS budget) and the designer's mocks, before the demo recording. Added 2026-10-30. |
| 7 | PH02 Track A (APP-015–019) | Act 2: real import. Per `docs/planning/PHASE-2-EXECUTION-PLAN.md`. |
| 8 | PH02 Track B (APP-020–025) | Acts 2/4 in full: the 114,000 bytes/minute ratio, CIDR match, the 45,000-cent queue, six linked views. |
| 9 | PH02 Track C (APP-026–031, APP-081) | Act 1 navigation and the canvas. |
| 10 | APP-086 (PH07) | Managed qualification residuals, in the first PH02 RDS window (Task D0); includes the 2026-10-30 qualification refusal diagnosis and a qualification-only window kind. |
| 11 | APP-080 (PH05) | RDS races, drift and recovery, same window class; required before any real organization. |
| 12 | PH03 (APP-032–047) | Acts 3 and 4: authoring, publication, AI (the first model call, APP-043). |
| 13 | PH04 (APP-048–057) | Acts 5 and 6 in full; the recorded six-act rehearsal and packet. |
| 14 | APP-079 (PH06) | When FORMA-365 delivers; blocks any real organization. |
| 15 | PH05, PH06 remainder | Pilot controls and qualification. |
| 16 | PH07 gate | Closes when PH06 passes; every ADR-010 deferral Done. |

## 7. Board changes

- **PH07 "Post-MVP completion"**: order 7, depends on PH06, sprint **S13**, entry/outcome/deliverables/exit criteria in `Tickets/plan.json`. It holds APP-082–086. Its gate cannot pass before PH06's, which is correct for a completion phase; its tickets may be picked up as soon as their own predecessors are Done.
- **APP-087 and APP-088 are placed in PH02 (sprint S03)**, not PH07, because they are Phase 2 preparation whose residuals must be closed before PH02's gate can pass (the parser bound and the jobs model); the board forbids a dependency on a later phase, and PH02 tickets must not depend on PH07 tickets, so they live where their consumers are. Neither is a dependency edge of APP-015/017 (their orders would have to be renumbered); their Logs name the consuming tasks.
- **Orders** 82–88 are appended after APP-080 (81), so every dependency has a lower order without renumbering.
- **APP-010** moves to In review (coordinator's instruction after the re-check).
- **PH01 exit criteria** 3 and 5 name APP-083 for the public-API concurrency sentence; **PH02 exit criterion 3** gains APP-081 (APP-027 review note) and the PH02 deliverables name APP-087/088.
- `Tickets/external-dependencies.json` is unchanged.

## 8. Judgment calls for Victor to confirm

*Accepted by Victor on 2026-10-29 ("Accept"), all items as written.*


1. The MVP definition in §1: Phase 1 live, acts 2/3/5 not shown and not simulated, seeded sources and definitions labeled as seeded.
2. Trimming APP-074 items 4 and 7 and APP-076 item 7 to APP-082/APP-083 (§3, "Why these two trims are safe").
3. Deferring the provider-operation audit kinds (D7.8 amendment + migration) past the MVP's handful of invitations (APP-084).
4. Accepting the ADR-003 custody default (0600 file on the FileVault Mac, synthetic beta invitations only) for the MVP, or vetoing it, in which case invitations wait for the Secrets Manager move.
5. The review tiers in §4.1, in particular reviewer-applied fixes without a re-check for Tier B slices.
6. Batched landing PRs (Done set at the batch merge) and evidence re-recording only at the RDS window.
7. The PH07 placement for deferred PH01 work and the PH02 placement for APP-087/088.
8. The pick-up order in §6, especially positions 1–6 before Phase 2 Track A starts in earnest.
9. The APP-078 window on November 1 (UTC afternoon) and the APP-076 go-live trial on November 2, with Victor as the verified SES recipient completing the signup in installed Chrome from a second network.

## Consequences

- PH01 can reach its gate on November 2 with twenty tickets Done and the deferrals named (APP-079, APP-080, APP-082, APP-083), per QUALITY-GATES.
- The board grows to 88 tickets, 7 phases and 13 sprints. `docs/planning/MVP-NOV-2-PLAN.md` holds the day-by-day order; `CLAUDE-HANDOFF.md` carries the dated summary.
- Nothing is weakened in the controlling ADRs. A sentence moved by this record is verbatim in its new owner; a residual deferred by this record is in a ticket with the review file and finding ids; after November 2, the pick-up order in §6 is the plan until the Phase 2 plan supersedes it.
