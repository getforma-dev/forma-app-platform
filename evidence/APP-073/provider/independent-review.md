# Independent review: APP-073 provider invitations and reconciliation (acceptance items 6 and 7, slice 3)

**Commits reviewed:** `7a2922a` (code) and `ca4fe57` (evidence), merged to `phase-one-spikes` as `236145b`. The checkout was at `29b6ff6` during the review; `git diff --stat 236145b HEAD -- app scripts contracts fixtures` changes only `contracts/implementation-lock.json` (ADR-009 B24), so every reviewed source is byte-identical to `236145b`.

**Controlling text:** `Tickets/2-in-progress/APP-073.md:45-46` (items 6 and 7) and its Verification paragraph; ADR-003 §Proposed confidential provider handoff (`docs/architecture/ADR-003-PUBLIC-BETA-ACCESS.md:37-43`); ADR-007 D7.8 and D7.19 (`ADR-007-PHASE-1-WRITE-AUTHORITY.md:64,108`); the merged contract `evidence/APP-006/upstream-auth-source/merged-contract-2026-10-29/1bf5d3b/README.md:19-23`; the public gatewasm source at `1bf5d3b2fb515c8ed82b0c5a044f7846915e28d0`, read as data with `gh api repos/getforma-dev/gatewasm/contents/<path>?ref=…` (blobs `bc12859a` management.rs, `09e12d0a` handlers/mod.rs, `e2b4cdd1` management_auth.rs, `7fb0f1a0` error.rs, `af0ba7f0` auth.rs, all equal to the README's, plus `db/pg_migrations/001_schema.sql`, `db/mod.rs`, `handlers/onboarding.rs`, `handlers/hosted_code.rs`); the implementer's `evidence/APP-073/provider/README.md` and its three stated risks.

**Method.** Read-only; nothing committed; this file is the only file written in the repository. (1) Read `provider_management.py`, `fake_fauth.py`, the `admission_operator.py` provider code (lines 350-407, 451-485, 588-845), `test_provider_invitations.py`, the `test_admission_operator.py` diff and check 24 in `admission_qualification.rs`, against the controlling text and the gatewasm source. (2) Ran the provider tests, the runtime unit tests and the admission campaign. (3) Ran four adversarial probes (H1, H2, H2b, H3) as extra methods on a subclass of the slice's own `ProviderDatabaseTests` (same disposable PostgreSQL 16.13 TLS fixture and loopback stand-in; the only fixture SQL is the aging helper the slice's tests already use, plus one UPDATE in H2b that simulates a crash). Scripts are in the session scratchpad (`probe/probe_app073.py`), not in the repository. Nothing called the deployed fAuth. `docker ps -a` before and after shows only `gatewasm-db-test-1` (six days old, another session's); short-lived `genui-*` containers from a concurrently running session came and went during the review and were not mine. Every container I started was removed.

## Verdict

**changes requested** — no blockers, two majors. (M1) `reconcile` attributes a provider invitation to a create row greedily, one row at a time. I reproduced it on the real fixture: with two unresolved creates for one address, it confirms a create that never reached fAuth with the *other* code's invitation, judges the applied create `failed`, and revokes that invitation remotely. With a stored capability (H2b), it also destroys the capability. (M2) The capability is a plaintext 0600 file. ADR-003:39 requires "encrypt or use the approved secret store at rest". The README argues this file is consistent with that text, but no approval designates it, and its backup claim is overstated for the operator host. Nothing grants GenUI authority wrongly. No token or key leaked anywhere I looked. No path creates blindly or creates twice. No failed or unknown remote operation restores local access. The loopback stand-in matches gatewasm's real statuses and bodies on every path the tests exercise.

## Local results (exact)

| Command | Result |
| --- | --- |
| `cd app/runtime && ./.venv/bin/python -m unittest test_provider_invitations` | 28 tests OK (38.1 s; the 15 database tests on their own container) |
| `app/runtime/.venv/bin/python -m unittest discover -s app/runtime` | 99 tests OK, 1 skipped (71.1 s). The README's 86 predates the APP-075 supervisor tests, which merged in the same window. |
| in `app/`: `GENUI_NATIVE_FIXTURE_PYTHON=<scratch>/t5/pyc GENUI_ADMISSION_REPORT=<scratch>/rev073p/admission.json CARGO_BUILD_JOBS=2 cargo +1.93.1 test --locked --lib storage::admission_qualification::native_admission_campaign -- --ignored --exact --nocapture` | 1 passed (21.7 s); report `passed: true`, **27 checks**, 11 codes, server 16.13; `grep -cE 'tinv_\|mgmt_\|item7.example'` on the report = 0 |
| H1–H3 probes (`<scratch>/probe/probe_app073.py`, run as `python -m unittest probe_app073.Probe….probe_…` from `app/runtime`) | results under M1 and m1; containers removed |
| `grep -rnE 'mgmt_[A-Za-z0-9]\|tinv_[0-9a-f]{8}\|example\.test' evidence/APP-073/` | no match |

## Findings

### Blocker

None.

### Major

**M1. Reconcile can attribute one invitation to the wrong create, judge an applied create `failed`, and revoke the wrong invitation.** `reconcile_create` (`app/runtime/admission_operator.py:716-753`) judges each pending or unknown create on its own. It excludes remote ids already stored on another row (`:737-738`), uses a window that has a lower bound only (`created_at − 300 s`, `:739`), and confirms the sole candidate (`:740-746`). Rows run in `created_at` order (`:800`). An older unresolved row therefore sees the invitations of every later unresolved row for the same address.

- **Reproduced (H2)**, using the README's own advice to "revoke and reissue":
  1. Code 1's create gets 500 before any write (unknown, never applied).
  2. The operator revokes code 1. The guard then allows code 2 for the same address.
  3. Code 2's create is applied, then the connection drops (unknown).
  4. `reconcile` confirms code 1's create with code 2's invitation `tinv_dc14…` (`found`), records code 2's create as `failed`/`not_found`, and sends code 1's revocation for `tinv_dc14…`. That revokes code 2's invitation. Code 2 stays `active` with a `failed` create.
- **H2b, the same sequence with code 2's capability stored** (the crash between the file and the UPDATE that the README designs for, simulated by one fixture UPDATE). The first reconcile misattributes as above. The second reconcile's sweep destroys code 2's valid capability (`:763-770`, its create is now `failed`).
- **By reading:**
  - Two concurrent `reconcile` processes each compute `known` before the other commits, and nothing serializes them per tenant: no lock, and no unique index on `remote_invitation_id`. So both could attribute the same invitation. My timing probe (H3) did not hit that window; the code allows it.
  - An invitation created outside GenUI in the window is attributable to a create that never applied. fAuth's self-serve console (`onboarding.rs:2576-2629`, role taken from the body) and the `/admin/invites` alias both create invitations. That is indistinguishable from "ours" by email, role and time alone.
- **Effect:** everything fails closed (no GenUI grant follows), but item 7's reconciliation record states false facts. `failed` is defined as "definitively not created" (README §Operation state machine). The address guard (`:371-378`), `process_revoke` (`:694-698`) and the sweep all trust it.
- **Fix:**
  - (a) Attribute only one-to-one: an invitation may match a row only if it lies in exactly one unresolved same-digest, same-role row's window. Otherwise every row involved stays `unknown`. Also bound each window above by the next same-digest create's `created_at` + skew.
  - (b) Serialize create judgments per provider tenant. Take the `provider_tenant_bindings` row `FOR UPDATE`, as `issue` already does (`:365-367`), or add a partial unique index on `(organization_id, remote_invitation_id) WHERE operation='create'` with the next migration.
  - (c) Accept only invitations with `invited_by_user_id` null. Management-key creates carry no inviter claims (`management.rs:1678,1744`); console and admin-UI creates do (`onboarding.rs:2618`), and the list returns the field (`management.rs:1653`).
  - (d) When a capability file exists for the row, take the remote id from the file, which was written from a validated 201, instead of inspecting.
  - Add H2 and H2b as tests.

**M2. The capability's at-rest protection rests on an argument, not on ADR-003's text or a recorded approval.** ADR-003:39 says the capability must be "encrypt[ed] or [in] the approved secret store at rest". Item 6 (`APP-073.md:45`) says "through the approved secret channel". `write_capability` (`provider_management.py:382-401`) writes the token in plaintext to a 0600 file. That is access control, not encryption. Unchecked host disk encryption is the only at-rest layer, and the README states this honestly (README:64). No ADR, ticket or plan text names this file as the approved store (searched `docs/` and the tickets for "approved secret"/"secret store"). README:60 calls the file "consistent with" ADR-003 and says "dumps, WAL, backups, snapshots and replicas cannot carry it". That holds for PostgreSQL, but not for the operator host's own backups, snapshots and synced folders (for example Time Machine, APFS local snapshots or a cloud-synced home directory), and unlink is not erasure. Nothing leaks today: only simulated tokens exist, and live use waits for APP-006 acceptance and APP-074/076. So this is not a blocker. **Fix:** do one of the following before the first real invitation.

- Encrypt the file with a key the operator host holds outside the file system tree, such as the OS keychain now and KMS under FORMA-248.
- Or record an owner decision (an ADR-003 amendment) that designates the operator-host 0600 file as the Phase 1 approved store, with conditions: full-disk encryption on, the directory excluded from backup and sync, a single named host, and synthetic or beta invitations only until FORMA-248.

Either way, correct README:60's backup sentence.

### Minor

- **m1. A revocation that lands while the create is in flight leaves a live capability for a revoked code. Reproduced (H1).** The stand-in delayed the create's response by 4 s, and `revoke` ran during it. `revoke` destroyed 0 capabilities (the file did not exist yet) and left its revocation `pending`/`awaiting_create_reconciliation` (`admission_operator.py:699-701`). The create then completed: `write_capability` ran, the row was `confirmed` and `capability_stored` was true (`:651-668`). End state: code `revoked`, capability file present, remote invitation `pending`. It stays that way until someone runs `reconcile`. GenUI handoff is still refused, because `reserve_code` answers `revoked`. But README:62's lifetime sentence ("destroyed by `revoke` before the provider is contacted") does not hold here. **Fix:** in the outcome transaction, run `lock_code`. If the code is no longer active and live, or a `revoke` row exists, skip `write_capability` and send the revocation immediately, as the contract-mismatch branch already does (`:664-667`). The lock order stays deadlock-free: no writer holds the code and then waits on a create row.
- **m2. A create confirmed by reconcile without a capability is left pending remotely.** `create_remote` revokes every confirmed-but-unstored create (`:664-667`); `reconcile_create` does not (`:746-753`). The slice's own test asserts the invitation stays `pending` (`test_provider_invitations.py:478`). The lost response's token can survive in a proxy or log. The live invitation also keeps the address guard closed for the code's whole lifetime. **Fix:** apply the same policy. If the create is confirmed and `capability_present` is false, run `record_revocations` and `process_revoke`.
- **m3. An ambiguous or unresolvable create has no closed way to end.**
  - `ambiguous_remote_match` stays `unknown` on every run (`:742-745`).
  - The code's revocation waits on it forever (`:699-701`).
  - D7.19 forbids generic SQL, and "the operator procedure with FormaStack" exists only as prose.
  - Stuck rows also sort first into the 100-operation budget (`:800`).

  **Fix:** add a time bound. fAuth clamps invitation lifetime to 168 h (`management.rs:1715`), so once `now() > created_at + 168 h + skew` nothing that create made can still be pending, and its revocation can be confirmed as moot. Optionally add a closed `settle` subcommand that records an operator decision with its evidence.
- **m4. Provider expiry and code expiry diverge.** No `expires_in_hours` is sent, so invitations live 72 h (`management.rs:1715`), while codes default to 14 days and allow up to 30. After 72 h the sweep removes the capability, but the guard (`:371-378`) still counts the confirmed create of the live code. The address therefore cannot be re-invited until the operator revokes the code, and the README does not say so. **Fix:** do one of these:
  - Send `expires_in_hours = min(168, code lifetime in hours)`.
  - Or treat a provider-expired invitation as ended in the guard.
  - Or document the revoke-and-reissue step.
- **m5. The one-time code can be lost after COMMIT.** `process_revoke` after a contract mismatch runs outside `create_remote`'s `try` (`:676-677`). A database error there reaches `main`, which prints `admission_operator_refused` and never prints the committed code. An interrupt during the up-to-20 s provider phase has the same effect. This fails closed: nobody knows the code, and the operator must find it with `list` and revoke it. **Fix:** wrap the call, and say in the README that an interrupted `issue` requires list-and-revoke.
- **m6. The sweep can destroy a capability while it is being written.** `write_capability` creates the final name and then writes into it (`provider_management.py:391-397`). A concurrent `reconcile` reads the empty or partial file as unparseable (`capability_expiry` returns `None`, `:408-421`) and unlinks it (`admission_operator.py:769`). `create_remote` then records `capability_stored: true`. The window is narrow and fails closed. **Fix:** write to a temporary name and `link()` it into place, or make the sweep skip invitations whose create row is locked (`FOR UPDATE SKIP LOCKED`).
- **m7. Remote drift of a confirmed invitation is never noticed.** fAuth revokes earlier pending invitations for an address on any create, including the console's (`onboarding.rs:2600-2605`, `management.rs:1722-1730`). The background sweeper also expires them (`db/mod.rs:191-195`). `reconcile` inspects only `pending`/`unknown` creates, so a capability whose invitation is already dead upstream still looks usable. **Fix:** when listing anyway, compare confirmed creates of live codes against `status=pending` and destroy capabilities whose remote id is no longer pending.
- **m8. Provider operations write no audit event, and their rows are overwritten in place** (`:661-662`, `:707-708`, `:747-748`). See the recommendation below.

### Nits

- **n1. Two upstream behaviours are not exercised by the stand-in.** (a) gatewasm's create is not atomic: the revoke-earlier UPDATE and the INSERT are separate statements (`management.rs:1722-1747`), so a 500 between them revokes earlier pending invitations without creating one. The stand-in's `status` fault processes nothing. (b) Timestamps from PostgreSQL go through chrono's `to_rfc3339`, which writes 0, 3, 6 or 9 fraction digits; the stand-in always writes 9. GenUI handles both (500 is `unknown`, and `parse_time` accepts 0–9 digits), but nothing exercises them.
- **n2. A misconfigured `GENUI_PROVIDER_URL` receives the key.** The tenant-wide management key goes out in the Authorization header of `GET /manage/config` before the tenant check (`provider_management.py:205`, `:279`). A wrong HTTPS host would therefore receive it. Consider a pinned host allowlist for the operator.
- **n3. `outcome_unrecorded` can report a capability that is not usable.** It reports `capability_stored` by `lexists` (`admission_operator.py:671-672`), which is true for a partial file.
- **n4. README risk 1 (suspended process) can be narrowed cheaply.** I agree it is residual. `create_remote` could refuse to send once the monotonic time since COMMIT exceeds grace/2.
- **n5. A 404 for an accepted invitation is stored as revocation `confirmed`.** The detail `not_pending` is printed but not persisted, so the ledger cannot tell "revoked" from "identity already created" (README risk 2).
- **n6. The address guard and `known` are per organization.** The one-tenant-per-organization invariant they rely on is enforced by the runtime registry (`app/src/storage/registry.rs:55`). `provider_tenant_bindings` does not enforce it (the PK includes `organization_id`, `0002_tenant_foundation.sql:25-30`), and the operator path never checks it.
- **n7. `create_remote` locks out of D7.1 order.** It locks the `provider_invitations` row before the organization (`:646-647`, then `lock_code` at `:667`). I found no deadlock cycle, because no writer holds a code and then waits on a create row. Still, it is outside D7.1's stated order, and nothing documents why.
- **n8. Check 24 has no positive control with a stand-in-created identity.** All three provider identities are refused. The cause is pinned anyway, because `denied_for_reconciliation` is written only on the expired, revoked and exhausted branches (`0003_admission.sql:368-389`). A fourth invited, live code admitted through the same path would remove any doubt about the identity's shape.

## README risks judged

1. **A suspended operator process can orphan a remote invitation.** Confirmed by reading and accepted as a residual. The orphan has no token holder after the next sweep, and it expires within 72 h. See n4.
2. **Provider operations write no audit events.** Consistent with D7.8 as written, but I recommend an amendment (below).
3. **A create confirmed by reconcile has no stored token.** Confirmed. The remaining gap is m2: such a create should also be revoked.

## Audit recommendation (ADR-007 D7.8)

**Yes, amend D7.8 before the first live invitation (APP-076).** The amendment adds operator kinds for provider-operation transitions.

- **The row is not a record.** Every other operator-channel change with an authority effect is chained, and `backfill` is exempt only because genesis heads are immutable (D7.19). `provider_invitations` rows, by contrast, are overwritten. The history is lost: pending → unknown → failed/confirmed, which remote id reconcile chose, and whether a revocation hit 200 or 404 (accepted).
- **The decisions need review.** M1 shows that reconcile verdicts can be wrong, so they need a tamper-evident trail.
- **fAuth's own audit does not cover this.** It records each management-key use by path (`management_auth.rs:52-58,175-186`): that GenUI called, not what GenUI concluded.
- **Suggested shape:** three operator kinds, `provider.invitation.created`, `provider.invitation.reconciled` and `provider.invitation.revoked` (one generic kind would also do). Each event carries `code_id`, `record_id = invitation_id` and `outcome_digest` over the canonical `{operation, state_before, state, remote_invitation_id, detail}`. The columns already exist.
- **Cost:** the operator RESTRICTIVE policy enumerates kinds (`0003_admission.sql:165-167`), so the change needs a migration. It can ride the next one; no table changes.

This is not a blocker for the local slice.

## Acceptance items

- **Item 6 — supported with limit.**
  - Holds:
    - An intended email and an explicit least-privilege role are required, and omission or widening is refused (`admission_operator.py:226-235`, `PROVIDER_ROLES = ("user",)`).
    - The create body is exactly `{email, role:"user", delivery_mode:"server_only"}`.
    - The remote id is recorded.
    - The key comes only from a private file or the environment, never argv, and is checked against the bound, invite-only tenant before any lock or write.
    - Application roles reach neither a write nor the provider.
    - The runtime image admits only `runtime/runtime.py` (`app/.dockerignore:15-17`).
    - A 201 with `invite_url`, delivery other than `suppressed`, another role or another address is never stored and is revoked at once.
  - Limits:
    - The capability-protection clause holds as access control only, pending M2.
    - The transport is simulated.
    - The capability is operator-side; the server handoff is APP-074/076's.
- **Item 7 — supported with limit.**
  - Holds:
    - Unknown and pending outcomes stay so and are judged only by list inspection.
    - `Client.create` has one call site, and reconcile never creates.
    - Every fault in the tests sent exactly one create.
    - Expiry and revocation block handoff immediately through the code status, and revocation rows propagate with pending/confirmed state.
    - No path writes `access_codes.status` back toward `active`, and my probes did not restore access either.
    - Check 24, which I re-ran, drives `Workspace::{reserve_code, record_pending_admission, admit}`: identities created through the stand-in gain no principal or entitlement once their code is revoked, expired or exhausted.
  - Limits:
    - M1: reconciliation can record false `confirmed`/`failed` states.
    - m1–m3.
    - Propagation happens only when an operator runs `reconcile`; there is no schedule.
    - The transport is simulated.

## Claims I tried to refute and could not

1. **No token leak.** The `invite_token` never reaches PostgreSQL, stdout or stderr, `list`, the campaign report or the evidence. I checked the code paths, the slice's dump and stream tests, my campaign report (0 hits) and an evidence grep.
2. **The management key never appears in argv, `repr` or anything GenUI writes.** The parser has no key option, and its usage refusal echoes nothing. A key file and a key variable set together are refused. The key file must be absolute, regular, not a link, owned by the operator, without group or other bits, and at most 1 KiB.
3. **The capability file has no permissive window.** It is created with `O_CREAT|O_EXCL|O_NOFOLLOW` and mode 0600, then `fchmod` 0600; umask can only tighten it. The directory must be owned by the operator, not a link, and without group or other bits.
4. **No retries and no blind duplicates.** Each request is sent once, and the deadline is enforced by a socket-closing watchdog. A second `issue` for an address with a live create is refused, serialized by the binding row lock. Contract-breaking 201s and lost responses are never repeated.
5. **The status classification matches gatewasm at `1bf5d3b`.**
   - Every create 400 precedes the first write (`management.rs:1684-1712` versus `:1722`).
   - 401 and 403 come from the middleware before the handler (`management_auth.rs:32-113`).
   - Revoke returns 404 `invite_not_found` exactly when no pending row matched (`:1809-1826`).
   - The built-in user role's `role_name` is `user` in both create and list (`management.rs:333-335`, `001_schema.sql:766`).
   - Invitation and token are both `tinv_<uuid v4>` (`:1718-1719`).
   - The list is `created_at DESC` (`:1609`).
   - Invitations are never deleted, only cascaded with their tenant (`001_schema.sql:618-619`), so offset paging can repeat a row but not skip one. Only `created_at` ties at microsecond precision could reorder rows.
6. **The stand-in faithfully reproduces gatewasm's statuses, bodies, headers and ordering on every path the tests use**, apart from n1.
7. **Revocation is local first.** The local revocation commits before any provider call, and a failed or unknown remote revocation leaves the code `revoked`: `reserve_code` answers `revoked` throughout.
8. **Check 24 really goes through the storage path.** Its refusal cause is pinned by `denied_for_reconciliation`, and the exhausted case is refused at `record_pending_admission`.
