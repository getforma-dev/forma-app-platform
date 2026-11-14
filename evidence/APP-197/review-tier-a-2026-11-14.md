# APP-195 and APP-197: independent Tier A review (2026-11-14)

Reviewer: Claude (Opus 5.5), as an independent Tier A reviewer. I did not write either branch. This review changed no code, ticket or ADR. Its only commit is this file.

| Branch | Head reviewed | Base | Range |
| --- | --- | --- | --- |
| APP-195 `trial/app-195-refresh-herd` | `383ba23a` | `origin/main` at merge-base `778f306d` | `778f306d..383ba23a` |
| APP-197 `trial/app-197-catalog-cadence` | `06e8eeb8` | stacked on APP-195 at `72ab2df3`, plus `origin/main` `778f306d` | `383ba23a..06e8eeb8` |

## Verdict

- **APP-195: no BLOCKER, no MAJOR.** The pool change is sound. The client cannot make a retry storm and never retries a command. Three MINORs and two NITs. The ticket rightly stays Backlog: run A of `refresh-herd` does not pass.
- **APP-197: no BLOCKER, one MAJOR.** The cadence code is correct: every guard I checked holds, and the four campaigns pass. The MAJOR is a test gap. The 30 s bound, the point of the change, is not proven: a window that restarts on every reuse passes the shipped campaign. Fix the test (a ready probe is below) before acceptance item 1 stays checked. Three MINORs, on the ADR's accuracy and the quarantine row, and two NITs.

## Scope and method

- Read `AGENTS.md`, `app/AGENTS.md`, `app/admin/AGENTS.md`, `docs/DEVELOPMENT.md`, ADR-004 (§Pool and driver lifecycle, §Migration and recovery, A5), ADR-007 (header, D7.9), `evidence/APP-008/native/independent-review.md`, `evidence/APP-008/native/r5/independent-review.md`, all of `evidence/APP-195/` and `evidence/APP-197/`, and the APP-195, APP-197, APP-198 and APP-199 tickets.
- Read the full diffs of both ranges: `pool.rs`, `catalog.rs`, the `qualification.rs` A5 and waiter checks, `live.ts`, `board.ts`, `document.ts`, `queries.ts`, `records.ts`, `ledger.ts`, `record-components/controller.ts`, `campaigns.tsv` and the docs.
- Read deadpool 0.13.1 (the locked version) `managed/pool.rs`: `timeout_get`, `try_recycle` and `UnreadyObject::drop`.
- Audited migrations 0001–0022 for every grant, ownership change and function that the four pool roles can reach. A script scanned all 15 `CREATE FUNCTION` bodies for DDL keywords, `EXECUTE` and `set_config`.
- Probed PostgreSQL 16.15 (the fixture image `postgres@sha256:5db6dac2…`) in a throwaway container: which catalog writes does a role with no CREATE or TEMP still have?
- Redid two A5 server break proofs and two client break proofs. Ran three gap probes (one server, two client) and a temporary window-boundary test. Every file was restored byte-identically and checked against `git show HEAD:<file>`.
- Did not run `refresh-herd`, as instructed.

## Commands and results

| Command | Where | Result |
| --- | --- | --- |
| `just check` | APP-195 `383ba23a` | 21/21 steps ok (6 min 18 s). Library 415 passed, 35 ignored. Admin 308/308. `py-promotion` 183 tests OK (4 skipped). `msrv` ran with 1.85.0 (not a skip). `board` OK: 195 tickets |
| `just check` | APP-197 `06e8eeb8` | 21/21 steps ok (6 min 18 s). Same counts. `py-promotion` ok. `board` OK: 198 tickets |
| `just check admin` | APP-195 | ok (21 s). Tree clean afterwards |
| `just campaign postgres` | APP-197 | `test result: ok. 1 passed` (50.85 s), 37 checks. Report: `.cache/reports/postgres.json` |
| `just campaign managed` | APP-197 | ok (11.17 s). Its `panicked at … tests.rs:1471` line is the synthetic case under test, and the receipt shows cleanup confirmed |
| `just campaign sharing` | APP-197 | ok (13.46 s) |
| `just campaign live-sharing` | APP-197 | ok (20.71 s) |
| Server breaks B1, B2; probes B3, P1, P2 | APP-197 | See [Break proofs](#break-proofs-redone) |
| `just campaign postgres` after all restores | APP-197 | ok (47.01 s), 37 checks. This run was a fresh rebuild from a clean tree at `06e8eeb8`, after the last restore (`pool.rs` `5dcc24d0…`, `qualification.rs` `e1961e47…`) |
| Client breaks C1, C2; probes G1, G2 | APP-195 | See [Break proofs](#break-proofs-redone) |

Host: a shared Mac, 10 cores, load average 6–7. A campaign from another worktree (`forma-w4`, `private-entries`) ran at the same time. Every campaign used its own labelled Docker database.

## Findings

### MAJOR

**M1 (APP-197). The 30 s upper bound is never tested. A window that restarts on every reuse passes the campaign.**
`app/src/storage/qualification.rs:1020-1026` (and `:961`, `:1119`); `app/src/storage/pool.rs:33`, `:506-514`.

- **Why it is missed.** Every A5 assertion advances the test clock by `pool::CATALOG_RECHECK` from a connection's last *reuse*, and no assertion fixes the value 30 s. So two regressions keep the campaign green:
  - **Sliding window.** `catalog_verified_at = now` on every successful reuse, not only after a recheck. Probe B3 applied this to the committed `pool.rs` and ran the shipped campaign: **the whole campaign passed, 37 checks** (rerun, 50.68 s).
  - **A larger constant.** Any `CATALOG_RECHECK` above the campaign's own run time passes, because the tests advance by the constant itself.
- **Failure scenario.** A refactor "refreshes" the timestamp on reuse, which looks like a harmless optimisation. Under steady traffic a connection is reused every few hundred milliseconds, so it is never compared again while it serves. Catalog drift on it then lasts until the pool retires the connection, not 30 s. ADR-004 A5's "at most 30 s" becomes false, and CI stays green.
- **Acceptance item 1** says "Each is proven by breaking it". That holds for the seven guards that were broken, but not for the bound itself. The record also says that A5-1 to A5-6 were not re-run against the final test file. B1 and B2 below close that for A5-2 and A5-6.
- **Fix.**
  - Replace the "after the window" block at `qualification.rs:1020-1023` with a reuse at 20 s and one at 29 s (both tolerated, no comparison), then one at 31 s (compared and refused). All three are measured from the connection's creation, with the drift injected mid-window. This exact probe, P1 (text in the [appendix](#appendix-the-window-probe-p1)), passes on the committed `pool.rs` (`test result: ok`, 37 checks) and fails on the sliding-window break (it panicked at the probe's `PROBE 31 s: the reuse compares and refuses`).
  - Pin the constant: `const _: () = assert!(CATALOG_RECHECK.as_secs() == 30);` in `pool.rs`, or a unit test.

### MINOR

**m1 (APP-197). Inside the window, every reuse reads the application-owned version relation with no structural check, and after the session-baseline check. A5 relaxes a rule of ADR-004 §Migration and recovery without naming it, and readiness's ordering changes too.**
`app/src/storage/pool.rs:452-465` (`verify_checks`), `:500-511` (`recycle`), `:917-923` (`readiness`); ADR-004 line 60 ("validate catalog structure before reading application-owned version/history relations"), line 73 ("Readiness … runs all three on every probe (unchanged)").

- **What changed.** Before A5, `recycle` compared the catalog before `verify_schema_version`. That was the R4 property: zero trap execution on reuse. Inside the window, `recycle` now runs `verify_baseline`, then `verify_identity`, then `SELECT version FROM genui_meta.schema_version` with no structural check on this checkout, and then returns the connection. The same applies to `Stores::readiness` on a reused connection: its lease's recycle reads the version relation before readiness's own comparison runs. So "readiness … (unchanged)" does not hold for ordering. The campaign's metadata-view check (`qualification.rs:940-970`) now advances the clock before it asserts zero marker executions. Nothing asserts what happens inside the window, where the trap does run.
- **Failure scenario.** An owner-side change gives `genui_meta.schema_version` a function to evaluate on read: a view, or an RLS policy whose `USING` calls a function. That function could call `set_config('search_path', …, false)` or set a `genui.*` scope. New connections refuse it. But for up to 30 s every reused connection runs it during recycle, *after* the baseline check, and the lease is handed out in that session state. The transaction's own scope is installed with `set_config(…, true)` (`authorization.rs:346`), which resolves through the session's `search_path`. This needs a DDL-capable identity, so it is inside A5's stated non-goal (a hostile administrator). But it is exactly the R4/R5 trap class that the APP-008 reviews required to be refused before any application relation executes.
- **Fix.** These cost nothing and keep the A5 gain:
  - In `recycle`, run `verify_baseline` *last*, after `verify_schema_version`. It reads only `current_setting`.
  - On an in-window reuse, read the version with `FROM ONLY genui_meta.schema_version`, after one `pg_catalog` query confirming that the relation keeps the OID recorded at the last comparison, `relkind = 'r'`, no row security and its two column types (`bool`, `int4`). That is sub-millisecond.
  - If both are declined, amend A5 explicitly. Name the §Migration and recovery sentence and the readiness ordering it relaxes, and say that an in-window reuse can execute what a replaced version relation evaluates.

**m2 (APP-197). The threat-model statement "hold no DDL privilege … cannot cause the drift this check detects" is broader than both the facts and the campaign check.**
ADR-004 line 83; `app/src/storage/qualification.rs:982-988`.

- **What is overstated.** The conclusion "no request path can cause catalog drift" is true; the audit is below. But PostgreSQL lets every ordinary role run `ALTER ROLE <itself> SET …`. In the probe, `genui_data`, with no CREATE, TEMP or CREATEROLE, wrote two `pg_db_role_setting` rows (`statement_timeout=0` globally, `work_mem=64MB` in `genui`) and created a large object. The catalog's `role_database_settings` section compares those rows. Such a setting only affects *new* sessions, which are always compared, so there is no security consequence. Separately, the new DDL assertion covers schema CREATE and ownership, relation ownership, database CREATE and TEMP, and role flags. It does not cover function or type ownership (`pg_proc.proowner`, `pg_type.typowner`).
- **Fix.**
  - Reword the bullet: the storage module issues only fixed statements; the four roles own nothing and hold no CREATE or TEMP; PostgreSQL still lets each role set its own per-role defaults, which apply only to new connections, and those are always compared.
  - Add `pg_proc` and `pg_type` ownership to the assertion.

**m3 (APP-197). The quarantine row's limitation is stale on this branch.**
`docs/DEVELOPMENT.md:109`.

- **What is stale.** The row still says run A fails "because every lease reuse repeats the catalog comparison", and that APP-197 "is meant to make it pass". On this branch a run A makes 4 comparisons, not 918, and still fails: first-try 200 is 70–74% and p95 is 6.8–8.4 s. The APP-197 evidence names the remaining causes: snapshot-limit 429s, database capacity, and A6 undecided.
- **Failure scenario.** After both merge, the row points readers at a cause that no longer applies and at a ticket whose own record says it does not make the campaign pass. ADR-012 needs the row to name the current limitation.
- **Fix.** State the post-A5 numbers and causes. Name what now makes it pass: the A6 decision and/or APP-198 and APP-199. Keep APP-195 as the owner.

**m4 (APP-195). The real-browser wiring of the new timing is untested. Two breaks leave all 308 admin tests passing.**
`app/admin/src/canvas/live.ts:208-221` (`browserTiming`); `app/admin/src/canvas/board.ts:1500-1527` (`onLive`).

- **The two probes.**
  - G1 changed the event name in `browserTiming.onVisible` (`visibilitychange` to `visibilitychanged`): 308/308 pass.
  - G2 made the board's `record.committed` hint run at once, with no spread and no deferral: 308/308 pass.
- **Failure scenario.**
  - With G1, a canvas tab that got a hint while hidden is never re-read when it is shown. It stays stale until the next hint, which is the "permanently stale visible page" this change must prevent.
  - With G2, the board's herd returns.
  - The work document is not exposed: its own 30 s interval and its `visibilitychange` call `shown()` (`document.ts:231-234`).
- The APP-195 record discloses that the board wiring has no test of its own. G1 is a further gap.
- **Fix.** Unit-test `browserTiming.hidden` and `onVisible` against a fake `document` (an `EventTarget` with a settable `visibilityState`). Test `onLive` (or an extracted function) with fake timing.

**m5 (APP-195). "Exactly one retry" holds per fetch. A hinted view refresh composes it with `runView`'s existing 429 loop: at most 8 requests.**
`app/admin/src/canvas/queries.ts:88-115` with `board.ts:803`.

- **How it composes.** `runView` makes up to four attempts (`MAX_BUSY_RETRIES = 3` after a 429, waiting `Retry-After`, clamped to 1–10 s). Each attempt goes through `liveRead`, which can retry one 503.
- **Why it is still safe.** The worst case is bounded and spaced (at least 1 s plus `Retry-After` between requests). `Retry-After` is a relative delay, so it keeps each client's 0–3 s offset and does not synchronise clients. So it is no storm. But the ticket and evidence say "a single retry", and the campaign models the view's 429s.
- **Fix.** Either state the composed bound, or pass the retrying `request` only to attempt 0.

**m6 (APP-195). Coming back `online` refreshes the work document at once, with no spread or retry. On one classroom Wi-Fi that event is synchronised.**
`app/admin/src/work/document.ts:241`.

- **Design context.** APP-195 deliberately leaves member-started refreshes unchanged. But `online` is not started by the member: when one access point recovers, about 25 devices fire it within seconds. The EventSource reconnect that follows already sends `stream.lagged`, which now uses the 0–3 s spread and the retry. So the immediate `online` read is redundant, and under load it is the one that shows "Updates are unavailable".
- **Fix.** Route `online` through `refresher.hint()`, or record the choice in the APP-195 decision and the class guidance.

### NIT

- **n1 (APP-197, `pool.rs:438`).** `connect` stamps `catalog_verified_at` after the checks finish, while `recycle` stamps the instant before they start. Use the pre-check instant in `connect` too (the struct literal at `:431` already has it). Then the window never overstates freshness by the duration of the check.
- **n2 (APP-197).** ADR-004 line 75 says "older than", and the ticket says "more than 30 s". The code uses `>=` (`pool.rs:508`). Align the words with the code.
- **n3 (APP-195, `live.ts:331`).** The discarded 503 response is not cancelled before the retry. `void response.body?.cancel()` frees the connection at once rather than at garbage collection.
- **n4 (APP-195, `pool.rs:178-186`).** The `ADMITTED` comment says the change "keeps ADR-004 §Pool and driver lifecycle true". That is right. Under a herd it also means commands queue FIFO behind up to 30 reads, for up to 5 s, rather than being refused at once. APP-195 run A measures this; keep it in the runbook text.

## Verified sound

**A5 (APP-197)**

- **Only the comparison is skipped.** `recycle` (`pool.rs:487-523`) runs all of the following unconditionally on every reuse: `clean` and `is_closed`, the credential generation and fingerprint, `verify_baseline`, `verify_identity` and `verify_schema_version`. Only `Catalog::verify` depends on `recheck`. No other path hands out a pooled object: `Lease::acquire` goes through `Pool::get`, and so through `recycle`. Only `Lease::release`, after a verified reset, returns one. B2 shows that skipping the version check is caught.
- **A failed recheck discards the connection.** In deadpool 0.13.1, an `Err` from `recycle` returns `Ok(None)` from `try_recycle`. `UnreadyObject::drop` then decrements the pool size, detaches the object and drops the `Connection`. Its `Driver` aborts, and `DriverFuture` releases both capacity permits only after the transport drops. The A5 check's count (+2: the refused reuse, then the refused replacement) is consistent. A failed readiness comparison drops the lease, and `Object::take` detaches it.
- **The timestamp belongs to one physical connection.** `catalog_verified_at` is a private field of `Connection`, which is not `Clone`. It is written only in `connect`, after `verify_admission` passes, and in `recycle`, after a passing recheck (`pool.rs:424-438`, `:512-514`). A reconnect is a new `Connection` from `connect`, with the full comparison. A cancelled recycle leaves the object to `UnreadyObject::drop`.
- **Readiness still compares.** `Stores::readiness` calls `verify_admission`, which is `verify_checks(client, true)`; so does its diagnosis. Production runs it every 15 s (`readiness.rs:58-63`). The ordering caveat is m1.
- **The test clock exists only in tests.** `advanced_ms`, `advance` and `advanced` live in `TestHooks`, which is `#[cfg(test)]`, as are `Settings.hooks`, `Stores::hooks` and `Lease::hooks`. `RoleManager::now` adds the offset only under `#[cfg(test)]`. The `dev-stand` feature does not enable `TestHooks`. The non-test library and binary build in `http-smoke` and `msrv` (`--lib`). The offset only moves forward (`fetch_add`), and each `Stores::new(fixture.settings())` gets its own hooks.
- **Role drift is still refused on every reuse.** That covers flags, memberships, TLS, replication role, `search_path`, the blank scope, the generation and the schema version. Because every migration bumps `genui_meta.schema_version` in its own transaction, a migration applied while the service runs is refused on the very next reuse. A5 delays only object-level drift that leaves the version unchanged.

**What a reused connection can do inside the window if the catalog drifted (focus 1)**

The comparison is skipped for at most 30 s after that connection's last comparison, plus the 3 s transaction it starts. Readiness runs every 15 s. When its comparison fails it discards the connection it probed, but not the role's other connection, and `ready()` turns false only after 45 s without a success.

| Drift (needs a DDL-capable identity) | Effect inside the window |
| --- | --- |
| Grant added, e.g. `UPDATE` on `genui.publications` to `genui_data` (the A5 check's case) | The fixed SQL never uses it. One defence-in-depth layer is missing |
| Grant revoked | Statements fail with 42501, so the request fails closed |
| RLS disabled or a policy loosened (`USING (true)`) | The sampled queries also filter `organization_id=$1` explicitly, and the scope is set per transaction. A cross-tenant read would also need a query with no predicate of its own. A dropped policy with RLS still on denies by default |
| Unique index or constraint dropped (for example the command-claim key) | Idempotency or integrity can be violated by writes in the window |
| FK or internal trigger disabled | Writes can violate integrity in the window |
| Table replaced by a view, or a policy that calls a function | Read by the transaction. For `genui_meta.schema_version`, read by every reuse during recycle (m1) |
| Definer function redefined by its owner role | The new body runs with `genui_admit` or `genui_lock` rights on the next call |
| Per-role settings (`role_database_settings`) | No effect on existing sessions. New sessions are compared |

**The DDL claim against the migrations (focus 1)**

- **0001.** Four roles are created `LOGIN NOINHERIT` with no flags. It revokes CREATE on `public` from PUBLIC, revokes ALL on the database from PUBLIC, and grants only CONNECT, USAGE on `genui_meta` and SELECT on `schema_version`.
- **0002–0022.**
  - CREATE on `genui_private` goes only to the NOLOGIN definer owners, and each migration revokes it again: `genui_lock` at 0002:278–283; `genui_admit` at 0003:488–497, 0008:186–190, 0011:241–245, 0012:71–132, 0015:150–154, 0016:77–81, 0017:166–170 and 0020:71–75.
  - No object is given to an application role with `OWNER TO`.
  - All 15 functions are `SECURITY DEFINER plpgsql` with a fixed `search_path`, owned by `genui_lock` or `genui_admit`. Their bodies contain no DDL, `EXECUTE` or `set_config`, only DML.
  - Dynamic `EXECUTE format(…)` appears only in migration-time `DO` blocks.
  - EXECUTE is granted to workspace (most), control (`lock_member_authority`) and data (`lock_member_authority`, `lock_work_revision`).
- **Conclusion.** No request path can cause catalog drift. m2 covers the wording.

**The pool (APP-195)**

- `ADMITTED = 32` per role. The 33rd caller is refused by `try_acquire_owned` with no wait, and the campaign asserts the refusal takes under 100 ms. The admission permit lives as long as the lease, so 32 is the number running plus the number waiting.
- The deadline is fixed before admission (`pool.rs:536`) and wraps `pool.get()`, which includes queueing, recycle and creation. It stays one absolute 5 s bound.
- Waiters queue FIFO on deadpool's tokio `Semaphore`, and idle objects are taken FIFO.
- Memory: at most 30 parked request futures per role (120 in the process), each with one semaphore wait node and no buffers.
- The connection counts (two per role, eight in all) and the 5/3/2 s bounds are unchanged.
- The test-only items compile out of the production build: `SATURATED`, `ACQUIRED`, `SERVICE`, `prepared_us`, `obtained`, `limit_admitted` and `catalog::VERIFIED`, including the closure bodies. They hold only counts and durations, no tenant data.

**The client (APP-195)**

- **Only reads retry.** `retryOnce` wraps only reads: the records and ledger GETs, the view POST, the record-components batch POST, and the document refresh (GET, or the `document_action=refresh` POST, which `query/route_tests.rs:851` pins as "not a command").
- **Commands never retry.** `submitCommand` and `postCommand` are unchanged. After a commit the document reads (`refresher.committed()`); it never resends.
- **No storm.** There is one retry per fetch, after a uniform 1–3 s (`Math.random` per client). `Retry-After` and server time are never used. An aborted read is never retried, and an abort cancels the wait. C1 confirms this.
- **Coalescing still works.** There is one waiting timer per key, and hints for that key join it. Events carry only kind, app and work, so a joined hint loses nothing. `now` cancels a waiting hint. The document's `queued` merges modes, with member winning. `runGroupHinted` clears the 250 ms `liveRuns` debounce.
- **No visible page stays stale.**
  - The document's 30 s interval and its `visibilitychange` both call `shown()`, so a visible document is at most 30 s stale.
  - The canvas resumes each stale key once, on `visibilitychange`; C2 confirms this. That relies on the untested `browserTiming` (m4).
  - A hint that arrives while a wait is running but the page is hidden still fires its timer.

**Campaign and quarantine**

- `refresh-herd` has `ci` = `-` in `campaigns.tsv`, and `run_ci` skips `-` rows (`scripts/dev/campaigns.sh:64`). No workflow at either head mentions it, and `origin/main` has no row for it. It is not red on main and not in CI.
- The `docs/DEVELOPMENT.md` row names the campaign, the date, the limitation (with its evidence) and the owning ticket, and the campaign still runs by hand. That meets the ADR-012 rule as written there. The rule's text speaks of workflows; this is a catalog campaign that was never in one. The content is m3.
- The evidence JSON contains no credentials, tokens or private data (a pattern scan found nothing).

## Break proofs redone

**Server** (`just campaign postgres` on APP-197, with the final `qualification.rs`). Each break was applied by exact-string replacement, run, then restored. The SHA-256 afterwards equalled HEAD (`pool.rs` `5dcc24d0…07ae`, and `qualification.rs` for P1/P2), and the worktree was clean after every one.

| Id | Break or probe | Result |
| --- | --- | --- |
| B1 (redo of A5-2) | `recheck` forced `false` | Failed at `qualification.rs:966`: marker `"4"`, expected `"0"`, as recorded |
| B2 (redo of A5-6) | `verify_schema_version` only when comparing | Failed at `qualification.rs:1077`: a reuse with version 1 was admitted, as recorded |
| B3 (new) | Sliding window: `catalog_verified_at = now` on every reuse | **Not caught.** On the rerun the whole campaign passed (37 checks, 50.68 s). The first attempt stopped earlier, at `qualification.rs:885` (`readiness` after a credential-generation change, `Deadline`). That was host load: the break only removes work, and B1, B2 and P2 all passed that line. That run is not counted |
| P1 (new probe) | Temporary test: drift mid-window, reuses at 20 s and 29 s tolerated with no comparison, 31 s compared and refused | Passed (37 checks, 54.41 s). On the committed code, 20 s and 29 s run no comparison and 31 s refuses |
| P2 (new probe) | P1 plus the B3 break | Failed at the probe's 31 s assertion (`qualification.rs:1033` in the probed file). The probe catches what the shipped campaign misses |

**Client** (`npx tsx --test …` on APP-195). `live.ts` (`dddfbc87…b656`) and `board.ts` (`f4eaef50…8293`) were restored to HEAD each time, and the suite passed again afterwards.

| Id | Break or probe | Result |
| --- | --- | --- |
| C1 (redo of "Only once more") | The second read replaced by a recursive `retryOnce` | Test 3 failed ("an unexpected random draw"): 6 pass, 1 fail. Restored: 7/7 |
| C2 (redo of "The canvas reads each stale key when shown") | The stale runs not called | Test 6 failed: 6 pass, 1 fail. Restored: 7/7 |
| G1 (gap probe) | `browserTiming.onVisible` listens for `visibilitychanged` | Whole suite 308/308 pass: **not caught** (m4) |
| G2 (gap probe) | The board's `record.committed` hint runs at once | Whole suite 308/308 pass: **not caught** (m4) |

## Limits

- The campaigns ran on local Docker PostgreSQL on a shared, loaded host. RDS behaviour, RDS catalog-query cost and the managed environment are not covered. Nothing here is managed or Phase 1 evidence.
- `refresh-herd` was not run, so its numbers are the authors'. I checked the code paths and reports that produced them only for consistency.
- The client review is code reading plus Node unit tests with fake timing. No browser ran, and the board wiring is unproven (m4).
- The DDL audit covers migration source and one PostgreSQL 16.15 probe. It does not prove the managed database's grants: there the catalog comparison at creation and readiness is the control.
- I did not review the 1,269-line `refresh_herd` campaign body line by line. I checked its registration, its quarantine, its test-only seams (`limit_admitted` and the counters) and that it is excluded from CI.
- B1 and B2 re-ran two of the seven recorded A5 breaks. A5-1, A5-3, A5-4, A5-5 and A5-7 were not repeated.

## Appendix: the window probe (P1)

This replaced `qualification.rs:1020-1023` (from `// After the window …` to its `assert!`) for one run each of P1 and P2, and was then removed. The `assert_eq!(compared(), before + 2, …)` that follows is unchanged. The probe advances 31 s in all, not 30.

```rust
    // REVIEW PROBE (temporary): reuses at 20 s and 29 s are tolerated, 31 s refuses.
    window.hooks().advance(Duration::from_secs(20));
    let before = compared();
    let mid = Lease::acquire(&window.data.inner).await.expect("PROBE 20 s: inside the window");
    assert_eq!(compared(), before, "PROBE 20 s: no comparison");
    mid.finish(false, Instant::now() + pool::TRANSACTION).await.unwrap();
    window.hooks().advance(Duration::from_secs(9));
    let before = compared();
    let late = Lease::acquire(&window.data.inner).await.expect("PROBE 29 s: inside the window");
    assert_eq!(compared(), before, "PROBE 29 s: no comparison");
    late.finish(false, Instant::now() + pool::TRANSACTION).await.unwrap();
    window.hooks().advance(Duration::from_secs(2));
    let before = compared();
    assert!(Lease::acquire(&window.data.inner).await.is_err(), "PROBE 31 s: the reuse compares and refuses");
```
