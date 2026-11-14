# APP-195: break-and-restore proofs (2026-11-14)

Branch `trial/app-195-refresh-herd`. Each guard below was broken in the working tree, the test that owns it was run and failed, and the file was restored from Git. After every restore the file's SHA-256 equalled the committed file's, and the owning test passed again. By Claude (Opus 5.5).

## Client (`app/admin`, `npx tsx --test test/live-refresh.test.ts`)

Committed digests: `app/admin/src/canvas/live.ts` `7a55972949e5537a837e565f91fa4940521cd2c3c3f1d9b3efa45333ef42daec` and `app/admin/src/work/document.ts` `abbb7a659fae895cd09ffa317a906ef52041fa48b26a8c309935c693cea94e69` (commit `9b6576a6`). After each restore: 5 pass, 0 fail.

| Guard | File | Break | Failing test(s) with the break |
| --- | --- | --- | --- |
| A hinted refresh starts inside 0–3 s | `canvas/live.ts` `hintDelay` | returns `0 * uniform(...)` (no spread) | 1 "a hinted refresh starts after a uniform random delay of 0–3 s"; 4 "refreshes the member starts are unchanged" (its waiting hint ran at 0, not 2700 ms) |
| Hints for a key join the one waiting | `canvas/live.ts` `hintSchedule.hint` | the `if (waiting.has(key)) return;` line removed | 1 (two reads and two draws instead of one) |
| The page that committed reads at once | `work/document.ts` `documentRefresher.committed` | `hints.now(...)` replaced by `hints.hint(...)` | 2 "the page that committed reads at once, and its read replaces a waiting hint" |
| The committed read replaces a waiting hint | `canvas/live.ts` `hintSchedule.now` | `cancel(key)` removed | 2 (the waiting hint also ran) |
| A 503 is read once more | `canvas/live.ts` `retryOnce` | `if (response.status !== 503) return response;` replaced by `return response;` | 3 "a failed read is tried once more after a random 1–3 s, and the second answer is final"; 4 (the hinted read returned the 503) |
| Only once more | `canvas/live.ts` `retryOnce` | the second read replaced by a recursive `retryOnce(...)` | 3 (`an unexpected random draw`: a third read was attempted) |
| Only a 503 or a network error is retried | `canvas/live.ts` `retryOnce` | `!== 503` replaced by `< 500` | 3 (`an unexpected random draw` on the 500 case) |
| An aborted read is never retried | `canvas/live.ts` `pause` | the `abort` listener removed | 3 (a read after the page stopped; the wait was not cancelled) |
| The retry waits 1–3 s | `canvas/live.ts` `retryDelay` | `uniform(timing, RETRY_MIN_MS, ...)` replaced by `uniform(timing, 0, ...)` | 3 (`retryDelay` 0 instead of 1000) |
| The member reads with the page's plain fetch | `work/document.ts` `documentRead` | always `retryOnce(...)` | 4 (the member's read was not the page fetch) |
| The member reads at once | `work/document.ts` `documentRefresher.member` | routed through `hints.hint(...)` | 4 (no read at 0 ms) |

The harness that applied each break, ran the test, restored with `git checkout --` and compared digests was a scratch script outside the repository.

## Server (`app/src/storage/pool.rs`, `just campaign postgres`)

Committed digest: `app/src/storage/pool.rs` `044c24d26c0857a98e93ea370d75bd426e99d33b234aed686015b73c2cef34f4` (commit `029c13bb`). After both restores the campaign passed: `test result: ok. 1 passed` (48.3 s), with the check "eight physical connections, two per role, thirty bounded waiters (the 33rd caller refused at once) and absolute acquisition deadline".

| Guard | Break | Result with the break |
| --- | --- | --- |
| Thirty waiters per role | `ADMITTED: usize = 32` set to `8` | failed after the check "operation-first authority fence holds until commit…": `qualification.rs:803` `Elapsed(())`. The pool never had 30 waiters; six were admitted and 24 refused. `test result: FAILED` (19.0 s) |
| The 33rd caller is refused at once | `try_acquire_owned()` replaced by `acquire_owned().await` (the 33rd waits) | failed at `qualification.rs:805`: `assertion failed: matches!(Lease::acquire(&stores.data.inner).await, Err(Error::Unavailable))`. `test result: FAILED` (19.7 s) |

Test first: before the pool change, the new expectation was run against the old constant and failed the same way as the first break (`qualification.rs:803`, `Elapsed(())`).

## The campaign's own proof by breaking

Run C of `refresh-herd` runs run B with every role admitting 8 callers again. The limit is the test-only `pool::limit_admitted`, which holds the other admission permits, so the production acquisition path is the one measured. It must reproduce the APP-193 finding (at least ten 503s). The numbers are in [refresh-herd-2026-11-14.md](refresh-herd-2026-11-14.md).

## Limits

- The client guards are proved at the unit level, on the shared timing in `canvas/live.ts` and on `documentRefresher` and `documentRead` in `work/document.ts`. How `canvas/board.ts` and `record-components/controller.ts` are wired to that timing is covered by the typecheck and the existing suite (306 tests), not by a board-level test of its own.
- The server breaks were run while no other campaign of this checkout was running. Other checkouts on the same host were building.
