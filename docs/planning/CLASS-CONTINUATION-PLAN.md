# Class continuation plan: keep everyone on a free plan, beta to Jan 15

Written 2026-11-15 from Victor's four decisions of the same day. It replaces the "End of the test" purge promised in the ADR-008 amendment and planned in [POST-CLASS-FOLLOW-UPS.md](POST-CLASS-FOLLOW-UPS.md) P1 and [runbook §10](../operations/TRIAL-RUNBOOK.md). It is a plan and a set of findings read in the code at `main` `4203bb0f`; nothing in it is built. Every line reference is to that commit.

The tickets are [APP-227](../../Tickets/0-backlog/APP-227.md) (notice v2 and the charter amendment), [APP-228](../../Tickets/0-backlog/APP-228.md) (the renewal to Jan 15 and the January budget), [APP-229](../../Tickets/0-backlog/APP-229.md) (`extend-entitlement`), [APP-230](../../Tickets/0-backlog/APP-230.md) (per-member deletion), [APP-231](../../Tickets/0-backlog/APP-231.md) (the free-plan AI switch) and [APP-232](../../Tickets/0-backlog/APP-232.md) (the production-move decision by Jan 1).

## 1. Decisions (Victor, 2026-11-15, final)

1. **Keep everyone by default after the class.** Students' accounts, organization membership, apps and data continue on a free plan after the class unless the student asks for deletion at privacy@getforma.dev. This replaces the charter promise "the organization's data is deleted by Dec 14". It ships as **notice v2 before the student codes go out on Nov 28**. Only Victor's own account has consented to v1.
2. **The beta keeps running to Jan 15, 2027.** Today the automatic stop is pinned (`ops/promotion/expiry_renewal.py:35`, `RENEWED_EXPIRY = '2026-12-16T08:00:00Z'`; `ops/promotion/month_renewal.py:29`, `MONTH = '2026-12'`), the ADR-019 amendment sets the monthly caps, and every release subtracts 18 hours from the stop (`ops/promotion/runner.py:226`). The live stop after release F (organization roles) is **2026-12-11T20:00Z**.
3. **The free plan after the class:** a small in-house AI allowance (about 10 model calls a day per member) plus unlimited own AI through MCP (agent tokens, `app/src/mcp/`; it costs Forma nothing). Pro and paid tiers come later with billing. Today the caps are `GENUI_MODEL_DAILY_CALLS` (1,000) and `GENUI_MODEL_DAILY_CALLS_PER_MEMBER` (80), in memory and process-wide (`app/src/composer/model.rs:86-96`). APP-222 part B plans per-organization allowances as data.
4. **The platform stays general.** These are plan and organization mechanisms, never class-specific code ([POST-CLASS-FOLLOW-UPS.md](POST-CLASS-FOLLOW-UPS.md) A13: limits are data, not class constants).

In Victor's words: "Keep everyone by default", "Renew to Jan 15", "Small daily allowance + own AI via MCP".

## 2. Findings, read in the code

### 2.1 The notice and consent

- **Where the text lives.** `app/src/auth/notice.rs:12` `NOTICE_VERSION: i32 = 1`; `:15` `NOTICE_TITLE`; `:19-26` the six `NOTICE_PARAGRAPHS`; `:29` the checkbox label. The "Your data" paragraph (`:24`) carries the Dec 14 promise, feedback@ as the data-request address and "we ask it to delete the account after the test". The module test (`:47-83`) pins `NOTICE_VERSION == 1`, the phrase "December 14, 2026" (`:51`), "feedback@getforma.dev" (`:52`), the "Your data" paragraph word for word (`:74-80`) and refuses the words class, student, teacher and Victor (`:53-55`). Every one of those assertions changes with v2.
- **How consent is stored.** The invitation page sends `consent=1`; the reservation carries `NOTICE_VERSION` (`app/src/auth/invite.rs:401`; `app/src/storage/sessions.rs:197-198`, `:531` into `genui_private.reserve_code`). `genui_private.admit_redemption` (current definition `app/migrations/0025_organization_roles.sql:159-287`) refuses a pending admission without a notice version (`:200`), inserts one `genui.member_consents` row (`:281-282`) and appends `admission.consented` on the admission chain with a digest of the version (`:271-272`). The table (`app/migrations/0023_consent_names_org_admin.sql:64-72`) is one row per member and version, insert-only, written only by `admit_redemption`; "no application role holds any privilege on it" (`0023:58-61`). `/admin` shows each member's latest `consent_version` and `consented_at` (`0025:338-372`, `org_admin_overview`).
- **An existing member is never re-asked.** Sign-in resolves a member through principals, memberships, organizations and entitlements (`app/src/storage/sessions.rs:781`) and never reads `member_consents`; no route compares a member's consent version with `NOTICE_VERSION`. Bumping the version changes only what new redemptions record.
- **The simplest correct way for pre-v2 members.** Ship v2 **before any further code is redeemed** (the rehearsal codes of Nov 26, runbook §4), so every student and helper consents to v2 at the invitation page and the only pre-v2 members are Victor's own accounts (his Nov 1 Gmail account and the build account if it is redeemed before v2 is live). For those, an **operator note** is sufficient and honest: Victor is the operator and the organization's owner, so his own consent protects no one from him; his written acceptance of v2 goes in the private approvals file with the release record. A consent prompt at sign-in, in the pattern of the first-run name prompt (`app/src/member_name.rs:1-4`, `app/src/query/canvas.rs:123` `needs_name`), is the right general primitive for the next notice change with real users, but it needs a new definer and grant on `member_consents` (a migration) plus a canvas interstitial and a campaign; that is listed under "later" (§4.3), not before Nov 28.

### 2.2 Entitlements

- Every redemption inserts an entitlement of `now()+interval '30 days'` (`0025:243-244`, originally `0003:409-410`). The table (`0003:89-98`) has `status IN ('active','expired','revoked')`, `expires_at NOT NULL` and `CHECK (expires_at > starts_at)`; nothing sets `expired`, so a lapsed member is `active` with a past `expires_at`.
- A lapse ends sign-in (`sessions.rs:781` and `:816` both require `e.status='active' AND e.expires_at>clock_timestamp()`) and every guest share the member sponsors (`app/src/storage/shared_access.rs:272-273` `sponsor_clause_for`, rechecked at `:385` and `:480`).
- Dates: Victor's Nov 1 account lapses **2026-12-01T20:15Z** (runbook §11.5); rehearsal codes redeemed Nov 26 lapse Dec 26; student codes redeemed Nov 28 lapse **Dec 28**, before Jan 15. The build account redeemed on or after Nov 15 lapses on or after Dec 15.
- The operator's `extend` reaches only unredeemed codes (`app/runtime/admission_operator.py:1303-1320`, bounded by the code's 30-day issuance maximum). Nothing extends an entitlement. `revoke_entitlement` (`:1331-1346`) is the pattern for a member-level change: organization `FOR SHARE`, member `FOR UPDATE`, the row update, one operator event.
- **The smallest correct `extend-entitlement`.** A new subcommand in `admission_operator.py` (and its `app/operator/overrides.json` entry plus `ops/promotion/admission.py` front): `extend-entitlement --organization <org> --until <timestamp> (--principal <id> | --all-active)`. In one transaction, as the operator's `genui_owner` identity (`:692-702`): lock the organization (active), then for each target membership with `status='active'`: `UPDATE genui.entitlements SET expires_at=greatest(expires_at,$until) WHERE status='active' AND expires_at<$until`, and append one `entitlement.extended` event per changed member on the operator chain (actor the member, outcome digest of the new `expires_at`). Refuse `until` in the past, `until` more than 400 days ahead, a lowered value (the `greatest` makes lowering impossible; a row already past `until` is reported `changed: false`), and an inactive organization. No epoch advance: an extension revokes nothing. **It needs migration 0026**: the operator chain's closed kind list is a policy `CHECK` (`0025:118-122`) and `append()` refuses an unlisted kind (`admission_operator.py:717-719`); `OPERATOR_KINDS` (`:166-170`) gains `entitlement.extended`. The same migration can carry the deletion kind of §2.3. Because the update also revives a lapsed-but-not-revoked member, the command run after release H restores Victor's Nov 1 account without an interim SQL.

### 2.3 Deletion on request: what an individual's data is

Tables that name a member, with the migration that defines them:

| Data | Where | On deletion |
| --- | --- | --- |
| The principal (issuer, provider tenant, provider subject) | `genui.principals` (`0002:31-37`) | **Keep as an identifier row** (see the FK below); the subject is fAuth's user id and points nowhere once the fAuth user is gone |
| Membership, display name, role, epoch | `genui.memberships` (`0002:38-42`, `0023:90-92`, `0025`) | Status `revoked`, `display_name` NULL, role `member` |
| Entitlements | `genui.entitlements` (`0003:89-98`) | Revoked (`revoke_entitlement`) |
| Consents | `genui.member_consents` (`0023:64-72`) | Keep: version and time only, the legal record |
| Code redemptions, provider invitations | `0003:118-137` | Keep: identifiers only |
| Sessions, pre-access sessions | `0006:41-63` | Delete |
| Agent tokens and attributions | `0010:42-65` | Delete (attributions name works; delete with the token) |
| Slack links | `0009:33-44` | Delete |
| Member cards, member labels | `0015:46`, `0014:54` | Delete |
| Uploads: `box_files.uploaded_by`, boxes, grants, briefs | `0011:41-141` | S3 versions by `admission.py delete-uploads` (`ops/promotion/admission.py:25`), then the rows |
| Apps they own and everything under them: revisions, publications, saved work, collections, records, shares, data copies, permissions | `0002:47-205`, `0008:54`, `0017:48`, `0020:8` | Delete, children before parents, the purge's own walk (`purge-organization`) restricted to one owner |
| Shares they made, shares made to them, live shares they sponsor | `work_shares` (`0008:54`), `live_work_shares` (`0017:48`) | Revoke/delete; a guest's `share_events` stay (identifiers) as at the purge |
| Records they created or updated in **other people's** apps | `internal_record_revisions.creator_id/updater_id` FK → `principals` (`0002:168-178`) | **Policy choice** (§6 Q3): keep with the author tombstoned (the principal row stays, the name is gone) or delete the revisions |
| Audit events naming them | `audit_events.actor_principal_id`, no FK (`0003:107-117`) | Keep: hash-chained, identifiers and kinds only, the notice's "minimal security log" |
| The fAuth account (email, password, sessions, codes) | fAuth tenant | `POST /manage/users/{id}/revoke-sessions` then `DELETE /manage/users/{id}` (hard delete; runbook §10 step 6); optionally `tenant_user_invites.email` and `login_attempts.email` by SQL on the fAuth database (swept after 90 days) |

The FK from record revisions to principals is why the principal row stays: deleting it would delete other owners' records. That is also what the notice should say.

**The procedure (`erase-member`, APP-230).** Two operator tasks and one object sweep, like the purge, for one principal: (1) `admission.py delete-uploads` for their S3 versions (exists); (2) `admission erase-member --organization <org> --principal <id> --confirm <id>`: requires the membership already revoked (`/admin` Remove or `revoke-membership`), refuses the last owner and an active member, then one transaction: the table walk above, children before parents, under the lifted FORCE as the purge does, proving afterwards that no content row of the member remains and the audit chains did not move, with a receipt of the counts before and after; one `member.erased` event on the operator chain (migration 0026). (3) The fAuth deletion by the operator's own identity, recorded with the receipt. The reply to the requester names the date and what stayed (identifiers; entries in others' apps, anonymized or deleted per Q3). Until APP-230 ships, a request is honoured by runbook §5 (remove, delete uploads) plus the fAuth deletion, which is what notice v1 promised during the test, and the full erase follows within the 30 days.

### 2.4 The free-plan AI switch

- Today the per-member allowance is the compiled default 80 (`model.rs:94`), overridable only by `GENUI_MODEL_DAILY_CALLS_PER_MEMBER` (`:96`), which "is not set in the managed deployment: there the code's default holds" (`:88`) and is read once per process (`:468-478`, `OnceLock`). The service's environment is the stack's task definition (`infra/src/stack.ts`; the names are pinned by `infra/test/security.test.ts:264-269`), so setting it is a template change, an infrastructure digest change and therefore a release; a parameter edited outside the promotion makes the next promotion refuse ("live service task revision differs from the reviewed stack", `infra/scripts/preflight.py:570`). The alarm text already says "raise `GENUI_MODEL_DAILY_CALLS` only with a release" (`ops/monitoring/observability.json:257,275`).
- **There is no release-free switch today.** The two ways to get one: (a) an env parameter added in release H, after which the Dec 12 change is still a change set, so still a release; or (b) APP-222 part B: `genui.organization_allowances` and the audited `admission set-allowance` command (APP-222 §5), after which the Dec 12 switch is one operator task.
- **Recommendation: (b).** Part B is already planned, it is the first step of A13, and it is the only path that makes allowances data. Cost: about 2–3 days of build plus review, inside release H (the schema release this plan needs anyway; Part B's migration becomes 0027, since 0025 is now the roles migration). The switch on Dec 12: `set-allowance --organization $TRIAL_ORG --key ai_calls_per_member_per_day --value 10`, a few minutes, no release. If Part B misses release H, keep 80 a day until it ships: the process cap of 1,000 a day (`model.rs:86`) and the $1,000 Anthropic organization limit bound the cost, and the next release that carries Part B subtracts its 18 hours like any other.

### 2.5 The expiry renewal to Jan 15

- **Pins.** `expiry_renewal.py:35` `RENEWED_EXPIRY` → `'2027-01-16T08:00:00Z'` (the next release subtracts 18 hours, so its stop reads **2027-01-15T14:00Z**, the pattern of the Dec 16 → Dec 15 14:00 renewal). `month_renewal.py:29` `MONTH` → `'2027-01'`, `:34` `TRIAL_CEILING_CENTS` to January's figures, and `december_projection` (`:39-58`) renamed to a month projection; it is already generic in everything but its name. The docstrings of both say "December" and "release B" and are rewritten.
- **A month argument or a new pin?** `month_renewal` refuses any month but the pinned one (`:69`), refuses once the month has begun (`:80`) and never lowers an amount (`:85-86`); the receipt key and the projection use `MONTH`. A `--month` argument would need a closed set and a test per month; a re-pin keeps the one-reviewed-value pattern the two commands were built on. **Re-pin.** December needs no re-assessment: a month entry holds only the four reserves (`:90`), and the compute is derived from the windows at reservation time; once December begins `reconcile` owns it anyway.
- **The 32-day bound makes Jan 15 unreachable in one release.** `preflight.reserve` requires the window's expiry within `max_expiry` of the reservation moment (`preflight.py:87` `timedelta(days=32)`, `:363`; mirrored at `:579` for the live check, and in `expiry_renewal.plan` through the projection, `expiry_renewal.py:82-96`). The stop after release G (notice v2) is 2026-12-11T02:00Z, so the renewal release must execute by Dec 10 02:00Z (`runner.py:227` refuses a stop within 24 hours), and from Dec 10 a window reaches at most Jan 11. Two ways out: **(a) raise `max_expiry` to 45 days** (ADR-019 amendment 2; `preflight.py:87`, the message at `:363`, `infra/test/preflight_test.py:448` and `ops/promotion/test_expiry_renewal.py:129`), after which a release executed from **Dec 1 14:00Z** reaches Jan 15 14:00Z; or (b) two renewal releases, the first to about Jan 10 and a second between Dec 15 and Jan 8 to Jan 15, each costing 18 hours and a review. **Recommendation: (a)**, one release fewer; the ledger already reserves across two months (`preflight_test.py:327`).
- **Tests that pin the values.** `ops/promotion/test_expiry_renewal.py:18` (`RENEWED`), `:22`, `:62`, `:71`, `:112-113`, `:124`, `:141`, `:161-164` (the `2026-12` fixture month); `ops/promotion/test_month_renewal.py:34`, `:55`, `:72`, `:83`, `:90`, `:103`, `:111`, `:155`, `:158`; `infra/test/preflight_test.py:448` (the "32 days" regex). `test_reconcile.py:89` uses `2026-12` as an example and may stay.
- **ADR-019.** The caps stay 21000/24800 cents (§2.6). The amendment's ceilings table gains two rows (December whole month, January 1–15) and the sentence "the December ceiling ($80 / $118 to the stop) ... needs no code change" is superseded. The AWS Budget parameter (`ops/monitoring/alarms.json:44-49`, default 248, "118 for December") **stays 248 through December** and becomes about 160 for January; the alarms stack is standalone and outside the promotion's digest (runbook §6), but A7 applies: change only the amount, never the email, or CloudFormation tries to replace the budget.
- **Operator order before release H, all within 24 hours of the merge** (runbook §2's order, repeated): (1) `reconcile observe/apply` for December (December has begun, so `reconcile` owns it; the live window must still validate); (2) `month_renewal` with a `2027-01` assessment bound to the post-apply `ledger_sha256` (rationale; `prior_extra_spend_cents 0`, `variable_reserve_cents 1500`, `auth_spent_or_reserved_cents 3800`, `unallocated_cents 0`), plan then apply; (3) `expiry_renewal` plan then apply against the hash the month apply printed, expecting `after_expiry 2027-01-16T08:00:00Z`, `next_release_expiry 2027-01-15T14:00:00Z`, caps 21000/24800 and `cost_by_month` for `2026-12` and `2027-01` under them; (4) set `PROMOTION_ENABLED`, merge the release-H pull request, approve the packet only if its stop reads 2027-01-15T14:00:00Z; `--revert` if it does not merge in time. The pins and the `max_expiry` change must be on `main` before step 2 (the commands read the checkout); merging them while promotion is paused costs no release.

### 2.6 What January costs, and whether the caps rise

- The audit of 2026-11-15 measured the beta at about $6.95 a day, **about $210 a month** with nobody signed in, $208 after the A14 cleanup (APP-216 "Why"; POST-CLASS A14–A18). January to the stop is about half that, plus whatever the Jan 1 decision (§4.3, APP-232) keeps running: parked about $15–20 a month, torn down about $0, a production stack its own budget.
- The reservation model (`preflight.py:110-114`): retained core for a whole month 0.0645 × 744 h + 4.30 + 3.60 ≈ **$55.90**; compute 0.1388676 an hour for the service and the broker. December with a Jan 15 stop: 5590 + 10360 (746 h) + 1500 reserve ≈ **17450 cents GenUI, 21250 project** with the $38 fAuth reserve. January 1–15 14:00: 5590 + 4900 (353 h) + 1500 ≈ **11990 GenUI, 15790 project**. Both fit the **21000 / 24800** caps. **The caps do not need to rise**; GenUI headroom in December is about $35. The check is the December `reconcile observe`: if the recorded extra spend (the measured account runs about $180 a month on the GenUI side against the model's $174) pushes December above 21000, raise by the ADR-019 pattern (`reservation_cap`, idle, after a code constant merge, no service release).

## 3. Notice v2, draft for Victor's approval

Based on v1 (`app/src/auth/notice.rs:19-26`). General platform text, no cohort word. The title and checkbox label are unchanged. Victor approves or edits this block before APP-227 ships it as `NOTICE_VERSION = 2`.

> **Before you join this Forma workspace**
>
> Forma (getforma.dev) is in early beta. It may change, break or go offline without notice, and there is no service guarantee or support commitment. You must be 18 or older to take part.
>
> You are joining an organization's workspace. Its owner and admins can see who has joined, your agreement to this notice, your role, when you were last active, and how much you have created. Apps shared with you may collect what you enter. The app's owner and its reviewers can see what you enter; unless the app keeps entries private, so can the other people it is shared with. Anything you build is private unless you share it. Forma's operators can access stored data to run and support the service.
>
> **What we keep:** your email address for sign-in, the name you choose, what you create and enter, the files you upload, and basic activity logs. Data is stored in the United States.
>
> **AI:** Forma includes a small daily allowance of AI use per member, to build apps and answer questions; when it is used up, building and Ask pause until the next day (UTC). Your prompts, and the files and entries you ask the AI to work with, are sent to Anthropic, our AI provider. If you connect your own AI agent through MCP, it runs on your own AI account and at your own cost, and Forma receives only what the agent sends it. Please don't upload sensitive personal, health or payment information.
>
> **After a trial period:** your account, your place in this workspace, your apps and your data continue on Forma's free plan; nothing is deleted when a course or test ends. Paid plans may be offered later, and we will not move you to one without asking. Allowances may change; we announce changes in the app.
>
> **Your data:** you can ask at any time, at privacy@getforma.dev, to delete your account and your data. We confirm within 7 days and complete the deletion within 30 days; backup copies expire about a week later. Deletion removes your sign-in account, your name, your apps, your uploads and what you entered in your own apps. Entries you made in apps that other people own stay with those apps, without your name. A minimal security log of account events (identifiers and times, no content) is kept. Your sign-in account (email and password) is held by our identity provider and is deleted with the rest.
>
> By checking the box you agree to take part on these terms.
>
> ☐ I have read the notice and agree to take part on these terms.

What changed from v1, paragraph by paragraph: "or support commitment" (no SLA); "owner and admins" and "your role" (ADR-021); the AI paragraph gains the allowance and MCP sentences; the new "After a trial period" paragraph is decision 1; "Your data" moves to privacy@, drops the Dec 14 purge and the "copy out" sentence, states the two deadlines and what stays. The "Entries you made in apps that other people own stay ... without your name" sentence assumes §6 Q3's recommendation; if Victor chooses deletion instead, it becomes "are deleted too".

## 4. The work, in three windows

Sizes: XS under an hour, S a day, M two to three days, L a week. "Schema" says whether the item needs a schema release. Releases are lettered on from F (organization roles): **G** is the notice release, **H** the schema release.

### 4.1 Before Nov 28 (must ship with or before the codes)

| # | Item | Ticket | Size | Risk | Schema | Release |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | **Notice v2**: `NOTICE_VERSION = 2`, the text of §3, the module test, the invitation page and `/notice` (both render from the one source), the ADR-008 charter amendment (continuation and opt-out deletion; v2 recorded verbatim beside v1), the class-guide email paragraph rewritten from v2 (the guide says so at §4), runbook §10 and §11.4 retitled from "the purge" to "deletion on request", the readiness table's "by Dec 14" row. | APP-227 | S | Low: text and a version; no storage change. The risk is timing: live **before the Nov 26 rehearsal codes** or those members are pre-v2 too. | No | **G**, by Nov 25 |
| 2 | Victor's written acceptance of v2 for his pre-v2 accounts (operator note in the private approvals file, with release G's record). | APP-227 | XS | None | No | with G |
| 3 | The ops pins and the 45-day bound merged while promotion is paused (`RENEWED_EXPIRY`, `MONTH`, `TRIAL_CEILING_CENTS`, `max_expiry`, their tests, ADR-019 amendment 2). | APP-228 | S | Low: operator-side code, read from the checkout; refusals are the safety. | No | none (a paused merge) |
| 4 | Codes: unchanged (`--expires-days 16`, runbook §4). The entitlement they create is 30 days; §4.2 extends it. | – | – | – | – | – |

Stop after G: **2026-12-11T02:00Z**.

### 4.2 By Dec 12 (class end)

| # | Item | Ticket | Size | Risk | Schema | Release |
| --- | --- | --- | --- | --- | --- | --- |
| 5 | **`extend-entitlement`** (§2.2) with migration 0026 adding `entitlement.extended` and `member.erased` to the operator chain; override entry; `admission.py` front; tests and a campaign. | APP-229 | S–M | Low–medium: a schema release follows the schema-release rules (snapshot, migrate, restore drill if required). The command cannot lower a date. | **0026** | **H**, Dec 2–9 |
| 6 | **`erase-member`** (§2.3) on the same migration; the deletion table walk, proofs and receipt; the fAuth step; the runbook procedure and the reply template. | APP-230 | M–L | Medium: deletion. Mitigated by the purge's pattern (refuse-and-roll-back proofs, counts before and after) and by requiring the membership already revoked. | 0026 | H if reviewed, else the next release; requests are honoured meanwhile by runbook §5 plus the fAuth deletion |
| 7 | **APP-222 part B**: the durable usage ledger, `organization_allowances`, `set-allowance`, usage on `/admin`. | APP-222 | L | Medium: a definer-admitted counter; the campaign (restart, two processes, concurrency) is the proof. | **0027** | H if reviewed (decision by Nov 30), else the next release |
| 8 | **The renewal**: reconcile December, add `2027-01`, renew the expiry, merge H (§2.5 order). | APP-228 | S | Medium: hash-bound steps within 24 hours; every step refuses before writing and has `--revert`. | No | around H |
| 9 | Right after H: `extend-entitlement --all-active --until 2027-02-15T00:00:00Z` for the organization (the stop plus the 30 days the original entitlement had, so a further renewal needs no second extension before mid-February). Revives the Nov 1 account. | APP-229 | XS | Low | – | after H |
| 10 | **Dec 12, class end**: `set-allowance ai_calls_per_member_per_day 10` for the organization (needs item 7 live); otherwise 80 a day stands until part B ships. | APP-231 | XS | Low | – | none |
| 11 | Dec 12: the AWS Budget parameter stays 248 for December (do not lower it to 118 as the alarm template's description says). | APP-228 | XS | Low (A7: amount only) | – | none |

Stop after H: **2027-01-15T14:00Z**. Any further release before Jan 15 subtracts 18 hours again unless the canonical expiry is renewed first.

### 4.3 By Jan 15 (renewal end)

| # | Item | Ticket | Size | Risk | Schema | Release |
| --- | --- | --- | --- | --- | --- | --- |
| 12 | **The production-move decision by Jan 1** (ADR-013 D13.7): park the beta (final snapshots; delete the instances, the load balancer and the WAF; keep the keys; the "class beta has ended" fixed response of APP-216 §3), renew again, or stand up `GenuiProduction` promoted by digest, with its account and budget. Written as an ADR-013 amendment with the January and February figures. | APP-232 | S (decision); the park or production work is its own ticket | Low for the decision | No | none |
| 13 | Jan 1: January budget parameter about 160; the January `reconcile`. | APP-228 | XS | Low | – | none |
| 14 | If the decision is "renew again": the February month, a renewed expiry and a release executed by Jan 14 14:00Z. | APP-232 | S | Medium (same windows as §2.5) | No | one release |
| 15 | Later, with the tiers (A13): consent at sign-in for a changed notice (the name-prompt pattern; a `record_consent` definer); entitlement length as plan data with "no lapse" on the free plan (A19), which removes item 9's recurrence; a lapse warning on `/admin` and to the member. | new tickets with A13 | M each | Low | Yes | later |

## 5. Release order

Keep releases few: each costs 18 hours of stop and its own review.

1. **G: the notice release, by Nov 25.** Notice v2 only (plus A2's "already graded" refusal code if it is reviewed by then; otherwise not). No migration. Stop 2026-12-11T02:00Z. Promotion paused again after it; the Nov 28 restart is not a release.
2. **H: the schema release, Dec 2–9**, in an announced window of the "fixes only in announced windows" period. Migration 0026 (`extend-entitlement`, `erase-member`) and, if reviewed by Nov 30, APP-222 part B as 0027. Preceded within 24 hours by the renewal steps of §2.5. Stop 2027-01-15T14:00Z. Followed by the bulk extension (item 9).
3. **No release Dec 12.** The free-plan switch is an operator task (item 10).
4. **After Jan 1, per APP-232.** Either nothing (park or tear down at the stop), or one renewal release by Jan 14 14:00Z.

Merges that change no runtime path do not promote (`.github/workflows/beta-promote.yml:27,41,56,61`, `runtime_changed`), and nothing promotes while `PROMOTION_ENABLED` is false (`:19`); the docs, tickets and ops pins of this plan therefore cost no release.

## 6. Open policy questions for Victor (each with a recommendation)

- **Q1. Where does the schema release land?** Before the class, inside G (riskier week, no interim for the Nov 1 account), or after it, as H on Dec 2–9. **Recommend H.** The Nov 1 account lapses Dec 1 and is revived by item 9; Victor builds and teaches from the build account, as the follow-ups already say.
- **Q2. One renewal release with a 45-day `max_expiry`, or two releases under the 32-day bound?** **Recommend 45 days** (ADR-019 amendment 2): one review and 18 hours fewer; the ledger already spans two months.
- **Q3. Deletion: what happens to entries a deleted member made in someone else's app, and how fast is deletion honoured?** **Recommend keep the entries with the author's name removed** (the principal row stays as an identifier; the owner's app stays whole; the notice says so) and **confirm within 7 days, complete within 30**, backups a week later. The alternative, deleting the revisions, punches holes in other members' apps.
- **Q4. The Dec 12 allowance switch waits for APP-222 part B?** **Recommend yes**: it is the only release-free, data-not-constant mechanism. If part B misses release H, 80 a day stands under the 1,000-a-day process cap and the $1,000 Anthropic limit until the next release, rather than adding an env parameter that would itself need a release each time it changes.

## 7. What this plan supersedes

- POST-CLASS-FOLLOW-UPS P1 (the Dec 14 purge) is replaced by deletion on request (APP-230); A10 is decided (renew to Jan 15); A17 becomes APP-232's decision by Jan 1; A19 gets its first step (APP-229).
- ADR-008's "End of the test" and notice v1 are amended by APP-227, not here.
- Runbook §10 and §11.4 (the purge) and the class guide's email paragraph are rewritten by APP-227; until then they describe the superseded promise.
