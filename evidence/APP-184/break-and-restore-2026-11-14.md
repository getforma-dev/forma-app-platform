# APP-184 daily model-call cap: break-and-restore runs, 2026-11-14 (W2)

Each test that proves the cap was run once with its guard deliberately broken. The run had to fail at that test's assertion. The code was then restored and the test passed again. Branch `trial/w2-admin-cap`, worktree `forma-w2`, local Docker PostgreSQL, loopback Messages stubs. No real model endpoint was used. The cap campaigns use a real `Anthropic` client with a low cap injected (`with_daily_cap`), against the stub.

## Run 1: the counter, the client and six campaign surfaces (before `1c24f21c`)

A script applied eight breaks to the uncommitted D10 work in one build:

| Guard broken | Test run | Failed at |
| --- | --- | --- |
| `DailyCap::take` never starts a new day (`if false && day > used.0`) | `cargo test -p forma-app-platform --lib -- composer::model::tests` | `the_daily_cap_counts_calls_and_starts_again_at_utc_midnight`, the first `assert!(cap.take(MIDNIGHT))` after midnight |
| `complete_with_documents` skips `admit` | `cargo test … -- composer::tests::the_client` | `the_client_sends_nothing_once_its_daily_cap_is_reached`, `composer/tests.rs:886`: the documents call did not answer `DailyCap` |
| Compose: generation's `DailyCap` falls through to the neutral 422 | `just campaign composer` | `composer_qualification.rs:669`: left `Err(CouldNotCompose("model_daily_cap"))`, right `Err(DailyLimit)` |
| Assistant: `DailyCap` → `could_not_draft` | `just campaign actions` | `actions_campaign.rs:1997` "assistant": left 422, right 429. The act and receipt surfaces before it passed. |
| Shape (widget): `Failure::DailyLimit` → `could_not_shape` | `just campaign pitch` | `pitch_qualification.rs:2678` "shape widget": left 422, right 429. Compose, Ask (records and widget) and Shape card and app before it passed. |
| MCP: `Refused::DailyLimit` → `could_not_compose` | `just campaign mcp` | `mcp_campaign.rs:1161`: left "could_not_compose", right "daily_limit" |
| Slack: the daily-limit text → "Forma's assistant isn't available right now." | `just campaign slack` | `slack_campaign.rs:1152`: the hook text differed |
| Explain: the `DailyCap` arm removed (falls to a silent `state: none`) | `just campaign box` | `box_campaign.rs:1680` "explain": left 200, right 429. Folder Ask before it passed. |

Restore: each broken file was copied back from a copy taken before the breaks. `git diff` before the breaks and after the restore were byte-identical (`cmp` of the two patches). All six campaigns had passed at that exact state just before the breaks. The unit tests passed again after the restore. On the review head `62f4e082`, which includes the 1,000 default and the final message text, `just campaign actions|mcp|slack|box|composer|pitch` each passed again (exit 0).

## Run 2: the canvas copy (TypeScript)

- **Broken:** the `daily_limit` branch in `canvas/compose.ts`'s 429 handling, and the `daily_limit` entry in `ask/client.ts`'s refusal map.
- **Command:** `npx tsx --test test/daily-limit.test.ts`.
- **Failed:** test 2 ("compose tells the daily cap apart from the member window") and test 3 (ask, shape and folders). Test 1 passed.
- **Restore:** both files were copied back. The run then gave 3 pass, 0 fail, and `git diff --stat` showed only the intended changes.

## Run 3: review M3 and the cap peek (`62f4e082`)

Two breaks were applied in one build, along with the APP-183 review breaks recorded in that ticket's file:

| Guard broken | Test run | Failed at |
| --- | --- | --- |
| `DailyCap::reached` always `false` | `cargo test -p forma-app-platform --lib` | `the_daily_cap_counts_calls_and_starts_again_at_utc_midnight`, the `closed.reached(…)` assertion |
| No cap peek before placing a pinned app that needs the transform (`if false && tailors …`) | `just campaign composer` | `composer_qualification.rs:716` "the daily limit": the composition answered `Ok` (the standard work placed) instead of `Err(DailyLimit)` |

Restore: `git checkout --` of the broken files back to the committed `62f4e082`; `git status` was clean except `evidence/`. Then `cargo test -p forma-app-platform --lib` gave 429 passed, and `just campaign composer` and `just campaign org-admin-routes` gave exit 0.

## Related: the per-address limits pin (APP-189 classroom network, ADR-006 amendment of 2026-11-14)

| Guard broken | Test run | Failed at |
| --- | --- | --- |
| `INVITE_ADDRESS` day window back to 20 | `cargo test … -- http::limits` | `a_refusal_names_the_window_that_blocked` |
| `SIGNIN_ADDRESS` minute window back to 10 | `cargo test … -- http::limits` | `a_refusal_names_the_window_that_blocked`, `limits.rs:312`: the pin `(40, 150, 5, 100)` |

Restore:
- First break: `git checkout -- app/src/http/limits.rs` back to `fb9b002c`.
- Second break: the same checkout also dropped the uncommitted 40-a-minute change, so it was applied again and committed as `b790d11b`. The limits tests passed, then the auth unit tests and the invite and session campaigns passed.

## Recheck fix round (`f8864d52`)

No cap guard changed in this round. The one new limit is N1: `POST /me/name` now shares the member's `MEMBER_WRITES` window, APP-113's 120 an hour. Its break and restore is round 1 of the [APP-183 record](../APP-183/break-and-restore-2026-11-14.md): with the window check disabled, write 121 was admitted. After the restore, on `f8864d52`:

- `just campaign composer`: exit 0, 20 checks, including the D10 (review M3) phase.
- `cargo test -p forma-app-platform --lib`: 432 passed, including the cap's unit tests.

Recheck N3 (the last-slot race) is recorded as accepted in [the recheck file](../APP-183/recheck-w2-2026-11-14.md).

