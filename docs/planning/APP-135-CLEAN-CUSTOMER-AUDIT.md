# APP-135 clean-customer audit

Focused walk of a new customer with no template library and no existing data. This is evidence for APP-135. It does not close the ticket. Acceptance checks stay open. Folders, sharing, MCP, Slack, rename, and Victor's keep/remove/defer decisions were not exercised.

## Environment

| Item | Value |
| --- | --- |
| Date | 2026-10-31 |
| Runtime under test | `574e26d0c10bd92b8f15e35f1211ee572193df7b` on `pivot/local-surfaces` (PR #5) |
| This document's base | `af63feba41aa825a972dde354e0a5d2195f7d608` on `origin/pivot/pitch-mvp` |
| Checkout | Isolated worktree `forma-pitch-audit-574e26d`. Its own `just setup` and gitignored `.env.dev`. Not another checkout's environment. |
| Instance | `just dev clean -d` with `GENUI_DEV_PORT_BASE=5500`. Alex Rivera at `http://127.0.0.1:5510/canvas`. Demo off (`data-demo` empty). No `seed-library`. |
| Left alone | Listeners on 127.0.0.1:4001 and 127.0.0.1:4010 were not reset, restarted, or modified. |
| Browser | Playwright 1.49.1, Chromium headless, viewports 1440×900 and 1280×800. A real page against the running server. No mocked model output. |
| Model | The server read the model key from the main checkout's gitignored `.env` (presence only; the key is not recorded). Compose, Ask, and Shape each returned a live model result. |

Screenshots in [app-135-clean-customer/](app-135-clean-customer/) show the synthetic organization and record ids. They do not include credentials.

Created app: Book Club Reading List, app `00000000-0000-4000-8000-00000000d0a4`, work `2bbc9e92-12c8-491a-8825-cd4c902a6026`, publication `8990ee87`.

## Steps

### 1. Empty workspace guidance — pass

- Action: open `http://127.0.0.1:5510/canvas` in a new browser context.
- Expected: guidance for an empty workspace, with no expense examples and no seeded library.
- Observed: header "Synthetic organization 0002" and "Empty canvas". Copy: "From idea to working app." and "Describe what you need. Bring your data. Shape a workspace that works the way you do." Cards: "Describe an app" and "Bring your files". Chips: "Create a reading list" (`data-intent` `/forma Create a book club reading list`), "Create a project folder" (`/folder new My project`), "Add the weather" (`/widget weather`). No expense starter. No library card until an app exists. The first screenshot frame shows "Reconnecting"; later frames show "Live". That did not block the walk.
- Evidence: [01-empty-guidance.png](app-135-clean-customer/01-empty-guidance.png).

### 2. Create a reading list through the composer — pass

- Action: click "Create a reading list".
- Expected: a simple records app, admitted from the live model, with no template library.
- Observed: `POST /v1/act` returned 404 `not_found` (empty command catalog), then `POST /v1/compose` returned 200, `origin` `generated`, title "Book Club Reading List". The page showed three cards: the app card "Unite", a "New book" form (Title, Author, Genre, Notes, Add), and "Books" with "0 records" / "No records yet." Provenance chip "Generated" / "Generated for your request (typed definition, admitted)." No approval steps. The 404 is the empty-catalog fallback into compose. The customer-visible result was the generated app.
- Evidence: compose response on the running server; the placed app is in [05-form-committed.png](app-135-clean-customer/05-form-committed.png).

### 3. Reload and reopen; name and provenance — pass, with a placement gap

- Action: reload the same browser, then open `/canvas` in a new browser context.
- Expected: the same name and generated provenance.
- Observed: the same browser kept "Book Club Reading List", "Generated", and the three cards, still at 0 records. A new browser context does not place the group. The header can say "Empty canvas" and "Your canvas is empty" while the library lists "1 app · 3 cards" and "Open your library" appears. After a later command placed the group, the Generated label was still on the app card.
- Verdict: name and provenance pass when the app is open. Placement across browsers fails.
- Severity: medium.
- Suggested fix: restore the member's placed works from the server. Local layout can remain a cache. "Layout saved locally" is the current copy.
- Evidence: reload text at 21:26 UTC; new-context text in the 21:28 UTC capture; [22-confirm-card.png](app-135-clean-customer/22-confirm-card.png) shows "Empty canvas" above a library that already has the app.

### 4. Create a record: cancel once, then confirm — fail on the form, pass on the utterance

- Action A: on the New book form, fill Title "The Dispossessed" and Author, Genre, and Notes "Ursula K. Le Guin" (the last three are the tester's fill, because those inputs had no title-like label), then click Add. Look for Cancel, then submit again.
- Expected: a confirm step, a cancel that writes nothing, then a confirmed save.
- Observed: Add posted `POST /v1/work/.../commands/add` immediately. The form said "Committed. Record version 1, work version 1, commit 2. Audit event 87071760-cf32-4c84-81ef-74873aede6fe." The Add button was then disabled. No Cancel and no confirm card. Books showed 1 record, filed by Alex Rivera. Genre and Notes repeat the tester's author string.
- Verdict: fail. Severity: high.
- Suggested fix: the canvas form should use the same confirm card as the utterance path before the command is posted.
- Evidence: [05-form-committed.png](app-135-clean-customer/05-form-committed.png).

- Action B: type `add The Left Hand of Darkness by Ursula K. Le Guin` and press Enter. Click Cancel. Repeat the sentence. Click "Add book".
- Expected: a draft, a cancel that writes nothing, then a confirmed record.
- Observed: `POST /v1/act` 200, action `create`, title "Book Club Reading List". Card "Add book?" showed Title "The Left Hand of Darkness", Author "Ursula K. Le Guin", Genre and Notes "—", chips "Book Club Reading List" and "No approver named", and "Understood in 1650 ms. Nothing is written until you confirm." Cancel dismissed it: "Cancelled. Nothing was changed." The repeat showed the same card (1556 ms). "Add book" wrote the row. Books then showed 2 records. Outcome line: "Filed: record v1, commit 3, audit 9b9022b6." Publication `8990ee87`, `sha256:9fa290e66035…7cd692`, work v1.
- Verdict: pass.
- Evidence: [22-confirm-card.png](app-135-clean-customer/22-confirm-card.png).

### 5. Open the record and check the saved values — fail

- Action: read the Books card. Click its rows (they are not links). Open Workspace, then the work page, then the only record link, "New book record".
- Expected: the saved title and author for each book.
- Observed on the canvas card: "The Left Hand of Darkness" / "Ursula K. Le Guin" / genre "—" / notes "—", and "The Dispossessed" / "Ursula K. Le Guin" / genre and notes "Ursula K. Le Guin" / both "Filed by Alex Rivera". The canvas rows are not links, so there is no per-row detail page.
- Observed on the work page: title "Book Club Reading List", "Saved work.", "Open on the canvas", "This work keeps records. Open them on the canvas.", and one record link, "New book record". No book titles. No Window start / Run views. No Generated chip (the full-page templates have no provenance field).
- Observed after clicking that link: `/work/00000000-0000-4000-8000-00000000d0a4/2bbc9e92-12c8-491a-8825-cd4c902a6026/records/e3703dd6-4057-4858-b880-7145a818c733/deb4a555-2d4f-4f8f-9558-da7cd27a372b`. Page title "Record". "Saved app. Schema book version 1". "No record committed yet (version 0); work version 1." Empty title, author, genre, and notes, plus "Submit add". That button was not clicked. The collection id matches the New book form card, not one of the two committed rows.
- Severity: high. The canvas list is right. The only full-page record says nothing was committed.
- Suggested fix: list the committed Books rows, and link each row to its committed version. Do not present the form's version-0 slot as the saved record.
- Evidence: [34-work-page.png](app-135-clean-customer/34-work-page.png), [42-empty-record.png](app-135-clean-customer/42-empty-record.png).

### 6. Ask about the new records — pass

- Action: in the intent bar, submit `? which books are on the reading list?`
- Expected: an answer about the two new records, from the live model, with demo mode off.
- Observed: `POST /v1/ask` 200. Text: "The reading list contains two books: The Left Hand of Darkness by Ursula K. Le Guin and The Dispossessed by Ursula K. Le Guin." Scope: "Asking about your canvas (1 app)". Both titles had receipt chips. "Ask reads; it never changes records, cards or the canvas." This was not a rehearsed demo answer.
- Evidence: [26-ask.png](app-135-clean-customer/26-ask.png).

### 7. Shape a supported card — fail on the Books card, pass on the app theme

- Action: `/shape books highlight the title`. Then the real card address from the command list, `/shape 1.records highlight the title`, and `/shape 1.records highlight the title in green`. Then `/shape 1.app dark compact` and Apply.
- Expected: a preview of a highlight on the Books card, and Apply only after that preview.
- Observed: `/shape books highlight the title` did not call the server. The page said "Nothing answers to books. Shape a card (app.card), an app or a widget." The command list names the card `1.2` / `1.records` ("Books"), the form `1.1` / `1.new`, and the app `1.0` / `1.app`. Both `1.records` phrases returned `POST /v1/shape` 422 `{"code":"could_not_shape","status":422}`. The page said "That came back as something other than a view, so nothing was shown. Nothing changed." Apply was absent. The credentials were present: the same server had just composed and answered with the model, and the next Shape call succeeded.
- `/shape 1.app dark compact` returned 200 `{"spec":{"density":"compact","mode":"dark"},"target":"app"}`. Preview: "Nothing changes until you apply it." Apply was clicked. The app card went dark, gained "shaped" and "Revert", and the page said "Every /1 card is shaped; each keeps its own view. Revert is on the app card." Theme persistence after reload was not checked.
- Severity: high for the generated list's records card.
- Suggested fix: a highlight of a records card that already has rows should become the closed highlight spec this product already admits, or the model should be retried once when the reply is not a view. The refusal can stay when the reply is still not a view. "books" should resolve to `1.records`, or the message should name `1.records`.
- Evidence: [37-shape-refused.png](app-135-clean-customer/37-shape-refused.png), [41-theme-applied.png](app-135-clean-customer/41-theme-applied.png).

### 8. Make the answer a card and pin it — pass

- Action: on the Ask result, click "Make it a card", then "Add as new card". Then click "Pin to canvas".
- Expected: both land on the canvas through the real controls.
- Observed: Make it a card showed a preview of the two records and "Nothing changes until you apply it. No record is written." "Add as new card" posted `POST /v1/cards` 200. The card said "A new card from / is on the canvas, made by you." Card id `utility-made-6c8abd96-b033-4783-a0cd-bdf501e22e6c`. Pin posted `POST /v1/cards` 200. The pinned card said "As answered on 2026-10-31 21:29 UTC · asked about your canvas · it does not run again" and repeated the answer. Card id `utility-pin-d02f97dd-8089-44e7-a6f9-744900df598d`. The Ask control then read "Pinned".

### 9. Reload and check persistence — pass for the saved objects

- Action: reload `/canvas` in the same browser after the pin.
- Expected: the records, the generated label, the made card, and the pinned answer are still there.
- Observed: all four remained. Books still showed both rows. Commit stayed at 3. The new-browser placement gap in step 3 is separate: a fresh context still does not place the app group, while the made and pinned cards do come back from the server.
- Theme Apply from step 7 was done after this reload and was not reloaded.

### 10. Canvas, Workspace, and the full work page — pass for navigation

- Action: open Workspace, open the work link, return to Canvas.
- Expected: each surface loads for this member.
- Observed: `GET /workspace` 200. "You are signed in." Saved work "Book Club Reading List", "Open on canvas", and "New book record". The older clean-instance Workspace 404 was not reproduced on this commit. The work page loaded at `/w/00000000-0000-4000-8000-00000000d0a4/2bbc9e92-12c8-491a-8825-cd4c902a6026` with the records-only copy and no query form. "Open on the canvas" and the canvas header link returned to the canvas, still showing the app, both records, the made card, and the pin. What those pages fail to show is step 5.

## Not a product failure

- `GET /v1/notes` 404 and `GET /v1/boxes/lineage/{app}/{work}` 404. No Notes app and no folder lineage. Both are the expected not-yet-present state.
- Genre and Notes on the first book match the author because the tester filled every non-title field with that string.

## Not tested

Folders and CSV upload, a second member, sharing and live updates, an MCP token, Slack, and renaming the app or using its new address. The founder keep/remove/defer list in APP-135 is unchanged.

## What this branch is

Documentation and the APP-135 register only. It does not contain the PR #5 code. PR #5 stays open for review. Nothing here was merged or deployed.
