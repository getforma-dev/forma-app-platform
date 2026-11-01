# APP-096 rehearsal log (local)

Documentation rehearsal of [PITCH-SCRIPT.md](PITCH-SCRIPT.md) section 5 and [RECORDING.md](RECORDING.md). No application code changed. Nothing was deployed. APP-096's beta acceptance stays open.

## Environment

| Item | Value |
| --- | --- |
| Commit | `af63feba41aa825a972dde354e0a5d2195f7d608` (`origin/pivot/pitch-mvp`) |
| Checkout | Worktree `forma-pitch-rehearsal`, its own `.env.dev`, volumes, and `just setup` |
| Port base | 7500. Diego `http://127.0.0.1:7501/canvas`, Sarah `:7502`, Alex `:7510` |
| Left alone | Listeners on 127.0.0.1:4001–4003 and :4010 were not reset or restarted |
| Model | Server log: `genui-dev: model Anthropic` (key read from the main checkout's gitignored `.env`; not printed) |
| Browser | Playwright 1.49.1, Chromium, 1440×900. A real page. No mocked model output |
| Parser | `just parser` in this worktree, before the servers that served the CSV |
| When | 2026-10-31 local. Server timestamps are 2026-11-01 UTC |

Diego and Sarah are separate listeners and separate principals. Diego's confirm names Approver Sarah Chen (`00000000-0000-4000-8000-00000000006d`). Alex is organization 0002 with `data-demo` empty. Diego and Sarah are organization 0001 with demo on and a "Synthetic data" chip. A second tab on one port was not used as a second member.

## What was verified

| Step | Input | Observed |
| --- | --- | --- |
| New customer | `Track our vendor contracts: vendor, owner and renewal date` on Alex | `POST /v1/compose` 200, origin `generated`, title **Vendor Contracts**, `duration_ms` 21524. Form: Vendor, Owner, Renewal date, button **Add contract**. Chip: Generated for your request (typed definition, admitted). |
| First record | Figma / Alex Rivera / 2027-03-01, then **Add contract** | Commits on click. "Committed. Record version 1, work version 1, commit 2." Row filed by Alex Rivera. No confirm card. |
| File | Drop `docs/pitch/folder-fixture/q3_pipeline.csv` on the canvas | Untitled folder, then title **Q3 pipeline** after Enter. Buttons Ask, Make a card, ✨ Make an app, Brief me. The file showed **Make data** at first. The library later lists the folder as Q3 pipeline, 1 file. |
| Ask the file | Ask tab: `which open deals are worth more than $50,000?` | "There are 4 open deals worth more than $50,000. These are: Cobalt Health at $94,500 in proposal stage, Delta Freight at $51,000 in prospect stage, Ember Foods at $76,500 in negotiation stage, and Helio Energy at $99,500 in prospect stage." No `q3_pipeline.csv · row N` citation in that text. |
| Card from the file | **Make a card**, then **Make card** | Server created generated app "Q3 pipeline" at `/q3-pipeline-card` with chart "Amount by close date". The summary card and "Built from folder" were not re-read on screen. |
| Library pick | `Review expenses over $200` on Diego | Origin `retrieved`, title Expense review queue, 526 ms. Canvas: **Kept the standard view.** Totals stayed 240 claims / $263K. Verified chip present. "Adapted to your request · Amount > $200" did not appear. |
| Do | `/expenses $86 Nobu, Sarah approves`, then **Submit claim** | Confirm in about 2.1 s: $86.00, Nobu, category other, submitted, Approver Sarah Chen, understood in 2148 ms. Status: "Filed: record v1, commit 6, audit 08894c3a. The share with Sarah Chen was refused; the record is filed." |
| Sarah sees it | Separate Sarah window, Expenses placed, records card loaded, then a reload in the first pass | Row "$86.00 Nobu other 2026-11-01 Submitted Diego Ramos" with Approve and Reject. |
| Approve | **Approve** on that Nobu row | Sarah's row became Approved. Waiting count went from 3 to 2. Diego's later claims card also showed Nobu Approved. |
| Ask | `? @expenses where are the red claims?` | "Asking about /expenses (all its records)". "2 records in expenses need attention … Delta, $389.00, submitted; Blue Bottle Coffee, $42.50, submitted." Receipt on each. Rehearsed. |
| Make and pin | **Make it a card** → **Add as new card**, then **Pin to canvas** | "Made by you from /expenses". "As answered on 2026-11-01 02:03 UTC · asked about /expenses · it does not run again". |
| Summarize | `? @expenses summarize where the money went` | Model text: "The money went to four expenses totaling $559.50. Travel was the largest category at $389.00 for a Delta flight. Meals accounted for $84.50…" Receipts on the response. A second question queued behind the first. |
| Shape the ledger | `/expenses.ledger only red invoices` | Filter status in submitted, rejected. "Only status is submitted or rejected". "/expenses.ledger is shaped. Revert is on the card." Stored title "Expense ledger (shaped)". |
| Shape the app | `/shape expenses dark green, compact`, then **Apply** | "Nothing changes until you apply it." Then "Every /expenses card is shaped; each keeps its own view. Revert is on the app card." |
| Alias | `/alias e expenses` | "/e now opens Expenses, for you only." |
| Fallback | `I spent $86 on a client dinner at Nobu last night, Sarah approves my expenses` | Confirm in 925 ms, category meals, "Understood in 745 ms". Cancelled. Not committed. |

## Not verified on this commit

- Beat 6 (MCP / Claude Code).
- Typed `approve the Nobu dinner`.
- A reload of the placed Vendor Contracts app, so the old "reload shows Seeded definition" pitfall is neither confirmed nor cleared. A new browser does not restore local canvas layout.
- The folder summary card, its **Verified** line, and "Built from folder: Q3 pipeline" on screen. The chart's address is in the library.
- ✨ Make an app timing.
- The minor-units money-field pitfall.
- A rehearsed Ask that adds "over $100".
- A sub-second live arrival on Sarah's already-open card. The first filing was visible on her records card after that card had loaded and after a reload. Two later filings (Muji, Ferry) were not visible on a freshly placed Sarah canvas within 20 seconds. Reload showed Muji. Wait until her claims list has rows before Diego files, and reload her window if the new row is missing.

## Failures worth remembering

- Generation is slower than the f0efc04 script (21.5 s, not 11.5 s) and the title is not stable ("Vendor Contracts", not "Vendor Contract Tracker").
- "Review expenses over $200" did not reshape the library app on this commit.
- The canvas form's **Add contract** writes immediately.
- The dev-stand share line still says the share with Sarah was refused. She still sees the claim because library access was already granted.
- Some Diego review cards showed "This card couldn't load · Try again" and were still failed in the filing screenshot.
