# APP-195: refresh-herd campaign (2026-11-14)

By Claude (Opus 5.5). Branch `trial/app-195-refresh-herd` at `029c13bb`, which carries both changes (30 waiters per role, and the 0–3 s client spread with one retry). Local Docker PostgreSQL on a shared macOS host (10 cores, load average 5–9 from other checkouts' builds).

```
just campaign refresh-herd
```

Result: **`test result: FAILED`** (334.6 s). Run C reproduced the finding, but run A did not meet the criteria: `run A meets the APP-195 criteria with 26 pages (largest passing: Some(5))`. The raw report is [refresh-herd-2026-11-14.json](refresh-herd-2026-11-14.json), a copy of `.cache/reports/refresh-herd.json`.

## Short answer

- **With both changes, 26 pages do not pass.** In run A, 9–15% of reads answered 200 at the first attempt. No page was current within 6 s in any herd. Most reads still failed after their retry. Herd 2's command was refused (503), so its hint went to nobody.
- **The waiter queue is not the limit; the work per refresh is.** One data-role transaction costs about 64 ms here, and 45 ms of that is the catalog comparison that every lease reuse runs. One page refresh (the document, the batch and the view) takes 12 data-role and 5 workspace-role transactions. Each role has 2 connections. A herd of 26 pages therefore needs about 312 × 64 ms / 2 ≈ 10 s of data-role time. That does not fit in a 0–3 s spread and a 6 s target, however long the queue is. With 32 admitted, refusals become waits, and the waits run into the 5 s deadlines (`Deadline` and the handler deadline, both 503).
- **What passes today:** 5 pages, with one commit every 5 s (run A's criteria, three herds at 10 s and four at 5 s). 10 pages fail, and 5 pages fail at a 3 s cadence.
- **The pre-approved fallback does not fix it.** Four data connections (10 in all) were tried as an uncommitted experiment. Data-role refusals went to 0, but the workspace role then refused 88 callers, every document still answered 503 and run A failed. The fallback was **not applied** and no ADR-004 amendment was written.
- **A release build is not fast enough either.** It brings the transaction cost to about 50 ms, of which the catalog comparison is about 37 ms. Run A still fails, and 5 pages pass only at a 10 s cadence there (see "Release profile").

## Setup

- 1 owner and 25 members of one organization, each with a session minted by the test-only constructor and an event stream (`GET /v1/events`) read as a body.
- One app (admitted and published through the shipped path) with one work: `items`, the one-row shared list the owner changes; `entries`, a second shared list with 8 entries; `current` (records_list), `total` (record_summary), `by_choice` (record_ranking), `by_item` (a typed table over `items`) and the `answer_form` form.
- **Deviations from the ticket's setup**, because the branch does not have W4: the work is shared with each member by `Workspace::share_work` (25 shares, `record_write`) instead of an organization-wide share; the second list is shared instead of `own`.
- Every page replays three reads at once through the composed router (commands, query and events routes under `http::uniform`): `GET /w/{app}/{work}` as JSON, `POST …/record-components` and `POST …/queries/by_item`. A page with the work open in a browser does either the document (work page) or the batch and the view (canvas), so this is heavier than a real page. The view keeps the canvas's existing 429 handling (wait `Retry-After`, at most three times).
- Measured alone, before the runs:

| Alone | p50 | p95 |
| --- | --- | --- |
| One page's three reads | 1155 ms | 2090 ms |
| One data-role transaction (`member_current`) | 64.1 ms | 77.1 ms |
| One catalog comparison (the SQL and comparison every lease reuse runs, on the fixture's superuser connection) | 44.6 ms | 62.4 ms |

| Read | Workspace-role transactions | Data-role transactions |
| --- | --- | --- |
| Document | 3 | 8 |
| Record components batch | 1 | 2 |
| Typed view | 1 | 2 |

## Runs (26 pages, three herds 10 s apart)

Per herd: the owner's command (status, ms), hints delivered, reads, first attempts that answered 200 and 503, reads still failed at the end, and pages current within 6 s.

| Run | Herd | Command | Delivered | Reads | First 200 | First 503 | Failed at the end | Current within 6 s |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| A: spread 0–3 s, one retry, 32 admitted | 0 | 200, 166 ms | 26/26 | 81 | 7 (8.6%) | 74 | 65 | 0/26 |
| | 1 | **503**, 1542 ms | 0/26 | 0 | — | — | — | 0/26 |
| | 2 | 200, 151 ms | 26/26 | 48 | 7 (14.6%) | 41 | 41 | 0/26 |
| B: one instant, no retry, 32 admitted | 0 | 200, 153 ms | 26/26 | 78 | 12 (15.4%) | 63 | 63 | 0/26 |
| | 1 | 200, 224 ms | 26/26 | 78 | 17 (21.8%) | 59 | 59 | 0/26 |
| | 2 | 200, 145 ms | 26/26 | 78 | 18 (23.1%) | 58 | 58 | 0/26 |
| C: run B with 8 admitted per role | 0 | 200, 154 ms | 26/26 | 78 | 8 (10.3%) | 70 | 70 | 0/26 (one page at 11.7 s) |
| | 1 | 200, 235 ms | 26/26 | 78 | 8 (10.3%) | 70 | 70 | 0/26 (one page at 2.9 s; not all three reads current) |
| | 2 | 200, 196 ms | 26/26 | 78 | 8 (10.3%) | 70 | 70 | 0/26 |

- **Run A** fails every criterion except hint delivery (where a commit happened) and command time (herds 0 and 2). Herd 0's retries and queue were still running at 10 s, and herd 1's command was refused (`503`, 1.5 s). Every document answered 503 twice. Admission refusals in run A: data 117, workspace 28. Hint delivery took 231–313 ms for all 26 (at most).
- **Run B** (measured only): at one instant, the workspace role, which every request uses first to resolve the session, refused 138 callers. Every document answered 503.
- **Run C** reproduces the finding: **210 first-attempt 503s** (≥ 10 required). Workspace-role refusals: 210.
- **Busy for the typed view:** the query worker answered `Busy` **0** times and `QueueExpired` **0** times in every run (run A: 52 typed-view runs and 72 record-component runs, all admitted). The view route answered 429 three times in run A and seven times in run B, from the record snapshot limit (at most four in memory) and not from the worker. Almost no request reached the worker: the pool refused or timed them out first.

## Fewer pages, and cadence

Run A's criteria with the first N pages (the owner's first), three herds 10 s apart:

| Pages | Result | First 200 by herd | Current within 6 s |
| --- | --- | --- | --- |
| 20 | fail | 12.7%, 6.3%, 16.7% | 0/20 in each herd; herds 1 and 2's commands took 2.3 s and 4.2 s |
| 15 | fail | 41.7%, 37.5%, 66.7% | 0/15 |
| 10 | fail | 72.7%, 63.6%, 77.8% | 1/10, 0/10, 0/10 (p50 10.5–11.6 s) |
| **5** | **pass** | 100% | 5/5 (max 5.5 s) |

For 5 pages, four herds at shorter intervals:

| Cadence | Result | Note |
| --- | --- | --- |
| 5 s | **pass** | 100% first-try 200; slowest page current at 5.4 s |
| 3 s | fail | pages current up to 14.5 s; one view answered 404 (below) |

**Sustainable today (this host, test profile): 5 pages, one commit every 5 s; in the release-profile run, 5 pages at 10 s (5 s missed by 0.3 s on a host at load 14).** For 26 pages there is no cadence at which run A passes.

## Release profile

The same campaign at `029c13bb` was built with `cargo test --release` and run with the same environment. The service image is a release build, but CI and `just campaign` use the unoptimized test profile.

```
GENUI_QUERY_PYTHON=$PWD/app/runtime/.venv/bin/python GENUI_REFRESH_HERD_REPORT=… cargo test --release --locked -p forma-app-platform --lib storage::qualification::refresh_herd::native_refresh_herd_campaign -- --ignored --exact --nocapture
```

Result: `test result: FAILED` (299.4 s), for the same reason. The host's load average rose to 14–15 during this run. The report is [refresh-herd-release-2026-11-14.json](refresh-herd-release-2026-11-14.json).

| Release profile | Value |
| --- | --- |
| One page's three reads, alone | p50 968 ms |
| One data-role transaction, alone | p50 49.5 ms |
| One catalog comparison, alone | p50 36.5 ms |
| Run A, first-try 200 by herd | 16.0%; herd 1's command refused (503, 480 ms); 16.7%. No page current within 6 s |
| Run B, first-try 200 | 21.8%, 23.1%, 21.8% (workspace refusals 138) |
| Run C, first-attempt 503s | 210 |
| Query worker `Busy` / `QueueExpired` for the typed view | 0 / 0 (60 typed-view runs in run A) |
| Most pages that pass run A | 5 (three herds 10 s apart) |
| 5 pages at a 5 s cadence | fail: two herds had a page current at 6.0 s and 6.3 s; one view answered `denied` (finding 1) |

An optimized build saves about a quarter of the transaction cost. The catalog query, which runs in PostgreSQL, stays the largest part.

## The fallback experiment (not applied)

At an earlier revision of the campaign (`24583ed1` + `9b6576a6` + the campaign before its sweeps), test profile, the data role was given 4 connections and the process 10 (`roles` and the data pool's `max_size`). The tree was then restored byte-identically.

| Herd | Command | First 200 | Documents | Admission refusals |
| --- | --- | --- | --- | --- |
| 0 | 200, 154 ms | 18.5% | 27 × `503>503` | data 0, workspace 88 (the whole run) |
| 1 | 200, 4974 ms | 14.8% | 27 × `503>503` | |
| 2 | **503**, 5001 ms | — | — | |

Doubling the data connections moved the limit to the workspace role and to the deadlines. The ticket's fallback covers the data role only, so it was not applied.

## Other findings

1. **A typed view over records answers the uniform 404 when the list changes while it runs.** `record_query::release` refuses (`Denied`) a snapshot whose collection gained a commit between capture and release. The route then answers 404, the canvas shows "You don't have access to this card." and the document shows the view as `denied`. Seen at a 3 s cadence (5 pages) and in the 15- and 20-page runs. That is a stale result, not a lost permission. It is outside APP-195; it needs its own ticket.
2. **A herd starves the next save.** In run A the owner's next command (10 s later) waited 1.5–5 s and was refused, so the hint for that change went to nobody until each page's 30 s check.
3. **Every request resolves its session on the workspace role first**, so the workspace role refuses first at one instant (run B). The document adds two more workspace reads (`can_edit_presentation`, `can_share_live_work`).

## Criteria

| Run A criterion | Result |
| --- | --- |
| No terminal refresh failure | **no**: 65 and 41 reads failed after their retry |
| ≥ 90% of refreshes 200 on the first attempt | **no**: 8.6% and 14.6% |
| Every page current within 6 s | **no**: 0/26 |
| Hint delivery 26/26 on each herd | herds 0 and 2: 26/26; herd 1: no commit, so no hint |
| Owner's command under 1.5 s | herds 0 and 2: yes; herd 1: 503 after 1.54 s |
| No `admission_saturated` for the command or the delivery read | herd 1's command answered 503; the delivery reads of herds 0 and 2 delivered 26/26 (a refused judgment delivers nothing) |

## Limits

- Local Docker PostgreSQL in a macOS VM on a host shared with other builds; RDS, Fargate and the network are not measured. The per-transaction cost, dominated by the catalog query, may differ on RDS.
- HTTP goes through the composed router in process (`oneshot`): no TLS to the client, no browser, no network. The replay follows the APP-195 client's timing and coalescing (`documentRefresher`): the committer reads at once, its echo is dropped when it arrived before the commit's answer, and one hint waits per page. The board's per-work coalescing is not replayed separately.
- Each page plays both the work page's read and the canvas board's reads at once, so it is heavier than a real page.
- The run was not repeated at `029c13bb` in the test profile; earlier revisions of the campaign gave the same picture (run A first-try 200 between 4% and 15%, run C ≥ 210 × 503).
