# Phase 2 Execution Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Every task is one board ticket slice: move the ticket on the board at the start and end of the task (`Tickets/README.md`), keep an evidence directory, and obtain an independent review before Done.

Prepared 2026-10-29 by the GenUI owner (Claude) after reading [ADR-009](../architecture/ADR-009-PHASE-2-DATA-PLANE.md) as revised and re-reviewed, ADR-007 and ADR-008, the seventeen PH02 tickets (APP-015–031), the [PH02 page](../../Tickets/phases/PH02.md), the PH02 exit criteria in `Tickets/plan.json`, the PH02 row of the [quality gates](QUALITY-GATES.md), the [Phase 2 handoff](PHASE-2-HANDOFF.md), the [platform design](../../design/forma-generative-app-platform-v2/design.md) and [demo design](../../design/forma-northstar-demo/design.md) at the sections the tickets cite, and the [Phase 2 map](../../evidence/APP-015/ph02-map.md) ("map §n"), whose §6 task cut this plan starts from and corrects to the revised ADR-009. Status: plan, not evidence. Nothing here is implemented, deployed or accepted, and no PH02 ticket is eligible for selection today. The orchestrator is the GenUI owner and single GenUI operator; the FormaStack/fAuth lane is a separate Claude session (see [CROSS-TEAM-HANDOFF](CROSS-TEAM-HANDOFF.md)).

**Goal:** Close PH02: all eighteen tickets Done (APP-015–031 and APP-081, the linked-view slice split from APP-027 by D9.14) and an accepted `evidence/phases/PH02/GATE.md`, proving that invited members of the live beta import real CSV/XLSX by drop and by keyboard, confirm interpretation, get exact receipted results in six linked views, arrange a persistent canvas, submit general forms and return to saved or new work: the import, interpretation, canvas and form parts of the demo's acts 2–6 ([ADR-008](../architecture/ADR-008-YC-DEMO-AND-BETA-MVP.md)).

**Architecture:** PH01's single Rust business binary (axum 0.8, forma-server 3.2.0 SSR, four fixed-role pools with transaction-local tenant scope, FORCE RLS and definer fences) gains a durable PostgreSQL job queue reached only through the `genui_jobs` definer and a worker authority (`WorkerScope`) that rechecks the member fence at start and at result (D9.4); a per-job `parser-host` executable around the APP-004 WASM parser (D9.1); a confined `runtime.py normalize` child that writes Parquet from typed rows (D9.3); immutable original and normalized revisions under the B1 object layout in the tenant's KMS prefix (D9.3); a Rust-owned typed query grammar compiled to a closed JSON plan executed by DuckDB children under work-conserving admission (D9.7) with one-statement record snapshots and cited receipts (D9.8); and native SSR pages with canvas islands for source cards, linked views, renderers, forms and personal presentation (D9.10, D9.11). Per-ticket acceptance runs on local PostgreSQL 16.13 with TLS and local S3-compatible storage; the managed-only sentences and the integrated trial run on the managed bucket and the live beta (D9.15).

**Tech Stack:** Rust 1.93.1 toolchain with `rust-version = "1.85"` for the app; `parser/` as its own workspace and lockfile (Wasmtime 36.0.19, calamine 0.37.0, csv-core 0.1.13, quick-xml 0.42.0, zip 9.1.3 with the vendored manifest patch, serde 1.0.228, serde_json 1.0.145, sha2 0.10.9) outside the app's Cargo graph (D9.1); `forma-ir`/`forma-server` `=3.2.0`; axum 0.8; tokio-postgres 0.7.18, deadpool 0.13.1, rustls 0.23.46; serde `=1.0.229`, serde_json `=1.0.154`, ring `=0.17.14`, reqwest `=0.12.28`; Python 3.11 with psycopg 3.3.6, boto3 1.43.121 and `duckdb==2.0.0` hash-locked; `@getforma/core`/`@getforma/build`/`@getforma/compiler` 3.2.0 strict SSR; Playwright 1.64.1 (APP-003 harness) plus the accessibility checker APP-029 pins; PostgreSQL 16.13 (`postgres@sha256:5d143123…`); AWS CDK 2.275.0.

**Spec:** [ADR-009](../architecture/ADR-009-PHASE-2-DATA-PLANE.md) (D9.1–D9.15 and amendments B1–B23 control), [ADR-007](../architecture/ADR-007-PHASE-1-WRITE-AUTHORITY.md) (revised), [ADR-008](../architecture/ADR-008-YC-DEMO-AND-BETA-MVP.md), [ADR-004](../architecture/ADR-004-TENANT-TRANSACTIONS.md), [ADR-005](../architecture/ADR-005-PHASE-1-SEQUENCING.md), [ADR-006](../architecture/ADR-006-SIGNIN-AND-VERIFICATION-PENDING.md), the acceptance sentences of APP-015–031 verbatim, the PH02 exit criteria and quality-gate row, the platform design §§6.3–6.5, 7, 8 and 9, [SHARED-CONTRACTS](../../design/forma-generative-app-platform-v2/SHARED-CONTRACTS.md) C02–C07, C13, C15, the demo design §§4, 6, 7 and 9, and [SCENARIOS](../../design/forma-northstar-demo/SCENARIOS.md) S01, S02, S04–S06.

## Global constraints

Copied from the specs; every task inherits them.

- Toolchain pins: `cargo +1.93.1`, compiler floor `cargo +1.85.0 check --locked --lib --tests` for `app/`, Node 22.21.0 / npm 11.6.2, Python 3.11 venvs at `app/runtime/.venv` and `contracts/.venv`. `CARGO_BUILD_JOBS=2`; aggregate Cargo jobs on this Mac stay ≤3 and no heavyweight build runs concurrently with the FormaStack session's builds. `parser/` keeps its own `rust-toolchain.toml` (1.93.1, `wasm32-unknown-unknown`) and lockfile and never enters `app/Cargo.lock` (D9.1).
- Every dependency is an exact pinned release in its lockfile (`Cargo.lock`, `requirements.lock` with `--require-hashes`, `package-lock.json`); new dependencies are named in the owning ticket log with registry checksum and license. Commands a task introduces are recorded in its ticket log when they first run; this plan names only commands that exist.
- The root `.dockerignore` is an exact-file allowlist: every new Rust, TSX, CSS, SQL, catalog, parser or runtime file that the image needs is added there in the same commit, and a context export is retained in evidence.
- Schema version pins move together or every pool refuses admission; `scripts/check-schema-pins.py`'s own list is the authoritative pin list (D7.12). The catalog cap is `octet_length(body) <= 524288` with the generator's guard at 419,430 bytes (D7.12, moved in 0003); `catalog-v3.json` is 199,290 bytes. Migration numbers are assigned in landing order (D7.12); planned PH02 numbers, renamed at landing if the order differs: 0008 jobs (A1), 0009 sources (A3), 0010 personal view state (C2), 0011 navigation (C1), 0012 receipt quarantine (B4), 0013 record identity (C6), 0014 shared views and work creators (C7). Every PH02 migration is **additive only** (new relations, nullable or defaulted columns, broadened CHECKs, new grants, policies, functions and chain kinds; never narrowing, dropping, renaming or rewriting existing values), fixed forward, with a snapshot before each on any managed database (D9.15).
- Storage bounds are fixed: 2 connections per role, 8 total, 5 s acquisition, 3 s transaction, 2 s cleanup, `statement_timeout 2000`, `lock_timeout 1000`, READ COMMITTED READ WRITE outside the control role (D7.6), credential documents ≤600 s. Nothing retries a mutation automatically. No transaction spans a job's work, an object transfer or an engine run (D9.4, D9.8, D9.9).
- Design values: workspace session 12 h absolute / 1 h idle, `__Host-` HttpOnly Secure SameSite=Lax cookies; idempotency key ≥128 bits CSPRNG scoped to (organization, principal, work, command, key), outcome checked before expected version, outcomes retained 24 h, same key with different canonical payload → 409; error codes 404 uniform denial, 403 `cross_site`, 409 conflict with recovery view, 409 `busy`/`in_flight` with `Retry-After: 2` (D7.9), 412 stale, 413 `payload_too_large` (B17), 422 `narrow_the_query`, 429 with `Retry-After` naming the limit (D9.13), 503 only for provider/store unavailability; admission limits 24 views, 8 operations per view, 256 KiB definition, 500 rows per page; totals cover the full authorized filtered input before pagination and a failed calculation is an error, never a zero or a partial value.
- Import profile is demo A6 (D9.2): CSV 10 MiB and at most min(100,000, ⌊320,000 / columns⌋) records (64,000 for the five-column events, B5); XLSX 5 MiB compressed / 50 MiB expanded, 10,000 rows × 32 columns per sheet, 320,000 aggregate cells; guest 2 GiB linear memory, 120 s, 5,000,000,000 fuel, 128 MiB output; archive 512 entries / 16 sheets / 100,000 shared strings; text 2,000 characters / 8 KiB; formula policy `reject`. Limits only ever reduce.
- Jobs (D9.2, D9.4): kinds `import` and `normalize` share **one job slot per task** (B6); queue 5 per organization and 20 per task, 429 with `Retry-After` beyond either; lease 120 s, heartbeat 30 s; a row on its fourth claim fails `worker_lost` (dead letter); every step is its own transaction; the browser polls `GET /v1/jobs/{job}` every 500 ms while non-terminal and visible; "visible within 1 s" is a committed transition shown within 1 s at p50.
- Memory on the 2 vCPU / 8 GiB task (D9.2): binary 1.5 GiB; job slot 2.5 GiB (`parser-host` or the normalize child, never both); two query children 2.5 GiB with an RSS kill line at 1.25 GiB each; total 6.5 GiB; RSS sampled every 100 ms; a kill is `resource_limit`, never a partial value; a line moves only on a recorded measurement and within 7 GiB.
- Normalization (D9.3): parser output only, typing in Rust, `runtime.py normalize` with `memory_limit='1GiB'`, `threads=1`, 120 s, D7.10's confinement, rows sorted by the confirmed time column then original coordinate, row groups of 100,000, ZSTD, `duckdb==2.0.0`; `normalized_digest` = SHA-256 of the Parquet bytes; `rows_digest` = `sha256("forma:normalized-rows:v1" ‖ 0x00 ‖ forma-json-v1 rows in sorted order)`; integers `BIGINT` (outside int64 → unknown with original text), money `BIGINT` minor units plus currency (USD, exponent 2), timestamps `TIMESTAMPTZ` UTC whole seconds.
- Objects (B1): staging `o/{org}/staging/{upload_id}`, inspection `o/{org}/staging/{upload_id}.inspection.json`, original `o/{org}/s/{source}/r/{rev}/original`, normalized `o/{org}/s/{source}/r/{rev}/part.parquet`; object before row; explicit `x-amz-server-side-encryption: aws:kms` with the tenant key id on every PUT and copy; lifecycle: staging current 2 days, noncurrent 1 day, multipart 1 day; trial organizations current 90 days, noncurrent 7 days; fixture organizations' `s/` noncurrent 30 days.
- Queries (D9.7): Rust owns the grammar and compiles a closed JSON plan, never SQL text; filters only in POST bodies, a query string on a `/v1` query route is 400; ≤8 inputs; 2 queries per task, work-conserving, round-robin across organizations, supersede older epochs per work, queue 12 per organization and 20 per task; deadline 1.8 s, kill with APP-005's 5 s backstop; `threads=2`, `memory_limit='1GiB'`, `max_temp_directory_size='2GiB'`, `enable_external_access=false`, `lock_configuration=true`; `RLIMIT_NOFILE` 64 and `RLIMIT_CORE` 0 per child, no `RLIMIT_NPROC`, no `RLIMIT_AS`; result cache off; source cache keyed `(organization_id, normalized_digest)`; ratios as `exact_rational` of ≤38-digit integers, never engine `/`; an aggregate over any unknown value is `unknown_value_in_aggregate`; record inputs ≤100,000 rows and 64 MiB (D9.8).
- Upload (D9.9): `POST /v1/sources/uploads`, `Content-Length` checked before any byte is read, `Content-Type` `text/csv` or `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`, same-origin, synchronizer token header, `Idempotency-Key`, attestation header (B18); 120 s absolute with a 64 KiB/s floor over any 10 s window; D7.14's body limits and 5 s handler deadline do not apply to this route only (B19).
- Beta limits per entitlement (D9.13): 20 imports per day, 200 MiB of stored originals, 6,000 queries per hour; overrides only through `set-limits`; daily and storage counts durable, hourly count in-process (single task, D7.18).
- Canvas (B16): canvas route target 28,420 B Forma gzip (framework cap); renderers beyond the canvas core may load as one native group within 40,708 B Forma / 64 KiB; every geometry feature records its byte delta.
- Fixture semantics stay exact: Northstar 1,140,000 total / 240,000 matched / 900,000 unmatched / 114,000 bytes per minute / 90,000 unmatched per minute, destinations 900,000 / 120,000 / 120,000, minute buckets 60,000; 120,000; 300,000; 600,000; 0; 60,000; 0; 0; 0; 0 over [2026-10-22 10:00:00, 10:10:00) UTC with E6 excluded; ops-runner 210,000 / 30,000 / 21,000; Spend Review 64,000 cents total and 45,000 cents at `amount_cents >= 20,000`, USD exponent 2; boundaries 50 / 20 / 30 / 5; canary 40 / 17 / 23 / 4; S05 versions 0→1→2 with ik-1/ik-2 and stale ik-3 → 409.
- Identity: literal issuer `gatewasm`, audience `gatewasm-app`, access 900 s; `sub` equals `user.id`; provider identifiers map through the closed registry (≤32 bindings, restart to replace) to GenUI organization UUIDs; no fAuth token JSON or Set-Cookie reaches the browser; pre-access sessions create no job, reserve no slot and receive no result.
- Operations: ADR-003's $100/month cap governs every GenUI window until Victor approves the beta figure (ADR-008); the eight trial-organization keys add about $8/month to that figure (D9.15); single GenUI operator; no LLM call before APP-043 (suggestions in PH02 are deterministic); synthetic or non-sensitive uploads only (B18); GenUI schema changes never overlap a fAuth deployment operation; deployments are stop-then-start (D7.18).
- Done requires independent review, exact-source CI and landing on `origin/main`; local checks are evidence, never a CI pass.

## Review focus

Failure modes the specs imply but no ticket's acceptance sentence names; each is pinned to its owning task and test.

1. **The claim picks another organization's queued row while an expired running row exists** and collides on the unique running index, stalling the queue (re-review R-M1). Task A1: `expired_running_row_is_the_only_claim_candidate_across_two_organizations`.
2. **A stale worker's result is accepted** because the result step does not compare `claim_sequence` through `release_job`. Task A2: `reclaimed_job_refuses_old_worker_result`.
3. **A concurrent claimer's 23505 or 55P03 surfaces as `Unavailable`** (503) instead of "nothing claimed" (R-m6). Task A2: `concurrent_claimers_classify_unique_violation_and_lock_timeout_as_nothing_claimed`.
4. **`job_queue` or `job_cursor` becomes reachable without `genui_jobs`**, the third and fourth RLS exceptions (B2). Task A1: `job_scheduling_relations_reachable_only_through_genui_jobs`.
5. **The new nullable `audit_events` columns change existing hashes** (B21). Task A1: `existing_audit_chains_verify_unchanged_after_0008`.
6. **The worker compares the enqueue epoch**, so a sign-out stops a job, or **skips the current recheck**, so a revoked trial keeps importing. Task A2: `signout_keeps_job_running_but_revoked_entitlement_fails_it_authority_revoked`.
7. **A revision row commits before both object writes return**, or a staging object outlives 2 days. Task A3: `revision_row_only_after_parquet_put_and_original_copy_return`, `tenant_lifecycle_rules_match_d9_3` (CDK test).
8. **A deleted source silently rebinds or denies the whole work** (`authorization.rs:174`). Task A3: `deleted_source_reopens_work_with_tombstone_and_never_rebinds`.
9. **Two jobs run at once on the task, or two parser runs share one Wasmtime engine.** Task A5: `one_job_slot_shared_by_import_and_normalize`; Task A4: `parser_host_runs_one_job_per_process_with_cleared_environment`.
10. **The image ships parser bytes without a passing qualification record** (D9.1). Task A4: `release_refuses_parser_digest_without_passing_record`.
11. **The upload route buffers the body or reads before checking the declared size.** Task A7: `declared_length_over_cap_is_413_before_any_body_byte`, `upload_streams_without_whole_body_buffer`.
12. **DuckDB opens the original or types a value** during normalization (M5). Task A6: `normalize_child_receives_only_typed_rows`.
13. **Floating point enters a ratio or money total** (DuckDB `/`, `sum` widening). Task B2: `ratio_returns_exact_integers_and_reduces_in_rust`; Task B3: `money_sum_stays_integer_minor_units`.
14. **`LIMIT` or top-N applies before aggregation and changes a total.** Task B2: `pagination_and_top_n_never_change_totals`.
15. **The record snapshot sequence disagrees with the rows read** under a concurrent commit (X1). Task B4: `snapshot_sequence_and_record_rows_agree_under_concurrent_commit`.
16. **A cached source reaches an engine without reauthorization, or a result cache appears.** Task B5: `source_cache_reauthorizes_before_engine_and_result_cache_is_absent`.
17. **`RLIMIT_NPROC` or `RLIMIT_AS` is applied** and starves the app's threads or the engine's reservations. Task B5: `engine_child_limits_are_nofile_64_and_core_0_only`.
18. **Filter values travel in URLs** to edge logs. Task B1: `query_string_on_v1_query_route_is_400`.
19. **A late response is accepted on epoch alone**, without its organization and work context. Task B6: `delayed_old_filter_work_and_org_responses_never_rendered`; Task C3 repeats it on the canvas.
20. **A creator write advances an epoch or writes a permission row**, blanking the creator's open canvas (map §5.1 #8). Task A3: `creating_source_writes_no_permission_row_and_advances_no_epoch`; Task C7 for work.
21. **The catalog crosses 419,430 bytes or a migration is not additive** on the live database. Every migration task (A1, A3, B4, C1, C2, C6, C7): `scripts/check-schema-pins.py`, the generator guard, and the qualifier case `migration_is_additive_against_previous_catalog` that A1 adds; each task records the catalog size.
22. **Another principal reads a personal view state.** Task C2: `personal_view_state_is_visible_only_to_its_principal`.

## Decisions (ADR-009 controls)

[ADR-009](../architecture/ADR-009-PHASE-2-DATA-PLANE.md) records D9.1–D9.15 and amendments B1–B23 (B4 withdrawn), revised and re-reviewed on 2026-10-29 (review and dispositions in [adr-009-review.md](../../evidence/APP-015/adr-009-review.md) and the ADR's last two tables). This plan does not restate them: where a task below names a table, function, grant, limit, key, order or code, the ADR controls, and ADR-007 controls everything ADR-009 does not amend. The items most tasks consume: D9.1 `parser-host`; D9.2 profile, slot and memory budget; D9.3 object layout, normalization and digests; D9.4 `genui.jobs`, `job_queue`, `job_cursor`, `genui_jobs` and its five functions, the five transactions, `WorkerScope`, the worker grants and the `worker` chain; D9.5 `import_source`, creator authority and operator-only sharing; D9.6 the contract edition; D9.7 the plan compiler, operators, admission and caches; D9.8 the one-statement snapshot and cited receipts; D9.9 the upload route; D9.10 presentation relations; D9.11 the canvas budget; D9.12 deleted sources; D9.13 limits; D9.14 the board amendments; D9.15 the environments, the trial-organization pool and additive migrations; B15 the operator subcommands; B18 under Victor's veto.

## Phase 1 entry

PH02's integrated entry is "PH01 is accepted for integrated delivery" (`Tickets/plan.json`), and a ticket may start earlier only when its own predecessors are Done. Current PH01 state (2026-10-29): APP-001–005 and APP-007 Done; APP-006, APP-008 and APP-011 In review, with PR 1 to `main` awaiting Victor's merge (PH1 plan Task 1); migration 0003 merged (`e39c1bb`, PH1 Task 4) and under independent review; APP-073 admission operations in progress (PH1 Task 5); APP-010's admission module merged as preparation (`4c50f31`, PH1 Task 6 part 1) while its 0005 migration and `definitions.rs` are not started and the ticket is Backlog; APP-009, APP-012, APP-013, APP-014, APP-074, APP-075, APP-076, APP-077 and APP-078 not started. "Board" below means a ticket dependency; "operational" means a PH01 product the task consumes without a board edge.

| PH02 task | Waits on (PH1 plan task → ticket) | Board or operational | Earliest start today |
| --- | --- | --- | --- |
| 0.1 ADR-009 | — | — | Done |
| 0.2 APP-027 split | — | — | now |
| 0.3 contract edition | T6 part 1 (merged) | operational | now, as APP-015 preparation evidence |
| A1, A2 APP-015 | T1 (APP-008 landing), T4–T5 (APP-073), T7 (APP-009) | board | after T7 |
| A2 APP-015 | T8 (`http::problem`, `http::limits`, APP-074 local), B4 (APP-077 operator package) | operational | after T7, T8, B4 |
| A3 APP-016 | T1 (APP-008), T6 (APP-010); B4 (APP-077 `upload-sources`) | board; operational | after T6 |
| A4 APP-017 packaging | — (APP-004 Done) | — | now, as APP-017 preparation evidence |
| A5 APP-017 | A2, A3, A4 | board | after A2, A3 |
| A6 APP-018 | A5; T6 (`create_work` bindings) | board; operational | after A5 |
| A7 APP-019 | A5, A6; C2 (`personal_view_state`, placement); T7 (CSRF), T9 (work page) | board; operational | after A6, C2 |
| B1 APP-020 | A6; T9 (APP-012) | board | after A6, T9 |
| B2 APP-021, B3 APP-022 | B1; B2 | board | in series |
| B4 APP-023 | B3; T10, T11 (APP-013/014), including `genui.query_receipts` with D9.8's columns in 0007 | board | after B3, T11 |
| B5 APP-024 | A2, B2; T9 (engine child, broker decision) | board; operational | after A2, B2 |
| B6 APP-025 | B4, B5 | board | after B4, B5 |
| C1 APP-026 | T6, T7 (APP-010, APP-009) | board | after T7 |
| C2 APP-027 geometry | 0.2; T6 (APP-010) | board after 0.2 | after T6 |
| C3 APP-081 | B6, C2 | board after 0.2 | after B6 |
| C4 APP-028, C5 APP-029 | C2; C4; A7 for C5's import tasks | board; operational | in series |
| C6 APP-030 | C5; T10, T11 | board | after C5, T11 |
| C7 APP-031 | C1, C6 | board | after C6 |
| D0 pool and deployments | A3's CDK change; Victor's beta figure; FormaStack tenants | operational | after A3 |
| D1 integrated trial, D2 gate | every task above; PH01 gate passed (PH1 Track D: APP-076, `evidence/phases/PH01/GATE.md`) | board (phase) | after PH01 gate |

Two PH01 obligations come from ADR-009 and are not yet in the PH1 plan text: T10's migration 0007 creates `genui.query_receipts(organization_id, receipt_id, digest UNIQUE per organization, canonical_receipt, cited_by_outcome NULL, created_at)` (D9.8, M10), and APP-077's `upload-sources` writes the B1 key layout (B1). Both are recorded on those tickets when their tasks start; B4 and A3 check them on entry.

## Operating rules for the orchestrator

- Work one ticket slice at a time per set of files. Create a worktree per ticket from `phase-one-spikes` (`git worktree add ../forma-app-platform-APP-0NN phase-one-spikes`); merge back with a merge commit on the exact reviewed head. Another agent works only inside its own worktree under `.claude/worktrees`; never edit there.
- Board moves: Selected → In progress → In review → Done with dated `## Log` entries; run `python3 Tickets/_tools/board.py render && python3 Tickets/_tools/board.py check` after every move. A ticket may be Selected only when `board.py next` lists it. Preparation evidence (Tasks 0.3 and A4) may run before selection and is retained under the owning ticket's evidence directory; its board status moves only when `next` lists it.
- Each slice ends with a fresh independent reviewer (`Tickets/_tools/REVIEW.md`) who did not implement it; findings and fixes are retained under `evidence/APP-0NN/independent-review.md`.
- Local checks per slice (all that apply to the files touched): `cargo +1.93.1 fmt --check`, `cargo +1.93.1 clippy --locked --lib --tests -- -D warnings`, `cargo +1.93.1 test --locked --lib -- --test-threads=1`, `cargo +1.85.0 check --locked --lib --tests`, the Docker campaign `GENUI_NATIVE_REPORT=<abs path> cargo +1.93.1 test --locked --lib storage::qualification::native_postgres_campaign -- --ignored --exact --nocapture`, `app/runtime/.venv/bin/python -m unittest discover app/runtime`, `app/runtime/.venv/bin/python app/runtime/qualify_tenant_schema.py --output <json>`, `python3 scripts/check-schema-pins.py`, `contracts/.venv/bin/python -m unittest discover -s contracts -p 'test_*.py' -v`, `npm --prefix app/admin run typecheck` and `npm --prefix app/admin run build`, `docker build --tag <tag> --file app/Dockerfile .` with `python3 app/runtime/qualify_image.py --image <tag> --output <json>`, `python3 scripts/check-foundation.py`, and `python3 Tickets/_tools/board.py check`; plus the commands a task introduces (parser package tests, browser verification), recorded in its ticket log. Record exact commands and results; never claim CI.
- Commit at every green checkpoint. Push each slice branch so the workflows run on the exact head; landing on `main` is a reviewed PR that Victor merges unless he delegates merging.
- Cloud: read-only observation is free; any mutation (CDK change, bucket rule, managed-bucket check, migration, deployment) happens only inside a Victor-approved window with a reserved ledger entry, coordinated with the FormaStack owner, never overlapping a fAuth deployment operation, and recorded with request identity and bounded timeouts.

## Sequencing

```
Track 0:  0.1 ADR-009 (done) ── 0.2 APP-027 split ── 0.3 contract edition (APP-015 prep)
PH01:     T1 land 008/011 · T4–T5 APP-073 · T6 APP-010 · T7 APP-009 · T8 APP-074 · T9 APP-012 · T10 APP-013 · T11 APP-014 · B4 APP-077
Track A (S03):  A1 APP-015 schema ─► A2 APP-015 runtime ──┐
                A3 APP-016 (after T6) ────────────────────┼─► A5 APP-017 jobs ─► A6 APP-018 ─► A7 APP-019
                A4 parser-host (now, prep) ───────────────┘                                   ▲
Track B (S04):  A6 + T9 ─► B1 APP-020 ─► B2 APP-021 ─► B3 APP-022 ─► B4 APP-023 (T10/T11) ─┐   │
                A2 + B2 ──────────────────────────────► B5 APP-024 ──────────────────────────┴─► B6 APP-025
Track C (S05):  C1 APP-026 (after T6, T7) ────────────────────────────────────────────────────┐   │
                0.2 + T6 ─► C2 APP-027 geometry ─► C4 APP-028 ─► C5 APP-029 ─► C6 APP-030 ─┴─► C7 APP-031
                                  │                                (T10/T11)
                                  ├──────────────────────────────────────────────────────────────►A7
                                  └─► C3 APP-081 linked views on the canvas (after B6)
Track D:  D0 trial-organization pool + PH02 beta deployments ─► D1 integrated trial (after PH01 gate) ─► D2 PH02 GATE.md
```

With the split, the longest chain is `A1 → A2 → A5 → A6 → B1 → B2 → B3 → B4 → B6 → C3`, and the C2–C7 chain runs beside Track B once T6, T10 and T11 land.

---

## Track 0 — decisions and board

### Task 0.1: Record ADR-009 (done)

**Files:**
- Create: `docs/architecture/ADR-009-PHASE-2-DATA-PLANE.md`, `evidence/APP-015/adr-009-review.md`, `evidence/APP-015/ph02-map.md`

**Interfaces:**
- Consumes: ADR-002/003/004/007/008, design §§6.3–6.5, 7, 8, the map.
- Produces: the reviewed names every later task uses (D9.1–D9.15, B1–B23).

- [x] **Step 1: Draft ADR-009** from map §5.4 (`93a7ee2`).
- [x] **Step 2: Independent review and revision** (2 blockers, 14 majors, 11 minors, 14 map gaps dispositioned, `2057fbc`).
- [x] **Step 3: Re-review and fixes** (R-M1, R-m1–R-m6, labels B19–B23, `3f5ec2a`).

### Task 0.2: Split APP-027 per D9.14 and record the ticket amendments (first board task)

**Files:**
- Create: `Tickets/0-backlog/APP-081.md`
- Modify: `Tickets/0-backlog/APP-027.md`, `Tickets/0-backlog/APP-024.md`, `Tickets/0-backlog/APP-026.md`, `Tickets/0-backlog/APP-031.md` (logs), the `Order` row of every ticket whose order is 34 or more (APP-032–APP-072 and APP-075–APP-080), `Tickets/plan.json` (PH02 deliverable 3 and exit criterion 4 ticket ranges), `docs/planning/QUALITY-GATES.md` (PH02 row owning tickets), `docs/planning/PHASE-2-HANDOFF.md` (the APP-019/APP-027 selection row), the generated board views via `render`

**Interfaces:**
- Consumes: D9.14; ADR-008's labelling convention for moved and split sentences; `board.py` rules (unique orders, every dependency ordered earlier, ≥3 acceptance checks, requirement coverage).
- Produces: APP-027 "Port measured canvas interaction and structured alternative" with `Depends on` = `APP-003, APP-010`, order 29 unchanged; **APP-081 "Integrate linked views and evidence connections on the canvas"** (Epic UX, Priority P1, Type Feature, Sprint S05, Phase PH02, Order 34, Size M, Requirements `C03, C06, R7, S01`, `Depends on` = `APP-025, APP-027`); APP-028–031 orders 30–33 and dependencies unchanged; every ticket formerly at order n ≥ 34 at n + 1 (APP-032 → 35 … APP-080 → 81).

Decided here. APP-027's third acceptance line is split at its semicolon, each clause verbatim: APP-027 keeps "Reload restores permitted personal layout and display settings." and APP-081 receives "Drawn connections correspond to actual source/view/evidence relationships rather than invented causality. (Split from APP-027 by ADR-009 D9.14.)" because evidence relationships need APP-023's receipts and APP-025's linked views. APP-027's other two lines stay unchanged. APP-081's two further acceptance lines are new (no marker): "Linked view cards on one canvas share a declared filter and show its scope; a filter change re-queries every linked card at one context and filter epoch through APP-025's path, including after cards are moved, resized, grouped or fit." and "The linked-view task completes through keyboard navigation and the focused view on the same work state and epoch, and a late response from a prior filter, work or organization never renders in any canvas card." APP-027's §What changes drops "actual evidence connections"; APP-081's §What changes carries it with the linked-view integration.

- [ ] **Step 1: Write the failing board expectation**: a scratch script (not committed) asserting APP-081 exists with the fields above, APP-027 depends on `APP-003, APP-010`, orders are unique and contiguous 1–81, and the split clauses appear verbatim in both files.
- [ ] **Step 2: Run** it; expected: FAIL (APP-081 absent).
- [ ] **Step 3: Implement**: create APP-081 with all required sections (Why, What changes, Acceptance tests, Verification, Migration and rollback, Log) and a dated Log entry citing D9.14; edit APP-027 (dependency, §What changes, third acceptance line, Log); renumber orders with a mechanical edit of the `| Order |` row only; append dated Log amendments to APP-024 (sentence 3's cache wording is proved on the source cache and stored receipt lookup by digest; the result cache is off in PH02 and its wording is deferred, D9.7), APP-026 (PH02 suggestions are recents and permitted catalog entries with no model; no thumbnail renderer in PH02; semantic suggestions are APP-042's) and APP-031 ("adoption" is island adoption of server-rendered nodes, work-version adoption stays APP-040; "label/layout-only patch" is a `personal_view_state` patch, definition patches are APP-034's); add APP-081 to the PH02 ranges in `plan.json`, QUALITY-GATES and the handoff row (canvas geometry now waits on APP-003/010, linked views on APP-025/027). Acceptance sentences of APP-024/026/031 stay verbatim.
- [ ] **Step 4: Run** `python3 Tickets/_tools/board.py render && python3 Tickets/_tools/board.py check` and the scratch script; expected: `OK` and PASS; `git diff --stat` shows one changed line in each renumbered ticket.
- [ ] **Step 5: Independent review** of verbatim clauses, dependency edges, orders and amendment wording → `evidence/APP-027/board-split-review.md`; fix.
- [ ] **Step 6: Commit** (`Split APP-027 per ADR-009 D9.14 into canvas geometry and APP-081 linked views`).

### Task 0.3: Contract edition `impl-…2` (APP-015 preparation, D9.6)

**Files:**
- Create: `evidence/APP-015/contract-edition/README.md`
- Modify: `contracts/schema.py` (regenerate `contracts/schema.json`), `contracts/make_fixtures.py`, `contracts/fixtures/{northstar,spend-review,canonical-vectors,negative-vectors}.json`, `contracts/test_contracts.py`, `contracts/verify_vectors.mjs`, `contracts/implementation-lock.json` (`contracts/.venv/bin/python contracts/lock.py write`), `contracts/README.md`, `app/src/admission/{canonical,digest,validate,tests}.rs`

**Interfaces:**
- Consumes: edition `impl-2026-10-28.1` and its vectors; D9.6's list.
- Produces: edition `impl-<UTC date of the landing commit>.2` adding exactly D9.6's items: job state `cancelled`, `cancel_requested`, `queue_position`, `expected_wait_seconds`; `coverage` (`complete`, `partial`) on input contracts and source revisions; `rows_digest` on source revisions; per-operator `{id, version}` and per-renderer `{kind, version}` lists in the canonical publication envelope and in receipts (scalar `operator_version`/`renderer_version` stay as the registry edition, `≥ 1`); `exact_rational {numerator, denominator}` (≤38-digit integers, lowest terms); `{window_minutes}` denominators; `partial` = partial coverage; `stale` = display older than its authorization freshness; `mapping_version` in receipt source entries; problem codes `source_deleted`, `source_quarantined`, `unknown_value_in_aggregate`, `lookup_ambiguous`, `resource_limit`, `payload_too_large`, 429 bodies naming the D9.13 limit; parser refusal codes as job `failure_code`s; digest domains `forma:normalized-rows:v1` (D9.3) and `forma:job-payload:v1` for `genui.jobs.payload_digest` (D9.4 names the column but no domain; recorded as an erratum note in the edition README, no other change). Rust `admission::validate::admit_definition` accepts `.1` and `.2` documents; stored `.1` digests verify unchanged.

- [ ] **Step 1: Write failing tests**: `test_contracts.py` cases `test_edition_two_retains_every_edition_one_vector`, `test_exact_rational_lowest_terms_and_38_digit_bound`, `test_job_state_cancelled_and_queue_fields`, `test_receipt_operator_and_renderer_identity_lists`; Rust `admission::tests::edition_two_vectors_match_contracts_lock` and `edition_one_definitions_still_admit`.
- [ ] **Step 2: Run** the contracts unittest, `node contracts/verify_vectors.mjs` and `cargo +1.93.1 test --locked --lib admission::`; expected: FAIL (fields and domains absent).
- [ ] **Step 3: Implement** the schema, fixtures, vectors and lock, and the Rust validator and digest domains.
- [ ] **Step 4: Run** all local checks; expected: pass; Python, JavaScript and Rust produce identical digests for every vector.
- [ ] **Step 5: Independent review** (contract drift, prior-vector retention, Rust parity) → `evidence/APP-015/contract-edition/independent-review.md`; fix.
- [ ] **Step 6: Commit** (`feat(contracts): edition impl-….2 for jobs, coverage, rationals and per-operator identities (APP-015 prep)`).

---

## Track A — S03 real inspected evidence (APP-015–019)

### Task A1: APP-015 slice 1 — migration 0008 jobs, `genui_jobs` and worker authority schema

**Files:**
- Create: `app/migrations/0008_jobs.sql`, `app/migrations/catalog-v8.json` (generated), `evidence/APP-015/schema/README.md`
- Modify: `app/migrations/manifest.json`, `app/migrations/catalog-query.json` (eighth fixed role `genui_jobs`, B11; re-pinned), the version pins `scripts/check-schema-pins.py` lists, `app/runtime/qualify_tenant_schema.py` (eight-role normalization; new cases), `app/src/storage/qualification_cases.rs`, `.dockerignore`

**Interfaces:**
- Consumes: 0003's `audit_events`/`audit_heads`, `entitlements`, `code_grant_templates`; 0002's `lock_member_authority` (owner `genui_lock`), `organization_permissions`; D7.12 runner and pin script.
- Produces (all names per D9.4, D9.5, D9.13, B11, B12, B21, B22): `genui.jobs` with FORCE RLS and `tenant_scope`; `genui_private.job_queue` (partial index on queued rows, unique partial index on `state = 'running'`) and `genui_private.job_cursor`; role `genui_jobs` created like `genui_admit` with its grants; functions `genui_private.enqueue_job(p_org uuid, p_job uuid, p_kind text) RETURNS TABLE(outcome text, queue_position integer, ahead_in_task integer)`, `claim_next_job(p_kinds text[]) RETURNS TABLE(organization_id uuid, job_id uuid, claim_sequence bigint, attempts smallint)`, `renew_job(p_org uuid, p_job uuid, p_claim bigint) RETURNS TABLE(renewed boolean, cancel_requested boolean)`, `release_job(p_org uuid, p_job uuid, p_claim bigint) RETURNS TABLE(released boolean, cancel_requested boolean)`, `request_job_cancel(p_org uuid, p_job uuid) RETURNS text` (`cancelled` | `requested` | `absent`); the B10 grant rows for `genui_worker` and `genui_workspace` on `genui.jobs`, `job_queue` functions, `memberships`, `organization_permissions`, `source_permissions`, `entitlements`, `genui.entitlement_limits`, `audit_events`, `audit_heads` and EXECUTE on `lock_member_authority` (issued as `genui_lock`), `claim_next_job`, `renew_job`, `release_job`, `enqueue_job`, `request_job_cancel`; the grant rows on `sources` and `source_revisions` are A3's; `organization_permissions.permission` CHECK broadened to `admin_policy` or `import_source`, `code_grant_templates` accepting `import_source`; `genui.entitlement_limits(organization_id, entitlement_id, imports_per_day, stored_original_bytes, queries_per_hour)` FORCE RLS, read by workspace and data; `audit_events.job_id uuid NULL`, `source_id uuid NULL` entering canonical bytes only when non-NULL; `writer_role` CHECK broadened to `worker` with kinds `job.completed`, `job.failed`, `job.cancelled`, workspace kinds `job.enqueued`, `job.cancel_requested`, `job.cancelled`, operator kinds `permission.granted`, `permission.revoked`, `limits.set`; the `worker` genesis head for every existing organization under its scope; `publications` `operator_version`/`renderer_version` CHECKs broadened to `>= 1` (B22); version CHECKs `BETWEEN 1 AND 8`.

- [ ] **Step 1: Write failing schema tests** in `qualify_tenant_schema.py`: `jobs_relation_has_rls_force_policy_and_org_leading_key`, `job_scheduling_relations_reachable_only_through_genui_jobs`, `genui_jobs_role_attributes_and_grants_match_d9_4`, `grant_set_equals_d7_3_plus_d7_1_plus_b10`, `every_audit_insert_role_has_restrictive_policies_for_six_chains`, `existing_audit_chains_verify_unchanged_after_0008`, `migration_is_additive_against_previous_catalog` (compares `catalog-v7` and `catalog-v8`: no dropped relation, column, grant, policy or narrowed CHECK); campaign cases in `qualification_cases.rs`: `enqueue_caps_five_per_organization_twenty_per_task_under_cursor_lock`, `expired_running_row_is_the_only_claim_candidate_across_two_organizations`, `claim_increments_sequence_and_attempts_and_sets_120s_lease`, `release_with_stale_claim_sequence_releases_nothing`, `request_job_cancel_deletes_queued_and_flags_running`.
- [ ] **Step 2: Run** the qualifier and the campaign; expected: FAIL (relations absent).
- [ ] **Step 3: Write 0008**, regenerate catalogs (fresh and upgrade paths byte-identical), update the manifest, `catalog-query.json`, the pins and `.dockerignore`.
- [ ] **Step 4: Run** all local checks; expected: pass; record `catalog-v8.json` size (< 419,430 bytes).
- [ ] **Step 5: Independent review** of the SQL (definer bodies, the running-index claim rule, grants, chain hashes, additivity) → `evidence/APP-015/schema/independent-review.md`; fix.
- [ ] **Step 6: Commit** (`feat(schema): migration 0008 jobs, genui_jobs and worker authority (APP-015)`).

### Task A2: APP-015 slice 2 — enqueue, claim, start, heartbeat, result, cancel and routes

**Files:**
- Create: `app/src/storage/jobs.rs`, `app/src/storage/worker_scope.rs`, `app/src/jobs/mod.rs`, `app/src/jobs/runner.rs`, `app/src/jobs/routes.rs`, `app/src/jobs/tests.rs`, `app/runtime/jobs_operator.py`, `app/runtime/permission_operator.py`, `app/runtime/test_jobs_operator.py`, `app/runtime/test_permission_operator.py`, `evidence/APP-015/README.md`
- Modify: `app/src/storage/mod.rs` (declare, re-export), `app/src/storage/authorization.rs` (`Action::ImportSource` ↔ `import_source`; `Resource::Organization` accepts it; `precheck` admits `Role::Worker` only through `WorkerScope`), `app/src/storage/pool.rs` (`TestHooks` phases `enqueue`, `claim`, `start`, `renew`, `result`), `app/src/storage/qualification.rs` (campaign groups), `app/src/lib.rs` (`pub mod jobs;`), `app/src/main.rs` (spawn the runner; mount routes), `app/operator/operator.py` (closed subcommands `jobs --stuck`, `grant-permission`, `revoke-permission`, `set-limits`), `.dockerignore`

**Interfaces:**
- Consumes: A1's functions and grants; `Workspace::resolve_session -> TrustedRequest` (T7); `Transaction::{begin, precheck, fence, finish}` with the entitlement join (T5); `http::problem::Problem`, `http::limits` (T8); D7.9's SQLSTATE classification; the operator channel (D7.19).
- Produces:
  - `pub enum JobKind { Import, Normalize }`; `pub enum JobState { Queued, Running, Failed, Done, Cancelled }`; `pub enum Limit { OrganizationQueue, TaskQueue, ImportsPerDay, StoredOriginalBytes, QueriesPerHour }`.
  - `pub struct NewJob { pub kind: JobKind, pub idempotency_key: String, pub payload: serde_json::Value }` (`payload_digest = digest("forma:job-payload:v1", canonical(payload))`).
  - `pub struct JobView { pub job: String, pub kind: JobKind, pub state: JobState, pub claim_sequence: Option<i64>, pub queue_position: Option<u32>, pub expected_wait_seconds: Option<u32>, pub cancel_requested: bool, pub failure_code: Option<String>, pub result_digest: Option<String> }` serialized as the edition-2 `job_state`.
  - `pub enum EnqueueResult { Queued(JobView), Replayed(JobView), Conflict, Busy { limit: Limit, retry_after_seconds: u32 } }`; `pub enum CancelResult { Cancelled(JobView), Requested(JobView), Terminal(JobView) }`.
  - `impl Workspace { pub async fn enqueue_job(&self, request: &TrustedRequest, job: &NewJob) -> Result<EnqueueResult>; pub async fn read_job(&self, request: &TrustedRequest, job: &str) -> Result<JobView>; pub async fn cancel_job(&self, request: &TrustedRequest, job: &str) -> Result<CancelResult> }` (D9.4 step 1, the enqueue limits of D9.13, read authority per R-m5).
  - `pub struct Claim { organization: String, job: String, claim_sequence: i64, attempts: i16 }` with getters; `pub enum Started { Running(WorkerJob), DeadLettered, Revoked }`; `pub struct WorkerJob { pub kind: JobKind, pub payload: serde_json::Value, pub principal: String }`; `pub struct Renewal { pub renewed: bool, pub cancel_requested: bool }`; `pub enum JobOutcome { Done { result_digest: String, revision: Option<NewRevision> }, Failed { failure_code: String } }`; `pub enum Finished { Recorded(JobState), Cancelled, Fenced, Revoked }`.
  - `impl Worker { pub async fn claim(&self, kinds: &[JobKind]) -> Result<Option<Claim>>; pub async fn start(&self, claim: &Claim) -> Result<Started>; pub async fn renew(&self, claim: &Claim) -> Result<Renewal>; pub async fn finish(&self, claim: &Claim, outcome: JobOutcome) -> Result<Finished> }` (D9.4 steps 2–5; `NewRevision` is A3's and `finish` refuses `revision: Some` until A3 lands).
  - `pub(super) struct WorkerScope` built only inside `storage` from a `Claim`: one statement reads the job's principal and that member's current epoch and takes `lock_member_authority`; never a `TrustedRequest`.
  - `jobs::runner::spawn(stores: Arc<Stores>, handlers: Handlers) -> tokio::task::JoinHandle<()>` with `#[async_trait] pub trait JobHandler { async fn run(&self, job: &WorkerJob, control: &JobControl) -> JobOutcome }` and `JobControl::{cancel_requested(&self) -> bool}`; idle claim poll 250 ms, heartbeat 30 s, a refused renewal stops the handler.
  - Routes: `GET /v1/jobs/{job}` → 200 `job_state` | 404; `POST /v1/jobs/{job}/cancel` (same-origin, synchronizer token) → 200 `job_state` | 404. `EnqueueResult::Busy` maps to 429 with `Retry-After` and the limit name; `Conflict` to 409.
  - Operator: `operator.py jobs --stuck` (running rows with expired leases, queued rows older than 10 minutes; read-only); `grant-permission` / `revoke-permission --organization <uuid> --principal <uuid> --permission {record_read,record_write,source_read,import_source} [--resource <uuid>]` under ADR-004:34's fence with epoch + 1 and `permission.granted`/`permission.revoked`; `set-limits --organization <uuid> --entitlement <uuid> [--imports-per-day N] [--stored-original-bytes N] [--queries-per-hour N]` with `limits.set`.

- [ ] **Step 1: Write failing tests** mapped to APP-015's sentences. Sentence 1: `enqueue_and_claim_durable_job_with_finite_lease_and_claim_sequence`, `reclaimed_job_refuses_old_worker_result`, `concurrent_claimers_classify_unique_violation_and_lock_timeout_as_nothing_claimed`. Sentence 2: `org_and_task_queue_caps_return_429_naming_limit_without_losing_jobs`, `states_queued_running_failed_done_cancelled_readable_with_position_and_wait`, `fourth_claim_fails_worker_lost`. Sentence 3: `cancel_while_queued_reports_cancelled_and_frees_queue`, `cancel_while_running_stops_before_irreversible_step`, `cancel_after_result_reports_done_history_intact`. Sentence 4: `signout_keeps_job_running_but_revoked_entitlement_fails_it_authority_revoked`, `expired_or_revoked_trial_denied_at_enqueue_start_and_result_both_orders`, `pre_access_and_anonymous_requests_create_no_job`, `cancelled_or_refused_job_releases_its_queue_reservation_and_keeps_committed_audit`, `job_read_is_uniform_404_for_other_principals_without_import_source`; operator unit tests for the four subcommands with a disposable PostgreSQL container.
- [ ] **Step 2: Run** the campaign and runtime unit tests; expected: FAIL (`Workspace::enqueue_job` undefined).
- [ ] **Step 3: Implement** `jobs.rs` (the five transactions), `worker_scope.rs`, the runner, routes and operator subcommands; the runner's handlers table is empty in this slice except a test-only handler.
- [ ] **Step 4: Run** all local checks; expected: pass; record claim and transition latencies.
- [ ] **Step 5: Independent review** (worker authority, fencing, cancellation chains, disclosure) → `evidence/APP-015/independent-review.md`; fix; move APP-015 to In review.
- [ ] **Step 6: Commit** (`feat(jobs): durable jobs, leases, cancellation and worker authority (APP-015)`).

### Task A3: APP-016 — immutable original and normalized revisions, tombstones, storage controls

**Files:**
- Create: `app/migrations/0009_sources.sql`, `app/migrations/catalog-v9.json`, `app/src/storage/sources.rs`, `app/src/sources/mod.rs`, `app/src/sources/objects.rs`, `app/src/sources/cache.rs`, `app/src/sources/routes.rs`, `app/src/sources/tests.rs`, `app/runtime/objects.py`, `app/runtime/test_objects.py`, `app/runtime/creator_operator.py`, `app/runtime/purge_operator.py`, `app/runtime/test_creator_operator.py`, `app/runtime/test_purge_operator.py`, `evidence/APP-016/README.md`, `evidence/APP-016/managed/README.md`
- Modify: `app/runtime/runtime.py` (modes `object-put`, `object-copy`, `object-delete`, `object-get`; the `cache` fetch reads B1 keys), `app/src/storage/authorization.rs` (source-read-or-creator; work precheck keeps deleted dependencies, D9.12), `app/operator/operator.py` (`upload-sources` writes B1 keys and `--relayout` copies D7.10-key objects byte-identically; closed subcommands `revoke-creator-authority`, `purge-organization-objects`), `infra/src/stack.ts` (tenant-role `s3:DeleteObject` on `o/${tenant}/*`; per-tenant lifecycle rules of D9.3), `infra/test/security.test.ts`, the version pins, `qualify_tenant_schema.py`, `.dockerignore`

**Interfaces:**
- Consumes: A1/A2 (`Worker::finish`, `WorkerScope`, chains); `genui.sources`, `source_revisions`, `work_bindings`, `work_source_dependencies` (0002); `admission::digest`; T9's source cache directory `/run/genui/cache/o/<organization>/`; the tenant-role session of `runtime.py`.
- Produces:
  - 0009: `sources.created_by uuid NULL`, `creator_revoked_at timestamptz NULL`, `display_name text NULL`, composite FK `(organization_id, created_by) → memberships` and RESTRICTIVE `WITH CHECK (created_by = nullif(current_setting('genui.principal_id', true), '')::uuid)`; `source_revisions.rows_digest genui.digest NULL`, `original_bytes bigint NULL`, a RESTRICTIVE insert policy for `genui_worker` requiring both; B10 rows: `genui_workspace` INSERT on `sources` and UPDATE(`deleted_at`) on `source_revisions`, `genui_worker` SELECT on `sources`, `source_revisions` and INSERT on `source_revisions`; workspace kind `source.deleted`, operator kinds `creator_authority.revoked`, `organization.objects_purged`.
  - `sources::objects::{staging_key(org: &str, upload: &str) -> String, inspection_key(org: &str, upload: &str) -> String, original_key(org: &str, source: &str, rev: &str) -> String, parquet_key(org: &str, source: &str, rev: &str) -> String}`; `sources::objects::put(org: &str, key: &str, body: impl AsyncRead, length: u64) -> Result<PutReceipt { sha256: String, length: u64 }, ObjectError>`, `copy(org, from, to) -> Result<(), ObjectError>`, `delete(org, key) -> Result<(), ObjectError>`, each one bounded `runtime.py object-*` call (S3 1 s connect / 30 s, KMS 1 s / 5 s; a timeout is a refusal).
  - `pub struct NewRevision { pub source: String, pub revision: String, pub mapping_version: i64, pub content_digest: String, pub normalized_digest: String, pub rows_digest: String, pub contract_digest: String, pub original_bytes: i64, pub canonical_envelope: Vec<u8> }` inserted by `Worker::finish` only after `put` of `part.parquet` and `copy` of the original returned; staging deleted after commit.
  - `impl Workspace { pub async fn read_source(&self, request: &TrustedRequest, source: &str) -> Result<SourceView>; pub async fn delete_source(&self, request: &TrustedRequest, source: &str) -> Result<DeleteResult> }` (creator or `admin_policy`); `sources::cache::evict(organization: &str, normalized_digest: &str) -> std::io::Result<()>` after commit; `POST /v1/sources/{source}/delete` → 200 | 404.
  - Work reads: a deleted dependency revision yields a per-view tombstone `{source_id, revision_id, content_digest, normalized_digest, deleted_at}` and a query against it is refused `source_deleted`; a revision whose object is missing is treated as deleted.
  - Operator: `revoke-creator-authority --organization --principal [--source <uuid> | --work <app>/<work>]` (ADR-004:34 order, epoch + 1); `purge-organization-objects --organization <uuid>` (tombstones every revision, deletes objects, `organization.objects_purged`; B18 default).

- [ ] **Step 1: Write failing tests** mapped to APP-016's sentences. Sentence 1: `revision_row_only_after_parquet_put_and_original_copy_return`, `revision_links_parser_and_mapping_versions_and_digests`, `worker_insert_without_rows_digest_or_original_bytes_refused`. Sentence 2: `failed_object_write_leaves_no_usable_revision_and_only_an_expiring_staging_object`, `identical_bytes_in_two_organizations_never_cross_read` (canary 40-byte values), `creator_authority_grants_source_read_only_on_own_source`, `creating_source_writes_no_permission_row_and_advances_no_epoch`. Sentence 3: `deleted_source_reopens_work_with_tombstone_and_never_rebinds`, `delete_evicts_cached_revision_and_frees_storage_quota`, `missing_object_is_treated_as_deleted`; CDK `tenant_role_may_delete_but_not_delete_versions`, `tenant_lifecycle_rules_match_d9_3`; operator tests for relayout, revoke and purge.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement** 0009 and pins, `sources.rs`, objects, cache eviction, the authorization changes, the operator subcommands and the CDK change.
- [ ] **Step 4: Run** all local checks against local PostgreSQL and local S3-compatible storage; expected: pass; record `catalog-v9.json` size.
- [ ] **Step 5: Run the managed-only sentences** (D9.15, m10) inside a Victor-approved window, cents, no running task: deploy the CDK change, then prove APP-016:38–39's tenant isolation, the IAM copy/delete with explicit SSE-KMS and the lifecycle rules against the managed bucket; retain receipts under `evidence/APP-016/managed/`.
- [ ] **Step 6: Independent review** (object-before-row, tenant isolation, tombstones, creator authority, lifecycle) → `evidence/APP-016/independent-review.md`; fix; move APP-016 to In review; commit (`feat(sources): immutable original and normalized revisions with tombstones (APP-016)`).

### Task A4: APP-017 slice 1 — `parser-host` packaging and image-bytes qualification (preparation, D9.1)

**Files:**
- Create: `parser/` copied from `spikes/APP-004` at a recorded commit (`parser/Cargo.toml`, `parser/Cargo.lock`, `parser/rust-toolchain.toml`, `parser/.cargo/config.toml` with `--remap-path-prefix` for `CARGO_HOME` and the checkout, `parser/guest/`, `parser/host/`, `parser/vendor/`, `parser/fixtures/`), `parser/host/src/job.rs`, `app/src/supervise.rs`, `app/src/imports/mod.rs`, `app/src/imports/sandbox.rs`, `scripts/check-parser-record.py`, `scripts/test_check_parser_record.py`, `.github/workflows/parser-release.yml`, `evidence/APP-017/packaging/README.md`
- Modify: `app/Dockerfile` (stage building the guest and native `parser-host` from `parser/` with its lockfile; image file `parser-manifest.json`), `app/src/lib.rs` (`pub mod imports; pub mod supervise;`), `app/runtime/qualify_image.py` (parser artifacts present, digests match the manifest), `.dockerignore`
- Unchanged: `spikes/APP-004/` (evidence links depend on it)

**Interfaces:**
- Consumes: the APP-004 guest ABI and host `Sandbox::{new, run}`, `Limits`, `RunReport`, refusal classes; `fixtures/qualify.py`.
- Produces:
  - `parser-host --job`: stdin is a 4-byte big-endian config length, the config JSON (`{profile: "demo", format, sheet?}`, ≤4,096 B) and the input bytes to EOF; stdout is the guest's one JSON document; stderr is one `RunReport` line; exit 0 parsed, 2 refused, 1 failure; one `Engine` per process. The guest copy returns `available_sheets` with the `sheet_selection_required` refusal after the workbook-wide preflight (the only guest change; it requalifies the guest bytes).
  - `supervise::Confinement { rss_limit_bytes: u64, deadline: Duration, nofile: u64, core: u64 }`; `supervise::run_confined(command: tokio::process::Command, stdin: impl AsyncRead, confinement: &Confinement) -> Result<ConfinedOutput, Killed>` with `env_clear()`, pipes only, RSS sampled every 100 ms, `pub enum Killed { Deadline, ResourceLimit, Crashed }`.
  - `imports::sandbox::parse(input: impl AsyncRead, config: &ParseConfig, deadline: Duration) -> Result<ParseOutput, ParseRefusal>` with `ParseConfig { format: Format, sheet: Option<String> }` (profile fixed to `demo`), slot line 2.5 GiB, deadline 120 s plus 5 s backstop; `imports::sandbox::verify_manifest() -> Result<ParserIdentity>` at startup (integrity only).
  - CI: `parser-release.yml` builds the image, extracts `parser_guest.wasm`, runs the APP-004 hostile suite and `fixtures/qualify.py` against those bytes and uploads `evidence/APP-017/parser-records/<sha256>.json`; `scripts/check-parser-record.py --wasm-sha256 <hex>` exits non-zero unless a passing record for that digest exists; the release preflight (APP-078's procedure) runs it.

- [ ] **Step 1: Write failing tests**: `parser/host` job-mode tests (`job_mode_reads_framed_config_and_input`, `sheet_selection_required_lists_sheets_after_preflight`), the ported APP-004 qualification suite against the job binary, `supervise::tests::killed_at_rss_line_reports_resource_limit`, `killed_at_deadline_reports_deadline`, `child_environment_is_empty`, `imports::sandbox::tests::parser_host_runs_one_job_per_process_with_cleared_environment`, `scripts/test_check_parser_record.py::test_release_refuses_parser_digest_without_passing_record`.
- [ ] **Step 2: Run** them; expected: FAIL.
- [ ] **Step 3: Implement** the copy, job mode, supervisor, sandbox wrapper, Dockerfile stage, record check and workflow.
- [ ] **Step 4: Run** all local checks, the parser package tests, the image build and `qualify_image.py`; expected: pass; record the guest digest and that the extracted image bytes pass the hostile suite.
- [ ] **Step 5: Independent review** (identity chain from source to image bytes, confinement, guest change) → `evidence/APP-017/packaging/independent-review.md`; fix.
- [ ] **Step 6: Commit** (`feat(imports): parser-host packaging and image-bytes qualification (APP-017 prep)`).

### Task A5: APP-017 slice 2 — import jobs: inspect, refuse, cancel, recover

**Files:**
- Create: `app/src/imports/inspect.rs`, `app/src/imports/import_job.rs`, `app/src/imports/routes.rs`, `app/src/imports/tests.rs`, `evidence/APP-017/README.md`
- Modify: `app/src/jobs/runner.rs` (register `JobKind::Import`; the slot runs one job at a time), `app/src/storage/sources.rs` (`Workspace::enqueue_import`), `app/src/main.rs`, `.dockerignore`

**Interfaces:**
- Consumes: A2 (`Workspace::enqueue_job`, `Worker::*`, runner), A3 (objects, `sources` insert), A4 (`imports::sandbox::parse`).
- Produces:
  - `pub struct StagedUpload { pub upload: String, pub content_digest: String, pub format: Format, pub byte_length: u64, pub display_name: String, pub attestation: String }`; `impl Workspace { pub async fn enqueue_import(&self, request: &TrustedRequest, staged: &StagedUpload, sheet: Option<&str>, idempotency_key: &str) -> Result<(String /* source */, EnqueueResult)> }` inserting `sources` (created_by = principal) and the job in one transaction; the import payload `{upload_id, content_digest, format, byte_length, sheet, attestation}`.
  - The import handler streams the staged original into `parse`, PUTs the inspection report to `inspection_key`, and finishes `done` with `result_digest` = SHA-256 of the report bytes, or `failed` with the parser's refusal code; it never creates a revision.
  - `pub struct Inspection { pub format: Format, pub selected_sheet: Option<String>, pub available_sheets: Vec<String>, pub sheet_required: bool, pub accepted: u64, pub rejected: u64, pub columns: Vec<InspectedColumn>, pub preview: Vec<InspectedRow> /* ≤500, original coordinates */, pub expanded_bytes: Option<u64> }`.
  - Routes: `GET /v1/sources/{source}/inspection` → 200 `Inspection` | 404 | 409 `inspection_pending`; `POST /v1/sources/{source}/inspections` `{sheet}` with `Idempotency-Key` → 202 `job_state` | 404 | 409 | 429.

- [ ] **Step 1: Write failing tests** mapped to APP-017's sentences. Sentence 1: `all_fixture_csv_xlsx_pairs_produce_counts_typed_rows_and_coordinates` (the APP-011 manifest pairs), `xlsx_without_sheet_reports_available_sheets_then_selected_sheet_parses`. Sentence 2: `workbook_wide_limits_apply_with_one_sheet_selected` (`oversize-dimension.xlsx`, the APP-004 bomb fixtures), `hostile_inputs_fail_visibly_with_parser_code`, `csv_record_bound_is_cell_derived` (64,000 records for five columns). Sentence 3: `macros_links_formulas_never_execute` (`formula-cache.xlsx`), `cancel_and_parser_crash_leave_no_revision_and_next_job_runs`, `one_job_slot_shared_by_import_and_normalize`, `slot_rss_line_kills_with_resource_limit`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement** the handler, inspection, routes and `enqueue_import`.
- [ ] **Step 4: Run** all local checks; expected: pass; record parser wall time and peak RSS per fixture.
- [ ] **Step 5: Independent review** (sandbox boundary, slot exclusivity, refusal visibility, disclosure) → `evidence/APP-017/independent-review.md`; fix; move APP-017 to In review.
- [ ] **Step 6: Commit** (`feat(imports): CSV and XLSX import jobs through parser-host (APP-017)`).

### Task A6: APP-018 — interpretation, confirmation and normalization

**Files:**
- Create: `app/src/imports/interpret.rs`, `app/src/imports/suggest.rs`, `app/src/imports/normalize.rs`, `app/runtime/normalize.py`, `app/runtime/test_normalize.py`, `evidence/APP-018/README.md`
- Modify: `app/runtime/runtime.py` (mode `normalize`), `app/src/jobs/runner.rs` (register `JobKind::Normalize`), `app/src/imports/routes.rs`, `app/src/storage/definitions.rs` (`create_work` refuses a binding to a revision without a confirmed `mapping_version`; PH01 operator-seeded rows exempt), `.dockerignore`

**Interfaces:**
- Consumes: A5 (`Inspection`, staged original), A3 (`NewRevision`, objects), A4 (`parse`, `run_confined`), `admission::{canonical, digest}`, `Workspace::create_work` (T6), the fixture oracles in `fixtures/phase-one/v1/{mappings,expected}.json`.
- Produces:
  - `pub struct MappingProposal { pub sheet: Option<String>, pub fields: Vec<FieldMapping>, pub time: TimeMeaning, pub coverage: Coverage, pub currency: Option<Currency> }` with `FieldMapping { column: u32, name: String, value_type: ValueType, unit: Option<Unit>, missing: MissingPolicy /* unknown */ }` and `TimeMeaning { column: u32, semantics: TimeSemantics /* event | interval */, timezone: Utc, window: HalfOpen }`.
  - `imports::interpret::confirm(inspection: &Inspection, proposal: &MappingProposal) -> Result<ConfirmedMapping, InterpretationRefusal>` with `pub enum InterpretationRefusal { TimestampRefused, UnitRefused, SemanticsRefused, CurrencyRefused, SheetRequired, FieldUnknown(u32) }` (codes `timestamp_refused`, `unit_refused`, `semantics_refused`, `currency_refused`, …); missing values and integers outside int64 become unknown with their original text, never repaired.
  - `imports::suggest::suggest(inspection: &Inspection) -> Vec<MappingSuggestion>` from header and type evidence only, no model; a suggestion is never a confirmation.
  - `POST /v1/sources/{source}/confirmations` `{mapping}` with `Idempotency-Key` → 202 `job_state` (normalize job whose payload carries the confirmed mapping, `mapping_version` = previous + 1 and the inspection digest; `payload_digest` is the mapping's digest) | 404 | 409 | 422 with the refusal code | 429.
  - The normalize handler re-runs `parse` with the confirmed sheet; after `parser-host` exits, Rust types, sorts by (time, original coordinate) and computes `rows_digest`, then streams forma-json-v1 rows to `runtime.py normalize`, which writes `part.parquet` in the given order (100,000-row groups, ZSTD); the worker PUTs, copies the original and finishes with `NewRevision`.
  - `GET /v1/sources/{source}/revisions` → the transformation history: each revision's `mapping_version`, confirmed mapping, digests and `created_at`.

- [ ] **Step 1: Write failing tests** mapped to APP-018's sentences. Sentence 1: `binding_refused_until_mapping_confirmed`, `confirmation_requires_sheet_fields_units_timezone_and_window_meaning`. Sentence 2: `ambiguous_fractional_invalid_timestamps_refused`, `undeclared_unit_and_cumulative_counter_refused`, `mixed_currency_refused`, `missing_values_and_unsafe_integers_shown_unknown_with_original_text`, `partial_coverage_marks_empty_buckets_and_full_window_rate_unknown`. Sentence 3: `remap_creates_new_revision_with_history_and_original_coordinates`, `suggestion_never_produces_revision_without_confirmation_carrying_mapping_digest`; normalization: `normalize_child_receives_only_typed_rows`, `parquet_row_order_matches_rows_digest_order`, `same_original_and_mapping_reproduce_normalized_digest`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement** interpretation, suggestions, the normalize handler and child, routes and the binding check.
- [ ] **Step 4: Run** all local checks; expected: pass; the Northstar events and inventory produce revisions whose digests are recorded.
- [ ] **Step 5: Independent review** (no silent repair, confirmation gate, determinism, confinement) → `evidence/APP-018/independent-review.md`; fix; move APP-018 to In review.
- [ ] **Step 6: Commit** (`feat(imports): confirmed interpretation and normalized revisions (APP-018)`).

### Task A7: APP-019 — accessible drop and keyboard upload source cards, upload route, beta copy

**Files:**
- Create: `app/src/imports/upload.rs`, `app/src/imports/props.rs`, `app/admin/src/sources/source-card.tsx`, `app/admin/verify/upload.mjs`, `evidence/APP-019/README.md`
- Modify: `app/admin/src/work/app.tsx` (drop zone, Upload control, source cards), `app/admin/src/invite/app.tsx` (beta copy, B18), `app/admin/build.ts`, `app/src/imports/routes.rs`, `app/src/http/limits.rs` (upload bucket), `app/src/main.rs`, `.dockerignore`

**Interfaces:**
- Consumes: A5 (`enqueue_import`, inspection routes), A6 (confirmation route, suggestions), A3 (`objects::put` to `staging_key`), C2 (`personal_view_state`, `workspace::placement::place`), T7 (`auth::csrf::require_same_origin`, synchronizer token), T9 (`GET /w/{app}/{work}`, `WorkProps`), APP-003 island patterns.
- Produces:
  - `POST /v1/sources/uploads` per D9.9: headers `Content-Length`, `Content-Type`, `Idempotency-Key`, the synchronizer token in `X-Forma-CSRF` (or the header name APP-009 lands), `Forma-Upload-Attestation: synthetic-or-non-sensitive` (B18); order: short authorization transaction (precheck `import_source`, fence, entitlement, D9.13 limits) → stream to `staging_key` with SHA-256 and length → `enqueue_import`; → 202 `{source, job}` | 400 | 403 `cross_site` | 404 | 409 | 413 `payload_too_large` | 415 | 429 naming the limit; 120 s absolute, 64 KiB/s floor over any 10 s window.
  - `pub struct SourceCardProps { pub source: String, pub display_name: String, pub state: SourceCardState, pub limits: UploadLimitsView, pub job: Option<JobView>, pub inspection: Option<InspectionView>, pub suggestions: Vec<SuggestionView>, pub failure: Option<FailureView>, pub geometry: Geometry, pub idempotency_key: String }` with `SourceCardState { Empty, Uploading, Queued, Inspecting, NeedsSheet, NeedsConfirmation, Normalizing, Ready, Failed, Cancelled }`; limits and the beta copy render before any transfer; progress comes from the browser's own request.

- [ ] **Step 1: Write failing tests** mapped to APP-019's sentences. Sentence 1: `drop_and_keyboard_upload_share_one_workflow_with_limits_shown_before_transfer`, `declared_length_over_cap_is_413_before_any_body_byte`, `upload_streams_without_whole_body_buffer`, `slow_body_under_floor_is_refused`, `upload_without_attestation_refused`. Sentence 2: `keyboard_only_progress_cancel_sheet_select_retry`, `mapping_confirmation_cannot_be_skipped_by_either_path`. Sentence 3: `new_card_placed_without_obscuring_and_stable_across_inspection_and_reload`, `empty_canvas_is_an_explicit_choice`; browser checks in `app/admin/verify/upload.mjs` on Chromium, Firefox and WebKit through the APP-003 harness.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement** the route, props, card, page wiring and copy.
- [ ] **Step 4: Run** all local checks and the browser verification; expected: pass; record the work route's measured Forma bytes.
- [ ] **Step 5: Independent review** (CSRF, streaming bounds, refusal codes, keyboard parity) → `evidence/APP-019/independent-review.md`; fix; move APP-019 to In review.
- [ ] **Step 6: Commit** (`feat(imports): accessible drop and keyboard upload source cards (APP-019)`).

---

## Track B — S04 correct bounded execution (APP-020–025)

### Task B1: APP-020 — typed grammar, admission, normalization and plan compilation

**Files:**
- Create: `app/src/query/grammar.rs`, `app/src/query/normalize.rs`, `app/src/query/typecheck.rs`, `app/src/query/plan.rs`, `app/runtime/fixtures/plan-vectors.json`, `app/runtime/test_query_engine.py`, `evidence/APP-020/README.md`
- Modify: `app/runtime/query_engine.py` (`execute_plan` over DuckDB's relational API with bound parameters; identifiers only from the plan's closed column list), `app/src/query/worker.rs` (request carries the plan and per-input descriptors), `app/src/query/routes.rs` (query string → 400), `app/src/admission/validate.rs` (definition admission calls `query::grammar::admit`), `.dockerignore`

**Interfaces:**
- Consumes: `Data::{run_query, release_result}`, `QueryRun`, `query::worker::execute`, `query::receipt::build`, `POST /v1/work/{app}/{work}/queries/{view}` (T9); `AdmissionError::UnregisteredOperator` (T6); confirmed revisions (A6); edition 2 (0.3).
- Produces: `query::grammar::admit(query: &serde_json::Value, inputs: &[InputContract], registry: &OperatorRegistry) -> Result<AdmittedQuery, QueryRefusal>` with `pub enum QueryRefusal { UnknownField(String), TypeMismatch { field: String, expected: ValueType }, UndeclaredReference(String), TooComplex { limit: &'static str }, UnregisteredOperator { id: String, version: u32 }, Encoding(String) }`; `query::normalize::canonical(query: &AdmittedQuery) -> Vec<u8>` with `digest("forma:query:v1", …)`; `query::plan::compile(query: &AdmittedQuery, parameters: &TypedParameters, inputs: &[ResolvedInput]) -> Plan`, `Plan` serialized as closed JSON `{plan_version: 1, inputs: [{input_id, kind, columns: [{name, type}]}], operations: [...], parameters: [...]}`; Python `execute_plan(plan: dict, cache_dir: pathlib.Path) -> dict`; the shared `plan-vectors.json` checked by Rust and Python; multi-input plans (≤8 inputs); stable sort with a required tie-break.

- [ ] **Step 1: Write failing tests** mapped to APP-020's sentences. Sentence 1: `only_declared_operators_and_registered_lookup_versions_compile`. Sentence 2: `unknown_field_type_mismatch_undeclared_reference_and_complexity_refused_before_engine`. Sentence 3: `normalized_form_deterministic_and_result_order_stable`, `sql_and_engine_commands_cannot_enter_grammar_or_plan` (hostile corpus), `rust_and_python_agree_on_plan_vectors`, `query_string_on_v1_query_route_is_400`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement** grammar, typecheck, normalization, compiler and `execute_plan`.
- [ ] **Step 4: Run** all local checks; expected: pass.
- [ ] **Step 5: Independent review** (no SQL text across the boundary, refusal before engine, determinism) → `evidence/APP-020/independent-review.md`; fix; move APP-020 to In review.
- [ ] **Step 6: Commit** (`feat(query): typed grammar, admission and plan compilation (APP-020)`).

### Task B2: APP-021 — exact arithmetic, units, windows and result scope

**Files:**
- Create: `app/src/query/arithmetic.rs`, `app/src/query/units.rs`, `app/src/query/window.rs`, `evidence/APP-021/README.md`
- Modify: `app/runtime/query_engine.py` (HUGEINT sums as decimal strings, ratio numerator and denominator as integers, half-open UTC windows, dense buckets only under complete coverage), `app/src/query/plan.rs`, `app/src/query/receipt.rs` (`truncated_page`, `total_scope`, `exact_rational`, `coverage`)

**Interfaces:**
- Consumes: B1's plan and executor; edition 2's `exact_rational`, `window_minutes`, `coverage`.
- Produces: `query::arithmetic::reduce(numerator: &str, denominator: &str) -> Result<ExactRational, QueryRefusal>` (≤38 digits, lowest terms; zero denominator is an error); `query::window::minutes(window: &HalfOpenWindow) -> Result<u64, QueryRefusal>` (whole minutes only); `query::units::compatible(left: &Unit, right: &Unit) -> Result<(), QueryRefusal>`; refusal codes `unknown_value_in_aggregate`, `currency_refused`, `unit_refused`, `timestamp_refused`, `narrow_the_query`.

- [ ] **Step 1: Write failing tests** mapped to APP-021's sentences. Sentence 1: `northstar_five_events_1140000_and_114000_per_minute_e6_excluded`, `boundary_canary_and_ops_runner_oracles`. Sentence 2: `mixed_currency_incompatible_units_ambiguous_time_unsafe_int_and_undeclared_coverage_refused`, `unknown_value_in_aggregate_refused`, `ratio_returns_exact_integers_and_reduces_in_rust`. Sentence 3: `pagination_and_top_n_never_change_totals`, `stable_ties_omitted_groups_lookup_cardinality_and_empty_buckets_reproducible`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement**.
- [ ] **Step 4: Run** all local checks; expected: pass; oracles recomputed from imported revisions, not fixture constants.
- [ ] **Step 5: Independent review** (no float path, scope before pagination) → `evidence/APP-021/independent-review.md`; fix; move APP-021 to In review.
- [ ] **Step 6: Commit** (`feat(query): exact arithmetic, units, windows and result scope (APP-021)`).

### Task B3: APP-022 — versioned domain operator registry (CIDR lookup, integer money)

**Files:**
- Create: `app/src/query/operators/mod.rs`, `app/src/query/operators/cidr.rs`, `app/src/query/operators/money.rs`, `evidence/APP-022/README.md`
- Modify: `app/runtime/query_engine.py` (integer CIDR match, money sums), `app/src/admission/validate.rs`, `app/src/storage/definitions.rs` (`Control::publish` writes the `{id, version}` list into the publication envelope)

**Interfaces:**
- Consumes: B1/B2; edition 2's per-operator identities; A1's broadened CHECKs (B22).
- Produces: `pub struct OperatorContract { pub id: &'static str, pub version: u32, pub inputs: &'static [ValueType], pub output: ValueType }`; `query::operators::OperatorRegistry::{launch() -> Self, get(&self, id: &str, version: u32) -> Option<&OperatorContract>}` with `cidr_exact_then_longest` v1 (IPv4 as unsigned 32-bit and prefix length, `(ip & mask) = network`, exact beats longest, equal-length duplicate → `lookup_ambiguous`, each event once) and `money_sum` v1, `money_compare` v1 (USD exponent 2, mixed currency → `currency_refused`); no registry input names an organization, space or app.

- [ ] **Step 1: Write failing tests** mapped to APP-022's sentences. Sentence 1: `exact_ip_precedes_longest_network_each_event_once` (Northstar and overlap fixtures), `ambiguous_inventory_refused`. Sentence 2: `spend_review_integer_minor_units_usd_2_totals_64000_and_queue_45000`, `money_sum_stays_integer_minor_units`, `mixed_currency_refused`. Sentence 3: `both_operators_register_versioned_contracts_through_one_registry`, `registry_has_no_organization_space_or_app_branch`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement**.
- [ ] **Step 4: Run** all local checks; expected: pass.
- [ ] **Step 5: Independent review** → `evidence/APP-022/independent-review.md`; fix; move APP-022 to In review.
- [ ] **Step 6: Commit** (`feat(query): versioned CIDR and money operators (APP-022)`).

### Task B4: APP-023 — receipt provenance, record snapshots, drilldown and quarantine

**Files:**
- Create: `app/migrations/0012_receipt_quarantine.sql`, `app/migrations/catalog-v12.json`, `app/src/storage/receipts.rs`, `app/src/query/snapshot.rs`, `app/src/query/released.rs`, `app/src/query/drilldown.rs`, `app/runtime/receipts_operator.py`, `app/runtime/test_receipts_operator.py`, `evidence/APP-023/README.md`
- Modify: `app/src/storage/queries.rs` (record inputs at one statement, D9.8), `app/src/storage/commands.rs` (a command may cite one released receipt), `app/src/query/receipt.rs`, `app/src/query/routes.rs`, `app/operator/operator.py` (`verify-receipts`), the version pins, `qualify_tenant_schema.py`, `.dockerignore`

**Interfaces:**
- Consumes: `Data::{commit_record, read_outcome}`, `CommandRequest`, `organization_sequences` (T10); APP-014 races and `TestHooks` (T11); `genui.query_receipts` from 0007 with D9.8's columns; B1–B3.
- Produces:
  - 0012: `source_revisions.quarantined_at timestamptz NULL`; operator kind `revision.quarantined`; a query against a quarantined revision is refused `source_quarantined`.
  - `query::snapshot::read_records(transaction, work: &WorkRef, inputs: &[RecordInput]) -> Result<RecordSnapshot { sequence: i64, rows: Vec<TypedRow> }>`: after the member and work fences, one CTE statement reads `next_commit_sequence − 1` as S and the latest revision of each record with `commit_sequence ≤ S`; ≤100,000 rows and 64 MiB, else `narrow_the_query`/`resource_limit`; rows go to the engine child on stdin.
  - `query::released::Released` — decided here: an in-process bounded map of released receipts keyed `(organization, principal, digest)`, 4,096 entries, 1 h TTL; `CommandRequest.cited_receipt: Option<String>` (digest) and `POST /v1/work/{app}/{work}/receipts/{digest}/keep` insert the canonical receipt into `query_receipts` (at most one per commit, `cited_by_outcome` set by the command); an unknown or expired digest answers 409 `receipt_expired` and the client re-runs the query.
  - `POST /v1/work/{app}/{work}/receipts/{digest}/drilldown` `{page}` → 500-row pages of the receipt's reproducible predicate under current authority | 404.
  - Operator: `verify-receipts --organization <uuid> [--sample N]` re-derives cited receipts and on mismatch sets `quarantined_at` with `revision.quarantined`.

- [ ] **Step 1: Write failing tests** mapped to APP-023's sentences. Sentence 1: `receipt_pins_definition_operator_source_mapping_parameters_actor_epoch_snapshot_and_result`. Sentence 2: `rederive_northstar_and_spend_review_from_retained_revisions`, `receipt_distinguishes_record_version_1_and_2` (S05), `snapshot_sequence_and_record_rows_agree_under_concurrent_commit` (a `TestHooks` commit between statements). Sentence 3: `drilldown_is_bounded_authorized_and_predicate_based`, `deleted_or_quarantined_revision_reproduction_fails_explicitly`, `cited_receipt_stored_once_per_commit`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement** 0012 and pins, snapshot, released map, citation and keep, drilldown, `verify-receipts`.
- [ ] **Step 4: Run** all local checks; expected: pass; record `catalog-v12.json` size.
- [ ] **Step 5: Independent review** (snapshot consistency under READ COMMITTED, citation authority, quarantine) → `evidence/APP-023/independent-review.md`; fix; move APP-023 to In review.
- [ ] **Step 6: Commit** (`feat(query): receipt provenance, record snapshots and quarantine (APP-023)`).

### Task B5: APP-024 — query admission, deadlines, confinement and cache authority

**Files:**
- Create: `app/src/query/slots.rs`, `app/src/query/source_cache.rs`, `app/runtime/query_broker.py` and `app/runtime/test_query_broker.py` (only if T9 did not adopt D7.10(5)'s broker), `evidence/APP-024/README.md`
- Modify: `app/src/query/worker.rs` (spawn through `supervise::run_confined` with `nofile` 64, `core` 0, RSS line 1.25 GiB; client disconnect cancels a queued query and interrupts a running one), `app/src/storage/queries.rs` (entitlement at admission, execution and delivery; `queries_per_hour`), `app/src/query/routes.rs`, `app/src/http/limits.rs`

**Interfaces:**
- Consumes: A2 (`Limit`, entitlement limits), A4 (`supervise`), B1–B2, T9's engine child and cache fetch, `Data::release_result`.
- Produces: `query::slots::QuerySlots::acquire(&self, organization: &str, work: &str, epoch: u64) -> Result<SlotGuard, Busy>` (2 per task, work-conserving, round-robin across organizations, supersede older epochs of the same work, queue 12 per organization and 20 per task, `Busy { limit: Limit, retry_after_seconds: u32 }` → 429); `query::source_cache::SourceCache::{get_or_fetch(&self, run: &QueryRun /* built only after the precheck and fence */, input: &str) -> Result<PathBuf>, evict(&self, organization: &str, normalized_digest: &str)}` releasing a path to an engine only after reauthorization; no result cache; an in-process hourly counter answering 429 `queries_per_hour`; the broker (two active, twenty queued) only if T9 has not adopted it. Operator query cancellation is not built: every query ends within 1.8 s plus the 5 s backstop (recorded limitation).

- [ ] **Step 1: Write failing tests** mapped to APP-024's sentences. Sentence 1: `overload_is_bounded_while_another_organization_progresses`, `waiting_organization_takes_next_free_slot`, `new_epoch_supersedes_queued_older_epoch`. Sentence 2: `cancel_and_deadline_release_engine_memory_and_temp`, `budget_exceeded_is_error_never_partial_total`, `engine_child_limits_are_nofile_64_and_core_0_only`, `rss_line_kills_child_with_resource_limit`. Sentence 3: `source_cache_reauthorizes_before_engine_and_result_cache_is_absent`, `revoked_and_foreign_receipt_lookup_disclose_nothing`. Sentence 4: `inactive_or_revoked_trial_and_pre_access_never_reserve_slot_or_receive_results`, `revocation_while_waiting_denies_delivery`, `queries_per_hour_limit_answers_429_naming_it`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement** slots, cache, confinement, entitlement checks, counter and (if needed) the broker.
- [ ] **Step 4: Run** all local checks; record the six-view wave timings, per-child RSS and the S02 six views plus the 1,000,000-row aggregate under the limits (D9.7); N3 is measured, not promised.
- [ ] **Step 5: Independent review** (fairness, fail-closed admission, cache authority, confinement limits) → `evidence/APP-024/independent-review.md`; fix; move APP-024 to In review.
- [ ] **Step 6: Commit** (`feat(query): fair admission, deadlines, confinement and cache authority (APP-024)`).

### Task B6: APP-025 — six linked views by exact context and filter epoch

**Files:**
- Create: `app/src/query/batch.rs`, `fixtures/phase-two/northstar-six-views.json`, `app/admin/src/work/receipt-inspector.tsx`, `app/admin/verify/linked-views.mjs`, `evidence/APP-025/README.md`
- Modify: `app/admin/src/work/app.tsx` (six linked views, filter in POST bodies, receipt inspector), `app/src/query/routes.rs` (`POST /v1/work/{app}/{work}/views` batch), `WorkProps` states, `app/operator/operator.py` (`seed` creates the six-view definition through the admitted path on managed environments)

**Interfaces:**
- Consumes: B4 receipts, B5 slots, T6's `create_definition_revision`/`Control::publish` (the six-view definition is admitted, never seeded as a placeholder), APP-003's epoch-drop pattern.
- Produces: the six-view Northstar definition, decided here: `total` (summary), `unmatched` (summary over lookup null), `destinations` (table with `cidr_exact_then_longest`), `timeline` (one-minute buckets, complete coverage), `rate` (ratio over `window_minutes`), `evidence` (table with drilldown); `POST /v1/work/{app}/{work}/views` `{context: {organization, app, work}, filter_epoch, filters, views: [..≤24]}` → one typed query per linked view, each response carrying `{context, filter_epoch, receipt}`; `WorkProps.state` ∈ `loading | empty | failed | denied | stale | partial | ready`, never a fabricated zero; the island drops any response whose context or epoch differs from the current one.

- [ ] **Step 1: Write failing tests** mapped to APP-025's sentences. Sentence 1: `destination_select_updates_six_views_same_epoch_with_scope_units_denominator`. Sentence 2: `delayed_old_filter_work_and_org_responses_never_rendered` (server-delayed controls in the browser harness). Sentence 3: `clearing_filter_restores_1140000_and_full_scope_totals`, `all_card_states_distinguishable_with_receipt_inspector`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement**.
- [ ] **Step 4: Run** all local checks and `app/admin/verify/linked-views.mjs`; record six-view refresh timings and the route's Forma bytes.
- [ ] **Step 5: Independent review** (context discard, filters out of URLs, state truthfulness) → `evidence/APP-025/independent-review.md`; fix; move APP-025 to In review.
- [ ] **Step 6: Commit** (`feat(workspace): six linked views by context and filter epoch (APP-025)`).

---

## Track C — S05 a useful persistent workspace (APP-026–031, APP-081)

### Task C1: APP-026 — company home, spaces, aliases, click and slash navigation

**Files:**
- Create: `app/migrations/0011_navigation.sql`, `app/migrations/catalog-v11.json`, `app/src/storage/navigation.rs`, `app/src/navigation/mod.rs`, `app/src/navigation/resolve.rs`, `app/src/navigation/routes.rs`, `app/src/navigation/tests.rs`, `app/admin/src/space/app.tsx`, `app/admin/src/shell/command.tsx`, `evidence/APP-026/README.md`
- Modify: `app/admin/src/home/app.tsx` (signed-in company home; anonymous landing unchanged), `app/admin/build.ts`, `app/src/main.rs`, the version pins, `qualify_tenant_schema.py`, `.dockerignore`

**Interfaces:**
- Consumes: sessions (T7), `Action::Discover`, `read_publication`, `spaces`/`apps` (0002), the fallback 404 (T8).
- Produces: 0011 `genui.aliases(organization_id, alias_normalized, target_kind, target_id, created_at, retired_at, UNIQUE (organization_id, alias_normalized) WHERE retired_at IS NULL)` (create and retire only with `admin_policy`, kinds `alias.created`, `alias.retired` on `workspace`) and `apps.display_name text NULL`; `navigation::resolve::resolve(request: &TrustedRequest, input: &str) -> Result<Resolution>` with `pub enum Resolution { Space(String), App(String), Work { app: String, work: String }, NoMatch }`; `POST /v1/navigate` `{input}` → 200 `{kind, id, href}` or `{kind: "none"}` (creates nothing, calls no model); `POST /v1/aliases`, `POST /v1/aliases/{alias}/retire`; `GET /` (company home for members), `GET /s/{space}`; recents and suggestions are permitted items only (recents and permitted catalog entries, no model; no thumbnails in PH02, per the Task 0.2 amendment), filtered before ranking; the shell ignores keystrokes originating in text entry.

- [ ] **Step 1: Write failing tests** mapped to APP-026's sentences. Sentence 1: `click_and_slash_enter_resolve_same_stable_ids`, `duplicate_alias_refused_and_retired_alias_old_link_resolves`. Sentence 2: `home_recents_and_suggestions_omit_unauthorized_metadata_before_ranking` (canary). Sentence 3: `unknown_command_creates_nothing_and_calls_no_model`, `typing_in_note_or_formula_never_navigates`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement** 0011 and pins, resolution, routes, pages and shell.
- [ ] **Step 4: Run** all local checks and the browser harness; record `catalog-v11.json` size.
- [ ] **Step 5: Independent review** (pre-ranking filtering, inert commands) → `evidence/APP-026/independent-review.md`; fix; move APP-026 to In review.
- [ ] **Step 6: Commit** (`feat(navigation): company home, spaces, aliases and slash navigation (APP-026)`).

### Task C2: APP-027 — canvas geometry, keyboard equivalence and personal layout (after Task 0.2)

**Files:**
- Create: `app/migrations/0010_personal_view_state.sql`, `app/migrations/catalog-v10.json`, `app/src/storage/presentation.rs`, `app/src/workspace/mod.rs`, `app/src/workspace/layout.rs`, `app/src/workspace/placement.rs`, `app/src/workspace/routes.rs`, `app/src/workspace/tests.rs`, `app/admin/src/work/canvas.tsx`, `app/admin/verify/canvas.mjs`, `evidence/APP-027/README.md`, `evidence/APP-027/budget.json`
- Modify: `app/admin/src/work/app.tsx` (pan, zoom, move, resize, group, fit, reset; focused view), `app/admin/build.ts`, the version pins, `qualify_tenant_schema.py`, `.dockerignore`

**Interfaces:**
- Consumes: `Workspace::read_work` and the Spend Review work (T6), APP-003 island, donor geometry and keyboard ideas rebuilt on Forma 3.2 signals (D9.11; no donor DOM code).
- Produces: 0010 `genui.personal_view_state(organization_id, principal_id, app_id, work_id, layout jsonb ≤ 64 KiB, filters jsonb ≤ 16 KiB, updated_at)` (`app_id` added for the composite FK to `saved_work`), RESTRICTIVE `USING` and `WITH CHECK (principal_id = current principal)`, written by workspace, not audited; `pub struct Geometry { pub x: i32, pub y: i32, pub width: u32, pub height: u32, pub group: Option<String> }`; `workspace::layout::Layout { cards: Vec<(String, Geometry)>, camera: Camera { pan_x: i32, pan_y: i32, zoom_percent: u16 } }`; `workspace::placement::place(existing: &[Geometry], size: (u32, u32)) -> Geometry` (non-obscuring, deterministic); `POST /v1/work/{app}/{work}/view-state` `{layout?, filters?}` → 204 (a patch to one field leaves the other untouched); per-feature byte deltas in `budget.json`.

- [ ] **Step 1: Write failing tests** mapped to APP-027's remaining sentences. Sentence 1: `pan_zoom_move_resize_group_fit_reset_on_numeric_geometry`, `adding_card_never_obscures_current_work`. Sentence 2: `keyboard_and_focused_view_complete_same_task_on_same_state`. Sentence 3 (kept clause): `reload_restores_personal_layout_and_display_settings`, `personal_view_state_is_visible_only_to_its_principal`; `route_budget_within_b16` with per-feature deltas.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement** 0010 and pins, layout, placement, routes and canvas island; a feature that does not fit 28,420 B moves to the native group, measured.
- [ ] **Step 4: Run** all local checks and `app/admin/verify/canvas.mjs` on three engines; record `catalog-v10.json` size and the route budget.
- [ ] **Step 5: Independent review** (one model for canvas and focused view, privacy of personal state, budget) → `evidence/APP-027/independent-review.md`; fix; move APP-027 to In review.
- [ ] **Step 6: Commit** (`feat(workspace): canvas geometry, keyboard path and personal layout (APP-027)`).

### Task C3: APP-081 — linked views and evidence connections on the canvas

**Files:**
- Create: `app/src/workspace/connections.rs`, `evidence/APP-081/README.md`
- Modify: `app/admin/src/work/canvas.tsx`, `app/admin/src/work/app.tsx`, `app/admin/verify/linked-views.mjs`, `app/admin/verify/canvas.mjs`

**Interfaces:**
- Consumes: B6 (`POST /v1/work/{app}/{work}/views`, epoch discard, receipt inspector), C2 (geometry, personal filters), A3/B4 (bindings, receipts).
- Produces: `workspace::connections::derive(definition: &AdmittedDefinition, bindings: &[WorkBinding], receipts: &[ReceiptRef]) -> Vec<Connection { from: CardRef, to: CardRef, kind: ConnectionKind /* source_to_view | view_to_evidence */ }>` (connections only from declared bindings and receipts, never inferred); linked cards on the canvas re-query through B6 on a filter change regardless of geometry changes.

- [ ] **Step 1: Write failing tests** mapped to APP-081's sentences: `connections_only_from_declared_bindings_and_receipts`, `linked_canvas_cards_share_filter_and_epoch_after_move_resize_group_fit`, `keyboard_and_focused_view_complete_linked_task_on_same_epoch`, `delayed_old_filter_work_and_org_responses_never_rendered_on_canvas`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement**.
- [ ] **Step 4: Run** all local checks and both browser verifications; record the route budget.
- [ ] **Step 5: Independent review** → `evidence/APP-081/independent-review.md`; fix; move APP-081 to In review.
- [ ] **Step 6: Commit** (`feat(workspace): linked views and evidence connections on the canvas (APP-081)`).

### Task C4: APP-028 — trusted native renderer catalog

**Files:**
- Create: `app/src/render/mod.rs`, `app/src/render/registry.rs`, `app/src/render/{summary,table,detail,timeline,chart,filter,form,note,status}.rs`, `app/src/render/tests.rs`, `evidence/APP-028/README.md`, `evidence/APP-028/budget.json`
- Modify: `app/admin/src/work/app.tsx`, `app/admin/build.ts`, `app/src/lib.rs`, `app/src/storage/definitions.rs` (`Control::publish` writes the `{kind, version}` list), `.dockerignore`

**Interfaces:**
- Consumes: `AdmissionError::UnregisteredRenderer` (T6), edition 2 per-renderer identities, C2's canvas, APP-003 bounds.
- Produces: `pub struct RendererContract { pub kind: &'static str, pub version: u32 }`; `render::registry::RendererRegistry::{launch() -> Self, admit(&self, kind: &str, version: u32) -> Result<&RendererContract, AdmissionError>}`; typed props per primitive with SVG geometry computed in Rust from numbers only; an unregistered kind or version refuses admission.

- [ ] **Step 1: Write failing tests** mapped to APP-028's sentences: `nine_primitives_render_admitted_props_on_ssr_and_respond_after_adoption`, `hostile_labels_escaped_and_svg_numeric_only`, `budgets_measured_with_all_primitives_imported`, `unregistered_renderer_version_refused`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement**.
- [ ] **Step 4: Run** all local checks and the three-engine browser harness; record route and props budgets.
- [ ] **Step 5: Independent review** → `evidence/APP-028/independent-review.md`; fix; move APP-028 to In review.
- [ ] **Step 6: Commit** (`feat(render): trusted native renderer catalog (APP-028)`).

### Task C5: APP-029 — accessible component states and equivalent focused workflows

**Files:**
- Create: `app/admin/verify/a11y.mjs`, `app/admin/verify/keyboard.mjs`, `evidence/APP-029/automated/`, `evidence/APP-029/keyboard/`, `evidence/APP-029/screen-reader/README.md`
- Modify: `app/admin/package.json`, `app/admin/package-lock.json` (the automated checker, exact pin, checksum and licence in the ticket log; ADR-009 "Left to named tickets")

**Interfaces:**
- Consumes: C4's primitives, A7's import workflow, B6's states through C3 where landed, the APP-003 Playwright harness.
- Produces: an automated check over every component × {loading, empty, error, denied, stale, partial, success, form conflict}; keyboard runs for canvas, import, filter and form tasks including focus through adoption; a screen-reader pass with findings and fixes against WCAG 2.2 AA. Decided here: "form" means C4's form primitive and the APP-013 record form; C3 and C6 rerun `keyboard.mjs` on their surfaces before review, and D1 reruns all.

- [ ] **Step 1: Write the failing checks** mapped to APP-029's sentences: the automated matrix, the keyboard runs, and the screen-reader script listing linked views, receipt details and commit/conflict announcements.
- [ ] **Step 2: Run**; expected: FAIL on the states and paths not yet accessible.
- [ ] **Step 3: Fix** the components, keeping fixes in the owning renderer files.
- [ ] **Step 4: Run** all local checks and the matrix; expected: pass; retain the screen-reader findings with tool and version.
- [ ] **Step 5: Independent review** → `evidence/APP-029/independent-review.md`; fix; move APP-029 to In review.
- [ ] **Step 6: Commit** (`test(a11y): component states, keyboard and screen-reader evidence (APP-029)`).

### Task C6: APP-030 — general forms and commands without weakening transaction invariants

**Files:**
- Create: `app/migrations/0013_record_identity.sql`, `app/migrations/catalog-v13.json`, `app/src/commands/forms.rs`, `app/src/commands/fields.rs`, `app/src/commands/transitions.rs`, `app/src/commands/references.rs`, `evidence/APP-030/README.md`
- Modify: `app/src/storage/commands.rs`, `app/src/storage/authorization.rs` (field, row and source-reference checks), `app/src/storage/race_cases.rs` (pre/post-activation retry), `app/admin/src/record/app.tsx`, the version pins, `qualify_tenant_schema.py`, `.dockerignore`

**Interfaces:**
- Consumes: `CommandRequest`, `Data::{commit_record, read_outcome}`, `CommandResult`, `commands::payload::digest` (T10); APP-014 races (T11); D7.16; `record_source_dependencies` (0002); C4's form primitive.
- Produces: 0013 `genui.record_identities(organization_id, app_id, work_id, collection_id, identity_digest, record_id, UNIQUE (organization_id, app_id, work_id, collection_id, identity_digest))` FORCE RLS, written by data in the command transaction (two racing creates are one commit and one conflict, demo design §6); `commands::forms::validate(schema: &RecordSchema, declaration: &CommandDeclaration, values: &serde_json::Value, current: Option<&Record>) -> Result<ValidatedValues, FieldRefusal>`; UI states `absent | pending | committed | failed | unknown | conflict` with authorized recovery; no wording implies an external payment.

- [ ] **Step 1: Write failing tests** mapped to APP-030's sentences: `server_enforces_types_constraints_record_identity_references_transitions`, `racing_creates_of_one_identity_commit_once_and_conflict_once`, `pre_activation_submit_then_post_activation_retry_yields_one_outcome`, `ui_states_absent_pending_committed_failed_unknown_conflict`, `concurrent_change_never_silently_overwritten`, `no_external_action_implied`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement** 0013 and pins, forms, fields, transitions, references and the page.
- [ ] **Step 4: Run** all local checks, the race campaign twice and `keyboard.mjs`; record `catalog-v13.json` size.
- [ ] **Step 5: Independent review** (ADR-004 order unchanged, idempotency semantics, authorization) → `evidence/APP-030/independent-review.md`; fix; move APP-030 to In review.
- [ ] **Step 6: Commit** (`feat(records): general forms and commands (APP-030)`).

### Task C7: APP-031 — saved work versus personal presentation

**Files:**
- Create: `app/migrations/0014_shared_views_and_work_creators.sql`, `app/migrations/catalog-v14.json`, `app/src/workspace/work.rs`, `evidence/APP-031/README.md`
- Modify: `app/src/storage/definitions.rs` (`create_work` with fresh bindings, empty collection, `created_by`), `app/src/storage/authorization.rs` (work creator authority, D9.5), `app/src/storage/presentation.rs`, `app/admin/src/work/app.tsx` (continue versus new; unsaved-edit recovery on context switch), the version pins, `qualify_tenant_schema.py`, `.dockerignore`

**Interfaces:**
- Consumes: `Workspace::{create_work, read_work}` (T6), `lock_work_revision` (0002), C1 navigation, C2 `personal_view_state`, C6 records; D7.4 (no application write path bumps `saved_work.version`).
- Produces: 0014 `saved_work.created_by uuid NULL`, `creator_revoked_at timestamptz NULL` with the composite FK and RESTRICTIVE check of D9.5; `genui.shared_views(organization_id, view_id, app_id, work_id, audience CHECK (audience = 'work_readers'), layout, filters, created_by, created_at)` with `view.shared` on `workspace`; creator authority on work = `record_read` and `record_write` (not `share_work`, not `export`) replacing only the `work_permissions` lookup; `POST /v1/work/{app}/new` → 201 `{work}`; `POST /v1/work/{app}/{work}/shared-views` → 201 `{view}`; notes persist as records, filters and layout as personal state, so a label or layout patch touches only `personal_view_state`.

- [ ] **Step 1: Write failing tests** mapped to APP-031's sentences: `continue_opens_same_work_pin_and_records`, `new_work_has_fresh_bindings_empty_collection_and_independent_state`, `creating_work_writes_no_permission_row_and_advances_no_epoch`, `reload_restores_notes_filters_and_layout`, `shared_view_has_stated_audience`, `context_switch_preserves_or_recovers_unsaved_edits`, `label_or_layout_patch_preserves_unrelated_filters_records_and_notes`.
- [ ] **Step 2: Run**; expected: FAIL.
- [ ] **Step 3: Implement** 0014 and pins, `work.rs`, the authorization change and the page.
- [ ] **Step 4: Run** all local checks; record `catalog-v14.json` size.
- [ ] **Step 5: Independent review** → `evidence/APP-031/independent-review.md`; fix; move APP-031 to In review.
- [ ] **Step 6: Commit** (`feat(workspace): saved work separate from personal presentation (APP-031)`).

---

## Track D — environments, integrated trial and the PH02 gate

### Task D0: Trial-organization pool and PH02 deployments to the beta (D9.15)

**Files:**
- Modify: `infra/src/stack.ts` (8 trial organizations, each with a tenant role, KMS key, bucket-policy statements and lifecycle rules in the existing tenant loop), `infra/test/security.test.ts`, the private registry file (≤32 bindings), `evidence/phases/PH02/environment/README.md`

**Interfaces:**
- Consumes: A3's storage controls; APP-078's window procedure; FormaStack's eight fAuth tenants and their `ten_` identifiers (requested through the cross-team lane); Victor's beta figure.
- Produces: the pool (single use per invitation; `purge-organization-objects` then key deletion scheduled and replenished by a CDK change); each PH02 increment deployed stop-then-start with a snapshot before every migration and forward fixes only.

- [ ] **Step 1: Write failing CDK tests**: `eight_trial_tenants_have_isolated_roles_keys_and_policies`, `trial_lifecycle_rules_match_d9_3`.
- [ ] **Step 2: Run** `npm --prefix infra test`; expected: FAIL.
- [ ] **Step 3: Implement**; request the tenants from FormaStack with D9.15 quoted; write the registry bindings.
- [ ] **Step 4: Deploy** inside a Victor-approved window; observe the service RUNNING on the recorded digest; reconcile the ledger.
- [ ] **Step 5: Independent review** → `evidence/phases/PH02/environment/independent-review.md`; commit.

### Task D1: PH02 integrated trial

- [ ] On the live beta once Victor approves its figure, otherwise in a reserved window, in installed Chrome with invited trial principals: drop and keyboard import of the Northstar events and inventory → confirmed mappings → six linked views with receipts and the oracles of the global constraints → a real form submission (S05 sequence) → personal layout → new versus continued work (ops-runner 210,000 / 30,000 / 21,000 beside Northstar's untouched 1,140,000) → reload; plus malicious-file, stale-response, cross-organization and resource-limit cases (QUALITY-GATES PH02 row); rerun `a11y.mjs` and `keyboard.mjs`; retain under `evidence/phases/PH02/trial/`; independent review.

### Task D2: The PH02 gate record

- [ ] Create `evidence/phases/PH02/GATE.md` from `Tickets/_tools/PHASE-GATE.md`, bind every exit criterion to its ticket evidence and the D1 trial, record limits and amendments (ADR-009's B-labels, the D9.7 confinement limitations, N3 measured not promised, B14's operator-only sharing until APP-048), obtain the independent reviewer's decision, then set `gate_status: passed` with the evidence reference in `Tickets/plan.json`, render and check the board, and write the Phase 3 handoff.

---

## Victor's actions (only these)

1. Approve the monthly figure for the always-on beta (ADR-008, still pending; D9.15 adds about $8/month for the eight trial-organization keys) or choose the scheduled-window alternative; this decides where D1 runs.
2. Let B18 stand (beta copy, per-upload attestation, retention purge, member delete) or veto it in favour of fixture-only imports.
3. Approve the bounded windows: A3's managed-bucket change and checks (cents) and D0's pool change and PH02 deployments.
4. Merge the reviewed PRs to `main` (PR 1 first), unless you delegate merging.

---

## Self-review

- Spec coverage: every PH02 acceptance sentence maps to a named failing test: APP-015 → A1–A2 (four sentences), APP-016 → A3, APP-017 → A4–A5, APP-018 → A6, APP-019 → A7, APP-020 → B1, APP-021 → B2, APP-022 → B3, APP-023 → B4, APP-024 → B5 (four sentences), APP-025 → B6, APP-026 → C1, APP-027 → C2, APP-081 → C3, APP-028 → C4, APP-029 → C5 (evidence sets), APP-030 → C6, APP-031 → C7; exit criteria 1–4 and the quality-gate trial → D1–D2. ADR-009's Consequences map: edition 0.3; B2/B9–B12/B22 A1; B7/B9/B15 (`jobs --stuck`, permissions, limits) A2; B1/B8/B13 (sources)/B15 (creator, purge) A3; D9.1 A4; B5/B6 A5; D9.3 normalization A6; D9.9/B17/B18/B19 A7; D9.7 B1–B5; D9.8 B4; D9.10 C1/C2/C7; B16 C2–C4; D9.12 A3; D9.13 A1/A2/B5; D9.14 0.2; D9.15 A3 step 5, D0, D1.
- Type consistency: `TrustedRequest`, `WorkerScope`, `JobKind`, `JobState`, `JobView`, `EnqueueResult`, `Limit`, `Claim`, `NewRevision`, `Inspection`, `StagedUpload`, `AdmittedQuery`, `Plan`, `QueryRefusal`, `Geometry` and the route paths are named identically wherever they appear.
- Review focus: twenty-two failure modes, each with an owning task and test.
- Proportion: the plan decides files, names, signatures, orders, routes and values; bodies are the implementer's. Decisions made here that ADR-009 leaves open are marked "decided here" (the split clauses, the `parser-host` framing, the released-receipt map, the six views, the job-payload digest domain) for the reviewer to check.
