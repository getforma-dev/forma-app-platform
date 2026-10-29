# ADR-009 — Phase 2 data plane, job authority and workspace presentation

Date: 2026-10-29. Owner: GenUI. Status: accepted implementation direction pending its independent review (`evidence/APP-015/adr-009-review.md`); it authorizes no runtime result. It decides what the seventeen PH02 tickets (APP-015–031) cannot decide alone. Evidence for every item is the [Phase 2 map](../../evidence/APP-015/ph02-map.md) (cited as "map §n"). It extends [ADR-007](ADR-007-PHASE-1-WRITE-AUTHORITY.md) and keeps every ADR-004 numeric bound; amendments are named **B1–B5** below. [ADR-008](ADR-008-YC-DEMO-AND-BETA-MVP.md) controls the target: Phase 2 serves the demo's acts 2–6 on the live invite-first beta, so the demo design's A6 reduction is the deployed profile and the platform design stays the code's design.

## Decisions

### D9.1 Parser packaging, concurrency and identity

The APP-004 parser ships as a **separate executable**, `parser-host`, built in the image from its own workspace and lockfile (`spikes/APP-004`, promoted to `parser/` by APP-017), exactly as D7.10 kept DuckDB out of the Rust binary. This avoids raising the app's Rust 1.85 floor to Wasmtime 36's 1.86 and the irreconcilable exact `serde`/`serde_json`/`sha2` pins (map §5.3 #14). The app spawns one `parser-host` process per import job, so every job has its own Wasmtime `Engine`, epoch deadline and memory; jobs never share an engine (map §5.5 #9). **Identity:** the image build compiles the guest with the pinned toolchain from the pinned sources; the allowlisted WASM digest is the digest that build produced, written into the image's `parser-manifest.json` and refused at startup if the file differs. Qualification binds sources, lockfile, vendor provenance and toolchain, not cross-host bytes, as `fixtures/qualify.py` already does, because the wasm32 module embeds absolute registry and checkout paths (APP-004 claims no cross-host byte identity).

### D9.2 Import profile and capacity on the beta task

The deployed profile is **demo A6**: CSV 10 MiB / 100,000 records, XLSX 5 MiB compressed / 50 MiB expanded, 320,000 aggregate cells; every other parser cap is the platform's (map §3.1). **Named reduction:** platform A4's 1,000,000 records is not reachable on the beta task and is not built; a streaming parser output protocol is out of PH02 scope. Capacity on the 2 vCPU / 8 GiB task: **one** import running per task and per organization (one 2 GiB parser fits the demo envelope, map §5.3 #2), a visible queue of 5 per organization and 20 per task, 429 with `Retry-After` past either cap.

### D9.3 Object layout, normalization and determinism (B1)

**B1 amends ADR-007 D7.10's object key** to the design's layout (map §5.3 #1): uploads stream to `o/{org}/staging/{upload_id}`; originals live at `o/{org}/s/{source}/r/{rev}/original`; normalized data at `o/{org}/s/{source}/r/{rev}/part.parquet`. The tenant role gains `s3:DeleteObject` on `o/${tenant}/staging/*` only; final keys are written once (copy from staging, then delete staging) and never deleted by the application, so a failed job leaves at most a staging object that the next upload or the operator sweep removes (map §5.1 #4). **Lifecycle:** parse produces an inspection report stored at `o/{org}/staging/{upload_id}.inspection.json`, not a revision; mapping confirmation enqueues a normalization job that writes `part.parquet`, then inserts the `source_revisions` row after both PUTs returned (ADR-002 object-before-row). A changed mapping is a new revision. **Writer:** `python3 /app/runtime.py normalize` with the pinned `duckdb==2.0.0`, sorted by the confirmed time column then original coordinate, row groups of 100,000, ZSTD. **Digests:** `content_digest` is SHA-256 of the original bytes; `normalized_digest` is `sha256("forma:normalized-rows:v1" ‖ 0x00 ‖ forma-json-v1 row stream in sorted order)`, independent of Parquet writer metadata; the Parquet file's own digest is recorded as `object_digest` and verified at fetch. **Column types:** text identifiers stay text; integers are `BIGINT` and a value outside int64 is shown as unknown with its original text (APP-018 "rejected or shown as unknown"); money is `BIGINT` minor units plus a currency column (USD, exponent 2, per the contract edition); timestamps are `TIMESTAMPTZ` UTC whole seconds.

### D9.4 Jobs and worker authority (B2)

`genui.jobs(organization_id, job_id, kind CHECK IN ('import','normalize'), state CHECK IN ('queued','running','failed','done','cancelled'), principal_id, membership_epoch_at_enqueue, idempotency_key, payload_digest, claim_sequence genui.safe_counter, lease_expires_at, failure_code NULL, result_digest NULL, created_at, updated_at, PRIMARY KEY (organization_id, job_id), UNIQUE (organization_id, principal_id, idempotency_key))`, FORCE RLS, created by the workspace role on enqueue (after the shared precheck including current entitlement). **Cross-organization claim:** the queue needs a pre-scope view, so `genui_private.job_queue(organization_id, job_id, available_at)` (no RLS, owned by `genui_owner`) is the third named exception to organization-leading relations (**B2** extends ADR-007 A2), readable only through `genui_private.claim_next_job(p_kinds text[]) RETURNS TABLE(organization_id uuid, job_id uuid, claim_sequence bigint)`, owned by a new definer `genui_jobs` (NOLOGIN, NOBYPASSRLS, attributes as `genui_admit`), EXECUTE to `genui_worker`, using `FOR UPDATE SKIP LOCKED`, a 120 s lease and fair round-robin across organizations. The worker then installs organization scope **and the enqueuing principal**, and the shared `precheck` accepts `Role::Worker` for job actions only, rechecking that principal's current membership, epoch and entitlement at claim and again before the result commits (map §5.1 #9). Lease renewal every 30 s and the result write compare `claim_sequence`; a stale claim's result is refused. Claim, result and one audit event commit in one transaction on a new **`worker` audit chain** (kinds `job.completed`, `job.failed`, `job.cancelled`; D7.8 amended). Cancellation before the irreversible step sets `cancelled`; after a committed result it reports `done`. Imports never take `organization_sequences`, so D7.7 is unchanged. The worker keeps its two connections.

### D9.5 Creator authority without permission-table writes

A new source and new saved work carry `created_by` (the principal), and the authorization SQL treats `created_by = principal` as `source_read` on that source and `work_read`/`work_write` on that work, in addition to permission rows. No permission row is written, so no epoch advances and no open display is invalidated (map §5.1 #8). Sharing with another member remains a permission-row write through the operator channel with ADR-004's epoch advance; membership, entitlement and epoch checks still gate the creator.

### D9.6 Contract edition `impl-…2`

APP-015 opens the edition (`evidence/APP-015/contract-edition/`), retaining every prior vector and reproducing the new ones in Rust and JavaScript: job state `cancelled`; `coverage` (`complete`, `partial`) on input contracts and source revisions; per-operator `{id, version}` and per-renderer `{kind, version}` identities in publications and receipts, replacing the single `operator_version`/`renderer_version` consts, with the matching 0002 CHECK migration; an `exact_rational` value `{numerator, denominator}` of ≤38-digit integers in lowest terms; a window-derived ratio denominator `{window_minutes}`; `partial` meaning partial coverage only (computed totals are never partial); `stale` meaning a display older than its authorization freshness (ADR-002); `mapping_version` in receipt source entries. Currency stays USD at exponent 2.

### D9.7 Query execution (B3)

Rust is the single source of truth for the grammar: admission and typing happen in `app/src/admission` and the query module, which compiles an admitted query into a closed JSON **plan** (operators and typed parameters, never SQL text); the Python executor maps the plan onto DuckDB's relational API with bound parameters; shared vectors check both sides. Execution follows D7.10's confined child (with APP-012's measurement deciding child-per-request versus APP-005's broker). **B3 amends design §6.3's admission for the beta task:** 2 running queries per task, 1 per organization, so a second organization always progresses (map §5.3 #3); queue 5 per organization, 20 per task; 429 with `Retry-After` beyond. The **result cache is off** in PH02 (C06 makes it optional; demo §6 says it earns nothing); the source cache keyed by revision object digest stays and is released only after authorization. Each engine child gets `RLIMIT_AS` 1.5 GiB, `RLIMIT_NOFILE` 64 and `RLIMIT_NPROC` 16 in addition to D7.10's controls; the absence of a network namespace inside Fargate is a recorded limitation (map §5.1 #15).

### D9.8 Receipts and record snapshots (B4)

APP-023's migration creates `genui.query_receipts(organization_id, receipt_id, digest UNIQUE per organization, canonical_receipt, cited_by_outcome NULL, created_at)`; receipts are stored only when cited by a committed record command or an explicit "keep" action, at most one per commit. **B4 amends D7.6:** a data-role read that joins record revisions with the commit sequence runs `REPEATABLE READ READ ONLY` so the records and `record_snapshot_sequence` come from one snapshot; read-only transactions cannot hit 40001 retries. Drilldown is a reproducible predicate with 500-row pages. Re-derivation of a cited receipt is an operator command (`verify-receipts`); a mismatch quarantines the revision.

### D9.9 Upload transport

`POST /v1/sources/uploads` streams the body: `Content-Length` is required and checked against the D9.2 cap before any byte is read; `Content-Type` must be `text/csv` or the XLSX media type; same-origin check plus the session's synchronizer token in a header; a required `Idempotency-Key`; streamed to `o/{org}/staging/{upload_id}` with SSE-KMS under the tenant key; the browser shows upload progress from its own request; never buffered whole in memory. The D7.14 body limits do not apply to this route; its own cap does.

### D9.10 Presentation schema

`genui.personal_view_state(organization_id, principal_id, work_id, layout jsonb ≤ 64 KiB, filters jsonb ≤ 16 KiB, updated_at)`, written by the workspace role for the session's own principal only and not audited; `genui.shared_views(organization_id, view_id, work_id, audience CHECK ('work_readers'), layout, filters, created_by, created_at)` with audit kind `view.shared`; `genui.aliases(organization_id, alias_normalized, target_kind, target_id, created_at, retired_at, UNIQUE (organization_id, alias_normalized) WHERE retired_at IS NULL)` with audit kinds `alias.created`, `alias.retired`; `apps.display_name`. All workspace-owned, FORCE RLS, grants in the owning ticket's migration.

### D9.11 Canvas budget

The canvas route's target moves from 28,100 B to the framework's enforced 28,420 B cap (ADR-001's open disposition is closed here for the canvas route only). Renderers beyond the canvas core load as one native group under ADR-001's 40,708 B Forma / 64 KiB ceiling. The donor's DOM code is not ported; APP-027 rebuilds the geometry model (numeric layout, pan, zoom, move, resize, group, fit, reset) on Forma 3.2 signals and measures each addition.

### D9.12 Deleted sources

Saved work whose dependency revision is deleted still opens: the work precheck no longer denies the whole work (`authorization.rs`), each affected view renders a tombstone with the historical source identity and digests, and a query against the deleted revision is refused with `source_deleted`; nothing rebinds to a newer revision (S06, C05).

### D9.13 Beta usage limits per entitlement

20 imports per day, 200 MiB of stored originals, 1,000 queries per hour, and the D9.2/D9.7 queues; exceeding a limit answers 429 with the limit's name. Values are recorded on the entitlement kind so the operator can change them without a migration.

### D9.14 Board amendment: split APP-027

APP-027 is split so canvas geometry, keyboard equivalence and personal layout depend on APP-003 and APP-010 only, and a new ticket carries linked-view integration on the canvas after APP-025. APP-028–030 follow the geometry slice; APP-031 keeps its dependencies. The Phase 2 plan's first task performs the split with renumbered orders, verbatim moved acceptance sentences and a board check.

### D9.15 Phase 2 trial environment and organizations

Per-ticket acceptance runs on local PostgreSQL 16.13 with TLS and local S3-compatible storage; the integrated PH02 trial runs on the live beta deployment once Victor approves its hosting figure, otherwise in a reserved window. Every PH02 migration then lands on a database with invited users: additive changes only, forward fixes, and a snapshot before each migration. Beyond the two fixture organizations, CDK provisions a pool of **8 trial organizations** (tenant role, KMS key at about $1/month each) that the operator assigns to invitations; more require a CDK change.

## Consequences

APP-015 opens the contract edition and the job tables and `genui_jobs`; APP-016 implements B1's layout, digests and staging; APP-017 promotes the parser executable; APP-020–024 implement D9.7 and B3; APP-023 creates `query_receipts` (B4); APP-026/031 implement D9.10; APP-027 is split first (D9.14). Each migration moves the version pins through `scripts/check-schema-pins.py` and stays inside the catalog budget of D7.12.
