# APP-198: a stale record snapshot is typed, re-captured once, never "no access"

Branch `trial/w4-private-entries` (W4), 2026-11-14. Decided by the Fable advisor in lieu of Victor; the ticket file is created on `main`.

## The defect

`record_query::release` (the last check before a record-backed query's body is written, C07) answered `Denied` when the list had moved between the snapshot's capture and its release (`current != snapshot.last_commit`), the same `Denied` as a binding mismatch or lost authority. The query route maps `Denied` to the uniform 404, and the canvas shows "You don't have access to this card." for a 404: a member whose colleague committed a row while their card computed was told they had no access.

## The fix

- `storage::Error::Stale` (`"record snapshot stale"`). `record_query::release` answers it for the moved list; `Denied` stays for a binding that changed (and authority stays in the callers' fences: `release_result` re-fences the member and re-reads the premise before the snapshot check; `release_guest_result` re-checks the guest). `app/src/storage/record_query.rs`, `app/src/storage/mod.rs`.
- The JSON query route (`query::routes::run_released`, shared with MCP `forma_run_view`): on `Stale` from the release, the view is captured, run, receipted and released **once more** while the handler's budget leaves room for an execution (`HANDLER_DEADLINE − INTERACTIVE_DEADLINE`, 3.2 s), logged as `query.stale_recaptured`; a second stale release answers the typed not-ready `409 {"code":"stale","status":409}` (`storage_refusal`). `release_code` logs `stale`. Never the uniform 404.
- The SSR work page: a stale release renders the `loading` card with badge `stale` and the message "Updating: the list changed while this view was computed…" (`page::Outcome::Stale`), not the denied card.
- The canvas client (`app/admin/src/canvas/queries.ts`): `409 stale` shows "Updating…" (`loading`/`stale`) and re-reads after one second, up to three times; after that "This list keeps changing. Try again in a moment." (`failed`/`stale`, with the Try again button). A 404 still means denied.
- MCP (`mcp::tools::storage_output`): `Stale` is the error output `stale` ("The list changed while this view was computed. Run it again."), not `not_found`.
- The guest live-share page shares `storage_refusal`, so a guest's stale release is the same `409 stale`.

## Light Tier A check

- **No new disclosure.** The stale answer is reached only after `run_query` admitted the member (precheck → fence → `Permit`, `record_read` on the work, `own_rows` on the snapshot) and after `release_result` re-fenced them and found the premise unchanged; it says only that the list the member already reads under RLS has a newer commit than the snapshot. The response carries no ids, counts or values. A member who cannot open the work never reaches the release (uniform 404 at `run_query`).
- **`Denied` still covers binding changes.** `release` checks the binding first (`require(binding(...) == snapshot's)`) and answers `Denied` before reading the commit sequence; the record-query campaign keeps its binding and permission cases on `Denied` (source permission removed → `Denied` at release and at the next capture).
- **Stale cannot mask a revocation.** The re-capture is a full `run_query` (authorization again); a member revoked between the first capture and the retry gets `Denied` from the retry's `run_query` (the uniform 404, as before).
- **Bounded.** One re-capture per request; the second attempt runs only when at least one execution fits before the handler deadline; snapshot slots are released between attempts (the first `QueryRun` is dropped before the second capture).

## Tests

| Test | Where | Shows |
| --- | --- | --- |
| A commit between capture and release | `storage::record_query_campaign::qualify` (run by the `box` campaign) | `release_result` and `release_guest_result` answer `Err(Error::Stale)` after a commit; a fresh capture releases; the binding/permission cases stay `Err(Error::Denied)` |
| Refusal mapping | `query::routes::tests::refusal_mapping_matches_design_section_nine` | `storage_refusal(&Error::Stale)` is `409 {"code":"stale","status":409}`, problem+json, no-store, no `Retry-After`; `Denied` stays 404 |
| The card | `query::routes::tests::view_states_loading_empty_failed_denied_never_zero` | `Outcome::Stale` → `loading`/`stale`, no number, no receipt, not denied |
| The route, once and twice | `box` campaign, `qualify_editable_csv` (APP-198 block) | the engine paused after computing (`Counting::pause_next`), a commit through the assistant's proposal and confirmation, resume: **200** with the current list (one more row; the engine ran twice); the re-capture paused too and a second commit: **409 `stale`**, body exactly `{"code":"stale","status":409}`, not the uniform 404, no third run; the next read is current (three more rows) |

## Results (2026-11-14)

- `cargo test --lib -p forma-app-platform query::routes::tests`: ok, 12 passed (the mapping and the card cases above included).
- `cargo test --locked -p forma-app-platform --bin forma-app-platform fabricated_zero`: ok (the SSR page renders every view state without a number; the stale card is a loading card).
- `just campaign box`: ok, **19 checks** (18 before; the APP-198 check last of the editable section), including `record_query_campaign::qualify` with the `Stale` assertions. The route block's assertions held: 200 after one re-capture with the engine called twice; `409 {"code":"stale","status":409}` on the second commit with no third run; the next read current. Log: the session's `box-198.log`.
- `cargo clippy --locked -p forma-app-platform --all-targets -- -D warnings`: clean. `npm --prefix app/admin run typecheck`, `test` (301 pass) and `build`: ok (`app/admin/dist` is built in CI, not committed).
- The full check set and the campaign sweep after this change are recorded in `evidence/APP-191/break-and-restore-2026-11-14.md` (the W4 branch's final sweep).

## Limits

- The re-capture is bounded by the handler budget; under load a first stale release late in the budget answers `409 stale` at once and the client re-reads (one second later, three times). The client then shows "This list keeps changing. Try again in a moment." with the Try again button rather than retrying forever.
- The SSR work page does not re-capture (its views are computed in one pass); it renders the stale card, and the page's island re-reads through the JSON route, which does.
- The guest live page maps a stale release to `409 stale` through the shared `storage_refusal`; it does not re-capture (the guest page is one document); its client re-reads as before.
