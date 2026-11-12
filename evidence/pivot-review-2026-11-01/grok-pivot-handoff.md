# Pivot: a clean trial for a new user

Victor, 2026-11-01. This note is the full record of the change in the MVP and the demo, the work done against that change, and the work that is still ahead. It does not replace the tickets. Ticket Markdown in the status folders stays authoritative. The short pointers on trunk (`pivot/pitch-mvp`) stay `docs/planning/DIRECTION.md`, `docs/planning/first-run/README.md`, and `docs/planning/LATER.md`.

Nothing in this note is merged. Nothing here authorizes a deploy. `https://beta.getforma.dev` is still the old sandbox.

## The change

The MVP a reviewer is handed is a clean start for a new person.

The earlier demo opened in the middle of someone else's work. Diego, Sarah, and Priya were already signed in. The organization already held a seeded library (about twenty apps). Expense review was already on the canvas with its cards, its claims, and its receipts. Demo mode supplied rehearsed sentences. The stage was built so it would never start empty. A reviewer can call that staged. That path is no longer the trial.

The trial is an invite into an empty organization. One person lands on an empty canvas, creates one app, adds one record, and shares it. The recipient has no account. Accepting the share is what makes them sign up. They arrive with no other apps. The shared app is the one both of them work from. A stranger gets the uniform 404.

The three-minute cap is only the recorded video of that path. The try-it link is a real workflow. Someone using it may take longer. The link is not cut to fit the recording.

The product for that path is the one already in the platform: an invite and a sign-in, an empty canvas, compose through admission, one audited record, and a share. It is a clean implementation of those steps for a new user. It is not a second demo mode, not a public signup, and not a canvas that starts halfway full.

## What the old demo was

These stay in the repository. They are the concept-car rehearsal. They are not deleted, and they are not what a reviewer is handed.

| Piece | What it is |
| --- | --- |
| APP-096 | The live rehearsal. Personas, a seeded library, and two windows already signed in. |
| APP-098 | Shortcuts whose purpose is that the stage never starts empty: personas, pre-filed claims, rehearsed sentences, the demo organization. |
| APP-105 | Acceptance that runs those scenarios. |
| APP-110 | Section A, written 2026-10-30, describes that seeded demo as what ships. |
| `docs/pitch/SCENARIOS.md` | The concept-car script. Frozen as history. |
| Pitch profile | Diego, Sarah, and Priya on `127.0.0.1:4001`, `4002`, and `4003`, with the seeded library. Local testing only. |

`GENUI_DEMO_MODE` and a rehearsed sentence do not appear on the trial path. The guide on the empty canvas names the next moves in ordinary words.

The pitch profile stays for development. It must not be what AWS serves. On 2026-11-01 a read-only check found that `https://beta.getforma.dev` is still that sandbox: demo mode on, Northstar, task definition revision 12, image `genui-synthetic` digest `sha256:b093b942d8840e2c1b580708b11ed95e8ceb4c4306f1710d3737d530d6b1e4c3`. No AWS resource was changed. Correcting beta waits for an approved window (APP-140). Do not record the demo there and do not send the panel there until that correction.

## What the MVP is now

One invite. One empty organization. One person. One app. One record. One share that creates the second person.

1. The first invite admits one new member into an organization with no library apps, no placed cards, and no records.
2. The canvas is empty. A short guide names the next moves.
3. The member describes one app. It appears through the existing composer and admission path. The model returns data. It does not return HTML, script, SQL, FMIR, or a grant. Admission can refuse it. A refusal leaves the canvas empty.
4. The member adds one record through the existing command path. A shown number carries its receipt.
5. The member shares that work. The recipient does not already have an account. Accepting the share signs them up. They arrive with this app and no other. A stranger gets the uniform 404.

Today `share_work` still refuses a grantee who is not already an active member of the organization (`app/src/storage/sharing.rs`). Closing that gap is APP-139. It is not built.

Creating the app has a hole in front of it. A generated definition is published onto an app the member already owns, with `edit_definition` and `run`. The clean dev stand creates that shell with `create-app` before sign-in. An invitation does not. A new member of an empty organization therefore gets `no_authoring_app`. APP-145 is the one exception: on that member's own first compose, the server creates one empty authoring shell and then continues the same compose. It does not invent a publication policy, and it does not skip admission.

The local twin of the live trial is the main checkout's clean profile, `127.0.0.1:4010`. A newly reset clean instance has no library apps, no placed cards, and demo mode off. Victor records the demo there. Each panelist gets an account on that same kind of world, on the live site this port matches, and can start empty or open an app that was shared with them. None of those accounts is Diego, Sarah, or Priya. A worktree clean port is a disposable copy. It is not that twin. APP-140 holds the environment split. The product path those accounts walk is APP-139.

## What does not change

- Model output is admitted data. Trusted first-party renderers draw it. The model does not write executable code, SQL, markup, FMIR, or a grant.
- Identity, tenant isolation, receipt provenance, idempotency, and transactional audit stay intact.
- The target rendering mode is Rust/FMIR server rendering plus the registered canvas island. The `PageResponse` security contract stays whole.
- The seeded rehearsal, the pitch scenarios, and the design history stay on disk. Later delivered design text outranks stale preparation prose. Frozen snapshots are not rewritten to match the new copy.
- Trunk is `pivot/pitch-mvp` until the November 3 landing decision (ADR-013). `main` stays the Phase 1 go-live record.
- Tickets stay in Backlog until every local predecessor is Done. The board refuses Selected, In progress, In review, or Done before that. `python3 Tickets/_tools/board.py next` follows the old dependency queue. Do not use it to find this work.
- Each implementation is its own branch and pull request from trunk. Review comes from Codex and Claude. Nothing is self-approved. Nothing is merged unless Victor asks.

## The product order

Start here. Each one is its own branch and pull request. Do not skip ahead because a later ticket is written.

| Order | Ticket | What it is | State on 2026-11-01 |
| --- | --- | --- | --- |
| In front of a new member's first description | APP-145 | One authoring shell when that member owns none. Grants `edit_definition`, `run`, and `discover` on an app they own. One audit event. A second compose reuses it. | Implemented on [PR 25](https://github.com/getforma-dev/forma-app-platform/pull/25). Open. Not merged. Ticket stays Backlog. |
| 1 | APP-141 | Create the app before the canvas. Template, then description. Cards are parts of that app. Widgets stay one-offs. A member who owns no app still sees `no_authoring_app` until APP-145. | Implemented on [PR 26](https://github.com/getforma-dev/forma-app-platform/pull/26). Open. Not merged. Ticket stays Backlog. |
| 2 | APP-142 | The united app is the page. Focus stays inside it. Fan-out is the only way the parts return to the board. The page is what is shared. The page resizes in this browser. A stack stays a stack. | Implemented on [PR 27](https://github.com/getforma-dev/forma-app-platform/pull/27). Open. Not merged. Ticket stays Backlog. |
| 3 | APP-144 | That same app is a real HTML page at the existing address `GET /w/{app}/{work}`. The trusted renderer draws the admitted app. The model does not write the HTML. | Not started. |
| 4 | APP-143 | Ask about the app, the focused part, or the weather, clock, or calculator widget that was clicked. The question does not grant read or write. | Not started. Depends on APP-142. |

The trial and the world around it:

| Ticket | What it is | State |
| --- | --- | --- |
| APP-139 | The guided first-run: empty canvas, one app, one record, one share that signs the recipient up. The guide does not read demo mode. | Not started. The authoring shell it needs is APP-145. The share-to-a-stranger gap is still in this ticket. |
| APP-140 | Keep the Diego/Sarah/Priya sandbox local. Pin `127.0.0.1:4010` as the twin of the AWS trial. Correct beta off the sandbox. Panel accounts are new accounts on the clean world. | The 2026-11-01 read-only check is recorded. Beta is still the sandbox. The correction, the panel accounts, and the recording are not done. Depends on APP-139. |

Stacks, focus, and the folder stay as APP-104, APP-106, and APP-120 already define them. Whether a card can leave the app and keep its data is not decided and is not a starting ticket.

## Later, after the page exists

These are written down so they are not forgotten. They are not the MVP and they are not where work starts. The spikes and APP-141 through APP-144 come first. APP-145 still sits in front of a new member's first description.

| Ticket | What it is |
| --- | --- |
| APP-150 | Sections the member marks as favorites, and a way to go to one. A place on the board or inside the app page. This member, this browser. A different list from the template favorites in APP-141. |
| APP-151 | Middle-click zooms into the focus that already exists. Tab then moves through that app. |
| APP-152 | An API bank. An API is an admitted definition (method, path shape, fields, which work it may touch). The platform runs it through the existing session, fence, command or query, and receipt. The model does not write the server. Dropping one attaches it to an app. It does not become its own app. |
| APP-153 | A spike: why the right pane holds Library, Commands, Ask, and Shortcuts, and which of those belong there. Answer this before moving Ask. APP-143 does not assume Ask stays a tab. |

PH02 through PH06, APP-078's reserved database window, canvas fences, and the platform-landing list inside APP-097 are untouched by this pivot.

## What was done

Trunk is `origin/pivot/pitch-mvp` at `c5b9452` (merge of PR 24, library tabs) as of this note. The open pull requests against that trunk:

| PR | Branch | What it carries |
| --- | --- | --- |
| [22](https://github.com/getforma-dev/forma-app-platform/pull/22) | `pivot/app-model-spikes` | Spikes that define the app model. Open. Not accepted. Not merged. |
| [25](https://github.com/getforma-dev/forma-app-platform/pull/25) | `pivot/app-145-shell` | The authoring shell. Review fixes are on the branch. Open. Not merged. |
| [26](https://github.com/getforma-dev/forma-app-platform/pull/26) | `pivot/app-141-create` | Create-an-app before the canvas. Open. Not merged. |
| [27](https://github.com/getforma-dev/forma-app-platform/pull/27) | `pivot/app-142-page` | The united app as a page. Open. Not merged. |

No GitHub review was posted by the implementer. No pull request was self-approved.

### APP-145, the shell a new member needs

`genui_private.ensure_authoring_shell` runs inside the fenced compose when `choose_authoring` finds nothing. The membership read is `FOR SHARE`. An advisory transaction lock serializes two first composes for the same member, and the waiter re-reads. The function is `SECURITY DEFINER`, `STRICT`, `VOLATILE`, `PARALLEL UNSAFE`, owner `genui_admit`, `GRANT EXECUTE` to `genui_workspace`, `search_path` `pg_catalog, pg_temp`.

The shell grants `edit_definition`, `run`, and `discover` only. The app insert is a column grant. Schema version 16. Migration `0016` sha256 `e9f5ad002af82bbd79934d71457112386661442610ca0834ea869acf1a8f87ec`. Catalog v16 sha256 `3ba0a86750eb36a2e2fa5673f17176d64da30b8115ac058d81b47d4f2cab7d3a`. Catalogs v1 through v15 stayed byte-identical.

The composer campaign was re-run after the lock and qualifier fix and passed. `qualify_tenant_schema.py` passed 378 checks. The native Postgres catalog-compare campaign was not re-run. Two overlapping first composes are covered by the lock. A production pause hook was not added.

### APP-142, the page

Worktree `/Users/victorvillacis/dev/FormaBuiltApps/forma-pitch-page`, branch `pivot/app-142-page`, commits `a31842d` and `0d919f2`.

- Uniting an app opens one page. The parts are the existing cards, moved in admitted order. The header keeps the app name and the retrieved, adapted, or generated label.
- Focus from the page opens one part. Closing it says "Back to the page" and puts the card back in the page. It does not scatter the board.
- Fan-out replaces break-apart as the only scatter. The label is "Fan out". Shift+U, the frame button, `/split`, and the old sentence still do that action. `/fan` is the named command. Each part returns to the spot it had before the page opened.
- The page resizes like a window. The minimum is 320 by 280, enough for the header and one part. The maximum is the viewport. The size is this member's browser layout, stored beside the existing united layout. A saved record without a size still loads. It is not a grant and it is not written to the server.
- Share sits on the page. `GET /v1/members` lists the other active members. `POST /v1/work/{app}/{work}/share` sends `{"grantee_principal_id"}` only. `share_work` was not changed. A card id is not sent.
- U on a selected stack says "A stack stays a stack. Select a card of the app, then press U." Leaf, fan, and spread still work. Uniting a card of an app can still pull that app's cards out of a stack, because the page is the app.

Pitch `GUIDE.md`, `SCENARIOS.md`, and `CANVAS-RULES.md` still say "break apart" and "one frame". Those snapshots were left as history. In-app copy and the shortcut table say "Fan out".

Fresh-browser check on the worktree stand `http://127.0.0.1:8001/canvas` (port base 8000, isolated browser). Ports 4001–4003 and 4010 were not opened.

- Library Unite on Expense review (demo data) opened one page, "is open. 7 parts."
- Focus opened Claims and total amount. Close read "Back to the page (Esc)" and announced "Back to the page." The page stayed one page.
- A corner drag stored 320×763 and set the sized state. Reload restored 320×763, the header, and seven parts.
- Share listed Priya Natarajan and Sarah Chen. The post body was only `grantee_principal_id`. The seeded library work is not Diego's to share, so the existing route returned 404 and the page said "The share was refused."
- Fan-out announced "Expense review (demo data) is back on the board." and returned each part to its previous spot.
- Library Stack made one stack of 7. Leaf moved it from 1/7 to 2/7. U refused and left the stack. Enter fanned 7 cards. Escape folded them. Spread announced "7 cards spread onto the canvas."

From `app/admin` in that worktree: `npx tsc --noEmit` exited 0. `npm test` passed 58, failed 0, duration_ms 702. `cargo test --locked -p forma-app-platform --lib composer::labels::tests::rules_know_the_catalog_and_the_verbs -- --exact` passed after `npm run build`, because rust-embed needs `app/admin/dist`. The worktree server was stopped afterward. The stand used for that check is a disposable copy of the pitch sandbox, used to exercise the page. It is not the clean trial.

### A design pass that was not implemented

A UI pass looked for gaps and convention bugs. It was a report only. Those findings were not fixed and are not the APP-142 task. The ones worth remembering:

- Beta home is still "Forma app platform" with a count. Trunk home is already "Your work. In good shape." That home copy is APP-140's world, and beta has not been corrected.
- `/signin` has no form when no organization handle is present.
- Confirm, chooser, and shape cards are dialogs that do not take the keyboard, so L, V, H, and `?` still fire behind them.
- Record rows are not links.
- Below 760px the canvas hides several chrome labels.

## What this working session did

The request was to continue the plan and the next tickets. The next ticket in the order was APP-142. APP-144, APP-143, APP-139, and APP-140 were not started.

Before that, on the same plan:

- Review fixes already on the table for PR 25 were pushed. The pull request body was updated so it no longer says the Docker composer campaign was skipped. No review comment was posted on GitHub.
- The design pass above was written as a report and left unfixed.

For APP-142:

- Branched `pivot/app-142-page` from `origin/pivot/pitch-mvp` in `/Users/victorvillacis/dev/FormaBuiltApps/forma-pitch-page`. The main checkout on `pivot/app-135-clean-audit` was not used for the implementation.
- Added the page module, the resize handle, the fan-out copy, the page share menu, and the stack refusal. Reserved the verb `fan` so an app cannot be addressed by that name. Recorded the one local change on the vendored canvas (`resizeItem`) and the `.dockerignore` allowlist entry for `page.ts`.
- Ran the admin typecheck and the admin test suite. Ran the reserved-verb Rust test after a production admin build.
- Verified the page in a fresh browser on port 8001, then stopped that server.
- Updated `Tickets/4-done/APP-142.md`, regenerated the board, and recorded the checks. Status stayed Backlog.
- Pushed the branch and opened PR 27. Stopped for Codex and Claude.

Ports 4001–4010 were not reset, not browsed, and not used as the demo. No AWS resource was changed. `contracts/.venv`, `app/admin/dist`, `.env.dev`, and Playwright snapshots were not committed.

## What is still ahead

Do these in order. Stop after each pull request for review. Do not merge until Victor asks.

1. Review of PR 22, PR 25, PR 26, and PR 27. Fix what Codex or Claude comment on. Do not treat an open pull request as accepted.
2. APP-144. The HTML page at `GET /w/{app}/{work}`, from the board and from the APP-142 page, using the existing work-page renderer. One fresh-browser check on a worktree port.
3. APP-143. Ask attached to the clicked app, the focused part, or a weather, clock, or calculator widget.
4. APP-139. The empty-canvas guide, and the share that signs up someone who has no account. The shell it needs is already on PR 25. The guide must not read `GENUI_DEMO_MODE`.
5. APP-140, when there is an approved window. Point the public site at the clean trial. Create Victor's account and the panel accounts on that world. Record the three-minute video on `127.0.0.1:4010`. Leave the pitch database in place.

Then, only after the page direction is in: APP-150, APP-151, APP-152, APP-153.

## Rules for the next session

- Read `docs/planning/DIRECTION.md`, `docs/planning/LATER.md`, `docs/planning/first-run/README.md`, and `Agents.md` before writing code.
- The trial a reviewer sees starts empty. Do not demo from Diego, Sarah, Priya, the seeded library, demo mode, or `https://beta.getforma.dev`.
- Do not reset or browse `127.0.0.1:4001`–`4003` or `4010` unless Victor asks. `4010` is his clean twin. A worktree port is not that twin.
- Do not merge. Do not self-approve. Do not post a review of bugs that were just fixed.
- Do not check an acceptance box until the check it names has passed.
- After a ticket edit, run `python3 Tickets/_tools/board.py render` and `python3 Tickets/_tools/board.py check`. Do not run `board.py next`.
- Model output stays data. Both example apps use the same path. No tenant-name branch.

## Where the decision is written

| Document | Role |
| --- | --- |
| `docs/planning/DIRECTION.md` | The four tickets to build, in order, and the note that APP-145, APP-139, and APP-140 sit around them. |
| `docs/planning/first-run/README.md` | The trial. Why a halfway-full canvas does not count. The two machines. The keep / not-the-trial / later piles. |
| `docs/planning/LATER.md` | APP-150 through APP-153. |
| `Tickets/0-backlog/APP-139.md` through `APP-145.md`, and `APP-150.md` through `APP-153.md` | The authoritative scope of each slice. |
| This file | The narrative of the pivot, the work done through 2026-11-01, and the list still ahead. |
