# APP-200: the record snapshot ceiling follows the query worker's admission

Branch `trial/w4-private-entries` (W4), 2026-11-14. Decided by the Fable advisor in lieu of Victor from the APP-197 measurement (26 open pages missed first-try 200 on 429s from the four-snapshot cap). The ticket lives on `docs/capacity-redecision` (PR #120), `Tickets/0-backlog/APP-200.md`.

## The change

- `app/src/storage/record_query.rs`: `Semaphore::new(4)` is replaced by `SNAPSHOT_SLOTS = crate::query::worker::TASK_RUNNING + crate::query::worker::QUEUE` (4 + 20 = **24**), behind `reserve_slot()`, which still `try_acquire`s and answers `Error::Busy` (the route's 429 with `Retry-After: 2`) without waiting. One organization's views are therefore refused first by the worker's frozen admission (D7.10), never by this cap.
- The comment on the snapshot's slot and the `Error::Busy` message (`storage/mod.rs`: "record snapshot capacity reached (the query worker's admission, 24 slots)") follow.
- `docs/architecture/CSV-APP-DATA.md` gains the dated line: the ceiling, its derivation, the worst case and the data pool statement.
- `storage::record_query_campaign::qualify` (run by the `box` campaign) holds `SNAPSHOT_SLOTS − 2` more snapshots beside its two live runs and expects `Busy` on the one past the ceiling, then a release after a drop.

## Light Tier A note

- **Memory.** Each snapshot is bounded at 64 MiB of canonical row bytes before any byte leaves PostgreSQL (`MAX_BYTES`, checked on the bounds CTE and while materializing); 24 × 64 MiB ≈ **1.5 GiB** in the 8 GiB task, in the worst case where every slot holds a maximal snapshot. The practical case is far smaller (the box campaign's 2,505-record collection is about 313 KB per snapshot). The engine's own memory line and deadline are unchanged.
- **Database reads stay bounded by the data pool.** A capture runs inside one data-role transaction on one pooled connection and releases it at `finish`; the slot outlives the connection (held through execution and release), so raising the slot count raises resident snapshots, not concurrent database reads. The pool's size and the per-statement budget (2 s) are untouched.
- **No authority change.** The slot is taken after the member's precheck and fence; a refused slot discloses nothing but the 429.

## Tests and results

| Test | Result |
| --- | --- |
| `storage::record_query::tests::snapshot_slots_follow_the_query_workers_admission`: 24 slots held, the 25th is `Busy`, one released, admitted again, then `Busy` again; the constant is 24 | `cargo test --lib -p forma-app-platform snapshot_slots_follow`: ok |
| Break proof: `SNAPSHOT_SLOTS` set back to `4` | the same test failed: `slot 4 refused: record snapshot capacity reached (the query worker's admission, 24 slots)`; restored, ok (`app200-break.log`) |
| `box` campaign (`record_query_campaign::qualify` saturation at the new ceiling: two live runs plus 22 more held, the 25th `Busy`, a drop frees one) | `just campaign box`: ok, 19 checks (`r5-box.log`) |

## Limits

- The 26-page release-profile `refresh-herd` measurement (zero view 429s in every herd, first-try share and p95 against the 2026-11-14 figures) is APP-197's campaign on another branch; the coordinator re-runs it there after both merge.
- The ceiling is per service process (ADR-007 D7.18: one task); a multi-task deployment multiplies it.
