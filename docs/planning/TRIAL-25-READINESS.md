# Trial readiness: 25 real people on the beta

Status: **approved by Victor on 2026-11-14; executing.** Tickets: APP-181 (charter) through APP-189 (go-live). The earlier draft of this file (commit `3018296a`) is superseded by this plan.

## Context

Victor wants 25 real people to sign up and try real things on the beta, each in a clean workspace. The trial is enforced the way a production SaaS would enforce it.

A read-only review (draft at `docs/planning/TRIAL-25-READINESS.md`, branch `plan/trial-25-readiness`) found the beta not ready:

- **No reviewed invite path.** `ops/beta/run-step.sh` is fenced (`ops/beta/lib.sh:47-51`), and `ops/promotion` has no admission command.
- **The charter is synthetic-only** (ADR-008:7-8).
- **The release is stuck.**
  - Run 43157872671's ledger basis is stale (`infra/scripts/preflight.py:429`).
  - Its scan disposition expires 2026-11-20.
  - The beta scales to zero 2026-11-25 08:00 UTC.
- **Real-user basics are missing:** no consent, no admin, no member removal, no model spend cap, no alarms.

What works today:
- The member journey (APP-139 boxes 1–3, APP-145).
- SES, verified live 2026-11-14: production access on, `getforma.dev` verified with DKIM, suppression on.
- Per-member privacy of apps inside one organization.

## Decisions (locked 2026-11-14)

| # | Decision |
| --- | --- |
| D1 | A real-user trial charter as an ADR-008 amendment: 25 named people, invite-only, beta label, no SLA, data terms, accepted risks. |
| D2 | One shared trial organization (`forma-trial`); each member's apps stay private unless shared. |
| D3 | Harden the beta to production standards. A separate production environment (ADR-013) is planned after the trial. |
| D4 | **Admin follows the standard SaaS split.** An Organization Admin area in the product, for org owners and admins (`admin_policy`), plus the audited operator back office for platform work. Victor is the trial org's admin. |
| D5 | **Consent is durable and enforced.** A required checkbox tied to a notice version, stored in the database and audited. |
| D6 | **Spend:** Victor sets an Anthropic spend limit (~$100 a month). The app enforces an app-wide daily model-call cap: default 300, overridable by env for tests, with a plain message when reached. AWS ceilings are raised for the trial window. |
| D7 | Invite codes go out from Victor's own email. No app-sent email (APP-165 deferred). Feedback goes to `feedback@getforma.dev`, linked in the app. |
| D8 | Before wave 1: alarms to Victor's email, plus one restore drill. APP-078 is deferred (one real organization). |
| D9 | **Compressed calendar:** rehearsal Thu **Nov 20**, wave 1 (5 people) Fri **Nov 21**, wave 2 (20) Mon **Nov 24**, trial ends Fri **Dec 5**. The beta is renewed to **Dec 8**, with a December ledger month. |
| D10 | **One schema release, 20→23.** Migrations 0021 and 0022 (APP-175) and the new 0023 ship together, which saves a release cycle. The stuck run is abandoned. Fallback below. |

Assumptions: adults who expect a rough beta; no regulated data; English; desktop Chrome and Safari; Victor is the only operator.
Out of scope: public sign-up; one organization per person; app-sent email; Slack for trial users; more than 25 people; a separate production stack.

## Calendar and critical path

| Date (UTC) | What | Who |
| --- | --- | --- |
| Fri Nov 14 | Charter amendment and tickets. `PROMOTION_ENABLED=false` while building, so no runs queue. Abandon 43157872671 after its 65-minute fence. APP-175 box 3 permission readback. | agents + **Victor** |
| Nov 14–18 | Three parallel build workstreams (W1, W2, W3 below), each in its own worktree, each PR independently reviewed. | agents |
| Tue Nov 18 | Tier A reviews closed. Schema-release file extended to 23. Merge W1 and W2. | agents |
| Wed Nov 19 | `reconcile observe`/`apply`, re-enable promotion, approve the 20→23 release. Apply the expiry renewal and the December month. Deploy the alarm template. Set the Anthropic spend limit. FormaStack raises the fAuth tenant limit to at least 30. | **Victor** |
| Thu Nov 20 | Restore drill (agents prepare, Victor runs). Dress rehearsal with a fresh alias (`victorvillacis99+trial1@gmail.com`) in Chrome and Safari. Fix-forward if needed. | **Victor** + agents |
| Fri Nov 21 | Wave 1: issue 5 codes and one announced restart; Victor emails the codes. | **Victor** |
| Mon Nov 24 | After 48 hours of watching: wave 2 (20 codes, one restart). | **Victor** |
| Dec 5 → Dec 8 | Trial ends. Purge the trial organization's data per the charter (procedure from W3). | **Victor** |

**Fallback.** If 0023 slips past Nov 19, promote `main` with schema 20→22 only before Nov 20. Then renew the scan disposition (W3.5) and ship 0023 in a second release on Nov 21. Wave 1 moves to Nov 24.

## Workstreams (built in parallel by agents)

### W1: migration 0023 (Tier A). Files: `app/migrations/0023_*.sql`, `catalog-v23.json`, `manifest.json`, `app/src/storage/admission.rs`, `sessions.rs`, the new `app/src/storage/org_admin.rs`, `ops/promotion/schema-release.json`.
- **W1.1 Consent record.**
  - An insert-only `genui.member_consents (organization_id, principal_id, notice_version, consented_at)`.
  - The pending-admission row carries `notice_version`, filled at `/invite/validate`, so it survives the fAuth round trip (`record_pending_admission`, `sessions.rs:595`).
  - `genui_private.admit_redemption` (0003:329) records it in the same transaction and appends the audit kind `admission.consented`. The kind is added to the `admission_chain` policy (0003:162-164; 0008:85 restates it).
  - Follow the version-check pattern in 0022:9-11.
- **W1.2 Org-admin read.** A SECURITY DEFINER `genui_private.org_admin_overview(org, caller)`, owned by a NOLOGIN role (the `genui_admit` pattern, 0003:12):
  - It requires a live membership with org `admin_policy` (`storage/authorization.rs:59`).
  - It returns per member: membership status, consent version and time, last session, apps owned, works, records, uploads, and the code/redemption status.
  - EXECUTE is granted to `genui_workspace`. Everything else is denied.
- **W1.3 Remove a member.** `genui_private.admin_remove_member(org, caller, principal)` follows the operator semantics of revoke membership plus entitlement (`admission_operator.py:1031-1057`). It appends the audit event, refuses removing oneself or the last admin, and bumps the epoch so sessions die.
- **W1.4** Extend `schema-release.json` from 21–22 to 21–23, with its reviewed digest.

  Tests: storage tests and a new `org-admin` campaign (`just campaigns` registry): consent stored and audited; a non-admin refused; a cross-organization caller refused; removal kills the session; audit chains verify.

### W2: app (Tier A for the invite and admin routes). Files: `app/src/auth/invite.rs`, `invite_tests.rs`, `app/admin/src/invite/app.tsx`, `app/src/pages.rs`; the new admin route, page and props (`app/admin/build.ts` route plus page, bound in `app/src/main.rs`); `app/src/composer/model.rs` and its callers; the canvas message maps.
- **W2.1 Notice and consent at `/invite`.**
  - The plain-language notice (text in the charter) is shown on the invite page.
  - The form posts exactly two fields, `code` and `consent=<NOTICE_VERSION>`. Today it allows one field (`invite.rs:458-464`). Update the module doc and the extra-field tests (`invite_tests.rs:379-385`).
  - A missing or wrong consent gets a neutral refusal.
  - SSR test: extend `main.rs:657-713`.
- **W2.2 Organization Admin page `/admin`.**
  - SSR, behind `routes::member` (`query/routes.rs:350`).
  - Non-admins get the uniform 404.
  - It shows the member table from W1.2.
  - "Remove member" is a CSRF-protected POST with a confirmation, calling W1.3.
  - The canvas top bar shows an "Admin" link only to admins.
- **W2.3 Daily model-call cap.**
  - A process-wide counter that resets at UTC midnight, checked in `Anthropic::send` (`model.rs:413`) and `complete_with_documents` (`:346`). It is a static, because five clients exist (`main.rs:255/291/292/294`, `slack/mod.rs:173`).
  - A new `ModelError::DailyCap` (code `model_daily_cap`) maps to 429 `daily_limit` in compose, Ask, Shape, folder Ask and explain.
  - The canvas messages (`compose.ts`, `ask.ts`, `explain.ts`, `box-api.ts`) say: "Today's AI limit for the trial is reached. It resets at midnight UTC."
  - Default 300 is in code. `GENUI_MODEL_DAILY_CALLS` overrides it for tests. No stack change: infra changes block promotion (`ops/promotion/candidate.py:172-200`).
- **W2.4** A feedback link (`mailto:feedback@getforma.dev`) on the canvas and the admin page.

  Tests: the invite campaign (consent), the org-admin campaign (W1), and the composer and actions campaigns (cap, with the env override set low). `just check admin test http-smoke`.

### W3: operator back office and operations (Python, `ops/promotion` module pattern: `AWS()`, `identity()`, `Store`, `--expected-state-sha256`, receipts; fake-AWS unit tests like `ops/promotion/test_maintenance.py`)
- **W3.1 `ops/promotion/admission.py`.**
  - Commands:
    - `issue --count N`: mint codes locally, write the invitation-input secret, RunTask the `OperatorTask` with the issue override for the trial org, wait, clear the secret, and write the codes to a private file;
    - `list`;
    - `revoke --code`;
    - `grant-admin --principal`: the operator `grant-permission admin_policy`;
    - `delete-uploads --principal`: list the member's object keys through the operator task, then delete every S3 version with Victor's identity;
    - `restart`: force a new deployment, announced.
  - Add trial-org entries to `app/operator/overrides.json` (it hard-codes Northstar today).
  - One restart serves every invite issued before it, up to 128 (`runtime.py:186-189`).
  - First confirm with a read-only `describe-stacks` that the `OperatorTaskDefinition` and provider-management parameters are still set (`stack.ts:499-504,549-561`).
- **W3.2 `expiry_renewal.py` and `month_renewal.py`.** Pinned, plan/apply, receipts, following the precedent of `reservation_cap.py` and `expiry_policy.py`. Plus an ADR-019 amendment with the trial ceilings: November GenUI $210 / project $248; December through Dec 8 at GenUI $70 / project $108. Victor confirms the numbers at apply, from `reconcile observe`.
- **W3.3 `ops/monitoring/alarms.json`.** A standalone CloudFormation template, outside `infra/src` so it doesn't block releases. It holds an SNS email topic to Victor; alarms on load-balancer 5xx, unhealthy targets, ECS running tasks below 1, and RDS free storage; and an AWS Budget alert. A Python test pins its resources. Victor deploys it once.
- **W3.4 `docs/operations/TRIAL-RUNBOOK.md`.** Waves and restarts, member removal (admin page plus `delete-uploads`), the deny-all switch, rollback, how feedback is handled, the restore drill steps, and the end-of-trial purge of the trial organization.
- **W3.5 Scan disposition renewal** through Dec 8, used only on the fallback path.

### W0: charter and tickets (agents, Nov 14)
- **W0.1** The ADR-008 amendment: the trial charter, the accepted risks (APP-079/080 unfinished, single task, single-AZ database, sign-out on deploy), and the notice text for Victor's approval.
- **W0.2** One ticket per step above under `Tickets/` (board regenerated and checked). Also update `docs/planning/TRIAL-25-READINESS.md` to this plan, and fix the stale SES text (`CLEAN-TRIAL-DEPLOY.md:62`, `BLOCKERS.md:12`).

## Rules for the agents (how the parallel work stays safe)

1. **Isolation.** One worktree and one branch per workstream. Never touch the shared main checkout. Workstreams own disjoint files. W1 owns migrations and storage; W2 owns routes, pages and admin TS; W3 owns `ops/` and `docs/operations`. Shared files (`main.rs`, `overrides.json`) change only in the owning workstream.
2. **Interfaces first.** W1 publishes the definer signatures and the Rust storage API (`org_admin.rs`) in its first commit. W2 builds against them with a test stand-in until W1 merges.
3. **Mocks, not live systems.**
   - Stub model servers (the existing campaign stubs) and fake-AWS clients (the existing `ops/promotion` tests).
   - The local dev stand's clean profile for end-to-end checks.
   - At most one Playwright smoke per UI step.
   - **No agent makes AWS calls or mutations, sends email, or approves anything.**
4. **Tests first, proven by breaking.** Each rule gets a unit test that fails before the change. Each campaign check fails once with its wiring removed.
5. **Review.** Every PR gets an independent reviewer agent, and its review file goes in `evidence/`. Tier A (migration, admission, consent, admin authority, deletion) needs a second-pass recheck. A blocker stops the merge.
6. **Merge rules.** `just check` plus the focused campaigns green locally. CI green (28 checks). Merge with `gh pr merge N --merge` only. No force push, no history rewrite.
7. **Victor-only actions** stay Victor's: deployment approvals, AWS operator applies, FormaStack, the Anthropic limit, codes to people. Agents prepare exact checklists with the expected output for each.
8. **Checkpoint.** A daily status to Victor: what merged, what is red, and what he needs to do that day.

## Verification (end to end)

- **Per PR:** `just check`, the focused campaigns, the review file, CI.
- **Release (Nov 19):** `/health/ready` passes; the schema version reads back as 23; the expiry reads Dec 8; alarms are in OK state.
- **Restore drill:** restore the latest snapshot to a scratch instance; the schema version and row counts match; then delete the scratch instance.
- **Rehearsal (Nov 20), with a fresh alias:**
  1. Code with notice and checkbox (no checkbox gets a refusal).
  2. Register in fAuth; the verification email arrives via SES.
  3. Empty canvas → build an app → save a record.
  4. Victor's `/admin` shows the member with consent v1.
  5. The cap message appears with a low env cap on the dev stand.
  6. Remove the member: their session dies and they get the 404.
  7. `delete-uploads` removes their files.
  8. Safari repeat.
- **Waves:** watch alarms, `/admin` and feedback mail for 48 hours before wave 2.
