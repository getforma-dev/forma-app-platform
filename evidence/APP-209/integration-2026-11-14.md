# Release C integration, 2026-11-14 (APP-209)

Branch `release/c-participation`, which started at `35d47c37` (B0's grammar, APP-202). The integration builder is Claude. It merged the builders' branches and then `main` in the order below. It never touched AWS, never merged to `main` and opened no pull request. All actors and data in the campaigns are synthetic.

## Merges

| Order | Branch | Head | Tickets | Merge commit |
| --- | --- | --- | --- | --- |
| 1 | `origin/release/c-b2-components` | `5d3a684d` | APP-204 (countdown, ranking by creator) | `0da9db92` |
| 2 | `origin/release/c-b4-approvers` | `5dd2b3f8` | APP-206 (approver standing) | `6843db0f` |
| 3 | `origin/release/c-b1-rules` | `731b283f` | APP-203, APP-205 (storage rules, forms, picker) | `8e93a91d` |
| 4 | `origin/main` | `c40d4535` | release B (schema 24, PR #124): APP-195/197 pool and refresh-herd gate, APP-196 exit codes, W3, the scan disposition, docs | the merge commit after `8e93a91d` |

PR #125 (`chore/app-194-scan-disposition`) was still open when `main` was fetched, so it is not in this tree.

## Conflicts and how each was resolved

1. **ADR-020 §17**, at merges 2 and 3. All three builders numbered their clarifications from 9. Each builder's subsection is kept whole and in merge order, and the items are renumbered into one sequence (mapping below). No item's text changed, apart from its number.
2. **`.github/workflows/tenant-foundation.yml`**, at merge 4. B1's `participation` step (APP-203) and main's `refresh-herd` step (APP-195, APP-197) were both appended after the private-entries step. Both are kept: `participation` first, then `refresh-herd`.
3. **`app/admin/src/record-components/controller.ts`**, at merge 4. B2 added the countdown helpers (`instant`, `countdownText`, `remainingSeconds`, `tickCountdowns`), and main added `const liveRead = retryOnce()` (APP-195) at the same place. Both are kept. Main's `refresh(live)` and B2's countdown ticker in the same function merged textually, and the admin typecheck and tests cover them together.
4. **`docs/README.md`**, at merge 4. Main reworded the ADR-019 row (the reservation ceiling of the 2026-11-14 amendment), and Release C added the ADR-020 row below it. Main's ADR-019 row and Release C's ADR-020 row are both kept.
5. **`docs/planning/TRIAL-25-READINESS.md`**, add/add at merge 4. Release C's design commit `55333401` had copied release B's plan at `f61b5999` and added the Release C amendment. The file was therefore merged three ways with `f61b5999` as the base (`git merge-file`). Main's two later changes were the gate-bound split (`6dd15b26`) and item 6's test-profile bounds. They applied beside the amendment. The one remaining hunk held two adjacent lines: the end of go/no-go item 5 and item 6. It keeps Release C's item 5 (the APP-206 sentence) and main's item 6 (the 8 s test-profile bound).
6. **Generated board views** (`Tickets/TRACKER.md`, `PHASES.md`, `SPRINTS.md`, `CATEGORIES.md`, `COVERAGE.md`, the phase, sprint and category pages, `index.json`), at merge 4. Main's side was taken, the views were regenerated with `python3 Tickets/_tools/board.py render`, and `board.py check` was run. Every hand-edited ticket file merged without a conflict. Main brings tickets APP-181 to APP-201, which the release B branch under `35d47c37` never carried. No APP ID is in two status folders.

### Merged without a textual conflict, and checked

- **`app/src/storage/mod.rs`.** It now exports the union: B4's `ApproverStanding` in `pub use sharing::{…}`, plus B1's `Refusal` in `pub use commands::{…}` and its `mod participation_campaign`.
- **The exhaustive matches** in `mcp/tools.rs`, `slack/flows.rs`, `actions/resolve.rs` and `actions/assistant.rs`. B1's single `Refused` arm in each sits beside B4's standing changes, and B4 added no new match over `CommandResult` or `DryRun`. `cargo check --all-targets` compiles the union.
- **`scripts/dev/campaigns.tsv`.** It holds the union: main's `refresh-herd` row and B1's `participation` row.
- **`managed_qualification.rs` `SOURCES` and `app/qualification/Dockerfile.dockerignore`.** The only new Rust file on either side is B1's `participation_campaign.rs`, which is listed once in each (236 entries). Main added no Rust source file, and its only change to `managed_qualification.rs` is a reflowed doc comment.
- **`evidence/APP-175/*` and `evidence/APP-193/finding-2026-11-14.md`.** The design commit had copied these from release B. They are byte-identical to main's copies.

## ADR-020 §17 renumbering

| Builder | Subsection | Branch numbers | Integrated numbers |
| --- | --- | --- | --- |
| B0 (APP-202) | the opening clarifications | 1–8 | 1–8 (unchanged) |
| B2 (APP-204) | "Clarifications, 2026-11-14 (APP-204, builder B2)" | 9–15 | 9–15 (unchanged) |
| B4 (APP-206) | "Clarifications, 2026-11-14 (APP-206, builder B4)" | 9–14 | 16–21 |
| B1, B3 (APP-203, APP-205) | "Clarifications, 2026-11-14 (APP-203 and APP-205, builders B1 and B3)" | 9–14 | 22–27 |

Per item:

- B4: 9 → 16 (`reviewer` means one who can open the work now), 10 → 17 (`ApproverStanding`), 11 → 18 (the read requires `record_write`), 12 → 19 (each path judges again), 13 → 20 (a missing standing), 14 → 21 (files outside B4's row).
- B1/B3: 9 → 22 (one compile arm per consumer), 10 → 23 (the bypass disables the creator rule), 11 → 24 (`Denied` outranks `requirement_not_met`), 12 → 25 (a form returns to the outcome's record), 13 → 26 (the picker's listing bound), 14 → 27 (form files outside B3's row).

Cross-references updated to the new numbers:

- `evidence/APP-206/break-and-restore-2026-11-14.md`: "items 9 to 14" → 16 to 21, §17.9 → §17.16 (three places), §17.11 → §17.18, §17.12 → §17.19.
- `evidence/APP-203/break-and-restore-2026-11-14.md`: "clarifications 9–14" → 22–27, §17.10 → §17.23.
- `evidence/APP-205/break-and-restore-2026-11-14.md`: "clarifications 13–14" → 26–27, §17.13 → §17.26.
- The ticket logs `APP-206.md` ("9–14" → also "16–21"), `APP-203.md` ("9–14" → also "22–27") and `APP-205.md` (§17.14 → §17.27). Each keeps the branch number beside the new one, so the builder's original statement can still be read.
- The code comments cite §17 only without an item number, or B0's unchanged items ("ADR-020 clarification 3" in `record_engine.py`), so none needed a change. B4's item 21 cites §17.8, which is B0's and also unchanged.
