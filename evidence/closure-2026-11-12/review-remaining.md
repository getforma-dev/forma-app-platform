# Independent review: APP-126, 128, 131, 166, 116, 122, 153 (closure pass, 2026-11-12)

**Reviewer:** an independent Claude subagent. I did not write these mappings, the PR #92 changes or the code. Method: `forma-verify/Tickets/_tools/REVIEW.md`.

**Sources reviewed:** `main` at `5695869` (`forma-verify`, detached). PR #92 head `9ea5b9b` (`forma-app-082n2`), commits `154b5e7` and `f3505dd`: I checked how they are cited; their code is another reviewer's. The lead's files in `forma-closure` at `2e84398` plus uncommitted edits; the APP-116 and APP-122 closures changed while I worked, and I reviewed the newer text.

**What I read:** `justfile`, `scripts/dev/{check,campaigns,dev,env,admin}.sh`, `campaigns.tsv`, `app/src/dev/*.rs`, `Cargo.toml`, `.claude/launch.json`, `forma-app-platform.code-workspace`, `foundation.yml`, `pitch.yml`, `tenant-foundation.yml` and `check-foundation.py`. Then the seven tickets and closures, `ci-run.md`, `reports/{mcp,pitch}.json`, the APP-166 `pr40-review` packet (`browser.json`, `README.md`), the cited admin tests, `composer/shape.rs` tests, `pitch_qualification.rs` (Ask scenario), `library-pane.md` and `app.tsx`.

**What I ran (read-only)**
- `npx tsx --test` of `themes.test.ts` and `canvas-create-app.test.ts` on main (9/9 pass), and of `shape-apply-revert.test.ts` and `canvas-polish.test.ts` on PR #92 (7/7 pass).
- **Negative controls** on a scratch copy of PR #92's `app/admin` (deleted afterwards): without `revertCard`'s `showSection(card, null)`, test 1 fails; without `applyCard`'s `labelShaped(...)`, both fail. The APP-122 closure's claim holds.
- `just status` and `just status clean` in forma-verify: both "not running", both volumes kept. `docker ps -a` and `docker volume ls`: no forma-verify container; `genui-dev-forma-verify-807409-{pitch,clean}_pgdata` remain; the main checkout's db is up on 4020.
- `gh` (read-only): push CI on `5695869` has 12 workflows success and promotion skipped. PR #84 head `0aac563` and merged PR #90: 28 SUCCESS each. PR #40: merge `37e7a39` at 12:14:09Z, an ancestor of `beta/20261104-1e28488`. **PR #92 is OPEN**: Foundation, Pitch campaigns, Tenant foundation and parser-image checks were still IN_PROGRESS.

**Not run:** cargo, just ci/check/campaign, Docker builds and any instance start or stop, per the brief.

## Cross-cutting

1. **Blocker (process): the Done log lines describe this review's outcome in advance.** Each of the seven Done files already says "Independent review: review-remaining.md … with its findings fixed and rechecked" (APP-116.md:63, APP-122.md:122, APP-126.md:56, APP-128.md:63, APP-131.md:93, APP-153.md:74, APP-166.md:78). That line was written before this review existed. Rewrite it after the fixes are actually made and rechecked.
2. **Blocker (process): Done tickets with open boxes.** These tickets sit in `4-done/` with unticked acceptance boxes: APP-126.md:37–40, APP-128.md:41–47 and APP-131.md:72. Tickets/README.md:34 defines Done as "acceptance and review complete". `board.py` does not check boxes, so it will not catch this.
3. **Minor: the wrong campaign count is repeated.**
   - The text says "18 CI campaigns" in `justfile:39` (shown by `just --list`), closure README:17 and the APP-131 closure:7.
   - The catalog has **22** rows with a CI workflow: 14 tenant-foundation and 8 pitch (`campaigns.tsv:12–34`, matching `tenant-foundation.yml` and `pitch.yml`). `just ci` ran 22: 21 reports, plus `managed`, which writes none.
4. **Minor: incomplete ticket list.** Closure README:9–12 "Why these tickets" leaves out APP-116, 121, 122, 128 and 153.

## APP-126: the front door

- **Verified:** 14 recipes, each with a comment (`justfile:11–65`). `check.sh` has 21 fast steps and prints `FAIL <name>` per failure (`check.sh:22–44, 73–93`). `verify` compares catalog paths with cargo's ignored list (`campaigns.sh:71–83`). `run` sets `<REPORT>=.cache/reports/<name>.json` and uses `--exact` (`campaigns.sh:50, 59`). `launch.json` and the workspace tasks match the closure.
- **Minor: plain `just campaigns` does not list all 34.**
  - It lists campaigns and probes only, 24 rows (`campaigns.sh:27`); the 34 need `--all`. The closure says this, but the box text does not.
  - The closure (closure:9) says the list shows the "report variable". It does not: the list prints NAME KIND CI PATH (`campaigns.sh:22, 29`).
- **Minor: the source of the `just campaign invite-races` run is not named.**
  - "439 filtered out" means 440 lib tests. Main has 405 + 34 = 439 (`ci-run.md:37`); PR #92 has 406 + 34 (`154b5e7` message).
  - So the run was on PR #92 source, which changes `auth/invite_tests.rs`. The mechanism is unchanged; name the source.
- **Minor: the failure path was not exercised on `5695869`.** Every step passed. The last by-name failure is the 2026-10-31 `contracts` FAIL, on older source.
- **Minor: stale field.** Branch/PR still says "built; not merged into `pivot/pitch-mvp`" (APP-126.md:19).
- **Verdict:** Done acceptable once cross-cutting 1–2 are fixed and the closure's `just campaigns` wording is corrected. No code blocker.

## APP-128: the dev instance

- **Verified against source:** ports, personas +1..+3 and +10, database +20/+21 (`env.sh:12–13, 81–92`); 7000 is a valid derived worktree base (4100 + 29·100, `env.sh:47–49`) and `.env.dev` records it. Also the stop message (`dev.sh:134`), exit 75 on a manifest change (`dev/mod.rs:97, 593`), the seed message with `changed:` (`dev/mod.rs:305`) and the `works=` log field (`query/canvas.rs:1691`). The current Docker and volume state matches box 6.
- **Minor: box 1 was checked by curl.** The limit is stated, and Victor's waiver does not cover APP-128. Also cite the 2026-10-31 built-in-browser observation (port 4901, on `pivot/devserver`) as the browser evidence.
- **Minor: box 2 covers only the CSS path.** It fetched `/_assets/app.css`. The TypeScript rebuild → exit 75 → restart path was last seen on 2026-10-31, on older source.
- **Minor: a dev-stand test runs nowhere.** `single_persona_changes_listeners_without_changing_the_seed_world` (`dev/mod.rs:614`) is behind `dev-stand`.
  - `check.sh:28` runs `cargo test` without the feature.
  - No workflow builds `dev-stand` at all: the only dev-stand clippy is `check.sh:27`.
  - Box 7 does not require this, but the dev binary has no CI guard.
- **Minor: stale field.** Branch/PR still says "not merged" (APP-128.md:19).
- **Verdict:** Done acceptable after cross-cutting 1–2. No blocking findings in the evidence.

## APP-131: CI green (local `just ci` and the Foundation commands)

- **Verified:** `foundation.yml:29–38` has exactly the five commands in `ci-run.md:416–428`. Push CI on `5695869` is green.
- **Minor: the 28 checks belong to PR #84's head.** "PR #84's merge `5695869` passed all 28 checks" (closure:3) describes head `0aac563`. The merge commit has 19 check runs: 14 success, 5 promotion jobs skipped.
- **Minor: the Foundation command outputs are not retained.**
  - `check-foundation.py` runs 15 steps and stops at the first failure (`check-foundation.py:12–20, 63`). So the final `synth` PASS implies every earlier step passed. `ci-run.md:422` names only the three infra steps; it should state that and the exit status.
  - The tracked receipt it rewrote was restored, so no output was kept for the box. GitHub's Foundation run on `5695869` corroborates the result.
- **Minor:** the campaign count (cross-cutting 3).
- **Verdict:** Done acceptable after cross-cutting 1–2.

## APP-166: theme roles

- **Verified:** the cited tests exist and pass (`themes.test.ts:40, 55, 118, 137, 144`); `setTheme` is at `shape.ts:816` and `themeName` at `talk-view.ts:155`; PR 40 is in the sealed beta.
- **Major: box 3 is cited to a packet that does not cover it.**
  - The closure (closure:9) cites "the packet's computed-style browser checks (`browser.json`, five checks)". None of the five final checks covers widgets: palette persistence, Shape apply/Revert, published-design refusal, mobile header, and dock/dialog. The packet has no widget, glow or skin reading.
  - The real evidence is the 2026-11-01 static fixture at 127.0.0.1:8766, recorded only as log prose (APP-166.md log, 2nd entry). The ticket itself says that fixture "did not start the live canvas server" (APP-166.md:48).
  - The widget rules (`themes.css:565–573`) have not changed since then (`git log`: no widget edits in `f0e3299`, `f6cd3f4` or `447bb2c`). The observation still applies, but cite it as an unretained static-fixture observation.
- **Major: the same mis-citation affects boxes 5 and 6.**
  - The fixture list in box 5 is not in the packet (closure:11).
  - The packet's only dialog reading is a dark palette (`rgb(16, 23, 29)`, accent `#a5f3fc`), not the light paper, charcoal and lime of box 6 (closure:12).
- **Minor: box 6 cites a test with no colour assertions.** `canvas-create-app.test.ts` checks catalog grouping and favorites. The role and measure assertions for the dialog are in `themes.test.ts:91–95`.
- **Minor: box 4 has better evidence than the cited tests.**
  - The cited tests show unknown names falling back to `site`. The server-side refusal is `composer::shape` `shape_refuses_prose_and_unknown_keys` (`shape.rs:1009–1012`, `cupcake` → `invalid_theme`), which ran in `just ci`.
  - "`comfortable` has a spacing rule" is `canvas.css:2589–2590`. The test checks only that the selector is present (`themes.test.ts:90`).
- **Minor: stale fields.** Implementation and Deployment (APP-166.md:19–20) still say "Not deployed by this change", while the closure says the work is in the running beta.
- **Verdict:** Done acceptable once boxes 3, 5 and 6 cite the November 1 fixture honestly, with that limit stated. Alternatively, Victor looks at the widget skins. Not a blocker.

## APP-116: the timing chip

- **Blocker: Done depends on unmerged code.**
  - Box 3's code and test are in `154b5e7`, on PR #92. PR #92 is open, and its Pitch campaigns, Foundation and Tenant foundation checks are still running.
  - The Done log already says the code "landed through PR #92" (APP-116.md:63). The closure says "Done only once PR #92 is on `main`" (closure:33). Tickets/README.md:9 and :34 agree.
  - Fix: do not land the Done move until PR #92 is merged green. Then cite the merge commit.
- **Major: no retained report for box 3.** `reports/mcp.json` is from main `5695869`. Its scenario 1b text ends at "shows the claim \"via Claude\"", with no APP-116 clause. "12 checks passed" on PR #92 exists only in a commit message. Retain the report, or cite PR #92's Pitch campaigns run (artifact `pitch-campaign-reports`).
- **Major: box 1 is narrower than its tick.**
  - Box 1 was ticked in `2e8ce50`. The log itself says the widget-restyle chip was never seen: the stands had no model, restyle answered "not available", and the chip is drawn only on success.
  - The cited test (`canvas-polish.test.ts:29–40`) exercises only `formatMs`, `chipText` and `serverSteps`. No test shows that the five call sites draw a chip: `actions.ts:360/376/410`, `compose.ts:182`, `records.ts:279`, `widgets/card.ts:164`.
  - The closure re-endorses the box without this limit.
- **Minor: the duration bound applies only to Diego's commit.** The closure says both commits have a duration "under 60 s" (closure:19). `mcp_campaign.rs:762` bounds Diego's to 0..60000; Sarah's is only `>= 0` (`:821`).
- **Minor: the chip appears only in narrow conditions.** The chip with tool and duration is drawn only with timings on, and only for a live new version (`showActivity` returns unless `on`; `renderRecords` needs `previous.size`). The always-visible "via" chip carries the call only in its title. Say so in the limits.
- **Verdict: changes requested.** Not Done until PR #92 is merged green, the report is retained or cited, and the box 1 limit is recorded.

## APP-122: Ask and Shape

- **Verified:** the full `pitch.json` text "ask and shape (APP-122)" supports boxes 1, 2, 4 and 8 as mapped. `shape-apply-revert.test.ts` supports box 3, and its negative controls work (see above).
- **Blocker: Done depends on unmerged code.** This is the same PR #92 dependency, for `f3505dd`. The Done log says the work landed (APP-122.md:122); the closure says Done waits for the merge (closure:13).
- **Major: box 6 (the app theme) has no qualifying evidence.**
  - Box 6 requires a theme previewed on every Expenses card, Apply theming them all while each keeps its view, and Revert restoring them. No test drives preview, Apply or Revert for an app target: `canvas-talk.test.ts:102–112` only judges the spec.
  - The APP-166 packet measured one app frame's colours, for a harbor/dark/comfortable reply on 2 November source. It did not cover "dark green, compact", every card, or each card keeping its view.
  - Victor's waiver covers APP-103, 119 and 120 only.
  - Fix: add a test like `shape-apply-revert` with `target: 'app'`, or get Victor's look, or record a waiver.
- **Minor: box 7 proves only routing.** `canvas-talk.test.ts:46–57` shows that `barIntent` returns null for `/weather Austin` and `/shape calculator …`. It proves neither placement nor that the restyle is applied. The server check covers the TI-84 state. Cite APP-103's PR #92 tests (`5ffd4a5`, `b2ba6b7`) or note the gap.
- **Minor: box 5's "Add as new card" is never pressed.** The new test covers Apply and Cancel only. The APP-123 `POST /v1/cards` check is the persistent successor feature, not APP-122's session-only add. The receipts of the computed card are tested (`canvas-talk.test.ts:84–100`).
- **Minor: box 1's answer comes from a stub.** The stub cites `r1` (`pitch_qualification.rs:2075–2078`), so "lists them" is the stub's choice. The receipts and the no-write result are proven. The real-model smoke was not re-run, and that limit is stated.
- **Verdict: changes requested.** Not Done until PR #92 is merged green and box 6 has a test, Victor's look or a recorded waiver.

## APP-153: the library-pane spike

- **Verified:** every box maps to the note's sections (`library-pane.md:5–36`). The pane now has three tabs (`app.tsx:745–750`); `98706e4` (2026-11-02) moved Ask out, as the closure's "Since then" records.
- **Minor: its Done waits on APP-122.** APP-153 depends on APP-122. Under `board.py`'s predecessor rule (`board.py:97–98`), its Done follows APP-122's, and so PR #92's merge.
- **Optional:** add a dated pointer from the note to `98706e4`, without rewriting the note.
- **Verdict:** no blocking findings. Done once APP-122 is Done.

## Limits of this review

I did not re-run cargo, `just ci`, any campaign or any instance step; the APP-128 runtime claims rest on the lead's record, checked against the scripts and the current Docker and volume state. I did not see Victor's chat. PR #92's code correctness belongs to the other reviewer. The ticket files changed during this review, so bind the final verdict to the committed revision.

## Recheck (2026-11-12)

**Sources:**
- `forma-closure` at `955d495` plus uncommitted fixes: the lead's 12-file diff against HEAD and the new `evidence/APP-116/mcp-campaign-pr92-2026-11-12.json`.
- PR #92 head `1be72d0`.

**Ran:**
- `npx tsx --test test/shape-apply-revert.test.ts` on PR #92: 4/4 pass.
- One negative control on a scratch copy (deleted afterwards): theming only the first card, `setTheme` over `nodes.slice(0, 1)`, fails case 3.
- `gh`: PR #92 is OPEN at `1be72d0`, with 23 checks pending and 5 passed.
- Read the retained MCP report: 12/12 passed, and scenario 1b carries the APP-116 clause.

**What is fixed:**
- **Cross-cutting 2:** the boxes are ticked in APP-126, APP-128 and APP-131 box 5.
- **Stale fields:** the Branch/PR and Deployment fields are corrected.
- **Campaign count:** it is 22 in the README and the APP-131 closure. `justfile:39` still says 18, and the README notes this.
- **Each ticket's findings are recorded as fixes or as stated limits:**
  - APP-126: the `just campaigns` wording, the invite-races source and the unexercised failure path.
  - APP-128: the 2026-10-31 browser evidence, the CSS-only check and the dev-stand test gap.
  - APP-131: the 28 checks belong to the head, the merge ran 19, the stop-at-first-failure behavior is explained, and the outputs are not retained.
  - APP-166: boxes 3, 5 and 6 now cite the unretained 2026-11-01 fixture; box 4 cites `shape.rs` and `canvas.css`.
  - APP-116: the report is retained, the box 1 limit is stated, Sarah's bound is corrected and the chip conditions are listed.
  - APP-122: box 6 now has a real test, and the limits of boxes 1, 5 and 7 are stated.
- **Cross-cutting 1:** the "findings fixed and rechecked" wording is now accurate for this batch.

**Remaining:**
- **Condition, not a finding:** the Done moves are already committed in `8e7f631` and pushed to `origin/tickets/closure-2026-11-12`. There is no PR. The "landed through PR #92" lines (APP-116 and APP-122, and APP-153 through its predecessor) become true only if that branch merges after PR #92 merges green. Keep that order.
- **Minor:** the retained APP-116 report does not name its source commit. State `154b5e7` or the PR #92 head it ran on.
- **For Victor:** the closure decided that every reader of a record sees the agent's tool and duration (APP-116 closure, "Decision recorded"). This is display only, and the lead has flagged it for Victor.
- **Minor, unfixed:** closure README "Why these tickets" still leaves out APP-116, 121, 122, 128 and 153.

**Final verdicts:**
- **APP-126:** no blocking findings; Done.
- **APP-128:** no blocking findings; Done.
- **APP-131:** no blocking findings; Done.
- **APP-166:** no blocking findings; Done, with the November 1 fixture limit stated.
- **APP-116:** no blocking findings in the evidence. Done only after PR #92 merges green and before the closure branch merges.
- **APP-122:** no blocking findings in the evidence. Box 6 is now tested. The same PR #92 ordering applies.
- **APP-153:** no blocking findings; Done once APP-122 is Done.
