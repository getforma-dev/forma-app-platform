# APP-195: revised decision, visibility deferral and quarantine (2026-11-14)

By Claude (Opus 5.5). This records the Fable advisor's revised decision (in lieu of Victor), made after [the first campaign](refresh-herd-2026-11-14.md). Branch `trial/app-195-refresh-herd`, after merging `origin/docs/charter-retention` at `b343aba9`; it applied cleanly.

## What changed

- **Visibility deferral** (`app/admin/src/canvas/live.ts`, `hintSchedule`):
  - A hint that arrives while `document.visibilityState === 'hidden'` schedules nothing; the key is marked stale.
  - The work document's existing `visibilitychange` refresh covers it: `documentRefresher.shown()` clears the mark and reads once.
  - The canvas has no visibility refresh of its own, so its schedule (`resumeWhenShown`) runs each stale key's latest refresh once, at once, when the page is shown again.
  - The 0–3 s spread and the single retry are unchanged.
- **Revised run A criterion** (`storage/qualification.rs`, `refresh_herd`): pages are current within 6 s at the 95th percentile (`CURRENT_WITHIN`), and every page within 10 s (`CURRENT_MAX`, new). The other criteria are unchanged.
- **New reports per run:**
  - `catalog_checks`: the catalog comparisons run during the run, from a test-only counter in `catalog.rs`.
  - `transaction_service`: per role, each lease's preparation (creation or recycle checks) plus the time it held the connection, from test-only samples in `pool.rs`.
  - Measured alone: `catalog_checks_in_20_transactions`.
- **Quarantine:**
  - The tenant-foundation step is removed, and the catalog `ci` column is `-`.
  - `docs/DEVELOPMENT.md` has a quarantine row naming APP-197.
  - `just campaign refresh-herd` still runs it.

## Unit tests and break-and-restore

`app/admin/test/live-refresh.test.ts` now has 7 tests:
- 6, "a hidden page schedules nothing for a hint and reads once when it is shown", covers both the canvas and the document;
- 7, "the committer on a visible page is unchanged by the visibility rule".

Each guard below was broken, the test was run and failed, and the file was restored with `git checkout --`. The SHA-256 then equalled the staged file's (`live.ts` `dddfbc87eb16eec39e7a8f2f3744f640faff106885933f5b1ace24073351b656`, `document.ts` `377e5eb19df66b58e43bf8cf1f9a6a3a5a22306547020cf6dac94ca3faffc5e8`), and 7 pass, 0 fail.

| Guard | Break | Failing test |
| --- | --- | --- |
| A hidden page schedules nothing | the `hidden()` branch disabled | 6 |
| The canvas reads each stale key when shown | the stale runs not called | 6 |
| Each stale key reads once only | the stale set not cleared | 6 |
| The document reads once when shown | `shown()` without its refresh | 6 |
| The document does not also resume (no second read) | the document's schedule given `resumeWhenShown` | 6 |
| The visible committer reads at once | `committed` routed through `hint` | 2 and 7 |

The implementation was written before these two tests. The breaks above show that the tests catch each rule.

## Campaign with the revised criteria (test profile)

`just campaign refresh-herd`: `test result: FAILED` (run A), with both earlier changes and the visibility deferral. Host load average 7–10. The report is [refresh-herd-revised-2026-11-14.json](refresh-herd-revised-2026-11-14.json).

| Run | Catalog checks | Data-role transaction service p50 | First-try 200 by herd | Current (p95, every page) |
| --- | --- | --- | --- | --- |
| A, 26 pages | 918 | 172.7 ms (436 leases) | 16.0%, 8.6%, 10.3% | no page current in any herd; herds 1 and 2's commands took 2.2 s and 4.1 s |
| B, 26 pages | 434 | 95.8 ms | 21.8%, 23.1%, 20.5% | none |
| C, 26 pages, 8 admitted | 229 | 84.4 ms | 10.3% each; **210 × 503** | none |

- Alone: one data-role transaction took p50 54.4 ms and ran **20 catalog checks in 20 transactions**. One catalog comparison took p50 47.1 ms.
- Under the herd, a data-role transaction's service time rises to about 173 ms: the catalog queries compete for the same PostgreSQL.
- 5 pages passed two herds out of three. Herd 2 had 4 first-attempt 503s and a p95 of 9.3 s. This run, at a higher host load, found no passing page count. The earlier run passed 5 pages.
- The query worker answered `Busy` 0 times and `QueueExpired` 0 times for the typed view (89 runs in run A).

## Checks

- `just check`: 20 steps ok on the first run. `board` failed only because `Tickets/index.json` was stale after the ticket edit. After `python3 Tickets/_tools/board.py render`, `just check board` passed.
- `just check admin` is part of `just check`. Its tests: 308 pass, 0 fail.
- Regression campaigns, each run alone, all `test result: ok`: `sharing` (19.5 s), `live-sharing` (43.2 s), `record-components` (25.6 s), `query` (94.9 s), `canvas` (30.6 s).
