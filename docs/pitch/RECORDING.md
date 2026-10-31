# Recording shot list and pre-demo checklist (YC demo)

How to set up, run and film the demo in short, retakeable shots. The words to say are in [PITCH-SCRIPT.md](PITCH-SCRIPT.md), section 5. The grammar is [CANVAS-RULES.md](CANVAS-RULES.md).

Updated 2026-10-31 (APP-096). Every shot below was run on a local dev instance on 2026-10-31 (`pivot/pitch-mvp` at f0efc04, the model key present), except those marked *(not re-run today)*. The demo runs on the laptop's dev instances: beta.getforma.dev still runs TaskDefinition 12, which has no Ask, Shape, made cards, folders' Ask and Make a card, or the clean slate.

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
| 1 | New customer | A | Type, Enter; talk over the wait | *Track our vendor contracts: vendor, owner and renewal date* | "Composing your app…", then Vendor Contract Tracker, **generated**, with a "New vendor contract" form and a "Vendor contracts" records card | 11.5 s and 11.6 s (2 runs) | 20 s |
| 2 | First record | A | Fill the form, **Add contract** | Vendor `Figma`, Owner `Alex Rivera`, Renewal date `2027-03-01` | "Committed. Record version 1, work version 1, commit 2. Audit event …"; the row on the records card with its receipt line | under 1 s | 10 s |
| 3 | A file | A | Drag `q3_pipeline.csv` onto empty canvas; name the folder | `Q3 pipeline` | A folder with **Ask**, **Make a card**, **✨ Make an app**, **Brief me** | instant | 8 s |
| 4 | Ask the file | A | **Ask**, then type in the Ask tab's input | *which open deals are worth more than $50,000?* | "There are 4 open deals worth more than $50,000 …" with four rows "q3_pipeline.csv · row 3: Cobalt Health · proposal · 94500.00" … each with **Receipt** | 2.2 s | 12 s |
| 5 | A card from data | A | **Make a card**, then **Make card** | — | "Amount by close date" chart with **Verified**, a Summary card ("AI summary · numbers from receipts"), "Built from folder: Q3 pipeline · 1 file" | card 0.25 s; summary 3.7 s | 15 s |
| 6 | Library pick | B | Type, Enter | *Review expenses over $200* | Expense review queue, "Adapted to your request · Amount > $200", **Verified** chips | 0.4 s (rehearsed reshape, APP-098) | 15 s |
| 7 | Do | B | Type, Enter, then **Submit claim** | */expenses $86 Nobu, Sarah approves* | "Submit claim?" $86.00 · Nobu · submitted · Approver: Sarah Chen · via /expenses; then "Filed: record v1, commit …, audit …" | 2.3 s (real model) | 15 s |
| 8 | Live arrival | C | Just point | — | The Nobu row on Sarah's Expense claims card, "waiting for your approval" | under 1 s | 5 s |
| 9 | Approve | C, then B | **Approve** on the Nobu row (or type *approve the Nobu dinner*, then **Approve claim**) | — | Diego's row flips to **Approved** | row button instant; typed 2.1 s | 15 s |
| 10 | Ask | B | Type, Enter | *? @expenses where are the red claims?* | Ask tab, "Asking about /expenses (all its records)", the waiting claims, each with **Receipt** | instant (rehearsed, APP-098) | 10 s |
| 11 | Made card, pin | B | **Make it a card** → **Add as new card**; then **Pin to canvas** | — | "Made by you from /expenses · only you see it"; "As answered on <date> … it does not run again" | under 1 s | 15 s |
| 12 | Live Ask | B | Type, Enter | *? @expenses summarize where the money went* | A paragraph with totals by category, every record listed with its receipt | 2.1 s (real model) | 12 s |
| 13 | Shape a card | B | Type, Enter, **Add as new card** | */expenses.ledger only red invoices* | Preview "Only status is submitted or rejected"; then "Expense ledger (shaped)", made by you | instant (rehearsed) | 12 s |
| 14 | Shape an app | B | Type, Enter, **Apply** | */shape expenses dark green, compact* | Preview "dark mode · green accent · compact density"; every Expenses card dark and green; "Revert is on the app card" | instant (rehearsed) | 10 s |
| 15 | *Optional* names | B | Type, Enter | */alias e expenses* | "/e now opens Expenses, for you only." | instant | 8 s |
| 16 | *Optional* agent | Claude Code, then B | Ask Claude | *What expenses are waiting for my approval?* | Claude answers through MCP as Diego *(not re-run today)* | — | 25 s |

Core shots 1–14: about 3 min 45 s before editing.

## Fallbacks

| If | Then |
| --- | --- |
| Shot 1 takes over 20 s | Play the labeled earlier-rehearsal recording from checklist step 10 ("the model is busy; this recording shows an earlier rehearsal"); if none is ready, continue to beat 2. If generation is refused ("could not be composed"), say so; it is the real model, and nothing is placed on a refusal |
| Shot 7 takes over 5 s | **Cancel**, and type the rehearsed sentence: *I spent $86 on a client dinner at Nobu last night, Sarah approves my expenses* (confirm card in 945 ms, category meals, note "a client dinner"; rehearsed, APP-098) |
| Shot 12 is slow | Skip it: shot 10 already showed Ask with receipts |
| ✨ Make an app wanted | It takes about 20 s plus a 3–4 s summary (measured once). Build it during setup, or show it in the edited video at 2× with the real time on screen |
| Sarah's card doesn't show the claim | She placed Expenses after the filing: it is there after `/expenses` (the record is filed either way) |

## Pitfalls (APP-135 register and this run-through)

1. **The clean slate's example chips point at apps it doesn't have** (APP-135). On :4010, the `/expenses` chip answers "There is no /expenses command." "Review expenses over $200" has no Expense review queue to land in there (not clicked in this run). Never click them; type the beat 1 sentence. They work after `just dev clean seed-library`.
2. **A generated app shows "Retrieved from library · Seeded definition" after a reload** (APP-135, `query/canvas.rs` `assemble`). Verified: after a reload, Vendor Contract Tracker read "Retrieved from library" and "Seeded definition" beside "Generated". Don't reload Alex's window between building and showing; a reset (checklist step 2) starts clean.
3. **The generator adds an approval step to simple lists** (APP-135; APP-132's reading list). In one of two runs, the vendor contracts card read "1 waiting for your approval" with Expire and Cancel buttons after the first record; in the other, "Nothing waiting for your approval". Don't comment on it.
4. **A money field in a generated form takes minor units** (new, 2026-10-31). Typing `7200` in a generated "Annual value" field stored $72.00: the record form sends the text as `minor_units` (`app/src/commands/routes.rs` `typed_value`). That's why the beat 1 sentence has no money field. Ticket it under APP-135.
5. **"The share with Sarah Chen was refused; the record is filed."** appears after every claim Diego files on the dev stand. The Expenses work is the library's seeded work; `grant-library-access` already gives Sarah read and write on it, and Diego isn't its creator, so the share is refused (see the note in `app/src/dev/seed.rs`). Sarah sees the claim anyway, but only if Expenses is already on her canvas (checklist step 6). Don't read the line aloud.
6. **Diego sees Approve and Reject on his own claims.** Clicking one is refused by the server (`refused_by_schema`, verified). Don't click them in Diego's window.
7. **Rehearsed Ask ignores conditions it doesn't model** (new, demo organization only, APP-098). "? @expenses which claims over $100 are still waiting for approval?" answered with every waiting claim, including the $42.50 one: the rehearsed path matched "waiting" and dropped "over $100". Use only the exact questions above. A question that starts with *why*, *explain*, *summarize* or *describe* always goes to the model.
8. **`/expenses $86 Nobu, Sarah approves` is not rehearsed**: it goes to the model (2.3 s) and files category "other". The long rehearsed sentence files "meals".
9. **The header reads "Synthetic organization 0001"** (0002 on the clean instance) and the avatar reads "00" on the dev stand, not "Northstar" or initials. Crop the header if it matters on film.
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
