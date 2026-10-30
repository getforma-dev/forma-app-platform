# APP-012 — independent review (Tier A)

Date 2026-10-29. Reviewed at `phase-one-spikes` tip `3f51d1a` (storage and worker half `3a1b5f3`/`6f0d1c3`; route and page half `3e17429`, `380b7d4`; integration `5430d84`, `57dd6be`, `3f51d1a`). While the review ran the branch moved to `7a085b9`, a documentation-only commit (ADR-006, ADR-003) that touches no APP-012 surface; the main checkout also carried another session's staged, uncommitted APP-013 review fixes (`http::problem` bodies, `contracts/validate.py`), which I did not review and which do not change any finding below. Reviewer: Claude (independent of the implementing sessions). Read-only: this file is the only repository write; nothing committed; no image built; nothing hosted or managed touched. Every run and probe used a `git archive` copy of `3f51d1a` outside the repository.

**Method.** (1) Read the controlling text: APP-012 acceptance and Log; ADR-010 §1 act 4, §2, §4.5; ADR-007 D7.9, D7.10, D7.14, D7.15; ADR-009 B1, D9.3, D9.8; ADR-004; design §§6.3, 7.2, 9; `contracts/{README.md,schema.json,validate.py}` and the Northstar fixtures; both author READMEs with their decisions and open questions. (2) Read every APP-012 source: `app/src/query/{mod,worker,receipt,routes,page,route_tests,tests}.rs`, `app/src/storage/queries.rs`, the parts of `storage/{authorization,definitions,sessions}.rs`, `supervise.rs`, `http/mod.rs`, `auth/{csrf,routes}.rs` and `main.rs` they call, `app/runtime/{query_engine,runtime,admission_operator,test_query_worker}.py`, `app/admin/src/work/app.tsx`, `storage/query_qualification.rs`, and `spikes/APP-005/query.py` at `25f92d6`. (3) Re-ran the README commands. (4) Wrote adversarial probes in the scratch copy only (appended to `query/tests.rs`, `storage/query_qualification.rs` and `main.rs` there, plus two Python probe scripts in the scratchpad).

## Verdict

**Changes requested.** No blocking finding. Four majors: one cross-organization admission defect, two receipt-validity defects, and one D7.9 conformance regression on the session-resolution status. Eleven minors. Every authorization claim I set out to refute held. Nothing reaches the engine before `run_query`. The release re-authorizes after execution, and five further premise changes beyond the campaign's three are each denied at release. The release precedes every response body. The confined child keeps its environment, descriptors, scratch and DuckDB lock, and a cancelled request kills it. No failed or denied card can carry a number.

## Verification re-run

| Command (in the scratch copy of `3f51d1a`) | Result |
| --- | --- |
| `app/admin`: `npm ci && npm run build` | exit 0 |
| `app`: `CARGO_BUILD_JOBS=2 cargo +1.93.1 test --locked` | lib **112 passed / 13 ignored**, bin 5, doctests 3 (the routes README's merged-tree row says 12 ignored; APP-014 added one since) |
| `app/runtime/.venv/bin/python -m unittest discover -s app/runtime -p test_query_worker.py` (Python 3.11.0, duckdb 2.0.0) | 11 OK |
| `OP -m unittest discover -s app/runtime` (`OP` = scratch Python 3.11.0 venv whose `pip freeze` equals `app/operator/requirements.lock`) | **132 OK** (115 s, includes the Docker PostgreSQL operator tests) |
| `GENUI_NATIVE_FIXTURE_PYTHON=OP GENUI_QUERY_PYTHON=OP GENUI_QUERY_REPORT=… cargo +1.93.1 test --locked --lib storage::query_qualification::native_query_campaign -- --ignored --exact --nocapture` (Docker) | 1 passed, **15 checks**, `passed: true`, 51.8 s. Authorized path p50 268.0 ms, p95 286.7 ms; child sequential p50 145.8 ms, p99 156.9 ms; wave of two p99 160.3 ms; peak RSS 54.0 MB |
| `GENUI_QUERY_PYTHON=OP GENUI_QUERY_LATENCY_REPORT=… cargo +1.93.1 test --locked --lib query::tests::engine_child_campaign -- --ignored --exact` | 1 passed. Sequential p50 140.7 ms, p95 149.9 ms, p99 156.8 ms; wave of two p50 147.0 ms, p99 152.7 ms; child's own time p50 80 ms; peak RSS 54.2 MB |
| `… auth::tests::native_session_campaign -- --ignored --exact` (Docker; regression check for the `pub(crate)` harness and `57dd6be`) | 1 passed, 18 checks |
| Review probes P1–P6, P15 (unit and binary tests in the scratch copy) | below |
| The query campaign with probes P9–P14 inserted (Docker, twice) | 1 passed both times. The 15 original checks are unchanged; the probe results are below |

The measurements agree with the README's within noise, so D7.10's gate (one wave of two at p99 ≤ 600 ms) passes on this host. Docker: only `gatewasm-db-test-1` (exited, not mine) before and after every run. Two `genui-native-*` containers of another session appeared once; I waited for them to finish and did not touch them. Every container my runs started was removed. 36 GiB free at the end. Not re-run: clippy/fmt, the MSRV check, the image build and `image_probe.py` (APP-078's), the operator unit tests, the other native campaigns, `smoke_http.py`, contracts lock/vectors, infra.

## Probes (what I tried to break)

| # | Probe | Observed | Judgment |
| --- | --- | --- | --- |
| P1 | Organization A fills admission (2 running + 20 waiting, 1 s fake children); then one request from organization B | B gets `QueryError::Busy` (429) | **M1** |
| P2 | Fake child answers valid rows after 2.6 s (interactive deadline 1.8 s, backstop 5 s) | The parent returns `Ok` rows | m1 |
| P3 | Request future dropped at 500 ms while the child sleeps 30 s | Child gone (`kill(pid,0)` fails), scratch empty, slot released | holds |
| P4 | Result line with an extra member; with a duplicated `outcome` key (`refused` then `rows`) | Both parse as rows (serde last-wins) | m3 |
| P5 | Wrong bytes (mode 0400) already at the revision's cache path, real child, two runs | `SourceUnavailable` both times; the file is never evicted | m4 |
| P6 | Window `2026-02-31T00:00:00Z`…`2026-03-01` and `0000-01-01T00:00:00Z`…`10:10` through `parse_parameters` → real child → `receipt::build` → `contracts/validate.py` | Both accepted. Results: `[[null]]` (rendered empty) and `[["1140000"]]`. The validator refuses both receipts (`ContractError`) | **M2** |
| P7 | Real child, measure `id` carrying an injected sub-select | `current_setting(...)` sub-select executed (columns `x, y`); `read_text('/etc/passwd')` and an `https://` `read_csv` failed `engine` | m5 (DuckDB's lock holds) |
| P8 | Real child: non-empty scratch, relative `cache_root`, `../`-prefixed organization | `scratch_refused`, `request_refused`, `request_refused` | holds |
| P9 | Source revision tombstoned (`deleted_at`) between execution and release | `release_result` Denied; next `run_query` Denied | holds |
| P10 | `saved_work.version + 1` between execution and release | Denied (a new run pins the new version) | holds |
| P11 | Entitlement `expires_at` moved into the past between execution and release, no epoch change | Denied | holds |
| P12 | Organization suspended between execution and release | Denied | holds |
| P13 | The member's `run` row deleted between execution and release, no epoch change | Denied | holds |
| P14 | Operator transaction holds the member row `FOR UPDATE` past `lock_timeout`; the member's live cookie on the JSON route and on `GET /workspace` | **503 `unavailable` on both**; engine dispatch +0 | **M4** |
| P15 | SSR work page with a hostile page title, view title, window echo and destination echo | All escaped; no raw `<script>` or `<img>` | holds |
| P16 | All 7 receipts from my campaign run, recomputed with `contracts/validate.py`; result digests against `forma:result:v1` of the oracle rows | Every digest recomputes; the `total` and destination result digests equal the oracle's | holds |
| P17 | Northstar fixture bundle (`impl-2026-10-28.1`) with its receipt cut to the read input (digest recomputed) → `validate_bundle` | Refused, "source evidence dependency mismatch" | **M3** |
| P18 | APP-005 identity: sha256 of `spikes/APP-005/query.py` at `25f92d6`, and an AST comparison | `d7d724b9…f187` matches. `POLICY` and `encode_rows` are identical; `connect`, `run_sql` and `compile_query` differ only by the named deviations plus a `bounds` check. Deviation 3 removed `validate_query` | holds; see M2, m5 |

## Findings

### Blockers

None.

### Majors

**M1 — One organization can make every other organization's queries 429 (admission queue is task-wide, not per organization).** `app/src/query/worker.rs:172` keeps one `waiting: AtomicUsize` for the whole task. `admit` (`worker.rs:285-294`) refuses at 20 waiters across all organizations, before it looks at the requester's organization. Design §6.3 (`design.md:303`) says "2 an organization, the rest waiting and 429 past 20 an organization". D7.10(3) says "a queue of 20" and does not settle per-organization versus per-task; the design does. P1: organization A at 2 running and 20 waiting turns organization B's first request into 429. On the live beta, one invited member with a runnable work and a loop can starve the other tenant (Northstar versus canary). This is cross-tenant availability interference, and the beta has no per-account query limit to stop it: the D7.14 buckets cover only sign-in, invite and resend (`auth/routes.rs:173`, `auth/invite.rs:388`, `auth/pending.rs:38`). **Fix:**
- Count waiters per organization: replace the `AtomicUsize` with a `Mutex<HashMap<String, usize>>` (or keep the count beside each organization's semaphore). Refuse when that organization's count is ≥ `QUEUE`, and decrement it in `Waiting::drop`.
- Optionally keep a task-wide ceiling for memory (for example `QUEUE * 2` in Phase 1).
- Add a unit test: B is admitted while A's queue is full; A's 23rd request is still 429.
- Say "per organization" in D7.10(3) (Tier C).

**M2 — Run parameters are never validated as contract timestamps, so impossible windows execute and the route releases a receipt the contract validator rejects.** `app/src/storage/queries.rs:209-230` (`timestamp`) checks shape and field ranges only: any day 1–31 in any month, and year 0000. `parse_parameters` (`queries.rs:250`) then compares the strings lexically. Deviation 3 (`query_engine.py:15-17`) removed APP-005's `validate_query`. That function validated the whole query, window included, against the contract (`spikes/APP-005/query.py:57`, called first in `compile_query`). The Rust side re-admits only the stored definition, so the effective query (stored query plus the window and destination added by `Parameters::apply`) is validated nowhere. P6:
- `2026-02-31` returns rows `[[null]]`, which the page renders as "No rows matched". The out-of-range cast never raises because Parquet statistics prune the scan.
- `0000-01-01` (DuckDB: year 1 BC) returns **1,140,000**.

Both receipts carry the submitted timestamp in `parameters`, and `contracts/validate.py` (`valid_timestamp`, `datetime.fromisoformat`) refuses both. `release_result` does not look at the receipt, so both are delivered with 200. A crafted JSON body or form post reaches this; the browser's `datetime-local` control does not. This breaks ADR-010 §2's receipt invariant: the number is shown with a receipt that is not a valid contract document. **Fix:**
- Expose the admission validator's existing `Time` rule (`app/src/admission/validate.rs:1823-1829`, built from `calendar_date` `:1870` and `clock` `:1893`) as `pub(crate) fn contract_timestamp(&str) -> bool`, and make `queries.rs::timestamp` call it.
- Defense in depth in `query_engine.py` `compile_query`: `datetime.datetime.strptime(v, "%Y-%m-%dT%H:%M:%SZ")` on both window bounds → `Refused("window_refused")`.
- Tests: refuse `2026-02-31`, `2026-02-29`, `2026-04-31` and `0000-01-01`; accept `2028-02-29`.
- Correct deviation 3's docstring: the effective query is no longer schema-validated in the child.

**M3 — The receipt's `source_revisions` lists only the input the view reads; the contract lists every source dependency of the work.** `app/src/query/receipt.rs:53-58` emits one entry, and storage README D4 records this as a decision. The contract says the opposite in three places, and no edition or ADR-002 amendment records a change:
- `contracts/validate.py:420-421`: `validate_bundle` requires `work.source_dependencies == required_sources == receipt.source_revisions`.
- Both Northstar fixtures (`contracts/fixtures/impl-2026-10-28.1/northstar.json`, `…/impl-2026-10-29.2/northstar.json`): the `total` receipt lists `events` and `inventory`.
- `contracts/README.md:32` ("A receipt records the concrete sources…"). Edition two's `partial` rule separately computes "the inputs the receipt's view reads", which presupposes that the list is wider than the read inputs.

P17: a fixture receipt cut to the read input fails `validate_bundle`. The campaign's `contract_valid` (`storage/query_qualification.rs:67`) calls only per-entity `validate`, so it cannot see this. A contract-conformant implementation would produce a different receipt digest for the same run. That breaks receipt reproducibility, and it matters before APP-013/023 begin citing receipts from outcomes. **Fix**, either:
- (a) **Recommended.** In `run_query`, read the work's `work_source_dependencies` (input, source, revision, digest; ordered by `input_id`) under the same fence; the precheck already requires live `source_read` on each. Carry them in `QueryRun`, emit them in `receipt::build`, and require equality in `release_result`. The engine still reads only `run.source`. Add a campaign assertion that a receipt plus the work passes a `validate_bundle`-style check. Or:
- (b) Open a contract edition that redefines the list as the read inputs, and update `validate_bundle`, both fixtures, the README row and the lock.

**M4 — A lock wait during session resolution now answers 503, which D7.9 forbids.** D7.9 (`ADR-007:72`, added with the APP-014 merge `8abd73a`) says: "in Phase 1 a lock wait past `lock_timeout` during session resolution itself … answers the uniform 404 like any other session-lookup failure … not 503, which design §9 reserves for provider or store outage". Seven minutes later, `57dd6be` (APP-012 OQ2) mapped every non-`Denied` `resolve_session` error to 503. That covers `GET /workspace` (`app/src/auth/routes.rs:437-438`) and the three APP-012 routes (`app/src/query/routes.rs:105-110`, `:140`). A 55P03 at `resolve_session`'s fence (`storage/sessions.rs:770`, `:331`) surfaces as `Error::Unavailable` (`authorization.rs:286-294`), so P14 gets 503 on both routes while an operator membership change holds the row. The routes README's justification ("those errors come from the store and are the same whoever asks") does not hold for this case. The lock is on this cookie's own member row, while a forged cookie stays 404. **Fix:**
- In `resolve_session` (`sessions.rs:749-805`), capture `let busy = result.is_err() && transaction.lock_timed_out();` before `finish`. Return `Err(Error::Denied)` when `busy` (the D7.9 Phase 1 rule, uniform 404), and keep 503 for genuine outage.
- Add campaign check P14 expecting 404 on `GET /workspace`, the JSON route, the page and the run form, with no dispatch.
- Correct the routes README OQ2 sentence.
- Alternatively, amend D7.9 to accept 503. That is the coordinator's choice; the code and the ADR must agree either way.

### Minors

**m1 — The parent never enforces the 1.8 s interactive deadline.** `worker.rs:392` computes `until`, but after `run_child` (`:422-437`) nothing compares the finish time with it. The only parent bound is the 5 s backstop from spawn, and the child's `deadline_ms` is the remaining time at spawn, measured by the child from its own `main()` after interpreter start-up. P2: a valid line at 2.6 s is accepted. The real child withholds late results itself (`query_engine.py:242-243`, `:285-286`), so this is defense in depth. **Fix:**
- After `run_child`, return `Failed(Deadline)` when `Instant::now() > until + 250 ms`.
- Pass `deadline_ms` minus a start-up allowance of about 100 ms.

**m2 — Budget overruns and queue expiry use the wrong status class.** `routes.rs:112-122` maps every `Failed(_)` to 503 `query_failed`. Design §6.3 (`design.md:311`, "A query past the budget: 422, never a partial number") and §9 (`:520`) give a budget overrun 422 `narrow_the_query`. A request that expired in the admission queue (`worker.rs:305-312`) is also `Failed(Deadline)`, so it becomes 503 `query_failed` and the page says "did not finish within its deadline", although it never started. **Fix:**
- Split `Failure::Deadline` into queue-expired and interrupted.
- Interrupted and `Killed` (RSS) → 422 `narrow_the_query`.
- Queue-expired → 429 with `Retry-After: 2` (page: the busy card).
- Keep 503 `query_failed` for engine/protocol failures.
- Or record the deviation beside the new codes in the D7.15 edition.

**m3 — The result parser is not strict.** `worker.rs:476-529` accepts unknown members and duplicate keys (P4), and does not check column names. **Fix:**
- Require the exact key set per outcome: rows → `{columns, elapsed_ms, outcome, partial, rows, total_scope, truncated_page}`; refused/failed → `{elapsed_ms, outcome, reason}`.
- Reject duplicate keys: `admission::decode` already does, and a Phase 1 line is far under its 1 MiB bound. Refuse anything larger.
- Require unique `[a-z][a-z0-9_]{0,63}` column names.

**m4 — A mismatching cached file is never evicted.** `ensure_cached` (`worker.rs:338-347`) treats any private regular file as present. Only the child checks its digest, and on `source_digest_mismatch` (`:489`) the file stays, so the revision is unavailable until someone deletes it by hand (P5). This fails closed. **Fix:** when the child reports `source_digest_mismatch`, re-hash in Rust and remove a mismatching file. Or verify in Rust before every spawn, which is cheap at Phase 1 sizes, and refetch.

**m5 — The child no longer validates identifiers it interpolates into SQL.** `query_engine.py:127` builds `AS "{item["id"]}"` from the measure id. Before deviation 3, APP-005's `validate_query` guaranteed a local id. P7 shows an injected sub-select executing; DuckDB's lock still blocked file and URL reads. Only a stored definition that passed APP-010 re-admission can reach this, so it is defense in depth. **Fix:** `require(re.fullmatch(r"[a-z][a-z0-9_]{0,63}", item["id"]) is not None, "query_shape_refused")` for every measure id (`column()` already closes field ids).

**m6 — Fetch deadline exceeds the handler deadline, and a killed fetch leaks its temporary file.** `FETCH_DEADLINE` is 10 s (`worker.rs:71`); the handler deadline is 5 s (`http/mod.rs:28`). A cold fetch cut at 5 s is SIGKILLed by `kill_on_drop`, which skips `fetch_revision`'s `finally` (`runtime.py:648-662`), so `.fetch-*` files accumulate in `<cache>/o/<org>/`. **Fix:**
- Bound the fetch by the request's remaining budget.
- Remove stale `.fetch-*` files in `Engine::new` and before a fetch.

**m7 — The "loading" card is a busy card.** `page.rs:285-291` (from `routes.rs:376`, `QueryError::Busy`) prints the badge `loading` with "Waiting: the query engine is busy. Run again in a few seconds", although nothing is loading. Routes decision 6 explains why: with no island there is no in-flight state. The state is distinct and number-free, so the acceptance wording is met, but the label misleads. **Fix:**
- Show the badge as "busy" (keep the `loading` state key if the acceptance mapping needs it).
- Say in the acceptance evidence that an in-flight loading state needs an island.

**m8 — `record_snapshot_sequence` rationale is stale.** `queries.rs:452-453` takes `max(commit_sequence)` over the work's revisions. README D4 says `organization_sequences` "arrives with 0007", but 0007 is on this tip, and D9.8 defines S as `next_commit_sequence − 1`. For Phase 1 (no record inputs) the work-scoped maximum identifies the same visible revisions, and the APP-013 review's R13 confirmed it agrees with the commits. **Fix:**
- Update D4.
- Switch to D9.8's S when record inputs arrive (APP-020+), and name that switch in APP-023's or APP-085's Log.

**m9 — The operator-path audit residual is not carried anywhere.** Storage OQ1 says the missing `register-source-revision`/`create-bound-work` audit kinds are "deferred with the other operator-kind work (APP-084)". `Tickets/0-backlog/APP-084.md` does not mention them, and ADR-010 §5 requires that no residual is lost. **Fix:** add the residual to APP-084 (Tier C).

**m10 — The outline lists every single-input query view, not only qualified ones.** `queries.rs:554-558`. With the edition-two Northstar definition, `rate` (aggregate plus ratio) would be listed and dispatched to the engine only to be refused, contrary to the routes README's "the page lists only what the qualified subset can run". The seeded edition-one definition has no `rate`, so November 2 is unaffected. **Fix**, either:
- Filter by the qualified operation types (`filter`, `project`, `aggregate`, `time_bucket`, `sort`, `limit`), or
- Correct the README.

**m11 — Digit grouping applies to text.** `page.rs:133-158` groups any all-digit string, so a digit-only text cell (an identifier) would render as a grouped number. Phase 1 fields are unaffected. **Fix:** carry each column's type from the engine (the `FIELDS` type or the measure operation) and group only integer columns.

### Nits

- Routes README merged-tree counts: 12 ignored is now 13.
- D1's revision id is `uuid5` re-stamped as version 4: deterministic, carrying the random-UUID version nibble. That is acceptable under the contract's syntax-only UUID check (`contracts/README.md:32`) and already documented in D1; ADR-009 D9.3 could say the same in one line.
- Design §6.3 keys the cache by digest, and D9.12 m3 evicts by `(organization, digest)`. The implementation keys it by `(source, revision)`. Equivalent in Phase 1; align in PH02.
- D7.10(2) says the credentialed fetch is "in the Rust binary". It is a Rust-spawned Python child with Rust re-verification, consistent with `app/README.md:7`'s AWS-SDK-in-Python rule, and worth one sentence in D7.10.
- `http::form` decodes lossily, so invalid UTF-8 in the destination becomes U+FFFD and runs as such.

## Acceptance

| Acceptance item | Evidence | Judgment |
| --- | --- | --- |
| An authenticated permitted principal runs the minimal typed query and receives an exact result with app/source/operator/parameter/policy/record-snapshot identities and result digest | Campaign checks 3 and 8 (route) re-run: 1,140,000 for [10:00, 10:10), 900,000 for `198.51.100.7`, the five timeline rows, through real APP-009 sessions. P16: every receipt digest recomputes with `validate.py`, and the result digests equal the oracle's | **Supported locally, changes requested**: M2 (receipts carrying unvalidated parameters) and M3 (source list against the contract) |
| The same query with foreign or revoked scope is denied before release, including when authorization changes between execution and delivery | Campaign checks 5, 6, 11, 12 re-run (32 foreign, pre-access and anonymous requests with no dispatch; `source_read`, creator-authority and membership revocation between execution and delivery). P9–P13 add tombstone, work version, entitlement expiry without an epoch change, organization suspension and `run` row deletion, each Denied at release | **Supported locally** |
| The native view exposes the supporting receipt and distinguishes loading, empty, failed and denied outcomes; an execution failure never displays a fabricated zero | Binary test `work_page_renders_every_view_state_without_a_fabricated_zero` (re-run); campaign checks 9, 12, 13; P15 (escaping). A failed or denied card is built from no result (`page.rs:279-301`), so it cannot carry a value | **Supported locally**, with m7 (the loading card is a busy card) |

Against ADR-010 act 4: 1,140,000 and the destination-filtered 900,000, the timeline, visible receipts and the four distinct states hold, and ratio, CIDR and money stay refused (`test_ratio_lookup_and_money_views_stay_refused` re-run). ADR-010 §1 act 4 also lists "reload restores". Neither the GET page (always idle) nor the no-JS POST provides that (a reload re-submits through the browser's dialog), and APP-012's acceptance does not name it. The coordinator should decide before APP-076 whether the re-submission counts or the reload clause waits for APP-025.

## Decisions and open questions

| # | Decision (storage half) | Judgment |
| --- | --- | --- |
| D1 | New revision and new bound work; the seeded rows are never rewritten | **Agree.** C05: pinned evidence is never rewritten. `register-source-revision` validates the envelope, reproduces the contract digest and is idempotent. `create-bound-work` copies the publication and bindings, rebinds one input to a live same-contract revision, and records the creator under creator authority, so no work grant is written. The seeded `…0069` is `SourceUnavailable` (campaign check 4). Audit residual: m9. |
| D2 | `events_v1` fields | **Agree**; the test's Parquet hash equals APP-077's upload. |
| D3 | The window is a closed, typed run parameter | **Agree** in design; its validation is M2. |
| D4 | Receipt fields with no Phase 1 column | `policy_epoch` = the publication's `policy_version`: **agree for Phase 1**. Publications are immutable and no other policy epoch exists; the contract minimum is 1. When server-side policy resolution or retirement arrives (APP-085), the epoch must become the authorizing policy version, re-checked at release. `record_snapshot_sequence` = the work's `max(commit_sequence)`: **agree for Phase 1**, with m8. `source_revisions` = the read input only: **disagree** (M3). `mode: published`, edition 1, `forma:query-receipt:v1` and `forma:result:v1`: agree (P16). |
| D5 | Child per request (measurement gate) | **Agree.** My runs reproduce the numbers (wave of two p99 ≈ 153–160 ms against the 600 ms gate). The decision is conditional on APP-078's Fargate re-measure with the same campaign shape, and that ticket should record the broker fallback trigger. |

| # | Open question (storage half) | Judgment |
| --- | --- | --- |
| 1 | No audit event for the two operator commands | Acceptable for November 2 under ADR-010 §4.5 (no new D7.8 kind); the retained command output is the record. Must be carried in APP-084 (m9). |
| 2 | Route mapping and release placement | Done in the second half; release placement is correct. Status mapping: see routes OQ1 and M4. |
| 3 | The seeded work keeps its placeholder binding | **Agree.** The demo opens the bound work. It needs a link (routes OQ5). |
| 4 | Inventory revision `…0071` keeps placeholder digests | **Agree.** `total` and `timeline` never read it; authorization rightly requires `source_read` on it. Under M3 fix (a) it appears in the receipt as bound, which is truthful. |
| 5 | Image and Fargate volume behavior | APP-078's; `EngineSettings::image()` fails startup closed if the directories cannot be made private (`main.rs:484-485`). |
| 6 | RSS sampling floor | Accept as a recorded limitation. |

| # | Decision (routes half) | Judgment |
| --- | --- | --- |
| 1 | Status mapping | **Agree** for the 404, 413, 422 `invalid_parameters`, 422 `narrow_the_query`, 422 `query_refused`, 429 and 503 `source_unavailable` rows. Two exceptions: budget and queue expiry (m2), and the lock-wait case (M4). |
| 2 | Refusal order | **Agree**, confirmed in code (`routes.rs:182-232`, `:403-498`): nothing reaches the engine before `run_query`, and `release_result` runs after serialization and before the body (JSON) or render (page). |
| 3 | No-JS SSR page; window required on the page, optional on JSON | **Agree** (ADR-010 §4.5). |
| 4 | View states | **Agree**, with m7 and m11. |
| 5 | Concurrent runs, releases after all computed | **Agree.** Each release is its own fenced transaction immediately before the render. A view released before a later view's denial keeps its number, which is the correct "authorized at release" semantics. |
| 6 | Server-side `loading` | Accept as the no-island compromise; label it busy (m7). |

| # | Open question (routes half) | Judgment |
| --- | --- | --- |
| 1 | New codes `invalid_parameters`, `query_refused`, `source_unavailable`, `query_failed` | **Keep all four.** `source_unavailable` is design §9's "cold query whose store or key is unreachable". `query_failed` covers only engine/protocol failure: a budget overrun is 422 `narrow_the_query` and queue expiry is 429 (m2). List them in the D7.15 contract edition (APP-013's), with `status` once the APP-013 M2 problem-body fix lands. |
| 2 | Store saturation → 503 | **Agree for genuine store outage only.** A 55P03 at session resolution must stay the uniform 404 per D7.9 (M4). `POST /auth/signout-all` and the APP-013 routes still map every error to 404, which matches D7.9 for that case. |
| 3 | Should the JSON route require a window? | **No change needed.** The whole-revision total (2,040,000) is exact, and its receipt records no window parameter, so it cannot be mistaken for the S02 number. Requiring a window would be a product rule, not an authority or contract rule. State in `routes.rs`' header that a view with no window of its own and no window parameter computes over the whole bound revision, and have APP-076's script post the windowed body. |
| 4 | `POST /w/{app}/{work}` | **Agree.** It is the minimum that makes an SSR page run anything. It carries fetch metadata, the 4 KiB bound, the session, a synchronizer token tied to the cookie (`csrf.rs:57-73`, exactly one token, constant-time), the query-string 404 and the closed form fields; a forged token is 403 before storage (campaign check 9). Replace it with an island only when one is qualified. |
| 5 | `GET /workspace` does not link the bound work | Not APP-012's acceptance, but ADR-010 act 1 shows "the home page listing the admitted app and saved work" and act 4 needs a way in. Give it an owner before November 2: an authorized work listing (APP-025, or a small follow-up on the APP-009/074 home page). The fallback is a typed URL in APP-076's script. |

## Claims I failed to refute

- **No dispatch before authorization.** Anonymous, pre-access, pre-access-as-session, canary replay, no-access, revoked and self-revoked requests never reach the engine (32 requests, campaign); the code calls the executor only after `run_query` returns `Ok` (`routes.rs:201-215`, `:352-371`).
- **Release denies on any premise change.** After `source_read` revocation, creator-authority revocation, membership revocation (campaign), tombstone, work version change, entitlement expiry without an epoch change, organization suspension and `run` row deletion (P9–P13), and for another principal or organization.
- **Release before every response body**, on both routes.
- **Confined child.** `env_clear` (only `sh`'s own `PWD`/`SHLVL`/`_` appear); descriptors 0–2 only (close-on-exec before exec, `RLIMIT_NOFILE` 64, no core); cwd is the private 0700 scratch outside the cache; scratch removed after exit, backstop kill and cancellation (P3). Cancellation at the handler deadline kills the child (`kill_on_drop`).
- **DuckDB lock.** `enable_external_access=false`, `lock_configuration=true`, autoload off and `threads=2` hold; reading a file outside the allowlist, `INSTALL httpfs`, and file or URL reads through an injected sub-select all fail (P7).
- **Python child refusals.** Another organization's cache, a symlink into it, a digest mismatch, a foreign `s3_key`, unknown request keys, traversal ids, a relative cache root and a dirty scratch are all refused (P8).
- **Digest verification on both sides.** The fetch child verifies before rename, and Rust re-hashes fetched bytes and removes a mismatch.
- **Never zero.** A deadline or kill reports `failed` with no rows; a failed or denied card carries no number; empty is "No rows matched".
- **Admission bounds.** 2 per organization, 4 per task and 20 waiting, with queue time charged to the deadline (fairness aside: M1).
- **Bodies only.** Filters travel only in closed bodies; any query string is 422 (JSON) or the uniform 404 (page).
- **CSRF.** The run form's fetch-metadata check plus synchronizer token.
- **Escaping.** SSR escapes every echoed value (P15).
- **Receipts and results.** Receipt and result digests reproduce under `validate.py` (P16).
- **APP-005 source identity.** The copy's identity and its verbatim `POLICY` (P18).
- **Measurement gate.** Passes on this host.

## Limits of this review

- Local Docker PostgreSQL with password TLS, not RDS IAM.
- Sessions came from the `cfg(test)` loopback issuer.
- The engine ran on macOS arm64 with the locked venv, not the linux image under Fargate.
- The S3 fetch was exercised only through the botocore Stubber and fake fetch children.
- No browser was used; the page was checked through the binary SSR test and my escaping probe.
- APP-076's hosted items, APP-078's image and window, and the deferred island states are outside this review.
