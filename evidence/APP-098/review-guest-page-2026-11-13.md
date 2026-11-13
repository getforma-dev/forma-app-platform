# Independent review: APP-098 DS-84, the guest page "Sample data" note

- **Reviewer and date:** an independent Claude subagent (Opus 5.5), 2026-11-13. I did not write this change.
- **Reviewed revision:** `97a1cc5` on `app098/shared-page-sample-note`, in the `forma-work` worktree (`git show HEAD`, 9 files, +67/−4).
- **Protocol:** I read the code and ran unit tests and the admin typecheck. I used no browser, no AWS, no dev stand and no campaigns. I edited nothing except this file.
- **Verdict: approve with changes.** The authorization and the client masking are sound, and the tests pass. Fix M1 in the ticket. m1 to m3 are recommended.

## Findings

- **M1 (major, ticket text).** `Tickets/2-in-progress/APP-098.md:218` says the connected canvas `/shared` "lists titles and links only, so it has no data to label". That is true of its server props (`sharing/live.rs:485-492`, empty `parts`), but not of the page.
  - The client fetches each connected app's guest JSON and renders its views and records into the card (`admin/src/shared/canvas-controller.ts:74`, `:79` `replaceSharedParts`).
  - That JSON now carries `sample_data: true` for a sample work, and the card ignores it. So a guest sees the same sample charts unlabelled on `/shared`.
  - Box 5 (`APP-098.md:247`) also strikes "and the live-shared pages" in full.
  - Fix: list `/shared` under "Not covered" and in box 5's remaining gaps. Or label the card in `refreshData` from `props.sample_data`, and hide the label in `mask`.
- **m1 (minor, wording, for Victor's QA).** The guest page can show the owner's filed records. `record_cards` come from `records_many` (`storage/shared_reads.rs:68`) and render as tables (`admin/src/shared/parts.tsx:39-51`), and record components are shown too.
  - So "Charts and tables here read Forma's synthetic sample data." (`admin/src/shared/app.tsx:23`) also covers real records. The reason given at `APP-098.md:216` ("guests file no records, so it has no second sentence") is true, but beside the point: a guest still sees records that someone filed.
  - The comment at `app.tsx:22` ("never the owner's own") has the same problem.
  - The data-copy over-label from the earlier review's m1 also applies here.
  - Suggested wording: "Charts here read Forma's synthetic sample data. Records shown were filed by the app's members."
- **m2 (minor, client).** The note is rendered only when the server props have `sample_data` set (`app.tsx:23`). A pending page has no note element.
  - If the invitation is accepted in another tab, the 3-second refresh turns the page available in place (`document.ts:89`, then `:92`). `:98` then finds no `[data-sample-data]`, so sample charts appear without the note until a reload.
  - The owner line (`app.tsx:20`) has the same existing gap.
  - Fix (optional): create the note on demand, as `:101-105` does for the add button.
- **m3 (minor, tests).** `sharing.test.ts` covers only the pure `sampleNoteHidden`. No test drives the wiring in `startSharedPage`: hidden by `unavailable` (`document.ts:66`) and shown after an available refresh (`:98`). `test/connected-authority.test.ts` already drives a page controller and could host such a test.

## Answers to the four questions

1. **Authorization and isolation: sound.**
   - `guest_transaction` (`storage/shared_access.rs:295-310`) opens `begin_shared` on the Data pool (`genui_data`). It sets `genui.organization_id` to the share's organization and runs `check_guest` before any read.
   - `genui.work_bindings` has SELECT granted to `genui_data` (`migrations/0002_tenant_foundation.sql:232`). It has one permissive policy, `tenant_scope` (:219), which allows the current organization only, and no later migration widens it.
   - The keys are `access.app`/`access.work`, which `check_guest` binds to the share row (`s.app_id=$5`, `s.work_id=$6`). The query (`storage/queries.rs:1708`) also filters on `organization_id` and only on catalog source ids, which are a public constant.
   - So the result is one boolean about the work the guest is already shown. `guest_work_reads` also requires all of the work's views and collections to be in the share (`shared_reads.rs:58`, :74-79), so there is no subset to leak past.
   - **Ordering.** The new query runs after the opening `check_guest` and before the closing one (`shared_reads.rs:92`). It is read-only, and any error, its own or the closing check's, returns `Err` through `tx.finish(false)` (:99). Nothing reaches props on failure.
   - **Which pages can carry it.** Only `guest_document` builds an available `SharedPageProps` (`sharing/live.rs:723-746`). The neutral page uses `..Default::default()` (:583-606), and the pending page is `..neutral` (:618). A revoked, expired or wrong-recipient share fails `resolve_guest_access` and gets the neutral page, or a 404 for JSON. None of these can carry `sample_data: true`.
2. **The client: correct.** On load, `unavailable` hides the note (`document.ts:121` → :66). A successful refresh restores it from the JSON after `replaceSharedParts` (:98), on the same authority-version guard as the parts (:90).
   - Revoke (:84-85), errors (:114), offline (:75), pagehide (:147), visibilitychange (:145), the bfcache pageshow (:148), monitor masks (:123) and record-component loss (:118) all go through `unavailable`.
   - `.sample-note` has no `display` rule, so `hidden` takes effect.
   - **JavaScript off:** acceptable. The server shows the note with the parts from the same authorized read, so the label matches what is shown and adds nothing beyond that.
3. **Contract: nothing else pins the shape.** `sharedJson` (`document.ts:21-30`) checks named fields only, and `sample_data` is optional.
   - `SharedAppProps` and `connectedEntries` are unchanged.
   - The `GENUI_SHARED_ENTRY_FIXTURE` output (`main.rs:647`) covers only neutral pages, and no script compares it.
   - No file outside `app/src` and `app/admin/src` names these props. The `bind::<SharedPageProps>("shared")` sites (`main.rs`, `dev/mod.rs:451`) pass against the built dist.
4. **The ticket: accurate apart from M1 and m1.**
   - The role and policy description (:216) and the line `shared_campaign.rs:723` (the new assert) are accurate.
   - "5 passed" matches my run.
   - The campaign does run the real query: the catalog is `synthetic: true` and the keys are non-empty, so `sample_works` issues the SQL. A missing grant would refuse the page.
   - I did not reproduce the red runs or `just campaign live-sharing`.

## What I ran

| Command | Result |
| --- | --- |
| `cargo test --locked -p forma-app-platform --bin forma-app-platform` | 8 passed, 0 failed (includes `work_page_renders_every_view_state_without_a_fabricated_zero`) |
| `cargo test --locked -p forma-app-platform --lib sharing::` | 4 passed, 0 failed, 443 filtered |
| `npm run -s typecheck` (in `app/admin`) | clean, exit 0 |
| `npx tsx --test test/sharing.test.ts` (in `app/admin`) | 5 passed, 0 failed |
| `strings app/admin/dist/shared.ae36e727.ir \| grep -i sample` | `sample-note`, `sample-data`, `sampleData` and the text are present, so the bin test rendered the committed TSX |

Limitations: I ran no campaign (`live-sharing` included) and checked nothing in a browser.

## Recheck (7a0625e)

**Verdict: approve.** M1, m1, m2 and m3 are fixed. One minor follow-up, r1, is for the ticket only.

- **`syncSampleNote`** (`admin/src/shared/document.ts:28-44`). It scopes itself to `host`, creates the note only for an available sample read when none exists, never adds a second one, and otherwise toggles `hidden` from `sampleNoteHidden`.
- **Guest page.** `unavailable()` hides the note (:91), and the available refresh shows it (:123). Both use `.shared-page-head`, which holds the server's note, so the created note lands where the server would put it. The show runs only after the `available` check and the authority-version guard, in the same synchronous block as `replaceSharedParts`.
- **Connected card.** `mask` hides the note (`canvas-controller.ts:58`). `refreshData` shows it (:87), only after the `available`/`can_add_to_canvas`/`added_to_canvas` checks, the epoch check and the version guard. Its host, the `.connected-owner` parent, exists both in the cards the server renders (`shared/canvas.tsx:26`) and in client cards (`canvas-controller.ts:36`). `mask` blanks the owner text but keeps the element.
- **No unverified display.** On the page and on the card, every unavailable, error, offline, pagehide or mask path goes through the hiding call, and a removed card is gone entirely. I found no path that shows the note for an unavailable or unverified read.
- **Wording.** `app.tsx:23` contains `SAMPLE_NOTE_TEXT` verbatim, and the test pins it. The rebuilt `dist/shared.66da3c13.ir` has the new sentence. No seed inserts records (only `storage/commands.rs` and a campaign do), so "filed by the app's members" holds.
- **Tests.** The new test drives `syncSampleNote` on the fake DOM: created once, hidden while unverified, shown again. As the ticket now says, nothing drives `startSharedPage` or `startConnectedApps` through the hide and show.
- **Ticket.** The corrected text matches the code. "No Rust changed" holds: the diff touches no `.rs` file. I did not reproduce "seen failing with its rule broken" or `just check admin test http-smoke`.
- **r1 (minor, ticket).** The data-copy over-label (work page review m1) is still not recorded in `APP-098.md`. A data copy keeps the sample source bindings, so the flag stays set, but the copy's views read the member's editable records. Both notes then say charts read synthetic data. Add it to DS-84's "Remaining".

| Command (in `app/admin`) | Result |
| --- | --- |
| `npm run -s typecheck` | clean, exit 0 |
| `npx tsx --test test/sharing.test.ts test/connected-authority.test.ts` | 7 passed, 0 failed |
