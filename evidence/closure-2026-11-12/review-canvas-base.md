# Independent review: canvas base batch (APP-092, APP-093, APP-094, APP-095, APP-145)

- **Reviewer:** an independent Claude subagent (Opus 5.5). It did not build these tickets and did not write the closure mappings.
- **Date:** 2026-11-12.
- **Reviewed source:** `main` at `5695869` (clean worktree `forma-verify`) and the lead's closure commit `2e84398` on `tickets/closure-2026-11-12`.
- **Method:** followed `Tickets/_tools/REVIEW.md`. For each ticket I read the acceptance and Verification sections, the closure file, the full check text in `reports/*.json`, and the source of every cited test or campaign. I judged whether each check actually asserts its box.

## What I read and ran

- **Read:**
  - the tickets and closure files;
  - `ci-run.md` and the reports `canvas`, `composer`, `sharing`, `pitch`, `query` and `command`;
  - `app/src/query/canvas.rs` and `canvas_tests.rs`;
  - `app/src/storage/{canvas_campaign,composer_qualification,sharing_qualification,pitch_qualification,query_qualification}.rs`;
  - `app/src/storage/composer.rs` (`choose_authoring`) and `app/migrations/0008_sharing.sql` (`share_work`);
  - `app/admin/src/canvas/{compose,queries,render}.ts` and the admin tests `work-render`, `canvas-cards`, `command-outcome` and `canvas-create-app`;
  - `infra/test/security.test.ts`, `app/src/main.rs` (routes) and `Tickets/0-backlog/APP-097.md`.
- **Ran on `main`:** `npx tsx --test app/admin/test/{work-render,canvas-cards,command-outcome,canvas-create-app}.test.ts`, which gave 25 pass and 0 fail. I also sent unauthenticated `curl` GETs to the dev server on port 4001. That port is a persona listener that injects a session cookie, so those GETs proved nothing about anonymous access, and I used them only to confirm the asset path.
- **Not run:** cargo, just, Docker and the campaigns, as briefed. I rely on the lead's `just ci` reports, and every check in them reads `passed: true`.
- **Overall:** every quoted check text matches its report. Unless a finding says otherwise, the campaign source asserts what the check text says.

## APP-092: Canvas in the signed-in workspace

- **MAJOR — a false claim on box 3.**
  - `evidence/APP-092/closure-2026-11-12.md:9` says "A pointer to both measurements was added to APP-097's log."
  - `2e84398` does not touch `Tickets/0-backlog/APP-097.md`, and its log (lines 54–61) has no 2026-11-12 entry and no mention of APP-092.
  - The scope ledger `APP-110` has no island-size item either.
  - The only record of the measurements is APP-092's own 2026-10-30 log line.
  - **Fix:** add the APP-097 log line, or strike the claim. Then record the 2026-11-12 re-measurement in APP-092's log as well.
- **MINOR — the re-measured bundle cannot be reproduced now.** The `forma-verify` `app/admin/dist` was rebuilt by the dev watcher after `just ci`. It now holds a 4.39 MB development build, `canvas.L7PJXLHV.js`. `canvas.c64fd907.js` (655,105 B, 212,760 B with gzip -9) can no longer be checked there. Say so in the closure's limits.
- **MINOR — box 1, "no canvas assets".**
  - `/_assets/{*filename}` is served without a session (`app/src/main.rs:329`), so an anonymous caller can fetch the hashed bundle.
  - The box holds only in the closure's sense: the 404 references no asset, and the bundle holds code but no tenant data.
  - **Fix:** state this as a limit.
- **MINOR — box 1, the "pre-access" case.** In `canvas_tests.rs:87-108` that case is a forged `__Host-genui_pre` cookie sent to an `Auth` with no sessions. A real pre-access session gets the uniform 404 only on the query routes (`query` check 18). `/canvas` reaches the same result through the shared `routes::member` (`canvas.rs:1671`).
- **Supported:**
  - box 1 by `canvas` checks 1 and 5 (`canvas_campaign.rs:358-414` and `586-639`), with a dispatch counter and the constant-body 404 predicate;
  - box 2 by `canvas.rs:1675`, where the organization comes from `member.organization()`, and the `?intent=` test;
  - box 4 by `canvas_tests.rs:151` (typed `Outcome::Released` requires a receipt) and the APP-094 evidence.
- **Verdict: changes requested.** Correct the false APP-097 claim (or add the pointer) and add the two limits above.

## APP-093: Composer

- **MAJOR — box 4: both cited checks are non-discriminating.**
  - `composer` check 1 asserts only that the foreign context excludes `SPEND_APP` (`composer_qualification.rs:221-230`). That app is not offered to `PRINCIPAL` even inside its own organization: the member's own library is `len 1 == APP` at line 210. The foreign `len == 1` entry is not identity-checked.
  - In the `query` "APP-093 m5" check, the canary's empty library is asserted before any library grant exists (`query_qualification.rs:2203-2219`), when Maya's library is empty too.
  - Discriminating evidence does exist in the same run: `pitch` check 13. The outsider's library is empty after Maya holds 20 library apps (`pitch_qualification.rs:1454-1460`), and "compose pinned to Northstar's app" is the uniform 404 (`:1502`).
  - **Fix:** re-cite pitch check 13. No new test is needed.
- **MINOR — box 2, malformed output.** The "malformed" half is shown by the unit tests `composer::tests::the_composers_own_codes_cover_prose_invented_inputs_and_unrunnable_views` and `model.rs:585` (`parse_reply("not json")`). It is not shown by `composer` check 8, which is a model failure.
- **MINOR — box 2, the refusal as the user sees it.**
  - The closure cites only the code mapping in `compose.ts:78`.
  - `canvas-create-app.test.ts:85-119` asserts "Nothing was placed" on `could_not_compose`; I ran it and it passed.
  - **Fix:** cite that test.
- **MINOR — box 3.** The scan commands (tracked files and the 504 KB `just ci` output) are not recorded, and the output was not retained. The scan does have a working negative control: the campaigns run with `--nocapture` (`scripts/dev/campaigns.sh:59`), and the pitch campaign logs to stdout with key `sk-ant-synthetic-pitch-campaign-key` (`pitch_qualification.rs:458, 738-753`).
- **Supported:**
  - box 1 by `composer` checks 3, 4 and 6, `pitch` check 4 (the adapted chips on the group route), `canvas_tests.rs:354` and the server-side rendering test (`main.rs`, "Retrieved from library");
  - box 3 by `security.test.ts:256` (`ValueFrom` the ComposerModel secret).
- **Verdict: changes requested (citations only).** Re-cite box 4 to `pitch` check 13, and record the scan commands.

## APP-094: Governed data on the canvas

- **MINOR — box 1 cites the wrong admin test.**
  - The cited test, "failed and denied views…" (`work-render.test.ts:62`), covers the work-page renderer `src/work/render.ts`, not the canvas card.
  - The canvas gate is `released()` (`app/admin/src/canvas/render.ts:211`, used at `queries.ts:115-117`). A 200 response without a receipt shows "came without its proof" and no number. That is asserted at `canvas-cards.test.ts:397`; I ran it and it passed.
  - **Fix:** re-cite.
- **MINOR — box 3.**
  - The log for 2026-10-30 says the stale-edit and audit checks "run in the APP-013 command campaign, not again through the canvas". The closure should keep that limit.
  - The canvas client's own behavior is tested in `command-outcome.test.ts:6-31`: the same key on retry, recovery by key, and 409 classified as a conflict. Replay through the group's server-minted key is `pitch` check 8. Cite both.
- **Supported:**
  - box 1 by `canvas_tests.rs:151` and `query` checks 7, 8 and 22;
  - box 2 by `query` checks 18 and 21 and `canvas` check 5;
  - box 3 by `command` checks 4, 6 and 17;
  - box 4 by `command` check 20 and `actions` "audit chains verify".
- **Verdict: no blocking findings. Done may be recorded** once the two citations are corrected.

## APP-095: Share an app with a teammate

- **MINOR — a false sentence about the mechanism in box 1.**
  - `closure:13` says "The rows are the teammate's own `genui.work_shares` set, and every read is judged against them."
  - No authorization path reads `genui.work_shares`. In `app/src` only `live_work_shares` (APP-154, `shared_access.rs:262`) is read.
  - Reads are judged against the copied `app_permissions`, `work_permissions` and `source_permissions` rows (`0008_sharing.sql:131-158`), and `work_shares` is the record of each share.
  - **Fix:** reword.
- **MINOR — box 3.**
  - The `live-sharing` citation is APP-154's guest share, not this share.
  - In `sharing` check 11 the "next request" is a store-level `work_outline` call at the new epoch (`sharing_qualification.rs:864-885`), not HTTP.
  - HTTP evidence of a revoked `record_read` on a shared work is `canvas` check 6 (`canvas_campaign.rs:803-816`). Cite it.
- **MINOR — a missing limit.** Revocation is tested only for `record_read`. The copied, organization-wide `source_read` and the app grants survive, as the ticket's Why says, and the closure should say so too.
- **Supported:**
  - box 1 by `sharing` checks 2, 3 and 6;
  - box 2 by `sharing` check 8 (byte-identical bodies for an existing and a nonexistent work, `:643-690`) and `pitch` check 13 under a constant-body 404 predicate;
  - box 4 by construction: `share_work` copies only grants the sharer holds and writes no definition. Checks 2, 4 and 12 confirm it.
- **Verdict: no blocking findings. Done may be recorded** after the wording fix and the limit.

## APP-145: One authoring shell for a new member

- **MINOR — box 2, "not a library app".** In `composer` check 9 that half is vacuous: the bare member's library is empty (`composer_qualification.rs:690`, then `:758`). It is carried by `pitch` check 1 instead, which the closure cites. In that check the same `choose_authoring` (`storage/composer.rs:65`) refuses an owned library app: "owning only a library app: nothing to draft on" (`pitch_qualification.rs:681`).
- **MINOR — box 6, "unchanged by the shell".** This is shown by path identity (the generated origin, the shell app, one publication; `:809-829`). No digest is compared. Name this as a limit.
- **Supported:**
  - boxes 1, 3, 7 and 8 by `composer` checks 9 and 10: the exact permission set `discover`, `edit_definition`, `run`, zero work or source grants, `+1 permission.granted` naming the shell, two real advisory-lock waiters seen in `pg_locks`, and a no-write footprint on listing;
  - box 4 by `composer` check 2 (no model call, no app row, `Refused::NotFound`);
  - box 5 by `composer` check 12;
  - box 9 by `composer` check 11 (entitlement expiry with the lock held: zero app, permission and audit rows);
  - `pitch` check 12 over HTTP.
- **Verdict: no blocking findings. Done may be recorded.**

## Limits of this review

- I did not re-run any Rust test or campaign. The pass results are the lead's `just ci` reports at `5695869`.
- No browser was used. The "visible" parts of APP-092, 093 and 094 rest on the server-side rendering tests, the admin tests and the 2026-10-30 browser walks in the logs.
- I did not check board placement or status moves. That stays with the board's landing rules.

## Recheck (2026-11-12)

- **Bound to:** `8e7f631`, plus the uncommitted tree it describes.
- **Checked:**
  - `git diff 2e84398 8e7f631` for `evidence/APP-09{2,3,4,5}*`, `evidence/APP-145*` and `Tickets/0-backlog/APP-097.md`;
  - the five ticket moves, read with `-M`.
- **Ran:** nothing new. No source changed.

**The earlier findings are resolved.**
- APP-097's log now records both measurements (line 62).
- APP-092 now states the limits on `/_assets/`, the forged pre-access cookie and reproduction.
- APP-093 box 4 now cites `pitch` check 13 and says why the earlier checks were not evidence. It now cites the refusal and malformed-output tests and records the scan commands.
- APP-094 now cites `canvas-cards.test.ts:397`, `command-outcome.test.ts` and `pitch` check 8, and keeps the APP-013 limit.
- APP-095's mechanism sentence is now correct (`0008_sharing.sql:131-158`). Box 3 now cites `canvas` check 6, and the `source_read` limit is added.
- APP-145's two limits are added.
- None of the new citations misstates its source.

**New findings in the moves:**
- **BLOCKER (board record).**
  - `Tickets/4-done/APP-092.md`, `APP-093.md`, `APP-094.md` and `APP-095.md` are Done with all 4 acceptance boxes still `- [ ]`.
  - `Tickets/README.md:24` makes the checklist authoritative, and `:34` defines Done as "acceptance and review complete". Every other Done ticket in this batch is fully ticked; APP-145 has 9 of 9.
  - **Fix:** tick the boxes that are now evidenced, then regenerate the board.
- **MAJOR.** The `Branch / PR` field on those four still reads "not merged; no PR" (APP-092 and APP-094), or names an unmerged `pivot/*` branch (APP-093 and APP-095). That contradicts Done, which means "landed". **Fix:** name the merge on `main` (PR 22–48 per the pass README) or the PR that landed each ticket.
- **MINOR.** `evidence/APP-095/independent-review.md` m6 now cites `Tickets/4-done/APP-095.md:49`, but line 49 is the review log line. The campaign-command line it means is line 50.

**Final verdicts at `8e7f631`:**
- **APP-092:** changes requested. The closure evidence is sufficient. Fix the board blocker and the `Branch / PR` field, then Done may stand.
- **APP-093:** changes requested, for the same two board fixes only.
- **APP-094:** changes requested, for the same two board fixes only.
- **APP-095:** changes requested, for the same two board fixes only. The m6 line number is optional.
- **APP-145:** no blocking findings. Done may be recorded.

## Recheck 2 (2026-11-12)

- **Bound to:** `8e7f631`, plus the uncommitted `git diff HEAD -- Tickets/4-done/APP-09{2,3,4,5}.md`.
- **What I checked:**
  - In all four tickets every acceptance box is now `[x]`: 4 of 4 each, with the box text unchanged. Each ticked box matches evidence accepted above.
  - The `Branch / PR` fields now say the work landed on `main` through `pivot/pitch-mvp`, the trunk since ADR-019. The ADR-019 link resolves, and each field links its closure file.
  - This resolves the blocker and the major from the first recheck.
- **Still open, optional:** the m6 line-number nit is in `evidence/APP-095/independent-review.md`, not in this file. The current task does not allow me to edit that file, so it stays open. It does not block Done.

**Final verdicts:**
- **APP-092:** no blocking findings. Done may be recorded.
- **APP-093:** no blocking findings. Done may be recorded.
- **APP-094:** no blocking findings. Done may be recorded.
- **APP-095:** no blocking findings. Done may be recorded.
- **APP-145:** no blocking findings. Done may be recorded.
- **Condition:** commit this tree as reviewed, regenerate the board, and run `board.py check`. I ran neither.
