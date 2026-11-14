# APP-204: countdown and ranking by creator, checks and break-and-restore proofs (builder B2, 2026-11-14)

Branch `release/c-b2-components`, built on `release/c-participation` at `35d47c37` (ADR-020, edition `impl-2026-11-14.2` from APP-202). ADR-020 §4 (C1) and §7 (C4) in builder B2's files only (§13), plus the seven dated clarifications 9–15 in ADR-020 §17 and this evidence. The commits are named in the ticket log.

## What was built

| Piece | File | What it does |
| --- | --- | --- |
| Capture | `app/src/storage/record_components.rs` | Every row carries its revision's `creator`; the payload gains `creators`, an id-sorted `[{id, label}]` array (`directory::label` for a member, `directory::neutral_label` for a guest); the key input a `matches` measure names is captured beside the component inputs, under the same `own_rows`; the snapshot digest domain is `forma:record-components-snapshot:v2`; a payload over 1,000,000 canonical bytes is `ProjectionLimit` before any child is launched. |
| Engine | `app/runtime/record_engine.py` | `countdown`: the current record (highest commit sequence) → `value` = timestamp + offset (or null), `label` = first text field (≤ 200 characters), `closed` from `closed_when` or a null target. `record_ranking` with `group_by: "creator"`: one group per creator, labelled from `creators`, sorted by value, label, then the (undisclosed) creator; `limit` ≤ 50. `matches` on summaries and rankings: rows equal on every named field (canonical bytes) to some key row. |
| Wire projection | `app/src/query/record_components.rs` | A countdown projection must carry a contract timestamp or null, a bounded `label`, `closed`, `value_type` timestamp and nothing else; no other kind may carry `label` or `closed`. Countdown props (`has_countdown`, `countdown_state`, `countdown_target`, `countdown_text`, `server_time`); `stamp` writes the server's instant and the text at that instant. |
| Route | `app/src/query/record_routes.rs` | `Batch.server_time` on every member and guest response, taken once after the release check and stamped on every countdown; `is_record_kind("countdown")`. |
| Client | `app/admin/src/record-components/*` | The SSR part and the imperative renderer show the timer (`role="timer"`, `data-countdown`, `data-countdown-target`, `data-server-time`); the controller ticks every 500 ms from `server_time` plus `performance.now()` since the response (`remainingSeconds`, `countdownText`, `tickCountdowns`), never `Date.now()`; a closed countdown never restarts. |

## Interfaces published

- HTTP `POST|GET /v1/work/{app}/{work}/record-components` and `/v1/shared/{share}/record-components`: top-level `server_time` (`YYYY-MM-DDTHH:MM:SSZ`), and per component `has_countdown`, `countdown_state`, `countdown_target`, `countdown_text`, `server_time`.
- Engine result projection: optional `label` and `closed` (countdown only); `value` is a contract timestamp for a countdown.
- Engine payload: `creator` on every dataset row; top-level `creators: [{id, label}]` (clarification 9).
- Snapshot digest domain `forma:record-components-snapshot:v2`; result and receipt domains and the receipt kind unchanged.
- `query::record_routes::is_record_kind("countdown")`, which the native work page, the canvas and the shared page already route through.

## Tests first

The tests were written before the code they exercise. The engine suite (`app/runtime/test_record_engine.py`, 15 new tests beside the 7 retained) first ran `FAILED (failures=3, errors=16)` of 22. The client tests (`app/admin/test/record-components-controller.test.ts`) first failed because `countdownText`, `remainingSeconds` and `tickCountdowns` did not exist. The Rust unit tests (`query::record_components`, `query::record_routes`, `storage::record_components`) first failed to compile against the missing `instant`, `stamp`, `countdown_text`, `released` and `RecordComponentsRun::for_tests`. The campaign's participation section was written against the storage and engine interfaces. Its first run found the one design gap of this ticket: a `creators` object keyed by principal ids cannot be encoded (forma-json-v1 keys are local ids), which is ADR-020 clarification 9.

## The record-components campaign (APP-204 section)

`just campaign record-components` admits a general participation app through the definition path (`create-app`, `create_definition_revision`, `publish`, `create_work` with three records bindings). The app has a shared `items` list, an `own` `responses` list and an `own` `expected` key list. Its views are a countdown (`opened_at` + 30, closed when `state` = closed), a `matches` summary, a ranking by creator with limit 50, and one with limit 1. The members are the owner, a reviewer (reviewer share) and three contributors: Ben Okafor and Ana Lima, who set display names through `set_display_name`, and a third who sets none. Every record is written by `commit_record`. The checks added (report: [record-components-campaign-2026-11-14.json](record-components-campaign-2026-11-14.json), 18 checks, `passed: true`):

- Before any item, a contributor's countdown is empty: no target, "Nothing is open yet", no timer.
- After two posts, the owner's and the contributor's countdown follow the item with the highest commit sequence ("Which shape?", target 15:05:30Z). Stamped at the server's 15:05:05Z it reads `0:25`, and an hour earlier `1:00:25`.
- After the reviewer's close it reads "Closed". Closing the older item makes that item current.
- On the owner's and the reviewer's page the ranking is Ben Okafor 2, Ana Lima 1, then the neutral `Member c0000c05` 1. The limit-1 ranking is truncated and its message says so. The receipt lists the items, responses and expected revisions (2, 5, 2) and names the countdown. It admits in Rust and validates under `contracts/validate.py`.
- On each contributor's page the ranking lists only that contributor, scored 0. Their receipt lists no expected revision.
- The result document carries no principal id. The page props name no member except the viewer, in their own receipt.
- On the dining template, a member's `creators` map carries the owner's display name and a guest's carries only the neutral handle.
- Three D7 checks run as shipped and again with `authorization::BYPASS` set; each held as shipped and failed under bypass:
  - the contributor's ranking lists only themselves;
  - the contributor's capture reads their own responses and no key row, while a reviewer's reads all;
  - the APP-191 delivery judgment (`openable_principals`) opens Ben's response to Ben, the reviewer and the owner, never to Ana or the third contributor.

## Commands and results (2026-11-14, worktree `forma-relc-b2`, commit `57b2dbf3`)

| Command | Result |
| --- | --- |
| `just check` | 20 steps ok: fmt, admin (330 tests), clippy, clippy-dev, test (461 library + 9 binary SSR + 3 doctests), http-smoke, msrv, catalog, infra, py-scripts, py-runtime (221 tests), shared-broker, py-qualification, py-app-scripts, py-ops, py-infra, schema-pins, contracts, board, bash-n. **py-promotion FAIL** (`failures=1, errors=2` of 178, all in `ops/promotion/test_schema_release.py`: "schema or deployment contract changed; separate reviewed release required"). This is release A's committed schema-release binding. The deployment contract digest covers `infra/src/`, `app/migrations/`, `app/runtime/runtime.py` and the Dockerfiles, none of which this change touches, and the tests compare against `HEAD`. |
| `just check admin` | ok (inside `just check`, and run alone after the client change) |
| `just campaign record-components` | ok, 1 passed (51.6 s), report `passed: true`, 18 checks |
| `just campaign private-entries` | ok, 1 passed (96.6 s), 60 checks |
| `just campaign live-sharing` | ok, 1 passed (75.1 s), 17 checks |
| `just campaign query` | **FAIL**, twice on this branch, at `query_qualification.rs:2632`: under a filled engine queue the work page's two source-query views showed `failed` where `loading`/`busy` was expected. **The same campaign also fails on the base `35d47c37`**, run in this worktree, earlier, at `query_qualification.rs:1796`: a source view had no summary. The load average was 10 to 17 throughout (other builders' campaigns). The failing views are source queries in a work with no record view, a path this change does not reach. Not attributable to APP-204; to be re-run on a quiet machine. |
| `app/runtime/.venv/bin/python -m unittest app/runtime/test_record_engine.py` | 22 tests OK |
| `cargo test --locked -p forma-app-platform --lib -- record_components record_routes` | 12 passed (1 ignored: the campaign) |

## Break-and-restore

Every guard this ticket adds was broken on its own by a script (the session's `breaks204.py`; results [breaks-2026-11-14.json](breaks-2026-11-14.json)). The script works on the committed tree. For each break it asserts the exact span and its occurrence count, replaces it, and runs the named command. It then restores the file with `git checkout --` and requires the file's blob to equal `HEAD`'s and `git status --porcelain` to be empty before the next break. All 19 were caught (every command exited non-zero) and every restore was byte-identical.

B1 is the point of ADR-020 §14.4. With `own_rows` replaced by `TRUE` in the capture's bounds and rows statements, the campaign stops at the contributor's ranking check (`record_components_campaign.rs:1035`). Ben's page then shows `[("Ben Okafor", "2", 2), ("Ana Lima", "1", 2), ("Member c0000c05", "1", 1)]` instead of `[("Ben Okafor", "0", 2)]`, and the storage scan `every_record_statement_carries_own_rows` refuses the statements.

| Break | Rule | Break (file: before → after) | Command (exit) | Failing tests | Restored byte-identical, tree clean |
| --- | --- | --- | --- | --- | --- |
| B1 | own_rows on the capture (bounds and rows) | record_components.rs (×2): `own = super::authorization::own_rows("v", "$2::text::uuid", "$3::text:…` → `own = "TRUE"` | `just campaign record-components` (101); `cargo test --locked -p forma-app-platform --lib -- storage::tests::every_record_statement_carries_own_rows` (101) | `native_record_components_campaign`; `every_record_statement_carries_own_rows` | yes (blob `f88376f5e0f6`) |
| B2 | the key input is captured | record_components.rs: `v["records"]["measure"]["key_input"].as_str(),` → `None,` | `cargo test --locked -p forma-app-platform --lib -- record_components record_routes` (101) | `a_matches_key_input_is_captured_beside_the_component_inputs` | yes (blob `f88376f5e0f6`) |
| B3 | a guest sees only the neutral handle | record_components.rs: `    if guest { ⏎         crate::directory::neutral_label(principal)` → `    if false { ⏎         crate::directory::neutral_label(principal)` | `cargo test --locked -p forma-app-platform --lib -- record_components record_routes` (101) | `a_guest_never_sees_a_members_chosen_name` | yes (blob `f88376f5e0f6`) |
| B4 | the payload bound | record_components.rs: `if encoded.len() > RECORD_COMPONENT_PAYLOAD_BYTES {` → `if false {` | `cargo test --locked -p forma-app-platform --lib -- record_components record_routes` (101) | `a_payload_the_child_cannot_read_whole_is_a_projection_limit` | yes (blob `f88376f5e0f6`) |
| B5 | the snapshot domain is v2 | record_components.rs: `pub const SNAPSHOT_DOMAIN: &str = "forma:record-components-snapshot:v2…` → `pub const SNAPSHOT_DOMAIN: &str = "forma:record-components-snapshot:v1…` | `cargo test --locked -p forma-app-platform --lib -- record_components record_routes` (101) | `the_snapshot_domain_is_v2_only` | yes (blob `f88376f5e0f6`) |
| B6 | a countdown value is a contract timestamp | record_components.rs: `.is_some_and(\|s\| !admission::contract_timestamp(s))` → `.is_some_and(\|_\| false)` | `cargo test --locked -p forma-app-platform --lib -- record_components record_routes` (101) | `a_countdown_projection_carries_an_instant_a_label_and_closed_only` | yes (blob `0f8f99c2c456`) |
| B7 | only a countdown carries label and closed | record_components.rs: `} else if v.label.is_some() ⏎             \|\| v.closed.is_some() ⏎   …` → `} else if v.value` | `cargo test --locked -p forma-app-platform --lib -- record_components record_routes` (101) | `a_countdown_projection_carries_an_instant_a_label_and_closed_only` | yes (blob `0f8f99c2c456`) |
| B8 | a closed countdown reads Closed | record_components.rs: `Some(target) if p.countdown_state == "open" => countdown_text(target -…` → `Some(target) => countdown_text(target - now),` | `cargo test --locked -p forma-app-platform --lib -- record_components record_routes` (101) | `countdown_props_and_the_server_instant` | yes (blob `0f8f99c2c456`) |
| B9 | every response carries server_time | record_routes.rs: `server_time: crate::boxes::register::utc(now),` → `server_time: String::new(),` | `cargo test --locked -p forma-app-platform --lib -- record_components record_routes` (101) | `every_response_carries_the_server_instant_and_stamps_countdowns` | yes (blob `bf9240a6fd12`) |
| B10 | countdown is a record kind | record_routes.rs: `            \| "record_budget" ⏎             \| "countdown"` → `            \| "record_budget"` | `cargo test --locked -p forma-app-platform --lib -- record_components record_routes` (101) | `a_countdown_is_a_record_kind` | yes (blob `bf9240a6fd12`) |
| E1 | the current record is the highest commit sequence | record_engine.py: `current=max(candidates,key=lambda item:(item[0]["sequence"],item[0]["i…` → `current=next(iter(candidates),None)` | `app/runtime/.venv/bin/python -m unittest app/runtime/test_record_engine.py` (1) | `test_child_protocol_carries_the_new_members`, `test_the_current_record_is_the_highest_commit_sequence` | yes (blob `8c4ec1c94b4f`) |
| E2 | a creator group is labelled, never identified | record_engine.py: `{"label":creators[creator],"missing":False` → `{"label":creator,"missing":False` | `app/runtime/.venv/bin/python -m unittest app/runtime/test_record_engine.py` (1) | `test_child_protocol_carries_the_new_members`, `test_contributor_snapshot_is_their_own_rows_and_no_key`, `test_labels_never_ids_leave_the_engine`, `test_reviewer_snapshot_ranks_every_contributor_by_label`, `test_ties_follow_labels_then_a_stable_hidden_order` | yes (blob `8c4ec1c94b4f`) |
| E3 | matches is typed equality on the whole value | record_engine.py: `return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure…` → `return json.dumps({k: v for k, v in value.items() if k != "input_id"} …` | `app/runtime/.venv/bin/python -m unittest app/runtime/test_record_engine.py` (1) | `test_typed_equality_on_every_named_field` | yes (blob `8c4ec1c94b4f`) |
| E4 | a ranking page is at most 50 | record_engine.py: `RANKING_LIMIT = 50` → `RANKING_LIMIT = 51` | `app/runtime/.venv/bin/python -m unittest app/runtime/test_record_engine.py` (1) | `test_fifty_is_the_bound_and_truncation_is_reported` | yes (blob `8c4ec1c94b4f`) |
| E5 | creators is id-sorted with no duplicate | record_engine.py: `require(entry["id"]>last);last=entry["id"]` → `last=entry["id"]` | `app/runtime/.venv/bin/python -m unittest app/runtime/test_record_engine.py` (1) | `test_group_by_and_group_field_are_exclusive_and_creators_must_cover_rows` | yes (blob `8c4ec1c94b4f`) |
| E6 | closed_when closes the countdown | record_engine.py: `closed=value is None or (current is not None and "closed_when" in conf…` → `closed=value is None` | `app/runtime/.venv/bin/python -m unittest app/runtime/test_record_engine.py` (1) | `test_closed_when_and_a_null_timestamp_close_it`, `test_the_current_record_is_the_highest_commit_sequence` | yes (blob `8c4ec1c94b4f`) |
| E7 | no filter narrows a countdown | record_engine.py: `require(kind!="countdown" or not applied)` → `pass` | `app/runtime/.venv/bin/python -m unittest app/runtime/test_record_engine.py` (1) | `test_malformed_countdowns_refuse` | yes (blob `8c4ec1c94b4f`) |
| T1 | the client never reads the wall clock | controller.ts: `const end = instant(target), start = instant(serverTime);` → `const end = instant(target), start = Math.floor(Date.now() / 1000);` | `npx --prefix app/admin tsx --test app/admin/test/record-components-controller.test.ts` (1) | `a browser clock an hour off changes nothing`, `remaining time is the target minus the server instant and the elapsed monotonic time` | yes (blob `0b9335272d13`) |
| T2 | a closed countdown never restarts | controller.ts: `if (timer.dataset.countdown !== 'open') continue;` → `(removed)` | `npx --prefix app/admin tsx --test app/admin/test/record-components-controller.test.ts` (1) | `a browser clock an hour off changes nothing` | yes (blob `0b9335272d13`) |

## D7, the receipt and the bounds (for the Tier A reviewer)

- **No new authority path.** Each input, including a `matches` key input, is read by the existing capture statements, which carry `own_rows` with the viewer's scoped principal. There is no new SQL statement: the rows statement gains `v.creator_id::text` only. The release check is unchanged.
- **What leaves.** The engine receives principal ids (row `creator`, `creators[].id`) and emits only labels. The parser admits no `label`/`closed` outside a countdown. The HTTP batch serializes props only (`run` is skipped). A guest gets neutral handles (clarification 10).
- **Receipt.** Kind, schema and domain are unchanged. The `snapshot` lists every captured input's revisions, the key input's included. The snapshot digest is domain v2.
- **Bounds.** Rows and canonical bytes are bounded as before (2,000; 512 KiB), the `creators` entries are bounded by the rows, the ranking page by 50 and the countdown label by 200 characters. A payload over 1,000,000 canonical bytes is refused before launch.

## Limits

- **The Tier A review is open.** It is the fourth acceptance item; this file is the builder's record, not the review.
- **`query` is red on base and branch under load.** See the table above.
- **Browser behaviour is untested here.** The ticking timer, the hour-off browser clock in a real browser, and the card's re-run on `record.committed` were not run in a browser. The re-run uses the existing paths: `canvas/board.ts` `record.committed` → `runGroup` → the record controller's `refresh`; the work page's hint refresh and 30 s timer in `work/document.ts`. Victor does browser QA.
- **No full-scale truncation run.** Truncation past 50 is proved in the engine with 51 creators and in the campaign with a limit-1 ranking, not with 51 real members.
- **The campaign writes `opened_at` itself.** `set_now` and `requires` are APP-203's and are not on this base (clarification 15).
- **The live-hint check uses the judgment, not open streams.** It calls the production judgment (`openable_principals`), not the 26-stream pump round of the private-entries campaign.
- **CI was not observed.**
