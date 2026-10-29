# Phase 1 Finish-Line Execution Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Every task is one board ticket slice: move the ticket on the board at the start and end of the task (`Tickets/README.md`), keep an evidence directory, and obtain an independent review before Done.

Prepared 2026-10-29 by the GenUI owner (Claude) after reading every remaining PH01 ticket, the code, the design and both team handoffs. Status: plan, not evidence. Nothing here is implemented, deployed or accepted. The orchestrator executing this plan is the GenUI owner and single GenUI operator; the FormaStack/fAuth lane is a separate Claude session (see [CROSS-TEAM-HANDOFF](CROSS-TEAM-HANDOFF.md), last sections).

**Goal:** Close PH01: all twenty tickets Done (APP-001–014, APP-073–078) and an accepted `evidence/phases/PH01/GATE.md`, proving invite → verified sign-in → current entitlement → authorized query → visible receipt → explicit command → atomic outcome/audit → restart/reopen on the managed synthetic environment.

**Architecture:** One Rust business binary (axum 0.8, forma-server 3.2.0 SSR pages) over four fixed-role PostgreSQL pools with transaction-local tenant scope, FORCE RLS and definer fence functions (ADR-004); a Python support process for credentials, migrations and the bounded query worker; fAuth as the identity provider behind GenUI's own opaque sessions (ADR-003, ADR-006); operator-only admission ledger (APP-073); managed RDS/ECS on AWS for the RDS window and the beta go-live trial (APP-078, APP-076). Local tickets prove behavior on real PostgreSQL 16.13 with TLS in Docker; hosted and managed acceptance live only in APP-076/078 in PH01, with APP-079/080 deferred (ADR-008).

**Tech Stack:** Rust 1.93.1 toolchain with `rust-version = "1.85"`; `forma-ir`/`forma-server` `=3.2.0`; axum 0.8; tokio-postgres 0.7.18 (`features = ["runtime"]` only), deadpool 0.13.1, rustls 0.23.46; serde `=1.0.229`, serde_json `=1.0.154`, ring `=0.17.14`, reqwest `=0.12.28`; Python 3.11 with psycopg 3.3.6 and boto3 1.43.121 hash-locked; `@getforma/build` 3.2.0 strict SSR; PostgreSQL 16.13 (`postgres@sha256:5d143123…`); AWS CDK 2.275.0.

**Spec:** [ADR-003](../architecture/ADR-003-PUBLIC-BETA-ACCESS.md), [ADR-004](../architecture/ADR-004-TENANT-TRANSACTIONS.md), [ADR-005](../architecture/ADR-005-PHASE-1-SEQUENCING.md), [ADR-006](../architecture/ADR-006-SIGNIN-AND-VERIFICATION-PENDING.md), the PH01 tickets under `Tickets/`, the platform [design](../../design/forma-generative-app-platform-v2/design.md) §§6–10, [SHARED-CONTRACTS](../../design/forma-generative-app-platform-v2/SHARED-CONTRACTS.md), the demo [scenarios](../../design/forma-northstar-demo/SCENARIOS.md) S05/S06, the FormaStack [identity contract](../../evidence/APP-006/upstream-auth-source/GENUI-IDENTITY-CONTRACT-2026-10-28.md) and the [managed plan](../../evidence/APP-008/managed-plan.md).

## Global constraints

Copied from the specs; every task inherits them.

- Toolchain pins: `cargo +1.93.1`, compiler floor `cargo +1.85.0 check --locked --lib --tests`, Node 22.21.0 / npm 11.6.2, Python 3.11 venvs at `app/runtime/.venv` and `contracts/.venv`. `CARGO_BUILD_JOBS=2`; aggregate Cargo jobs on this Mac stay ≤3 and no heavyweight build runs concurrently with the FormaStack session's builds.
- Every dependency is an exact pinned release in `app/Cargo.toml`/`Cargo.lock` or `requirements.lock` (`--require-hashes`); new dependencies are named in the owning ticket log with registry checksum and license.
- The root `.dockerignore` is an exact-file allowlist: every new Rust, TSX, CSS, SQL or catalog file that the image needs is added there in the same commit, and a context export is retained in evidence.
- Schema version is pinned in eight places that move together or every pool refuses admission: the migration file, `app/migrations/manifest.json`, `app/migrations/catalog-vN.json`, `app/src/storage/catalog.rs` (`EXPECTED` include), `app/src/storage/pool.rs` (`verify_schema_version`), `app/runtime/runtime.py` (`DATABASE_SCHEMA_VERSION`, manifest checks, history `CHECK`), `app/runtime/qualify_tenant_schema.py` expectations and `app/src/storage/managed_qualification.rs` pins. The canonical catalog must stay under 262,144 bytes (v2 is 123,415).
- Storage bounds are fixed: 2 connections per role, 8 total, 5 s acquisition, 3 s transaction, 2 s cleanup, `statement_timeout 2000`, `lock_timeout 1000`, credential documents ≤600 s. Nothing retries a mutation automatically.
- Design values: workspace session 12 h absolute / 1 h idle, pre-login state 10 min, `__Host-` HttpOnly Secure SameSite=Lax cookies; idempotency key ≥128 bits CSPRNG scoped to (organization, principal, work, command, key), outcome checked before expected version, outcomes retained 24 h, same key with different canonical payload → 409; error codes 404 uniform denial, 403 `cross_site`, 409 conflict with recovery view, 412 stale, 422 `narrow_the_query`, 429 with `Retry-After`, 503 only for provider/store unavailability; admission limits 24 views, 8 operations per view, 256 KiB definition, 500 rows per page; query deadline 1.8 s, DuckDB 2 threads / 1 GiB, read-only, no extensions or httpfs; one JWKS refetch per minute, fail closed; totals cover the full authorized input and a failed calculation is an error, never a zero.
- Fixture semantics stay exact: Northstar 1,140,000 / 900,000 / 114,000, ops-runner 210,000 / 30,000 / 21,000, Spend Review 64,000 cents total and 45,000 cents at `amount_cents >= 20,000`; S05 versions 0→1→2 with ik-1/ik-2 and stale ik-3 → 409.
- Identity: literal issuer `gatewasm`, audience `gatewasm-app`, access 900 s, access-only `/auth/jwks`; `sub` equals `user.id`; `user.email_verified` is backend metadata; provider identifiers `ten_`/`usr_` map through the closed registry (≤32 bindings, restart to replace) to GenUI organization UUIDs `…0001` (Northstar) and `…0002` (Canary); no fAuth token JSON or Set-Cookie ever reaches the browser.
- Operations: $100/month cap for the bounded qualification windows with the $5 auth allowance counted once (October headroom $2.51; the ledger 24-hour reconciliation rule applies); the always-on beta needs the new monthly figure ADR-008 asks Victor for; single GenUI operator; the repository is public under its all-rights-reserved notice until deployment is complete; no LLM call before APP-043; no real customer data; GenUI schema changes never overlap a fAuth deployment operation; observe STOPPED (or, for the beta, RUNNING on the recorded digest) before claiming the end state.
- Done requires independent review, exact-source CI and landing on `origin/main`; local checks are evidence, never a CI pass.

## Review focus

Failure modes the specs imply but no ticket's acceptance test names; each is pinned to the owning task below.

1. **A partial schema-version pin update** leaves every pool refusing admission (`Error::Unavailable`) in local and managed runs. Pinned to Task 3: a repository check asserts the eight pins agree.
2. **A new table without `ENABLE`/`FORCE ROW LEVEL SECURITY` and the `tenant_scope` policy** (the 0002 loop covers only 0002's tables) silently loses isolation. Pinned to Tasks 4, 6, 7, 10: `qualify_tenant_schema.py` asserts every `genui` table has rls+force+policy and organization-leading keys.
3. **The canonical catalog exceeding 262,144 bytes** returns `NULL` and refuses every connection. Pinned to Task 3: the generator fails above 80 % of the cap.
4. **Lock contention surfacing as `Unavailable` after `lock_timeout` 1000 ms** instead of a typed conflict, mapped to the wrong HTTP status. Pinned to Tasks 10–11: race tests assert the typed `CommandResult`/HTTP mapping under a held writer lock.
5. **Mounting a production `/auth/callback` beside the operator harness** makes axum refuse startup whenever the qualification file is present. Pinned to Task 7: startup refuses `GENUI_AUTH_QUALIFICATION_CONFIG_FILE` in release builds and a test proves the harness is absent from the release router.

## Decisions the implementer cannot make alone (ADR-007 controls)

[ADR-007](../architecture/ADR-007-PHASE-1-WRITE-AUTHORITY.md) records them as D7.1–D7.19, revised on 2026-10-29 after its [independent review](../../evidence/APP-073/adr-007-review.md) (2 blockers, 10 majors, 13 minors, each dispositioned in the ADR's last table). The restatement that used to live here drifted from the ADR (review M10) and is removed: implementers read the ADR, and where any task below restates a name, signature, grant or lock order, the ADR controls. The items most tasks consume: D7.1 the six `genui_admit` definer functions (`resolve_code`, `resolve_handle`, `reserve_code`, `record_pending_admission`, `admit_redemption`, `advance_member_epoch`) and the operator-only revocation; D7.2 the two pre-scope indexes; D7.3 the grant table with its SELECT column; D7.6 the single 40001 retry (amendment A1); D7.7 sequence rows created by the operator; D7.8 head-row audit chains with enumerated kinds; D7.9 `Admission`/`CommandResult`/`PublishResult`, the `command_claims` claim row, the command and session lock orders and the 409 `busy`/`in_flight` mapping; D7.10 the confined query child with its five controls; D7.12 the baseline `BETWEEN 1 AND 2` ledger, per-migration CHECK ALTERs, the one-time cap move to 524,288 and the `genui_admit` catalog normalization (A3); D7.14 the 256 KiB command body; D7.17 the admission arithmetic; D7.18 stop-then-start deployment; D7.19 the operator channel.

## Operating rules for the orchestrator

- Work one ticket slice at a time per set of files. Create a worktree per ticket from `phase-one-spikes` (`git worktree add ../forma-app-platform-APP-0NN phase-one-spikes`); merge back with a merge commit on the exact reviewed head.
- Board moves: Selected → In progress → In review → Done with dated `## Log` entries; run `python3 Tickets/_tools/board.py render && python3 Tickets/_tools/board.py check` after every move. A ticket may be Selected only when `board.py next` lists it.
- Each slice ends with a fresh independent reviewer (REVIEW.md) who did not implement it; findings and fixes are retained under `evidence/APP-0NN/`.
- Local checks per slice: `cargo +1.93.1 fmt --check`, `cargo +1.93.1 clippy --locked --lib --tests -- -D warnings`, `cargo +1.93.1 test --locked --lib storage:: -- --test-threads=1`, `cargo +1.85.0 check --locked --lib --tests`, the Docker campaign `GENUI_NATIVE_REPORT=<abs path> cargo +1.93.1 test --locked --lib storage::qualification::native_postgres_campaign -- --ignored --exact --nocapture`, `app/runtime/.venv/bin/python -m unittest discover app/runtime`, `app/runtime/.venv/bin/python app/runtime/qualify_tenant_schema.py --output <json>`, `python3 scripts/check-foundation.py`, and `python3 Tickets/_tools/board.py check`. Record exact commands and results; never claim CI.
- Commit at every green checkpoint; do not push while GitHub billing is blocked. When billing clears, push once, let the ten workflows run, fix forward, land.
- Cloud: read-only observation is free; any mutation (secret, upload, change set, task, activation) happens only inside a Victor-approved window with a reserved ledger entry, coordinated with the FormaStack owner, and is recorded with request identity and bounded timeouts.

## Sequencing

```
Victor: GitHub billing ─┐
                        ▼
T1 land APP-008 + APP-011 (CI, main) ──► APP-073/APP-010 Selected
T2 ADR-007 ──► T3 runner N-versions ──► T4 0003 ──► T5 APP-073 ──► T7 APP-009 ──► T8 APP-074 local
                                     └─► T6 APP-010 ─────────────────────────────► T9 APP-012 ──► T10 APP-013 ──► T11 APP-014
Track B: APP-075 (B1 entry tests → B2 local harness → B3 image/supervisor) ∥ APP-077 (B4 operator package; B5 coordinator deferred to APP-080) → APP-078 (B6 RDS window, ADR-008 rows → B7 review)
Track C (beta):    C0 merged contract bound (done) → C1 Victor's figure + mail transport → C2 FormaStack lean fAuth + mail (FORMA-366) → C3 GenUI deploy (APP-078 window, service up, codes)
Track D:           APP-076 beta go-live trial in Chrome (needs T11, C3, FORMA-366) → PH01 GATE.md (names APP-079/080 deferrals)
```

---

## Track A — local implementation

### Task 1: Land APP-008 and APP-011 once Actions run

**Files:**
- Modify: `Tickets/4-done/APP-008.md`, `Tickets/4-done/APP-011.md` (log, move to Done), `evidence/APP-008/ci/<sha>.json`, `evidence/APP-011/ci.json`

**Interfaces:**
- Consumes: Victor's billing fix (the refused annotation disappears from a planning-run retry).
- Produces: `origin/main` at the reviewed head; APP-008 and APP-011 Done, which makes `board.py next` list APP-073 and APP-010.

- [ ] **Step 0: Decide the draft's fate before the exact-source push** — run `cargo +1.93.1 clippy --locked --all-targets -- -D warnings` and `cargo +1.85.0 check --locked --lib --tests` on the tip; if either fails on `app/src/storage/managed_qualification.rs`, revert `4ac7639` onto an `app-075` branch so the pushed tip carries only reviewed code; record the chosen Done revision in APP-008's Branch/PR field.
- [ ] **Step 1: Confirm Actions run with one cheap workflow** — after the repository is public, trigger only `planning.yml` (`gh workflow run planning.yml --ref phase-one-spikes`) and read `gh run view <id> --json jobs`; expected: `steps` > 0 and conclusion success. If still refused, stop and report (an account payment lock may also block public repositories); do not retrigger.
- [ ] **Step 2: Push `phase-one-spikes`** (`git push origin phase-one-spikes`) and watch the ten workflows; record each run id and conclusion in `evidence/APP-008/ci/<HEAD-sha>.json`.
- [ ] **Step 3: Fix forward any failure** in its own commit (format, clippy, the ignored native campaign on Linux, fixtures reset trial); rerun; never edit historical receipts.
- [ ] **Step 4: Land** — open a PR to `main`, obtain the independent reviewer's note, merge with a merge commit, record the merge sha in both tickets, move APP-008 and APP-011 to `4-done/`, repair incoming links, render and check the board, commit.

### Task 2: Record ADR-007 (write authority, schema layout, query execution)

**Files:**
- Create: `docs/architecture/ADR-007-PHASE-1-WRITE-AUTHORITY.md`
- Create: `evidence/APP-073/adr-007-review.md`
- Modify: `docs/architecture/DECISIONS.md` (one sentence pointing at ADR-007)

**Interfaces:**
- Consumes: decisions D7.1–D7.19 in the ADR, ADR-004 request/operation states.
- Produces: the reviewed names every later task uses: `genui_admit`, `resolve_code`, `admit_redemption`, `advance_member_epoch`, `organization_sequences`, `audit_events`, `CommandResult`, `Admission`, `begin_serializable`, `runtime.py query`.

- [x] **Step 1: Write ADR-007** containing D7.1–D7.17 verbatim with their rationale and the table of grants per role; state explicitly what ADR-004 text each item extends and that none of ADR-004's bounds change.
- [x] **Step 2: Independent review** by a fresh reviewer against ADR-003/004 and design §§6.1–6.3, 9; record findings and fixes in `evidence/APP-073/adr-007-review.md`.
- [x] **Step 3: Commit** (`docs: record ADR-007 phase 1 write authority`).

Done 2026-10-29: ADR-007 drafted (`cff5d6f`), independently reviewed (`evidence/APP-073/adr-007-review.md`: 2 blockers, 10 majors, 13 minors) and revised the same day with every finding dispositioned in the ADR's last table; the plan's restatement was replaced by a pointer and Tasks 3–11 aligned (review M10/m13).

### Task 3: Generalize the migration runner and catalog tooling to N versions

**Files:**
- Modify: `app/runtime/runtime.py` (`DATABASE_SCHEMA_VERSION`, `migration_sources`, `catalog_resources`, `initialize_migration_history`, `apply_migrations`, `probe_database`), `app/runtime/generate_schema_catalog.py`, `app/runtime/test_runtime.py`, `app/runtime/qualify_tenant_schema.py` (table-count and version expectations become derived from the manifest), `fixtures/reset_trial.py` (derive the expected relation list from `catalog-vN.json` instead of the hardcoded 26, version the fixture edition `app011-fixtures-v2` with its pins)
- Create: `scripts/check-schema-pins.py`

**Interfaces:**
- Consumes: `manifest.json` shape `{schema_version:1, database_version:N, migrations:[{file, sha256}], catalog:[{file, sha256}]}`.
- Produces: `migration_sources(directory) -> list[(version, sha256, sql)]` for N files named `000N_*.sql`; `catalog_resources(directory) -> (query, {1..N: expected})`; `apply_migrations` upgrading from any recorded version `k < N` by applying `k+1..N` in one transaction with per-step catalog proof; `check-schema-pins.py` exiting non-zero unless manifest `database_version`, `runtime.py`, `pool.rs`, `catalog.rs` include, and `managed_qualification.rs` pins agree.

- [x] **Step 1: Write failing tests** in `test_runtime.py`: `test_migration_sources_accepts_three_pinned_files`, `test_apply_migrations_upgrades_v2_to_v3_and_repeat_is_noop`, `test_new_ledger_starts_at_version_two_and_every_step_is_proven`, `test_generator_refuses_snapshot_over_80_percent_cap`; and `scripts/test_check_schema_pins.py` asserting a mismatched `pool.rs` constant fails.
- [x] **Step 2: Run** `app/runtime/.venv/bin/python -m unittest discover app/runtime -v`; expected: the new tests FAIL (manifest with three entries refused).
- [x] **Step 3: Implement** the generalization (keep exact-pin checks, the frozen 0001 hash, the advisory lock `(1819409017, 2)`, origin-mode and search-path rules); the `has_history` branch reads the recorded max version and applies the remaining migrations, verifying `catalog-v{k}` after each; `initialize_migration_history` keeps the baseline `CHECK (version BETWEEN 1 AND 2)` on both CHECKs and every later migration ALTERs them to its own N (D7.12), so the per-step catalog proof runs on the fresh path too; the generator snapshots after every migration and refuses a snapshot above 209,715 bytes.
- [x] **Step 4: Run** the unit tests and `qualify_tenant_schema.py` against the unchanged two-migration manifest; expected: all pass with identical v1/v2 bytes (no catalog change yet).
- [x] **Step 5: Commit** (`feat(runtime): migration runner and catalog tooling for N schema versions`).

Done 2026-10-29 in the implementer's worktree and merged (`dc3900d`, `26c1622`): 38 runtime tests, 3 pin-check tests and 14 fixture tests pass; Docker catalog generation reproduces `catalog-v1`/`v2` byte-identically; the old and new runners issue identical statements on the two-migration manifest. Independent review pending (recorded in `evidence/APP-073/`); the cap move of D7.12 is 0003's.

### Task 4: Migration 0003 — admission ledger, entitlements, audit, definer principal (APP-073 slice 1)

**Files:**
- Create: `app/migrations/0003_admission.sql`, `app/migrations/catalog-v3.json` (generated), `evidence/APP-073/schema/README.md`
- Modify: `app/migrations/manifest.json`, `app/src/storage/catalog.rs:8-10`, `app/src/storage/pool.rs:486-498`, `app/src/storage/managed_qualification.rs` (`expected_pins`), `app/runtime/qualify_tenant_schema.py`, `.dockerignore`

**Interfaces:**
- Consumes: ADR-007 names; domains `genui.safe_counter`, `genui.digest`, `genui.local_id`; `tenant_scope` policy text from 0002:219.
- Produces tables (all PK organization-leading, FORCE RLS, `tenant_scope`): `genui.access_codes(organization_id, code_id, code_digest UNIQUE(organization_id, code_digest), kind CHECK IN ('trial','member_invite'), status CHECK IN ('active','revoked','exhausted','expired'), destination_organization_id = organization_id CHECK, max_redemptions safe_counter CHECK BETWEEN 1 AND 10, redeemed_count bigint, issued_at, expires_at CHECK <= issued_at + 30 days, trial_days integer, issued_by text, revoked_at)` (the grant template lives in `code_grant_templates`); `genui.code_reservations(organization_id, reservation_id, code_id, kind CHECK IN ('presignup','pending'), context_digest, issuer NULL, provider_tenant_id NULL, provider_subject NULL, state CHECK IN ('live','consumed','expired','denied_for_reconciliation'), expires_at, consumed_at)` (D7.1/D7.17: pre-signup rows never count, at most 10 live per code; pending rows count); `genui.code_grant_templates(organization_id, code_id, resource_id, permission)` with explicit FK-checked pairs refusing `admin_policy` and `review_publish`; `genui.code_redemptions(organization_id, code_id, principal_id, redemption_id UNIQUE, payload_digest, committed_at, audit_event_id)`; `genui.entitlements(organization_id, principal_id FK memberships, entitlement_id UNIQUE, status CHECK IN ('active','expired','revoked'), kind, starts_at, expires_at, revoked_at)`; `genui.provider_invitations(organization_id, invitation_id, issuer, provider_tenant_id FK provider_tenant_bindings, remote_invitation_id NULL, intended_email_digest, provider_role NOT NULL, delivery_mode CHECK ('server_only'), operation CHECK IN ('create','revoke'), state CHECK IN ('pending','confirmed','failed','unknown'), code_id FK, created_at, updated_at)`; `genui.audit_events` and `genui.audit_heads` with the RESTRICTIVE per-role policies (D7.8); `genui_private.code_index` and `genui_private.handle_index` (D7.2); role `genui_admit`, its six functions and its grants including the lock-enabling columns (D7.1, A4); the A3 catalog changes (`genui_admit` in every fixed-role list of `catalog-query.json` and the qualifier's normalization, `GRANT genui_admit TO genui_migration WITH INHERIT FALSE, SET TRUE`, the temporary `CREATE ON SCHEMA genui_private` and its revocation) and the one-time cap move to 524,288 / 419,430 (D7.12); `ALTER TABLE genui_meta.schema_version/schema_migrations` CHECKs to `BETWEEN 1 AND 3`. `organization_sequences`, `command_claims` and `command_outcomes` are 0007's (Task 10); the backfill of sequence and head rows for the two existing organizations is the operator package's (D7.7, D7.19).

- [ ] **Step 1: Write failing schema tests** in `qualify_tenant_schema.py` (new cases): `admission_tables_have_rls_force_policy_and_org_leading_keys`, `code_index_has_no_rls_and_only_definer_reads`, `app_roles_cannot_write_admission_tables_directly`, `workspace_can_execute_admit_and_resolve_only`, `admit_redemption_consumes_exactly_one_slot_under_race` (two sessions racing the last slot via `pg_stat_activity` wait barrier), `admit_redemption_refuses_expired_revoked_exhausted_foreign`, `advance_member_epoch_locks_org_then_member_and_writes_one_audit_event`, `audit_chain_hashes_link_per_org_per_role`.
- [ ] **Step 2: Run** `qualify_tenant_schema.py`; expected: FAIL (relations absent).
- [ ] **Step 3: Write 0003**, regenerate catalogs with `generate_schema_catalog.py --output-dir <new>` (fresh install path and v2→v3 upgrade path must produce byte-identical `catalog-v3.json`; the test from Task 3 proves it), update `manifest.json` (database_version 3, new file hashes), `catalog.rs` include, `pool.rs` version 3, `managed_qualification.rs` pins, `.dockerignore` (0003 file, catalog-v3.json).
- [ ] **Step 4: Run** the schema campaign, `cargo test --lib storage::`, the native Docker campaign and `scripts/check-schema-pins.py`; expected: all pass, catalog-v3 size recorded (< 209,715 bytes).
- [ ] **Step 5: Independent review** of the SQL (grant table, definer bodies, lock order, audit chain) → `evidence/APP-073/schema/independent-review.md`; fix; commit (`feat(schema): migration 0003 admission ledger and definer principal (APP-073)`).

### Task 5: APP-073 — admission operations, entitlement checks, operator command

**Files:**
- Create: `app/src/storage/admission.rs`, `app/runtime/admission_operator.py`, `app/runtime/test_admission_operator.py`, `evidence/APP-073/README.md`
- Modify: `app/src/storage/mod.rs` (declare module, re-export `Admission`, `VerifiedNonmember`), `app/src/storage/authorization.rs:144-189` (entitlement join in `precheck`, recheck in `fence`), `app/src/storage/qualification_cases.rs`, `app/src/storage/qualification.rs` (campaign), `app/runtime/tenant_seed.py` (seed one active trial code and one expired code per fixture organization, digests only)

**Interfaces:**
- Consumes: `genui_private.resolve_code`, `admit_redemption`, `advance_member_epoch`; `Transaction::begin`; `storage::Error`.
- Produces: `pub struct VerifiedNonmember { pub(super) issuer: String, pub(super) provider_tenant: String, pub(super) provider_subject: String, pub(super) registry_sha256: String }` (constructed only inside `storage`; APP-009's session adapter fills it); `pub enum Admission { Admitted { principal: String, epoch: i64, entitlement: String }, Pending { reason: PendingReason }, AlreadyMember, Refused }`; `impl Workspace { pub async fn resolve_code(&self, code_digest: &str) -> Result<Option<String>>; pub async fn resolve_handle(&self, handle_digest: &str) -> Result<Option<String>>; pub async fn reserve_code(&self, organization: &str, code_digest: &str, context_digest: &str) -> Result<Reservation /* typed outcome per D7.1 */>; pub async fn record_pending_admission(&self, identity: &VerifiedNonmember, organization: &str, reservation: &str) -> Result<PendingAdmission /* typed outcome */>; pub async fn admit(&self, identity: &VerifiedNonmember, organization: &str, pending_admission: &str, payload_digest: &str) -> Result<Admission> /* callers invoke it only after an exchange reporting email_verified true */; pub async fn advance_epoch(&self, request: &TrustedRequest, reason: &str) -> Result<i64> }`; `precheck` additionally requires an `entitlements` row with `status='active' AND expires_at > now()` for the principal, and `fence` re-reads it after `lock_member_authority`; operator CLI `python3 app/runtime/admission_operator.py {issue,list,revoke,extend} --organization <uuid> …` running as `genui_migration` → `SET LOCAL ROLE genui_owner`, printing the raw code exactly once to stdout and nothing to logs.

- [ ] **Step 1: Write failing Rust tests** (in `qualification_cases.rs`/`qualification.rs`, Docker fixture): `admission_resolves_only_known_digests_without_scope`, `admission_commits_membership_entitlement_audit_atomically`, `identical_retry_returns_stored_outcome_without_second_slot`, `changed_payload_or_principal_refuses_outcome`, `last_slot_race_admits_exactly_one` (two `admit` calls with `TestHooks` phase `admit`), `redemption_vs_code_revocation_both_orders`, `entitlement_use_vs_revocation_both_orders`, `expired_entitlement_denies_precheck_fence_and_delivery`, `forged_operator_identity_cannot_issue` (an app role calling the operator SQL path is refused).
- [ ] **Step 2: Run** the campaign; expected: FAIL (`Workspace::admit` undefined).
- [ ] **Step 3: Implement** `admission.rs` (each operation is one `Transaction` on the workspace pool: `begin` with a nonmember variant that installs organization scope and no principal for the definer calls, `resolve_code`/`resolve_handle` before scope, `reserve_code`, `record_pending_admission` and `admit_redemption` inside, `finish(commit)`), extend `precheck`/`fence` SQL, add `TestHooks` phase `admit`, write `admission_operator.py` (issue: 32 random bytes → base32 secret shown once, digest `sha256:` stored, `code_index` + `access_codes` + `code_grant_templates` + optional `provider_invitations(pending)` in one transaction; list: identifiers/status/counts only; revoke and extend as operator-channel transactions in D7.1's lock order writing the `operator` chain; `provision` creating the organization with its `organization_sequences` row, five `audit_heads` rows and `handle_index` row, and `backfill` for the two existing organizations per D7.7), and its unit tests with a disposable PostgreSQL container.
- [ ] **Step 4: Run** all local checks; expected: pass; record the configured defaults (D7.17) in `evidence/APP-073/README.md` before issuing any code.
- [ ] **Step 5: Independent review** (authorization, concurrency, disclosure, operator channel) → `evidence/APP-073/independent-review.md`; fix; move APP-073 to In review; commit (`feat(storage): admission ledger operations and operator command (APP-073)`).

### Task 6: APP-010 — persist and admit typed definitions and work bindings

**Files:**
- Create: `app/migrations/0005_definition_work_grants.sql`, `app/migrations/catalog-v5.json`, `app/src/admission/mod.rs`, `app/src/admission/canonical.rs`, `app/src/admission/digest.rs`, `app/src/admission/validate.rs`, `app/src/admission/tests.rs`, `app/src/storage/definitions.rs`, `evidence/APP-010/README.md`
- Modify: `app/src/lib.rs` (`pub mod admission;`), `app/src/storage/mod.rs`, `app/src/storage/authorization.rs` (`Transaction::begin_serializable`), pins (eight places), `.dockerignore`, `contracts/README.md` (note the Rust implementation and the vector check command)

**Interfaces:**
- Consumes: `contracts/schema.json` `$defs` (app_definition, input_contract, record_schema, publication, saved_work, work_binding), `contracts/implementation-lock.json` vectors, `contracts/fixtures/*.json`, grants from 0005 (D7.3), `Permit`.
- Produces: `admission::canonical::encode(value: &serde_json::Value) -> Result<Vec<u8>, AdmissionError>` (forma-json-v1: sorted keys, restricted scalars, depth ≤32, ≤1 MiB); `admission::digest::digest(domain: &str, canonical: &[u8]) -> String` (`sha256:` + hex of SHA-256(domain ‖ 0x00 ‖ bytes), domains `forma:app-definition:v1`, `forma:input-contract:v1`, `forma:record-schema:v1`, `forma:query:v1`, `forma:query-receipt:v1`, `forma:command-payload:v1`, `forma:source-binding:v1`, `forma:result:v1`); `admission::validate::admit_definition(bytes: &[u8]) -> Result<AdmittedDefinition, AdmissionError>` with `pub enum AdmissionError { UnknownField(String), ExecutableContent(String), TooManyViews(usize), TooManyOperations(usize), TooLarge(usize), UnregisteredRenderer(String), UnregisteredOperator(String), ClientGrant(String), Encoding(String) }` and limits 24 views, 8 operations per view, 256 KiB, 500 rows; `impl Workspace { pub async fn create_definition_revision(&self, request: &TrustedRequest, app: &str, admitted: &AdmittedDefinition) -> Result<DefinitionRevision>; pub async fn create_work(&self, request: &TrustedRequest, app: &str, publication: &str, bindings: &[WorkBinding]) -> Result<SavedWork>; pub async fn read_work(&self, request: &TrustedRequest, app: &str, work: &str) -> Result<SavedWorkDetail> }`; `impl Control { pub async fn publish(&self, request: &TrustedRequest, app: &str, revision: &str, policy: &str) -> Result<PublishResult> }` (SERIALIZABLE, one retry on 40001 per D7.6, `Stale` → 412); readers return canonical bytes bounded 262,144.

- [ ] **Step 1: Write failing tests**: `admission::tests::canonical_vectors_match_contracts_lock` (every vector in `implementation-lock.json`), `rejects_unknown_fields_executable_strings_and_client_grants_with_coded_reason`, `rejects_over_complex_definitions` (25 views, 9 operations, 256 KiB+1), `unregistered_renderer_or_operator_version_refuses`; storage campaign: `definition_publication_and_work_persist_and_reload_after_store_restart` (close `Stores`, build new `Stores`, read back digests), `publication_contains_input_contracts_not_author_bindings`, `workspace_cannot_write_publications_control_cannot_write_saved_work`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement** 0005 (grants per D7.3, no new tables; the `owner_publish` policy rows for both organizations are seeded by the operator package under scope, not by the migration), regenerate `catalog-v5.json`, move the pins to 5 (`scripts/check-schema-pins.py`), implement `admission` and `definitions.rs`, add `begin_serializable`. Create the Spend Review app, publication and saved work in the Northstar organization through this admitted path as the S05 fixture for Task 10: the seeded Spend Review rows exist only in the canary organization as a foreign-key negative, and every seeded `admission_digest`/`normalized_digest` is an APP-002 placeholder, never an admitted definition.
- [ ] **Step 4: Run** all local checks plus `python3 contracts/validate.py` cross-check that Rust and Python digests agree for all fixtures; expected: pass.
- [ ] **Step 5: Independent review** → `evidence/APP-010/independent-review.md`; move APP-010 to In review; commit (`feat: typed definition admission and persisted work bindings (APP-010)`).

### Task 7: APP-009 — sessions, sign-in, callback, pending verification, sign-out

**Files:**
- Create: `app/migrations/0006_sessions.sql`, `app/migrations/catalog-v6.json`, `app/src/auth/mod.rs`, `app/src/auth/capability.rs`, `app/src/auth/cookies.rs`, `app/src/auth/csrf.rs`, `app/src/auth/pkce.rs`, `app/src/auth/provider.rs`, `app/src/auth/routes.rs`, `app/src/auth/pending.rs`, `app/src/auth/tests.rs`, `app/src/auth/synthetic_issuer.rs` (cfg(test): loopback Ed25519 issuer reused from `spikes/APP-006/src/tests.rs` patterns), `app/src/storage/sessions.rs`, `app/admin/src/signin/app.tsx`, `app/admin/src/pending/app.tsx`, `evidence/APP-009/README.md`
- Modify: `app/src/main.rs` (routes per ADR-006; feature-gated harness per D7.13; `into_make_service_with_connect_info`), `app/Cargo.toml` (`[features] qualification-harness = []`), `app/src/auth_qualification.rs` (`#![cfg(feature = "qualification-harness")]`), `app/admin/build.ts`, pins, `.dockerignore`, `app/runtime/smoke_http.py` and `evidence/APP-006/hosted-callback/http_probe.py` expectations for the new routes

**Interfaces:**
- Consumes: `genui_auth_verifier::{Verifier, Config, ContractProfile::Forma248AccessOnly, VerifiedIdentity}`; `Workspace::{resolve_code, resolve_handle, reserve_code, record_pending_admission, admit, advance_epoch}`; `Registry::agrees`; ADR-006 routes, cookies and lifetimes; 0006 tables `genui.prelogin_flows(organization_id, flow_id, kind CHECK IN ('signin','signup'), state_digest, reservation_id NULL, code_verifier stored encrypted (no nonce: ADR-006), signup_handoff_id NULL, client_address_class, created_at, expires_at)`, `genui.pre_access_sessions(organization_id, session_id, issuer, provider_tenant_id, provider_subject, email_verified boolean, admission_flow_id, refresh_token_ciphertext bytea, created_at, last_seen_at, expires_at, revoked_at)`, `genui.sessions(organization_id, session_id, principal_id FK memberships, membership_epoch_at_mint, entitlement_id, registry_sha256, created_at, last_seen_at, absolute_expires_at, revoked_at)` with cookie values stored as SHA-256 digests.
- Produces: `impl Workspace { pub async fn begin_flow(&self, organization: &str, kind: FlowKind, handoff: Option<&str>) -> Result<FlowStart /* cookie value, state, code_challenge */>; pub async fn complete_flow(&self, cookie: &str, state: &str) -> Result<FlowRecord>; pub async fn mint_pre_access(&self, flow: &FlowRecord, identity: &VerifiedIdentity, email_verified: bool, refresh: &[u8]) -> Result<PreAccess>; pub async fn mint_session(&self, pre: &PreAccess, admitted: &AdmittedMember) -> Result<SessionCookie>; pub async fn resolve_session(&self, cookie: &str) -> Result<TrustedRequest>; pub async fn resolve_pre_access(&self, cookie: &str) -> Result<PreAccess>; pub async fn sign_out(&self, cookie: &str) -> Result<()>; pub async fn sign_out_all(&self, request: &TrustedRequest) -> Result<()> }`; `auth::routes::router(state) -> Router` mounting exactly the ADR-006 routes; `auth::csrf::require_same_origin(headers) -> Result<(), Problem>`; `auth::provider::Client { par, exchange, refresh, logout, resend }` bounded 1 s connect / 3 s total, `deny_unknown_fields` structs; `TrustedRequest` is constructed only in `sessions.rs` from the session row, the current `memberships.membership_epoch` and `stores.registry_identity()`.

- [ ] **Step 1: Write failing tests** (`auth/tests.rs` with the synthetic loopback issuer and the Docker storage fixture): one test per ADR-006 route row plus `pre_access_cannot_reach_protected_routes_or_mint_session`, `replayed_callback_mismatched_state_unknown_kid_forged_token_refused`, `cross_site_post_is_403_cross_site`, `idle_and_absolute_expiry_and_signout_everywhere_invalidate_and_bump_epoch`, `pending_resend_mapping_for_all_five_provider_outcomes`, `both_admission_completion_orders_mint_at_most_one_session`, `unverified_mailbox_commits_no_slot_membership_or_entitlement`, `revoked_entitlement_during_pending_never_mints`, `release_build_has_no_harness_routes_and_refuses_qualification_env`, `no_provider_token_or_set_cookie_reaches_browser`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement** 0006 and the eight pins (6), extract the harness helpers into `auth/*` (keep `auth_qualification.rs` compiling behind the feature), implement `sessions.rs`, routes, SSR pages (`SigninProps { handle_known: bool, csrf: String }`, `PendingProps { resend_available: bool, cooldown_seconds: u32, csrf: String }`), the fallback handler, and the router changes.
- [ ] **Step 4: Run** all local checks, `scripts/check-foundation.py` (updated smoke expectations), the Docker image build with the new allowlist and `qualify_image.py`; expected: pass; page sizes under 28,420 B forma / 76,800 B page.
- [ ] **Step 5: Independent review** → `evidence/APP-009/independent-review.md`; move APP-009 to In review; commit (`feat(auth): fAuth-backed sessions, sign-in, pending verification and sign-out (APP-009)`). Then send the FormaStack owner the final route contract SHA.

### Task 8: APP-074 — public landing, invitation entry, limits and trusted proxy (local part)

**Files:**
- Create: `app/admin/src/invite/app.tsx`, `app/src/auth/invite.rs`, `app/src/http/mod.rs`, `app/src/http/limits.rs`, `app/src/http/proxy.rs`, `app/src/http/problem.rs`, `app/src/http/fallback.rs`, `app/src/http/tests.rs`, `evidence/APP-074/local/README.md`
- Modify: `app/admin/src/home/app.tsx` (landing copy; keep `Foundation workspace` marker or update smoke pins), `app/src/main.rs`, `app/admin/build.ts`, `.dockerignore`

**Interfaces:**
- Consumes: `Workspace::{resolve_code, reserve_code, begin_flow}`, `auth::provider::Client::par` with `invite_token` from the provider-invitation secret (APP-073 record → secret reference read by the Python helper into a private file, same pattern as the qualification JSON).
- Produces: `GET /invite`, `POST /invite/validate` per ADR-006; `http::limits::Buckets::check(class: &AddressClass, key: LimitKey) -> Result<(), Problem /* 429 with Retry-After */>` with the ADR-006 values; `http::proxy::client_address(headers, peer: SocketAddr) -> AddressClass` (rightmost `X-Forwarded-For` hop, peer must be the ALB's subnet or loopback in tests); `http::problem::Problem { status, code }` → `application/problem+json`; fallback route returning uniform 404 with `no-store`, `nosniff`.

- [ ] **Step 1: Write failing tests**: `invalid_unknown_expired_revoked_exhausted_codes_answer_identically_and_consume_no_slot`, `forged_forwarded_for_does_not_evade_limits`, `signin_and_invite_floods_hit_ip_account_global_limits_without_provider_calls`, `unknown_and_protected_paths_return_uniform_404_problem_json_with_security_headers`, `body_over_limit_and_slow_body_are_refused`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement** the pages, routes, limit buckets, proxy extraction, problem responses and fallback.
- [ ] **Step 4: Run** all local checks and the image qualification; expected: pass.
- [ ] **Step 5: Independent review** → `evidence/APP-074/local/independent-review.md`; APP-074 stays Backlog/Selected as the board allows (it has no hosted items: they are APP-076's and APP-079's by ADR-008); commit (`feat(http): public landing, invitation entry, limits and trusted proxy (APP-074 local)`).

### Task 9: APP-012 — authorized typed query with receipt

**Files:**
- Create: `app/runtime/query_engine.py` (copy of the APP-005 executor with recorded source identity), `app/runtime/test_query_worker.py`, `app/src/query/mod.rs`, `app/src/query/worker.rs`, `app/src/query/receipt.rs`, `app/src/query/routes.rs`, `app/src/query/tests.rs`, `app/src/storage/queries.rs`, `app/admin/src/work/app.tsx`, `evidence/APP-012/README.md`
- Modify: `app/runtime/requirements.lock` (`duckdb==2.0.0` with hashes for linux_aarch64 and x86_64), `app/runtime/runtime.py` (`query` and `cache` modes), `app/Dockerfile` (unchanged stages; new files via allowlist), `.dockerignore`, `app/src/main.rs`, `app/admin/build.ts`

**Scope decided here:** the minimal queries are the Northstar `total` (1,140,000 bytes) and `timeline` views, the only fixture views inside APP-005's qualified operator subset; CIDR lookup, ratio (the 114,000 bytes/minute rate) and money literals (the Spend Review `queue`) stay refused and belong to PH02 (APP-020+). A truthful receipt needs a real source revision: the operator package's `upload-sources` normalizes the fixture CSV (`[event_id, asset, time, destination, bytes]`) to the contract `events_v1` shape (`[time, destination, bytes]`, asset selected by the trusted adapter) as Parquet through the APP-004/005 adapter, uploads it under `o/<org>/sources/<source_id>/<revision_id>` and records the real `normalized_digest`/`contract_digest` in `source_revisions` and `work_bindings` for the Northstar organization, replacing the placeholder digests; the receipt mode is `published` with real publication, principal, epochs and record snapshot sequence.

**Interfaces:**
- Consumes: APP-005 `execute_query(query, source_path) -> {rows, receipt_inputs}`; APP-011 oracles in `fixtures/phase-one/v1/expected.json`; `Workspace::resolve_session`; `admission::digest`; `Permit` with `Action::Run` on `Resource::App` and `Action::SourceRead` on `Resource::Source`.
- Produces: `impl Data { pub async fn run_query(&self, request: &TrustedRequest, app: &str, work: &str, view: &str, parameters: &serde_json::Value) -> Result<QueryRun /* source revision, bindings, record snapshot sequence, epochs */>; pub async fn release_result(&self, request: &TrustedRequest, run: &QueryRun) -> Result<()> /* second precheck+fence before delivery */ }`; `query::worker::execute(run: &QueryRun, deadline: Duration) -> Result<WorkerResult, QueryError>` spawning `python3 /app/runtime.py query` per D7.10 with request JSON `{organization_id, source: {source_id, revision_id, normalized_digest, s3_key}, query, parameters, deadline_ms}` and result line `{rows ≤500, total_scope, result_digest, partial:false, truncated_page, elapsed_ms}`; `query::receipt::build(run, result) -> QueryReceipt` matching `contracts/schema.json` `query_receipt` with `digest = digest("forma:query-receipt:v1", canonical)`; `POST /v1/work/{app}/{work}/queries/{view}` → 200 `{rows, receipt}` | 404 | 422 `narrow_the_query` | 429 | 503; `GET /w/{app}/{work}` SSR page with `WorkProps { state: "loading"|"empty"|"failed"|"denied"|"ready", receipt: Option<ReceiptView> }` and no fabricated zero on `failed`.

- [ ] **Step 1: Write failing tests**: `test_query_worker_matches_app011_oracles` (Northstar `total` 1,140,000 and the destination-filtered 900,000, and `timeline`; the 114,000 ratio and the Spend Review money views are asserted refused, D7.10), `engine_child_has_no_credentials_or_network_config` (empty environment, `enable_external_access=false`, scratch removed), `admission_bounds_two_per_organization_four_per_task_then_429`, `worker_is_killed_at_deadline_and_reports_failed_not_zero`, `worker_cannot_read_outside_organization_cache`; Rust: `authorized_member_gets_exact_result_and_receipt_digest`, `foreign_or_revoked_scope_denied_before_release_including_change_between_execution_and_delivery`, `view_states_loading_empty_failed_denied_never_zero`, `receipt_vectors_match_contracts`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement** the worker modes (cache fetch via tenant role with digest verification; DuckDB config per D7.10), `queries.rs`, `worker.rs`, `receipt.rs`, routes and page; record `duckdb` 2.0.0 registry checksums and license in the APP-012 log.
- [ ] **Step 4: Run** all local checks, the unit tests with a local cache directory seeded from `fixtures/phase-one/v1`, and the image qualification (duckdb import under `--network=none`); expected: pass.
- [ ] **Step 5: Independent review** → `evidence/APP-012/independent-review.md`; move APP-012 to In review; commit (`feat(query): bounded typed query with receipt (APP-012)`).

### Task 10: APP-013 — one declared command, outcome and audit in one transaction

**Files:**
- Create: `app/migrations/0007_command_outcomes.sql`, `app/migrations/catalog-v7.json`, `app/src/storage/commands.rs`, `app/src/commands/mod.rs`, `app/src/commands/payload.rs`, `app/src/commands/routes.rs`, `app/src/commands/tests.rs`, `app/admin/src/record/app.tsx`, `evidence/APP-013/README.md`
- Modify: `app/src/storage/authorization.rs` (new `Transaction::outcome_lookup_or_insert` between `precheck` and `fence`; `Permit::commit_record` requiring `Action::RecordWrite`), `app/src/storage/pool.rs` (`TestHooks` phases `precheck`, `outcome`, `fence`, `record_lock`), pins, `.dockerignore`, `app/src/main.rs`, `app/admin/build.ts`

**Interfaces:**
- Consumes: `genui.command_claims` (D7.9, insert-only, PK on the five-part scope + key, `payload_digest`, `claimed_at`); `genui.command_outcomes(organization_id, app_id, work_id, principal_id, command_id, idempotency_key CHECK ~ '^[0-9a-f]{32}$', payload_digest, state CHECK ('committed'), record_id, record_version, work_version, commit_sequence, audit_event_id FK audit_events, committed_at, retry_expires_at)` PK on the five-part scope + key with FK to the claim; D7.7 sequence (0007 creates the table; the operator backfills rows); D7.8 audit; `lock_member_authority`, `lock_work_revision(expected_work_version)`.
- Produces: `pub struct CommandRequest { app, work, publication, command, schema, schema_version_expected, record: Option<String>, expected_record_version: i64, expected_work_version: i64, values: serde_json::Value, idempotency_key: String }`; `impl Data { pub async fn commit_record(&self, request: &TrustedRequest, command: &CommandRequest) -> Result<CommandResult>; pub async fn read_outcome(&self, request: &TrustedRequest, app: &str, work: &str, command: &str, key: &str) -> Result<Option<CommandOutcome>> }` in ADR-004/D7.9 order: `precheck(RecordWrite)` → `INSERT INTO command_claims … ON CONFLICT DO NOTHING RETURNING 1` (zero rows: read the committed outcome → `Replayed` on the same `payload_digest`, `Conflict` on a different one; a wait past `lock_timeout` on the in-flight claim → `InFlight`) → `lock_member_authority` → `lock_work_revision` with the caller's `expected_work_version` → `organization_sequences FOR UPDATE` → `internal_records FOR UPDATE` and `expected_record_version` (0 = absent; mismatch → `Conflict`) → `audit_heads FOR UPDATE` → insert revision, current pointer, outcome (FK to the claim), audit event → commit; `commands::payload::digest(values) -> String` (`forma:command-payload:v1`); `POST /v1/work/{app}/{work}/commands/{command}` with header `Idempotency-Key` (or hidden field) → 200 | 400 (missing/short key) | 404 | 409 recovery view, `in_flight` or `busy` with `Retry-After: 2` | 412 current revision | 413 over 256 KiB | 503 only for transport/unknown outcome, per D7.9; SSR record page with the form carrying the per-render key (D7.16).

- [ ] **Step 1: Write failing tests** (S05 sequence on the Docker fixture): `command_v0_to_v1_writes_one_revision_one_outcome_one_audit_event`, `identical_key_and_payload_replays_without_second_write`, `same_key_different_payload_conflicts`, `stale_expected_version_conflicts_with_recovery_view_and_no_partial_write`, `second_command_with_ik2_reaches_v2`, `restart_then_reopen_same_record_version_and_outcome`, `outcome_visible_only_to_its_principal_under_current_authority`, `audit_chain_verifies`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement** 0007 and pins (7), `commands.rs`, payload digest, routes, page; keep every statement inside the 3 s transaction and 2000 ms statement timeout.
- [ ] **Step 4: Run** all local checks; expected: pass.
- [ ] **Step 5: Independent review** → `evidence/APP-013/independent-review.md`; move APP-013 to In review; commit (`feat(records): atomic declared command with outcome and audit (APP-013)`).

ADR-009 D9.8 obligation (recorded by the Phase 2 plan): migration 0007 also creates `genui.query_receipts` with the D9.8 columns, so Phase 2's receipt citation builds on it.

### Task 11: APP-014 — retries, conflicts and both authorization race orders

**Files:**
- Create: `app/src/storage/race_cases.rs` (cfg(test)), `evidence/APP-014/README.md`
- Modify: `app/src/storage/qualification.rs` (campaign groups), `app/src/storage/pool.rs` (checkpoint plumbing)

**Interfaces:**
- Consumes: `TestHooks` phases from Task 10; the operator client pattern at `qualification.rs:668-734` (writer takes `organizations FOR SHARE`, `memberships FOR UPDATE` and advances the epoch; a test-only adoption writer takes `saved_work FOR UPDATE` and bumps `version` through the owner channel, D7.4); `pg_stat_activity` lock-wait barrier.
- Produces: campaign groups `lost_response_retry_returns_original_outcome_with_stale_expected_version`, `same_key_and_different_key_races_commit_exactly_one_intended_write`, `other_principal_cannot_retrieve_key`, `revocation_vs_commit_both_orders`, `adoption_vs_commit_both_orders`, each asserting the typed `CommandResult` and the HTTP mapping of D7.9 under a held writer lock (`lock_timeout` path answers 409 `busy`, never 503).

- [ ] **Step 1: Write the failing campaign groups** (they fail until checkpoints and writers exist).
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement** the checkpoints and writers; no production API change.
- [ ] **Step 4: Run** the full Docker campaign twice; expected: deterministic pass; record timings.
- [ ] **Step 5: Independent review** → `evidence/APP-014/independent-review.md`; move APP-014 to In review; commit (`test(storage): command retry, conflict and authorization race campaign (APP-014)`).

---

## Track B — managed tenant qualification: APP-075, APP-077, APP-078 (parallel with Track A)

B1–B3 belong to APP-075, B4 to APP-077 and B5 to APP-080 (ADR-008); both depend only on APP-008 and may run as preparation evidence while APP-008 awaits landing (board status moves only when `board.py next` lists them). B6–B7 are APP-078, the single reserved window, which depends on both and on a Victor-approved slot outside any auth window.

### Task B1: Finish and test the managed-entry draft

**Files:** Modify `app/src/storage/managed_qualification.rs`; create `app/src/storage/managed_qualification/tests.rs`; `evidence/APP-075/entry/README.md`.

**Interfaces:** Produces the reviewed entry `storage::managed_qualification::managed_readonly_entry` with: `cleanup_confirmed` reported `true` only after `Stores.close()` succeeded and `pool::slots().0 == 0` (refusals before construction report `cleanup_applicable:false`); `issued_at <= now + 5`; required remaining time ≥60 s; non-case panics labeled `entry_panic`; stdout drained contract documented; fixture pin covering `contracts/fixtures/northstar.json` and `spend-review.json` in addition to `setup.json`.

- [ ] **Step 1: Write failing tests**: closed/duplicate-key manifest parsing, selector/arming/UID refusals, pins/registry/CA mismatch, private file ownership/mode/link/size refusals, deadline refusals (span > 600, remaining < 60), sanitized output, `Stores.close` and permit release on success, case panic and timeout (with a `TestHooks`-blocked case).
- [ ] **Step 2: Run** `cargo +1.93.1 test --locked --lib storage::managed_qualification`; expected: FAIL.
- [ ] **Step 3: Implement** the repairs above.
- [ ] **Step 4: Run**; expected: pass; `cargo +1.85.0 check` passes.
- [ ] **Step 5: Independent review** (completing `evidence/APP-008/native/managed-entry/independent-handoff-review.md`'s open items in a new APP-075 review file); commit.

### Task B2: Local-only positive harness

**Files:** Create `app/qualification/local_harness.py`, `evidence/APP-075/local-positive/README.md`.

**Interfaces:** Produces a script that builds the Linux `cfg(test)` executable in `rust:1.93.1-bookworm` (`cargo test --no-run --locked --lib`, path taken from `--message-format=json`), starts `postgres@sha256:5d143123…` on an internal network with `--network-alias genuisyntheticfoundation-databaseb269d8bb-pta9s49jelmg.cg58i06y2r4e.us-east-1.rds.amazonaws.com` (fallback `--add-host` if the embedded DNS refuses the alias), `ssl=on` with an owned synthetic CA and SAN cert copied to `/etc/ssl/certs/rds-global-bundle.pem`, applies the current migrations and seed through the contracts venv, writes `credentials.json`, the byte-exact registry and manifest as UID 65532, runs the exact five-argument invocation, drains stdout and asserts `passed:true` with the six checks.

- [ ] Steps: write the harness; run; fix; record exact image/exe/file hashes; independent review; commit.

### Task B3: Qualification image and private supervisor

**Files:** Create `app/qualification/Dockerfile`, `app/qualification/Dockerfile.dockerignore`, `app/runtime/qualification_supervisor.py`, `app/runtime/test_qualification_supervisor.py`; modify `infra/src/stack.ts` (optional `QualificationTask` definition, count zero, same runtime role, no migration secret) and `infra/test/security.test.ts`.

**Interfaces:** Supervisor reuses `runtime.refresh/load/atomic_write`, writes manifest and registry under `/run/genui` 0700/0600, launches the exact executable with the five fixed arguments, refreshes credentials until exit, retains `APP008_MANAGED_RESULT`, honors the original deadline, receives no operator password. Image pins the reviewed RDS CA (`sha256 90a6f1a7…`) at the fixed path and forbids mounts.

- [ ] Steps: tests first (supervisor refusals, exact args, no serve), implement, build image, confined `qualify_image.py`-style checks, CDK tests, independent review, commit.

### Task B4 (APP-077): Operator package (migrate, seed, admission operator, fixture upload)

**Files:** Create `app/operator/Dockerfile`, `app/operator/Dockerfile.dockerignore`, `app/operator/operator.py` (subcommands `migrate`, `seed`, `admission {issue,list,revoke,extend}`, `upload-sources`, `verify-catalog`), `app/operator/requirements.lock`; modify `infra/src/stack.ts` (optional `OperatorTask` definition using the migration execution role and `PGPASSWORD` secret; overrides ≤8,192 bytes; `entryPoint` matches).

- [ ] Steps: package `runtime.apply_migrations`, `tenant_seed.seed`, `contracts/validate.py` and fixtures, `admission_operator.py`; S3 upload of fixture source bytes to `o/<org>/sources/…` with KMS; tests; image build; CDK tests; review; commit.

ADR-009 B1 obligation (recorded by the Phase 2 plan): `upload-sources` writes the B1 object layout (`o/{org}/s/{source}/r/{rev}/original` and `…/part.parquet`), not D7.10's earlier key.

### Task B5 (APP-080, deferred by ADR-008): Two-process race coordinator (local proof)

Not a PH01 task since ADR-008: APP-080 (PH05) owns the coordinator and the RDS lock-observation rows before any real organization. The steps below stay as the implementation plan for that ticket.

**Files:** Create `app/qualification/race_coordinator.py`, `app/src/storage/managed_races.rs` (cfg(test)); `evidence/APP-080/coordinator/README.md`.

**Interfaces:** Run-scoped advisory-lock phase beacons (`pg_try_advisory_lock(hashtext(run_id||phase))`) and `pg_blocking_pids`/`pg_locks` observations per the managed plan; application actor reserves one workspace lease for coordination; operator actor uses two migration connections; both revocation orders, suspension and saved-work version change; monotonic epoch restoration; peer-loss cleanup; never extends the 3 s transaction.

- [ ] Steps: tests on disposable PG 16.13/TLS first; implement; run; review; commit.

### Task B6 (APP-078): The RDS window (after APP-075 and APP-077 Done, outside any fAuth deployment operation, Victor-approved; ADR-008 rows)

- [ ] Reconcile the ledger basis; new window UUID via `infra/scripts/preflight.py --input … --ledger …` (count 0, `extra_task_hours` ≥ 2); TemplateURL upload path reviewed; stack update with the new `ImageDigest` and the two optional task definitions after observing 0/0/0 and the consumer stopped.
- [ ] Review the ten-minute campaign cap against image startup and reserved task-hours; the drift-trial target choice moved to APP-080 (ADR-008). Run the ADR-008 rows only (readiness comparison, migrations once and again, seed, four role bindings, grant/RLS isolation matrix, HTTPS surface); take the pre-migration snapshot but leave its restore drill to APP-080.
- [ ] Retain receipts under `evidence/APP-078/`; independent review; move APP-078 to In review, then Done after CI/landing of any source changes.

---

## Track C — beta go-live path with FormaStack (ADR-008)

The joint six-hour identity rehearsal, its pre-start list and its in-window runbook are deferred to APP-079 / FORMA-365 by Victor's disposition of 2026-10-29 ([ADR-008](../architecture/ADR-008-YC-DEMO-AND-BETA-MVP.md)). Track C now puts fAuth and GenUI live for the invite-first beta. The old C1/C2 material stays valid for APP-079 in `evidence/APP-006/hosted-callback/schema-one-sequence/` and `evidence/APP-006/window-schedule-collision-check-20261029.md`.

### Task C0: Bind the merged provider contract (done 2026-10-29)

- [x] `handoff/forma-247/README.md`, `config.example.json` and `provision.py` at gatewasm `dfba787` and `MAIL-VERIFICATION.md` plus the `return_entry_url` provisioning change at `1bf5d3b` are hash-bound in `evidence/APP-006/upstream-auth-source/merged-contract-2026-10-29/` (fetched from the now-public repository, blob SHA-1s matching GitHub's tree; the FormaStack checkout was not touched). APP-006 records the server-only invitation and PAR contract and the mail contract as accepted merged source; verification is attested through `user.email_verified`, not a JWT claim.
- [x] Gate split applied in `Tickets/external-dependencies.json`: FORMA-247/248 cleared by disposition with the disposition record as evidence; FORMA-365 blocks APP-058 and APP-079; FORMA-366 (mail qualification) and FORMA-367 (lean deployment) block APP-076.

### Task C1: Victor's two choices (the GenUI figure still blocks the hosted steps C3 and Track D; SES and FORMA-367 are approved)

- [ ] The monthly figure for the always-on beta (ADR-008 default about $185–210/month including FormaStack's lean fAuth, or a scheduled daily window). Record it as a dated amendment in `docs/planning/SYNTHETIC-ENVIRONMENT-COST.md` and rewrite the ledger `basis` in `.cache/operations/beta/budget-ledger.json` from actual retained resources before any reservation.
- [x] The mail transport: Victor approved SES SMTP with FORMA-367 on 2026-10-29 ("Approved: deploy the lean always-on fAuth (FORMA-367) at about $33/month using SES SMTP. Coordinate the timing with GenUI.", relayed by FormaStack). Default as planned: Amazon SES SMTP in account 877573132739 with a verified `getforma.dev` sender; alternative Resend). GenUI creates the DNS records at Porkbun and records the record ids privately; FormaStack configures fAuth's SMTP secret.

### Task C2: FormaStack deploys fAuth lean for the beta (FORMA-367 deployment, FORMA-366 mail qualification; FormaStack's work, GenUI receives)

- [ ] Request from the FormaStack owner, with Victor's approvals quoted: a qualified image at or after `1bf5d3b`, a lean always-on deployment, tenants mapped to Northstar `…0001` and Canary `…0002`, the beta client with exact callback `https://beta.getforma.dev/auth/callback` and `return_entry_url` `https://beta.getforma.dev/signin/{handle}` (handles supplied by GenUI from APP-009's registry), the controlled-inbox trial receipt, and the deployed issuer identity plus `ten_`/`usr_` identifiers delivered privately.
- [ ] Record the receipt under `evidence/APP-006/upstream-auth-source/`, write the private identity registry file (ADR-004, at most 32 bindings) and the beta client configuration into the execution secret, set FORMA-366 and FORMA-367 `cleared` in the register, regenerate the board; APP-076 leaves Blocked. Request SES production access as soon as Victor chooses SES: the sandbox mails only verified addresses.

### Task C3: GenUI deploys the beta build (after Tasks 1–11 and B6)

- [ ] APP-078's window with the service stopped (ADR-008 rows: readiness comparison, migrations 0003–0007 once and again, seed, four role bindings, grant/RLS isolation matrix, HTTPS surface, cleanup); then set the service desired count to 1 on the exact candidate image, point `beta.getforma.dev` at the load balancer with the already validated certificate, and issue the YC viewers' and demo principals' invite codes with the operator CLI (raw codes delivered privately once). The service stays running.

---

## Track D — APP-076 beta go-live trial and the PH01 gate

- [ ] After Tasks 9–11, B6/C3 and FORMA-366 cleared: run APP-076's acceptance list in installed Chrome against `https://beta.getforma.dev` with real mail; exercise rollback as a redeploy of the previous task definition and restore the service; reconcile the ledger against the approved beta figure; independent review; APP-076 Done with the service observed RUNNING on the recorded digest.
- [ ] Create `evidence/phases/PH01/GATE.md` from `Tickets/_tools/PHASE-GATE.md`, bind every exit criterion to its ticket evidence, record limits and amendments (ADR-005/006/007/008) and name the ADR-008 deferrals (APP-079 in PH06, APP-080 in PH05) as limitations, obtain the independent reviewer's decision, then set `gate_status: passed` with the evidence reference in `Tickets/plan.json`, render and check the board, update `docs/planning/PHASE-2-HANDOFF.md` to READY.

---

## Victor's actions (only these)

1. Approve the monthly figure for the always-on beta (ADR-008; default about $185–210/month including FormaStack's lean fAuth) or choose the scheduled-window alternative.
2. Choose the mail transport (default Amazon SES with a verified `getforma.dev` sender) and approve its small cost.
3. Approve the APP-078 window slot when GenUI proposes it (a few dollars inside the approved figure).

Repository visibility stays public with the all-rights-reserved notice until the beta deployment is complete (done at his direction on 2026-10-29); setting it back to private afterwards is his call, remembering that any fork made meanwhile stays public.

---

## Self-review

- Spec coverage: every PH01 acceptance sentence maps to a task (APP-073 → 4–5, APP-010 → 6, APP-009 → 7, APP-074 → 8, APP-012 → 9 + D, APP-013 → 10, APP-014 → 11, APP-075 → B1–B3, APP-077 → B4, APP-078 → B6/C3, APP-076 → D, APP-006 → C0, APP-008/011 → 1); the ADR-008 deferrals are APP-079 (PH06) and APP-080 (PH05); PH01 exit criteria 1–5 → D.
- Type consistency: `VerifiedNonmember`, `Admission`, `CommandResult`, `CommandRequest`, `QueryRun`, `TrustedRequest` constructor location and the three definer functions are named identically in Tasks 2, 4, 5, 7, 9, 10, 11.
- Review focus: the five failure modes each have an owning task and test.
- Proportion: the plan decides files, names, signatures, orders and values; bodies are left to the implementer except the ADR-004 transaction order, which the spec fixes.
