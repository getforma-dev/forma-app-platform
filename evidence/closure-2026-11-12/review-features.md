# Independent review: APP-101, APP-102, APP-103, APP-108 and the test-gap commits

- **Reviewer:** an independent Claude subagent (Opus 5.5). It did not write the closure mappings or the test-gap commits.
- **Date:** 2026-11-12.
- **Reviewed source:**
  - code on `main` at `5695869` (the `forma-verify` checkout);
  - the closure files at `2e84398` (`forma-closure`, branch `tickets/closure-2026-11-12`);
  - `tests/closure-gaps` at `b2ba6b7` (`forma-app-082n2`: `ea1ae1e`, `8e91665`, `5ffd4a5`, `b2ba6b7`).
- **Method:** [REVIEW.md](../../Tickets/_tools/REVIEW.md). File:line references are to `main`, except `box_campaign.rs`, `slack/tests.rs` and `widgets-calculator-clock.test.ts`, which are on the branch.

## What I read and ran

- **Read:**
  - the four tickets;
  - their closure files, the pass README and `ci-run.md`;
  - the full check text in `reports/pitch.json`, `actions.json` and `slack.json`, and in `APP-108/box-campaign-2026-11-12.json`;
  - the cited tests and the code under them: `composer/{matcher,transform,lineage,commands}.rs`, `storage/pitch_qualification.rs`, `storage/actions_campaign.rs`, `actions/{receipt,receipt_tests,ledger}.rs`, `widgets/tests.rs`, `boxes/{intent,routes}.rs`, `mcp/tools.rs`, `slack/flows.rs`, and in `app/admin/src/canvas/` `slash.ts`, `library.ts`, `board.ts`, `ledger.ts`, `boxes.ts`, `box-api.ts` and `widgets/*`, plus `canvas-commands.test.ts`.
- **Ran in `forma-app-082n2`:**
  - `npx tsx --test test/widgets-calculator-clock.test.ts`, from `app/admin`: 3 passed;
  - `cargo test --locked -p forma-app-platform --lib slack::tests::`: 10 passed.
- **Mutation checks:** run on a copy of `app/admin/src` in my scratch directory; no worktree was edited.
  - A tokenizer that skips unknown characters fails the parser test at `alert(1)`.
  - A clock that ignores the zone (`TZ=UTC`) fails the Tokyo test.
- **Not run:** no Docker campaign, `just` or build in either checkout, and no browser. The box campaign (18 checks) and the invite-races campaign (12 checks) are taken as the lead recorded them.

## APP-101: instant pick and live transform

- **Major: box 7 as written is no longer the behavior on `main`.** The closure gives no supersession note.
  - The box says `/` lists library slugs and named apps, and `/apps` shows the list as a canvas card.
  - Since APP-118/119/120, `/` alone lists verbs and no addresses (`canvas-commands.test.ts:75-82`, which the closure cites as evidence), and `/apps` opens the Library pane (`library.ts:723-733`).
  - `/commands` is "never a canvas card" (`slash.ts:315`).
  - **Fix:** record in the ticket that APP-118/119/120 superseded box 7. The address grammar (`/expense words` posts `/v1/act` pinned, `canvas-commands.test.ts:155-166`) and the server listing (pitch "slash commands") still meet the box's intent.
- **Minor: box 3 cites the wrong campaign check.**
  - Pitch "compose (hostile)" (`pitch_qualification.rs:1280`) is the generation path.
  - Transform refusal is covered only by unit tests (`transform.rs:1129-1252`). The step from refusal to `Kept` in the route is shown only by reading the code, and end to end only for the case with no model key (`pitch_qualification.rs:1283-1294`).
- **Minor: box 4 rests on the October 30 browser pass.**
  - On `main`, the campaign only re-opens the standard group (`pitch_qualification.rs:875-878`).
  - The toggle (`board.ts:1210-1233`) has no test.
  - The October 30 browser pass predates the APP-118/120 canvas rewrite. The closure does not name it as this box's evidence.
- **Minor: box 5 is tested end to end in one order only.**
  - The campaign shares after the adaptation exists.
  - The case where a teammate is already viewing when the reshape lands is tested only in the in-process bookkeeping (`lineage.rs:140-161`).
- **Minor: box 9 has only its negative half tested.** The test shows that a member who cannot open the app does not see the command (`pitch_qualification.rs:1020-1023`). No check shows that a member who can open it sees `/big-claims`.
- **Minor:** `Branch / PR` still says "not merged" (`APP-101.md:19`). The same holds for APP-102, and APP-103 says "Not started". Name the landing PRs before Done.
- **Box 1 holds:** `matcher.rs:633-676` covers all 100 examples and 100 negatives over 20 apps, with a p95 assertion. Boxes 2, 6 and 8 hold on the pitch checks quoted.

## APP-102: receipt upload and the expense ledger

- **Major: box 4's range total is not evidenced on `main`, and the closure overstates it.**
  - The campaign sums the listing itself (`in_october`) and only stores it as a timing (`actions_campaign.rs:1435-1491`). No ledger total for a range is ever compared with it. "The Oct 1–30 range sums to 5400 cents" is the test's own arithmetic.
  - Range and status totals are computed in the client `totals()` (`ledger.ts:93-123`), and nothing tests it.
  - The live re-read (`board.ts:796`) was checked only in the October 30 browser pass. `ledger.ts` changed again on November 2 (`5ed9333`).
  - The ticket log itself says "the ledger's client arithmetic has no TS unit runner". One exists now.
  - **Fix:** add a `tsx` test of `totals()` with a range and a status. At minimum, cite the October 30 browser pass as this box's evidence and name the limit.
- **Minor: the "Debug shows no bytes" assertion can never fail.**
  - `receipt_tests.rs:106-109` asserts that `!format!("{parsed:?}").contains("PNG")`.
  - A derived `Debug` of `Vec<u8>` prints `[137, 80, 78, 71, …]`, which never contains "PNG".
  - The real evidence is the hand-written impl (`receipt.rs:51-60`).
  - **Fix:** assert equality with `Receipt(image/png, N bytes)`.
- **The other boxes hold:**
  - Boxes 1–3 match the full `actions.json` check text, plus the October 30 vision smoke (a stated limit).
  - Box 5: reading `receipt.rs` shows no filesystem call, and its only log line carries no image data.

## APP-103: utility widgets

- **The branch closes the parser gap correctly** (see the commits below). The closure is right that Done waits for the merge.
- **Major: the screenshots the Verification asks for were never kept.**
  - Verification asks for "a canvas screenshot of each skin".
  - The October 30 screenshots were "not committed" (`APP-103.md:69`), and `main` has no `evidence/APP-103/`.
  - The closure's limit ("No browser screenshot … in this pass") suggests that earlier ones are on file.
  - **Fix:** Victor's look at the six skins and the three commands, or a recorded waiver.
- **Minor: the cited command test registers stand-ins, not the real commands.**
  - Box 1 cites `canvas-commands.test.ts` as "registers `/weather`, `/calculator` and `/clock`". It registers its own spies (`canvas-commands.test.ts:56`), not `WIDGET_COMMANDS`.
  - The real wiring (`app.tsx:905-909`) is untested. Its only browser evidence predates APP-118/120.
- **Minor: box 2 asks for the request round trip (≤ 1.5 s).** The smoke measured only the model step (514–704 ms).
- **Boxes 3 and 4 hold:** `widgets/tests.rs:62-292`, plus the new parser test.

## APP-108: folders

- **Major: the MCP negative control cannot detect what it claims.**
  - `box_campaign.rs:1009` passes Maya's folder UUID as `folder`.
  - `mcp/tools.rs:1384-1388` turns any non-number into a name, and `intent.rs:222-258` matches names only among the caller's own numbered folders. A UUID never matches, so the control would pass even if folder listing leaked between members.
  - `:1016` asserts only `isError`, not `not_found`.
  - **Fix:** name the folder `"Q3 board pack"` (and `"1"`) and assert the `not_found` code. That also gives resolution by name, which is the Slack phrasing ("from the Q3 metrics folder"), its first end-to-end run. Today that path is covered only by the `folder_ref` parse test (`intent.rs:278-295`).
- **Major: the closure leaves out limits the ticket log records.**
  - The Summary card's real-click re-walk is "pending" (`APP-108.md:76`).
  - The canvas walk "did not cover the teammate's window", and folder sharing has "no canvas UI" (`:78`).
  - The viewer is `box-viewer.ts`, not the focus modal box 1 names (`:72`).
- **Minor: the closure misdescribes the `/storage 1 …` path.** It says `/storage 1 …` "is the same intent" in `boxes::intent`.
  - On the canvas, `/storage` is the client verb (`boxes.ts:1160-1223`). It resolves the folder in the browser and posts `/v1/boxes/{box}/apps` (`box-api.ts:378-380`), not `/v1/act`.
  - The campaign exercises that route elsewhere, but the client parse of `1 create me an app for metrics` has no test.
- **Minor: the Slack test's not-found assertion can never fail.** In `slack/tests.rs:215`, `!contains("a1") && !contains("Q3")` cannot fail, because the refusal carries no data. **Fix:** assert equality with `REFUSED`.
- **Minor: the box-campaign evidence is hard to bind to a source revision.**
  - `box-campaign-2026-11-12.json` names no source revision, so its tie to `8e91665` rests on the closure text.
  - The box campaign is not in CI (`campaigns.tsv:24`), so CI will never re-run the new MCP check.
  - The pass README says "every check runs on `main` at `5695869`". This campaign did not.
- **Boxes 1–8 hold on the server side:** the full report text matches each mapped row, and the sharing check (the CSV opens, the PDF and the image stay 404, only the used files are listed) does test what it claims.

## The test-gap commits

- **`ea1ae1e` (APP-082 N2): correct.**
  - Whole-table principal and entitlement counts run over the `postgres` superuser fixture (`qualification.rs:438-446`), so RLS hides nothing.
  - They are strictly stronger than the per-subject zero they replace. The slow-create step also gains an unchanged PAR count.
  - No assertion was weakened.
- **`8e91665` (APP-108): correct and passing.**
  - The only production change is `folder_message` becoming `pub(super)`.
  - The positive MCP check is sound: one generation call, a new work, the label, and lineage to the CSV.
  - Its weaknesses are the two findings above.
- **`5ffd4a5` and `b2ba6b7` (APP-103): correct and deterministic.**
  - They use fixed instants and the `en-US` Intl format.
  - The `test/*.test.ts` glob picks the file up.
  - Both catch the failures they claim (mutation checks above).

## Verdicts

- **APP-101:** no blocking findings. Before Done, record in the ticket that APP-118/119/120 superseded box 7.
- **APP-102:** changes requested. Box 4's range total and live update need a `totals()` test, or at least the October 30 browser pass cited as their evidence with the limit named.
- **APP-103:** Done once `tests/closure-gaps` is on `main` and Victor has looked at the skins (or the screenshot requirement is waived).
- **APP-108:** Done once `tests/closure-gaps` is on `main` and the closure lists the ticket log's open limits. Strengthening the MCP control is recommended; it needs a box-campaign re-run.
- **`tests/closure-gaps` at `b2ba6b7`:** fit to merge, with no blocking findings.

## Recheck (2026-11-12)

The same independent reviewer rechecked only the fixes.
- **Code:** PR #92 (`closure/acceptance-gaps`), commit `9ea5b9b`. Commits `f3505dd` and `154b5e7` are another reviewer's and are not covered here.
- **Records:** `git diff 2e84398 8e7f631` for the APP-101, 102, 103 and 108 closure files and their `Tickets/4-done/` files.
- **Ran in `forma-app-082n2` at `9ea5b9b`:**
  - `npx tsx --test test/ledger-totals.test.ts`: 1 passed. With the range end changed from `>` to `>=` (on a scratch copy), it fails.
  - `cargo test --locked -p forma-app-platform --lib actions::receipt_tests`: 5 passed, 1 ignored (the real-model smoke).
  - `gh pr view 92`: **OPEN, not merged**, with CI still running.

**The code fixes are resolved.**
- The MCP teammate control now names `"Q3 board pack"` and `"1"` and asserts `structuredContent.error == "not_found"` (`box_campaign.rs:1003-1025`). If folder listing leaked across members, either name would resolve and the test would fail.
- The receipt `Debug` assertion is now an equality (`receipt_tests.rs:106-110`).
- `ledger-totals.test.ts` checks the inclusive range ends, the status filter, the other collection, per-currency sums, and that people and categories add up. I re-derived the arithmetic by hand; it is correct.
- Minor: the box campaign's capped context grew from 2,879 to 3,548 bytes between the `8e91665` and `9ea5b9b` runs, although the campaign code before that step is unchanged. The caps are asserted on structure, so this is not a failure. The cause is unexplained, and the figure should not be cited as stable.

**The record fixes are resolved:**
- APP-101's box 7 supersession note;
- APP-102's re-citation, including the "own arithmetic" note and the live re-read limit;
- APP-108's four open limits and the corrected `/storage` path;
- APP-103's waiver, which I take on the lead's record because I cannot see the chat.

**New findings in the records:**
- **Blocker: APP-102, APP-103 and APP-108 are Done before their tests have landed.**
  - All three are in `Tickets/4-done/`, and their logs say the evidence "landed through PR #92". PR #92 is open.
  - [README](../../Tickets/README.md):34 defines Done as "landed in the agreed source". Line 9 says work on an open PR is logged "without declaring it Done".
  - Their own closure files say "Done only once PR #92 is on `main`".
  - **Fix:** merge PR #92 before the records branch, or keep the three tickets out of Done until then.
- **Blocker: APP-108 is Done with all nine acceptance boxes unticked** (`Tickets/4-done/APP-108.md:48-56`). `board.py check` counts boxes but does not check ticks (`board.py:37`), so it passes. **Fix:** tick each box against its closure mapping.
- **Minor: the APP-108 closure names the wrong source for its report.**
  - Line 5 says the report came from `tests/closure-gaps` at `8e91665`, "`main` plus test-only commits".
  - The report now carries the check text from `9ea5b9b`. That head also includes `154b5e7`, which changes production code (`actions/routes.rs`, `storage/agents.rs`).
  - **Fix:** name the revision that actually ran.
- **Minor:** the APP-103 closure still names `tests/closure-gaps`. The work now ships as PR #92.
- **Minor, carried over:** the `Branch / PR` field is still stale on all four tickets (line 19).

## Final verdicts (2026-11-12)

- **APP-101:** no blocking findings; Done stands.
- **APP-102:** the acceptance evidence is now sufficient, but Done is premature until PR #92 is merged to `main`.
- **APP-103:** the acceptance evidence is sufficient with the recorded waiver, but Done is premature until PR #92 is merged to `main`.
- **APP-108:** changes requested on the record: tick the nine boxes and land PR #92 first. The evidence itself is sufficient.
- **`9ea5b9b`:** fit to merge, with no blocking findings.
