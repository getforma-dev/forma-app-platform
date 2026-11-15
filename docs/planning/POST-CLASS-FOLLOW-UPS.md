# Class follow-ups: what is left, with dates

Written 2026-11-15 after Release C went live on beta. It covers what remains for Victor's class (Nov 29–30, 25 adult students in his organization on `beta.getforma.dev`) and what we come back to after it.
- The plan is [TRIAL-25-READINESS.md](TRIAL-25-READINESS.md).
- The procedures are in the [runbook](../operations/TRIAL-RUNBOOK.md) and the [class guide](../operations/CLASS-GUIDE.md).
- Dates are "no later than" unless marked as a hard deadline.

**The class is remote** (Victor, 2026-11-15). Students, and Victor, join from home, school, a library or anywhere else, not from one classroom network. So:
- every student usually has their own internet address;
- "on my screen" in the guide means screen share;
- "pin it in the room" means send it to the students.

## Where things stand (2026-11-15)

- **Beta runs releases A, B and C.**
  - Release C is candidate `ffba79bd` (tag `beta/20261114-ffba79b`; [record](../../evidence/APP-209/release-2026-11-14.md)).
  - Health is 200 and promotions are paused.
- **The automatic stop is 2026-12-14T20:00Z.** Every release moves it 18 hours earlier.
- **Alarms** go to `victor@getforma.dev`, and their subscription is confirmed. That covers five alarms plus the $248 monthly budget.
- **fAuth**, our own sign-in service (`gatewasm`, `auth-staging.getforma.dev`):
  - trial tenant user limit: 1000 by default;
  - sign-in rate-limit bursts raised to 200 for login, 60 for register and 120 for token;
  - email: SES is out of the sandbox.
- **Anthropic:** a $1,000 monthly organization limit, Max's $200 monthly credit, and the app's own cap of 1,000 model calls a day.

## Before and during the class

| By | What | Who |
| --- | --- | --- |
| **Nov 26** (recommended) | **fAuth per-client rate limits** (item A1 below). Because the class is remote, this protects the class itself: today one misbehaving client or bot can use up the shared sign-in allowance for everyone. | Claude builds; Victor approves the deploy |
| After the new app builder merges | Victor builds the class apps (attendance, homework, quiz). This step is deferred from Nov 24 until the new app builder platform is merged. | Victor |
| Before Nov 26 | Issue the codes for Victor's build account, a helper and the rehearsal students, then one announced restart ([runbook §4](../operations/TRIAL-RUNBOOK.md)). A code works only after the next restart. | Claude, on Victor's word |
| Nov 26 | Rehearsal with 2–3 students on Chrome, Safari and a phone ([guide §2](../operations/CLASS-GUIDE.md)). | Victor |
| Nov 28 | Issue the 25 student codes and do the evening restart. | Claude |
| Nov 28 | Email the codes (the draft is in [guide §4](../operations/CLASS-GUIDE.md)). | Victor |
| Nov 29–30 | Class. Watch the alarm email, `/admin` and the feedback mail. No deploys. | Victor; Claude fixes forward in announced windows |

## After the class

| # | By | What | Why | Cost and how |
| --- | --- | --- | --- | --- |
| P1 | **Dec 14, before 20:00 UTC (hard deadline)** | **Purge the class data**, then **delete the students' fAuth sign-in accounts**. | It is the charter's promise (ADR-008 amendment, "End of the test"). Beta stops itself at 20:00 UTC. | [Runbook §10](../operations/TRIAL-RUNBOOK.md): revoke codes → remove members → suspend → `purge --confirm` → delete the fAuth users (`DELETE /manage/users/{id}`) → delete the manual snapshots. Each release before then moves the stop 18 h earlier, so purge first or renew. |
| A1 | Before inviting anyone beyond this trial, ideally before Nov 26 | **fAuth per-client rate-limit buckets.** Key the limiter on the real client address that Caddy forwards, not on Caddy's peer address. | Behind Caddy every client shares one bucket, so one client can exhaust sign-in for all. | `gatewasm`: swap `PeerIpKeyExtractor` for the forwarded-address extractor. Make sure only Caddy can reach the container, so the header can't be spoofed. Then build, qualify, publish and deploy a new image. A host template update does not re-run the boot script (OPERATIONS.md), so apply it on the host or force a fresh boot. About 2–4 hours plus a short sign-in outage. |
| A2 | With the next release that happens anyway | **A clear "already graded" message.** Today a student who resubmits graded homework sees the generic "The command was refused: a value is not one this command accepts. Nothing was changed." | Once Victor grades an entry it is locked (APP-213, ADR-020 §17.43), and the generic text confuses students. The class guide explains it for now. | The server and its clients (canvas, MCP, Slack) share a fixed, versioned list of refusal codes, the contract edition. A locked entry returns the generic "denied", so the client can't tell it apart. It needs a new code such as 422 `locked`: a new contract edition, server and client updates, and a release. About 2–3 hours. |
| A3 | Bundle with a later release | **Purge code cleanups** from the APP-212 Tier A review. MINOR-1: derive the blocked set from foreign-key edges, so a future migration fails with the named refusal. MINOR-2: the runbook's lock-duration wording. NITs: comments and two untested but identical paths. | Future-proofing only. The Dec 14 purge behaves the same without them, and nothing is ever lost either way, because a refusal rolls back. | `app/runtime/admission_operator.py`, which ships in the operator image, so it reaches beta only with a release. About 1 hour. |
| A4 | Any time; no release needed | **Stop hard-coding the AWS account and database identifiers**: `infra/scripts/common.py` (`ACCOUNT`) and `app/qualification/local_harness.py` (`HOST`, `RESOURCE_ID`). | They are deliberate safety pins: the scripts refuse any other account or database. AWS account ids are not secrets, the database is private with IAM sign-in, and the values are already in git history. This is cleanup, not a fix for a vulnerability. | Move the values to a private config or GitHub variable and keep the "refuse on mismatch" check. About 1 hour plus tests. The scripts are needed for code issuance and releases, so test carefully. |
| A5 | After the class | **Why refresh-herd's third herd runs slow on CI.** | Since 2026-11-14 the CI gate passes on two of three herds ([PR #133](https://github.com/getforma-dev/forma-app-platform/pull/133), Victor's decision), because GitHub runners measured herd 3 at 9.4–9.9 s p95 against 6.9 s for herds 1–2. | Measure locally. Check the catalog re-verification cadence (APP-197: every 30 s per connection) against the herd timing. |
| A6 | After the class | **Make fAuth host template changes apply on update.** | A UserData-only update stops and starts the same instance, and the boot script doesn't run again. The 2026-11-15 rate-limit change needed a manual host edit. | Use `cfn-init`/`cfn-hup` or an SSM-managed config, or make UserData changes force replacement. |
| A7 | After the class | **The alarm template's fixed budget name.** Changing `NotificationEmail` makes CloudFormation try to replace the named budget, and AWS refuses. | The 2026-11-15 address change needed a delete and recreate. | Let CloudFormation generate the budget name, or document delete-and-recreate (runbook §6/§11.1 already do). |
| A8 | After the class | **Ticket chain re-scope.** APP-194, APP-209, APP-212 and the trial tickets can't reach Done, because their predecessor chain runs through APP-181 → APP-139, which isn't Done. | The board's rule refuses Done without Done predecessors, so trial work stays in Backlog with logged evidence. | A planning decision: re-scope APP-181/APP-139 or record the trial's completion. Use `Tickets/_tools/board.py`. |
| A9 | Optional | **The 26-page refresh-herd release measurement** on a quiet host (`evidence/APP-197/release-measurement-recipe.md`). | Cut on 2026-11-14 as fail-forward. The CI gate and the 25-way race are the class's evidence. | About 30 min locally. |
| A10 | Decide by Dec 1 | **Keep beta after Dec 14?** The stop is 2026-12-14T20:00Z. | Continuing needs a renewal plus a release, and a production environment (ADR-013) is planned after this test. | `expiry_renewal` takes effect only through a later release (runbook §2). |
| A11 | Optional | **fAuth `return_entry_url`** for the trial clients, so the post-verification page shows a "Continue to app" link. | Cosmetic. Without it students see "Return to the app and sign in". | One idempotent `POST /manage/oauth-clients` per client ([runbook §3](../operations/TRIAL-RUNBOOK.md)). |
| A12 | After the class | **"Sign out everywhere" also revokes the member's live guest shares.** | Since APP-215 a sponsor's sign-out keeps their guest shares alive (a fix for links dying on sign-out), so a stolen session's guest shares survive "sign out everywhere" (review of PR #137, MAJOR 2). | In `sign_out_all`, revoke the member's live shares in the same transaction, or show their active shares with a one-click revoke on the sign-out-everywhere page. Add a campaign check. About 1–2 h plus a release. |
