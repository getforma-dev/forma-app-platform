# ADR-007 — Phase 1 write authority, schema layout and query execution

Date: 2026-10-29. Owner: GenUI. Status: accepted implementation direction pending its independent review (recorded in `evidence/APP-073/adr-007-review.md`); it authorizes no runtime result. It extends [ADR-004](ADR-004-TENANT-TRANSACTIONS.md) for the writes that APP-073, APP-009, APP-010, APP-012 and APP-013 must perform; every ADR-004 bound (two connections per role, eight total, 5 s acquisition, 3 s transaction, 2 s cleanup, FORCE RLS on every tenant relation, no raw client or SQL leaving the storage module, no automatic mutation retry) is unchanged. [ADR-003](ADR-003-PUBLIC-BETA-ACCESS.md), [ADR-005](ADR-005-PHASE-1-SEQUENCING.md) and [ADR-006](ADR-006-SIGNIN-AND-VERIFICATION-PENDING.md) remain controlling for their subjects. The [execution plan](../planning/PHASE-1-EXECUTION-PLAN.md) carries these items as D7.1–D7.17 and assigns them to tasks.

## Why a decision is needed

Migration 0002 grants the four application roles SELECT only; the only non-read privileges belong to `genui_lock`, which ADR-004 confines to fixed row-locking functions and forbids from owning mutation functions. ADR-004 also forbids direct application mutation of the grant tables and requires every membership change to take `organizations FOR SHARE` then the member row `FOR UPDATE` and advance that member's epoch in the same transaction. The remaining Phase 1 tickets must create principals, memberships, permissions, entitlements, sessions, definitions, publications, saved work, records, outcomes and audit rows, and must look up an access code before any organization scope exists. Without one recorded decision, each ticket would invent its own write path.

## Decisions

### D7.1 A dedicated definer principal for authority writes

Create `genui_admit` — `NOLOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS`, with no IAM membership and no application-role membership — owning exactly three `SECURITY DEFINER STRICT VOLATILE` functions with `SET search_path = pg_catalog, genui, pg_temp` and `PUBLIC` revoked:

| Function | Purpose | EXECUTE |
| --- | --- | --- |
| `genui_private.resolve_code(p_code_digest text) RETURNS uuid` | Returns the organization of a known code digest from `genui_private.code_index`, or NULL; reads nothing else. | `genui_workspace` |
| `genui_private.admit_redemption(p_org uuid, p_pending_admission uuid, p_issuer text, p_provider_tenant text, p_provider_subject text, p_payload_digest text) RETURNS TABLE(principal_id uuid, membership_epoch bigint, entitlement_id uuid, redemption_id uuid, outcome text)` | In one transaction: requires the transaction-local organization scope to equal `p_org`; locks `organizations FOR SHARE`; locks the pending admission and its `access_codes` row `FOR UPDATE`; rechecks expiry, revocation and remaining count; on an identical replay (same identity and payload digest) returns the stored redemption without consuming anything; otherwise inserts the principal, the membership at epoch 1, the code's grant template into the permission tables, the entitlement, the redemption and one audit event, and increments `redeemed_count`; `outcome` is `admitted`, `replayed`, `expired`, `revoked`, `exhausted` or `mismatch`. | `genui_workspace` |
| `genui_private.advance_member_epoch(p_org uuid, p_principal uuid, p_reason text) RETURNS bigint` | Locks `organizations FOR SHARE` then the membership `FOR UPDATE`, increments `membership_epoch`, writes one audit event (`session.sign_out`, `session.sign_out_all`, `entitlement.revoked` or `membership.revoked`) and returns the new epoch. | `genui_workspace` |

`genui_admit` holds column-limited INSERT on `principals`, `memberships`, the four `*_permissions` tables, `entitlements`, `code_redemptions` and `audit_events`, UPDATE(`redeemed_count`, `status`) on `access_codes`, UPDATE(`membership_epoch`) on `memberships`, and SELECT on the columns those functions read. `genui_lock` is unchanged. These functions perform no generic SQL, accept no permission names from callers (the grant template lives on the code row the operator wrote) and expose no bypass flag; a verified nonmember reaches `admit_redemption` only through the storage module's private nonmember entry, which installs organization scope for the definer call and never issues a member `Permit`.

### D7.2 Code lookup before organization scope

FORCE RLS keyed on `genui.organization_id` makes a code lookup impossible before scope is set. `genui_private.code_index(code_digest genui.digest PRIMARY KEY, organization_id uuid NOT NULL REFERENCES genui.organizations)` carries no RLS, is owned by `genui_owner`, and is readable only through `resolve_code`. The operator issuance command inserts the index row and the scoped `genui.access_codes` row in one transaction; revocation and expiry are judged on the scoped row after scope is set. An unknown digest returns NULL and the caller renders the same neutral answer as any other refusal.

### D7.3 Business write grants arrive with their owning ticket

Grants are added by the migration of the ticket whose transaction and audit implementation uses them, never earlier, and every new relation has `ENABLE` and `FORCE ROW LEVEL SECURITY` with the `tenant_scope` policy (`USING` and `WITH CHECK`) stated explicitly in that migration, because the 0002 loop covers only 0002's tables.

| Role | INSERT | UPDATE (column-limited) | DELETE |
| --- | --- | --- | --- |
| `genui_workspace` (APP-009, APP-010) | `prelogin_flows`, `pre_access_sessions`, `sessions`, `apps`, `definition_revisions`, `definition_inputs`, `definition_record_schemas`, `saved_work`, `record_collections`, `work_bindings`, `work_source_dependencies`, `audit_events` | `sessions`/`pre_access_sessions` (`last_seen_at`, `revoked_at`) | `prelogin_flows`, `pre_access_sessions`, `sessions` |
| `genui_control` (APP-010) | `publication_policies`, `publications`, `publication_inputs`, `publication_record_schemas`, `audit_events` | — | — |
| `genui_data` (APP-012, APP-013) | `internal_records`, `internal_record_revisions`, `record_source_dependencies`, `command_outcomes`, `query_receipts`, `audit_events` | `internal_records` (`current_version`), `organization_sequences` (`next_commit_sequence`) | — |
| `genui_worker` | none in Phase 1 | — | — |

No role ever receives UPDATE or DELETE on `audit_events`, or direct UPDATE on `memberships.membership_epoch`, `organizations.status` or `saved_work.version`; those stay with `genui_lock`'s fence columns and `genui_admit`'s functions. Permission-table rows are written only by `admit_redemption` and by the operator channel.

### D7.4 Adoption stays operator and test-only in Phase 1

No application write path bumps `saved_work.version` before PH03 (C10). APP-014's adoption race uses a test-only writer through the owner channel, exactly as the current campaign drives revocation.

### D7.5 Every new relation is organization-leading

Pre-login flows and pre-access sessions know their organization when they are created (from the sign-in handle or the code's server-bound destination) and are tenant-leading like every other `genui` relation; the callback confirms the organization from the verified token through the registry. Rate-limit counters are in-process for the single-task deployment and are a recorded limitation, not a table.

### D7.6 Isolation levels

`Transaction::begin_serializable` exists for the control role only and is used by publication writes with one retry, then 412/409 per design §6.2. Everything else stays `READ COMMITTED READ WRITE`.

### D7.7 Commit sequence

`genui.organization_sequences(organization_id uuid PRIMARY KEY REFERENCES genui.organizations, next_commit_sequence genui.safe_counter NOT NULL)` is advanced `FOR UPDATE` inside the command transaction after both fences. This serializes commands per organization; accepted for Phase 1 and revisited with APP-015's job model.

### D7.8 Audit

`genui.audit_events(organization_id uuid, event_id uuid, writer_role text CHECK (writer_role IN ('workspace','control','data')), kind text CHECK (kind IN (closed list)), actor_principal_id uuid NULL, app_id uuid NULL, work_id uuid NULL, record_id uuid NULL, code_id uuid NULL, outcome_digest genui.digest, occurred_at timestamptz NOT NULL, previous_hash text NOT NULL, hash text NOT NULL, PRIMARY KEY (organization_id, event_id))`, insert-only for every role, one SHA-256 chain per organization per writer role appended under `pg_advisory_xact_lock(hashtext(organization_id::text || writer_role))`; `hash = sha256(previous_hash || canonical event bytes)`. Object Lock anchoring stays with APP-053/057 (QUALITY-GATES); PH01's SR10 obligation is the insert-only chain and its verification.

### D7.9 Typed results instead of disclosing errors

`storage::Error` keeps its five variants. Admission returns `Ok(Admission)` with `pub enum Admission { Admitted { principal: String, epoch: i64, entitlement: String }, Pending { reason: PendingReason }, Refused }`; commands return `Ok(CommandResult)` with `pub enum CommandResult { Committed(CommandOutcome), Replayed(CommandOutcome), Conflict(RecoveryView), Stale(RecoveryView) }`. HTTP: `Committed`/`Replayed` → 200; `Conflict` → 409 with the recovery view; `Stale` → 412; a same-key request still in flight (unique-row wait past `lock_timeout`) → 409 with `Retry-After: 2`; `Error::Unavailable`/`Deadline` from a fence wait → 503 with `Retry-After: 2`; `Error::UnknownOutcome` → 503 and the client re-reads the outcome by key under current authority (ADR-002).

### D7.10 Query execution

Phase 1 runs typed queries through the qualified APP-005 DuckDB executor packaged into the runtime image (`duckdb==2.0.0` hash-locked; executor copied to `app/runtime/query_engine.py` with its source identity recorded) as `python3 /app/runtime.py query`: a per-request subprocess spawned by the Rust binary with a closed JSON request on stdin and exactly one JSON result line on stdout, killed at 1.8 s, DuckDB `threads=2`, `memory_limit='1GB'`, read-only, extension autoloading off, no httpfs, working directory `/run/genui/cache/o/<organization>/`. Source bytes are fetched once per revision by the helper from S3 key `o/<organization>/sources/<source_id>/<revision_id>` through the tenant role and verified against `source_revisions.normalized_digest` before use. The minimal Phase 1 queries are the Northstar `total` and `timeline` views, the only fixture views inside the qualified operator subset; CIDR lookup, ratio and money literals stay refused until PH02. The operator package normalizes the fixture CSV to the contract `events_v1` shape as Parquet through the reviewed APP-004/005 adapter and records real digests, replacing the seeded placeholders for the Northstar organization. This reuses the accepted support-process pattern; no DuckDB dependency enters the Rust binary.

### D7.11 Refresh-token custody

As ADR-006: encrypted with a random key held only in process memory, referenced from the session row, never sent to the browser, revoked through provider logout when the GenUI record is destroyed; a restart invalidates stored tokens and the user starts a fresh sign-in.

### D7.12 Migration numbering, runner and catalog budget

Migration numbers are assigned in landing order (planned 0003 admission, 0004 definitions/work grants, 0005 sessions, 0006 commands); a ticket that lands earlier renames its file first. The runner is generalized once to N versions before 0003: `migration_sources` accepts the manifest's N files, `apply_migrations` upgrades from any recorded version to the latest with a catalog proof after each step, and each migration `ALTER`s both version `CHECK`s to `BETWEEN 1 AND N`. Each version ships its `catalog-vN.json`; the generator refuses a snapshot above 209,715 bytes (80 % of the 262,144 cap), and if a version needs more, the cap moves coordinately in `catalog-query.json`, `catalog.rs` (`BOUND`) and `runtime.py` in that same slice. `scripts/check-schema-pins.py` fails unless the manifest, `runtime.py`, `pool.rs`, `catalog.rs` and `managed_qualification.rs` pins agree.

### D7.13 Harness retirement

`auth_qualification` compiles only behind Cargo feature `qualification-harness`, which release builds do not enable; release startup refuses `GENUI_AUTH_QUALIFICATION_CONFIG_FILE`; the product `auth` module owns `/auth/callback`. The published harness image `18c33f5` remains the artifact for the identity-only rehearsal.

### D7.14 Trusted proxy and limits

`axum::serve(..).into_make_service_with_connect_info::<SocketAddr>()`; the client address is the rightmost `X-Forwarded-For` hop because only the ALB can reach the task; in-process token buckets per address class, per account and global with the ADR-006 values; bodies ≤4 KiB on auth and admission forms, ≤64 KiB on JSON command and query routes; handler deadline 5 s.

### D7.15 HTTP contract

JSON routes live under `/v1/`, pages are unversioned; errors are `application/problem+json` with only a stable `code`; authenticated responses are `Cache-Control: no-store`; a fallback answers unknown paths with the uniform 404 and the security headers (design §9).

### D7.16 Idempotency key for pre-activation forms

The key travels as a typed prop rendered into a hidden field (static SSR routes refuse `config_json`); an active island mints its own key.

### D7.17 Admission defaults recorded before any code is issued

Trial entitlement 30 days; code expiry 14 days (operator maximum 30); `max_redemptions` default 1 (maximum 10); pre-signup reservation 10 minutes; post-signup pending admission 24 hours, bound to the provider identity, counted against remaining redemptions and committing nothing until an exchange reports `email_verified` true; list and audit pages 100 rows; resend cooldown 60 s and 5 per 24 hours.

## Consequences

APP-073 (0003), APP-010 (0004), APP-009 (0005) and APP-013 (0006) implement their migrations against this layout; APP-012 implements the query worker. The schema campaign (`qualify_tenant_schema.py`) gains assertions that every `genui` table has RLS, FORCE and the policy, that no role beyond the table above holds any write, that `genui_admit` holds exactly its grants and that `code_index` is reachable only through `resolve_code`. Nothing here weakens ADR-004's bounds, ADR-003's admission rules or ADR-006's session contract.
