# Recording shot list (YC demo video)

How to film the demo in short, retakeable shots. The words to say are in [PITCH-SCRIPT.md](PITCH-SCRIPT.md). Every scene runs on **beta.getforma.dev**, signed in through the Northstar link.

## Setup, before the first take

| Item | Setting |
| --- | --- |
| Laptop | Chrome, 1440×900 window, zoom 100%, Diego signed in. Turn on **Show timings** (toolbar menu, or `#timing=1` (the canvas URL takes no query string), APP-116) for the scenes marked ⏱. |
| Phone | Sarah signed in, mirrored or filmed. Slack open as Sarah in a second tab or app. |
| Canvas | Starts clean (Library → Remove all, or a fresh browser profile). Lay out the scene's cards first with `/arrange`. |
| Claude Code | `.mcp.json` from "Connect an agent". `FORMA_AGENT_TOKEN` exported with `read -rs`, so the token never appears on screen. |
| Recording | Hide the bookmarks bar. Turn on Do Not Disturb. Keep the cursor highlight off unless the scene needs it. |

## Rehearsed versus live

Rehearsed sentences answer instantly inside the demo organization (APP-098). Use them for the story scenes so every take is identical. Run **at least one live, unrehearsed sentence on camera** with timings on: it proves the AI is real. Speed up waits only with a visible "2×" and the real time shown.

## Shots

| # | Scene | Device | What to do | Sentence or click | ⏱ | Time |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Clean canvas | Laptop | Open the library pane (`L`), then close it | — | | 5 s |
| 2 | Brain dump | Laptop | Type, then **Confirm** | *"I spent $86 on a client dinner at Nobu last night, Sarah approves my expenses"* | ⏱ | 15 s |
| 3 | Live approval | Phone, then laptop | Sarah taps **Approve**; cut to Diego's canvas flipping | — | ⏱ | 10 s |
| 4 | Instant pick and reshape | Laptop | Type; the app lands, then reshapes | *"Review expenses over $200"* | ⏱ | 10 s |
| 5 | Receipt | Laptop | Open the receipt on any number | click **Verified …** | | 5 s |
| 6 | Live AI (unrehearsed) | Laptop | Type something not in the script | e.g. *"show me support tickets by priority this week"* | ⏱ | 10 s |
| 7 | Receipt photo | Laptop | Drop the fixture receipt (`docs/pitch/receipt-fixture.png`) | — | ⏱ | 10 s |
| 8 | One workflow (Victor picks one) | Laptop and phone | Time off, purchase (two steps), invoice (PO mismatch), access, or deal desk (8% approves itself) | see SCENARIOS.md, "Rehearsed sentences" | ⏱ | 20 s |
| 9 | Slack | Laptop Slack, then phone Slack | `/forma` the sentence, **Confirm**; Sarah approves in her DM | *"I paid 42.50 for a Figma seat for the design team, Sarah signs off"* | | 20 s |
| 10 | Any agent (MCP) | Laptop: Claude Code | Ask, then show the canvas "via Claude" chip | *"What's waiting for my approval? Approve the Figma seat."* | ⏱ | 20 s (speed up ×2) |
| 11 | Agent governance (if APP-115 is live) | Laptop | An agent scoped to Engineering tries the time-off app; open the Agents activity panel to show the refusal | — | | 15 s |
| 12 | Folder and formation | Laptop | **+ Folder**, drop the 3 fixture files (`docs/pitch/folder-fixture/`), click the folder, type; then open a receipt to "Source file" | *"build a metrics app and explain it"* | ⏱ | 25 s |
| 13 | Brief | Laptop | **Brief me**; click a "p. 1" citation so the PDF opens at that page | — | | 10 s |
| 14 | Share | Laptop, then phone | Share the folder app with Sarah; she opens it live | — | | 10 s |
| 15 | Your canvas | Laptop | Stack three cards and shuffle; **Unite** an app (cards assemble); **F** to focus a card | — | | 15 s |
| 16 | Widgets and notes | Laptop | `/calculator`, then *"make it a TI-84 in Matrix style"*; then `/note Call the auditor Friday` | — | ⏱ | 10 s |

Total: about 3½–4 minutes before editing. Cut to fit the 4-minute demo section of the script.

## Retakes and resets

- **Fresh records:** file new ones with a different merchant or amount; old ones stay in the history. Approved records cannot be reopened (by design).
- **Pending item for Sarah:** keep one claim pending before scene 3 by filing it in shot 2 and not approving it until the take.
- **Canvas layout:** Library → Remove all, or a new browser profile.
- **Agent token:** if you sign out, tokens and Slack links end. Create a new token and run `/forma link` again before scenes 9 and 10.
