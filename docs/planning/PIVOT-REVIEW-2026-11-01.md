# Forma platform: November 1 pivot review

This review reconstructs the original design, the seeded pitch, today's clean-user decision, Grok's delivery, and the remaining work. Source baseline: `origin/pivot/pitch-mvp` at `c5b9452`. Review work is isolated on `codex/pivot-review-2026-11-01`; the combined candidate retains the four reviewed topic histories. Delivery details follow below; no deployment is implied. GitHub was read on November 1. Ticket status, branch implementation, CI and deployment are different facts.

## Assessment

The platform has a real foundation: hosted identity integration, scoped storage, typed admission, confined query/parser execution, persisted records with idempotent audited commands, permission-checked sharing and live-event infrastructure. The earlier seeded demo exercised much of that foundation. It also used rehearsed shortcuts and pre-existing users/apps, which cannot prove the new trial.

The clean first-user journey is **not yet complete**. The app-creation and united-page implementations have been built and reviewed, with verified defects repaired. The linked PRs record CI and landing. A recipient who has no account still cannot accept a work share and sign up through the product. The standalone app document remains the legacy work page. Today's retained beta observation still describes the seeded sandbox. A clean local persona is not a real hosted signup test. No billing/checkout functionality is implied by the phrase “paid-user journey.”

## Before, pivot, now, next

| Stage | Goal and implementation | What it proves |
| --- | --- | --- |
| October 23 delivered platform design | Industry-independent typed AppDefinition, trusted renderers, immutable publications, saved work, PostgreSQL records, Parquet/DuckDB evidence, identity/tenant boundaries and receipts. Two different apps on one build. | The intended architecture and acceptance obligations; a frozen design is not runtime evidence. |
| October 28–30 foundation and deadline cut | Published Forma 3.2 baseline; Phase 1 auth/storage/query/command work; ADR-010 cut to invite → query → receipt → command → reopen. | Much of the real plumbing exists and has tests; managed/phase acceptance remains explicitly incomplete. |
| October 30–31 seeded pitch | ADR-011 adds canvas, composer, 20-app library, folders/imports, workflows, sharing, agent surfaces and live hints. Diego/Sarah/Priya and demo-mode shortcuts support the rehearsal. | Broad product capability, but not a new customer's unaided start. |
| November 1 clean trial | First invite into an empty organization → describe an app → commit a record → share to someone without an account → signup → same work as a page/on their canvas. Three-minute cap applies only to video. | The right acceptance target. Real ingestion and loading are required even with fictional input data. |
| Source at initial review | Trunk included the planning pivot and earlier repairs. PRs 22, 25, 26, 27 were open; this review added repairs and tests. | Reviewed source and bounded checks, not completed signup sharing or a release. |
| After the trial | APP-097 reconciles retained pitch code/debt with PH01–PH07. APP-150–153 remain later ideas. | Long-term platform work remains visible; it has not been silently replaced by the demo. |

Primary sources: [delivered platform design](../../design/forma-generative-app-platform-v2/deliverables/v1/md/design.md), [delivered Northstar design](../../design/forma-northstar-demo/deliverables/v1/md/design.md), [ADR-010](../architecture/ADR-010-NOV-2-MVP-CUT.md), [ADR-011](../architecture/ADR-011-NOV-2-EMERGENCY-PIVOT.md), [first-run decision](first-run/README.md), [DIRECTION](DIRECTION.md). The old design's version, latency, capacity and cost assumptions are historical targets; this review does not requalify them.

## The handoffs and Grok inventory

Found both requested handoffs in the original checkout, untracked: `GROKASSIGNEDWORKNOTES.md` and `docs/planning/PIVOT-2026-11-01.md`. Their original bytes are preserved in [review evidence](../../evidence/pivot-review-2026-11-01/README.md). The earlier note's “still open” table is stale. The later note is the useful pivot handoff, but “implemented” still needs review and does not mean merged.

Git commits are attributed to Victor; the association with Grok comes from the handoffs and PR descriptions, not an independently authenticated model-author identity.

| PR | Work | GitHub state at review |
| --- | --- | --- |
| [5](https://github.com/getforma-dev/forma-app-platform/pull/5) | Dev workspace auth binding; records-only work pages; authorized canvas deep link | Merged |
| [6](https://github.com/getforma-dev/forma-app-platform/pull/6) | Clean-customer audit and corrected verdicts | Merged; audit is partial |
| [7](https://github.com/getforma-dev/forma-app-platform/pull/7) | Rehearsal corrections, including failed live-refresh claim | Merged; seeded rehearsal |
| [8](https://github.com/getforma-dev/forma-app-platform/pull/8) | Contract/admission gap matrix | Merged; gaps remain |
| [9](https://github.com/getforma-dev/forma-app-platform/pull/9) | Real files/folders browser audit | Merged; partial audit |
| [10](https://github.com/getforma-dev/forma-app-platform/pull/10) | Development dry-run corrections | Merged |
| [11](https://github.com/getforma-dev/forma-app-platform/pull/11) | Generated library checked in contract qualification | Merged |
| [12](https://github.com/getforma-dev/forma-app-platform/pull/12) | Library card drag/drop and footprint repair | Merged |
| [13](https://github.com/getforma-dev/forma-app-platform/pull/13) | Invitation races/provider-fault campaign | Merged; synthetic provider evidence |
| [14](https://github.com/getforma-dev/forma-app-platform/pull/14) | Parser release artifact directory correction | Merged |
| [15](https://github.com/getforma-dev/forma-app-platform/pull/15) | Canvas/matcher/composer architecture and evidence map | Merged |
| [16](https://github.com/getforma-dev/forma-app-platform/pull/16) | APP-013 review fixes rechecked | Merged |
| [17](https://github.com/getforma-dev/forma-app-platform/pull/17) | Clean first-run trial decision | Merged; plan, not trial implementation |
| [18](https://github.com/getforma-dev/forma-app-platform/pull/18) | Local sandbox versus clean trial twin | Merged; rule, not environment conversion |
| [19](https://github.com/getforma-dev/forma-app-platform/pull/19) | Read-only beta observation | Merged; beta still sandbox in that observation |
| [20](https://github.com/getforma-dev/forma-app-platform/pull/20) | App/page product tickets | Merged; requirements |
| [21](https://github.com/getforma-dev/forma-app-platform/pull/21) | Current product sequence and first-authoring-shell ticket | Merged; requirements |
| [22](https://github.com/getforma-dev/forma-app-platform/pull/22) | App/component/widget/folder model spikes | Open; identity wording corrected locally |
| [23](https://github.com/getforma-dev/forma-app-platform/pull/23) | Later favorites, navigation and API bank | Merged; future work only |
| [24](https://github.com/getforma-dev/forma-app-platform/pull/24) | Library-pane tab spike | Merged; question, not redesigned UI |
| [25](https://github.com/getforma-dev/forma-app-platform/pull/25) | First authoring shell, migration 0016 | Open at `3a1784e`; local review repairs |
| [26](https://github.com/getforma-dev/forma-app-platform/pull/26) | Create modal before placement | Open at `ed4e496`; local review repairs |
| [27](https://github.com/getforma-dev/forma-app-platform/pull/27) | United app page, focus return, fan-out, resize and share menu | Open at `0d919f2`; local review repairs |

At the initial CI inspection PR22 and PR26 had 12 successful PR-triggered workflows each. PR25 had four failures (Foundation, Parser release, Pitch, Tenant foundation). PR27 had ten successful and two running. PR27 subsequently completed all 12 successfully. These observations bind those remote commits, not the local repaired candidate; full image/deployment acceptance is separate.

## What an app means in this implementation

In product terms, an app is the admitted experience and saved work the person uses. In storage, `app_id` can be an authoring container reused for multiple generated publications and saved works. The authoritative work identity is organization + app + work; the work pins its publication/definition. Records belong to the work. Share and page links must retain that full work identity, not resolve the newest publication merely by app id.

Cards/components are parts of that experience. Unite/page/focus/fan-out arrange or display those same parts; they must not copy the records. A stack is a visual collection, not a permission boundary. A widget is a product category, not proof it has no persisted backing: notes use governed records. A standalone HTML app page remains authenticated; “HTML page” does not authorize public anonymous access.

The current share menu grants read access by default. It does not make the recipient an app owner or automatically grant record write. App labels/aliases currently use app identity in places where multiple works share the authoring shell; independent naming is a remaining identity test, not a promise.

## Findings repaired during review

| Finding | Repair and evidence |
| --- | --- |
| PR25 made read-only command/catalog/action preparation create an app shell | Separate read-only context from explicit compose preparation; keep shell creation on intentional compose paths. Real database regression checks no-write reads and concurrent first composes. |
| PR25 could create a duplicate shell when the first one was published while another compose waited | Acquire the member advisory lock before selecting an authoring app; deterministic database regression failed before the fix and passed after it. |
| PR25 could grant a shell after the entitlement expired during that wait | Recheck current entitlement after acquiring the lock; expiry regression verifies denial with no app/grant/audit write. |
| PR25 omitted migration 0016 and catalog v16 from image allowlists | Admit both files in service, qualification and operator contexts. Existing source-packaging checks and schema qualification exercise the change; no migration bytes rewritten. |
| PR26 Create could retrieve existing work | Send the explicit generation intent; cancellation/refusal tests verify no placement. |
| PR26 advertised templates but only supplied four prompt starters | Label them example descriptions. True admitted-template instantiation remains incomplete. |
| PR26 favorites crossed members on one browser origin and storage exceptions could interrupt the flow | Scope to the existing member digest and tolerate unavailable browser storage. |
| PR26 had a failing existing admin test, while ordinary CI omitted admin tests | Repair the affected test, add creation regressions, and run admin tests in local check and Foundation CI. |
| PR27 narrow resized pages retained desktop columns/header | Reflow according to page width while preserving actual card nodes, drafts and receipts. Browser evidence belongs to the APP-142 review. |
| Fresh-checkout `just check` compiled RustEmbed before `admin/dist` existed | Build/typecheck/test admin before clippy and other Rust asset consumers. The failure was reproduced on a newly set-up checkout. |
| Stylesheet edits did not update the running development preview | The existing tsx watcher now restarts the full strict build on CSS edits; an edit-and-restore probe confirmed new CSS and manifest bytes without generated-file patches. |
| PR22 collapsed product identity with storage app id and claimed widgets have no app backing | Clarify app/publication/work identity, actual ownership and governed notes. |
| Current docs still sent contributors to APP-001 or the seeded recording; ticket dependencies and rollback claims contradicted the pivot | Update entry points, dependency order, migration/rollback wording and a separate 180-second recording outline, preserving design/history and existing evidence. |

Detailed command results and any remaining review findings are recorded below and in the owning ticket evidence. A regression test that uses a fake model or provider is labeled as such; it is not a hosted customer trial.

## Delivery review

Victor subsequently authorized updating the tickets and PRs, pushing the fixes and merging into the current repository trunk, `pivot/pitch-mvp`. ADR-013's frozen `main` branch is unchanged. PR review comments and descriptions carry the exact source checks; the connected account is also the PR author, so GitHub cannot record a self-approval. This is independent agent code review recorded as comments, not a formal GitHub approval event.

| PR | Reviewed fix head | Evidence |
| --- | --- | --- |
| 22 | `3382d785f1bbc2f96d856d000e74befedc0df3d3` | App/publication/work identity and governed widget corrections; board checks |
| 25 | `acba5466d427c210ecb45a6b7ae902d78e5519f5` | Read-only context, concurrency/expiry regressions, schema/image contexts; independent Tier A recheck |
| 26 | `febfaae53315d85c341804494dd7c22f47f6db43` | Explicit generation, member favorites, refusal/cancel regressions |
| 27 | `808cca3bb48f6bb8f785a15d02d0cb53668207d9` | Responsive page, bounded sharing, successful owned-work sharing/live browser evidence |

The retrospective source review also covered merged PRs 5–16: development-only issuer fallback, exact-work authorization/deep links, contract-library verification, library drag/placement, invitation fault/race fixtures and parser artifact paths; the evidence-only APP-013 close was checked against the current command/idempotency implementation. PRs 17–24 are planning/observation work; their original diffs were reconciled against current routes and ticket requirements. No additional blocking regression was found. Historical screenshots and hosted rehearsals were read as evidence, not re-executed or presented as fresh checks.

This read-through also confirmed that current provider invitation capabilities reach the service only on task start. APP-139 now explicitly requires runtime invitation creation/delivery without a per-share operator restart and without expanding ordinary workspace authority.

The handoff says Grok stopped after delivering PR 27 for review. A subsequent remote/worktree check found no new APP-139/143/144 branch or tracked work in the original checkout. A resident CLI process alone is not evidence of active implementation. APP-144 is the next isolated Codex lane; APP-139/143/140 remain unfinished. PR and CI links are the authoritative landing status; the inventory above records the initial review snapshot.

## Remaining work for the requested experience

| Priority | Owner | Required result |
| --- | --- | --- |
| Before first live compose | APP-145 | Independent source recheck is complete; require green exact-source CI and provision a real clean organization with the required space and owner-publish policy, no pre-created customer app; prove first compose on a member with no shell. |
| Current app creation | APP-141 | Finish actual template instantiation if templates are promised; keep generation labeled. Combine creation and APP-142 into one coherent page presentation, with cancel/refusal/reload evidence. |
| Current app presentation | APP-142 | Review and successful owned-work share/live update are evidenced. Preserve focus/fan-out, narrow layout and browser-local persistence while integrating creation. |
| Standalone app | APP-144 | Extend the existing authorized `/w/{app}/{work}` from a legacy query/new-record surface into the admitted parts and form as one usable document; use the same command and receipt path. |
| Required new recipient | APP-139 | Durable work-bound invitation, runtime provider capability delivery without a per-share task restart, verified recipient signup, no premature grants, acceptance expiry/revocation/replay tests, preserved destination, recipient page/canvas placement and real live update. Existing active-member sharing cannot stand in for this. |
| Contextual Ask | APP-143 | Bind questions to the selected app/part/widget without creating authority. This remains planned; APP-153 owns the later placement decision for the Ask pane. |
| Trial delivery | APP-140 | Correct beta to the clean world with reviewed images/schema; real accounts, actual provider/mail and clean local/live comparison; exact build rehearsal and a ≤180-second recording. |
| After Nov 2 | APP-097, APP-129, PH01–PH07 | Reconcile pitch debt; remaining authoring-rule/contract gaps, managed trial residuals, recovery and production qualification. APP-150–153 are later, not required evidence for this recording. |

Synthetic data is permitted. A preloaded app, seeded recipient, bypassed login or simulated response would still skip a required transition. Current `:4010` is useful for product interaction, but its automatic persona session and pre-created shell hide two of those transitions. It cannot alone qualify the whole journey.

## Validation and handoff

The combined candidate passed **all 19 `just check` steps**, with no failed or skipped step: 61 admin tests, strict native SSR build, both clippy variants, 346 Rust library tests, seven SSR tests, three doctests, HTTP/security smoke, Rust 1.85 compatibility, campaign catalog, 38 infrastructure tests, Python suites (17 + 179 + 10 + 13 + 12 + 56), schema pins, 33 contract tests plus canonical vectors/lock/library verification, board and shell syntax. Thirty-one opt-in Rust campaigns are intentionally ignored by the ordinary unit suite. [Summary](../../evidence/pivot-review-2026-11-01/check-summary.txt); [tested changed-source hashes](../../evidence/pivot-review-2026-11-01/candidate-source-sha256.json).

Focused backend evidence is [APP-145 review](../../evidence/APP-145/REVIEW-2026-11-01.md): final Composer 12 and MCP 12 checks, deterministic red/green duplicate-shell and expiry regressions, independent recheck of the authority changes. Schema qualification (378), native PostgreSQL (36), packaging (10) and script checks (17) passed; the report states which preceded the final wrapper refinements. SQL/catalog bytes stayed unchanged from PR25. These are disposable real-database campaigns, with synthetic model/provider boundaries where specified.

The [combined clean browser pass](../../evidence/pivot-review-2026-11-01/clean-browser.json) used fresh Chrome on this checkout's isolated `:6010`, demo mode off, empty library and no existing saved work. Cancel sent no compose request. Create sent `generate: true`, the real model produced an admitted Reading List in 6,984 ms, Unite opened its page, the real form committed a synthetic book, and a same-browser reload restored that page and record. [Screenshot](../../evidence/pivot-review-2026-11-01/clean-browser.png). The first harness attempt successfully generated an app but used an absent conditional Library selector; its [failure](../../evidence/pivot-review-2026-11-01/clean-browser-first-attempt.json) is retained. The selector was corrected to the existing header control and only this disposable profile was reset before the successful full rerun. No product response was substituted.

The local clean stand still uses automatic persona sign-in and an operator-created authoring shell, so this browser result proves product creation/persistence, **not real signup or APP-139/140 completion**. APP-145's database campaign separately tests a member with no shell. The [APP-142 browser review](../../evidence/APP-142/REVIEW-2026-11-01.md) proves successful owner-created sharing and a recipient record update without reload, within a ten-second test timeout (not an exact latency measurement), plus resize/focus/fan-out. [APP-141](../../evidence/APP-141/CODEX-REVIEW-2026-11-01.md), [model-spike corrections](../../evidence/APP-146/CODEX-REVIEW-2026-11-01.md) and [CSS watcher probe](../../evidence/APP-128/CSS-WATCH-2026-11-01.md) retain their focused results.

Independent document review tightened APP-140 so persona footage cannot qualify the required sign-in progression, even with disclosure. Board validation passes 152 tickets, eight phases, 14 sprints and all 80 requirement mappings; no ticket or phase was promoted to Done. Ticket Order values now express the recorded prerequisite sequence without changing APP IDs.

The fixes are committed on their original PR branches and assembled with their histories in `forma-pivot-review-20261101`, branch `codex/pivot-review-2026-11-01`. Exact-source remote CI and landing are recorded on the linked PRs. Owner listeners 4001–4003 and 4010 were untouched. Review stands are stopped and their local volumes retained. No AWS changes, image release, hosted authentication trial or completed recording is claimed. All three release images still need qualification on the final deployment candidate before any deployment.
