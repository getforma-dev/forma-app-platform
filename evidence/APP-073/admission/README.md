# APP-073 slice 2 — admission operations, entitlement checks, operator command (plan Task 5)

Date 2026-10-29. Base `e39c1bb` (phase-one-spikes, migration 0003 merged). Status: implemented and qualified **locally** on disposable PostgreSQL 16.13 in Docker with verified TLS; no managed (RDS) application. The independent review ([independent-review.md](independent-review.md), changes requested) was answered in place on 2026-10-29; see §Independent review fixes and the review's §Dispositions. The exact-command table below records the final runs after those fixes (the original slice's counts are noted where they changed). Nothing here establishes sessions, provider calls, public routes or hosted admission (APP-009/074/076).

Controlling text: [ADR-007](../../../docs/architecture/ADR-007-PHASE-1-WRITE-AUTHORITY.md) D7.1 (functions, the nonmember entry, operator-only revocation), D7.2, D7.3, D7.7, D7.8, D7.9 (`Admission`), D7.12, D7.17, D7.19; plan Task 5 (where it restates names, ADR-007 controls); [APP-073](../../../Tickets/0-backlog/APP-073.md) acceptance tests; ADR-003; ADR-004; ADR-006 (pending admission, Flow 2); what 0003 built ([schema-0003/README.md](../schema-0003/README.md)); and the coordinator's instruction to fold the 0003 independent review's fixes (M1, M2, m1, m2, m3, m5, m6) into this slice. The review file itself (`evidence/APP-073/schema-0003/independent-review.md`) lives in the main checkout and is not part of this branch.

## D7.17 defaults, recorded before any code is issued

`admission_operator.py defaults` prints the configured values; the admission campaign runs it first and asserts `SELECT count(*) FROM genui.access_codes` is 0 at that moment (check 1 of [admission-campaign.json](admission-campaign.json)). The values are the ones 0003 enforces in its CHECKs and definer bodies:

| Setting | Value | Enforced by |
| --- | --- | --- |
| Trial entitlement | 30 days | `admit_redemption` (`now()+interval '30 days'`), `access_codes.trial_days CHECK (= 30)` |
| Code expiry | default 14 days, maximum 30 after issuance | operator argument bound 1–30; `CHECK (expires_at <= issued_at + interval '30 days')` |
| Redemptions per code | default 1, maximum 10 | operator argument bound 1–10; `CHECK (max_redemptions BETWEEN 1 AND 10)` |
| Pre-signup reservation | 10 minutes, never counted, at most 10 live per code | `reserve_code` |
| Pending admission | 24 hours, counted against remaining redemptions | `record_pending_admission` |
| List rows | 100 | operator `list` (`truncated` flag beyond it) |
| Code secret | 32 CSPRNG bytes (256 bits), base32, shown once | operator `issue` (`secrets.token_bytes(32)`) |
| Sign-in handle | 16 CSPRNG bytes (128 bits), base32 lowercase | operator `provision` |
| Provider invitation role | allowlist `PROVIDER_ROLES = ("user",)`; every other role, an omitted role and a near spelling refuse `provider_role_refused` | operator `issue` (review M1). Source: FormaStack `Forma/work/FORMA-367/gatewasm` at `1bf5d3b`, `crates/server/src/handlers/management.rs:1694-1712` and `hosted_code.rs:74` (a `server_only` invitation requires a role; built-in roles are `role_admin` and `role_user`). Widened only by an APP-006-accepted provider contract, never by operator input |

## What changed

| Area | Files |
| --- | --- |
| Storage admission | `app/src/storage/admission.rs` (new): `VerifiedNonmember` (fields `pub(super)`, constructed only inside `storage`; `compile_fail` doctest), `Reservation`, `PendingAdmission`, `PendingReason`, `Admission` (D7.9), `SignOut`, `code_digest`, `handle_digest`, `Workspace::{resolve_code, resolve_handle, reserve_code, record_pending_admission, admit, advance_epoch}`, test hooks `reserve`/`pending`/`admit`/`sign_out`, unit tests with the shared digest vectors |
| Nonmember entry, entitlement checks | `app/src/storage/authorization.rs`: `Transaction` holds `Option<&TrustedRequest>`; `begin_nonmember` (workspace pool only, organization scope or none, blank principal and epoch) can never precheck, fence or obtain a `Permit`; `precheck` requires an `active` entitlement with `expires_at > clock_timestamp()`; `fence` re-reads it after `lock_member_authority`; `query_classified` keeps a SQLSTATE inside storage (D7.9); `app/src/storage/pool.rs` `RolePool::role` |
| Migration 0004 | `app/migrations/0004_entitlement_reads.sql` (new, sha256 `138c655d…a3681c2`): `GRANT SELECT(organization_id,principal_id,entitlement_id,status,expires_at) ON genui.entitlements TO genui_workspace,genui_control,genui_data` (D7.3's SELECT column, by column), version CHECKs `BETWEEN 1 AND 4`; `catalog-v4.json` (generated); pins moved to 4 (`runtime.py`, `catalog.rs`, `pool.rs`, `managed_qualification.rs`, native campaign literals); `.dockerignore` admits 0004, catalog-v4 and `admission.rs` |
| 0003 review fixes (in place) | `app/migrations/0003_admission.sql` (now sha256 `55e65c50…d9d99af`, 37,359 bytes, after the independent review's M2 and m3; before them `3317bed3…2ed903`), `catalog-v3.json` (now `04b33a6b…2169a8`), `catalog-v4.json` (now `336f870e…e43c6`), `manifest.json` (`cc27fbac…894d74d`) — see §Review fixes and §Independent review fixes |
| Operator command | `app/runtime/admission_operator.py` (new): closed subcommands `defaults`, `provision`, `backfill`, `issue`, `list`, `revoke`, `extend`, `revoke-entitlement`, `revoke-membership`, `verify` |
| Seed | `app/runtime/tenant_seed.py`: each active synthetic member gets one deterministic synthetic entitlement (kind `member_invite`, 2026-10-01 → 2036-10-01) so the APP-008 fixtures keep reading under the new precheck |
| Tests | `app/src/storage/admission_qualification.rs` (new Docker/TLS campaign `native_admission_campaign`, `#[ignore]`), `app/src/storage/qualification.rs` (fixture items `pub(super)`, `fixture_python()`), `app/runtime/test_admission_operator.py` (new), `app/runtime/qualify_tenant_schema.py` (0004 grant set, review fixes) |
| CI | `.github/workflows/tenant-foundation.yml`: new step running the admission campaign after the native campaign |
| Evidence | this directory; `evidence/APP-073/schema-0003/README.md` corrected (review m6) |

### Digest constructions (Rust and Python agree on fixed vectors)

- Access code: 32 CSPRNG bytes, printed as 52 RFC 4648 base32 characters in groups of four (`XXXX-XXXX-…`). A typed code is normalized by dropping hyphens and ASCII whitespace and upper-casing; only canonical base32 decoding to exactly 32 bytes (trailing bits zero) is a code. Stored digest `sha256:` + hex SHA-256(`forma:access-code:v1` ‖ 0x00 ‖ the 32 bytes). Vector: bytes 0..31 → `AAAQ-EAYE-…-DYPQ` → `sha256:3d6abee2…3752cc` (`admission.rs` and `test_admission_operator.py`).
- Sign-in handle: 16 CSPRNG bytes as 26 lowercase base32 characters; digest under `forma:signin-handle:v1`. Vector: bytes 100..115 → `mrswmz3infvgw3dnnzxxa4lsom` → `sha256:4a08c5c0…2e7c22`.
- Intended provider email: `sha256:` + hex SHA-256(`forma:intended-email:v1` ‖ 0x00 ‖ trimmed lowercase address); the address itself is never stored. The digest is unkeyed, so it is pseudonymous, not secret: anyone holding a candidate address can confirm it. It must be handled as personal data, not as a hidden value (review nit).

## Exact commands and results

Interpreters: the existing locked venvs of the main checkout (this worktree has none). `PYC` = the scratch wrapper `/private/tmp/claude-501/-Users-victorvillacis-dev/df0e31da-4f04-46a7-8043-30cdac91f9c2/scratchpad/t5/pyc`, which only runs `<main checkout>/app/runtime/.venv/bin/python` with `PYTHONPATH=<main checkout>/contracts/.venv/lib/python3.11/site-packages` (equivalent to the CI venv, which installs both locks). `PY` = `<main checkout>/app/runtime/.venv/bin/python` alone.

| Command (repository root unless noted) | Result |
| --- | --- |
| in `app/`: `CARGO_BUILD_JOBS=2 cargo +1.93.1 test --locked --lib storage:: --no-run` with the tests and the module declarations written but `admission.rs` absent (TDD red run) | compile failure, 38 errors (missing `storage::admission`, `Workspace::{resolve_code, reserve_code, …}`) |
| `PYC app/runtime/generate_schema_catalog.py --output-dir <new scratch dir>` (three runs) | exit 0; catalog-v1/v2 byte-identical to the committed files; v3 `04b33a6b…` and v4 `336f870e…` identical across all three runs and a fourth run after the last campaign edit; bodies 205,463 / 206,604 bytes, files 201,584 / 202,710 bytes (guard 419,430); receipt [generation.json](generation.json) (the original slice's v3/v4 were `16fcd0ed…`/`8f8e280b…`) |
| `python3 scripts/check-schema-pins.py` | exit 0, `agree: true`: five version pins = 4; four bound pins = 524,288; `catalog-v2/v3/v4.json` → 2/3/4 |
| `PYC app/runtime/qualify_tenant_schema.py --output <scratch>/schema1.json` | exit 0, `passed: true`, **289 checks** (279 before the review fixes; +1 digest typing, +9 malformed head/event hash writes refused 23514), `container_removed` and `fresh_container_removed` true; [schema-campaign.json](schema-campaign.json) (its `source_sha256` equals the committed migrations, catalogs, seed and campaign) |
| `PYC -m unittest discover -s app/runtime` | 59 tests OK (39 existing + 20 operator tests, 3 of them added by the review fixes; all DB tests ran against their own container) |
| `PY -m unittest discover -s app/runtime` | 59 tests OK, 1 skipped (the contract-encoder comparison needs the contracts packages) |
| `PY -m unittest discover -s scripts -p 'test_*.py'` | 5 tests OK |
| `PYC fixtures/reset_trial.py --output <new scratch dir>` | exit 0, `passed: true`, owned workspaces removed; [reset-trial-summary.json](reset-trial-summary.json) |
| `<main checkout>/spikes/APP-005/.venv/bin/python -m unittest discover -s fixtures` | 14 tests OK |
| `python3 scripts/check-foundation.py` | exit 0 (it rewrites `evidence/APP-007/local-preparation.json` as a side effect; that change was reverted, not committed) |
| in `app/admin`: `npm ci`, `npm run build` (Node 22.21.0, npm 11.6.2) | exit 0 |
| in `app/`: `cargo +1.93.1 fmt --check` | exit 0 |
| in `app/`: `CARGO_BUILD_JOBS=2 cargo +1.93.1 clippy --locked --all-targets -- -D warnings` | exit 0 |
| in `app/`: `CARGO_BUILD_JOBS=2 cargo +1.93.1 test --locked` | exit 0: lib 31 passed / 3 ignored; bin 1 passed; doctests 2 passed (both `compile_fail`: `TrustedRequest`, `VerifiedNonmember`) |
| in `app/`: `CARGO_TARGET_DIR=target/msrv CARGO_BUILD_JOBS=2 cargo +1.85.0 check --locked --lib --tests` | exit 0 |
| in `app/`: `GENUI_NATIVE_FIXTURE_PYTHON=PYC GENUI_NATIVE_REPORT=<scratch>/native2.json CARGO_BUILD_JOBS=2 cargo +1.93.1 test --locked --lib storage::qualification::native_postgres_campaign -- --ignored --exact --nocapture` | 1 passed, report `passed: true`, 36 checks (the APP-008 TLS campaign, now under the entitlement precheck and the v4 catalog); [native-campaign.json](native-campaign.json) |
| in `app/`: `GENUI_NATIVE_FIXTURE_PYTHON=PYC GENUI_ADMISSION_REPORT=<scratch>/admission2.json CARGO_BUILD_JOBS=2 cargo +1.93.1 test --locked --lib storage::admission_qualification::native_admission_campaign -- --ignored --exact --nocapture` | 1 passed, report `passed: true`, **26 checks** (24 before the review fixes; new checks 13 and 20 are review m2), no raw code in the report; [admission-campaign.json](admission-campaign.json) |

Run the two `#[ignore]` campaigns one at a time (`--exact`): they share the process-wide eight-connection capacity and the native campaign asserts it exactly. `qualify_tenant_schema.py` uses trust authentication without TLS on loopback; the TLS paths are the two Rust campaigns and the operator-command tests, where the command always connects `sslmode=verify-full` to a throwaway CA. Every container these commands started was removed; `docker ps -a` afterwards shows only `forma367-qual-pg` and `gatewasm-db-test-1`, which predate this work and belong to other sessions.

Process notes: the Rust tests were written first and the red run above is a compile failure, not a behavioral failure. The Python test module was written before `admission_operator.py`, but no red run was recorded before the command existed; its first run had 3 failures, all in the test helper (pg_dump 16.13's per-run `\restrict` keys made dump comparisons unequal), fixed in the helper.

## Acceptance items (local only) and where they are proven

| APP-073 acceptance (local part) | Evidence |
| --- | --- |
| Operator command issues, lists and revokes bounded codes on the real role/tenant harness (PG 16.13, verified TLS, four roles, `genui_lock` fences); raw code only at creation, absent from storage, logs, URLs and evidence | admission campaign checks 3 and 25 (pg_dump with and without `--data-only`, stderr empty on every success, list output has no code or digest, report has none); operator tests `test_issue_prints_the_secret_once_and_stores_only_its_digest`, `test_refusal_output_never_carries_values` |
| Ordinary application/member role and forged operator identity denied | campaign check 24 (all four roles through the command refused with the fixed reason `operator_identity_refused`; the wrong password with `admission_operator_refused`; `SET ROLE genui_owner`, `code_index`/`access_codes`/`entitlements`/`audit_events` writes refused 42501 for workspace, control and data inside member transactions; wrong operator password refused; nothing written); `test_forged_operator_identity_and_application_roles_are_refused` |
| Organization, permissions, expiry, entitlement server-fixed; foreign, guessed, malformed, expired, revoked, exhausted refuse without disclosure; reservation grants nothing | campaign checks 4–6, 9, 14, 21, 23 and the schema campaign (`reserve_code`/`record_pending_admission`/`admit_redemption` outcome matrix, m3 malformed identity) |
| Redemption, slot, membership, entitlement and audit atomic, only with `email_verified` true; 24-hour pending admission bound to the identity, counted, granting nothing; last-slot race admits exactly one; identical retry consumes once; different principal or payload refuses; rollback and restart | campaign checks 6 (unverified mailbox: `Pending{EmailUnverified}`, nothing written), 7 (cancelled redemption rolls back), 8–10 (atomic admission, replay, mismatch, restart), 18–19 (observed-lock-wait last-slot race; concurrent identical retries); a repeated pending admission of one identity on one code returns the live row (schema campaign; operator test, review m3) |
| Both redemption/code-revocation orders and both entitlement-use/revocation orders; expiry cannot resurrect; distinct effects of code and entitlement revocation | campaign checks 11, 12, 14–17 and 21; an identical retry after entitlement revocation (13) or racing a membership revocation (20) is `Refused` (review m2) |
| Reservation expiry, explicit extension, bounded list and audit, no cross-tenant reset | campaign checks 5, 21 and 25; operator tests for extend (never shortens, review m5)/revoke/list (lapsed and reconciliation counts, review m3)/verify (one snapshot, review m1); `verify` recomputes both organizations' chains |
| Provider invitation: intended email, explicit role, remote id, pending state; omitted role refuses; revocation records the remote revoke as pending | `test_provider_invitation_is_recorded_pending_without_the_address`, `test_issue_argument_bounds`, `test_issue_refusals_write_nothing` (local durable state only) |

Not covered here, by the ticket's own division: the managed RDS application (APP-076), provider timeout/partial-outcome injection and reconciliation of unknown remote outcomes (needs the accepted APP-006 contract; this slice records `pending` operations only and never calls the provider), sessions, public routes and browsers (APP-009/074), an independent authorization/concurrency review of this slice.

## Review fixes applied to 0003 (coordinator instruction)

0003 had never been applied outside disposable databases, so it was edited in place and catalog-v3 regenerated and re-pinned.

- **M1** `admit_redemption`: a pending admission refused because its code is revoked, expired or exhausted is now marked `denied_for_reconciliation` with one `admission.denied_for_reconciliation` event (ADR-006 Flow 2), exactly like the 24-hour case; `already_member` still writes nothing. Schema campaign: the three cases now assert `audit_events +1`, the state, the event's code, and a retry answering `expired`; the chain count of `admission.denied_for_reconciliation` is 4.
- **M2** principal insert is `ON CONFLICT (organization_id, issuer, provider_tenant_id, provider_subject) DO NOTHING; IF NOT FOUND THEN … 'already_member'` (first write of the branch, no savepoint). Schema campaign race case (4): one identity through two codes, observed lock wait, `admitted` and `already_member`, one principal, the loser's pending row stays live and its code's count stays 0. Storage originally also mapped any residual 23505 from `admit_redemption` to `Admission::AlreadyMember`; after the independent review's nit it maps every database refusal there to `Error::Unavailable` (rolled back), because the identity key can no longer raise it and a future unique key must not read as a business fact.
- **m1** (Rust): `Workspace::advance_epoch` calls `advance_member_epoch` instead of `lock_member_authority` (the definer's `FOR UPDATE` is the fence); the admission campaign races two concurrent sign-outs of one member with an observed lock wait: one advances (epoch 4), the other is refused, no deadlock.
- **m2** `genui_admit` no longer holds SELECT on `principals.principal_id` (grant set and campaign updated).
- **m3** `record_pending_admission` and `admit_redemption` return no row for issuer/tenant/subject lengths outside the 0002 column bounds (schema campaign cases added); storage checks the registry and an **active** scoped provider binding before calling, and the admission campaign proves an inactive binding refuses both the pending admission and the redemption.
- **m5** the schema campaign and the operator `verify` command both check that every `code_redemptions.audit_event_id` resolves to an `admission.redeemed` event of the same organization, code, actor and `outcome_digest = payload_digest`.
- **m6** `schema-0003/README.md` corrected (STRICT folding with any NULL constant; CI run `40947220411`; 13 implementation choices and their dispositions).
- **m4** was first left for the coordinator's ADR change; that amendment (`db1ad04`) landed, and the independent review raised it as M2, so the three columns are now `genui.digest` (§Independent review fixes). Not done here: **m7** (record only); the coordinator is amending ADR-007 text (m4, m5 deletion consequence, m1 order) on the main branch. The operator command and the campaigns already use the m4 conventions (`sha256:<hex>`, genesis head at `chain_sequence` 1, first event 2).

## Implementation choices (for review)

1. **Entitlement reads by column, migration 0004.** D7.3 assigns `entitlements` SELECT to workspace, control and data and says reads are granted by column; 0004 grants the five columns the precheck, fence and APP-009's session reference read. **Renumbering (D7.12):** ADR-007 planned 0004 for APP-010's definition/work grants; this lands first, so APP-010 becomes 0005, sessions 0006 and commands 0007, and each later cumulative catalog is regenerated.
2. **Entitlement clock.** Precheck and fence judge `expires_at > clock_timestamp()` (statement wall clock), not `now()`, so an entitlement that expires inside a transaction is refused at the fence (campaign check 11).
3. **Entitlement revocation** is an operator transaction: organization `FOR SHARE` → member `FOR UPDATE` → entitlement `revoked` → `membership_epoch + 1` → `entitlement.revoked` event. The epoch move is what makes it observable through the member fence in both race orders; membership revocation is the same with `memberships.status`.
4. **Nonmember entry is a `Transaction` without a request.** It is called only from `admission.rs` (a convention, stated at the definition: the type system admits any `storage` module), requires the workspace pool, installs the organization (or nothing for the two lookups) with blank principal and epoch, and `precheck`/`fence` return `Denied` for it, so it can never yield a `Permit`. A reserve/pending/admit call for an organization whose code is unknown returns `Err(Denied)` (no row), never a disclosure.
5. **`VerifiedNonmember` carries `email_verified`.** The plan lists four fields; adding the exchange's backend `user.email_verified` makes the ADR-006 gate structural: `admit` answers `Admission::Pending { reason: EmailUnverified }` and touches nothing when it is false. `PendingReason` has that one variant.
6. **`Admission` mapping (D7.9).** `admitted` and `replayed` → `Admitted` (a replay is "the prior result"); `already_member` → `AlreadyMember`; `expired`, `revoked`, `exhausted`, `mismatch` → `Refused`. A code-side refusal commits (it now records the reconciliation denial); a mismatch commits nothing because the function wrote nothing.
7. **Typed `SignOut { One, All }`** instead of a reason string; the definer's own reason check stays.
8. **Operator command identity.** It connects with whatever `PGUSER` the environment names and then refuses unless `session_user = current_user = genui_migration` (so a forged identity is refused by the command and, independently, by PostgreSQL privileges), then proves the reviewed latest catalog with `runtime.verify_schema_catalog` before any write, then `SET LOCAL ROLE genui_owner` and explicit organization scope — the same identity and role switch as `runtime.migrate`/`tenant_seed.seed` (`genui_migration`, verify-full TLS, `SET LOCAL ROLE genui_owner`). `issued_by` is stored as `genui_migration:<label>`; the label is an operator note, not authentication. `PGPORT` is honored (runtime.py's `migrate` fixes 5432).
9. **Secret output.** The code (and a provisioned handle) is printed once, in the single JSON line on stdout, only after `COMMIT` succeeded; refusals print a fixed reason code on stderr and never an argument value (argparse's own error echo is replaced).
10. **Backfill writes no audit event**: D7.8's operator kinds are closed and name no backfill kind; the genesis heads are the record and `verify` recomputes them.
11. **`organization_sequences` (D7.7) is 0006's (now 0007's).** `provision` and `backfill` call a hook that returns a `deferred` note today and **refuses** once `genui.organization_sequences` exists, so the APP-013 slice must implement the row before either command can run again.
12. **Extension** only for an `active`, unexpired code and never past `issued_at + 30 days`; an expired code is reissued, never resurrected.
13. **Revocation and provider invitations.** `revoke` inserts a `revoke` operation in state `pending` for each non-failed `create` invitation of the code, so the required remote revocation is durable; no provider call is made.
14. **Seeded synthetic entitlements** (see What changed) so existing fixtures keep their APP-008 meaning; they are fixture authority, not admission results.
15. **Duplicate pending admissions** (review observation): first accepted as bounded; superseded by the independent review's m3(a). One identity now holds at most one live pending admission per code: `record_pending_admission` answers a repeated callback with the live row's id and expiry as `pending`, consumes the new presignup and writes no row or event. One identity can still hold one pending admission on each of several codes; after the first redemption the others answer `already_member`.

## Questions for 0003

None beyond the review's findings, which are fixed above. Two residual notes: `reserve_code` still checks only the context digest's shape (the binding to the browser flow rests on APP-009 keeping the reservation id inside the pre-login record, as the review observed); and the review's m4 column typing is left for the coordinator's ADR change.

## Open questions

1. ADR-007 D7.12's planned numbering (0004 definitions/work, 0005 sessions, 0006 commands) shifts by one because this slice adds 0004; the ADR text and plan Tasks 6/7/10 file names need the same renumbering.
2. Should `provision` also be how the two existing organizations get their first sign-in handle (no `rotate-handle`/handle issuance exists for them yet; APP-009 needs one per organization)?
3. Should backfill have an operator audit kind (D7.8 amendment), or is the genesis head sufficient?
4. The admission operator is not in the runtime image (`.dockerignore` unchanged for it); APP-077 packages it with the operator task.

## Independent review fixes (2026-10-29)

The independent review ([independent-review.md](independent-review.md)) requested changes (M1, M2) and listed minors and nits. Every fix below was written test first; the red runs are recorded. Dispositions, including declined items, are appended to the review file.

| Finding | Change | Failing test first (red) |
| --- | --- | --- |
| M1 provider role | `admission_operator.py`: `PROVIDER_ROLES = ("user",)` checked with `require()`; the `ROLE` pattern and the three-name denylist are gone (D7.17 table above) | `test_issue_argument_bounds`: `member` was accepted (`Refused not raised`) |
| M2 hash typing | `0003_admission.sql`: `audit_heads.head_hash`, `audit_events.previous_hash`, `audit_events.hash` are `genui.digest`; catalog-v3/v4 regenerated, manifest re-pinned, `check-schema-pins.py` agrees | schema campaign: the new typing assertion failed with all three columns `text` |
| m1 verify snapshot | `verify` sets REPEATABLE READ and READ ONLY before its first statement and reads the organization without a row lock | `test_verify_reads_one_snapshot_while_an_admission_appends` (an `admission.pending` committed between the head and event reads): `audit_chain_mismatch` |
| m2 replay after revocation | `admission.rs`: `replayed` rechecks an active membership with an active, unexpired entitlement in the same transaction; otherwise `Admission::Refused` | admission campaign: the retry after `revoke-entitlement` returned `Admitted { epoch: 2 }` |
| m3(a) one pending per identity and code | `record_pending_admission`: after the `already_member` check, a live unexpired pending row for the same identity and code is returned as `pending`, the new presignup is consumed, nothing is inserted or audited | `test_second_pending_admission_of_one_identity_reuses_the_live_one`: a second pending id was issued |
| m3(b) lapsed pending visibility | `list` adds per code `lapsed_pending_admissions` (`kind='pending'`, `state='live'`, `expires_at<=now()`) and `denied_for_reconciliation` | `test_list_reports_lapsed_and_reconciliation_pending_admissions`: `KeyError` |
| m5 extend never shortens | `extend` refuses `expiry_not_extended` unless the new expiry is later than the current one | `test_revoke_and_extend_are_audited_and_bounded`: `--expires-days 1` and `4` on a 5-day code were accepted |

**Reconciliation record (m3(b), ADR-006:43).** A pending admission that lapses with no later sign-in stays `state='live'` in storage, is never counted against remaining redemptions, and is reported by `list` as `lapsed_pending_admissions`; one denied at a later sign-in (expired, or its code revoked, expired or exhausted) is `denied_for_reconciliation` with an `admission.denied_for_reconciliation` event. This listing is the operator's reconciliation record for both.

**Trial extension (m5) — documented limit.** Phase 1's "trial extension or reissuance" (ADR-003:51, acceptance 5) is code extension (`extend`, later expiry only, never past `issued_at + 30 days`, audited `code.extended`) plus reissue (a new `issue`). There is no extension of an admitted member's 30-day entitlement: D7.8's operator kinds are closed and name no `entitlement.extended`, and the review's fix offers recording this limit in ADR-007 D7.19 as one option. That ADR sentence is the coordinator's (this branch does not edit `docs/`).

**Seeded codes (m6, plan Task 5) — not added.** Plan Task 5 lists `tenant_seed.py` seeding one active and one expired trial code per fixture organization. They were deliberately not added: codes are issued only through the operator command (D7.19, `admission_operator.py issue`), which prints the secret once and records `code.issued` on the operator chain. A seeded code would be a second issuance path with no operator event and no shown-once secret, and both campaigns already issue, expire (fixture clock moves) and revoke their own codes through the command. The seed writes only the labelled synthetic entitlements described above. Recording the same disposition in the plan is the coordinator's (no `docs/` edits here).

Final commands after the fixes (repository root unless noted; `PYC`/`PY` as above; every container started was removed, `docker ps -a` before and after shows only `gatewasm-db-test-1`, plus containers of other sessions that started and ended independently):

| Command | Result |
| --- | --- |
| `PYC app/runtime/generate_schema_catalog.py --output-dir <new scratch dir>` (four runs) | exit 0; v1/v2 unchanged; v3 `04b33a6b…2169a8`, v4 `336f870e…e43c6` identical across runs; only the three column types and the `record_pending_admission` body differ from the original v3/v4 |
| `python3 scripts/check-schema-pins.py` | exit 0, `agree: true` (pins 4; bounds 524,288; catalog-v2/v3/v4 → 2/3/4) |
| `PYC app/runtime/qualify_tenant_schema.py --output <scratch>` | exit 0, `passed: true`, 289 checks, both containers removed ([schema-campaign.json](schema-campaign.json)) |
| `GENUI_NATIVE_FIXTURE_PYTHON=PYC … cargo +1.93.1 test --locked --lib storage::admission_qualification::native_admission_campaign -- --ignored --exact --nocapture` | 1 passed, `passed: true`, 26 checks ([admission-campaign.json](admission-campaign.json)) |
| `GENUI_NATIVE_FIXTURE_PYTHON=PYC … cargo +1.93.1 test --locked --lib storage::qualification::native_postgres_campaign -- --ignored --exact --nocapture` | 1 passed, `passed: true`, 36 checks ([native-campaign.json](native-campaign.json)) |
| `PYC -m unittest discover -s app/runtime`; `PY -m unittest discover -s app/runtime` | 59 OK; 59 OK, 1 skipped |
| `PY -m unittest discover -s scripts -p 'test_*.py'`; `<main checkout>/spikes/APP-005/.venv/bin/python -m unittest discover -s fixtures` | 5 OK; 14 OK |
| `PYC fixtures/reset_trial.py --output <new scratch dir>` | exit 0, `passed: true`, owned workspaces removed ([reset-trial-summary.json](reset-trial-summary.json)) |
| in `app/admin`: `npm ci`, `npm run build` (Node 22.21.0, npm 11.6.2) | exit 0 |
| in `app/`: `cargo +1.93.1 fmt --check`; `CARGO_BUILD_JOBS=2 cargo +1.93.1 clippy --locked --all-targets -- -D warnings` | exit 0; exit 0 |
| in `app/`: `CARGO_BUILD_JOBS=2 cargo +1.93.1 test --locked` | exit 0: lib 31 passed / 3 ignored; bin 1 passed; doctests 2 passed |
| in `app/`: `CARGO_TARGET_DIR=target/msrv CARGO_BUILD_JOBS=2 cargo +1.85.0 check --locked --lib --tests` | exit 0 |
