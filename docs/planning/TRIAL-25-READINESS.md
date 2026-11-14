# Trial readiness: 25 real people on the beta

Status: **draft for Victor's decisions** (2026-11-14). Nothing here is approved or started.

Victor wants 25 real people to sign up and try real things, each in a clean workspace. This plan says where the beta stands against that and what is left, in order. Facts come from two read-only research passes over `main` at `bf6a587`, plus a read-only check of the SES account on 2026-11-14. Where this file and an older document disagree, the newer verified fact is marked.

## Where we stand

**Not ready yet.** Three things block, regardless of anything else:

1. **Nobody can be invited through a reviewed path.** The documented member invitation (`docs/pitch/CLEAN-TRIAL-DEPLOY.md:99-101`) runs `ops/beta/run-step.sh`. That now refuses, because deploy state moved to the promotion bucket (`ops/beta/lib.sh:47-51`), and `ops/promotion` has no admission command. Each invitation's capability is also read only at service start (`app/runtime/runtime.py:182`), so every invite needs a restart, and a restart signs everyone out.
2. **The beta charter says synthetic data only.** ADR-008:7-8 says "synthetic data, labeled beta, no SLA", and APP-079/APP-080 "block any real organization". The stack is tagged `Environment=synthetic` (`infra/src/stack.ts:176`). A real-user trial needs an explicit, recorded decision with its accepted risks.
3. **The deployment is stuck, and it stops soon.**
   - Live is `05db000` (2026-11-05, schema 20), with 158 commits on `main` since.
   - Release run 43157872671 (schema 20→22) waits for approval, but its budget basis is stale. The ledger must be reconciled within 24 hours (`infra/scripts/preflight.py:429`), so an approval now refuses after taking the lock.
   - Its scan disposition expires 2026-11-20.
   - The beta scales to zero at **2026-11-25 08:00 UTC** (`stack.ts:439-446`), and promotions stop about two days earlier.
   - December has no ledger month, and November has about $6 of headroom under the $180 ceiling (ADR-019:11).

What already works:
- **Joining, as an invited member:** the hosted owner journey passed on 2026-11-01. A new member gets an empty canvas, a short guide, templates, and their own first app through the composer (APP-139 boxes 1–3, APP-145).
- **Email:** the SES account is out of the sandbox (verified live 2026-11-14: production access on, 50,000 a day, domain `getforma.dev` verified with DKIM, account suppression for bounces and complaints). Older repo text saying "sandbox" (`CLEAN-TRIAL-DEPLOY.md:62`, `Tickets/BLOCKERS.md:12`) is stale. Only fAuth sends mail today, and only verification mail. Whether fAuth's sender is this SES account is unverified; the dress rehearsal shows it.
- **Separation inside one organization:** each member's apps are private unless shared. That rests on per-work grants, which the campaigns cover.
- **The clean-trial deployment:** one organization (`forma-trial`), demo mode off, no seeded data, no persona names.

## The checklist

Legend: **V** = Victor's action or decision, **C** = code, **O** = operations or configuration. "Blocks" means the trial cannot start without it.

| # | Item | Who | Blocks? | Notes |
| --- | --- | --- | --- | --- |
| 1 | Record the trial charter: an ADR-008 amendment naming the cohort, invite-only access, the beta label, no SLA, data terms and the accepted risks | V + C | yes | Decision D1 |
| 2 | Choose the workspace model | V | yes | Decision D2 |
| 3 | Set the trial dates, and renew the beta expiry and the December ledger month to cover them | V + O | yes | Decision D3; renewal is a reviewed change |
| 4 | Unstick the release: abandon 43157872671, wait the 65-minute fence, reconcile, run a fresh promotion of `main`, approve it within 24 hours | V + O | yes | `docs/pitch/BETA-PROMOTION.md:93-178`; before 2026-11-20 |
| 5 | APP-175 box 3: read back the execute-role permission update | V | yes, for #4 | `Tickets/0-backlog/APP-175.md:44` |
| 6 | A reviewed post-cutover admission command: issue, list and revoke member codes for the trial organization | C + O | yes | Replaces the fenced `run-step.sh` path |
| 7 | Invitations without one restart each: load new capabilities without a restart, or batch every invite into one scheduled restart | C or O | yes | `runtime.py:182` |
| 8 | Raise the fAuth trial tenant's user limit from 10 to at least 30 | V (FormaStack) | yes | `CLEAN-TRIAL-DEPLOY.md:53` |
| 9 | A beta notice and consent at sign-up: what is stored, that Anthropic processes prompts and file excerpts, that data may be deleted at trial end, how to ask for deletion | C + V (text) | yes | No privacy or terms page exists; Decision D6 |
| 10 | A model spend guard: a spend limit on the Anthropic account, plus an app-wide daily cap that tells the member plainly when it is reached | V + C | yes | Decision D4; only per-member hourly windows exist (`composer/routes.rs:60`) |
| 11 | Delete a member's data on request: a reviewed operator procedure, tested on the dev stand | C + O | yes | No delete route exists (APP-064/065 Backlog) |
| 12 | Dress rehearsal: Victor with a fresh address (a `+trial` alias, since his own is already a member), one other browser (Safari), the whole journey, including the verification email arriving | V + C | yes | `APP-140.md:56` |
| 13 | Alarms: 5xx rate, unhealthy target, task count 0, and an AWS Budget alarm, all to Victor's email; an SES event destination for bounces and complaints | O (reviewed stack change) | strongly | None exist (`stack.ts`; `infra/README.md:26`) |
| 14 | A one-page incident and support runbook: who looks, how to roll back, how a member reports a problem, the deny-all switch | C | strongly | Decision D7 (feedback channel) |
| 15 | A read-only admin view: members, invite and redemption status, last active, apps and records per member, model calls today, recent errors | C | no | Decision D5; a new operator authority, so Tier A review |
| 16 | One restore drill to a scratch instance from the latest snapshot | O | recommended | APP-068; never done |
| 17 | Managed-database isolation verdict (APP-078) | O + V (window) | recommended | Matters most if a second real organization is added |
| 18 | Invite in waves: 5 people, watch for 48 hours, then the other 20 | V | — | |
| 19 | No deploys while people are using it, outside an announced window: every deploy is a ~4-minute outage that signs everyone out | V | — | `stack.ts:412-418` |

Not needed for this trial: APP-163 and APP-164 (workspace discovery and switching), APP-165 (app-sent invitation email), unless Decision D5b picks it. Also not needed: the guest-share boxes of APP-139, unless members share outside the cohort.

## Phases (execution order, after approval)

Each step is sized; anything L is split. "Blocked by" names step ids.

### Phase A: decisions and charter
Verified when the ADR-008 amendment is merged and the board has a ticket per step below.
- **A1** Write the trial charter amendment from the decisions (S). Blocked by: Victor's answers.
- **A2** Create one ticket per step in this plan, with the board regenerated and checked (S). Blocked by: A1.

### Phase B: unstick and extend the beta (Victor runs, I prepare)
Verified when a fresh promotion of `main` is active, `/health/ready` passes, and the expiry covers the trial dates.
- **B1** Prepare the exact abandon, reconcile and fresh-run sequence as a checklist, with each command's expected output from the runbook (S, C). Blocked by: A2.
- **B2** Victor: the APP-175 permission readback, then the sequence (V). Blocked by: B1.
- **B3** A reviewed expiry renewal and December ledger month for the trial window (M, C + V). Blocked by: B2 and D3.

### Phase C: let people in
Verified when a fresh address completes code → register → verify email → sign in → first app, with no operator restart per person.
- **C1** A post-cutover admission command in `ops/promotion` (issue, list, revoke member codes for the trial organization), with tests and a Tier A review (M). Blocked by: A2.
- **C2** Invitation capability without a restart, or a documented batch procedure, decided by a short spike reading `runtime.py` and the broker (S spike, then S or M). Blocked by: A2.
- **C3** Victor: raise the fAuth tenant limit (V). Blocked by: none.
- **C4** Dress rehearsal (item 12) and the record of its result (S). Blocked by: B2, C1, C2, C3, D1.

### Phase D: real-user obligations
Verified when a new member sees and accepts the beta notice before first use, the daily model cap refuses with a plain message in a test, and the erase procedure removes one test member's data on the dev stand.
- **D1** A beta notice page and consent at code redemption (text from D6), with an SSR test (M). Blocked by: A2.
- **D2** An app-wide daily model-call cap from configuration, with a plain refusal message, unit and campaign tests; plus Victor's Anthropic spend limit (S + V). Blocked by: A2.
- **D3** The member data-erase procedure (operator command or reviewed SQL) for one member's apps, records, uploads and membership, tested on the dev stand (M). Blocked by: A2.

### Phase E: running it
Verified when a forced unhealthy state on the dev stand, or a test alarm, emails Victor, and the runbook names every step.
- **E1** Alarms, a budget alarm and the SES event destination as a reviewed stack change (M, C + V applies). Blocked by: B2.
- **E2** The incident and support runbook (S). Blocked by: C1, D3.
- **E3** The restore drill, if D8 says yes (S, O + V). Blocked by: B2.

### Phase F: admin view (if D5 says yes)
Verified when Victor, signed in as the operator principal, sees the trial's members and their status, and a non-operator gets the uniform 404.
- **F1** A read-only operator route and page, members and invite status only (M, Tier A review). Blocked by: C1.
- **F2** Activity columns: last active, apps and records, model calls today (S). Blocked by: F1.

### Phase G: invite in waves
- **G1** 5 people; watch errors, model spend and support mail for 48 hours (V). Blocked by: C4, D1–D3, E1, E2.
- **G2** The other 20 (V). Blocked by: G1.

## Decision log (to fill from Victor's answers)

| Decision | Chosen option | Why |
| --- | --- | --- |
| D1 Real-user charter | — | — |
| D2 Workspace model | — | — |
| D3 Trial dates and length | — | — |
| D4 Budgets (AWS, model) | — | — |
| D5 Admin view; app-sent invites | — | — |
| D6 Notice and terms text | — | — |
| D7 Feedback channel | — | — |
| D8 Restore drill and isolation verdict before the trial | — | — |

## Not yet specified
- **Phase C2:** whether the service can load an invitation capability without a restart. A spike answers it; until then the fallback is one batched restart per invite wave.
- **Phase F:** where the operator principal comes from (an operator-only membership, or a configured principal id). Decided at F1 pickup, with the Tier A reviewer.
- **One organization per person** (if D2 picks it): fog. It needs per-organization fAuth tenants, stack and broker changes, and budget well beyond today's ceiling, and would be planned separately.

## Always / Never
- Always: invite-only; the beta label; Victor approves every deployment; real data stays out of Git; every change gets an independent review; Tier A review for admission, operator authority and deletion.
- Never: self-serve public sign-up; seeded demo data or persona names in the trial organization; deploys during use outside an announced window; turning demo mode on in the trial; sending email from the app without a decision (APP-165).

## Progress

| Phase | Status |
| --- | --- |
| A Decisions and charter | waiting on Victor |
| B Unstick and extend | not started |
| C Let people in | not started |
| D Real-user obligations | not started |
| E Running it | not started |
| F Admin view | not started |
| G Waves | not started |
