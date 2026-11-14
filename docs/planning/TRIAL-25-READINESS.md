# Real live test: 25 real people on the beta (Victor's class, Nov 29–30)

Status: **approved by Victor on 2026-11-14; executing.** Tickets: APP-175 (release A), APP-181 to APP-199; go-live is APP-189. Earlier drafts of this file are superseded.

## Context

Victor will teach a weekend class on **Nov 29–30**. His 25 students, all adults, will join **his own organization** on `beta.getforma.dev` and use the platform for real:
- Victor builds apps for his class: attendance, homework assign-and-submit and grading, formative quizzes, and a live game quiz if it is ready.
- Students use those apps, and can also build and share their own apps for anything.

Forma stays a **general platform**: the MVP serves data apps and many other scenarios. This plan adds **no classroom features, names or templates**. It adds general capabilities that let Victor shape classroom apps himself, which is the proof that the platform is malleable. AGENTS.md says the same: "Both example apps must use the same general path without tenant-name branches."

**Readiness today (verified 2026-11-14):** not ready.
- **No reviewed invite path:** `ops/beta/run-step.sh` is fenced (`ops/beta/lib.sh:47-51`), and `ops/promotion` has no admission command.
- **The charter is synthetic-only** (ADR-008:7-8).
- **The release is stuck.** Run 43157872671's ledger basis is stale (`infra/scripts/preflight.py:429`). Its scan disposition expires Nov 20. The beta scales to zero Nov 25.
- **Basics are missing:** no consent, no admin, no member removal, no model cap, no alarms, no restore drill.
- **General gaps that real shared use exposes:**
  - members show only as "Member 1a2b…", because names exist only for demo organizations (DS-44);
  - a record list can't keep entries private to their creator (`admission/validate.rs:966-977`: a record schema has only id, version and fields);
  - nothing limits who may perform a command, so anyone who can write can, for example, set their own grade or approval;
  - sharing goes to one person at a time (`storage/sharing.rs:212`).

What works:
- The member journey: code → register → verify email → empty canvas → build an app → records (APP-139, APP-145).
- Sharing with write access, and live updates.
- SES out of the sandbox, verified live: `getforma.dev` with DKIM, suppression on.
- Per-app privacy between members.

## Decisions (locked 2026-11-14)

| # | Decision |
| --- | --- |
| D1 | A real-user charter as an ADR-008 amendment: Victor's organization, 25 adult students, invite-only, beta label, no SLA, data terms, accepted risks. Notice v1 is rewritten for this cohort (W0). |
| D2 | **One organization: Victor's** (today's clean-trial organization `forma-trial`, given his class's display name if the operator supports it). Victor is its admin (`admin_policy`). Students are **members**, not share-link guests, so they can submit, build and share. |
| D3 | Harden the beta to production standards. The separate production environment (ADR-013) is planned after this test. |
| D4 | **Admin follows the standard SaaS split:** an in-product Organization Admin area for org admins, plus the audited operator back office for platform work. |
| D5 | **Durable, enforced consent:** a required checkbox tied to the notice version, stored and audited. |
| D6 | **Real member names (general):** each member sets the name others see, at first sign-in, stored in the database. It is used in share lists, admin, records and Slack. The DS-44 file stays demo-only. |
| D7 | **Private entries (general):** a record list can be declared "each person sees and changes only their own entries". The app's **reviewers** (its owner, plus members granted reviewer) see and change all entries. The same rule holds on every path: canvas, app page, record page, queries and summaries, live updates, Ask, MCP, Slack, guests and exports. |
| D8 | **Who may do what (general):** a command can be limited to reviewers (approve, grade, close). Contributors can't run it, even on their own entries. |
| D9 | **Share with many (general):** share an app with "everyone in the organization", and choose a role when sharing: viewer, contributor or reviewer. A sharer may grant reviewer only if they hold `record_review`, and `share_work` is never copied (ADR-011), so the owner controls who reviews. (Fable advisor, 2026-11-14.) |
| D10 | **Spend:** an app-wide daily model-call cap, default **1,000 a day** in code (env override for tests). The plain message says building and Ask pause until 00:00 UTC, while apps and entries keep working. Anthropic limits: **$150 for November and $100 for December**, with 50% alerts; raise November to $200 if Nov 29 spends over $60. Per-member hourly limits stay; the per-address limits for invitations and sign-in are raised for one classroom network. (Fable advisor, 2026-11-14.) |
| D11 | **Live interaction (stretch):** a Kahoot-style live quiz is attempted only if a design spike shows it can be built from general primitives (a shared "current item", timed responses, a live summary). Go or no-go on **Nov 24**; if no, the class uses formative quizzes. **Decided 2026-11-14: NO-GO** (Fable advisor in lieu of Victor, from the [APP-193 finding](../../evidence/APP-193/finding-2026-11-14.md)). Fair timed play needs several new primitives, and a leaderboard needs an exception to D7. The class uses formative quizzes: an `own` responses list and a reviewer summary that Victor can project. |
| D12 | Invite codes go out from Victor's own email. No app-sent email. Feedback goes to `feedback@getforma.dev`, linked in the app. |
| D13 | Alarms to Victor's email, plus one restore drill, before codes go out. APP-078 is deferred (one real organization). |
| D14 | **Two schema releases.** Release A: today's `main` (schema 20→22) before the Nov 20 scan expiry. Release B: migrations 0023 and 0024 (schema 22→24; amended 2026-11-14, when W4's private entries and roles became 0024), all new data capabilities, on Nov 23, with a renewed scan disposition. |

Assumptions: adults; no regulated data; English; desktop Chrome and Safari plus phones for check-ins; Victor is the only operator; the class needs internet access.
Out of scope: anything classroom-specific in the platform; public sign-up; organization per person; app-sent email; more than 25 students; a separate production stack.

## Calendar

**Every date below is a "no later than"** (Victor, 2026-11-14: "No arbitrary time limits... If things can move forward let's move forward"). Each step starts the moment its prerequisites are done; only the class weekend (Nov 29–30) is fixed. The only waits kept are technical guards the code enforces: the 65-minute credential fence before `maintenance abandon`, reconcile only while no release holds the lock, an approval within 24 hours of a fresh reconcile, and no approval in the 05:00–05:45 UTC backup window.

| Date | What | Who |
| --- | --- | --- |
| Fri Nov 14 | Charter and notice v1 (cohort text), tickets revised. Promotions paused (done). Victor: APP-175 permission readback; reject or cancel run 43157872671. | agents, **Victor** |
| Sat Nov 15 | After the 65-minute fence: `maintenance inspect`/`abandon`, `reconcile`, re-enable promotion, approve **Release A** (main, 20→22). **Done 2026-11-14 at 07:17 UTC** ([record](../../evidence/APP-175/release-2026-11-14.md)). Victor (no cost): open one shared work page on beta with the browser's developer tools and send the time of its JSON document request. That is the beta document-read timing the capacity statement scales from (a document read is 11 transactions; APP-197). | **Victor** (agents prepare the checklist) |
| Nov 15–21 | Parallel build: W1–W4. W5 spike Nov 17–18. | agents |
| Sat Nov 22 | Tier A reviews closed; merge W1–W4. | agents |
| Sun Nov 23 | Renewed scan disposition. **Release B** (22→24). Expiry renewal to Dec 15 plus the December month. Alarm template deployed. Anthropic limit set. FormaStack raises the fAuth tenant limit to at least 30. Restore drill. | **Victor** |
| Mon Nov 24 | Victor builds his class apps on beta with the composer, using a fresh alias plus one helper account. | **Victor** + agents fix forward |
| Wed Nov 26 | Rehearsal with 2–3 real students: sign-up, consent, names, his apps, private entries, grading, sharing, building their own app. | **Victor** |
| Fri Nov 28 | Issue 25 codes, then one restart in the evening. Victor emails the codes. No deploys after this. | **Victor** |
| Nov 29–30 | Class. | Victor + students |
| Dec 1–12 | Real use continues. Watch and learn. Fixes only in announced windows. | |
| by Dec 14 | Purge the organization's data per the charter; the beta stops Dec 15 unless renewed. | **Victor** |

## Workstreams

### W0: charter, notice and tickets (agents, Nov 14)
- Revise the ADR-008 amendment on `plan/trial-25-readiness` (PR #115): Victor's organization, adult students, purge date, accepted risks. Rewrite notice v1 to say that the teacher can see entries in apps he owns or reviews, and that students can build their own apps.
- Revise the tickets: APP-182 consent, APP-183 admin, APP-184 cap, APP-185–188 operations, APP-189 go-live. Add tickets for D6 names, D7 private entries, D8 reviewer commands, D9 share with many, and the W5 spike.
- Update `docs/planning/TRIAL-25-READINESS.md`.

### W1: migration 0023 and storage (Tier A). Owns `app/migrations/`, `catalog-v23.json`, `manifest.json`, `app/src/storage/*` (except the read paths W4 changes), `ops/promotion/schema-release.json` (release B).
- **Consent:** insert-only `genui.member_consents`. `notice_version` is carried on the pending admission (`storage/sessions.rs:595`) into `genui_private.admit_redemption` (0003:329), with the audit kind `admission.consented` (policy at 0003:162-164, restated at 0008:85).
- **Names:** `memberships.display_name`, set by the member, bounded and printable (`directory::usable_name`). `directory::label` uses it for any organization, and the DS-44 file only fills in for demo organizations.
- **Admin definers**, owned by a NOLOGIN role (the `genui_admit` pattern, 0003:12), requiring live `admin_policy` (`authorization.rs:59`):
  - `org_admin_overview`: per member, the name, status, consent, last active, apps, works, records, uploads and code status;
  - `admin_remove_member`: revoke membership and entitlement, bump the epoch, audit; refuses self or the last admin.
- **Private entries and reviewers:**
  - a work-level permission `record_review` (the CHECK at 0002:201 gains it);
  - a reviewer-aware membership in the share grant;
  - `access_codes.note` for the operator.
- **Org-wide share:** a grant to all current and future members of the organization. It is revoked as a single unit.

### W2: app surfaces. Owns `app/src/auth/invite.rs` and its tests, the `app/admin/src/*` pages, `app/src/pages.rs`, the new admin route and page, `composer/model.rs` (the cap), and the share dialog.
- **Invite page:** notice v1, plus a required `consent` field. `/invite/validate` accepts exactly `code` and `consent=1` (today it accepts one field: `invite.rs:458-464`).
- **First-sign-in name prompt.**
- **`/admin`** for org admins: members, consent, activity and Remove (CSRF, confirm). Non-admins get the uniform 404.
- **Share dialog:** viewer / contributor / reviewer, plus "Everyone in the organization".
- **Daily model cap:** a process-wide counter in `Anthropic::send` and `complete_with_documents`. The new `ModelError::DailyCap` becomes 429 `daily_limit` with a plain message.
- **Feedback link.**

### W3: operator back office and operations (Python). Owns `ops/`, `app/runtime/admission_operator.py`, `app/operator/overrides.json`, `docs/operations/`.
- **`ops/promotion/admission.py`:** `issue --count N --notes-file`, `list`, `revoke`, `grant-admin`, `set-org-name` (only if the operator supports it), `delete-uploads`, and `restart`. One restart serves up to 128 invites (`runtime.py:186-189`).
- **`expiry_renewal.py` and `month_renewal.py`,** plus an ADR-019 amendment with ceilings covering Nov 23 to Dec 15.
- **Scan disposition renewal** for release B.
- **`ops/monitoring/alarms.json`,** a standalone template outside `infra/src`.
- **`docs/operations/TRIAL-RUNBOOK.md`:** codes and restarts, removing a member, deny-all, rollback, feedback, the restore drill and the purge.

### W4: private entries and reviewer commands, end to end (Tier A; one design-it-twice step first, Nov 15)
- **Grammar:** a record schema gains an optional `visibility: "own"` (absent means shared) and a command gains an optional `reviewer_only: true` (`admission/validate.rs:966`, `:1569`). Each meaning has one encoding; an explicit `"shared"` or `false` is refused. These go through the contract edition update (`contracts/`, lock), and older definitions stay valid unchanged.
- **One storage rule:** in an `own` collection, a member sees and changes only rows they created, unless they hold `record_review` or own the app. Reviewer-only commands need `record_review` or ownership. The rule is applied in **one** helper, called by every path:
  - `storage/actions.rs` (`list_records`, `work_reads`) and `storage/record_query.rs` (queries, summaries, ranks);
  - `query/record_components.rs` and the record routes;
  - `query/canvas.rs` and `canvas_ledger.rs`;
  - `storage/shared_reads.rs` (guests see shared collections only);
  - `storage/commands.rs`;
  - the live-event delivery gate (`storage/sharing.rs`);
  - composer Ask (`composer/ask.rs`), MCP (`mcp/tools.rs`) and Slack (`slack/flows.rs`).
- **Composer:** it can generate both properties from plain words, such as "each person sees only their own responses" or "only reviewers can grade". Generation prompts and admission tests are added; there are no templates with classroom names.

### W5: live-interaction spike (done 2026-11-14: NO-GO)
- Show whether a live quiz can be expressed with general primitives: a shared current item the owner advances, responses in an `own` list, and a live summary or leaderboard view. Report what is missing. A "go" means a small general primitive, never a quiz feature.
- **Result** ([finding](../../evidence/APP-193/finding-2026-11-14.md)): NO-GO.
  - Gaps: one answer per person (G1), closing an item (G2), a reference picker (G3), a clock (G4) and a published leaderboard (G5).
  - The spike also found the refresh herd (G6), which affects every shared app at 25 members. It is fixed before the class as **APP-195**:
    - live refreshes start after a random 0–3 s delay, with one retry;
    - the data pool's bounded waiter set grows from 8 to 32, within the same 5 s deadline;
    - a `refresh-herd` load campaign proves it;
    - it ships in release B, with no migration. The pool change is W1's area and the client change W2's.
  - **Measured, then re-decided (2026-11-14, Fable advisor in lieu of Victor).** Built as above, the herd still failed: at most 5 pages passed, and the pre-approved fallback of 4 data connections failed too. One page refresh takes 17 transactions at about 64 ms each, and about 45 ms of each is the catalog comparison that runs on every connection reuse. The re-decision:
    - **APP-195** keeps the 0–3 s spread and the single retry, adds visibility deferral (a hidden tab refreshes when it is looked at), and keeps the 32-waiter set. Its run A criteria become p95 current within 6 s and every page within 10 s. The `refresh-herd` campaign stays quarantined, out of CI, until APP-197 makes it pass.
    - **APP-197** (W1 area, Tier A) runs the catalog comparison at connection creation and readiness, and on reuse only when the connection's last check is over 30 s old. Identity, baseline and schema version are still checked on every reuse. ADR-004 gains amendment A5: the check defends against operational drift, the application roles hold no DDL privilege, and no claim against a hostile database administrator is made. This makes each transaction about three times cheaper. Amendment A6 (4 workspace, 4 data, 2 control and 2 worker connections, 12 in all) is drafted and applies only if run A still fails after A5.
    - **W4 (APP-191):** a commit to an `own` list sends a live hint only to pages that can see the row. The class's real herd is 25 submissions a minute; measured, they land in 2.6 s and the reviewer page is current 67 ms after the last one.
    - **APP-198** (W4 area): a list that changed while a view ran is re-read once, then shown as "Updating…". It is never shown as "You don't have access to this card".
    - **APP-199**, after the class: one transaction per read path and a server-advised spread.
    - No load test runs on beta, because beta is the cohort's organization. A scratch-RDS measurement happens only on Victor's word: it needs an IAM policy for the restored instance and one task run, under $2.
  - Post-class order: G1, then G2 with G3 (and G4), then G5 only after an ADR decides the D7 exception.

### Class guidance: live updates with 25 open pages

This goes in the runbook (APP-188) and on the Nov 24 checklist. Victor's text:

> **Live updates and 25 open pages.** When you change something on a page that students have open, every open page re-reads it. Release B makes this about three times cheaper, and the measured limit is written in the runbook. Three rules keep you well inside it:
> 1. **Anything students write goes in a private-entries list** (answers, submissions, check-ins). Their saves then update only your page and their own, not the other 24.
> 2. **Shared lists are yours to write.** Post an assignment or advance an item, then wait about ten seconds before the next change to the same page. One change at a time.
> 3. **One live page on the projector: your reviewer summary.** Students don't need to keep a shared page open after they've submitted. A page they come back to updates when they look at it.
>
> **What students may see during a burst, and what to say:**
> - "Updates are unavailable": wait a few seconds and press Refresh.
> - A form that says "The result of your command could not be confirmed" or "The record was busy. Submit again in a moment": submit the same form again. That is safe, because the same form can't save twice; it can only return the stored result. Only a fresh submission from a new form makes a second entry, and then both count in summaries and the reviewer takes the newest.
> - "You don't have access to this card" right after someone else saved: the card is stale, not a lost permission, so press Refresh. *(Delete this item once APP-198 has shipped; item 6 of the go/no-go list requires it before codes go out.)*
>
> **Fallback:** if live updates misbehave, stop relying on them and refresh your projected summary by hand between items. Submitting through forms doesn't depend on live updates.

## Rules for the agents

1. One worktree and branch per workstream, and never the shared main checkout. Workstreams own disjoint files as listed. A shared file changes only in its owner's workstream.
2. Interfaces first. W1 publishes the storage API and the migration shape; W4 publishes the visibility helper's signature. Others build against them.
3. **Mocks, not live systems:** stub model servers, fake AWS clients, the local dev stand. At most one Playwright smoke per UI step. **No agent touches AWS, sends email or approves anything.**
4. Tests first, proven by breaking. W4 adds a campaign that walks **every** read and write path with a contributor, a reviewer, another member and a guest. Each path's check fails with the helper bypassed.
5. Every PR gets an independent reviewer agent, with the review file in `evidence/`. Tier A (W1, W4, the invite and admin routes, admissions, deletion) gets a recheck. A blocker stops the merge.
6. `just check`, the focused campaigns, and CI green (28 checks). Merge with `gh pr merge N --merge` only. No force push. **Regression guard:** the existing data, folder, sharing and pitch campaigns stay green, because the data scenarios must not degrade.
7. Victor-only actions stay his; agents prepare exact checklists with the expected output.
8. A daily status to Victor: what merged, what is red, and what he does that day.

## Go/no-go for the 25 codes (Nov 28)

All yes, or no codes go out.
1. **Release B read back:** schema 24, `/health/ready`, expiry 2026-12-16, November and December ledger months, scan disposition valid to Dec 15, nothing left to deploy.
2. **Rehearsal of Nov 26 passed** on Chrome, Safari and a phone with at least two students. Each red item is fixed and re-run, or accepted in writing. The verification mail reached a Gmail and an Outlook address within five minutes, and not in spam.
3. **Guards live:** the daily cap and its message seen on beta; Anthropic limits set for November ($150) and December ($100), with 50% alerts; alarms OK and the test email received; the restore drill recorded.
4. **`/admin`:** Victor is admin, consent v1 shows for the rehearsal members, Remove is tested, and the test members are removed.
5. **Victor's apps are built and verified** with the helper account (own entries, the reviewer command, summaries) and shared with everyone in the organization with the intended roles.
   - The helper account fills every form field of every app, and no form shows an ID text box (G3). "Which X" fields are enums or short text.
   - "Advance" or "close" is a reviewer-only update of a state field, never a transition, because a record's creator can't fire a transition on it.
   - A shared student page holds at most one typed query view; record summaries are used otherwise.
6. **`refresh-herd` green on main** (APP-195, APP-197) at the revised criteria. The catalog-check count and the sustainable commit cadence are written in the runbook, and Victor's beta document-read timing is recorded. APP-198 (stale views are never shown as "no access") is merged.
7. **Externals:** FormaStack confirms in writing an fAuth tenant limit of at least 30; a test mail to feedback@getforma.dev arrives; the per-address limits are raised, and the code email says to redeem at home.
8. **Codes path:** `admission.py issue --count 25` passed with fake AWS; codes exist only in the private file; the one-shot secret is cleared; the evening restart is scheduled; the code email is drafted with "bring a laptop", the beta statement and the fallback.
9. **No open Tier A blocker,** CI green on main, and a known-issues list for students written. The list includes: a second answer or submission is saved too and both count in summaries (the reviewer takes the newest), a late answer still counts, and there is no timer; and a page may take up to ten seconds to show someone else's change (press Refresh to see it sooner).

## Verification (end to end)

- **Per PR:** `just check`, the focused campaigns, the review file, CI.
- **Release A (Nov 15):** `/health/ready` passes; the schema version reads 22.
- **Release B (Nov 23):**
  - `/health/ready`;
  - the schema version reads 24;
  - the expiry reads Dec 15;
  - alarms OK;
  - a test alarm email arrives;
  - the restore drill matches.
- **Victor's build day (Nov 24):** with the composer, he creates:
  - an attendance app whose check-ins are private, with Victor as reviewer;
  - a homework app with shared assignments, private submissions and a reviewer-only Grade command;
  - a quiz app with private answers and a summary.

  Using a helper account, confirm that each person sees only their own entries, Victor sees all, a contributor can't grade, and the summaries match. The helper fills every field of every form. The 25-member herd is proved by the `refresh-herd` evidence (APP-195), because a rehearsal with 2–3 students can't show it.
- **Rehearsal (Nov 26) with 2–3 students:**
  1. Code, notice and checkbox; sign up; the verification email arrives.
  2. Set a name.
  3. Open the shared apps and submit.
  4. Victor grades; the student sees their grade, and nobody else's.
  5. The student builds and shares their own app.
  6. `/admin` shows everyone with consent v1.
  7. Remove a test member: their session dies.
  8. Repeat on Safari and on a phone.
- **Class weekend:** watch alarms, `/admin` and feedback mail. No deploys.
