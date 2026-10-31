# Pitch scenarios: the five flows, the exact words, the two-window choreography

Status: built on `pivot/scenarios` (ADR-011; APP-093 action path, APP-094 library, APP-095 sharing and live updates, APP-098 demo shortcuts). Every number and every state change in these scenarios is real: the records are typed records committed through the audited, idempotent command path; the permissions are the share path's; the live updates are the event stream re-authorized at delivery; the receipts are the governed query path's. The only staged part is the parse of the rehearsed sentences inside the demo organization (APP-098 DS-38), and the member still confirms before anything is written.

Demo organization: Northstar (`00000000-0000-4000-8000-000000000001`), listed in `GENUI_DEMO_ORGANIZATIONS`, with `GENUI_DEMO_MODE=1`. Personas (synthetic display names from `GENUI_DISPLAY_NAMES_FILE`): **Diego Ramos** (employee), **Sarah Chen** (manager, approver), **Priya Natarajan** (security lead, the library's author). Outside the demo organization nothing canned applies.

## Setup (operator, once)

1. `seed-library --organization <Northstar>`: the 14 library apps, the **Expense claims** app among them (`contracts/fixtures/library/expense-claims.json`): record schema `expense_claim` (amount USD cents, merchant, category meals|travel|software|office|other, spent_on, note, status submitted|approved|rejected), commands `submit` (create) and `decide` (transitions submitted→approved, submitted→rejected only).
2. Grants (ADR-007 D7.3, operator): Diego and Sarah `discover` + `run` on the Expense claims app and `source_read` on its claims source; Priya is the seeded author.
3. `GENUI_DISPLAY_NAMES_FILE`: `{"members":[{"principal_id":"<Diego>","display_name":"Diego Ramos"},{"principal_id":"<Sarah>","display_name":"Sarah Chen"},{"principal_id":"<Priya>","display_name":"Priya Natarajan"}]}`.
4. Pre-existing claims so the canvas never starts empty: the library seeds one claim (Blue Bottle Coffee, $42.50) on Priya's work — `submitted` in the current fixture (APP-113: a create can no longer start approved); on beta, seeded before that fix, it stays approved; before the pitch, file two more as Diego through the canvas and decide them as Sarah (the campaign does exactly this: one approved Nobu dinner, one rejected Figma seat). Keep one pending for the live scenario or file it on stage.
5. Two browser windows, side by side: **left** Diego signed in at `/canvas`, **right** Sarah signed in at `/canvas`. Both show "Live" in the top bar (the event stream is open).

## The one sentence (APP-120)

Teach this first: **Type `/` and a verb to change the canvas, `/` and an app's name to tell that app something, or just a sentence.** The canvas holds things; apps are the only governed thing. Apps contain cards (`expenses.ledger`, or `3.2`); widgets (Weather, Calculator, Clock, Note, Folder) are yours and one-off; stacks, unite frames and focus only arrange. `/` alone shows the fourteen verbs (Place `/app` `/card` `/widget`; Arrange `/arrange` `/unite` `/split` `/focus`; Make `/folder <name> formation <instruction>`, `/rename`, `/alias`; Help `/apps` `/commands` `/shortcuts` `/agent`); every app answers to its `/name`, shown as a grey chip wherever the app appears. Every older command still works as a hidden alias (`/expense`, `/pto`, `/weather`, `/note`, `/help`).

## Scenario 1: employee expense → manager approval, live

**Left window (Diego).** Type in the intent bar, exactly:

> I spent $86 on a client dinner at Nobu last night, Sarah approves my expenses

(Or address the app: `/expenses $86 at Nobu, client dinner, Sarah approves`; `/expense …` does exactly the same.)

What happens (measured in the campaign, synthetic data): the status line reads "Understood in ~600 ms"; a **confirmation card** rises above the intent bar:

- Amount **$86.00** · Merchant **Nobu** · Category **meals** · Spent on **yesterday's date** · Note **a client dinner** · Status **submitted**
- the heading names the governed command: **Submit claim?**
- chips: *Expense claims* (the library app; the first time it is also "placed on your canvas"), **Approver: Sarah Chen**
- the note "Nothing is written until you confirm. The command is committed through the audited path with a server-minted key.", the footer "via `/expenses`" and the buttons **Submit claim** / Edit / Cancel.

Say: "Diego just said it. Forma turned it into a typed record under the app's schema, found Sarah by name in his organization and minted the command. Nothing has been written yet."

Click **Submit claim**. The status line reads "Filed: record v1, commit N, audit <8 hex>. Shared with Sarah Chen for approval." The **Expense claims** group appears (or pulses) on the canvas with its **Expense claims** records card: the Nobu claim at the top, status chip **submitted**, "Filed by Diego Ramos", and the card's provenance line (publication, definition digest, commit sequence, work version).

**Right window (Sarah).** Within a second, without a reload: the Expense claims group is placed on her canvas (`app.shared` on her stream, the group fetched under her own grants). Her records card reads **"1 waiting for your approval · N records"**; the Nobu row carries **Approve** and **Reject**.

Say: "Sarah never set anything up. The share granted her exactly Diego's subset — read, write on this work — and nothing else; a non-member gets a 404 that is byte-identical for an existing and a nonexistent app."

Either click **Approve** on the row, or type in her intent bar:

> approve the Nobu dinner

A confirmation card: *Approve claim: $86.00 · Nobu · a client dinner?* — Status **approved**, the record chip "Record: $86.00 · Nobu · a client dinner (v1)", **Approve claim**, "via `/expenses`". Click Approve claim. Her card refreshes: status chip **approved**, "0 waiting for your approval".

**Left window.** The Nobu row turns **approved** live (`record.committed` on Diego's stream re-read his records list), with a brief glow. Say: "That decision is an audited command: version 2 of the record, one `command.committed` event on the organization's hash chain, and the chain verifies."

**Close the beat with Unite (APP-106).** On the left window type `/unite expense` (or select one of its cards and press **U**): every Expense claims card flies into one frame titled with the app and its *retrieved* label (numbers in a row, the chart, the table, the form and records beside them), with the Nobu claim still showing **approved**; **F** on any card opens it in focus, ←/→ walk the app's cards, Esc returns; **Shift + U** breaks it apart. Say: "Those cards were never separate things. They are one governed app."

Measured on the local stand (Docker PostgreSQL, loopback model stub): parse-to-card ~0.6 s (deterministic for the rehearsed sentence), confirm commit ~0.15 s, share ~0.1 s, `app.shared` on Sarah's stream ~40 ms after the share, `record.committed` on Diego's stream well under a second. See `actions-campaign.json` in the evidence for the exact run.

## Scenario 2: the manager asks what is waiting

**Right window (Sarah).** Type:

> what's waiting for my approval

The answer is `open`: her Expense claims group is brought into view (placed if it was not on her canvas yet) and its records card leads with **"N waiting for your approval"**; the pending rows carry Approve / Reject. No model call for this sentence in the demo organization; outside it the model classifies it as `open`.

Say: "Every row here is a typed record she may read; the list is the record page's authorization, under row-level security, bounded and newest first."

## Scenario 3: security investigation retrieval

**Left or right window.** Type:

> Investigate outbound traffic from build-worker-17

The action path answers `place` (not a record action), and the canvas hands the sentence to the composer (`POST /v1/compose`). Library-first: the router matches **Outbound traffic investigation** (total bytes, bytes by destination, bytes per hour, largest transfers) with high confidence and instantiates it as a new saved work bound to the organization's registered traffic revision under the member's own `source_read`: origin chip **retrieved**. The cards run through the governed query path; each number carries its **Verified** receipt (digest, source revisions, principal, epochs). Click a receipt to open every field and the receipt document.

Say: "Nothing here was generated. It was retrieved from a library of admitted definitions and runs on this organization's data with a receipt on every number."

Honest note: on the local stand the traffic source bytes must be in the engine cache (`library_sources.py build`) for the cards to show values; the beta deploy places them (APP-096, DS-32).

## Scenario 4: a new team app, then share it

**Left window (Priya, or any member who owns an authoring app).** Type:

> Build a PTO request app: employees request days off with start date, end date and reason; managers approve or reject; show requests by status and by month

The action path answers `place`; the composer's router finds no library fit; the generation model (Sonnet by default) is asked for one `app_definition` content object under the closed grammar, which is parsed strictly, pinned to the member's exact source contracts and admitted whole; the result is drafted on the member's authoring app, owner-published and saved: origin chip **generated** (a refused output is retried at most twice carrying only the refusal code; three refusals end in a neutral "could not compose", never a rendered fragment). The canvas places the app with its views and its request form.

Share it: from the work page (`/w/{app}/{work}`), the share form posts `POST /v1/work/{app}/{work}/share` to a member of the organization chosen from `GET /v1/members` (display names where the operator set them). The teammate's canvas places the app live (`app.shared`); a non-member's requests stay the uniform 404.

Honest notes: generation needs the member to own an app with `edit_definition` and `run` (ADR-007 D7.3; DS-20); generated views are limited to the engine's qualified operators (DS-33); generation takes several seconds (budget 60 s per attempt) and must be rehearsed with the real key before the pitch (`app/scripts/composer_smoke.sh`).

## Scenario 5: the Slack act (APP-100)

The manager lives in Slack. Same records, same audited path, same live canvas; Slack is one more door into it.

Before the pitch (once, `docs/pitch/SLACK-SETUP.md`): Diego and Sarah each run `/forma link` in Victor's workspace and confirm the link page while signed in to `beta.getforma.dev` as themselves. A sign-out (which advances the member's epoch) ends the Slack link: re-link after one.

**Slack, Diego's window.** Type:

> /forma I spent $86 on a client dinner at Nobu last night, Sarah approves my expenses

Slack shows "Working on it…" at once (the acknowledgement); within about a second an ephemeral card replaces it, visible to Diego only: *File this in Expense claims?*, the typed values (Amount $86.00, Merchant Nobu, Category meals, Spent on yesterday, Note, Status submitted), "Approver: **Sarah Chen** · Nothing is written until you confirm", and **Confirm** / **Cancel**. Click **Confirm**. The card becomes "Filed: **$86.00 · Nobu · a client dinner**, sent to **Sarah Chen** for approval."

Say: "Diego never left Slack. The same parser, the same typed draft, the same audited command and share as the canvas."

**Slack, Sarah's DMs.** A DM from Forma arrives about a second later: "**Diego Ramos** filed a claim for your approval", the claim (amount, merchant, date, category, note, filed by) and **Approve** / **Reject**. On the canvas (right window) the Expense claims group arrives live as in scenario 1, with the row's **via Slack** chip.

Click **Approve**. The DM is replaced by "**Approved by Sarah Chen** · 2:14 PM" with no buttons. Diego gets a DM: "Your Nobu claim ($86.00) was **approved** by **Sarah Chen**." Both canvases flip the row to **approved** live, marked via Slack.

Say: "Sarah's click was judged as Sarah: her link, her current session epoch, her grant on this work. Anyone else clicking that button gets a refusal; clicking it twice changes nothing."

Alternatives on stage: Sarah can type `/forma what's waiting for my approval` (the pending list with Approve / Reject on each claim) or `/forma approve the Nobu dinner` (a confirmation card, then **Confirm**).

Measured on the local stand (`storage::slack_campaign`, Docker PostgreSQL, loopback Slack and model stubs, synthetic payloads): acknowledgement under 10 ms (and under 3 s with a 10-second model), command to card ~0.75 s, Confirm to "Filed" ~0.3 s, Confirm to Sarah's DM ~1.1 s, Approve to the replaced DM and Diego's DM ~0.6 s. The live rehearsal in Victor's workspace is still to be recorded (APP-100).

Honest notes: Slack reaches only `https://beta.getforma.dev` (or a tunnel); the "via Slack" chip is kept in process memory and disappears after a restart (DS-59); a link binds whoever confirms it (DS-60); the approver DM goes to linked members who can decide the claim (DS-61).

## Scenario 6: two vendors' agents on one governed record (MCP, APP-099)

Forma is an MCP server (`POST /v1/mcp`, Streamable HTTP, stateless). Any MCP client acts through it as the member who connected it: the same identity, permissions, receipts and audit as the canvas, and its work appears live on the team's canvases.

**Connect (once per person, before the pitch).** On the canvas, click **Connect an agent**, name it ("Claude" for Diego, "OpenAI" for Sarah) and **Create token**. The token is shown once; copy it into the client. Forma keeps only its SHA-256 digest; the panel lists connected agents and revokes them. Signing out of the canvas moves the member's epoch and ends every token of that member: mint after the last sign-in.

- Claude Code (Diego):

  ```sh
  claude mcp add --transport http forma https://<host>/v1/mcp --header "Authorization: Bearer <YOUR_FORMA_TOKEN>"
  ```

- OpenAI Responses API (Sarah), as a `tools` entry; the rehearsal client is `app/scripts/mcp_openai_smoke.py` (key from the gitignored `.env`, never printed; OpenAI calls the server from its own infrastructure, so the URL must be the beta host or a tunnel, never loopback):

  ```json
  {"type": "mcp", "server_label": "forma", "server_url": "https://<host>/v1/mcp",
   "headers": {"Authorization": "Bearer <YOUR_FORMA_TOKEN>"}, "require_approval": "never"}
  ```

**The act.** Left window: Diego's canvas; right window: Sarah's canvas; a terminal with Claude Code in front of the left window.

1. Diego, in Claude: *"Log my $86 Nobu dinner and send it to Sarah."* Claude calls `forma_list_apps`, then `forma_draft_record` (app "Expense claims", amount 86, merchant Nobu, category meals, approver Sarah): a draft, typed under the app's schema and dry-run by storage, nothing written. Claude shows the values and asks; Diego says yes; Claude calls `forma_commit`: one audited command (server-minted idempotency key, `command.committed` on the chain), then the share to Sarah.
2. Right window: the claim arrives live (`app.shared`, then her records card), with a **via Claude** chip on the row.
3. Sarah, in an OpenAI agent: *"What's waiting for me? Approve the Nobu dinner."* `forma_list_pending` → `forma_draft_decision` → `forma_commit`.
4. Left window: Diego's row flips to **approved** live (`record.committed`), **via OpenAI**.

Say: "Two vendors' agents just acted on one governed record. Neither got a right Diego or Sarah does not hold; every write was confirmed, typed, audited and idempotent, and both canvases watched it happen."

Tools (closed set, typed JSON Schemas, annotations): `forma_list_apps`, `forma_run_view` (rows ≤ 50 with the receipt id, the canvas's query path), `forma_compose_app` (the composer; the app is placed live), `forma_draft_record`, `forma_draft_decision`, `forma_commit` (held drafts: 10 minutes, the member's and token's own, one record each; a repeated commit replays the same outcome), `forma_list_pending`, `forma_share`. Refusals: a missing, malformed, unknown, revoked, foreign or epoch-stale token and a cookie without a bearer are all the byte-identical 404; a foreign `Origin` is 403; inside a tool, every denial is the same `not_found` result.

Measured on the local stand (`storage::mcp_campaign`, Docker PostgreSQL, stub engine; debug build): draft ~0.8 s (the expense app placed on first use), commit + share ~0.24 s, `app.shared` on Sarah's stream ~45 ms later, Sarah's draft + commit ~0.5 s, `record.committed` on Diego's stream immediately.
## Scenario 6b: an agent scoped to Security cannot touch People (APP-115)

Agents get their own identity and a scope the member chooses; every call is logged and the refusals show up live.

**Before the pitch.** On Diego's canvas, open **Connect an agent**, name it "Security bot", choose **Only selected spaces → Security**, allowed actions **Read** and **Draft**, expiry **7 days**, and **Create token**. Diego needs the Time off app (People) and the Access requests app (Security) granted, as in the workflows setup.

**The act.**
1. Run the rehearsed probe with that token: `FORMA_SCOPED_AGENT_TOKEN` exported in the shell (the `read -rs` way, never on the command line) and `FORMA_ORIGIN=https://<host>`, then `app/scripts/mcp_haiku_agent.py --scope-demo`. Or, from Claude Code with that token: *"File a vacation request in Time off for Nov 10–12."* The agent lists only the Security apps (Time off is not offered), then tries `forma_draft_record` on Time off and gets "Not found.": the same answer as an app that does not exist.
2. On Diego's canvas: **Connect an agent → Activity**. The Security bot's timeline shows, newest first, **Draft a record · Time off · refused: outside scope (People)** next to the allowed calls, with the scope ("Security · read, draft"), call and refusal counts. The panel re-reads every 4 seconds, so the row appears while you watch.
3. Click **Revoke**: the next call from that agent is the uniform 404.

Say: "This agent was given Security, read and draft. It tried an HR app, and Forma refused it before touching any data. The refusal is on the agent's timeline, hash-chained, with who it acted for."

Rehearsed on the local stand: `storage::mcp_campaign` (`GENUI_MCP_STAND_SECONDS`) mints the same token and writes it to the stand file as `security_token`, so `--scope-demo --stand-file <scratch>/mcp-stand.json` runs the probe.

## Scenario 7: a folder of files becomes an app with receipts (APP-108)

The fixture is `docs/pitch/folder-fixture/` (synthetic: `q3_metrics.csv`, `q3_pipeline.csv`, `q3_metrics_brief.pdf`; every figure invented).

1. Click **+ Folder**, type "Q3 metrics", and drop the three files. The CSVs show their typed columns at once (month: date, revenue: money, signups: integer, churn_rate: decimal); click a PDF tile and it opens in the viewer.
2. Click the folder (its chip appears in the bar) and type: **"build a metrics app and explain it"**. In the demo organization the rehearsed metrics app (DS-73) lands at once with "Built from folder: Q3 metrics · 2 files" and a Summary card whose every number is a receipt-carrying value (DS-74). Outside the demo the generation model gets only the capped context (names and types, each schema with at most 20 rows; PDFs by name).
3. Open a KPI card's receipt: "Source file: q3_metrics.csv (folder Q3 metrics)" with **Open file**.
4. **Brief me** on the folder: the overview, a paragraph per file, and the PDF's statements cited "q3_metrics_brief.pdf, p. 1" (DS-75 in the demo; real citations otherwise).
5. Share the app with Sarah: she opens it and its receipts, reads the CSV it was built from, and does not see the PDF.
6. Same in other surfaces: Slack `/forma build a metrics app from the Q3 metrics folder` (or `/forma folder 1 formation brief me`); MCP `forma_list_folders`, `forma_folder_formation {folder, instruction}`, `forma_folder_brief {folder}`.

## The canvas: library pane and controls (APP-104)

The canvas starts empty except for what the member brought onto it and any record waiting for their decision (Sarah's pending expense always shows). **Library** in the top bar (or `L`) opens the pane. APP-120: it has two sections, **Apps** (Category → App → Card, every card numbered `3.2` and named `expenses.ledger`) and **Widgets** (Weather, Calculator, Clock, Note, Folder; Note and Folder expand to your notes and folders). Every row has an on-canvas dot, its name in grey and Add/Remove; app rows read "n of m cards", and everything updates on each add and remove. In the bar, `/card expenses.ledger` (or `/card 3.2`) places one card and `/expenses` (or `/app expenses`) the whole app, with autocomplete. `/apps [name]` opens this pane filtered by the name, and its suggestions place an app as Add does. Every verb, with its arguments and examples, is in one registry that drives the `/` list and the pane's Commands tab (`/commands`, `/help`); `?` opens the Shortcuts tab. The placed set is kept in this browser (DS-67).

Controls follow Figma: scroll or two-finger swipe pans (Shift: sideways); Cmd/Ctrl + scroll and pinch zoom at the cursor; Space + drag or middle-drag pans; `V` move, `H` hand (toolbar bottom center, with the zoom menu); Cmd/Ctrl + `=`/`-` zoom, Shift + 0 / 1 / 2 zoom to 100% / fit / selection; click a card's header to select, Shift + click to add, drag the header to move, arrows nudge 1 px (Shift 10 px), Delete puts the selection back in the library (no data changes), Esc clears; `?` shows the sheet. Touch: one finger pans the empty canvas, two fingers pinch. No shortcut fires while typing.

Stacks (after macOS Stacks and BumpTop's piles): drag a card onto another and hold a moment (the target lights up), select cards and press Cmd/Ctrl + G, or "Stack" on an app in the library. A stack shows its top card with the edges of the others, "+N" and "2/5"; horizontal scroll, Shift + scroll, ←/→ or a swipe leafs through it; "+N", Enter or a double-click fans it out (Esc folds it back; drag a card out of the fan to take it out); Cmd/Ctrl + Shift + G spreads it; double-click the name to rename. A new record waiting for you in a stacked app pulses the stack and brings its records card to the top. `/arrange` lays the canvas out one row per app.

## What each screen shows, in one line each

- **Intent bar status**: "Understanding…" → "Understood in N ms. Review the record, then confirm." → "Committing through the audited command path…" → "Filed: record v1, commit N, audit ab12cd34. Shared with Sarah Chen for approval."
- **Confirmation card**: the sentence quoted; typed values (money as dollars); the app chip; the approver chip or "No approver named"; Confirm / Edit / Cancel. Edit puts the sentence back in the bar.
- **Records card**: title (schema), "N waiting for your approval · M records", the table (amount, merchant, category, spent on, note, status chip, filed by), Approve / Reject on pending rows for a member with `record_write`, the provenance line (publication, definition digest, newest commit sequence, work version), the outcome line after a decision ("Approved. Record version 2, commit N. Audit event …").
- **Live chip**: "Live" while the stream is open; cards glow briefly when a live event re-read them.

## Two-window choreography (script)

1. Both windows at `/canvas`, both "Live". Left: Diego, right: Sarah. Zoom the right window so her records card is in view.
2. Left: type scenario 1's sentence. Pause on the confirmation card (read the values and the approver chip aloud). Confirm.
3. Right: the group arrives on its own. Point at "1 waiting for your approval". Type "approve the Nobu dinner" (or click Approve). Confirm.
4. Left: the row turns approved. Open the records card's outcome line and the provenance line; mention the audit event.
5. Right: type "what's waiting for my approval" → "Nothing waiting for your approval".
6. Either window: scenario 3, then open one receipt.
7. Priya's window (or the left window if Diego owns an authoring app): scenario 4; share from the work page; watch it arrive on the other window.

## Routes behind the scenarios

- `POST /v1/slack/commands`, `/v1/slack/interactivity`, `/v1/slack/events` (Slack-signed; absent `SLACK_BOT_TOKEN`/`SLACK_SIGNING_SECRET`: uniform 404) and `GET|POST /slack/link` (the member's link confirmation).
- `POST /v1/act` `{"utterance"}` or `{"decide": {app_id, work_id, record_id, status, note?}}` → a confirmation card payload (`command_request` with a server-minted `Idempotency-Key`, typed `values`, `approver`, `target`, `confirm_token`), `{"action":"open"}` or `{"action":"place"}`. Rate-limited 60/member/hour; uniform 404 for strangers; 422 `could_not_parse` / `could_not_draft`; 503 `composer_unavailable` without a key outside the demo organization.
- `GET /v1/work/{app}/{work}/records` → the work's current records, newest first, at most 200, with schema, commands and display labels; uniform 404 otherwise.
- `POST /v1/mcp` (APP-099: MCP Streamable HTTP, bearer agent token, 120/token/hour, long deadline; `GET` 405), `POST|GET /v1/agent-tokens`, `DELETE /v1/agent-tokens/{id}` (canvas session, same-origin).
- APP-115: `POST /v1/agent-tokens` also takes `scope` (`"all"` or `{"spaces": [...], "apps": [...]}`), `actions` and `expires_in_days` (1–30); `GET /v1/governance/agents` (the member's agents with scope, status and counts) and `GET /v1/governance/agents/{id}/activity` (that agent's access log, newest first). Member-scoped only.
- Folders (APP-108): `GET|POST /v1/boxes`, `POST /v1/boxes/uploads`, `GET /v1/boxes/{box}`, `POST /v1/boxes/{box}/name|share`, `POST /v1/boxes/{box}/files` (multipart, one `file` part), `GET /v1/boxes/{box}/files/{file}/content|preview`, `POST /v1/boxes/{box}/files/{file}/data|attach`, `POST /v1/boxes/{box}/apps`, `GET|POST /v1/boxes/{box}/brief`, `GET /v1/boxes/lineage/{app}/{work}`, `GET|POST /v1/boxes/explain/{app}/{work}`; `POST /v1/act` also takes `box_id`.
- `POST /v1/work/{app}/{work}/commands/{command}` (existing, audited), `POST /v1/work/{app}/{work}/share` (existing), `GET /v1/events` (existing), `POST /v1/compose` (existing).

## Evidence

`evidence/APP-093/scenarios/`: `actions-campaign.json` (the Docker campaign's 10 checks and timings), `two-window-shoot-notes.json` (the Playwright two-window run's measurements), `README.md`. The screenshots of the two-window run stay in the session scratch (synthetic data; not retained in the repository).

## Workflows (APP-107): the approval processes real companies run

Five more record apps join the library, each the same governed pattern as Expense claims: a typed request, a named approver, an audited decision, live for everyone, reachable from the canvas, Slack and any MCP agent. Victor picks one for the demo; all five are rehearsed. History cards run over labeled synthetic CSV data; the requests filed on stage are live records.

| App (slash) | Category | Request | Decisions | Declared rule (server-enforced) |
| --- | --- | --- | --- | --- |
| Time off (`/time-off`, alias `/pto`) | People | leave type, start, end, days, note | submitted → approved / rejected | — |
| Purchase requests (`/purchase-requests`, alias `/purchase`) | Finance | item, vendor, amount, cost center, justification | submitted → approved (≤ $5,000) or → manager_approved → approved (> $5,000); any step → rejected | guards on `amount`; finance cannot approve before the manager |
| Vendor invoices (`/vendor-invoices`, alias `/invoice`) | Finance | vendor, invoice number, amount, PO, due date | received → approved → paid, or → disputed | PO match flag (display only, DS-72) |
| Access requests (`/access-requests`, alias `/access`) | Security | system, role, reason, expires on | requested → granted / denied | — |
| Deal desk (`/deal-desk`, alias `/discount`) | Sales | account, deal amount, discount %, reason | submitted → approved / rejected | ≤ 10% approves on create (one audited command) |

`/access` (alias) and `/access-requests` open Access requests; the Access review tracker is `/access-review-tracker` (aliases `/access-review`, `/access-audit`). APP-120: every app's address is its title, lowercased and hyphenated; the earlier short names stay as aliases.

**Personas.** Diego Ramos files; Sarah Chen is the manager; Priya Natarajan decides the purchase's finance step on the local stand. On beta, until a third account exists, Sarah decides both purchase steps (say so: "on stage Sarah plays both roles"; DS-71).

**Setup (operator, beta).** Re-run `seed-library --organization <Northstar>` (adds the five apps; the 14 existing ones exact-match). Re-run `admission grant-library-access --organization <Northstar> --principal <persona>` for each persona (it covers every seeded library app, the five new ones included, with their sources; idempotent; `--record-write` as before). Upload the five new history sources like the others (`library_sources.py plan` lists their B1 keys: `leave_requests`, `purchase_requests`, `ap_invoices`, `access_grants`, `discount_requests`).

**Rehearsed sentences** (demo organization: no model call, DS-69; outside it the model parses them):

1. Time off. Diego: *"Book me off Nov 24–26 for Thanksgiving, Sarah approves"* → card: Leave type vacation · Start 2026-11-24 · End 2026-11-26 · Days 3 · Note Thanksgiving · Status submitted · Approver Sarah Chen. Confirm. Sarah: Approve on the row (or *"approve Diego's Thanksgiving time off"*). Diego's row turns approved live.
2. Purchase, two steps. Diego: *"Buy 12 Figma Enterprise seats from Figma for $7,200, cost center design, Sarah and Priya approve"* → card with Approver 1 Sarah Chen, Approver 2 Priya Natarajan; Confirm shares with both. The row carries its PO reference (`PO-XXXXXXXX`). Sarah's row offers **Approve (manager)** and Reject only; her Approve drafts `manager_approved` with "Manager approved by the app's declared rule: amount > $5,000.00". Priya's row then offers Approve / Reject; her Approve commits `approved` (version 3). A finance approval posted before the manager's is refused by storage. Under $5,000 (*"Buy a monitor arm from Atlas Office Supply for $180, cost center engineering, Sarah approves"*) one approval completes it.
3. Vendor invoice. Priya: *"Log invoice FIG-2207 from Figma for $7,450, PO-XXXXXXXX, due Nov 30, Sarah approves"* (the PO from step 2) → status received and the flag **"PO mismatch: PO-XXXXXXXX is $7,200.00, this invoice is $7,450.00"** on the card and on the row; it never blocks. Sarah approves, then *"mark the Figma invoice paid"*.
4. Access. Diego: *"I need admin access to the production database until Nov 30 for the billing migration, Sarah approves"* → status requested; Sarah **Grant** (or *"grant the production database access"*). The history card lists grants expiring in November.
5. Deal desk. Diego: *"Request an 8% discount for Quill Education on a $48,000 renewal, multi-year commitment"* → status **approved** with "Approved by the app's declared rule: discount pct ≤ 10%": one audited command, no approver. *"Request a 15% discount for Kestrel Logistics on a $120,000 deal, competitive pressure, Sarah approves"* waits for Sarah.

**The second vendor, mocked (DS-70).** On the local stand, `app/scripts/mcp_haiku_agent.py --stand-file <stand>` gives Claude Haiku Forma's MCP tools with Sarah's agent token and asks "what's waiting for my approval? approve the Figma seat purchase": `forma_list_pending` → `forma_draft_decision` → `forma_commit`; Diego's row shows **approved · via Haiku**. With the OpenAI key added, `mcp_openai_smoke.py` against beta does the same as OpenAI.

**Measured** (`storage::workflows_campaign::native_workflows_campaign`, Docker PostgreSQL, loopback model stub, debug build): card 0.8–1.1 s on first use (the app is placed for the member), Confirm ~0.11 s, a decision (draft + commit) 0.5–0.6 s, the two-step purchase 2.1 s, the auto-approved discount 0.6 s, an MCP decision 0.9 s.
