# Real live test: 25 real people on the beta (Victor's class, Nov 29–30)

Status: **approved by Victor on 2026-11-14; executing.** Tickets: APP-175 (release A), APP-181 to APP-194; go-live is APP-189. Earlier drafts of this file are superseded.

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
| D9 | **Share with many (general):** share an app with "everyone in the organization", and choose a role when sharing: viewer, contributor or reviewer. |
| D10 | **Spend:** an app-wide daily model-call cap, default **400 a day** in code (env override for tests), with a plain message when reached. Victor sets an Anthropic spend limit, about **$150 for the class month**. Per-member hourly limits stay. |
| D11 | **Live interaction (stretch):** a Kahoot-style live quiz is attempted only if a design spike shows it can be built from general primitives (a shared "current item", timed responses, a live summary). Go or no-go on **Nov 24**; if no, the class uses formative quizzes. |
| D12 | Invite codes go out from Victor's own email. No app-sent email. Feedback goes to `feedback@getforma.dev`, linked in the app. |
| D13 | Alarms to Victor's email, plus one restore drill, before codes go out. APP-078 is deferred (one real organization). |
| D14 | **Two schema releases.** Release A: today's `main` (schema 20→22) before the Nov 20 scan expiry. Release B: migration 0023, all new data capabilities, on Nov 23, with a renewed scan disposition. |

Assumptions: adults; no regulated data; English; desktop Chrome and Safari plus phones for check-ins; Victor is the only operator; the class needs internet access.
Out of scope: anything classroom-specific in the platform; public sign-up; organization per person; app-sent email; more than 25 students; a separate production stack.

## Calendar

| Date | What | Who |
| --- | --- | --- |
| Fri Nov 14 | Charter and notice v1 (cohort text), tickets revised. Promotions paused (done). Victor: APP-175 permission readback; reject or cancel run 43157872671. | agents, **Victor** |
| Sat Nov 15 | After the 65-minute fence: `maintenance inspect`/`abandon`, `reconcile`, re-enable promotion, approve **Release A** (main, 20→22). | **Victor** (agents prepare the checklist) |
| Nov 15–21 | Parallel build: W1–W4. W5 spike Nov 17–18. | agents |
| Sat Nov 22 | Tier A reviews closed; merge W1–W4. | agents |
| Sun Nov 23 | Renewed scan disposition. **Release B** (22→23). Expiry renewal to Dec 15 plus the December month. Alarm template deployed. Anthropic limit set. FormaStack raises the fAuth tenant limit to at least 30. Restore drill. | **Victor** |
| Mon Nov 24 | W5 go or no-go. Victor builds his class apps on beta with the composer, using a fresh alias plus one helper account. | **Victor** + agents fix forward |
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
- **Grammar:** a record schema gains `visibility: "shared" | "own"` (default `shared`) and commands gain `allowed: "contributor" | "reviewer"` (default `contributor`) in `admission/validate.rs:966`. These go through the contract edition update (`contracts/`, lock), and older definitions stay valid unchanged.
- **One storage rule:** in an `own` collection, a member sees and changes only rows they created, unless they hold `record_review` or own the app. Reviewer-only commands need `record_review` or ownership. The rule is applied in **one** helper, called by every path:
  - `storage/actions.rs` (`list_records`, `work_reads`) and `storage/record_query.rs` (queries, summaries, ranks);
  - `query/record_components.rs` and the record routes;
  - `query/canvas.rs` and `canvas_ledger.rs`;
  - `storage/shared_reads.rs` (guests see shared collections only);
  - `storage/commands.rs`;
  - the live-event delivery gate (`storage/sharing.rs`);
  - composer Ask (`composer/ask.rs`), MCP (`mcp/tools.rs`) and Slack (`slack/flows.rs`).
- **Composer:** it can generate both properties from plain words, such as "each person sees only their own responses" or "only reviewers can grade". Generation prompts and admission tests are added; there are no templates with classroom names.

### W5: live-interaction spike (Nov 17–18; build only on a "go" by Nov 24)
- Show whether a live quiz can be expressed with general primitives: a shared current item the owner advances, responses in an `own` list, and a live summary or leaderboard view. Report what is missing. A "go" means a small general primitive, never a quiz feature.

## Rules for the agents

1. One worktree and branch per workstream, and never the shared main checkout. Workstreams own disjoint files as listed. A shared file changes only in its owner's workstream.
2. Interfaces first. W1 publishes the storage API and the migration shape; W4 publishes the visibility helper's signature. Others build against them.
3. **Mocks, not live systems:** stub model servers, fake AWS clients, the local dev stand. At most one Playwright smoke per UI step. **No agent touches AWS, sends email or approves anything.**
4. Tests first, proven by breaking. W4 adds a campaign that walks **every** read and write path with a contributor, a reviewer, another member and a guest. Each path's check fails with the helper bypassed.
5. Every PR gets an independent reviewer agent, with the review file in `evidence/`. Tier A (W1, W4, the invite and admin routes, admissions, deletion) gets a recheck. A blocker stops the merge.
6. `just check`, the focused campaigns, and CI green (28 checks). Merge with `gh pr merge N --merge` only. No force push. **Regression guard:** the existing data, folder, sharing and pitch campaigns stay green, because the data scenarios must not degrade.
7. Victor-only actions stay his; agents prepare exact checklists with the expected output.
8. A daily status to Victor: what merged, what is red, and what he does that day.

## Verification (end to end)

- **Per PR:** `just check`, the focused campaigns, the review file, CI.
- **Release A (Nov 15):** `/health/ready` passes; the schema version reads 22.
- **Release B (Nov 23):**
  - `/health/ready`;
  - the schema version reads 23;
  - the expiry reads Dec 15;
  - alarms OK;
  - a test alarm email arrives;
  - the restore drill matches.
- **Victor's build day (Nov 24):** with the composer, he creates:
  - an attendance app whose check-ins are private, with Victor as reviewer;
  - a homework app with shared assignments, private submissions and a reviewer-only Grade command;
  - a quiz app with private answers and a summary.

  Using a helper account, confirm that each person sees only their own entries, Victor sees all, a contributor can't grade, and the summaries match.
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
