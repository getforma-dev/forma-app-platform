# Forma pitch MVP: guide, highlights and definitions

This is the one page to read before using, demoing or operating the pitch build on **beta.getforma.dev**. The scene-by-scene demo script is [SCENARIOS.md](SCENARIOS.md). The deploy procedure is [DEPLOY-RUNBOOK.md](DEPLOY-RUNBOOK.md). The filming plan is [RECORDING.md](RECORDING.md). The investor narrative is [PITCH-SCRIPT.md](PITCH-SCRIPT.md). How people, agents and AI connect (bring your own AI or use ours) is [CONNECTIONS.md](CONNECTIONS.md).

## What it is, in one paragraph

Forma is a canvas where teams describe what they need and get working apps: expense approvals, security investigations, budgets. The apps run on the team's real (here: synthetic) data with proof attached to every number, and they are shared with teammates and update live for everyone. The AI picks an app from a prebuilt library or builds one, and it can reshape that app to the request. The AI never gets to run code, write SQL or grant access: everything it produces is typed data that the platform checks before use. The same governed apps are reachable from Slack and from any AI agent (Claude, OpenAI) through MCP.

## Highlights (what to show)

1. **Instant pick, then a live reshape.** "Review expenses over $200" lands the expense app at once. The model then adds the `Amount > $200` filter in under a second, labeled *adapted*.
2. **Brain dump to approval.** "I spent $86 on a client dinner at Nobu, Sarah approves my expenses" becomes a typed draft. You confirm it, it is filed and shared, and Sarah's canvas shows it live. She approves, and your canvas flips.
3. **Receipt photo to a row.** Drop a receipt and the exact row to be added appears before anything is written.
4. **Slack.** `/forma …` files the expense, and Sarah gets Approve/Reject in a direct message.
5. **Any agent.** Claude (or OpenAI) acts through Forma's MCP server under the member's own permissions, marked "via Claude".
6. **Your canvas, your way.** Bring cards from the library pane, stack and shuffle them, and shape widgets in words ("make it a TI-84 in Matrix style").
7. **Proof on every number.** Each governed figure carries a receipt (`Verified …`) that identifies the source revision and query.
8. **Folders: your files become apps.** Drop PDFs and spreadsheets into a folder, click it, and say "build a metrics app and explain it". The CSVs are parsed and typed at once, the app is built from them, labeled "Built from folder", and its summary's numbers each come from a receipt that names the file.

## Getting in

| What | Where or who |
| --- | --- |
| Sign-in | **https://beta.getforma.dev/signin/northstar** (readable alias, after the next deploy) or the original Northstar link, `https://beta.getforma.dev/signin/<Northstar handle>` (the handle is in `forma-app-platform/evidence/APP-009/handles.json`). The bare `/signin` page only says "open your organization's sign-in link". |
| Diego Ramos (files expenses) | Laptop, Victor's getforma.dev account. |
| Sarah Chen (approves) | Phone, Victor's outlook account. |
| Slack | Workspace "Forma Beta", app "Forma". Each persona has run `/forma link` once. |

Signing out of the canvas also ends that person's agent tokens and Slack link. Relink after a sign-out.

## Using the canvas

The full rules (what is on a canvas, names and aliases, verbs, the Ask and Shape jobs, adding and removing, shortcuts) are in [CANVAS-RULES.md](CANVAS-RULES.md).

**Start from your data (APP-124).** An empty canvas offers three ways in: **Describe what you need**, **Browse your library**, and **Drop files or upload** (a new folder holds the files). A folder with files offers **Ask** (a read-only question about its tables, each answer row with its file and row), **Make a card** (a minimal app with one card, the most useful view of its data, linked back to the folder) and **✨ Make an app** (the full formation); both Make buttons confirm first. Details: [CANVAS-RULES.md](CANVAS-RULES.md#start-from-your-data-app-124).

**Intent bar (bottom).** One sentence to learn: **Type `/` and a verb to change the canvas, `/` and an app's name to tell that app something, or just a sentence.**

**The model (APP-120).** The canvas holds things; apps are the only governed thing.

- **Apps** (governed) contain **cards**, their views. A card never exists without its app. It is addressed `app.card` (`expenses.ledger`) or by its pane number (`3.2`).
- **Widgets** (one-off, yours, no governed data): Weather, Calculator, Clock, Note and Folder. A folder holds files; its one power, formation, makes an app that links back to it. A note keeps its private storage but is a widget to you.
- **Arrangements** (layout only): stacks, unite frames and focus. They are never listed in the Library.

**Verbs: seventeen, in six groups** (APP-121 added `/rename` and `/alias` to Make; APP-122 added the Shape and Ask groups; APP-123 added `/forma` to Make). They come from one registry (`canvas/command-registry.ts`). `/` alone lists exactly these, each with its signature (`<required>`, `[optional]`). From the first letter (`/e…`), the matching verbs come first, then a separate section, "Apps — type /name and what you want": the matching apps on your canvas and your saved apps (an app's exact name or alias is always offered, even on a clean canvas). After `/name `, a hint row marks the argument being typed and shows an example. Tab fills a suggestion, Enter runs it, ↑/↓ move.

| Group | Verb | Does |
| --- | --- | --- |
| Place | `/app <app name or number>` | Places every card of an app (`/app expenses`, `/app 3`). A bare `/<address>` does the same (`/expenses`). |
| Place | `/card <app.card or number>` | Places one card (`/card expenses.ledger` and `/card 3.2` are the same card). |
| Place | `/widget <weather\|calculator\|clock\|note\|folder> [args]` | Places a widget (`/widget weather Austin`, `/widget note Call the auditor`). `/widget folder` opens the folder picker. |
| Arrange | `/arrange` | Lays the board out by app. |
| Arrange | `/unite <app>`, `/split <united app>` | Assembles an app's cards into one frame; breaks it apart again. |
| Arrange | `/focus [app.card or number]` | Opens the selected card (or the one named) in focus. |
| Make | `/forma <words>` | Exactly a plain sentence: picks or builds an app from the words, behind its confirm card (APP-123). |
| Make | `/<folder> <words>` · `/folder:<name> <words>` | Builds an app from a folder's files (`/q3 build a metrics app`, `/folder:q3 brief me`; the folder's name in lowercase with hyphens, or its number). The long form `/folder <name> formation <instruction>` still works. `/folder` alone opens the picker; `/folder new <name>` makes one. A verb or an app of the same name answers first; the full form `/folder:<name>` always reaches the folder (APP-123). |
| Shape | `/shape <app.card\|app\|widget> <words>` | Changes how a card, an app (a theme on every card) or a widget looks or what it shows: a preview, then **Apply** (or **Add as new card**), labeled *shaped* with **Revert** (APP-122). `/<app>.<card> <words>` is the shortcut for a card (`/expenses.ledger only red invoices`). |
| Ask | `/ask [app\|app.card] <question>` | Opens the Ask tab with the question: a read-only answer with receipts; it changes nothing (APP-122). Name an app or card first to ask about it, or `@`-mention it anywhere (`? @expenses.total why is it high?`, APP-123); `? <question>` is the quick form. |
| Help | `/apps [query]` | Opens the Library, filtered. It never places a canvas card. |
| Help | `/commands [search]`, `/shortcuts`, `/agent` | The pane's Commands tab, its Shortcuts tab, and "Connect an agent". |
| — | Any other sentence | The instant pick and live reshape; a short chooser when two apps fit equally; "Build something new" generates one. |

**The four jobs.** Where you type picks the job, never the AI: **Ask** (`/ask`, `?`) finds out and changes nothing; **Shape** (`/shape`, the dot) changes how something looks or what it shows, never records; **Do** (`/<app> <words>`) runs an app's own command, the only way records change; **Forma** (a plain sentence, `/forma <words>`, a folder's formation `/<folder> <words>`) makes something new.

**How to ask.** Ask answers questions about your data in the right pane's **Ask** tab, in text, with the records it used and their receipts.
- `/ask expenses why did spend jump in March?` asks about the Expenses app (all its records); `/ask expenses.ledger which claims are overdue?` (or `/ask 3.2 …`) about that card's records only. The full forms `/ask app:expenses …` and `/ask card:expenses.ledger …`, and your own shortcuts (`/ask e …`), do the same.
- `/ask why is this high?`: when the first word is not one of your apps or cards (and a verb never is), the whole line is the question, about the selected card, else the canvas. `? <question>` is the quick form of that.
- `@` scopes a question anywhere in it (APP-123): `? @expenses.total why is it high?`, `? why so many @app:expenses`, `@card:expenses.ledger …`, your own shortcut (`@q3`). One mention per question; a widget or folder has no records to ask about.
- `?` on the canvas puts `? ` in an empty bar; ⋯ → **Ask about this** on a card opens the tab scoped to it; the tab has its own input. A chip shows the scope; its **×** removes it, and the conversation is about the whole canvas again (APP-123).
- Every answer has **Pin to canvas**: a read-only answer card labeled "As answered on <date>", with the question, the text and every record with its receipt. It never runs again and its text is not derived again (ask again for today's answer); it is yours alone, stays across sessions while you can still read the apps it cites, and has **Remove** (APP-123).
- An answer whose records come from one list has **Make it a card**: a preview of a new card showing exactly those records (each with its receipt), with **Add as new card** or **Cancel**. Nothing is added until you click (APP-123).
- It never changes records, cards or the canvas. A count or total lists every record it counted. Only the newest 200 records of each app are read; when there are more, the answer says so and never presents a partial total as the total.

**How to shape.** Shape changes the look or the view, through a preview you apply.
- A card: `/shape expenses.ledger only red invoices`, the dot shortcut `/expenses.ledger only red invoices`, or its full form `/card:expenses.ledger only red invoices`. An app: `/shape expenses dark green, compact` (a theme on every card; `/shape app:expenses …`). A widget: `/shape calculator make it a TI-84` (`/shape widget:calculator …`), or the widget's own **Shape** field, which applies at once.
- ⋯ → **Shape…** on any card, app frame or widget puts `/shape <its address> ` in the bar.
- What comes back: a preview with **Apply**, **Add as new card** (cards) or **Cancel**; applied, the item is labeled *shaped* with **Revert**. A shaped card stays live as its records change. Records never change, and every number keeps its receipts.
- **Add as new card** stores the card for you ("Made by you from /expenses.ledger · only you see it", APP-123): it belongs to its app, comes back on every visit and stays live over its records. **Remove** (click twice) deletes it, audited. The app's owner or an admin sees **Add for everyone** on their own cards: every member who can read the app's data then sees it ("added for everyone by the app's owner"), and **Keep for me only** takes it back; on someone else's card added for everyone they see **Keep for its maker only** and **Remove**. A card shows only while you can read its app's data. You keep at most 100 cards and pinned answers.

**Why not `/expenses:ask`?** The colon always means *kind:name* (`/app:expenses`, `/card:expenses.ledger`, `/widget:calculator`, `/folder:q3`): it names a thing, never an action. Jobs are verbs and come first: `/ask expenses …`, `/shape expenses …`.

**Aliases: nothing breaks.** These are accepted and autocompleted from their first letter but not listed under `/` alone: `/weather`, `/calculator`, `/clock` and `/note` (= `/widget <kind>`), `/help` (= `/commands`), `/folder` alone (the picker), and every app's old name (`/expense`, `/pto`, `/purchase`, …). There is no `/remove`: the card's ×, Delete on a selection, or the Library's Remove take things off (as in Figma and Miro).

**Addressing an app.** `/<address> [words]` tells that app something: the words tailor it or, when they file or decide, file or decide in that app. `/expenses $86 at Nobu, client dinner, Sarah approves` files the claim; `/expense …` still does exactly the same. A saved `/big-claims …` opens its saved work. The confirm card names the governed command ("Submit claim", "Approve claim: …") and says which app it goes to ("via /expenses").

**The address rule.** An app's address is its title, lowercased and hyphenated: Expenses is `/expenses`, Time off `/time-off`, Purchase requests `/purchase-requests`. Every name an app answered to before stays as a hidden alias forever (`/expense`, `/pto`, `/purchase`). The address shows wherever the app does, as a small grey chip: on its Library row (with "also /expense"), on its app card on the canvas, in the bar's hint ("/expenses — Expenses · file a claim or approve one") and on the confirm card. Click a chip to put `/name ` in the bar. Expense claims is shown as "Expenses" and the review queue as "Expense review (demo data)" (the catalog's display title; the stored definitions are unchanged).

The hint after an address comes from the app's definition: the server derives it from the create command's fields (status and initial fields left out), plus "… or approve/reject" when the app has decisions. An app that keeps no records answers to "what to show". The examples come from the catalog's intent examples. The six demo workflow apps keep their hand-written hint and example (the catalog's `command.hint` override).

**Workflow apps (APP-107).** Each address places its app; with text after it, files or decides in that app.

| Address | Does |
| --- | --- |
| `/expenses` (`/expense`, `/claim`) | Expense claims: "$86 at Nobu, client dinner, Sarah approves". |
| `/time-off` (`/pto`, `/leave`, `/vacation`) | Time off: "Nov 24–26 for Thanksgiving, Sarah approves". |
| `/purchase-requests` (`/purchase`, `/purchases`, `/po`) | Purchase requests; over $5,000 the manager approves, then finance. |
| `/vendor-invoices` (`/invoice`, `/invoices`, `/payables`) | Vendor invoices; a PO that does not match its purchase request is flagged. |
| `/access-requests` (`/access`, `/access-request`) | Access requests; the system owner grants or denies. The access review tracker is `/access-review-tracker` (`/access-review`). |
| `/deal-desk` (`/discount`, `/discounts`) | Deal desk; 10% or less approves itself by the app's rule. |

**The right pane: Library | Commands | Ask | Shortcuts.** One pane, four tabs. `L` and `/apps` open it on Library. `/commands` and `/help` open it on Commands. `/ask …`, `?` first in the bar and ⋯ → Ask about this open it on Ask (APP-122: `?` now means Ask). `/shortcuts` and ⋯ → Shortcuts and commands open it on Shortcuts. `L` on another tab switches to Library; `L` on Library closes the pane. The × and Esc close it (Esc clears a search first), and focus returns to what opened it. The tabs are ARIA tabs (←/→, Home, End). At phone width the pane is the bottom sheet, with the tabs on top.
- Library: two sections. **Apps** (Category → App → numbered cards) and **Widgets** (Weather, Calculator, Clock, Note, Folder; the Note and Folder rows expand to your notes and folders). Every row shows an on-canvas dot (filled when it is on the canvas), its name in grey (an app's `/address` chip with "also /…", a card's `app.card`, a widget's `/weather`) and an Add/Remove toggle; app rows read "n of m cards". The pane is a pure projection of the board: it re-renders from one `canvas:changed` event after every add and remove.
- An app row also shows its hint and an example; a click on the example puts it in the bar. "Stack" brings an app as a stack; search finds by name, number, address, alias, hint or `app.card`.
- Commands: the seventeen verbs in their six groups, with arguments, aliases, examples and a search (Ask reads `/ask [app|app.card] <question>`, with `? where are the red invoices` among its examples); the footer reads "Your apps answer to /name — see /apps".

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
- `?` on the canvas puts `? ` in an empty bar, ready to ask (with something half-typed, it only focuses the bar); `?` first in the bar opens the Ask tab (APP-122). `/shortcuts` (or ⋯ → Shortcuts and commands) opens the pane's Shortcuts tab: every action with its click path and its keys or /command, and a link at the top to the Commands tab, where every command has its arguments.

**Folders (APP-108).** Lead with click → type:

1. **Click** "+ Folder" (toolbar or the Library's Widgets → Folder row), drop files on empty canvas, or **Drop files or upload** on an empty canvas: a folder opens with its name ready to edit (click the title any time to rename). Files brought from the empty canvas just land; the folder's buttons are the next step (APP-124).
2. **Drop** PDFs, images, CSV, XLSX or text onto it, or use "Add files" (up to 20 files, 20 MB each). A CSV or XLSX is read at once and shows its column types and first rows; adjust any type, or keep the defaults ("Keep as card" places a data card with receipts).
3. **Click the folder** to make it active: its chip appears in the bar ("📁 Q3 board pack ×") and plain sentences now act on it: "build a metrics app and explain it", "brief me". × or Esc clears it.
4. **Buttons** (with files): "Ask" (the Ask tab scoped to the folder's tables, APP-124), "Make a card" (one card, after its confirm, APP-124), "✨ Make an app" (formation: an editable prompt, prefilled "Build an app that shows the key metrics and explain them") and "Brief me" (an overview, a paragraph per file and key points; PDF statements cited "file, p. N").
5. **@-mention** a folder in any sentence: "build a metrics app from @Q3 metrics".
6. **Commands last**, for power users: `/<folder> <instruction>` (`/q3 build a metrics app`, APP-123) and its full form `/folder:<name> <instruction>`, the long form `/folder <name|number> formation <instruction>` (the Make verb), `/folder` (picker), `/folder new <name>`, `/folder <name|number>`, `/folder uploads`; aliases `/storage`, `/box`, and `/widget folder`. In an Ask line (`? …`, `/ask …`) an `@` mentions an app or a card, never a folder.

A CSV dropped on the canvas or the bar outside a folder goes to your "Uploads" folder and becomes a data card. Apps built from a folder show "Built from folder: <name> · N files" (click it to open the folder) and a Summary card ("AI summary · numbers from receipts"); a receipt lists the source files. Sharing such an app gives the teammate exactly the files it was built from.

**Connect an agent.** The header button mints a token, shown once, and gives the setup lines for Claude Code or OpenAI.

## Local dev stand

The whole product on your laptop is one command from the checkout: **`just dev`** (Diego, Sarah and Priya, already signed in, at port base +1, +2, +3: http://127.0.0.1:4001/canvas, :4002, :4003 in the main checkout) and **`just dev clean`** (a new customer, Alex Rivera, at port base +10, :4010: no library, no records, demo mode off; `just dev clean seed-library` installs the template apps). It runs the `genui-dev` binary (the binary's own `product::routes`) under process-compose over this checkout's Compose PostgreSQL; data survives restarts. A front-end edit shows on the next page load with no command, a Rust edit is `just restart`, `just dev [clean] reset` starts over, `just dev [clean] stop` stops it. A worktree gets its own port base (in its `.env.dev`), so checkouts run side by side. Decided in [ADR-012](../architecture/ADR-012-DEVELOPER-WORKFLOW.md) D12.1-D12.2; the full procedure moves to `docs/DEVELOPMENT.md` (ADR-012 plan step 6).

Real versus beta: sign-in is minted, not fAuth (each persona's port adds its session cookie; each browser gets its own session); synthetic data only; objects in a local directory, not S3; the model only when `ANTHROPIC_API_KEY` is in the clone's `.env` (read into the server process only, never printed); Slack only with `SLACK_BOT_TOKEN` and `SLACK_SIGNING_SECRET` in your shell; CSV/XLSX parsing only after `just parser`.

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
