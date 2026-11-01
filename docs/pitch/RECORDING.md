# Recording shot list and pre-demo checklist (YC demo)

How to set up, run and film the demo in short, retakeable shots. The words to say are in [PITCH-SCRIPT.md](PITCH-SCRIPT.md), section 5. The grammar is [CANVAS-RULES.md](CANVAS-RULES.md).

Updated 2026-10-31 (APP-096). Shots were rehearsed again on `pivot/pitch-mvp` at `af63feba41aa825a972dde354e0a5d2195f7d608`, in an isolated worktree (port base 7500: Diego `http://127.0.0.1:7501/canvas`, Sarah `:7502`, Alex `:7510`). The model key was present (server log: `genui-dev: model Anthropic`). The browser was Playwright Chromium, 1440×900. The log is [REHEARSAL-2026-10-31.md](REHEARSAL-2026-10-31.md). Shots not run on that commit are marked *(not re-run on af63feb)*. The table below still lists the main checkout's ports. `just status` prints the checkout you are in. Do not reset or restart a checkout whose listeners are already on 4001–4010. beta.getforma.dev still runs TaskDefinition 12, which has no Ask, Shape, made cards, folders' Ask and Make a card, or the clean slate.

## The instances

| Window | Persona | Instance | URL (main checkout) | Starts as |
| --- | --- | --- | --- | --- |
| A | Alex Rivera, a new customer | `just dev clean` (demo mode off, no library, no data) | http://127.0.0.1:4010/canvas | Empty canvas |
| B | Diego Ramos, employee | `just dev` (Northstar, the demo organization) | http://127.0.0.1:4001/canvas | Empty canvas |
| C | Sarah Chen, manager | `just dev` | http://127.0.0.1:4002/canvas | Expenses on the canvas |

The ports are the main checkout's (port base 4000). A worktree has its own base in its `.env.dev` (`just status` prints the URLs). The model key is read from the main clone's `.env`; the server log says `genui-dev: model Anthropic` when it is there.

## Before the demo: checklist

Run from the main checkout, in this order:

1. `just dev -d`: the pitch instance (Diego, Sarah, Priya). For fresh data, `just dev reset -d` instead: it starts over with three example claims (Sweetgreen $42 approved, Delta $389 and Blue Bottle Coffee $42.50 waiting).
2. `just dev clean reset -d`: Alex starts as a brand-new customer (no apps, no data). Do this before every take of beat 1.
3. **Don't** run `just dev clean seed-library` for the pitch: it installs the 20 template apps, and the new-customer story is gone. Use it only to show templates on the clean instance (after it and a reload, `/expenses` places the Expenses app there, verified).
4. `just status` and `just status clean`: both running, the URLs as above.
5. Open the three windows (Chrome, 1440×900, zoom 100%), and reload each one after any reset.
6. Sarah's window: type `/expenses` and Enter, so the Expenses app is on her canvas. Her canvas only updates live for an app that is on it (pitfall 5).
7. Diego's and Alex's windows: empty canvases (the "From idea to working app." page). If Diego's is not empty, Library → Remove on each app.
8. Finder: `docs/pitch/folder-fixture/` open, `q3_pipeline.csv` ready to drag.
9. Optional, for the ⏱ shots: ⋯ → **Show timings**, or `/canvas#timing=1` *(not re-run today)*.
10. Optional fallback for beat 1: keep a clearly labeled recording of an earlier clean-instance rehearsal ready to play. Capture it **before** the reset in step 2; do not prebuild in another :4010 window, since all those windows share Alex's organization and database and the app would appear on the live canvas. If no recording is ready, skip the fallback and continue to beat 2 when the model is slow.
11. Optional beat 6 (MCP): **Connect an agent** in Diego's window, create a token, and set up Claude Code as in SCENARIOS.md scenario 6 (`read -rs` so the token never shows) *(not re-run today)*.
12. Turn on Do Not Disturb, hide the bookmarks bar.

After the demo: `just dev stop` and `just dev clean stop` (data is kept).

## Shots

| # | Beat | Window | Do this | Exact words | Audience sees | ⏱ measured | Time |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | New customer | A | Type, Enter; talk over the wait | *Track our vendor contracts: vendor, owner and renewal date* | "Composing your app…", then a generated app. On `af63feb`: title **Vendor Contracts**, chip **generated**, form Vendor / Owner / Renewal date, button **Add contract**, records card "Vendor contracts" | 21.5 s (`duration_ms` 21524). Older 11.5 / 11.6 s were f0efc04 | 30 s |
| 2 | First record | A | Fill the form, **Add contract** | Vendor `Figma`, Owner `Alex Rivera`, Renewal date `2027-03-01` | Commits on click, no confirm card. "Committed. Record version 1, work version 1, commit 2." Row: Figma, Alex Rivera, 2027-03-01, Filed by Alex Rivera | under 1 s | 10 s |
| 3 | A file | A | Drag `q3_pipeline.csv` onto empty canvas; name the folder | `Q3 pipeline` | Folder opens as **Untitled folder**. After Enter, the title is Q3 pipeline. Buttons: **Ask**, **Make a card**, **✨ Make an app**, **Brief me**. The file may still say **Make data** | instant | 8 s |
| 4 | Ask the file | A | **Ask** after the title has changed; type in the Ask tab | *which open deals are worth more than $50,000?* | "There are 4 open deals worth more than $50,000. These are: Cobalt Health at $94,500 …" The older "q3_pipeline.csv · row 3" citation was not in this answer | model; not separately timed | 15 s |
| 5 | A card from data | A | **Make a card**, then **Make card** | — | Library afterwards lists `/q3-pipeline-card` and a chart "Amount by close date". **Verified**, the summary card, and "Built from folder" were not re-read on screen *(partial on af63feb)* | not re-timed | 15 s |
| 6 | Library pick | B | Type, Enter | *Review expenses over $200* | Expense review queue, origin retrieved, then **"Kept the standard view."** Totals stayed 240 / $263K. **Verified** is on the claims total. "Adapted to your request · Amount > $200" was **not** shown | 526 ms | 15 s |
| 7 | Do | B | Type, Enter, then **Submit claim** | */expenses $86 Nobu, Sarah approves* | "Submit claim?" $86.00 · Nobu · category other · submitted · Approver: Sarah Chen. Then "Filed: record v1, commit …" and the refused-share line (pitfall 5) | understood in 2148 ms | 15 s |
| 8 | Live arrival | C | Sarah's claims card must already list rows. Then look | — | After the card had loaded, it showed Nobu, Submitted, Filed by Diego Ramos. A freshly placed canvas did not show the new row within 20 s; reload if it is missing. The record is filed either way | not a clean sub-second live capture on af63feb | 10 s |
| 9 | Approve | C, then B | **Approve** on the Nobu row. The typed *approve the Nobu dinner* path was *(not re-run on af63feb)* | — | Sarah's row became **Approved** (waiting count 3 → 2). Diego's later claims card also showed Nobu Approved. The same-moment flip was not captured | row button on Sarah | 15 s |
| 10 | Ask | B | Type, Enter | *? @expenses where are the red claims?* | "Asking about /expenses (all its records)". Answer named Delta $389.00 and Blue Bottle Coffee $42.50, each with **Receipt**. Nobu was already approved, so it was not in the red list | rehearsed | 10 s |
| 11 | Made card, pin | B | **Make it a card** → **Add as new card**; then **Pin to canvas** | — | "Made by you from /expenses". "As answered on 2026-11-01 02:03 UTC · asked about /expenses · it does not run again" | under 1 s | 15 s |
| 12 | Live Ask | B | Type, Enter, after the previous Ask finishes | *? @expenses summarize where the money went* | Model text began "The money went to four expenses totaling $559.50…" with receipts. A second question can queue behind the first | not isolated | 15 s |
| 13 | Shape a card | B | Type, Enter | */expenses.ledger only red invoices* | "Only status is submitted or rejected"; Delta and Blue Bottle; "2 of 4 records shown". Status: "/expenses.ledger is shaped. Revert is on the card." Stored title "Expense ledger (shaped)" | rehearsed | 12 s |
| 14 | Shape an app | B | Type, Enter, **Apply** | */shape expenses dark green, compact* | Preview "Nothing changes until you apply it." After Apply: "Every /expenses card is shaped; each keeps its own view. Revert is on the app card." The older "dark mode · green accent · compact density" words were not copied off the screen | rehearsed | 10 s |
| 15 | *Optional* names | B | Type, Enter | */alias e expenses* | "/e now opens Expenses, for you only." | instant | 8 s |
| 16 | *Optional* agent | Claude Code, then B | Ask Claude | *What expenses are waiting for my approval?* | Claude answers through MCP as Diego *(not re-run today)* | — | 25 s |

Core shots 1–14: about 3 min 45 s before editing.

## Fallbacks

| If | Then |
| --- | --- |
| Shot 1 takes over 25 s | Play the labeled earlier-rehearsal recording from checklist step 10 ("the model is busy; this recording shows an earlier rehearsal"); if none is ready, continue to beat 2. If generation is refused ("could not be composed"), say so; it is the real model, and nothing is placed on a refusal. This commit took 21.5 s and succeeded |
| Shot 7 takes over 5 s | **Cancel**, and type the rehearsed sentence: *I spent $86 on a client dinner at Nobu last night, Sarah approves my expenses* (on `af63feb`: card in 925 ms, category meals, "Understood in 745 ms"; the rehearsal cancelled the draft) |
| Shot 12 is slow | Skip it: shot 10 already showed Ask with receipts |
| ✨ Make an app wanted | It takes about 20 s plus a 3–4 s summary (measured once). Build it during setup, or show it in the edited video at 2× with the real time on screen |
| Sarah's card doesn't show the claim | Wait until her Expense claims card lists rows before Diego files. If the new row is still missing, reload her window. The record is filed either way. On `af63feb` a freshly placed canvas did not show a follow-up claim within 20 s; a reload did |

## Pitfalls (APP-135 register and this run-through)

1. **Empty-canvas chips no longer point at Expenses** (re-checked on `af63feb`). They are "Create a reading list", "Create a project folder", and "Add the weather". The old `/expenses` chip and "There is no /expenses command" were not reproduced. Still do not click the chips during beat 1: they start a different object. Type the beat 1 sentence.
2. **Generated provenance while the app is on the canvas** reads "Generated for your request (typed definition, admitted)." A reload of that placed app was **not completed** on `af63feb` (a new browser does not restore local layout, and `/vendor-contracts` did not place it). Do not tell the old "reload turns it into Seeded definition" story, and do not promise the reload until it has been checked. Keeping Alex's window open through beat 1 avoids the question.
3. **An approval step on this vendor app was not reproduced.** The form was only **Add contract**. There was no "waiting for your approval" and no Expire or Cancel on the vendor card. One run is not a guarantee. Don't comment on an approval UI that is not there.
4. **A money field in a generated form takes minor units** *(not re-run on af63feb)*. The earlier observation stands as a reason beat 1 has no money field: typing `7200` stored $72.00. Do not add a money field to the sentence to "make it richer."
5. **"The share with Sarah Chen was refused; the record is filed."** appears after every claim Diego files on the dev stand. The Expenses work is the library's seeded work; `grant-library-access` already gives Sarah read and write on it, and Diego isn't its creator, so the share is refused (see the note in `app/src/dev/seed.rs`). Sarah sees the claim anyway, but only if Expenses is already on her canvas (checklist step 6). Don't read the line aloud.
6. **Diego sees Approve and Reject on his own claims.** Clicking one is refused by the server (`refused_by_schema`, verified). Don't click them in Diego's window.
7. **Rehearsed Ask ignores conditions it doesn't model** (demo organization only, APP-098; *(not re-run on af63feb)*). The earlier run of "? @expenses which claims over $100 are still waiting for approval?" answered with every waiting claim, including the $42.50 one. Use only the exact questions above. A question that starts with *why*, *explain*, *summarize* or *describe* goes to the model; the summarize sentence did so on `af63feb`.
8. **`/expenses $86 Nobu, Sarah approves` is not rehearsed**: it goes to the model (2.3 s) and files category "other". The long rehearsed sentence files "meals".
9. **The header reads "Synthetic organization 0001"** on Diego and Sarah, with a **Synthetic data** chip, and "Synthetic organization 0002" on Alex. The avatar reads "00". Crop the header if it matters on film. Alex's `data-demo` is empty; Diego's and Sarah's are on. Their `/v1/members` lists are different, and the confirm card's approver principal is Sarah's. That is how this rehearsal showed they are two members.
10. **Cards on Sarah's canvas re-query after a live update**, and one may show "This card couldn't load · Try again" for a moment. It recovered on its own in this run.
11. **Make a card is fast; its Summary card waits on the model** (about 4 s). ✨ Make an app is about 20 s.

## Rehearsed versus live

Inside the demo organization (windows B and C), these answer from rehearsed paths, without the model: the reshape in shot 6, the long Nobu sentence (fallback), the questions in shot 10 and the shapes in shots 13 and 14 (APP-098). Shots 7, 9 (typed), 12 and everything in window A are the real model. Keep at least one live line on camera with timings on (shot 1 or shot 12). Speed up waits only with a visible "2×" and the real time shown.

## Honesty notes (ADR-011: a concept car in the demo organization)

- Rehearsed paths are canned only in the demo organization (`GENUI_DEMO_ORGANIZATIONS`); window A (demo off) has none. Every shortcut is in APP-098.
- Everything after a parse is real in every window: the draft, the confirm, the audited commit, the permissions, the live update and the receipts.
- Release and onboarding still owe what APP-135 finds: pitfalls 1–4 above are real-customer issues, not demo staging.

## Retakes and resets

- **Beat 1 again:** `just dev clean reset -d`, reload window A. Takes seconds.
- **Fresh claims:** `just dev reset -d` (three example claims), reload B and C, then repeat checklist steps 6 and 7. Or file a new claim with another merchant; approved claims can't be reopened (by design).
- **Diego's made cards and pinned answers** stay across visits: **Remove** (click twice) deletes one for good; × only hides it until the next visit.
- **A theme:** **Revert** on the Expenses app card.
- **Agent token:** a sign-out ends tokens and Slack links; mint a new one after the last sign-in.
