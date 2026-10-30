# APP-014 — command retries, conflicts and both authorization race orders (plan Task 11)

Date 2026-10-29. Base `ea8ba23` (APP-013 merged). Status: implemented and qualified **locally** on disposable PostgreSQL 16.13 in Docker with verified TLS. No hosted run, no managed (RDS) run, no CI run, no independent review yet (ADR-010 §4.1 Tier B); ticket not moved (no `Tickets/` or `docs/` edits). The RDS lock-observation repeat of these race orders is APP-080's (ADR-008, ADR-010 §5).

Controlling text: [APP-014](../../Tickets/0-backlog/APP-014.md) acceptance; [plan Task 11](../../docs/planning/PHASE-1-EXECUTION-PLAN.md) (the five campaign groups, typed `CommandResult` plus the D7.9 HTTP row under a held writer lock, `lock_timeout` → 409, never 503); [ADR-007](../../docs/architecture/ADR-007-PHASE-1-WRITE-AUTHORITY.md) D7.1 (operator revocation order), D7.4 (test-only adoption writer through the owner channel), D7.9 (results, claim, lock order, HTTP rows); [ADR-004](../../docs/architecture/ADR-004-TENANT-TRANSACTIONS.md) (writers participate in the same fences); [ADR-002](../../docs/architecture/ADR-002-CONTRACT-EDITION.md) §Retry (outcome before expected version; current authorization before disclosure; visibility before delivery); APP-013's checkpoints and campaign ([evidence/APP-013](../APP-013/README.md)).

## What changed

| File | Change |
| --- | --- |
| `app/src/storage/race_qualification.rs` (new, `cfg(test)`) | `storage::race_qualification::native_race_campaign` (ignored test, Docker): the five plan groups, 23 group checks + 2 closing checks (25). |
| `app/src/storage/mod.rs` | `#[cfg(test)] mod race_qualification;` |
| `app/src/storage/command_qualification.rs` | Test helpers made `pub(super)` for reuse (`values`, `scalar`, `footprint`, `paused`, `committed`, `conflict`, `TestPages`, `Reply` and its fields, `send`, `json_headers`, `vault_token`); no behavior change. |
| `app/src/storage/definition_qualification.rs` | Test helper `revoke` (the operator's `revoke-permission`) made `pub(super)`. |
| `app/src/storage/managed_qualification.rs` | `SOURCES` 76 → 77 (the new file, lexicographic); the completeness unit test passes. |
| `app/qualification/Dockerfile.dockerignore` | Admits `app/src/storage/race_qualification.rs`. |
| `.dockerignore` | Comment only: the new file is `cfg(test)` and stays out of the service context, like `query_qualification.rs`. |

**No production code changed** (`commands.rs`, `pool.rs`, `authorization.rs`, `sessions.rs`, routes untouched). APP-013 already provides every checkpoint the plan's "checkpoint plumbing" step named (`precheck`, `outcome`, `fence`, `record_lock`, `commit`), so `pool.rs` needed nothing. Plan deviations, for review: the cases live in one `race_qualification.rs` (the repository's `*_qualification.rs` campaign convention, as APP-013's `command_qualification.rs`) instead of `race_cases.rs` + groups in `qualification.rs`; the APP-008 native campaign is unchanged.

## How the races are driven

Every race is serialized by a single armed checkpoint in the command transaction and by lock waits observed in `pg_stat_activity` (`wait_event_type='Lock'`); there is no sleep used as a barrier. Writers:

- **Revocation, unheld:** the real operator command `revoke-membership` (genui_migration over verify-full TLS, D7.1 order).
- **Revocation, held across a checkpoint:** fixture-only SQL in the operator channel (`SET LOCAL ROLE genui_owner`, explicit organization scope; `organizations FOR SHARE` → member `FOR UPDATE` → `status='revoked'`, `membership_epoch+1`), committed or rolled back on cue.
- **Adoption (D7.4, test-only):** the same owner channel; `organizations FOR SHARE` → `saved_work FOR UPDATE` → `version+1`.
- **"FOR NO KEY UPDATE" holders:** a no-op `UPDATE` of the member or saved-work row, to reach the authorization-fence `busy` row.
- Between revocation orders the revoked reviewer is re-activated by fixture-only SQL in the D7.1 order (as the APP-008 campaign); every assertion is on the database (per-record revisions, outcomes and data-chain events; total claims), never on a response alone.

## Acceptance tests (ticket) and evidence

[race-campaign.json](race-campaign.json) (run 1) and [race-campaign-run2.json](race-campaign-run2.json) (run 2): `passed: true`, 23 group checks + 2 closing (25), 17 commits in all, identical results.

| Acceptance | Group and what it proves |
| --- | --- |
| 1. Lose the response after commit and retry the same scoped key/payload with now-stale expected version: after current authorization, return the original outcome with one record revision and mutation audit. | `lost_response_retry_returns_original_outcome_with_stale_expected_version` (4 checks): the route's 200 body is discarded; a second key moves the record to version 2; the retry with expected version 0 answers 200 **byte-identical** to the lost body and `Replayed(o1)` in storage; the lookup by key is identical; per-command one revision, one outcome, one `command.committed` event, no new claim. The same key with a changed payload is 409 `conflict` (`payload_mismatch`) whose recovery view carries the original outcome and the current version/values. **Authorization before disclosure:** after `revoke-permission run` (epoch advanced) the retry and the lookup are the uniform 404 / `Denied` at the old and the new epoch; after `grant-permission run` the retry again returns the original outcome. After every pool and store is replaced, a fresh session's retry is still byte-identical (the old session is ended with its vault). |
| 2. Race same-key and different-key edits; one intended write commits, changed-payload reuse and stale distinct edits conflict with authorized recovery, and other principals cannot retrieve the key. | `same_key_and_different_key_races_commit_exactly_one_intended_write` (4): same key + payload behind an in-flight claim waits (observed) and **replays** the winner when it commits; same key + changed payload waits and conflicts `payload_mismatch` naming the winner's outcome; a different key on the same expected version waits on the organization sequence (observed), conflicts `expected_version` (2 vs 3, the winner's values, no stored outcome) and its recovery on version 3 under a new key commits; on the route, both waits held past `lock_timeout` answer **409 `in_flight`, `Retry-After: 2`**, then 200 (replay) and 409 `conflict` until the recovery commits. Seven intended writes → exactly seven revisions, outcomes and events. `other_principal_cannot_retrieve_key` (3): while the author's identical retry holds its fences, the reviewer (with `record_write`) submitting the author's key and payload gets **its own fresh claim** judged on its own premise (409 `expected_version`, `stored_outcome: null`, no lock wait), its lookup is `None` / 404; a member without `record_write` is 404 for lookup and submission; the canary organization is `Denied`; at the row level the data role under the reviewer's scope sees 0 claims/outcomes under the key (1 under the author's); the reviewer's own command under the same key string commits its own outcome and each lookup returns only its owner's; an author claim in flight neither blocks nor answers the reviewer's same-key command. |
| 3. Pause a new command and race it with membership revocation or work-version adoption in both orders; authoritative change first prevents stale commit, earlier authorized commit remains recorded and current visibility governs response delivery. | `adoption_vs_commit_both_orders` (5): **adoption first** (paused at `precheck`, adoption committed) → `Stale` naming work version 2, nothing written, claim released; **adoption first, lock held** → the route command's claim waits on the writer (observed), the writer commits during the wait → **412 `stale`** naming version 3; **held past `lock_timeout`** → `FOR UPDATE` blocks the claim's reference → **409 `in_flight`**, a `FOR NO KEY UPDATE` holder blocks the work fence → **409 `busy`**, both `Retry-After: 2`, never 503; **command first** (paused at `record_lock`, holding the work `FOR SHARE`) → the adoption waits (observed), the command commits on work version 3, the adoption then commits 4; the outcome stays recorded and its identical retry replays it (200, `work_version` 3); a new edit on version 3 is 412 naming 4; its recovery on 4 commits; **claim-first interleaving** (paused at `outcome`) → the adoption already waits on the claim's key share, the command's work fence takes `FOR SHARE` ahead of the queued writer **without deadlock** and commits. `revocation_vs_commit_both_orders` (7): **revocation first** (paused at `precheck`, `revoke-membership` committed) → `Denied`, nothing written, claim released, next request `Denied`/404; **revocation first, lock held** → the claim waits on the member row (observed), the revocation commits during the wait → **404**; **held past `lock_timeout`** → revocation in progress `FOR UPDATE` → **409 `in_flight`**, `FOR NO KEY UPDATE` → **409 `busy`**, both `Retry-After: 2`, never 503; **request arriving while the writer already holds the row** → see finding F1; **command first** (paused at `record_lock`) → `revoke-membership` waits (observed), the command commits, then the revocation; the revision, outcome and event **remain recorded** (another member reads version 1) while the revoked principal's identical retry and lookup are `Denied` at both epochs and 404 on the route; **claim-first interleaving** → the revocation already waits on the claim's member key share, the fence upgrades ahead of it without deadlock, commits, then the revocation commits; **delivery after the fence** → a replay of a committed key holds no member lock before its fence, so `revoke-membership` commits without waiting, and the resumed replay is refused (`Denied`): the stored outcome is delivered only under current visibility. |
| SR10 | Closing: the operator recomputes the data chain to its head; 17 `command.committed` events for 17 outcomes and 17 claims, each outcome naming its own event; no refused or raced command left a claim, outcome or event. |

Timings (ms, run 1 / run 2): lost response 3,294 / 3,195; same/different key 4,092 / 4,030; other principal 2,754 / 2,710; adoption 4,504 / 4,456; revocation 8,184 / 8,165; whole campaign 29,155 / 28,457 (wall 40 s / 29 s including the incremental build).

**Sensitivity (mutation) check.** With a temporary mutant in `commands.rs` that delivers `Replayed` before the authorization fence, the campaign fails (`other_principal_cannot_retrieve_key`'s `fence` barrier is never reached; log kept in scratch only). The mutant was reverted byte-for-byte (no diff on `commands.rs`) before the evidence runs.

## Findings

- **No APP-013 defect found.** Every typed result, HTTP row, lock-order interleaving and write count matched D7.9/ADR-002; both claim-first interleavings (a writer queued `FOR UPDATE` behind the claim's foreign-key key share while the command upgrades to `FOR SHARE`) complete without deadlock.
- **F1 (D7.9 conformance, not fixed — owner decision needed).** A route request that arrives while an operator member writer *already* holds the member row waits in **session resolution** (`Workspace::resolve_session` → its own `lock_member_authority` fence) and, past `lock_timeout`, answers the **uniform 404** (the route's `session()` maps any lookup error to "no session"), not D7.9's "a 55P03 on an authorization fence in any request → 409 `busy` with `Retry-After: 2`". It is never 503 and fails closed, and requests that resolved their session before the writer arrived get the D7.9 rows (409 `in_flight`/`busy`, proven above). The same `Err(_) → 404` mapping is APP-009's pattern on every protected route (`auth/routes.rs:434`, `:520`), so the fix is not APP-013-local: either a typed `Lookup::Busy` from `resolve_session` (55P03 noted by `Transaction::lock_timed_out`) mapped to 409 `busy` on all session routes, or a D7.9 wording amendment accepting the uniform 404 at session resolution. The check records the observed status (`session_wait_observation` in the report) rather than asserting either.
- **Observation (consistent with APP-013 decision 2).** A revocation or adoption writer that takes the row `FOR UPDATE` is first met by the claim's foreign-key `KEY SHARE` (in the D7.9 order the claim precedes the fences), so a held writer of that kind answers `in_flight`, and only a `FOR NO KEY UPDATE` holder reaches the fence's `busy` row. Both are 409 with `Retry-After: 2`.
- **Recommendation (not done here).** Neither the APP-013 command campaign nor this race campaign is a step of `.github/workflows/tenant-foundation.yml`; adding both (about 22 s and 30 s) would make the landing PR's exact-source CI bind them (ADR-010 §4.2).

## Exact commands and results

`PYC` = `/private/tmp/claude-501/-Users-victorvillacis-dev/df0e31da-4f04-46a7-8043-30cdac91f9c2/scratchpad/app009/pyc` (main checkout `app/runtime/.venv/bin/python` with the contracts venv's `site-packages` on `PYTHONPATH`, as APP-009/010/013). `cargo +1.93.1` and `+1.85.0`; Node 22.21.0; `CARGO_BUILD_JOBS=2`; one Docker campaign at a time.

| Command (repository root unless noted) | Result |
| --- | --- |
| in `app/admin`: `npm ci --no-audit --no-fund && npm run typecheck && npm run build` | exit 0 |
| in `app/`: `cargo +1.93.1 fmt --check` | exit 0 |
| in `app/`: `cargo +1.93.1 clippy --locked --all-targets -- -D warnings`, and with `--features qualification-harness` | exit 0 both |
| in `app/`: `cargo +1.93.1 test --locked` | lib 105 passed / 13 ignored, bin 4, doctests 3 |
| in `app/`: `cargo +1.93.1 test --locked --features qualification-harness` | lib 116 passed / 13 ignored, bin 4, doctests 3 |
| in `app/`: `CARGO_TARGET_DIR=target/msrv cargo +1.85.0 check --locked --lib --tests --bins` (also with the feature) | exit 0 both |
| in `app/`: `GENUI_NATIVE_FIXTURE_PYTHON=PYC GENUI_RACE_REPORT=<scratch> cargo +1.93.1 test --locked --lib storage::race_qualification::native_race_campaign -- --ignored --exact --nocapture` (twice) | 1 passed both runs, 23 + 2 checks ([race-campaign.json](race-campaign.json), [race-campaign-run2.json](race-campaign-run2.json)); the first draft (22 + 2 checks) also passed twice before the delivery-after-fence order was added |
| in `app/`: `… storage::command_qualification::native_command_campaign …` (`GENUI_COMMAND_REPORT`) | 1 passed, 20 checks ([command-campaign.json](command-campaign.json)) |
| `PYC app/runtime/qualify_tenant_schema.py --output <scratch>` | exit 0, `passed: true`, 359 checks, every container removed ([schema-campaign.json](schema-campaign.json)) |
| `python3 scripts/check-schema-pins.py` | exit 0, `agree: true`, every pin 7 ([schema-pins.json](schema-pins.json)) |
| `python3 -m unittest discover -s scripts -p 'test_*.py'` | 17 OK |
| `PYC -m unittest discover app/runtime` | 131 OK (operator `DatabaseTests` included) |
| `PYC -m unittest discover -s app/qualification` | 6 OK |
| `PYC -m unittest discover -s fixtures -p 'test_*.py'` | 14 OK |
| `<main>/contracts/.venv/bin/python -m unittest discover -s contracts -p 'test_*.py'` | 26 OK |
| `docker build -f app/qualification/Dockerfile --target inputs --output type=local,dest=<scratch> .` + a scratch check against `local_harness.source_names()` | 77 `SOURCES`, all present and byte-identical in the qualification context, the new file included, no evidence/ticket/doc/build-output leaks ([image-contexts.json](image-contexts.json)); no image built |
| `python3 Tickets/_tools/board.py check`; `git diff --check` | OK (88 tickets); clean |

Every container these commands started was removed (the fixture's own removal, the qualifier's cleanup); `docker ps -a` afterwards shows only another session's long-exited `gatewasm-db-test-1`. Disk free stayed above 40 GiB.

## Limits

- Local password authentication over TLS, not RDS IAM; synthetic identities; HTTP sessions are minted by the `cfg(test)` constructor (the session campaign owns the fAuth path). The RDS repeat with lock observations is APP-080's.
- Held writers are fixture-only SQL in the operator channel's order; the unheld revocations are the real operator command; the adoption writer is test-only by D7.4 (no application adoption path exists before PH03). The fixture-only restorations write no operator audit event.
- The database itself is not restarted (tmpfs fixture); the restart is of every pool and store.
- `lock_timeout` is the pool baseline's 1,000 ms; held-past-timeout cases rely on it and on `idle_in_transaction_session_timeout` (3,000 ms) not being reached by a paused command.
- No independent review, no CI run, nothing pushed.
