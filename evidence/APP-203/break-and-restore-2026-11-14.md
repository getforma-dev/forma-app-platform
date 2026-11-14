# APP-203: break-and-restore proofs and checks (builder B1, 2026-11-14)

Branch `release/c-b1-rules`, built on `release/c-participation` at `35d47c37` (B0's grammar). The implementation commit under proof is `0f557305`; ADR-020 §5–6 and the clarifications 9–14 of §17 are what it builds. The campaign report of the run recorded here (at `0f557305`) is [participation-campaign-2026-11-14.json](participation-campaign-2026-11-14.json) (synthetic actors and data only).

## How it was built and proved

Tests and code were written in the same pass, and the first runs failed where they should have: the route's 422 test failed until the canvas copy existed; the campaign's first run failed its 25-way race on an `Unavailable` (the process admission bound, below) and its first lock-order probe missed the one-second `lock_timeout` window. The probe now pauses the transaction at the test checkpoint `key_match` and probes both locks with `FOR UPDATE NOWAIT`.

The `participation` campaign (`just campaign participation`, Docker) admits two general apps through the shipped path: ADR-020 §10's example verbatim from `contracts/fixtures/participation.json` (`definition_v1`), and a sessions app (an `own` list with a writable `closes_at` deadline and an `own` keyed check-in list whose `check_in` requires the referenced session open and `closes_at > now`, stamping `checked_at`). The actors are an owner, a reviewer by personal share, two contributors and 25 class members through the organization-wide share, and a guest (refused). The checks marked "fails under bypass" run twice: as shipped they hold, and with `authorization::BYPASS` set (own_rows TRUE, the key match's creator rule off, §17.10) they fail.

Checks of the recorded run (20, all passed, 211.7 s):

1. Both apps admit, publish and save; `responses` and `expected` are own collections.
2. C3 `set_now`: a post stamps `opened_at` with the transaction's instant, equal to the revision's `committed_at` and the outcome's; a contributor cannot post; nobody can supply the stamp.
3. C2: a second create for the same key replaces the first (version 2 of the first record; one `command.committed` event and one outcome naming it; creator and updater the contributor); the second form's id is never created; the dry run names the entry it replaces; a replay returns the stored outcome.
4. C2: identical values are 422 `unchanged` (dry run too); nothing written; the claim released, so the same key is judged again.
5. C2 × D7 (fails under bypass): two creators with the same key keep two entries; neither create nor the dry run touches the other's.
6. C2 × D7 (fails under bypass): a reviewer's own create never matches a contributor's entry, although the reviewer reads every entry.
7. C2: the reviewer's listing counts one entry per creator per key.
8. C3: open and inside the window commits; after the reviewer's `close`, 422 `requirement_not_met` (dry run too), nothing written; a contributor cannot reopen; the reviewer's `reopen` re-stamps and answers are accepted again.
9. C3: a reference to a record that does not exist, to another collection's record or to an id outside the work is the uniform 404 (commit and dry run).
10. C3 `now`: 31 s after the post on the database's clock an open item refuses answers (dry run too); a `reopen` re-stamps and answers are accepted again.
11. C3 `set_now` on a contributor's create: `checked_at` is the outcome's `committed_at`.
12. C3 × D7 (fails under bypass): a `via` reference into another member's own list is the uniform 404 (commit and dry run); the creator and a reviewer resolve it.
13. C3 `now`: `closes_at > now` accepts before the deadline and refuses at it; a member's close of their own session refuses their next check-in.
14. APP-205: the picker over the shipped reads (see `evidence/APP-205/`).
15. APP-205 × D7 (fails under bypass): the picker over an own list.
16. C2 race: 25 simultaneous creates by one creator for one key: exactly one entry, its state one of the submitted; 21 commits with versions 1..21, every outcome naming it; 4 were 422 `unchanged`; no other requested id exists.
17. C2 lock order (D7.9): paused right after the key match, the replacing create holds the organization's commit sequence and the matched entry's row (both refuse `FOR UPDATE NOWAIT`), then commits the entry's next version.
18. C3 race: in 6 rounds of a reviewer's `close` against 8 members' answers, every accepted answer (17) committed before the close in the organization's sequence; every other (31) was 422 `requirement_not_met` with nothing written.
19. C2 scale: 25 members × 50 items (1,275 key matches; the match statement p50 5.9 ms, p95 13.0 ms, max 112 ms against ADR-004's 2 s budget); exactly one entry per member per item.
20. Guests: a live share of the work (own collections) is refused.

## Break and restore

Each rule was broken on its own by a script (`breaks.py`, session scratchpad) in a separate detached worktree at `0f557305`. For each break it replaced exactly one occurrence of the rule's text (asserting it was unique), ran the named check, recorded what failed, restored the file with `git checkout --`, and required `git status --porcelain` to be empty before the next break; every restore was byte-identical to `HEAD`. Rows A are APP-203's (the participation campaign); APP-205's rows are in `evidence/APP-205/`.

| Break | Rule | Break (file: before → after) | What failed (first panic) | Detected | Restored, tree clean |
| --- | --- | --- | --- | --- | --- |
| A1 | the key match reads only the committer's rows | `commands.rs`: `format!("coalesce(v.creator_id={SCOPED},false)")` → `"TRUE".to_owned()` | exit 101; `storage::participation_campaign::native_participation_campaign`; C2 x D7: a reviewer's own create never matches a contributor's entry, although the reviewer reads every entry does not hold as shipped | yes | yes |
| A2 | the via read carries own_rows (current_state) | `commands.rs`: `super::authorization::own_rows("v", "$2::text::uuid", "$3::text::uuid", super::authorization::SCOPED)), &[&org` → `"TRUE"), &[&organization,&app,&work,&collection,&record,&version]` | exit 101; `storage::participation_campaign::native_participation_campaign`; C3 x D7: a via reference into another member's own list is the uniform 404 (commit and dry run); the creator and a reviewer resolve it does not hold as shipped | yes | yes |
| A3 | every requires guard is judged | `commands.rs`: `held &= guard.holds_at(&read[index].1, now);` → `let _ = guard.holds_at(&read[index].1, now);` | exit 101; `storage::participation_campaign::native_participation_campaign`; assertion `left == right` failed | yes | yes |
| A4 | now is the instant plus the offset | `rules.rs`: `timestamp_of(s + offset)` → `timestamp_of(s - offset)` | exit 101; `storage::participation_campaign::native_participation_campaign`; respond by 00000000-0000-4000-8000-000000000c03 must commit: Ok(Refused(RequirementNotMet)) | yes | yes |
| A5 | set_now stamps the transaction's instant | `commands.rs`: `let value = if declaration.set_now.contains(&field.id) {` → `let value = if false && declaration.set_now.contains(&field.id) {` | exit 101; `storage::participation_campaign::native_participation_campaign`; post by 00000000-0000-4000-8000-000000000c01 must commit: Err(Denied) | yes | yes |
| A6 | identical values are refused unchanged | `commands.rs`: `if entry.values == state {` → `if false && entry.values == state {` | exit 101; `storage::participation_campaign::native_participation_campaign`; assertion `left == right` failed | yes | yes |
| A7 | a keyed create replaces the creator's entry | `commands.rs`: `if current_version == 0` → `if false` | exit 101; `storage::participation_campaign::native_participation_campaign`; assertion `left == right` failed: the replacing create names the first entry at its next version | yes | yes |
| A8 | the matched row is locked FOR UPDATE | `commands.rs`: `let locking = if lock { " FOR UPDATE OF r" } else { "" };` → `let locking = if lock { "" } else { "" };` | exit 101; `storage::participation_campaign::native_participation_campaign`; the matched entry's row is locked by the key match | yes | yes |
| A9 | the dry run names the entry it replaces | `actions.rs`: `replaces = Some(entry.record.clone());` → `let _ = Some(entry.record.clone());` | exit 101; `storage::participation_campaign::native_participation_campaign`; Valid { collection: "391ba0b3-f505-4ff1-b98b-99a5a6313471", current_version: 0, state: Array [Object {"field_id": String("item"), "value": Object {"input_id": String("items"), "rec | yes | yes |
| A10 | the dry run judges requires | `actions.rs`: `if !judge_requires(transaction, org, app, work, declaration, &composed, &now).await? {` → `if false && !judge_requires(transaction, org, app, work, declaration, &composed, &now).await? {` | exit 101; `storage::participation_campaign::native_participation_campaign`; assertion `left == right` failed | yes | yes |

Not broken, and why: the key match's `own_rows` clause and the commit's defensive refusal of a matched row of another creator are both implied by the creator rule (A1) as shipped, so neither can be observed by itself; both are disabled together under the bypass, where checks 5, 6 and 12 fail. The scan test `every_record_statement_carries_own_rows` keeps the new statement (it carries `own_rows`).

## Commands and results

All in this worktree at `0f557305` unless named.

| Command | Result |
| --- | --- |
| `cargo test --locked -p forma-app-platform --lib -- storage::rules storage::commands::tests commands::tests storage::tests::every_record_statement storage::participation_campaign::tests managed_qualification` | ok (60 passed) |
| `cargo test --locked -p forma-app-platform --bin forma-app-platform` | ok (9 passed; the three SSR picker tests included) |
| `just check fmt clippy clippy-dev` | ok |
| `cd app/admin && npm test` / `npx tsc --noEmit` | 329 passed / ok |
| `just campaign participation` | ok, 211.7 s (report above) |
| `just campaign command` / `race` / `private-entries` / `workflows` / `actions` / `mcp` / `slack` | ok: 61 s / 74 s / 108 s / 140 s / 104 s / 81 s / 61 s |
| `just campaign record-components` / `canvas` / `pitch` | ok: 73 s / 66 s / 185 s |
| `just campaign query` | failed twice while other builds and campaigns loaded the machine, each time at a different timing-sensitive load-shedding assertion (`query_qualification.rs:1897`, a page 503; `:2632`, views `failed` where `loading`/`busy` was expected); at the same commit in an idle worktree it passed (105 s), as it does at the base `35d47c37` (108 s). Not a regression of this change; recorded so the reviewer can rerun it. |
| `python3 Tickets/_tools/board.py render` and `check` | ok (187 tickets) |
| `just check` | every step ok (fmt, admin, clippy, clippy-dev, test, http-smoke, msrv, catalog, infra, the Python suites, schema-pins, contracts, board, bash-n) except `py-promotion`, which fails identically at the base `35d47c37` (178 tests, 1 failure and 2 errors in `ops/promotion/test_schema_release.py`: release A's committed schema release is bound to its own baseline) |


## Findings and limits

- **Process admission.** One service process admits eight data transactions at a time (`pool::capacity`, two connections per role) and refuses the next with `unavailable` (503) before any statement. The campaign resends such a refusal with the same key, as a client after a 503 may; it met 664 refusals in the 25-way race and 34,793 at scale (each a 20 ms retry). The canvas client answers a 503 by reading the outcome by key and, when nothing committed, says "could not be confirmed; submitting again with the same form is safe". A class of 25 pressing submit in the same instant will see that for some members. This is unchanged behaviour of release B, outside B1's files; the plan's class guidance (APP-209) should name it.
- The 422 bodies are proved at the route's `answer` (`commands::tests`), not over a served HTTP route in a campaign.
- The 30 s window is proved on the database's real clock (one 31 s wait), not with a moved clock.
- The independent Tier A review of authorization, concurrency, the refusal codes' disclosure and contract drift is not done (the ticket stays Backlog until its predecessor APP-202 is Done, the board rule). CI was not observed by the builder.
