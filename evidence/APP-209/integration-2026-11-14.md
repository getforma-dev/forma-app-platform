# Release C integration, 2026-11-14 (APP-209)

Branch `release/c-participation`, which started at `35d47c37` (B0's grammar, APP-202). The integration builder is Claude. It merged the builders' branches and then `main` in the order below. It never touched AWS, never merged to `main` and opened no pull request. All actors and data in the campaigns are synthetic.

## Merges

| Order | Branch | Head | Tickets | Merge commit |
| --- | --- | --- | --- | --- |
| 1 | `origin/release/c-b2-components` | `5d3a684d` | APP-204 (countdown, ranking by creator) | `0da9db92` |
| 2 | `origin/release/c-b4-approvers` | `5dd2b3f8` | APP-206 (approver standing) | `6843db0f` |
| 3 | `origin/release/c-b1-rules` | `731b283f` | APP-203, APP-205 (storage rules, forms, picker) | `8e93a91d` |
| 4 | `origin/main` | `c40d4535` | release B (schema 24, PR #124): APP-195/197 pool and refresh-herd gate, APP-196 exit codes, W3, the scan disposition, docs | `dcada745` |

After the merges, `b848b08f` corrected the comment on `participation_campaign.rs`'s `admitted_commit`. It still described the 8-caller pool, and after the merge with main each role admits `pool::ADMITTED` = 32 callers. The change is to the comment only.

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

## Checks on the integrated tree

The checks ran at `b848b08f` on the integration builder's macOS host. The user's own applications (Zoom, the editor and the Docker VM) kept the 1-minute load between 4 and 6.4 for most of the runs, so this was not a quiet host. On the coordinator's instruction, the regular campaigns ran at that load, one at a time, each alone. The 26-page refresh-herd measurement waits for a quiet host.

### `just check`

Every step passes except `py-promotion`: fmt, admin (347 tests), clippy, clippy-dev, test (478 library tests plus the binary SSR tests and doctests), http-smoke, msrv (Rust 1.85.0), catalog, infra, py-scripts, py-runtime (235), shared-broker, py-qualification, py-app-scripts, py-ops (40), py-infra, schema-pins, contracts (the edition `impl-2026-11-14.2` lock: 76 artifacts and 27 retained), board and bash-n.

**`py-promotion`: 1 failure in 238 tests, `test_schema_release.CommittedReleaseTests.test_committed_release_is_bound_to_this_tree`.** The tree's deployment-contract digest is `3493b0a6…`, and release B's committed record (`ops/promotion/schema-release.json`, `to.deployment_contract_sha256`) pins `88d17cb5…`, which is `main`'s digest. The merge did not cause this. `ops/promotion/candidate.py` `deployment_contract_digest` covers `app/qualification/Dockerfile.dockerignore`, and that is the only deployment-contract input that differs between `main` and this tree. Release C must change it. The qualification image compiles `managed_qualification.rs`, whose `SOURCES` `include_bytes!` B0's retained edition fixtures and `contracts/fixtures/participation.json` and B1's `participation_campaign.rs`, so the image's build context has to admit them (ADR-020 §17.8). Release C changes no migration, `runtime.py`, Dockerfile or CDK file.

**Consequence for APP-209: an open decision, not resolved here.** ADR-020 and APP-209 plan Release C as a normal promotion whose deployment-contract digest equals the live baseline's. That premise does not hold for this tree, and the current promotion code has no path for it:

- `runner.reviewed_release` admits a normal promotion only when the schema-manifest digest and the deployment-contract digest both equal the baseline's. Release C's schema manifest is unchanged, but its deployment contract is not.
- Otherwise it calls `schema_release.applicable`, which needs a committed reviewed release. `schema_release.load` refuses a release unless both digests differ between `from` and `to`, so a schema-24 → 24 release record cannot be written either.
- The promotion would therefore stop with `schema or deployment contract changed; separate reviewed release required`.

The integration builder did not change the promotion contract, release B's reviewed record or the deployment-contract file list. Each of those is a promotion-contract change for its owner and an independent review. For APP-209 the choices appear to be:

- keep the qualification image's dockerignore out of the deployment contract;
- admit a reviewed contract-only release; or
- deliver it by the separately reviewed path that `docs/DEVELOPMENT.md` names for "other schema and infrastructure changes".

Until then, this test stays red on any tree that carries both release B's committed record and Release C's dockerignore. That includes this branch's CI, and `main` after Release C merges until release B's record is retired.

### Campaigns, each run alone

| Campaign | Result | Checks | Time | 1-min load at start |
| --- | --- | --- | --- | --- |
| `participation` | pass, first run | 20/20 | 86 s | 4.58 |
| `record-components` | pass, first run | 18/18 | 22 s | 4.87 |
| `private-entries` | pass, first run | 60/60 | 30 s | 4.34 |
| `actions` | pass, first run | 20/20 | 38 s | 4.18 |
| `mcp` | pass, first run | 15/15 | 29 s | 4.11 |
| `slack` | pass, first run | 12/12 | 35 s | 4.62 |
| `share-routes` | pass, first run | 17/17 | 25 s | 5.06 |
| `sharing` | pass, first run | 12/12 | 15 s | 4.82 |
| `workflows` | pass, first run | 7/7 | 31 s | 5.01 |
| `query` | pass, first run | 26/26 | 69 s | 4.41 |
| `box` | pass, first run, after `just parser` built the sealed parser | 20 checks | 102 s | 6.39 |
| `refresh-herd` (15-page gate; `GENUI_REFRESH_HERD_FULL` unset) | pass, first run | gate A and gate C | 97 s | 3.97 |

No campaign needed a rerun, so none is recorded as load-sensitive. Each command was `just campaign <name>`.

**`refresh-herd`.** Gate A ran 15 pages, three herds 10 s apart, jitter of 0–3 s, one retry and 32 admitted callers per role, and passed at main's 8 s test-profile bound:

- p95 current after the send: 4,447, 4,361 and 3,589 ms;
- every page current within 10 s (max 4,516 ms);
- first-try 200 share 1.0, no read answered 503 and no terminal failure;
- the owner's command took 101, 196 and 74 ms;
- 4 catalog checks in the run;
- no admission saturation.

Gate C ran 15 pages at one instant with no retry and 8 admitted per role. It reproduced the APP-193 finding as designed: 111 saturations, and 37 terminal failures per herd. Report: [refresh-herd-gate-2026-11-14.json](refresh-herd-gate-2026-11-14.json). The 26-page release-profile measurement (`GENUI_REFRESH_HERD_FULL=1`) was not run.

### The 25-way race, re-recorded

The `participation` campaign's C2 race sends 25 simultaneous creates by one creator for one key. Each `Unavailable` it meets is resent with the same key, up to 400 times. A waiter past the 5 s `ACQUIRE` deadline answers `Deadline`, which the race does not resend, so that would fail the campaign.

| | B1, at `0f557305`, on the 8-caller pool | Integrated, at `b848b08f`, on main's 32-caller pool |
| --- | --- | --- |
| Admission refusals (503) resent in the 25-way race | 664 | **0** |
| Race outcome | 21 commits (versions 1..21) and 4 × 422 `unchanged`, one entry | 19 commits (versions 1..19) and 6 × 422 `unchanged`, one entry, no other requested id created |
| Admission refusals at scale (25 members × 50 items) | 34,793 | **0** |
| Scale wall time | 131.8 s | 32.3 s |
| Key-match statement p50 / p95 / max (budget 2 s) | 5.9 / 13.0 / 112.4 ms | 4.1 / 6.3 / 17.3 ms |

No submission failed for good. All 25 settled as committed or `unchanged` without one admission refusal, and no `Deadline` occurred, so none would have shown a member B1's "could not be confirmed; submitting again is safe" note. The C3 close race also held: across 6 rounds, the 26 accepted answers all committed before the close, and the 22 others were 422 `requirement_not_met` with nothing written. Report: [participation-campaign-2026-11-14.json](participation-campaign-2026-11-14.json).

## Limitations

- These are local results on a loaded shared host with Docker PostgreSQL. They are not CI and not beta. The builder did not observe this branch's GitHub CI.
- The race was run once. The count of 0 shows that 25 callers fit inside one role's 32 admissions. It does not bound a class larger than 32 submitting in one instant, or two roles' traffic together.
- PR #125 (release B's scan disposition) is not in this tree.
- APP-207 (composer) and APP-208 (multi-select) are not merged yet. Tier A reviews for APP-203, APP-204 and APP-206, and the APP-202 review, remain open.
