# Independent review: app pages, sharing and the app-model spikes

- **Reviewer:** an independent Claude subagent (Opus 5.5), started by the lead. It did not write the closure mappings, the tickets or the code.
- **Date:** 2026-11-12.
- **Reviewed source:** `main` at `5695869` (detached checkout `forma-verify`) and the closure commit `2e84398` on `tickets/closure-2026-11-12`.
- **Batch:** APP-141, APP-142, APP-143, APP-144, APP-154 and the spikes APP-146 to APP-149. Guide: `Tickets/_tools/REVIEW.md`.

## What I read and ran

- **Read:**
  - each ticket and its closure file, plus the pass's `README.md` and `ci-run.md`;
  - the full check text in `reports/{composer,pitch,query,command,canvas,live-sharing,shared-identity,record-components,sharing,schema}.json`;
  - ADR-018, APP-168, APP-176 and `docs/planning/app-model/{app,components,widget,box}.md`;
  - every admin test the closures cite, and the product source behind them;
  - the November 1 evidence the closures lean on: APP-141 completion record, APP-143 README and `browser.json`, APP-144 `browser-results.json`, APP-154 `REVIEW.md`.
- **Ran:**
  - `npx tsx --test` on 13 admin test files on `main` (`canvas-create-app`, `app-view-layout`, `app-view-controller`, `app-chat`, `app-assistant`, `context-ask`, `work-render`, `authority`, `canvas-library`, `presentation`, `canvas-page`, `sharing`, `app-data-actions`): **108 pass, 0 fail**.
  - `gh pr view` for PRs 22, 26, 27, 30, 31, 36 and 41: all MERGED into `pivot/pitch-mvp`. `git merge-base --is-ancestor` shows that merges 0984614, 578f97a, a4fdc1a, 9ba2061 and 6c308f1 are all in `5695869`.
  - `gh pr view 22 --json files`: only `Tickets/`, `docs/planning/` and `evidence/APP-146/` changed.
  - `gh run list --commit 5695869`: every push workflow succeeded, and "Promote to beta" was skipped.
- **Not run:** cargo, just, campaigns, Docker, a browser. The report JSONs carry no commit identity, so their link to `5695869` rests on `ci-run.md`.

## Cross-cutting findings

- **X1 — blocker for APP-142 and APP-144.** ADR-018 (APP-168, Done) removed fan-out for an app's components.
  - On `main`, `breakApart` only returns "Apps stay together. Choose Card, Browse or Open view." (`app/admin/src/canvas/unite.ts:372`).
  - A stack of one app's cards becomes that app's Stack view (`stacks.ts:401-409`, `migrateAppViews`). `Tickets/README.md:18` and APP-168's log record this.
  - Boxes that promise fan-out or legacy stacks are therefore not met on `main`. Ticking them would put false behavior into a Done ticket.
  - **Fix:** a dated log entry, and a note on each affected box: "superseded by ADR-018/APP-168; November 1 evidence is historical". Do not count the box as verified on `main`.
- **X2 — minor.** `docs/README.md:82-87` classifies the spike documents as Reference, but their text is now stale:
  - `components.md:12,23` call record lists and APP-155–158 future work, which is now implemented;
  - `components.md:21` says "Fan out returns the parts to the board";
  - `widget.md:19` says Ask has no widget scope, but `talk.rs:247-271` now parses one;
  - `app.md:32` describes Unite rather than Open.
  - Each box was true when written. The closures' "its later edits keep the statements above" needs a limit and a dated pointer to ADR-018, not a rewrite of the text.
- **X3 — minor.** The commit list in the spike closures (line 15) is wrong. `f732d0b` and `cc23c55` changed only `composed-app.md`. `box.md` and `widget.md` have had no edit since `0a96590`, which the list omits.
- **X4 — minor (attribution).** Some November 1 reviews name only agent roles, not a model or tool:
  - "Independent review by root" and "Peer review of root naming" (`evidence/APP-143/README.md:25`);
  - "root" and "review_page" (`evidence/APP-154/REVIEW.md:4,21`).
  - The closures call these reviews independent without saying so.

## APP-141: create an app before it reaches the canvas

- **Major — box 7 ("creation opens the app as one page") cites a test that never creates an app.**
  - Closure line 15 cites `app-view-controller.test.ts:92`, which only switches modes.
  - The behavior holds by source: `compose.ts:178-181` opens the view through `unite`, which is `setAppView(…,'open')` (`unite.ts:359`).
  - No test on `main` covers it, and the November 1 browser proof predates ADR-018. Cite the source with that limit, or add a test.
- **Minor — box 4 (widgets).** Closure line 12 cites `canvas-library.test.ts:167`, a Library listing, and `widget.md`, a planning document. The claim holds by construction (`canvas/widgets/` makes no compose or placement call). Say "by source inspection".
- **Minor — box 5.** Composer check 13 "discovers" through `composer_context` (`composer_qualification.rs:1078-1093`), not `GET /v1/templates`. That route is static by inspection (`composer/routes.rs:117-130`). The genuine-provider half rests on the APP-139 hosted record, which the closure does not cite.
- **Minor — box 3** has no evidence for the `app.card` addressing.
- **Holds:**
  - Boxes 1 and 2 use the real modal handlers: the template click and Cancel send nothing, Confirm posts `{intent, template_id}`, and a refusal fetches no group (`canvas-create-app.test.ts:85-119`).
  - Box 6: composer checks 14 and 18, with the share half from November 1.
- **Verdict:** changes requested, mapping only. No blocker.

## APP-142: the app is a page

- **Blocker (X1) — box 1, second sentence** (`APP-142.md:57`).
  - "Break-apart is fan-out" is false on `main`.
  - Closure line 9 also misstates the current behavior: "the app goes back to its App Card" is wrong, because break-apart is a message that changes nothing.
- **Blocker (X1) — box 5** (`APP-142.md:61`). Legacy stacks migrate to the app's Stack view and APP-176 is retiring them. Closure line 13 admits this but maps the box as met.
- **Major — box 2's citations do not prove it.**
  - Box 2's "unless the member fans out" is the same X1 problem.
  - `app-view-controller.test.ts:261-312` never closes Focus, and `presentation.test.ts:59` is an action dialog.
  - `closeFocus` returns the card to the page by inspection (`focus.ts:381-405`), but no test covers it.
  - APP-168's November 2 browser check (Focus returns to the same App Card) is the nearest post-ADR-018 evidence, and the closure does not cite it.
- **Minor — box 4 cites the legacy route.**
  - Closure line 12 cites `POST …/share` and the `sharing` campaign.
  - The page's Share (`unite.ts:490`) uses `/v1/work/{app}/{work}/shares` (`sharing/api.ts:23`), covered by `live-sharing` check 14.
  - `canvas-page.test.ts:52` tests `appShareRequest` (`page.ts:34`), which nothing calls. Do not cite it.
- **Holds:** box 3 is covered by `app-view-layout.test.ts:11`, `canvas-page.test.ts:9,17`, the Open-size tests and APP-168's "Only Open resizes".
- **Verdict:** changes requested (blocker X1).

## APP-143: Ask about the thing you clicked

- **Major — box 2's Focus-to-card scope has no test on `main`.**
  - The scope comes from `scopeOfCard` (`ask.ts:122`, wired at `ask.ts:571-581`), and nothing tests it.
  - `app-assistant.test.ts:105` and `app-chat.test.ts:235` only show that a scope already passed in is preserved.
  - The proof that a records part names its collection is the November 1 browser run, before ADR-018 reworked Focus. Record that limit.
- **Minor — box 4's log claim cites the wrong file.**
  - Closure line 10 cites `ask-unit-tests.txt`, whose five tests are about bodies, not logging.
  - The claim holds by inspection: `talk.rs:370-380,413-422,479-490` log only identifiers, counts and timing.
  - The UI half is `context-ask.test.ts:9,33`, which the closure does not cite.
- **Minor:** X4.
- **Holds:** box 1 (`app-chat.test.ts:66` posts exactly one work); box 3 (pitch check 16, `context-ask.test.ts:17`); box 4 receipts (pitch check 17).
- **Verdict:** no blocking findings. Correct the mapping before the move.

## APP-144: open an app as a page

- **Blocker (X1) — box 4 drops a clause silently.** Closure line 10 restates box 4 (`APP-144.md:61`) without "Fan-out still returns the parts to the board". That clause is false on `main`.
- **Major — box 7's three citations do not test it.**
  - Closure line 13 cites query check 20 (record selection), `record-components` (aggregates) and `app-data-actions.test.ts:122` (decimal display). None tests initial enum restrictions.
  - The correct evidence is on `main` and ran in the Rust suite: `commands::tests::declared_form_selects_exact_command_and_unconditional_initial_enum` (`app/src/commands/tests.rs:803`).
- **Minor — box 3.** `command` check 16 is the record page, not `/w`. The document posts through `submitCommand` (`work/document.ts:172-186`). The proof that a refusal stays on the page is only the November 1 `browser-results.json` (`more.refusal.samePage`).
- **Minor — box 4's first half.** The "outline authority" and "reload restores" checks do not show a record crossing between page and board; only November 1 `board.twoWay` does.
- **Minor — box 5's widget half** is unmapped (`recipient-live-2026-11-01/widget-no-document.png`).
- **Holds:**
  - Boxes 1, 2, 5 and 6: query checks 14, 15, 17 and 18; `storage/queries.rs:1872`; `work-render.test.ts:44,77,97`; `live-sharing` checks 6, 9 and 13; `authority.test.ts:28`.
  - The live timings are November 1 local observations, as the ticket says.
- **Verdict:** changes requested (blocker X1, major on box 7).

## APP-154: private live sharing

- **Major — box 7 (`APP-154.md:42`) has no negative control for two conditions.**
  - Organization shutdown failing closed and overlapping independent grants being preserved are exercised by no `live-sharing` check and no November 1 record.
  - Shutdown is enforced only by the SQL predicate `o.status='active'` (`storage/shared_access.rs:260`). Check 11 covers only revoked history that hides an active grant (`shared_campaign.rs:1007-1017`).
  - Add the assertions, or split these conditions out as APP-176 did for APP-169.
- **Minor — box 3 (`APP-154.md:38`).** No cited check covers an expired invitation. Expiry is enforced by `s.expires_at>clock_timestamp()` (`shared_access.rs:264`). Check 11 concerns expired links, not acceptance.
- **Major — box 8's "independent Tier A review" is a cross-review between co-authors.**
  - The roles are "root" and "review_page", with no model named, and each wrote part of the change (`REVIEW.md:19,23`).
  - It is from November 1, before PR 36 and later work on `main`. Closure line 14 should state these limits.
- **Holds:**
  - Boxes 1, 2, 4, 5 and 6 map to real check text: `live-sharing` checks 2, 4–6, 9, 11 and 14, and `shared-identity` checks 3 and 5–7. Synthetic providers are disclosed.
  - Box 9: PR 30 (`9ba2061`) is in `main`. The schema qualifier passed 395 of 395.
- **Verdict:** changes requested (major). No blocker in the source I read.

## The spikes, APP-146 to APP-149

I read each document against its ticket, APP-120 and the cited source. Every cited file and function exists on `main` (`choose_authoring`, `draft_publish_save`, `record_links`, `build_from_box`, `boxes/{intent,ask,mod}.rs`, `storage/{labels,work_labels,boxes}.rs`, migrations 0002 and 0018). PR 22 changed no code.

- **APP-146 (`app.md`).** The four sections are at lines 9-18, 24-26, 32-34 and 38, and claims carry citations. It has no "proposed" sentences, which is fine because every claim is implemented. Minor: X2 (`app.md:32`). **Verdict:** no blocking findings.
- **APP-147 (`components.md`).** Kinds and addresses are at lines 9-15, "same component" at 21 and the open detach questions at 29-34. Minor: the fan-out "no copy" claim is line 21, not line 19 (Unite). Minor: X2. **Verdict:** no blocking findings.
- **APP-148 (`widget.md`).** The kinds and their backing are at lines 7-11, note and folder at 10-11 and Ask at 19. Minor: X2, since line 19 is now false on `main`. **Verdict:** no blocking findings, with a limit to add.
- **APP-149 (`box.md`).** Lines 7, 11, 15 and 19 match the closure. "Keep" is recorded with what would break, and formation goes through admission (`boxes/routes.rs:1304-1309`). **Verdict:** no blocking findings.

## Limits of this review

- **Statuses:** every predecessor of these tickets is still in Backlog on `2e84398`, and the board must confirm them before any move.
- **Browser and hosted claims:** November 1 browser evidence and hosted records were read, not reproduced.
- **Rust and campaigns:** none re-run; I relied on `ci-run.md` and the report JSONs.
- **Scope:** this review checks the mappings and the behavior they claim. It is not a new full Tier A review of APP-154's code.

## Recheck (2026-11-12)

Same reviewer. I checked the fixes with `git diff 2e84398 8e7f631` over the batch's closures, their tickets (now in `4-done/`) and the new `Tickets/0-backlog/APP-178.md`. Each new citation matches the source on `5695869` that I had already read. No code changed. I edited only this file.

- **X1 resolved.** The fan-out and legacy-stack clauses now carry dated notes: "Superseded 2026-11-02 by ADR-018 (APP-168)". This covers APP-142 boxes 1, 2 and 5 (`APP-142.md:57,58,61`) and APP-144 box 4 (`APP-144.md:61`). The closures cite `unite.ts:372` and `stacks.ts:401-409` and do not count these clauses as verified on `main`. The false "goes back to its App Card" wording is gone.
- **X2 and X3 resolved.** Every spike closure now has a dated limit naming its stale lines, and the wrong commit list is gone.
  - Residual minor: the APP-148 limit names ADR-018 as the later decision, but `widget.md:19` went stale because of APP-143.
- **X4 resolved.** The APP-143 and APP-154 closures state the role-only attribution.
- **APP-141 resolved.** Box 7 rests on `compose.ts:178-181` with its limit stated. Box 4 is marked "by source inspection". Box 5 cites `routes.rs:117-130` and the hosted record. Box 3 has a limit.
- **APP-142 resolved.** Box 2 states that no test closes Focus and cites APP-168's November 2 check. Box 4 cites `/shares` and `live-sharing` check 14.
- **APP-143 resolved.** Box 2 states that `scopeOfCard` is untested. Box 4 is re-cited to the `talk.rs` lines and `context-ask.test.ts:9,33`.
- **APP-144 resolved.** Box 7 cites `commands/tests.rs:803`. The refusal and two-way limits and the widget half are stated.
- **APP-154 resolved.**
  - Organization shutdown, overlapping grants and the expired invitation are split into APP-178. Its checks require a mutation that removes each predicate and makes the check fail. The source boxes (`APP-154.md:38,42`) are annotated.
  - The co-author cross-review limit is stated. Each part was reviewed by someone who did not write it, so box 8 stands with that limit.
  - Residual minor: closure line 19 points to this review as "this pass's attributable review". This is a review of the mappings, not a new Tier A code review.
- **Process (minor).**
  - `8e7f631` moved the tickets to Done, with logs saying "findings fixed and rechecked", before this recheck happened. This section is that recheck.
  - `python3 Tickets/_tools/board.py check` on the current working tree stops at `Tickets/4-done/APP-166.md`. Its link to `review-remaining.md` is broken. That ticket is outside this batch, but the check fails until that file lands.

**Final verdicts:**
- **APP-141:** no blocking findings.
- **APP-142:** no blocking findings. The superseded clauses are historical.
- **APP-143:** no blocking findings.
- **APP-144:** no blocking findings. The superseded clause is historical.
- **APP-154:** no blocking findings. The untested clauses are now APP-178.
- **APP-146:** no blocking findings.
- **APP-147:** no blocking findings.
- **APP-148:** no blocking findings.
- **APP-149:** no blocking findings.
