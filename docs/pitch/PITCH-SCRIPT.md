# Forma: presentation script

For Victor, presenting to the YC panel or investors. About 8 minutes spoken plus a 4-minute live demo (section 5; the optional beats take it to 5). Lines in plain text are said aloud. The shot-by-shot filming plan and the pre-demo checklist are in [RECORDING.md](RECORDING.md). *Italic lines are stage directions.* **[Fill in]** marks what only you can supply. The grammar behind every typed line is [CANVAS-RULES.md](CANVAS-RULES.md); the definitions are in [GUIDE.md](GUIDE.md).

Updated 2026-10-31 (APP-096) for what is built on `pivot/pitch-mvp` at f0efc04: the four jobs (Ask, Shape, Do, Forma), cards made by you and pinned answers, start from your data, the clean-slate new customer, and `just dev`. **Every typed line in section 5 was run on a local dev instance on 2026-10-31**, with the timings it showed; lines that were not run are marked *(not re-run today)*. The demo runs on the laptop's dev instances (`just dev` and `just dev clean`), because beta.getforma.dev still runs TaskDefinition 12, which has none of this.

Three rules for every claim:
1. Say only what is built and measured. The figures marked *(measured)* below come from the test runs and the dev-instance run-through.
2. Call it what it is: a beta on synthetic data. In the demo organization some rehearsed sentences answer from canned, deterministic paths, like a concept car on a show floor (ADR-011; every shortcut is in APP-098). Everything after the parse is real: the drafts, the audited commits, the permissions, the live updates and the receipts. The new-customer instance has no shortcuts at all: it runs the real model for everything.
3. Never name a real company as a customer. There are none yet, and you say so plainly.

---

## 1. Opening (30 seconds)

Every team runs on small software: the expense tracker in a spreadsheet, the approval chain in someone's inbox, the dashboard an engineer built two years ago that nobody can change.

AI can now write that software in seconds. But what it writes is code, and nobody on the team can check it, secure it or trust the numbers it shows.

Forma lets a team describe the app they need and get it in seconds. The app is safe by construction: every number carries proof, and every change is audited. It is shared with the team and updates live in front of everyone.

## 2. The problem (45 seconds)

Today a team has two options:
- **Builders like Retool and Airtable.** Someone technical assembles the app by hand. It works, but it's slow, and the app is trusted code.
- **AI app generators.** You get an app in a minute, but it's generated code with generated access rules. When that goes wrong, real data leaks: a public CVE against Lovable apps (CVE-2025-48757) was scored 9.3, though Lovable disputes it.

Teams don't want more code. They want working tools they can trust, that their colleagues can use tomorrow, that they can change by asking.

## 3. Why I built this (60 seconds)

**[Fill in: your personal story, in two or three sentences. For example, what you saw teams struggle with, and what made you start.]**

The foundation came from a smaller problem. In March I was building fAuth, our identity service. It needed branded sign-in pages under a very strict security policy, and I did not want a JavaScript server running next to a Rust service just to render a login page.

So I built a framework: Forma Stack. You write the interface in TypeScript. It compiles to a compact binary format, and Rust renders it, with no JavaScript runtime on the server. Interactive parts of the page come alive in the browser without re-rendering everything. Forma Stack is open source, MIT licensed, now at version 3.2, and published on npm and crates.io.

Then I realized something. The discipline I built in for security is exactly what AI-generated team software is missing:
- typed structures instead of free-form code;
- nothing executed that wasn't reviewed;
- identity checked on every request.

That's Forma. In one sentence: I built Forma to make software structured, reusable and easier to change precisely, by people and by AI.

## 4. What Forma is (45 seconds)

Forma is a canvas for teams. You type what you need, and an app lands on the canvas. You share it with a teammate, and it's live for both of you.

You talk to it in four jobs, and where you type picks the job, never the AI:
- **Forma** makes something new: a plain sentence picks or builds an app.
- **Do** changes records, only through the app's own commands: `/expenses $86 Nobu, Sarah approves`.
- **Ask** finds out and can't change anything: `? @expenses where are the red claims?`
- **Shape** changes how a card looks or what it shows, never the records: `/expenses.ledger only red invoices`.

Four things make it different:
1. **Library first.** Most requests are something a team has needed before. Forma picks the right prebuilt app instantly, without even calling a model, then reshapes it to your request. It only generates something new when nothing fits.
2. **AI output is data, never code.** The model proposes a typed app definition. Our platform checks it, admits it or refuses it whole, and renders it with our own trusted components. The model never writes code, SQL, markup or permissions.
3. **Proof on every number.** Every figure carries a receipt that names the exact data version and query that produced it.
4. **One governed system, many front doors.** The same apps and permissions are on the canvas, in Slack, and available to any AI agent (Claude, OpenAI or your own) through MCP, the open standard agents use to call tools.

## 5. Live demo (about 4 minutes)

*Three browser windows on the laptop, set up by the checklist in [RECORDING.md](RECORDING.md#before-the-demo-checklist): **Alex** (a brand-new customer, `just dev clean`, http://127.0.0.1:4010/canvas), **Diego** (employee, http://127.0.0.1:4001/canvas) and **Sarah** (manager, http://127.0.0.1:4002/canvas), Diego and Sarah side by side. Diego's and Alex's canvases start empty; Sarah's already shows Expenses.*

| # | Beat | Who, where | Time |
| --- | --- | --- | --- |
| 1 | A new customer: a sentence builds an app; a file becomes a card | Alex, :4010 | 75 s |
| 2 | Forma picks from the library and reshapes | Diego, :4001 | 20 s |
| 3 | Do: a governed record, an approval, live on two screens | Diego :4001 and Sarah :4002 | 55 s |
| 4 | Ask versus Shape, made cards and pins | Diego, :4001 | 75 s |
| 5 | *Optional:* your own names | Diego, :4001 | 10 s |
| 6 | *Optional:* bring your own agent (MCP) | Diego, Claude Code | 30 s |

Core beats 1–4: about 3 min 45 s. With 5 and 6: about 4 min 25 s.

### Beat 1: a new customer, from nothing (75 s, Alex on :4010)

*Alex's window: the empty canvas, "What do you need?". Don't click the example chips under it (pitfall 1 in RECORDING.md).*

This is a company that signed up a minute ago. No apps, no templates, no data. And here there are no rehearsed shortcuts: everything you'll see in this window is the real model.

*Type, and press Enter:*

> Track our vendor contracts: vendor, owner and renewal date

*While "Composing your app…" shows (about 11 s, measured twice: 11.5 and 11.6 s), say:* the model is writing a typed app definition, not code. Our platform checks it against the contract and admits it whole or refuses it whole.

*The app lands:* "Vendor Contract Tracker", labeled **generated**, with a "New vendor contract" form and a "Vendor contracts" records card. *Fill the form: Vendor* `Figma`, *Owner* `Alex Rivera`, *Renewal date* `2027-03-01`, *then* **Add contract**.

*Point at the form's footer:* "Committed. Record version 1, work version 1, commit 2. Audit event …". *Point at the records card:* the row, and under it the receipt line (publication, definition digest, commit, work version). That's a governed record, from one sentence, in a workspace that had nothing.

*Now drag* `q3_pipeline.csv` *(from `docs/pitch/folder-fixture/`, ready in Finder) onto an empty part of the canvas.* A folder opens with its name ready: *type* `Q3 pipeline` *and Enter.* It offers three buttons: **Ask**, **Make a card**, **✨ Make an app**.

*Click* **Ask**; *in the Ask tab's own input, type:*

> which open deals are worth more than $50,000?

*(About 2 s.)* Four deals, and every one cites its file and row: "q3_pipeline.csv · row 3: Cobalt Health · proposal · 94500.00", each with a **Receipt**. Ask only reads; it can't change a thing.

*Click* **Make a card**, *then* **Make card** *in the folder's confirm.* A chart, "Amount by close date", with a **Verified** receipt, plus an AI summary whose numbers come from receipts. It says "Built from folder: Q3 pipeline". Data, then a card, then an app: no new concepts.

*Fallback if the model is slow:* keep talking over "Composing your app…" up to 20 s. Past that, say "the model is busy; here's one I built before we started" and switch to a fourth window where you built the same sentence during setup (never reloaded: pitfall 2). Make a card itself takes under half a second; only its summary waits on the model (about 4 s). **✨ Make an app** is the full build from the file (about 20 s plus the summary, measured once): show it only if you have time, or build it during setup.

### Beat 2: Forma picks from the library (20 s, Diego on :4001)

*Switch to Diego's window, empty canvas. Type:*

> Review expenses over $200

The Expense review queue landed instantly from our library of prebuilt apps, and it's already reshaped: "Adapted to your request · Amount > $200". *(Measured: 0.4 s on the server.)* It's labeled *adapted*, so you always know what the AI did. *Point at any number's* **Verified** *chip:* every governed number carries a receipt.

*Honesty note (APP-098):* in the demo organization this sentence's reshape is rehearsed: it answers without the model. Outside the demo organization the model writes the same typed filter (0.7 to 1.0 s, measured earlier).

### Beat 3: Do: a governed record and a live approval (55 s, Diego and Sarah)

*Diego's window. Type:*

> /expenses $86 Nobu, Sarah approves

`/expenses` and words is **Do**: it can only run one of the Expenses app's own commands. *The confirm card rises (about 2.3 s, the real model):* "Submit claim?", Amount **$86.00**, Merchant **Nobu**, Status **submitted**, **Approver: Sarah Chen**, "via /expenses", and "Nothing is written until you confirm."

*Click* **Submit claim**. Filed: record version 1, a commit, an audit event. *(Don't read the rest of the status line aloud: pitfall 5.)*

*Point at Sarah's window:* the Nobu claim is already on her Expenses card, "waiting for your approval". Nobody refreshed anything.

*In Sarah's window, click* **Approve** *on the Nobu row* (or type `approve the Nobu dinner` and click **Approve claim** on its confirm card, about 2 s). *Point back at Diego's window:* the row flips to **Approved**, live.

That approval is an audited command: version 2 of the record, on the organization's hash chain. And Diego can't approve his own claim: the server refuses it. *(Don't click Approve in Diego's window: pitfall 6.)*

*Fallback if the model is slow (over 5 s):* Cancel, and type the rehearsed sentence instead, which answers in under a second in the demo organization: `I spent $86 on a client dinner at Nobu last night, Sarah approves my expenses` (measured: confirm card in 945 ms, category **meals**). *Honesty note (APP-098): that sentence's parse is canned in the demo organization; the draft, confirm, commit and live update are real.*

### Beat 4: Ask versus Shape (75 s, Diego on :4001)

Two jobs that look alike and never mix. **Ask** finds out and can't change anything. **Shape** changes how a card looks or what it shows, and never touches a record.

*Type:*

> ? @expenses where are the red claims?

*The Ask tab opens, "Asking about /expenses (all its records)":* the claims that need attention, each with its **Receipt**. *(Instant: rehearsed in the demo organization, APP-098; the answer is computed from the records.)*

*On that answer, click* **Make it a card**, *then* **Add as new card**. A card made by me: "Made by you from /expenses · only you see it", showing exactly those records, live. *Click* **Pin to canvas** *on the same answer:* a pinned answer, "As answered on <date> … it does not run again". A record of what Ask said, never passed off as live.

*Now a question with no rehearsal. Type:*

> ? @expenses summarize where the money went

*(About 2 s, the real model.)* A paragraph with totals by category, and every record it used, each with a receipt. The AI explains the numbers; it can't invent one.

*Now Shape. Type:*

> /expenses.ledger only red invoices

*A preview:* "Only status is submitted or rejected", with **Apply**, **Add as new card**, **Cancel**. Nothing changes until I click. *Click* **Add as new card**: "Expense ledger (shaped)", made by me.

*Type:*

> /shape expenses dark green, compact

*Preview:* "dark mode · green accent · compact density". *Click* **Apply**. Every Expenses card is restyled, each keeps its own view, and **Revert** is on the app card. Records untouched.

*Honesty note (APP-098):* "where are the red claims?", "only red invoices" and "dark green, compact" answer from rehearsed paths in the demo organization; "summarize where the money went" goes to the model. Stick to these exact words: a rehearsed question that adds a condition (for example "over $100") is answered as if the condition weren't there (pitfall 7).

### Beat 5 (optional): your own names (10 s, Diego)

*Type:* `/alias e expenses`. "/e now opens Expenses, for you only." Everything has a name you type, and your shortcuts are yours alone; the organization's names never break.

### Beat 6 (optional): bring your own agent (30 s) *(not re-run today)*

*Connect an agent in the header mints a token for Claude Code. In Claude Code:* "What expenses are waiting for my approval?" Claude is calling Forma through MCP with Diego's own permissions, nothing more. It can draft, but it can't bypass the approval. *Setup and words: SCENARIOS.md, scenario 6. This beat was not re-run on 2026-10-31; rehearse it before using it.*

*Close:* that was a beta on synthetic data. Every record you saw is real in the system: typed, audited, permissioned and updated live. And the first minute was a customer with nothing, building from a sentence and a file.

## 6. How it fits together (60 seconds)

There are four layers, and each one is a decision about trust.

1. **Forma Stack, the framework.** TypeScript compiled to a binary format and rendered by Rust. The interactive parts come alive in the browser. There is no `eval` and no injected code, and a strict security policy applies by default. It's open source.
2. **fAuth, identity.** Our multi-tenant identity service. It runs sign-in and multi-factor authentication and issues signed tokens. fAuth knows who signed in but never sees team data. The platform never sees passwords: it verifies fAuth's signed token and issues its own short, secure session.
3. **The platform, governance.** Every organization's data is isolated by the database itself, with row-level security on every table and no role able to bypass it. Every number is computed on the server and carries a receipt. Every change is one transaction with an audit record and a retry-safe key. Live updates are re-authorized for each viewer before delivery.
4. **The surfaces.** The canvas, Slack and MCP all call the same governed operations. A new surface adds reach, not risk.

## 7. Why it's safe, and why that's the moat (45 seconds)

Speed is not our edge. A modern framework renders a page of cards about as fast as ours. Safety is the edge, and it's built into the structure:
- **AI output is typed data.** It's admitted whole or refused whole.
- **Tenant isolation is enforced by the database.** It doesn't depend on application code getting it right.
- **A second synthetic organization replays the first one's identifiers** and must get nothing back.
- **Every answer to a forbidden request looks identical to "not found"**, so nothing leaks through error messages.
- **Approvals are real.** An author can't approve their own publication, and an agent can draft but not decide for you.

A competitor can add a security checkbox. They can't retrofit a system where the AI never writes code.

## 8. Why now, and why YC (30 seconds)

Two things changed this year.
1. **Teams are adopting AI agents.** Those agents need somewhere governed to do work, not just a chat window.
2. **MCP became a common standard** for how agents call tools.

YC's request for "A Cloud for Small Software" asks for small software that's as easy to share with colleagues as a Google Doc. That's how we read our fit with it: a cloud for small team software that people and agents can safely build, share and change.

## 9. Market and competition (30 seconds)

| Option | What you get | What's missing |
| --- | --- | --- |
| Spreadsheets, scripts | Flexible | No permissions, no audit, no proof |
| Retool, internal-tool builders | Solid apps | Built by hand; the app is trusted code |
| Lovable and AI app generators | Speed | Generated code with generated access rules |
| Airtable, Glide | Easy data apps | Limited governance; AI is an add-on |
| Claude or ChatGPT artifacts | Instant | Single-user, no shared governed data |
| **Forma** | **Describe, share, live, and governed** | Early: beta, synthetic data |

**[Fill in, if you have it: market sizing. None is in our documents yet; don't improvise a number on stage.]**

## 10. Business model (20 seconds)

The model is a managed platform subscription per team or organization. Pricing and billing units are still open questions we'll answer with the first pilots. Our cost model is roughly $115 per organization per month at ten organizations, falling with scale.

## 11. Where we are: honest status (30 seconds)

- **Forma Stack:** 3.2, open source, published. Eight packages: TypeScript compiler, client runtime, Rust renderer and server.
- **fAuth:** running on staging, with hosted sign-in, multi-factor authentication and signed tokens.
- **The platform:** a beta at beta.getforma.dev since this week, invite-only, synthetic data: canvas, library, reshaping, receipts, approvals, Slack and MCP. What you saw today (Ask and Shape, made cards, start from your data, the new-customer path) runs on the same code on our dev instances and goes to the beta with the next deploy.
- **Traction:** none yet. No customers, no pilots. That's what the next months are for.
- **Team:** solo founder. I build with AI coding agents under strict review. Phases, tickets and evidence are in the repository.
- **Next:**
  - first pilot teams on real data, after a data protection review;
  - real per-organization onboarding (today an operator sets up organizations);
  - generated themes;
  - the full review chain for publishing team apps.

## 12. The ask (15 seconds)

**[Fill in: amount, what it buys (first hires, pilots, months of runway), and the milestone it reaches.]**

## 13. Close (15 seconds)

Small team software is about to be written by AI whether we like it or not. Forma makes sure it's software a team can trust, with typed apps, proof on every number and every change audited, on a canvas the whole team shares. Thank you.

---

## Appendix A: hard questions, short answers

**"Isn't this just Retool with AI?"**
Retool apps are code that someone trusts. In Forma, the AI produces typed definitions that our platform checks and renders with its own components. Nothing generated ever runs as code. That's the difference between "AI helped write it" and "AI can't break it."

**"What stops Anthropic or OpenAI from building this?"**
They build the agents, and we're where the agents' work becomes governed. Forma works with every vendor through MCP: Claude and OpenAI agents act on the same audited records, under the member's own permissions (built and tested in APP-099).

**"Why build your own framework?"**
It started as a security need for fAuth's sign-in pages. It gives us server rendering with no JavaScript runtime, no `eval`, and a strict security policy by default. That foundation is what makes "AI output is data" enforceable end to end. We don't claim it's the fastest framework.

**"Solo founder?"**
Yes. I run development with AI coding agents under explicit review gates, and the decision records and evidence are in the repository. The first hires are planned with the round. **[Fill in your hiring plan.]**

**"Was the demo real?"**
It's a beta on synthetic data, run today on our dev instances. The records, permissions, audit, live updates and receipts are all real. For stage timing, a few rehearsed sentences in the demo organization skip the language model (the library reshape, the Nobu sentence, "where are the red claims?", "only red invoices", "dark green, compact"); the others went to the real model in about 2 seconds. The new-customer window has no shortcuts: its app and its answers were the real model. Every shortcut is listed in a register (APP-098) and switched off outside the demo organization.

**"How fast is it?"**
*(measured)* The instant app pick takes about 4 ms. The live reshape takes 0.7 to 1.0 s with the real model. A question to Ask, or a sentence parsed into a draft, takes about 2 s with the real model. A new records app from one sentence takes about 11 s, and a full app from a file about 20 s. An approval reached the other screen in tens of milliseconds in our test runs.

**"What does it cost to run?"**
The demo costs about $170 a month to host. At ten organizations the cost model is about $115 per organization per month.

**"What happens to customer data?"**
fAuth sees identities, never data. Each organization's data is isolated by the database and encrypted with its own key. The model sees schemas and synthetic rows, never who asked. A data protection review comes before the first real organization.

## Appendix B: numbers you can and can't say

| Say | Don't say |
| --- | --- |
| Beta, synthetic data, invite-only | "Customers", "users", "pilots" |
| Pick about 4 ms; reshape 0.7–1.0 s; Ask about 2 s; a new records app about 11 s *(measured)* | "Fastest framework", or any competitor speed ranking |
| Forma Stack 3.2, MIT, published on npm and crates.io | Download counts as traction (they're mostly our own CI) |
| Row-level security on every table; receipts on every governed number; audited, idempotent commands | SOC 2, HIPAA certification, enterprise SSO (none are done) |
| fAuth: hosted sign-in, multi-factor authentication, signed tokens | SAML or SCIM (not built) |
