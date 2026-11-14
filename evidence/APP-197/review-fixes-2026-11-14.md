# APP-195 and APP-197: the Tier A review's fixes, the gate and the 26-page measurement (2026-11-14)

By Claude (Opus 5.5), on `trial/app-197-catalog-cadence`. This answers [review-tier-a-2026-11-14.md](review-tier-a-2026-11-14.md), which found no BLOCKER and one MAJOR. It also implements the plan the Fable advisor accepted in lieu of Victor (PR #120): the class is accepted on A5 as measured; the CI gate is 15 pages; the 26-page run is a recorded measurement; A6 is not applied. `origin/docs/capacity-redecision` at `56b7582c` was merged first. It has not merged to main yet.

## Fixes

| Finding | Fix | Commit |
| --- | --- | --- |
| **M1:** the 30 s bound was untested; a window that restarts on every reuse passed | The reviewer's probe, measured from the connection's last comparison: drift injected at 10 s; reuses at 20 s and 29 s tolerated with no comparison; the reuse at 31 s compares and refuses. Each step subtracts the real time already spent, so the ages hold under load (`d6c69256`); a first version failed once when the DDL took over a second. Plus `const _: () = assert!(CATALOG_RECHECK.as_secs() == 30);` | `d6e628e0`, `d6c69256` |
| **m1:** inside the window the version relation was read with no structural check, after the baseline | Inside the window a reuse checks the version relation from `pg_catalog` before reading it: same OID as at its last comparison, `relkind = 'r'`, no row security, column types 16 and 23 (`boolean`, `integer`). It then reads `FROM ONLY`, and `verify_baseline` runs last. Since `12a68ba0` this check is part of `verify_identity`'s one statement, with no extra round trip. The metadata-view check no longer advances the clock. A new case adds a row-security policy that runs code. Both refuse reused connections inside the window with zero marker executions | `d6e628e0`, `12a68ba0`; A5 text `71d5ca75` |
| **m2:** the A5 privilege wording; function and type ownership | A5 now says the roles own nothing and hold no CREATE or TEMP, and that PostgreSQL still lets each role set its own defaults, which only new connections see, and those are always compared. The campaign also asserts that no function, type or large object is owned by the four roles | `d6e628e0`, `71d5ca75` |
| **m3:** a stale quarantine row | Removed with the quarantine (plan item 3) | `d778b729` |
| **m4:** the browser wiring was untested | `test/live-wiring.test.ts` covers three things. `browserTiming` is checked against a page `EventTarget`: the real `visibilitychange` name, `hidden`, and unsubscribe. The board's `onLive` is driven with mock timers: a hint waits its drawn delay, a second hint joins it, `stream.lagged` waits too, and a hidden page reads once when shown. The document's triggers are covered as well | `2200b6d3` |
| **m5:** "a single retry" composes with the view's 429 loop | No code change (bounded). A hinted typed-view refresh makes up to four attempts in `runView` (`MAX_BUSY_RETRIES = 3`, each after `Retry-After`, clamped to 1–10 s). Each attempt can retry one 503 after 1–3 s, so at most 8 requests, at least 1 s apart, and never synchronised: `Retry-After` is relative, and each client keeps its own 0–3 s offset. Every other hinted read makes at most 2 | — |
| **m6:** `online` refreshed at once | Coming back `online` reads the work document like a hint: 0–3 s and one retry. The page's triggers are now `listenForRefreshes(page, network, refresher, signal)`; Refresh and becoming visible are unchanged | `2200b6d3` |
| n1, n2, n3 | `connect` measures the window from before its checks. A5 says "at least 30 s", matching `>=`. A refused 503 answer's body is cancelled before the retry | `d6e628e0`, `71d5ca75`, `2200b6d3` |
| n4 | Runbook text, for APP-188. Under a herd, commands queue FIFO behind up to 30 reads, for up to 5 s, instead of being refused at once | — |

## Tests first

- **m1.** With the metadata-view check no longer advancing the clock, the campaign first ran against the pool without the relation check. It failed at `qualification.rs:966`: inside the window the reused connections executed the trap view, marker `"4"` against `"0"`. With the fix it passes (37 checks).
- **M1.** The probe passes on correct code. Its red is shown by breaks R1 and R3 below.
- **m4 and m6.** `live-wiring.test.ts` was written first. Tests 2 and 3 failed: `board.onLive is not a function` and `listenForRefreshes is not a function`. Test 1 passed, because the wiring it tests was already correct; its red is break G1.

## Break proofs

**Server** (`just campaign postgres`). R1 to R5 ran on `d6e628e0`'s `pool.rs` (`991e1547…cc54`). R1b, R4b and R5b re-ran them on `12a68ba0`'s, where the relation check is folded into `verify_identity`. After each break the file was restored byte-identically and checked against its committed digest.

Final run, against the final tests (`d6c69256`) and `pool.rs` `e116bbbab9bcfb07df9387f66359173f9484b69968e5c3ae4786172993ceb6bf` (`12a68ba0`). The earlier runs against `d6e628e0` gave the same R1–R5 results.

| Id | Break | Result |
| --- | --- | --- |
| R1 | Sliding window: `catalog_verified_at = now` on every reuse (the restarting timer) | FAILED at the probe: "the reuse at 31 s compares and refuses" |
| R2 | `CATALOG_RECHECK` set to 60 s, the assertion kept | Does not compile: `error[E0080]: evaluation panicked: assertion failed: CATALOG_RECHECK.as_secs() == 30` |
| R3 | `CATALOG_RECHECK` set to 60 s, the assertion removed | FAILED at the probe: "the reuse at 31 s compares and refuses" |
| R4 | Inside the window, the OID comparison disabled | FAILED: "a replaced version relation is never read on reuse" (the same-shape replacement was reused) |
| R5 | The row-security part of the shape check disabled | FAILED: `assertion failed: Lease::acquire(pool).await.is_err()` (the row-security case; reused connections were admitted) |
| R6 | The `relkind` part of the shape check disabled | **Not caught on its own.** The campaign passed: a view replacement always has a new OID, which R4's comparison refuses first, and PostgreSQL 16 cannot turn a table into a view in place. Kept as defence in depth |

On `d6e628e0`, where the relation check was still its own query, a first R4 (the recorded OID used instead of re-reading the relation) failed at the metadata-view check. A first re-run of R4 against `12a68ba0` passed, because no test then replaced the relation with one of the same shape. The same-shape case in `d6c69256` closes that gap.

The earlier seven A5 breaks are in [break-and-restore-2026-11-14.md](break-and-restore-2026-11-14.md).

**Client** (`npx tsx --test test/live-refresh.test.ts test/live-wiring.test.ts`). Restored each time, and 10/10 passed afterwards: `live.ts` `10b48fbc…`, `board.ts` `610913c7…`, `document.ts` `c99325ac…`.

| Id | Break | Failing tests |
| --- | --- | --- |
| G1 | `browserTiming.onVisible` listens for `visibilitychanged` | 8 (`browserTiming`) and 9 (the board's hidden page) |
| G2 | The board's `record.committed` hint runs at once | 9 |
| G3 | The board's `stream.lagged` hint runs at once | 9 |
| m6-a | `online` routed to `member()` | 10 |
| m6-b | The document listens for `offline` instead of `online` | 10 |

## The CI gate, test profile (`just campaign refresh-herd`)

**The gate does not yet pass reliably.** It passed once locally, failed twice locally (host load 10–11), and **failed on CI** at `b4d51cea`. Every miss is one of two things: the first-try share, which view 429s from the record snapshot cap of 4 pull down, or a p95 just over 6 s, because a 429 waits its 2 s `Retry-After`. With the cap raised to 24 (APP-200's decided constant, as an uncommitted experiment) the gate passed with margin.

| Run | Code | Result | First-try 200 by herd | p95 by herd | View 429s | Gate C 503s |
| --- | --- | --- | --- | --- | --- | --- |
| Local 1 (load 4–8) | `d6e628e0` | **pass** (94.7 s) | 97.9%, 100%, 91.7% | 4.1, 3.7, 4.3 s | 1, 0, 5 | 111 |
| Local 2 (load 10) | `d6e628e0` | fail | 95.8%, 91.7%, **89.6%** | 5.4, **6.7**, 4.7 s | 3, 5, 5 | 111 |
| **CI**, `ubuntu-24.04` | `b4d51cea` | **fail** | 91.7%, 91.7%, **85.4%** | 5.6, **6.4**, **6.5** s | 4, 6, 12 | 111 |
| Local 3 (load 11) | `12a68ba0` | fail | 93.8%, 93.8%, 91.7% | **6.9**, 5.9, 4.8 s | 4, 3, 5 | 111 |
| Local, snapshot cap 24 (experiment) | `12a68ba0` + cap 24 | **pass** | 100%, 100%, 97.9% | 5.0, 4.2, 4.4 s | 0, 0, 0 | 111 |

In every run: 0 reads failed after the retry, 15/15 hints delivered, every command under 300 ms, and 4 catalog checks. At cap 4, gate A had no data or workspace admission refusals in the runs with a saved report (local 1, local 2 and CI).

*Corrected after the recheck (R-m1).* This paragraph first said "no data or workspace admission refusals in gate A" for every run. The cap-24 run, which is the configuration that will ship, had one:

- gate A's `admission_saturated` is `{"data": 1}`;
- herd 2 had one first-attempt 503 (`reads_with_a_503` 0, 0, 1), which its retry recovered (`terminal_failures` 0);
- with the cap raised, view reads no longer wait on 429s, so their load moves onto the data role's 32-caller queue, which overflowed once at 15 pages;
- the gate asserts no refusal only for the command and the delivery read, so a read refusal does not fail it.

Reports: [local 1](refresh-herd-gate-2026-11-14.json), [local 2](refresh-herd-gate-rerun-2026-11-14.json), [CI](refresh-herd-gate-ci-b4d51cea-2026-11-14.json), [cap 24](refresh-herd-gate-snapshot24-experiment-2026-11-14.json).

Local 1 in detail ([report](refresh-herd-gate-2026-11-14.json)):

| Gate A, 15 pages | Herd 0 | Herd 1 | Herd 2 |
| --- | --- | --- | --- |
| Command | 200, 49 ms | 200, 56 ms | 200, 123 ms |
| Hints delivered | 15/15 | 15/15 | 15/15 |
| First-try 200 | 97.9% | 100% | 91.7% |
| Reads failed after the retry | 0 | 0 | 0 |
| Current: p95 / slowest page | 4.1 / 4.4 s | 3.7 / 3.8 s | 4.3 / 5.3 s |
| View 429s | 1 | 0 | 5 |

- Gate A also had: 4 catalog checks; a data-role service p50 of 30.9 ms; no admission refusals.
- **Gate C** (15 pages at one instant, 8 admitted): **111 first-attempt 503s**, with 111 workspace refusals.
- One data-role transaction alone: 10.0 ms; 0 catalog checks in 20 transactions.

## The 26-page measurement, release profile

**Not met in today's runs.** The earlier A5 release run (`06e8eeb8`, host load about 10; [report](refresh-herd-release-2026-11-14.json)) had 0 failed reads, every page current within 6.1 s and a p95 of 5.2–5.8 s; it missed only first-try 200. Today's runs, at `d6e628e0` and later, on a host at load 10–22 with other worktrees' campaigns in the same Docker VM, did not reproduce it. The data role refused 57–129 callers, and some reads failed after their retry.

| Run (`GENUI_REFRESH_HERD_FULL=1`, `cargo test --release`) | Load | Failed reads by herd | p95 by herd | Slowest page | Command | First-try 200 | View 429s |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `d6e628e0`, [1](refresh-herd-full-release-loaded-2026-11-14.json) | 11–22 | 12, 0, 0 | 18.3, 9.5, 8.2 s | 18.4 s | 54–69 ms | 49%, 68%, 72% | 10, 17, 14 |
| `d6e628e0`, [2](refresh-herd-full-release-2-2026-11-14.json) | 10–14 | 38, 5, 0 | 26.4, 16.4, 7.7 s | 27.5 s | 62–627 ms | 30%, 46%, 72% | 9, 23, 14 |
| `12a68ba0` + snapshot cap 24, [experiment](refresh-herd-full-release-snapshot24-experiment-2026-11-14.json) | 13–20 | 2, 0, 0 | 15.4, 6.3, 10.9 s | 18.1 s | 57–82 ms | 70%, 89%, 70% | 0, 0, 0 |

- In every run the owner's command stayed under 1.5 s, and delivery was 26/26.
- The release-profile gate inside those runs (15 pages) passed only with the cap at 24: first-try 100%, p95 at most 4.1 s.
- I cannot separate code effects from host load here. One data-role transaction alone measured 10.1–11.2 ms in today's release runs, against 7.6 ms in the earlier one; that is host load, the m1 check, or both.

## Commands and results

- `just check` at `b4d51cea` (before the folded identity check): all 21 steps ok; library 415 passed, 35 ignored; admin 311/311. `just check admin`: ok.
- At the same head: `postgres` ok (52.2 s), `managed` ok (10.5 s), `sharing` ok (13.4 s), `live-sharing` ok (23.9 s); `refresh-herd` failed (gate, local 2 above).
- At `d6c69256`: `postgres` ok (49.9 s), with the exact probe and the same-shape case. The final rerun of `just check` and the four campaigns is in the PR.
- CI at `b4d51cea`: every workflow green except Tenant foundation, which failed only at the new gate step (all 16 earlier steps green).

## Limits

- Local Docker PostgreSQL in a macOS VM on a shared host. CI's runner is a different machine; the gate's herd 2 first-try share was 91.7% against the 90% line.
- The relation check covers the version relation, which is the only application-owned relation the reuse checks read. Inside the window, other drifted objects are still used by transactions for up to 30 s (A5, "Consequence").
- "Run the baseline last" is defence in depth; no test isolates its order, since nothing runs before it.
- The board test drives the exported `onLive` with mock timers and a page `EventTarget`; no browser ran.
