# APP-082 independent review

Date: 2026-11-05

Reviewed SHA: `a9a730531dc6644d2fa6ef291a967080886f596a`

Independent reviewer, reading the checkout at `/Users/victorvillacis/dev/FormaBuiltApps/forma-app-082` (branch `chore/app-082-ci-binding`). `git rev-parse HEAD` is that SHA, and it is `origin/main`. This is not the implementer. No other file was edited. The ticket was not checked. Nothing was committed or pushed. AWS was not called. The process on `127.0.0.1:4001` was not touched. The campaign was not re-run. The binding is the GitHub Actions receipt plus source reading, not a second local execution.

## Method

Compared the ticket's open acceptance item 3 with the workflow step on this SHA, the GitHub Actions run, the uploaded `invite-races.json`, and `auth::invite_tests::native_invite_race_campaign`. Each of the twelve `notes.record` strings was matched to the assertions immediately above it, then to the production route, SQL, or operator function those assertions call. The earlier local `race-campaign.json` was compared only as corroboration. `hosted_acceptance` and `managed_acceptance` were left false.

## Verdict

The receipts are bound for acceptance item 3. The tenant-foundation job ran this exact source, the step command is the campaign, the artifact passed with the same twelve checks the source asserts, and those checks are the route, lock, provider, and database outcomes required by acceptance items 1 and 2.

No defect requires a code change. An APP-074 or APP-076 rerun is not required. Item 3 can be checked. This review does not check it and does not mark the ticket Done.

This run is not hosted acceptance and not managed or RDS acceptance. Both flags in the report are false. The database is the workflow's local PostgreSQL 16.15 Docker fixture.

## What I compared

### Run and artifact

GitHub Actions run `41955377535` (`Tenant foundation qualification`), confirmed through the Actions API:

- Event: push to `main`
- Head SHA: `a9a730531dc6644d2fa6ef291a967080886f596a`
- Display title: Merge pull request #77 from getforma-dev/chore/app-163-fauth-gate
- Job `tenant-foundation`, id `127008916230`, conclusion success
- Step 16, "Qualify route-level invitation races and provider faults (APP-082)", success, `2026-11-05T02:45:29Z` to `2026-11-05T02:46:41Z`

The job log shows the step command, then twelve `APP082 check:` lines, then `APP082 invite race campaign: 12 checks passed`, then:

```text
test auth::invite_tests::native_invite_race_campaign ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 430 filtered out; finished in 71.72s
```

The step command in that log is the command in `.github/workflows/tenant-foundation.yml` lines 66–67:

```text
GENUI_INVITE_RACE_REPORT="$RUNNER_TEMP/tenant-foundation/invite-races.json" cargo test --locked -p forma-app-platform --lib auth::invite_tests::native_invite_race_campaign -- --ignored --exact --nocapture
```

The artifact `app-008-tenant-foundation-evidence` was downloaded again for this review. Its `invite-races.json` sha256 is `8574a23e396075e6a4bc95d654c8bccd3018f6e04089799452fd279f480446d4`, identical to `evidence/APP-082/ci-run-41955377535/invite-races.json`. That matches `evidence/APP-082/README.md`. The report has `passed: true`, schema version 1, twelve checks, every check `passed: true`, `hosted_acceptance: false`, `managed_acceptance: false`, and `server_version` `16.15 (Debian 16.15-1.pgdg13+2)`. Dispatch is `par` 28, `register` 9, `exchange` 26, `provider_creates` 13. The twelve names are byte-for-byte the twelve `notes.record` literals in `app/src/auth/invite_tests.rs` (lines 2382, 2433, 2484, 2562, 2639, 2701, 2754, 2822, 2904, 2994, 3042, and 3139).

`evidence/APP-082/race-campaign.json` (sha256 `0dd52616abae028c016ab2673e21554113021458f69d6326d3cdee4e19bbfe07`) has the same check names, pass flags, dispatch counts, limits, scope, and schema version. Its server version is `16.13 (Debian 16.13-1.pgdg13+1)`. It is the earlier local run already used for items 1 and 2. It is not the exact-source receipt.

`passed: true` on each check is written by `Notes::record` (`invite_tests.rs` 2169–2174) only after the assertions above it. The report's top-level `passed` is then written (`invite_tests.rs` 3141–3170). A failed assertion panics the ignored test, so the step would not be success and the log would not print that check. The log printed all twelve names before `test ... ok`.

`server_version` is `SHOW server_version` on the fixture (`app/src/storage/qualification.rs` 280–287), which also requires the version to start with `16.15`. The image pin is `postgres@sha256:5db6dac2ef5dcff512f4f00a009c3c332b9f2f28f24277175bf3e4a3d02ef83a` (same file, lines 24–25). The 16.13 string in the ticket's local-harness sentence is the earlier run, not this CI database.

### How a lock wait is observed

`lock_wait` (`app/src/storage/admission_qualification.rs` 168–181) polls `pg_stat_activity` for `datname='genui' AND wait_event_type='Lock'` and returns only when the count is at least the requested number, or fails after 10 seconds. It does not select the waiting query or relation. That is the admission campaign's barrier this ticket says to reuse. It still identifies the waiter: the holder is paused inside an open transaction after it has taken the conflicting row lock, exactly one competing request is spawned, and `lock_wait` must then see a Lock wait. The holder's session is idle in the transaction (a client wait, not a Lock wait). The production statement the competitor runs is the `FOR UPDATE` that conflicts with that hold, cited per check below. Checks that do not say "observed lock wait" do not call `lock_wait`.

### The twelve checks

Routes under test are `POST /invite/validate` (`app/src/auth/routes.rs` 33, handler `app/src/auth/invite.rs` 418) and `GET /auth/callback` (`routes.rs` 39, `callback` at 463, `arrive` at 345). A denied arrival is HTTP 403 with the denied page (`routes.rs` 540–558). `denied_page` (`invite_tests.rs` 2005–2006) requires that status and `page=denied`. `neutral_refusal` (1996–1997) requires HTTP 400 and `page=invite;failed=true;limited=false`.

1. Concurrent last slot, lines 2310–2382. The signup callback is `GET /auth/callback`. `mint_pre_access` calls `genui_private.record_pending_admission` and then, in test builds, stops at the `pending` checkpoint before commit (`app/src/storage/sessions.rs` 594–601). That function locks the code row `FOR UPDATE` (`app/migrations/0003_admission.sql` 277–279) and holds it until the transaction ends. The test sees no live pending row while stopped, then spawns `POST /invite/validate`. `handoff` (`invite.rs` 345–387) reaches `begin_flow`, which calls `reserve_code` (`sessions.rs` 506–512). `reserve_code` locks the same code row `FOR UPDATE` (`0003_admission.sql` 237–238). `lock_wait` sees that waiter. After release, the callback is `/auth/pending`, the validate response is the neutral refusal, the PAR counter and `POST /manage/invites` count are unchanged, and one live pending row exists with `redeemed_count` 0. The later verified sign-in is `/workspace` with one principal, one active entitlement, `redeemed_count` 1, status `exhausted`, and one `admission.redeemed`. A later validate stays the neutral refusal and does not create again. `reserve_code` refuses once a live pending admission consumes the only redemption (`0003_admission.sql` 244–248), which is why the waiter does not start PAR (`invite.rs` 388, reached only after `begin_flow` returns a flow).

2. Identical retries, lines 2384–2433. Two prepared verified sign-ins of one pending identity. The first callback stops at the `admit` checkpoint after `admit_redemption` returns and before commit (`app/src/storage/admission.rs` 349–354). `admit_redemption` holds the code row `FOR UPDATE` (`0003_admission.sql` 347–349). The second callback waits; `lock_wait` sees it; `redeemed_count` is still 0 while the first is stopped. Both callbacks then land on `/workspace` with different session cookies, one redemption, one principal, one entitlement row, one `admission.redeemed`, and two session rows. Each session then opens `/workspace`. The second call is the consumed-pending replay (`0003_admission.sql` 355–363), not a second slot.

3. Expired entitlement, lines 2435–2484. This check does not claim a lock wait. A later sign-in stops at the `mint` checkpoint, which is after `fence` and before the entitlement re-read (`sessions.rs` 781–790). `fence` calls `genui_private.lock_member_authority` (`sessions.rs` 408–418), which takes the membership `FOR SHARE` (`app/migrations/0002_tenant_foundation.sql` 241–255). While stopped, a superuser update sets that entitlement's `expires_at` to one millisecond after `starts_at`. The released callback is the denied page with no live session cookie. Active entitlements are 0, entitlement rows stay 1, redemption and principal stay 1. Both sessions minted in check 2 then get the uniform 404 from `GET /workspace`. A later sign-in is denied and adds no entitlement row.

4. Entitlement revocation racing a sign-in, lines 2486–2562. The sign-in stops at the same `mint` checkpoint, holding the member fence. `revoke-entitlement` locks that membership `FOR UPDATE` (`app/runtime/admission_operator.py` 989–1002) and so waits; `lock_wait` sees it. After release the callback is `/workspace` and the operator result is `changed: true`. Active entitlements are 0, the entitlement row count stays 1, and `redeemed_count` stays 1. The new session and the earlier session from check 1 are the uniform 404. A later sign-in is the denied page and the active-entitlement count stays 0. The denial path does not insert an entitlement: with no live pending admission, `complete_admission` only reads an active entitlement and otherwise returns `Denied` (`sessions.rs` 754–763, `routes.rs` 441–454).

5. Admission first, lines 2564–2639. The verified sign-in stops at the `admit` checkpoint, holding the code row. Operator `revoke` locks that code `FOR UPDATE` (`admission_operator.py` 912–923) and waits; `lock_wait` sees it. The callback is `/workspace`, revoke reports `changed: true`, and the code is then `revoked` with `redeemed_count` 1, one principal, and one active entitlement. That session still opens `/workspace`. A later sign-in is `/workspace` with the same redemption count and still one `admission.redeemed`.

6. Revocation first, lines 2641–2701. A fixture transaction holds `FOR SHARE` on the organization and `UPDATE`s the code to `revoked` without committing (`invite_tests.rs` 2656–2657). The verified callback waits; `lock_wait` sees it; principals are still 0. After commit the callback is the denied page with no live session. Principals, active entitlements, and entitlement rows are 0, `redeemed_count` is 0, the code is `revoked`, one `denied_for_reconciliation` reservation exists, and one `admission.denied_for_reconciliation` audit exists. A later sign-in stays denied with no principal. `admit_redemption` writes that state when the code is revoked (`0003_admission.sql` 375–379). The name's "no membership" is not a separate count. `genui.memberships` references `genui.principals` (`0002_tenant_foundation.sql` 38–41), and the principal count for that subject is 0, so no membership row for that subject can exist.

7. Create dropped before a response, lines 2703–2754. `fake_fauth.py` `drop_before` closes the socket before the invite is written (lines 222–223). `Client.create` sends one POST and maps a sent request with no status to `unknown` / `transport_ambiguous` (`app/runtime/provider_management.py` 378–385). `request` marks the exchange sent once connect succeeded and any later failure returns no status (264–306). The issue result is that state, with no capability and no remote id, and the POST count increased by one. Reconcile inside the grace (`reconcile_grace_seconds` is 120, same file line 80) leaves the row `unknown` and reports `deferred >= 1` (`admission_operator.py` 1744–1747 and 1971). After the row and the stand-in clock are moved 900 seconds, reconcile completes and the create is `failed`, with no further POST. Public validate is the neutral refusal, the PAR count is unchanged, and no principal exists. `judge_creates` records `failed` / `not_found` from the list and does not call create (1717–1787).

8. Create timeout, lines 2756–2822. The stand-in sleeps 13 seconds after the write (`fake_fauth.py` 229–230). The client deadline is `request_deadline_seconds` 10 (`provider_management.py` 75 and 266). The issue returned in the test's 8-to-18-second window, as `unknown` / `transport_ambiguous`, with no capability, and the POST count increased by one from the previous check. A client that had accepted the late 201 would have been `confirmed`, not `unknown`. Reconcile inside the grace leaves the row `unknown`. After aging, reconcile completes with create `confirmed` and revoke `confirmed`, the POST count unchanged, and the stand-in status `revoked`. Public validate is the neutral refusal and mints no principal. A confirmed create with no capability is revoked by `judge_creates` (1804–1810), which is the lost-token path the name states.

9. Revoke timeout, lines 2824–2904. After a real pending signup, revoke uses `delay_after` of 13 seconds. The command returns in the same 8-to-18-second window with `changed: true` and provider state `unknown`. The code is `revoked`, the create POST count is unchanged, and the capability file is gone before the remote answer. The verified sign-in is the denied page with no principal and no active entitlement. Reconcile then shows create `confirmed` and revoke `confirmed` with no additional create, and a second sign-in stays denied. `process_revoke` sends again for a revoke row in `pending`, `unknown`, or `failed` (`admission_operator.py` 1666–1694). Reconcile step 6 selects those rows and calls it (1993–2003).

10. Signup after local expiry, and the outstanding callback after exhaustion, lines 2906–2994. Expiry is a committed `expires_at` update. The verified sign-in is denied, with no principal, no active entitlement, one `denied_for_reconciliation` reservation, and one matching audit. That write is `admit_redemption` when the pending admission's code is expired (`0003_admission.sql` 380–382). The exhaustion half prepares `POST /invite/validate` and hosted registration while `redeemed_count` is 0, then `consume_last_slot` (`admission_qualification.rs` 281–323) takes the slot through `reserve_code`, `record_pending_admission`, and `admit`. The original `GET /auth/callback` is then denied, with no live session, no principal, no active entitlement, and no membership row for that subject. `denied_for_reconciliation` count and audit are 0. `admission.redeemed` is 1, the slot-taker has one principal, and `redeemed_count` stays 1. `record_pending_admission` returns `exhausted` before any pending insert or denial audit (`0003_admission.sql` 289–292). The denial audit is written by `admit_redemption` only when a pending row already exists (385–389). The check states that split; it does not claim an audit the function does not write. The limit text in the report says the same: one server-only code cannot host two subjects, so the competing admission is the fixture and the refused request is the already prepared callback.

11. Remote revocation, lines 2996–3042. The stand-in invitation is set to `revoked`. Reconcile completes with create `confirmed` and revoke `confirmed`, the capability file is gone, the code stays `active`, and the create POST count is unchanged. `ended_remotely` records that revoke and removes the capability; it does not update the code (`admission_operator.py` 1813–1855). Public validate is the neutral refusal, the PAR count is unchanged, and no principal is minted.

12. Restart and reopen, lines 3044–3139. A new `Auth::loopback` and router are built on the same stores and the same capability directory (`invite_tests.rs` 3066–3070). The old session is the uniform 404. The old pre-login callback is HTTP 400 on the sign-in page and does not add a principal. The old resend token is HTTP 403. A fresh pending page's resend redirects to `/signin`. The current member signs in to `/workspace` on the new router. The member revoked in check 4 stays denied with no active entitlement. An unspent code still gets HTTP 303 from `POST /invite/validate`. The recorded name says this is auth-memory and CSRF replacement in the same process, not an operating-system or pool restart. `resolve_session` already treats a vault that no longer holds the refresh reference as a restart (`sessions.rs` 835–840). The check does not claim more than it ran.

The report limits (`invite_tests.rs` 3155–3159) match the artifact: synthetic loopback issuer plus `fake_fauth.py`, no deployed fAuth, mail, browser, or AWS call; races stop on the test checkpoints or a `pg_stat_activity` lock wait; the provider deadline is the create/revoke timeout; the exhaustion and restart limits are the ones above; raw codes and tokens are not in the report. The rendered report is checked for those secrets before it is written (3163–3167). The downloaded JSON contains none.

## Findings

None. No hole in the proof and no wrong outcome. Nothing here requires a production or campaign code change.

In particular, these are not defects:

- `lock_wait` counts Lock waits instead of printing the waiting query. The single in-flight competitor and the conflicting `FOR UPDATE` identify the waiter, and that is the barrier this ticket reuses.
- Check 6 does not run a membership `count(*)`. The principal count is 0 and memberships cannot exist without that principal.
- Check 10's missing `denied_for_reconciliation` row is the `record_pending_admission` early return, and the check says so.
- The restart is in-process. The check and the report limits say so.
- The create and revoke elapsed window is 8 to 18 seconds around the 10-second client deadline and a 13-second stand-in delay. The stored state is `unknown` / `transport_ambiguous`, which a received 201 would not be.
- CI is PostgreSQL 16.15. The local 16.13 report is a different run. Neither report claims hosted or RDS acceptance.

## Item 3

Item 3 can be checked. The campaign ran in the tenant-foundation workflow on `a9a730531dc6644d2fa6ef291a967080886f596a`, and this review binds the receipts. No defect was found, so no fix and no rerun of the affected APP-074 or APP-076 cases is required before the next invitation beyond the demo principals. The ticket stays Backlog until someone checks the item. This file does not do that.
