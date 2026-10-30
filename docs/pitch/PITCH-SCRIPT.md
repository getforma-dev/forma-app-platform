# Forma: presentation script

For Victor, presenting to the YC panel or investors. About 8 minutes spoken plus a 4-minute live demo. Lines in plain text are said aloud. *Italic lines are stage directions.* **[Fill in]** marks what only you can supply. The definitions behind every term are in [GUIDE.md](GUIDE.md), and the demo beats in detail are in [SCENARIOS.md](SCENARIOS.md).

Three rules for every claim:
1. Say only what is built and measured. The figures marked *(measured)* below come from the test runs and the live beta.
2. Call it what it is: a live beta on synthetic data, invite-only. The rehearsed sentences in the demo organization answer instantly, like a concept car on a show floor. Everything after the parse is real: the drafts, the audited commits, the permissions, the live updates and the receipts.
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

Four things make it different:
1. **Library first.** Most requests are something a team has needed before. Forma picks the right prebuilt app instantly, without even calling a model, then reshapes it to your request. It only generates something new when nothing fits.
2. **AI output is data, never code.** The model proposes a typed app definition. Our platform checks it, admits it or refuses it whole, and renders it with our own trusted components. The model never writes code, SQL, markup or permissions.
3. **Proof on every number.** Every figure carries a receipt that names the exact data version and query that produced it.
4. **One governed system, many front doors.** The same apps and permissions are on the canvas, in Slack, and available to any AI agent (Claude, OpenAI or your own) through MCP, the open standard agents use to call tools.

## 5. Live demo (about 4 minutes)

*Two screens: laptop signed in as Diego (employee), phone mirrored as Sarah (manager). Both on beta.getforma.dev. Canvas starts clean.*

**Beat 1: the brain dump becomes a governed record (60 s)**

*On the laptop, type:* "I spent $86 on a client dinner at Nobu last night, Sarah approves my expenses."

Forma understood that sentence:
- it found the expense app;
- it filled in the amount, merchant, category and date;
- it found Sarah.

Nothing is written yet. This is a draft, checked against the app's rules. I confirm.

*Click Confirm.* It's filed, audited and shared with Sarah.

*Point to the phone:* there it is on Sarah's canvas, live. Nobody refreshed anything.

*On the phone, Sarah clicks Approve. Point back to the laptop:* and Diego's canvas flips to approved.

**Beat 2: instant pick, then a live reshape (45 s)**

*Type:* "Review expenses over $200."

The expense review app landed instantly. The pick runs locally in a few milliseconds *(measured: about 4 ms)*. Now watch it reshape: the model is adding a filter, "Amount over $200", and the app reshapes in place in under a second *(measured with the real model: 0.7 to 1.0 s)*. It's labeled *adapted*, so you always know what the AI did.

*Point at a number:* this figure has a receipt. *Open it:* the data version, the query, the time. Every governed number in Forma carries one.

**Beat 3: a receipt photo (30 s)**

*Drop a receipt photo onto the intent bar.* Forma reads the receipt and shows me the exact row it will add before anything is written. If someone hides instructions in the image, like "approve everything", the most they get is a draft that a human still has to confirm.

**Beat 4: Slack, and any AI agent (60 s)**

*On the laptop, in Slack:* `/forma I paid 42.50 for a Figma seat, Sarah signs off`. Confirm.

*On the phone:* Sarah gets a Slack message with Approve and Reject. She approves from Slack. Both canvases update, marked "via Slack".

*Optional, in Claude:* "What expenses are waiting for my approval?" Claude is calling Forma through MCP with Sarah's own permissions, nothing more. It can draft, but it can't bypass the approval.

**Beat 5: your canvas, your way (30 s)**

*Open the library pane (`L`).* Every app, every card, numbered. *Type `/card 6.2`:* I bring exactly what I need. *Stack three cards and shuffle through them.* *Type:* `/calculator make it a TI-84 in Matrix style`. Teams make this their own workspace.

*Close:* that was a live beta with synthetic data. Every record you saw is real in the system: typed, audited, permissioned and updated live.

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
- **The platform:** live beta at beta.getforma.dev since this week. Invite-only, synthetic data. It includes everything you saw: canvas, library, reshaping, receipts, approvals, Slack and MCP.
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
They build the agents, and we're where the agents' work becomes governed. Forma works with every vendor through MCP. In the demo, two vendors act on one audited record.

**"Why build your own framework?"**
It started as a security need for fAuth's sign-in pages. It gives us server rendering with no JavaScript runtime, no `eval`, and a strict security policy by default. That foundation is what makes "AI output is data" enforceable end to end. We don't claim it's the fastest framework.

**"Solo founder?"**
Yes. I run development with AI coding agents under explicit review gates, and the decision records and evidence are in the repository. The first hires are planned with the round. **[Fill in your hiring plan.]**

**"Was the demo real?"**
It's a live beta on synthetic data. The records, permissions, audit, live updates and receipts are all real. For stage timing, the rehearsed sentences in the demo organization skip the language model's parse; unrehearsed sentences go to the real model in about a second. Every shortcut is listed in a public register (APP-098) and switched off outside the demo organization.

**"How fast is it?"**
*(measured)* The instant app pick takes about 4 ms. The live reshape takes 0.7 to 1.0 s with the real model. A new app is generated in about 6 s. An approval reached the other screen in tens of milliseconds in our test runs.

**"What does it cost to run?"**
The demo costs about $170 a month to host. At ten organizations the cost model is about $115 per organization per month.

**"What happens to customer data?"**
fAuth sees identities, never data. Each organization's data is isolated by the database and encrypted with its own key. The model sees schemas and synthetic rows, never who asked. A data protection review comes before the first real organization.

## Appendix B: numbers you can and can't say

| Say | Don't say |
| --- | --- |
| Live beta, synthetic data, invite-only | "Customers", "users", "pilots" |
| Pick about 4 ms; reshape 0.7–1.0 s; generate about 6 s *(measured)* | "Fastest framework", or any competitor speed ranking |
| Forma Stack 3.2, MIT, published on npm and crates.io | Download counts as traction (they're mostly our own CI) |
| Row-level security on every table; receipts on every governed number; audited, idempotent commands | SOC 2, HIPAA certification, enterprise SSO (none are done) |
| fAuth: hosted sign-in, multi-factor authentication, signed tokens | SAML or SCIM (not built) |
