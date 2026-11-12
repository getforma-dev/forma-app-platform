# APP-082 independent review, re-bound to current main

Reviewer: Claude subagent, independent of the implementer Grok. I did not write the campaign, the binding commit `2850d59`, or the earlier review.

Date: 2026-11-12 (UTC).

Source reviewed: `5695869747b4f66a25617662deec002fba751e38` (`main`, "Merge pull request #84 from getforma-dev/feat/app-169-artifact-kind"). Checkout `/Users/victorvillacis/dev/FormaBuiltApps/forma-board`, `git rev-parse HEAD` = that SHA. Ticket: [APP-082](../../Tickets/4-done/APP-082.md).

Scope: this file is the only file I wrote. I did not edit the ticket, commit, push, call AWS, or touch the running dev pitch instance. GitHub was used only through `gh` read commands and an artifact download into a scratch directory.

## Why a re-binding is needed

The receipt in the ticket and [README.md](README.md) is run `41955377535` on `a9a730531dc6644d2fa6ef291a967080886f596a`. Since then, 63 commits landed on `main`. APP-084 changed two files the campaign drives: `app/runtime/admission_operator.py` (+112/−52) and `app/runtime/provider_management.py` (+4/−2). APP-084 also added migrations `0021` and `0022`. [REVIEW.md](../../Tickets/_tools/REVIEW.md) says a receipt on different source does not transfer by default. So item 3 has to be bound to the current source.

Between `a9a7305` and `5695869`, `git diff` shows that these files are byte-identical: `app/src/storage/sessions.rs`, `app/src/storage/admission.rs`, `app/src/auth/routes.rs`, `app/src/auth/invite.rs`, `app/migrations/0002_tenant_foundation.sql`, `app/migrations/0003_admission.sql`, `app/runtime/fake_fauth.py` and `app/src/storage/qualification.rs`. In `app/src/auth/invite_tests.rs`, only line 959 changed, and that line is in the APP-078 `campaign()`, not `race_campaign()`. All line numbers in the race campaign are the same as before. Migrations 0021 and 0022 change only the operator/workspace audit-kind policies, add `provider_invitations.detail`, and add the `provider_tenant_index` and `work_artifact_kinds` tables. They do not touch `reserve_code`, `record_pending_admission` or `admit_redemption`.

The APP-084 changes that the campaign actually exercises are these:
- Each committed provider-invitation state change now appends one operator-chain event (`note_invitation` → `append`, which takes `audit_heads` FOR UPDATE). See `admission_operator.py` 686–694, called from `create_remote` 1664 and 1695, `process_revoke` 1732 and 1745, `judge_creates` 1849, and `ended_remotely` 1909.
- The `detail` column is now written.
- Every issue calls `claim_provider_tenants`.
- `PROVIDER_HOSTS` is an allowlist for the HTTPS provider host (`provider_management.py` 110–113 and 244). Loopback HTTP is unchanged.

None of these changes the outcome logic the checks assert. The new run below is what proves they did not break it.

## Exact-source receipt

`gh run view 43144269940 --json headSha,conclusion,jobs`:

| Field | Value |
| --- | --- |
| Workflow | Tenant foundation qualification (`.github/workflows/tenant-foundation.yml`) |
| Event / branch | push / `main` |
| headSha | `5695869747b4f66a25617662deec002fba751e38` |
| Run conclusion | success; created `2026-11-12T02:51:36Z`, updated `2026-11-12T03:15:58Z` |
| Job | `tenant-foundation`, id `130938339563`, success, `02:51:48Z` → `03:15:58Z` |
| Step 16 | "Qualify route-level invitation races and provider faults (APP-082)", success, `2026-11-12T03:09:37Z` → `2026-11-12T03:10:51Z` |
| Same job, related steps | 11 APP-073 admission operations: success. 15 APP-074 invite redemption: success. |
| Artifact | `app-008-tenant-foundation-evidence`, id `13911568445`, created `03:15:55Z` |

Every other workflow on `5695869` also concluded success: Planning integrity, Contract qualification, Bounded query, Released baseline, Parser, Parser release, Published canvas consumer, Auth consumer, Synthetic fixture, Pitch campaigns and Foundation preparation. "Promote to beta" was skipped.

Step 16 log (`gh run view 43144269940 --log --job 130938339563`, filtered to the step):

```text
##[group]Run GENUI_INVITE_RACE_REPORT="$RUNNER_TEMP/tenant-foundation/invite-races.json" cargo test --locked -p forma-app-platform --lib auth::invite_tests::native_invite_race_campaign -- --ignored --exact --nocapture
running 1 test
APP082 check: … (12 lines, 03:09:52Z → 03:10:51Z)
APP082 invite race campaign: 12 checks passed
test auth::invite_tests::native_invite_race_campaign ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 438 filtered out; finished in 73.78s
```

The step command is the one in `tenant-foundation.yml` lines 66–67. It is also `scripts/dev/campaigns.tsv` line 18 (`invite-races`). Exactly one test ran (`--exact`, 1 passed).

Report comparison. I downloaded the artifact into a scratch directory. Its `invite-races.json` has sha256 `8574a23e396075e6a4bc95d654c8bccd3018f6e04089799452fd279f480446d4`, byte-identical to [ci-run-41955377535/invite-races.json](ci-run-41955377535/invite-races.json). The report contains:
- `passed: true` and `schema_version` 1
- 12 checks, every check `passed: true`
- `hosted_acceptance: false` and `managed_acceptance: false`
- `server_version` `16.15 (Debian 16.15-1.pgdg13+2)`
- dispatch `par` 28, `register` 9, `exchange` 26, `provider_creates` 13

The 12 log lines equal the 12 report names, in order. [race-campaign.json](race-campaign.json) has the same 12 names and differs only in `server_version` (16.13, an earlier local fixture). The report carries no SHA, run id or timestamp, so identical bytes across two source revisions are expected. The report binds to source only through the run metadata above. It does not bind by itself.

## Independent local run

I ran this from the worktree at `5695869` with `export PWD=$(pwd)`:

```text
GENUI_INVITE_RACE_REPORT=<scratch>/local-invite-races.json cargo test --locked -p forma-app-platform --lib auth::invite_tests::native_invite_race_campaign -- --ignored --exact --nocapture
```

The run started `2026-11-12T03:30:59Z` and ended `03:34:47Z`, on a disposable Docker PostgreSQL 16.15 fixture. Results:
- Build: 2m 35s.
- `APP082 invite race campaign: 12 checks passed`.
- `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 438 filtered out; finished in 73.43s`, exit 0.
- The local report's sha256 is `8574a23e…46d4`, identical to CI's.

The report stayed in my scratch directory and is not retained here. It is byte-identical to the retained CI copy.

## Acceptance mapping (source at `5695869`)

All lines are in `app/src/auth/invite_tests.rs` unless another file is named. `Notes::record` (2171–2174) prints a check and marks it passed only after the assertions above it. A failed assertion panics the ignored test.

| Acceptance claim | Check, lines | What the source asserts |
| --- | --- | --- |
| **Box 1.** Concurrent last-slot redemption | 1, 2310–2382 | The signup callback stops at the `pending` checkpoint (`sessions.rs` 594–601), holding the code row (`0003_admission.sql` 277–279). A second `POST /invite/validate` waits on `reserve_code`'s FOR UPDATE (0003 237–238). `lock_wait` observes the wait (2346; `admission_qualification.rs` 168–181). The waiter gets the neutral refusal (2351), the PAR and create counts do not change (2357–2358), and there is 1 live pending row with 0 redeemed (2359–2360). The later verified sign-in gives `/workspace` with redeemed 1, `exhausted`, 1 principal, 1 entitlement and 1 `admission.redeemed` (2369–2375). A later validate is still refused (2379). |
| Identical retry | 2, 2384–2433 | The first callback stops at `admit` (`admission.rs` 349–354), and the second callback is seen waiting (2414). Result: two `/workspace` responses with distinct sessions (2418–2420), redeemed 1, 1 principal, 1 entitlement row, 1 `admission.redeemed`, 2 session rows (2421–2426). |
| Requests already waiting in admission | 1, 2, 4, 5, 6 | Each spawns its competitor only after the holder pauses, and each asserts `lock_wait` (2346, 2414, 2517, 2600, 2663). |
| Late response / existing session cannot revive expired entitlement | 3, 2435–2484 | The sign-in stops at `mint` after `fence`, before the entitlement re-read (`sessions.rs` 781–790). The expiry is committed while it is stopped (2445). Result: denied page, no live session (2448–2458), active entitlements 0 with rows still 1 (2459–2460), check-2 sessions get the uniform 404 (2464–2470), and a later sign-in is denied with no new row (2476–2483). |
| Code/entitlement revocation; revoked entitlement not revived | 4, 2486–2562 | `revoke-entitlement` waits on the member fence. The waiting statement is `lock_member` FOR UPDATE (`admission_operator.py` 1023–1028, 1031–1045), and the wait is observed at 2517. Result: mint `/workspace` and `changed: true` (2526–2532), active 0 with rows 1 (2533–2534). The new session and the check-1 session both get the uniform 404 (2537–2549), and a later sign-in is denied (2555–2561). |
| Both race orders | 5, 2564–2639 (admission first); 6, 2641–2701 (revocation first) | **Admission first:** code `revoke` waits on `lock_code` FOR UPDATE (`admission_operator.py` 946–951, 954–966; observed at 2600). The admission stands, the code becomes `revoked` with redeemed 1, the session still works, and replay does not take a second slot (2609–2638). **Revocation first:** an uncommitted code-row update (2657) holds the lock while the callback is seen waiting (2663). Result: denied, no session, 0 principals, 0 entitlement rows, redeemed 0, code `revoked`, 1 `denied_for_reconciliation` row and audit, and a later sign-in is still denied (2668–2700). The `denied_for_reconciliation` state is written by 0003 375–379. |
| Restart/reopen | 12, 3044–3139 | A replaced `Auth` and router run on the same stores (3066–3070). Result: old session 404 (3071–3074); old pre-login callback 400 `signin` with no principal (3081–3086); old CSRF 403 (3094); fresh resend gives `/signin` (3104); current member `/workspace` (3107–3114); revoked member denied (3117–3123); unspent code 303 (3133–3138). The recorded name limits this to an in-process restart. |
| **Box 2.** Create timeout/lost response and reconcile | 7, 2703–2754; 8, 2756–2822 | For `drop_before` (`fake_fauth.py` 222–223), the issue result is `unknown` / `transport_ambiguous` with no capability and no remote id (2710–2723). This is the client's `status is None` path (`provider_management.py` 385–389). The POST count goes up by 1 (2724). Reconcile inside the grace defers it (2725–2730; `admission_operator.py` 2012–2015, 2027). After aging, it is `failed` with no second POST (2731–2739), and validate is refused with PAR unchanged (2746–2752). For the 13 s `delay_after` against the 10 s deadline (`fake_fauth.py` 229–230; `provider_management.py` 75, 268–271), the elapsed time is 8–18 s and the state `unknown` (2768–2779). After aging, the create is `confirmed` and then revoked because the token was lost (2790–2797; `admission_operator.py` 1858–1865), with no second POST (2798), the remote status `revoked` (2804–2809), and validate refused (2815–2820). |
| Revoke timeout and retry; signup after local revocation | 9, 2824–2904 | Revoke returns `changed: true` with the remote state `unknown` in 8–18 s (2856–2864). The code stays `revoked` (2865) and the capability file is gone (2867–2875). The verified sign-in is denied with 0 principals and 0 entitlements (2881–2888). Reconcile retries the revoke to `confirmed` with no extra create (2889–2897; `admission_operator.py` 1716–1754, 2049–2062), and the sign-in is still denied (2902–2903). |
| Signup after local expiry / exhaustion | 10, 2906–2994 | **Expiry:** denied, 0 principals, 0 entitlements, 1 `denied_for_reconciliation` row and its audit (2924–2936; 0003 380–382). **Exhaustion:** the callback is prepared while the code is valid, then `consume_last_slot` takes the slot (2953–2963). The original callback is then denied with no session, principal, entitlement or membership (2968–2980). There are 0 denial rows/audits, because `record_pending_admission` returns early (0003 289–292). The single `admission.redeemed` belongs to the slot taker (2986–2993). |
| Remote revocation propagation | 11, 2996–3042 | The stand-in marks the invite `revoked` (3005–3011). Reconcile records the revoke as `confirmed` and removes the capability; the code stays `active` and no create is sent (3012–3029; `ended_remotely`, `admission_operator.py` 1869–1915). Validate is refused with PAR unchanged (3034–3040). |
| No blind duplicate create / fabricated success / GenUI grant | 7–11 | The POST count is unchanged after every unknown outcome (2739, 2798, 2866, 2897, 3029). An unknown revoke is reported `unknown`, not success, and the local revoke is never undone (2862–2865; `admission_operator.py` 966–970). No principal is minted (2887, 2903, 2930, 2975). |
| Retain partial identity outcome; reconcile via operator procedure | 6, 9, 10 | The `denied_for_reconciliation` reservation and audit are retained (2684–2688, 2932–2936) and surfaced by operator `list` (`admission_operator.py` 915, 938). Provider invitation state is reconciled by `reconcile` (2889–2896). See finding N3. |
| **Box 3.** Campaign in tenant-foundation on the exact source plus independent review | Run `43144269940` / job `130938339563` / step 16 on `5695869` | Step success with 12 checks. The artifact bytes equal the retained copy. This review binds them. No defect was found, so no APP-074 or APP-076 rerun is required. APP-074's own step 15 also passed in the same job. |

## Findings

### Blocking

None.

### Non-blocking

- **N1. The earlier review is unattributed and stale.** [independent-review.md](independent-review.md) names no reviewer. It was committed in `2850d59` together with Grok's binding, the README and the ticket move, so its independence cannot be checked. Its last line says "The ticket stays Backlog until someone checks the item", but that same commit moved the ticket to In review and checked box 3. Its `admission_operator.py` and `provider_management.py` line citations (912–923, 989–1002, 1666–1694, 1717–1810, 1813–1855, 1993–2003, 378–385) are shifted by APP-084. Its binding is to `a9a7305`, not current main. Keep it as history. This file supersedes it for item 3.
- **N2. Three "mints nothing" assertions are vacuous.** `principals(&c, "usr_lost_create")` (2753), `"usr_slow_create"` (2821) and `"usr_remote_revoked"` (3041) query subjects that no step ever registers. Real subjects use the `usr_00000000-…` form (2011–2016). They are always 0. The meaningful controls are the neutral HTTP 400 refusal, which never starts PAR, and, in checks 7 and 11, the unchanged PAR count. Suggestion for a later edit: assert the PAR count in check 8 as well, and drop or replace those three lines.
- **N3. Provider-side identity cleanup is not exercised.** The campaign retains the partial outcome (the `denied_for_reconciliation` row and audit) and reconciles provider invitations. It runs no step that removes or links the fAuth identity created by hosted registration. ADR-006 Flow 2 assigns that step to "the provider's supported procedure", and the app has no command for it. This matches the ADR, but it is a stated limit, not proof.
- **N4. Accepted limits, already stated in the report's `limits` field.** The public last-slot race is hold-versus-validate, not two completed redemptions: one server-only invite is single-use and email-bound. The restart is in-process. `lock_wait` counts any `Lock` waiter in `genui` and does not name the waiting query. The holder-then-single-competitor structure identifies the waiter. The provider is `fake_fauth.py`. Neither hosted nor managed acceptance is claimed.
- **N5. Not in this campaign's scope.** The campaign does not assert the new APP-084 `provider.invitation.*` operator events. APP-073's admission campaign (step 11, same job) asserts the operator chain count of 24 that includes them. Per the ADR-007 D7.8 amendment, an unknown revoke writes `provider.invitation.revoked` with an `outcome_digest` over `state: unknown`. A reader who sees only the kind could mistake that for success. That is APP-084's design, not an APP-082 defect.
- **N6. Cosmetic.** The section comments number 11 blocks while 12 checks are recorded. The second provider block has no comment header, and the comment "8." is the ninth check.

## Verdict

Item 3 is satisfied on current main: the campaign ran in the tenant-foundation workflow on `5695869` (run `43144269940`, job `130938339563`, step 16, success). Its report equals the retained copy byte for byte. An independent local run on the same SHA reproduced it. The source at that SHA asserts what boxes 1 and 2 claim, within the limits stated above. No defect was found, so no fix and no APP-074 or APP-076 rerun is owed. Moving the ticket to Done remains the lead's board action. This file does not move it.

Verdict: no blocking finding — Done may be recorded.
