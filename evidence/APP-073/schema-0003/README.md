# APP-073 slice 1 — migration 0003 (plan Task 4) local evidence

Date 2026-10-29. Base `93a7ee2` (phase-one-spikes). Status: implemented and qualified **locally** on disposable PostgreSQL 16.13 in Docker; no CI run, no managed (RDS) application, no independent review yet. Controlling text: [ADR-007](../../../docs/architecture/ADR-007-PHASE-1-WRITE-AUTHORITY.md) D7.1, D7.2, D7.3, D7.8, D7.12, D7.17, D7.19 and its Consequences paragraph; plan Task 4; ADR-004; ADR-003; the runner review `evidence/APP-073/runner-review.md` (uncommitted in the main checkout when this slice was made) items M1, m1, m2, m3 (m5's fixture edition bump deliberately not done).

## What changed

| Area | Files |
| --- | --- |
| Migration 0003 | `app/migrations/0003_admission.sql` (34,935 bytes, sha256 `865ec1d5…88d0b`) |
| Catalog | `app/migrations/catalog-v3.json` (generated; 199,290 bytes, sha256 `aacabcfb…51cb8`), `catalog-query.json` (A3 role lists, bound 524,288; sha256 `3e3e744d…fafba`), `manifest.json` (database_version 3) |
| Pins moved to 3 | `app/runtime/runtime.py` `DATABASE_SCHEMA_VERSION`, `app/src/storage/catalog.rs` `EXPECTED`, `app/src/storage/pool.rs` `verify_schema_version`, `app/src/storage/managed_qualification.rs` `CATALOG`, native-campaign fixture literals in `app/src/storage/qualification.rs` (runner review m4) |
| Cap move (D7.12) | `CATALOG_BOUND = 524288` in `runtime.py` (catalog file bytes and `max_output_bytes`), `BOUND` in `catalog.rs`, `octet_length(body)<=524288` and `max_output_bytes` in `catalog-query.json`; generator guard derives 419,430 from it |
| Runner review | M1: `migrate()` and `probe()` refuse a manifest whose `database_version` differs from `DATABASE_SCHEMA_VERSION`, before connecting; `.github/workflows/tenant-foundation.yml` runs the pin check, the scripts tests and the runtime tests before the campaigns. m1: catalog-bound agreement group in `scripts/check-schema-pins.py`. m2: generator refusal names `body` or `file` length; the receipt records both (`body_bytes`, `file_bytes`, `guard_bytes`). m3: pin check asserts each `catalog-vk.json` (k ≥ 2) records both version CHECKs as `version <= k` and that consecutive catalogs differ |
| Campaign | `app/runtime/qualify_tenant_schema.py`: `admission_checks` plus the A3 normalization (`genui_admit` in the creator-bookkeeping list) and the two generic checks that 0003 changes (policy shape, function owners) |
| Tests | `app/runtime/test_runtime.py`, `scripts/test_check_schema_pins.py` |
| Image context | `.dockerignore` admits `0003_admission.sql` and `catalog-v3.json` |

Catalog sizes: catalog-v3 file **199,290 bytes**, database `jsonb::text` body **203,170 bytes** (guard 419,430, bound 524,288). catalog-v1 (3,905 / 3,829) and catalog-v2 (123,415 / 125,966) are byte-identical to the committed files after the query change; the generator produced identical catalog-v3 bytes in three runs.

## Exact commands and results

Interpreters: the existing locked venvs of the main checkout (this worktree has none). `W` = `/private/tmp/claude-501/-Users-victorvillacis-dev/df0e31da-4f04-46a7-8043-30cdac91f9c2/scratchpad/with_contracts.py`, which only prepends `contracts/.venv/lib/python3.11/site-packages` to `sys.path` (equivalent to `PYTHONPATH=contracts/.venv/lib/python3.11/site-packages`). `PY` = `<main checkout>/app/runtime/.venv/bin/python` (Python 3.11).

| Command (repository root unless noted) | Result |
| --- | --- |
| `PY W app/runtime/qualify_tenant_schema.py --output <scratch>/red-run.json` with the 0003 SQL not yet in the tree (TDD red run) | exit 1: `AssertionError` at the genui_admit function check (no admission functions); all containers removed |
| `PY W app/runtime/qualify_tenant_schema.py --output <scratch>/run2.json` | exit 0, `passed: true`, **276 checks**, every container removed; retained as `schema-campaign.json` (its `source_sha256` equals the committed sources) |
| `PY W app/runtime/generate_schema_catalog.py --output-dir <new scratch dir>` (3 runs) | exit 0; v1/v2 identical to committed, v3 identical across runs; final receipt `generation.json` |
| `PY -m unittest discover -s app/runtime` | 39 tests OK |
| `PY -m unittest discover -s scripts -p 'test_*.py'` | 5 tests OK |
| `python3 scripts/check-schema-pins.py` | exit 0, `agree: true`: five version pins = 3; four bound pins = 524,288; `catalog-v2.json` → 2, `catalog-v3.json` → 3 |
| `<main checkout>/spikes/APP-005/.venv/bin/python -m unittest discover -s fixtures` | 14 tests OK |
| `PY W fixtures/reset_trial.py --output <new scratch dir>` (the APP-005 venv locally lacks psycopg; CI installs the runtime lock into it) | exit 0, `passed: true`, owned containers removed; `reset-trial-summary.json` |
| in `app/admin`: `npm ci`, `npm run build` (Node 22.21.0, npm 11.6.2; needed for the embedded page assets) | exit 0 |
| in `app/`: `CARGO_BUILD_JOBS=2 cargo +1.93.1 fmt --check` | exit 0 |
| in `app/`: `CARGO_BUILD_JOBS=2 cargo +1.93.1 clippy --locked --all-targets -- -D warnings` | exit 0 |
| in `app/`: `CARGO_BUILD_JOBS=2 cargo +1.93.1 test --locked` | exit 0: 27 passed / 2 ignored, 1 passed, 1 passed |
| in `app/`: `CARGO_TARGET_DIR=target/msrv CARGO_BUILD_JOBS=2 cargo +1.85.0 check --locked --lib --tests` | exit 0 |
| in `app/`: `GENUI_NATIVE_FIXTURE_PYTHON=<scratch wrapper: PY with contracts on PYTHONPATH> GENUI_NATIVE_REPORT=<scratch>/native.json CARGO_BUILD_JOBS=2 cargo +1.93.1 test --locked --lib storage::qualification::native_postgres_campaign -- --ignored --exact --nocapture` | exit 0, 1 passed, report `passed: true` with 36 checks (TLS pools against the v3 catalog); `native-campaign.json` |

`qualify_tenant_schema.py` uses local trust authentication without TLS (`sslmode=disable`, loopback only); the TLS path is the native campaign. No container started by these commands remains (`docker ps -a` shows only two containers that predate this work and belong to other sessions).

## What the schema campaign asserts for 0003 (ADR-007 Consequences)

- Every 0003 relation (`access_codes`, `code_grant_templates`, `code_reservations`, `code_redemptions`, `entitlements`, `provider_invitations`, `audit_events`, `audit_heads`) is owned by `genui_owner` with ENABLE + FORCE RLS and exactly one permissive `tenant_scope` policy with the 0002 expression in USING and WITH CHECK; all indexes and FKs are organization-leading (the generic checks); `genui_private.code_index`/`handle_index` carry no RLS and no policy.
- The catalog's (role, relation, privilege, column) set for `genui_lock`, `genui_admit` and the four application roles equals catalog-v2's grants plus the literal D7.1 `genui_admit` set (122 column grants, no table-level grant); no application role holds anything on a 0003 relation; `genui_admit` holds USAGE on `genui` and `genui_private` only; EXECUTE on the six functions is `genui_workspace` only, and the 0002 fences are unchanged.
- `genui_admit`: all seven attributes false (NOLOGIN, NOINHERIT, …), member of nothing, incoming only `genui_migration` (`SET TRUE, INHERIT FALSE` plus PostgreSQL's creator ADMIN bookkeeping), no CREATE anywhere, no CONNECT. The six functions have the exact D7.1 identity arguments and result types, `SECURITY DEFINER STRICT VOLATILE`, `search_path=pg_catalog, pg_temp`; no body contains an UPDATE/INSERT/DELETE of `genui.organizations` and every scoped body takes `organizations … FOR SHARE`; a negative test proves the organizations tuples' `xmin`/`ctid`/`status` are unchanged after every call of the campaign.
- `code_index`/`handle_index` and every 0003 relation are unreadable and unwritable by all four application roles (42501); control, data and worker cannot EXECUTE any of the six; no application role can `SET ROLE genui_admit`.
- RESTRICTIVE audit bindings: the INSERT holders of `audit_events` (`genui_admit` by column grant, `genui_owner` by ownership) equal the targets of the RESTRICTIVE INSERT policies, and the UPDATE holders of `audit_heads` equal the targets of the RESTRICTIVE UPDATE policies; nobody holds UPDATE, DELETE or TRUNCATE on `audit_events`. Mismatched `writer_role` or `kind` is refused for `genui_admit` (four cases) and the operator (three cases); matching rows are accepted; head UPDATE reaches only the own chain.
- Behavior, all as `genui_workspace` with organization-only scope (as the storage module's nonmember entry would install):
  - `resolve_code`/`resolve_handle` before scope: known → organization; unknown, malformed, retired → NULL; a future-dated retirement (rotation overlap) still resolves.
  - `reserve_code`: `reserved` (10-minute presignup row, never counted), `expired`, `revoked`, `exhausted`, `too_many_reservations` at 10 live (an expired one does not count); unknown, foreign-organization, mismatched-scope and malformed calls return no row.
  - `record_pending_admission`: `pending` (24-hour row bound to the identity, reservation consumed, one audit event), `reservation_expired` (consumed, unknown, expired), `expired` and `revoked` (code changed after the reservation), `exhausted` by D7.17 arithmetic (single-use code with one live pending admission), `already_member` writes nothing.
  - `admit_redemption`: `admitted` creates principal, epoch-1 membership, the template's grants verbatim (2 app, 1 work, 1 source, no organization permission), a 30-day entitlement, the redemption and one `admission.redeemed` event in one call; a single-use code with one pending admission is admitted (not exhausted) and its status becomes `exhausted`; a two-use code becomes `exhausted` exactly at 2; `replayed` returns the stored identifiers and writes nothing; changed payload, another identity, unknown and foreign pending admissions give `mismatch`; `already_member` consumes nothing; an expired pending admission becomes `denied_for_reconciliation` with one audit event and answers `expired`; revoked, expired and (operator-forced) exhausted codes refuse with no write.
  - Concurrency with an observed lock wait (`pg_stat_activity.wait_event_type = 'Lock'`): two identities racing `record_pending_admission` for the last slot (one `pending`, one `exhausted`); a redemption and a new pending admission racing for the last slot in both orders (exactly one admission); two identical concurrent redemptions (one `admitted`, one `replayed`, one slot consumed).
  - `advance_member_epoch`: own epoch advances for `session.sign_out` and `session.sign_out_all`, each with one audit event; another principal, a stale epoch, a foreign scope and any other reason (`membership.revoked`, `session.signed_in`, `admission.redeemed`) return NULL with no write; it waits behind the operator's revocation order (organization FOR SHARE → member FOR UPDATE) and then refuses the moved epoch.
- Audit chains: for both organizations every event hash is recomputed in Python (independently of the SQL) in `chain_sequence` order from the genesis head and the tail equals the head; a tampered stored `kind` fails verification (positive control, rolled back).

## Implementation choices the ADR leaves open (for review)

These are not deviations from ADR-007 text; each is the narrowest reading I could find and is listed so the reviewer can overrule it.

1. **No-row refusals.** A transaction scope that differs from `p_org`, an unknown or non-active organization, an unknown code digest under scope, and a malformed digest argument return no row (as the 0002 fences do), not a typed outcome; these are programming or configuration errors, not business refusals. An unknown pending admission is `mismatch`; an unknown reservation is `reservation_expired`. `advance_member_epoch` returns NULL for every refusal. A missing admission audit head raises SQLSTATE 55000 (configuration, as D7.7 treats a missing sequence row).
2. **Suspended organization.** The four scoped admission functions refuse (no row) when `organizations.status <> 'active'`; `advance_member_epoch` does not, so sign-out always works.
3. **`reserve_code` exhaustion** uses the D7.17 `remaining` (redemptions plus live pending admissions); presignup rows never count.
4. **Audit events written.** `record_pending_admission` writes `admission.pending`; `admit_redemption` writes `admission.redeemed` or `admission.denied_for_reconciliation`; `advance_member_epoch` writes its reason. Event columns: `actor_principal_id` (the new principal or the signing-out member), `code_id`, `outcome_digest` = the payload digest for `admission.redeemed`, otherwise NULL.
5. **Hash encoding.** `hash`/`head_hash` are `sha256:<64 hex>` (the repository's digest form). Canonical event bytes are the forma-json-v1 object of every column except `hash`, keys in ASCII order, explicit nulls, `chain_sequence` as an integer, `occurred_at` stored truncated to whole seconds and encoded `YYYY-MM-DDTHH:MM:SSZ` in UTC (ADR-002's timestamp rule).
6. **Genesis sequence.** `audit_heads.chain_sequence` is `genui.safe_counter` (≥ 1) per D7.8, so a genesis head cannot hold 0: the fixture (as the operator package would) creates genesis heads at 1, and the first event of a chain is `chain_sequence = 2`. APP-077 must use the same convention, or D7.8 should say otherwise.
7. **Trial length.** `access_codes.trial_days` (plan column) is `CHECK (trial_days = 30)`; the entitlement is fixed at 30 days per D7.1/D7.17; `entitlements.kind` copies the code's kind (`trial` or `member_invite`).
8. **Template shape.** `code_grant_templates(permission, app_id, work_id, source_id)` with a per-permission shape CHECK, FKs to `apps`, `saved_work` and `sources`, and `UNIQUE NULLS NOT DISTINCT` instead of a primary key (work permissions need both app and work identifiers, so a single `resource_id` cannot be FK-checked).
9. **Redemption row.** `code_redemptions` adds `pending_admission_id` (UNIQUE, FK to the pending row) and `entitlement_id` (FK) so a replay returns exactly the stored result; `audit_event_id` has **no** FK because a referential check row-locks the referenced row, which needs UPDATE on `audit_events`, and D7.8 makes `audit_events` insert-only for every role (the owner's UPDATE, DELETE and TRUNCATE are revoked).
10. **Operator chain binding.** `genui_owner` holds INSERT on `audit_events` by ownership, so per D7.8 it receives RESTRICTIVE policies binding it to the `operator` chain and kinds, and to its own head.
11. **Replay epoch.** `replayed` returns the member's current epoch (the stored redemption has no epoch column).
12. **Added NOT NULL** on `audit_events.writer_role` and `audit_heads.writer_role` (the ADR's DDL omits it; a NULL would escape both unique constraints).
13. **Liveness.** A row is live while `state = 'live' AND expires_at > now()`; only `admit_redemption` changes an expired row's state (`denied_for_reconciliation`).

## Observations for the next tasks (not blocking 0003)

- **Entitlement reads for prechecks (Task 5).** D7.3 lists `entitlements` SELECT for workspace, control and data under later tickets; per the instruction that application grants arrive with their tickets, 0003 grants none. Task 5's planned `precheck` entitlement join therefore needs a migration of its own (or this slice's grant set amended) before it can run as those roles.
- **Duplicate pending admissions.** One identity can hold live pending admissions on several codes or twice on one code; each holds a slot until it is redeemed or expires (24 h). After the first redemption the others answer `already_member` and keep their slot until expiry. Two concurrent admissions of one identity through **different** codes serialize only on the principals unique key: the loser gets SQLSTATE 23505 (raised, not a typed outcome), which D7.9 already classifies inside storage.
- **STRICT with NULL constants.** A call whose arguments are all NULL constants is folded to NULL by the planner before the EXECUTE check, so any role "calls" the functions with NULLs and gets NULL; harmless, observed while writing the campaign (the EXECUTE denials use non-NULL arguments).
- **`managed_qualification.rs`** keeps the wire field name `catalog_v2_sha256` in its operator manifest (APP-075 track); it now carries the catalog-v3 digest. Renaming it changes that manifest's shape and is left to APP-075.
- **`infra/scripts/auth_schema_preflight.py`** (the executed APP-006 schema-one preflight) pins the previous `catalog-query.json` digest `e17ceb5e…` and `max_output_bytes == 262144`; re-emitting it from this checkout now refuses. It targets a version-1 database and was not changed.
- **Organization backfill.** The Consequences check that the two existing organizations have `organization_sequences` and `audit_heads` rows cannot hold at 0003 (`organization_sequences` is 0006's; the backfill is APP-077's) and is not asserted.
- **Names.** File and evidence directory follow the assignment (`0003_admission.sql`, `schema-0003/`); the plan says `0003_admission_ledger.sql` and `schema/`.
- **Process.** The 0003 SQL was drafted before the red run but kept outside the tree for it; the red run therefore exercised the extended campaign against the unchanged two-migration manifest (with only the re-pinned `catalog-query.json`).

## Blocking questions

None. Every 0003 item of ADR-007 was implementable as written; the interpretations above are listed for the independent review.
