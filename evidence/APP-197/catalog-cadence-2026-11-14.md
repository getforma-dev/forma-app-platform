# APP-197: catalog comparison at most every 30 s on reuse (2026-11-14)

By Claude (Opus 5.5), on branch `trial/app-197-catalog-cadence`. The code is commit `e22027dd` and ADR-004 amendment A5 is commit `e1599297`; both sit on APP-195's `72ab2df3`. Local Docker PostgreSQL in a macOS VM, on a host shared with other builds (load average 7–11 during these runs).

## Short answer

- With A5, a data-role transaction costs **10.2 ms** in the test profile instead of 54.4 ms, and 7.6 ms in the release profile instead of 49.5 ms. A run of 20 transactions makes **0** catalog comparisons instead of 20. Run A, at 26 pages, makes **4** instead of 918.
- **Run A still fails in the test profile** (CI's profile). First-try 200 is 70–74% (the target is 90%), p95 is 6.8–8.4 s (the target is 6 s), and one herd had one page that was never current. On the other side, no read failed after its retry, delivery was 26/26 and the commands took 54–107 ms.
- **In the release profile run A almost passes.** Herd 1 meets every criterion. Herds 0 and 2 miss only first-try 200 (87.7% and 82.7%); their p95 is 5.5 and 5.8 s and every page is current within 6.1 s. Most first-try misses there are 429s on the typed view: the record snapshot limit allows four in memory, and the canvas retries after `Retry-After`.
- **What passes now:** 15 pages, with one commit every 5 s, in both profiles (it was 5 pages, or none, before). 20 pages fail on first-try 200 and p95.
- **A6's condition is met** (run A fails in the test profile, and the release p95 is above 5 s). Per the decision, A6 is drafted and not applied. An uncommitted A6 experiment is reported below.
- **Not un-quarantined.** `refresh-herd` stays out of CI, and its row in `docs/DEVELOPMENT.md` stays.

## Before and after (test profile, `just campaign refresh-herd`)

Before is APP-195's rerun with the revised criteria ([report](../APP-195/refresh-herd-revised-2026-11-14.json)). After is this branch ([report](refresh-herd-2026-11-14.json)).

| Measure | Before | After (A5) |
| --- | --- | --- |
| One data-role transaction alone, p50 | 54.4 ms | **10.2 ms** |
| Catalog comparisons in 20 transactions alone | 20 | **0** |
| One catalog comparison alone, p50 | 47.1 ms | 48.1 ms (unchanged; it now runs rarely) |
| One page's three reads alone, p50 | 1069 ms | 513 ms |
| Run A: catalog comparisons | 918 | **4** |
| Run A: data-role transaction service p50 | 172.7 ms | **32.8 ms** |
| Run A: workspace-role transaction service p50 | 135.8 ms (run B: 126.5) | 21.1 ms |
| Run A: first-try 200 by herd | 16.0%, 8.6%, 10.3% | 72.8%, 70.4%, 74.1% |
| Run A: reads failed after the retry | 55, 63, 32 | **0, 0, 0** |
| Run A: current, p95 / slowest page | none current | 7.9 / 15.5 s, 8.4 / 9.1 s, 6.8 / 6.9 s (25/26) |
| Run A: owner's command | 138, 2237, 4146 ms | 54, 107, 59 ms |
| Run A: admission refusals | data 211, workspace 28 | data 50, workspace 0 |
| Run B: first-try 200 | 21.8%, 23.1%, 20.5% | 35.9%, 34.6%, 34.6% |
| Run C (8 admitted): first-attempt 503s | 210 | 210 |
| Most pages that pass run A | none (5 passed two herds of three) | **15** |
| Shortest cadence that passes, for those pages | — | **5 s** (3 s fails) |
| Query worker `Busy` / `QueueExpired`, typed view | 0 / 0 | 0 / 0 (127 runs in run A) |

## Release profile (`cargo test --release`, same test)

[Report](refresh-herd-release-2026-11-14.json).

| Measure | Before (APP-195 at `029c13bb`, old criteria) | After (A5) |
| --- | --- | --- |
| One data-role transaction alone, p50 | 49.5 ms | **7.6 ms** |
| One catalog comparison alone, p50 | 36.5 ms | 40.7 ms |
| Run A: catalog comparisons | — | 4 |
| Run A: data-role transaction service p50 | — | 23.4 ms |
| Run A: first-try 200 by herd | 16.0%, herd 1 refused, 16.7% | 87.7%, **91.4%**, 82.7% |
| Run A: reads failed after the retry | 53, —, 35 | 0, 0, 0 |
| Run A: p95 / slowest page | none current | 5.5 / 5.6 s, 5.2 / 5.4 s, 5.8 / 6.1 s |
| Run A: view 429s by herd | 6, —, 1 | 10, 7, 15 |
| Run A: result | fail | fail: herds 0 and 2 on first-try 200 only; herd 1 passes |
| Most pages that pass | 5 | 15 (and 15 at a 5 s cadence; 3 s fails) |

## A6 (drafted, not applied)

The decision applies A6 (4 workspace, 4 data, 2 control and 2 worker connections, 12 in all) only if, after A5, run A fails in CI (test profile) or passes with a p95 above 5 s. Both are true: run A fails in the test profile, and even the release herd that passes has a p95 of 5.2 s. It was not applied; that waits for the decision. The draft text for ADR-004 §Pool and driver lifecycle:

> *Amendment A6, <date> (APP-197):* the workspace and data roles hold at most four connections each, and the control and worker roles two each: twelve connecting or live driver slots in all, instead of two per role and eight in all. The bounded waiter set (32 per role), the 5 s acquisition deadline, the 3 s transaction maximum and the 2 s cleanup bound are unchanged. A driver still owns its role and total capacity permits until its transport closes. Every PostgreSQL connection limit of the deployment (RDS `max_connections` and the IAM roles' `CONNECTION LIMIT`) must admit twelve service connections plus the operator's. ADR-007's frozen "two connections per role, eight in all" is amended for these two roles only. The qualification's expectation follows: `slots()` = 12, `[4, 2, 4, 2]`; `Stores::close` waits for 12 permits.

**The A6 experiment (uncommitted, test profile).** With A5 in place, `roles` and the pools' `max_size` were set to 4/2/4/2 (12 in all), `just campaign refresh-herd` was run, and `pool.rs` was restored. Its SHA-256 then equalled `e22027dd`'s (`5dcc24d0…`). [Report](refresh-herd-a6-experiment-2026-11-14.json).

| Run A, 26 pages | A5 only | A5 + A6 experiment |
| --- | --- | --- |
| First-try 200 by herd | 72.8%, 70.4%, 74.1% | 54.3%, 53.1%, 79.0% |
| Reads failed after the retry | 0, 0, 0 | 4, 5, 0 |
| p95 / slowest page | 7.9 / 15.5 s, 8.4 / 9.1 s, 6.8 / 6.9 s | 18.0 / 24.7 s, 15.3 / 16.4 s, 6.5 / 6.6 s |
| Data-role transaction service p50 | 32.8 ms | 57.9 ms |
| Data-role admission refusals | 50 | 86 |
| Most pages that pass; their cadence | 15; 5 s | 15; 5 s |

On this host, doubling the data and workspace connections made run A worse. Each transaction took longer (58 ms instead of 33), because the same PostgreSQL served more concurrent work. So the database server is now the limit, not the pool. The experiment does not support A6 here; on RDS the database's capacity would decide it.

## The qualification of the cadence

`just campaign postgres` passes, with 37 checks (48.4 s). The new check:

> ADR-004 A5: inside the 30 s window a reused connection skips only the catalog comparison (catalog drift tolerated, no comparison run), while identity, session-baseline and schema-version drift are refused on reuse; a new backend and readiness always compare; after the window a reuse compares again and refuses; the four application roles hold no DDL privilege.

Two existing checks changed and now advance the injectable clock past the window for their reused connections:
- "replaced metadata view is refused on new TLS connections, and on reused ones once the 30 s catalog window has passed, with zero marker execution and restored readiness";
- the search-path shadowing check, which also refuses the poisoned worker session inside the window.

The guards are broken and restored in [break-and-restore-2026-11-14.md](break-and-restore-2026-11-14.md).

## Checks

All on this branch after merging `origin/docs/capacity-redecision` (`f9fdf92f`), which creates APP-197:
- `just check`: all 21 steps ok, including `py-promotion` and `board`. The library tests pass (415 passed, 35 ignored); admin 308/308.
- Campaigns, each run alone, all `test result: ok`: `sharing` (14.4 s), `live-sharing` (19.0 s), `record-components` (16.0 s), `query` (71.4 s), `canvas` (20.4 s), `managed` (10.7 s), `postgres` (49.2 s).
- `just campaign refresh-herd`: FAILED (run A), as above; `cargo test --release` of the same test: FAILED (run A), as above.

## Limits

- Local Docker PostgreSQL and a shared, loaded host. RDS's catalog query cost and connection behaviour are not measured.
- Inside the window, a reused connection does not detect catalog drift for up to 30 s, and a relation replaced by a view can be read during that time (ADR-004 A5). The campaign asserts the window's edges, not a production drift.
- The remaining first-try misses are in part the record snapshot limit (four snapshots in memory) answering 429 for typed views. That limit is outside APP-197.
- A6 was measured only as an uncommitted experiment, in the test profile.
