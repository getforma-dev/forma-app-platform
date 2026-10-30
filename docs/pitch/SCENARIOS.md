# Pitch scenarios: the four flows, the exact words, the two-window choreography

Status: built on `pivot/scenarios` (ADR-011; APP-093 action path, APP-094 library, APP-095 sharing and live updates, APP-098 demo shortcuts). Every number and every state change in these scenarios is real: the records are typed records committed through the audited, idempotent command path; the permissions are the share path's; the live updates are the event stream re-authorized at delivery; the receipts are the governed query path's. The only staged part is the parse of the rehearsed sentences inside the demo organization (APP-098 DS-38), and the member still confirms before anything is written.

Demo organization: Northstar (`00000000-0000-4000-8000-000000000001`), listed in `GENUI_DEMO_ORGANIZATIONS`, with `GENUI_DEMO_MODE=1`. Personas (synthetic display names from `GENUI_DISPLAY_NAMES_FILE`): **Diego Ramos** (employee), **Sarah Chen** (manager, approver), **Priya Natarajan** (security lead, the library's author). Outside the demo organization nothing canned applies.

## Setup (operator, once)

1. `seed-library --organization <Northstar>`: the 14 library apps, the **Expense claims** app among them (`contracts/fixtures/library/expense-claims.json`): record schema `expense_claim` (amount USD cents, merchant, category meals|travel|software|office|other, spent_on, note, status submitted|approved|rejected), commands `submit` (create) and `decide` (transitions submitted→approved, submitted→rejected only).
2. Grants (ADR-007 D7.3, operator): Diego and Sarah `discover` + `run` on the Expense claims app and `source_read` on its claims source; Priya is the seeded author.
3. `GENUI_DISPLAY_NAMES_FILE`: `{"members":[{"principal_id":"<Diego>","display_name":"Diego Ramos"},{"principal_id":"<Sarah>","display_name":"Sarah Chen"},{"principal_id":"<Priya>","display_name":"Priya Natarajan"}]}`.
4. Pre-existing claims so the canvas never starts empty: the library seeds one approved claim (Blue Bottle Coffee, $42.50) on Priya's work; before the pitch, file two more as Diego through the canvas and decide them as Sarah (the campaign does exactly this: one approved Nobu dinner, one rejected Figma seat). Keep one pending for the live scenario or file it on stage.
5. Two browser windows, side by side: **left** Diego signed in at `/canvas`, **right** Sarah signed in at `/canvas`. Both show "Live" in the top bar (the event stream is open).

## Scenario 1: employee expense → manager approval, live

**Left window (Diego).** Type in the intent bar, exactly:

> I spent $86 on a client dinner at Nobu last night, Sarah approves my expenses

What happens (measured in the campaign, synthetic data): the status line reads "Understood in ~600 ms"; a **confirmation card** rises above the intent bar:

- Amount **$86.00** · Merchant **Nobu** · Category **meals** · Spent on **yesterday's date** · Note **a client dinner** · Status **submitted**
- chips: *Expense claims* (the library app; the first time it is also "placed on your canvas"), **Approver: Sarah Chen**
- the note "Nothing is written until you confirm. The command is committed through the audited path with a server-minted key." and the buttons **Confirm and file** / Edit / Cancel.

Say: "Diego just said it. Forma turned it into a typed record under the app's schema, found Sarah by name in his organization and minted the command. Nothing has been written yet."

Click **Confirm and file**. The status line reads "Filed: record v1, commit N, audit <8 hex>. Shared with Sarah Chen for approval." The **Expense claims** group appears (or pulses) on the canvas with its **Expense claims** records card: the Nobu claim at the top, status chip **submitted**, "Filed by Diego Ramos", and the card's provenance line (publication, definition digest, commit sequence, work version).

**Right window (Sarah).** Within a second, without a reload: the Expense claims group is placed on her canvas (`app.shared` on her stream, the group fetched under her own grants). Her records card reads **"1 waiting for your approval · N records"**; the Nobu row carries **Approve** and **Reject**.

Say: "Sarah never set anything up. The share granted her exactly Diego's subset — read, write on this work — and nothing else; a non-member gets a 404 that is byte-identical for an existing and a nonexistent app."

Either click **Approve** on the row, or type in her intent bar:

> approve the Nobu dinner

A confirmation card: *Decide this record?* — Status **approved**, the record chip "Record: $86.00 · Nobu · a client dinner (v1)", **Confirm**. Click Confirm. Her card refreshes: status chip **approved**, "0 waiting for your approval".

**Left window.** The Nobu row turns **approved** live (`record.committed` on Diego's stream re-read his records list), with a brief glow. Say: "That decision is an audited command: version 2 of the record, one `command.committed` event on the organization's hash chain, and the chain verifies."

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

- `POST /v1/act` `{"utterance"}` or `{"decide": {app_id, work_id, record_id, status, note?}}` → a confirmation card payload (`command_request` with a server-minted `Idempotency-Key`, typed `values`, `approver`, `target`, `confirm_token`), `{"action":"open"}` or `{"action":"place"}`. Rate-limited 60/member/hour; uniform 404 for strangers; 422 `could_not_parse` / `could_not_draft`; 503 `composer_unavailable` without a key outside the demo organization.
- `GET /v1/work/{app}/{work}/records` → the work's current records, newest first, at most 200, with schema, commands and display labels; uniform 404 otherwise.
- `POST /v1/work/{app}/{work}/commands/{command}` (existing, audited), `POST /v1/work/{app}/{work}/share` (existing), `GET /v1/events` (existing), `POST /v1/compose` (existing).

## Evidence

`evidence/APP-093/scenarios/`: `actions-campaign.json` (the Docker campaign's 10 checks and timings), `two-window-shoot-notes.json` (the Playwright two-window run's measurements), `README.md`. The screenshots of the two-window run stay in the session scratch (synthetic data; not retained in the repository).
