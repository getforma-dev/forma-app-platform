# Forma pitch MVP: guide, highlights and definitions

This is the one page to read before using, demoing or operating the pitch build on **beta.getforma.dev**. The scene-by-scene demo script is [SCENARIOS.md](SCENARIOS.md). The deploy procedure is [DEPLOY-RUNBOOK.md](DEPLOY-RUNBOOK.md). The investor narrative is [PITCH-SCRIPT.md](PITCH-SCRIPT.md). How people, agents and AI connect (bring your own AI or use ours) is [CONNECTIONS.md](CONNECTIONS.md).

## What it is, in one paragraph

Forma is a canvas where teams describe what they need and get working apps: expense approvals, security investigations, budgets. The apps run on the team's real (here: synthetic) data with proof attached to every number, and they are shared with teammates and update live for everyone. The AI picks an app from a prebuilt library or builds one, and it can reshape that app to the request. The AI never gets to run code, write SQL or grant access: everything it produces is typed data that the platform checks before use. The same governed apps are reachable from Slack and from any AI agent (Claude, OpenAI) through MCP.

## Highlights (what to show)

1. **Instant pick, then a live reshape.** "Review expenses over $200" lands the expense app at once. The model then adds the `Amount > $200` filter in under a second, labeled *adapted*.
2. **Brain dump to approval.** "I spent $86 on a client dinner at Nobu, Sarah approves my expenses" becomes a typed draft. You confirm it, it is filed and shared, and Sarah's canvas shows it live. She approves, and your canvas flips.
3. **Receipt photo to a row.** Drop a receipt and the exact row to be added appears before anything is written.
4. **Slack.** `/forma …` files the expense, and Sarah gets Approve/Reject in a direct message.
5. **Any agent.** Claude (or OpenAI) acts through Forma's MCP server under the member's own permissions, marked "via Claude".
6. **Your canvas, your way.** Bring cards from the library pane, stack and shuffle them, and restyle widgets in words ("make it a TI-84 in Matrix style").
7. **Proof on every number.** Each governed figure carries a receipt (`Verified …`) that identifies the source revision and query.
8. **Folders: your files become apps.** Drop PDFs and spreadsheets into a folder, click it, and say "build a metrics app and explain it". The CSVs are parsed and typed at once, the app is built from them, labeled "Built from folder", and its summary's numbers each come from a receipt that names the file.

## Getting in

| What | Where or who |
| --- | --- |
| Sign-in | The **Northstar sign-in link**, `https://beta.getforma.dev/signin/<Northstar handle>` (the handle is in `forma-app-platform/evidence/APP-009/handles.json`). The bare `/signin` page only says "open your organization's sign-in link". |
| Diego Ramos (files expenses) | Laptop, Victor's getforma.dev account. |
| Sarah Chen (approves) | Phone, Victor's outlook account. |
| Slack | Workspace "Forma Beta", app "Forma". Each persona has run `/forma link` once. |

Signing out of the canvas also ends that person's agent tokens and Slack link. Relink after a sign-out.

## Using the canvas

**Intent bar (bottom).** Type a sentence, or `/` for commands.

| Command | Does |
| --- | --- |
| `/apps` | Shows every app you can open. |
| `/expense` | Opens the expense ledger. `/expense <text>` files or decides an expense. |
| `/card <name or number>` | Brings one card (for example `/card 6.2`). |
| `/app <name>` | Brings all of an app's cards. |
| `/arrange` | Lays the board out by app. |
| `/unite <app>`, `/split <app>` | Assembles an app's cards into one frame; breaks it apart again. |
| `/note <text>` | Keeps a quick note as a card (your own record; edit in place, saved when you leave it). |
| `/weather <place>`, `/calculator`, `/clock <zone>` | Utility widgets; restyle them in words. |
| Any other sentence | The instant pick and live reshape; a short chooser when two apps fit equally; "Build something new" generates one. |

**Workflow commands (APP-107).** Each opens its app; with text after it, files or decides in that app.

| Command | Does |
| --- | --- |
| `/pto` (`/time-off`, `/leave`, `/vacation`) | Time off: "Nov 24–26 for Thanksgiving, Sarah approves". |
| `/purchase` (`/purchases`, `/po`) | Purchase requests; over $5,000 the manager approves, then finance. |
| `/invoice` (`/invoices`, `/payables`) | Vendor invoices; a PO that does not match its purchase request is flagged. |
| `/access` (`/access-request`) | Access requests; the system owner grants or denies. The access review tracker is now `/access-review`. |
| `/discount` (`/deal-desk`, `/discounts`) | Deal desk; 10% or less approves itself by the app's rule. |

**Library pane (right, `L`).**
- Organized as Category → App → numbered cards.
- Click to add or remove a card. "Add all" brings a whole app, "Stack" brings it as a stack, and search finds by name or number.

**Stacks.**
- Making one: drag a card onto another and hold, or select several cards and press Cmd/Ctrl+G.
- Shuffling: Shift+scroll or ←/→.
- Fanning out: click the count, or press Enter. Esc folds it back.
- Spreading back onto the board: Cmd/Ctrl+Shift+G.

**Focus and unite.** The `[ ]` button on a card (or `F`) opens it in a focus modal where ←/→ or a swipe walk the app's cards and Esc returns; "Unite" on the app card, the library row, `/unite` or `U` assembles the app's cards into one frame (KPIs, chart, table, form and records beside them), and "Break apart" or Shift+U puts every card back.

**Controls (as in Figma).**
- Scroll pans. Cmd/Ctrl+scroll or a pinch zooms at the cursor. Space+drag or middle-drag pans.
- `V` move and `H` hand, plus the zoom menu.
- Shift+0 / 1 / 2 zoom to 100% / fit / selection.
- Click a header to select, Shift+click to add, and drag to move.
- Delete returns a card to the library; it never deletes data.
- `?` lists all shortcuts.

**Folders (APP-108).** Lead with click → type:

1. **Click** "+ Folder" (toolbar or the library pane's Folders group), or drop files on empty canvas: a folder opens with its name ready to edit (click the title any time to rename).
2. **Drop** PDFs, images, CSV, XLSX or text onto it, or use "Add files" (up to 20 files, 20 MB each). A CSV or XLSX is read at once and shows its column types and first rows; adjust any type, or keep the defaults ("Keep as card" places a data card with receipts).
3. **Click the folder** to make it active: its chip appears in the bar ("📁 Q3 board pack ×") and plain sentences now act on it: "build a metrics app and explain it", "brief me". × or Esc clears it.
4. **Buttons**: "✨ Formation" (an editable prompt, prefilled "Build an app that shows the key metrics and explain them") and "Brief me" (an overview, a paragraph per file and key points; PDF statements cited "file, p. N").
5. **@-mention** a folder in any sentence: "build a metrics app from @Q3 metrics".
6. **Commands last**, for power users: `/folder` (picker), `/folder new <name>`, `/folder <name|number>`, `/folder uploads`, `/folder <name|number> formation <instruction>`; aliases `/storage`, `/box`.

A CSV dropped on the canvas or the bar outside a folder goes to your "Uploads" folder and becomes a data card. Apps built from a folder show "Built from folder: <name> · N files" (click it to open the folder) and a Summary card ("AI summary · numbers from receipts"); a receipt lists the source files. Sharing such an app gives the teammate exactly the files it was built from.

**Connect an agent.** The header button mints a token, shown once, and gives the setup lines for Claude Code or OpenAI.

## Operating it (operator only)

- **Deploy:** follow [DEPLOY-RUNBOOK.md](DEPLOY-RUNBOOK.md). It covers building and qualifying the three images, pushing them, preflight, the reviewed change set, operator tasks and activation. Every AWS change needs Victor's word, recorded in the private approvals file.
- **Secrets:** keys live only in the clone's gitignored `.env`. They go into AWS Secrets Manager by piping from that file, are never printed, and are never put in chat or in a commit. After a secret changes, force a new deployment of the service.
- **Health:** `/health/live` returns 200. Every protected route answers the same 404 when signed out.
- **Rollback:** each build refuses any schema but its own (the live database is at schema 11), and a replaced task definition cannot be redeployed. Fix forward by default; the other paths are in the runbook's "Rollback" section.
- **Known shortcuts and limits:** every mock, hard-coded path and pitch limit is in [APP-098](../../Tickets/0-backlog/APP-098.md) (DS-01 onward). The demo shortcuts only run inside the demo organization. Everything after a parse is real.

## Definitions

| Term | Meaning |
| --- | --- |
| **App** | One typed definition: its data (sources and records), its views, its actions (commands such as *submit* or *decide*), and who may use it. It has revisions and a publication. |
| **Work (saved work)** | A member's own copy of an app, bound to their data. It is what gets opened, shared and permissioned. |
| **Folder** | A member's files in one card (internally a "box"): PDFs, images, CSV/XLSX and text, kept as immutable objects. A confirmed CSV/XLSX becomes a registered source revision; apps built from the folder record which files they used. |
| **Formation** | The AI keyword for a folder: everything after it is the instruction (build an app, brief me). |
| **Card** | One view of an app on the canvas: a number, chart, table, records list, form or ledger. Apps have several cards. |
| **Widget** | A stand-alone utility card (weather, calculator, clock) with no governed data. |
| **Library** | The prebuilt apps an organization can retrieve instantly (14 in Northstar). |
| **Retrieved / adapted / generated** | How an app reached the canvas: as-is from the library, reshaped from a library app, or newly built by the model. The label is always shown. |
| **Instant pick (matcher)** | Local, model-free choice of a library app from the request (about 4 ms). |
| **Transform** | The model's typed reshape of an app (filter, window, grouping, columns, chart, sort). It is checked against the app's contract and admitted before use. |
| **Admission** | The gate every definition passes before it can run. Model output enters only here, as data. |
| **Receipt** | Proof attached to a governed number: which source revision and which query produced it. |
| **Record / command** | Typed rows (for example an expense claim) changed only by audited, idempotent commands. |
| **Share** | Grants a teammate the sharer's access to a work. The teammate's canvas updates live. |
| **Live events** | The server stream (`/v1/events`). Each event is re-authorized for each viewer before delivery. |
| **Organization / tenant** | The isolation boundary. Every table enforces row-level security per organization. |
| **Demo organization** | The organization where rehearsed sentences parse instantly (Northstar on beta). Nothing canned runs anywhere else. |
| **Persona** | A demo member with a display name (Diego, Sarah). |
| **Agent token** | A member-minted bearer token for MCP clients. It never exceeds the member's own rights and is revocable. |
| **Slack link** | Binds a Slack user to a member through `/forma link`. A Slack id alone confers nothing. |
| **MCP** | Model Context Protocol: the open standard agents use to call tools. Forma serves eight tools at `/v1/mcp`. |
| **Folder** | A card that holds files (PDF, CSV, XLSX, images). `/folder formation <instruction>` has the AI form an app or a cited brief from it; every number and quote traces back to its file. |
| **Stack / fence** | A pile of cards you can shuffle, and a labeled area that groups cards. Fences are not built yet. |
| **fAuth** | Forma's identity service: hosted sign-in and the organization's identity provider. |
| **Forma Stack** | The Rust framework underneath: server-rendered FMIR pages plus registered canvas islands. |

## Where things live

| Thing | Path |
| --- | --- |
| Pitch code | `FormaBuiltApps/forma-pitch-mvp`, branch `pivot/pitch-mvp` |
| Decisions | `docs/architecture/ADR-011-NOV-2-EMERGENCY-PIVOT.md` |
| Tickets | `Tickets/` (APP-091 to APP-105; the deferred-work revisit is APP-097, the shortcut register is APP-098) |
| Demo script | `docs/pitch/SCENARIOS.md` |
| Slack setup | `docs/pitch/SLACK-SETUP.md`, `docs/pitch/slack-app-manifest.json` |
| Deploy | `docs/pitch/DEPLOY-RUNBOOK.md` |
