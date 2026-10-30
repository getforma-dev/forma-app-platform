# APP-012 storage and query-worker half: author evidence

> 2026-10-29: the route and page half (open question 2 below) is recorded in [routes/README.md](routes/README.md); its `query-campaign.json` (15 checks after the integration onto `b497ede`) supersedes the counts here for the current source. This file's measurements and hashes are left as recorded at `3a1b5f3`.

**Author evidence, not acceptance.** This covers plan Task 9's storage and worker half only: `Data::run_query` and `Data::release_result`, the confined DuckDB engine child (`app/runtime/query_engine.py`, `runtime.py query`), the credentialed revision-cache fetch (`runtime.py cache`), the Rust supervisor and admission (`app/src/query/worker.rs`), the receipt builder (`app/src/query/receipt.rs`) and the operator commands that make the receipt truthful. The JSON route, the `/w/{app}/{work}` page and its loading/empty/failed/denied states wait for APP-009's sessions (`resolve_session`) and are the second half. No independent review has run, so no acceptance box is checked. Nothing touched AWS. Everything ran on one Apple-silicon Mac (Darwin arm64) with local Docker; every container started was removed.

## Decisions

### D1. The placeholder digests: a new revision and a new bound work, never a rewrite (APP-077 open question 2, review M2)

The coordinator's decision (2026-10-29), implemented here:

- `upload-sources` (the APP-077 fix, merged separately on `phase-one-spikes`) writes the real objects at ADR-009 B1's `o/{org}/s/{source}/r/{rev}/{original,part.parquet}` under a **new** revision id, `uuid5(namespace = organization id, name = content digest)` re-stamped as version 4 (the contract accepts only v4/v7 UUIDs), and prints one record `{organization_id, source_id, revision_id, content_digest, normalized_digest, object_digest, row_count, contract_digest}`. For Northstar the id is `9161c1e4-27c0-4cbb-addf-eb7ce504bf45`.
- **`admission_operator.py register-source-revision --organization <org> --record <file>`** (new, operator channel, D7.19 conventions: migration identity, verify-full TLS, catalog proof, `genui_owner` under explicit scope, organization `FOR SHARE`). Before connecting it checks the record is exactly those eight keys, the organization matches, the UUIDs and digests are well formed, `object_digest == normalized_digest` (the stored `part.parquet` digest, ADR-009 D9.3 option a) and `revision_id` equals the content-derived identity. It then inserts one `source_revisions` row whose envelope is the source's existing same-contract revision envelope with the new identity, digests, row count and creation time, validated as a `source_revision` by `contracts/validate.py` and reproducing the record's `contract_digest`. A rerun of the same record is `"changed": false`; any other content at an existing revision id is refused `revision_exists_differs`. The seeded revision `…006f` and its placeholder digests are never touched.
- **`admission_operator.py create-bound-work --organization --app --from-work --work --creator --input --source --revision`** (new). It writes a **new** saved work at version 1 on the origin work's publication, copying its context and bindings except that `--input` binds the new revision of the same source (live, same contract digest), with a fresh empty collection per published record schema. The creator must be an active member holding `run` on the app and `source_read` on every bound source (as the workspace's `create_work` requires) and is recorded as `created_by`, so creator authority (ADR-007 amendment, review M2) stands in for work grants. A rerun with the same bindings is unchanged; the seeded work `…0069`, its bindings and its record are untouched. The new work also starts with no record, which is what S05's `ik-1` at version 0 needs.
- **No audit event** is written by either command: D7.8's operator kinds are closed and name none (a new kind is an ADR amendment, not opened before November 2, ADR-010 §5); as for the seed, the retained JSON output of each command is the record. Open question 1 below.
- **No migration was needed.** `genui_data` already reads `saved_work`, `work_bindings`, `work_source_dependencies`, `sources`, `source_revisions`, `definition_*`, `publications`, `internal_record_revisions`, the permission tables and (0004) `entitlements`, and executes `lock_member_authority`/`lock_work_revision` (0002). The operator writes as `genui_owner`.

Production order for APP-078's window: `upload-sources` → save its record → `register-source-revision --record` → `create-bound-work --from-work 00000000-0000-4000-8000-000000000069 --work <new> --creator <principal> --input events --source 00000000-0000-4000-8000-00000000006e --revision 9161c1e4-27c0-4cbb-addf-eb7ce504bf45`. The demo opens the new work.

### D2. `events_v1`, not APP-005's `event_id` shape (APP-077 open question 3)

`query_engine.py`'s `FIELDS` is the contract's `events_v1` shape (`time`, `destination`, `bytes`), the three columns the operator's Parquet holds; `event_id` was never a contract field. The test builds the Parquet with the operator's own adapter steps and asserts its SHA-256 equals the uploaded `part.parquet` `0c2582d8…a082` (`test_normalized_revision_is_the_uploaded_object`), so the engine is tested on the exact bytes the managed upload produces.

### D3. The S02 window is a run parameter

The seeded Northstar definition's `total` and `timeline` views carry no window, and the uploaded revision has six events (E6 at 10:10). Summed whole, `total` is 2,040,000 (`test_the_window_parameter_is_what_excludes_e6`); 1,140,000 is the half-open window [10:00, 10:10), which design §7.2 step 3 has the member select ("She selects `build-worker-17` and [10:00, 10:10) UTC; the island posts one typed query at filter epoch 1"). Run parameters are therefore closed and typed (`storage::parse_parameters`): an optional `window {start, end}` (contract timestamps, start < end, applied as the query's half-open `window`; refused for a view with its own window), an optional `destination` equality filter (the destination-filtered 900,000) and `filter_epoch`. Nothing else is accepted, so no money literal, ratio or lookup can be introduced by a parameter. The receipt records them as `parameters` (named values `destination`, `window_end`, `window_start`) and `filter_epoch`; `provenance.query_digest` is the stored view query's `forma:query:v1` identity and `predicate_digest` the effective query's.

### D4. Receipt fields with no Phase 1 column

- `policy_epoch` is the publication's `policy_version` (0002 keeps no separate policy epoch; the seeded receipts use 1).
- `record_snapshot_sequence` is the largest `commit_sequence` among the work's record revisions at the run's snapshot, 0 when there is none (`organization_sequences` arrives with 0007).
- `source_revisions` lists the one source the query read, not every binding of the work.
- `mode` is `published` with `publication_id`; the edition stays 1 (`contracts/schema.json` `query_receipt`), digest `forma:query-receipt:v1`, result digest `forma:result:v1` over APP-005's `{"columns_are_query_defined": true, "rows": …}`.

### D5. Child per request is kept (the D7.10 measurement gate)

Design §5.2 budgets the six-view refresh at p99 2 s: one filter epoch fanned to six queries, two at a time (three waves), each inside about 1.8 s after the round trip, with 100 ms assumed for a warm operation. The gate applied: **one wave of two child executions at p99 ≤ 600 ms** (three waves inside 1.8 s). Measured (recorded run, table below): one child execution p50 ≈ 142 ms, p99 ≈ 165 ms; a wave of two p50 ≈ 140–152 ms, p99 ≤ 158 ms, so three waves take about 0.5 s; the whole authorized path (`run_query` + child + `release_result` + receipt) p50 ≈ 247 ms; the image's own child in a linux/arm64 container p50 ≈ 116 ms. About 80 ms of each child is the engine's own work (interpreter start, `duckdb` import, private database, query); the rest is spawn, pipes and parsing. That is about 1.3–1.6 times the 100 ms warm assumption but well inside the budget, so **APP-005's supervised broker is not adopted**. Limit: an Apple-silicon host, not Fargate; APP-078 re-measures in the window with the same campaign shape.

## What was built

| File | What |
| --- | --- |
| `app/runtime/query_engine.py` | Copy of `spikes/APP-005/query.py` at `25f92d6` (sha256 `d7d724b9…f187`). `POLICY` verbatim; `connect`, `encode_rows`, `run_sql`, `compile_query` are APP-005's code with five named deviations listed in its docstring: (1) `events_v1` fields; (2) `time_bucket` may rename its output (the seeded `timeline` buckets `time` as `minute`), replacing the column in place; (3) definition validation is the Rust re-admission (APP-010) while the compiler still refuses every operator, literal and shape outside the subset; (4) the source is the one cached revision of the request's organization, a regular file directly in `<cache>/o/<org>/`, digest re-checked against `normalized_digest`; (5) the empty read-only database is created in the parent's private scratch, the receipt and admission are the Rust parent's. Closed request (`organization_id`, `source{source_id, revision_id, normalized_digest, s3_key}`, `query`, `deadline_ms` ≤ 1,800, `cache_root`, `scratch`), exactly one JSON result line: `rows` (columns, rows ≤ 500, `truncated_page`, `partial: false`, `total_scope`), `refused` with a fixed code, or `failed`; never raw engine text. |
| `app/runtime/runtime.py` | Modes `query` (loads `query_engine.py` by path, since `python3 -I` drops the script directory) and `cache` (one bounded `GetObject` of `o/<org>/s/<src>/r/<rev>/part.parquet` with that organization's scoped storage credentials from the credentials file, SHA-256 checked, `0400`, atomic rename into `<cache>/o/<org>/`). The module scope note names both. |
| `app/src/storage/queries.rs` | `QueryRun` (no public constructor; read-only accessors), `Data::run_query` and `Data::release_result` in ADR-004 order on the data role: precheck `record_read` on the work (which includes `run` on the app and live `source_read` on every dependency), `genui_lock` fence, `lock_work_revision` on the pinned publication and version, the stored definition re-admitted, the view's query, its one bound live source revision under the member's `source_read`, `run` again, the record snapshot; release repeats precheck and fence and requires the same publication, work version, definition digest, policy version, binding and live revision. Every refusal is `Error::Denied`. |
| `app/src/query/worker.rs` | `Engine::execute(run, deadline)`: admission (2 per organization, 4 per task, 20 waiting, else `QueryError::Busy` for a 429 with `Retry-After: 2`; queue time counts against the deadline and an expired waiter launches nothing), the cache check and credentialed fetch with digest verification in Rust, a private `0700` scratch outside the cache removed after exit or kill, the child through `supervise::run_confined` (`env_clear`, three pipes, `RLIMIT_NOFILE` 64, no core, RSS line 1.5 GiB, 5 s backstop kill), and strict parsing of the one result line. `Failed` carries no row: a failure can never render as zero. |
| `app/src/query/receipt.rs` | Edition-1 receipt and its digest. |
| `app/runtime/admission_operator.py` | `register-source-revision`, `create-bound-work` (D1). |
| `app/runtime/requirements.lock` | `duckdb==2.0.0` with the reviewed APP-005 hashes, block verbatim (so the operator lock stays the verbatim union). |
| `app/Dockerfile`, `.dockerignore` | `query_engine.py` copied to `/app/query_engine.py`; the new Rust sources allowlisted. |
| `app/src/storage/managed_qualification.rs` | The six new sources bound into the source bundle. |

DuckDB 2.0.0 license: MIT (as recorded for APP-005, `evidence/APP-005/dependency.json`).

## Tests and results (commit `3a1b5f3` on `25f92d6`; Rust 1.93.1, `CARGO_BUILD_JOBS=2`)

File SHA-256 at `3a1b5f3`:

```
18c345c5426e0527fd399a54b825258374f341f0e24eeaff407b77af14af13f5  app/runtime/query_engine.py
56456e360f0c7bb4ca5265eb9fa81690c4b83d7257ecdbb81836597c7ae60bf8  app/runtime/runtime.py
c2e7aea0a3878296f71c098341f3eda7d8cf463c238686cec2fab4e25cb470c1  app/runtime/admission_operator.py
44afc33a9824c1db9e70df410cae2dadb85eeb20b5250950bf9233b54915df43  app/runtime/requirements.lock
dbc77a4a0665c2f15819eba5ed798c1d1f8cd24cfc47ff29cc1c60c3f2a5efb3  app/src/query/worker.rs
bca7341defb2de87e1033e78172bd519d9f510eb49a42edbdae0433b7ae00449  app/src/query/receipt.rs
9830e823064c82b9eff24b4524e17703734670aaf14f0f24578b2620ac46ae3d  app/src/storage/queries.rs
86826c7a366539d6405103159a0218cca94bcc1a9ce573728f4f4c0c489d63e5  app/Dockerfile
```

`PY` = a scratch Python 3.11.0 venv installed from `app/operator/requirements.lock` with `--require-hashes --only-binary=:all:` (the union of the runtime, contracts and APP-005 locks: duckdb 2.0.0, jsonschema, psycopg, boto3).

| Command | Result |
| --- | --- |
| in `app/admin`: `npm ci && npm run build` | exit 0 (assets for the `RustEmbed` build) |
| in `app/`: `cargo +1.93.1 fmt --check`; `cargo +1.93.1 clippy --locked --all-targets -- -D warnings` | exit 0; exit 0 |
| in `app/`: `cargo +1.93.1 test --locked` | lib **82 passed / 9 ignored**; bin 1; doctests 2. New: `receipt_vectors_match_contracts` (both edition-1 contract receipts keep their digest under the Rust identity; the seeded `total` view's query digest equals the receipt's `query_digest`; a built receipt has exactly the schema's members), `admission_bounds_two_per_organization_four_per_task_then_429` (30 concurrent for one organization: 8 rejected at once, peak 2 running; 9 across three organizations: peak 4 per task, 2 per organization), `worker_is_killed_at_deadline_and_reports_failed_not_zero` (`sleep 30` killed at the backstop, scratch removed; a child `failed`, an empty or extra line, a number cell, `partial: true`, a nonzero exit are all `Failed`; refusals keep their code; a spent deadline launches nothing), `engine_child_has_no_credentials_or_network_config` (empty environment, descriptors 0-2 only, cwd = private scratch; scratch inside the cache refused), `uncached_or_mismatched_revision_is_unavailable_and_never_executes` (no cache and no fetch: nothing launched; fetched bytes that miss `normalized_digest` refused and removed; matching bytes served), `parameters_are_closed_and_validated`, `effective_query_adds_window_and_destination_only`, and `source_bundle_covers_every_compiled_source` with the six new files bound |
| in `app/`: `CARGO_TARGET_DIR=target/msrv cargo +1.85.0 check --locked --lib --tests` | exit 0 |
| `PY -m unittest discover -s app/runtime` | **131 passed** (111 s; includes the Docker PostgreSQL operator tests). New `test_query_worker.py` (11): the real `python3 -I runtime.py query` child with `env={}` over the operator-shaped Parquet (SHA-256 = the uploaded `0c2582d8…a082`): `total` 1,140,000, destination `198.51.100.7` 900,000, `timeline` columns `[minute, destination, bytes]` whose per-minute sums equal `expected.json`'s non-zero `minute_bytes`; whole-revision `total` 2,040,000 (the window is the parameter); the `rate` ratio view → `operator_not_qualified`, the `destinations` lookup and Spend Review's money `queue` refused, a money literal on events → `literal_not_qualified`; another organization's cache, a symlink into this one and a wrong digest refused; `enable_external_access=false`, `lock_configuration=true`, autoload off, `threads=2`, reading a file outside the allowlist and `INSTALL httpfs` fail; `deadline_ms: 1` → `failed`, no rows; unknown request keys and a foreign `s3_key` refused; the cache fetch (botocore Stubber) writes `0400` and refuses a digest mismatch leaving nothing. New operator pure test: the record is closed and content-derived (`9161c1e4-…bf45` for Northstar) |
| `GENUI_OPERATOR_TEST_PARSER=<main>/parser/target PY -m unittest discover -s app/operator -p 'test_*.py'` | **29 passed** (the lock is still the verbatim union with `duckdb` now in the runtime lock) |
| `PY -m unittest discover -s scripts -p 'test_*.py'`; `python3 scripts/check-schema-pins.py` | 17 passed; `"agree": true` (schema version 5, catalog bound 524,288; no migration) |
| in `app/`: `GENUI_NATIVE_FIXTURE_PYTHON=PY GENUI_QUERY_REPORT=… cargo +1.93.1 test --locked --lib storage::query_qualification::native_query_campaign -- --ignored --exact --nocapture` | **1 passed, 8 checks** ([query-campaign.json](query-campaign.json)): register/create-bound-work (refusals `revision_exists_differs`, `record_revision_refused`, `record_digest_refused`, `record_organization_refused`, `creator_lacks_authority`; seeded rows unchanged); `authorized_member_gets_exact_result_and_receipt_digest` (1,140,000 / 900,000 / five buckets, each released, receipts contract-valid by `contracts/validate.py`); the seeded placeholder work is `SourceUnavailable`; denials before any engine (unknown view, form view, lookup view, unclosed or money parameters, foreign organization, member without access, stale epoch; no release to another principal or organization); `foreign_or_revoked_scope_denied_before_release_including_change_between_execution_and_delivery` (source_read revocation, creator-authority revocation and membership revocation each between execution and delivery); latency; database and files removed |
| in `app/`: `GENUI_QUERY_PYTHON=PY GENUI_QUERY_LATENCY_REPORT=… cargo +1.93.1 test --locked --lib query::tests::engine_child_campaign -- --ignored --exact --nocapture` | 1 passed ([child-latency.json](child-latency.json)) |
| in `app/`: `native_postgres_campaign`, `native_definition_campaign`, `native_admission_campaign` (same flags, run after this change) | 1 passed each: 36, 18 and 27 checks, `passed: true` (the shared storage modules are unaffected) |
| `docker build -f app/Dockerfile -t genui-app012:3a1b5f3 .` then `docker run --rm --network=none --read-only --cap-drop=ALL --security-opt=no-new-privileges -v <probe>:/probe:ro --entrypoint /usr/local/bin/python3 genui-app012:3a1b5f3 -I /probe/image_probe.py /probe/cache /probe/views.json` | image `sha256:5dde20d0…2399` (linux/arm64, 446,178,022 bytes, built from commit `3a1b5f3`): DuckDB 2.0.0 imports with no network; the image's own `python3 -I /app/runtime.py query` child with `env={}` as UID 65532 returns 1,140,000, 900,000 and the five buckets ([image-probe.json](image-probe.json), [image_probe.py](image_probe.py)). The image was removed afterwards; it is not a release candidate |

Docker: `docker ps -a` showed only `gatewasm-db-test-1` before and after every campaign. One `genui-native-…` container that appeared between runs belonged to a parallel session; this slice waited for it to finish and did not touch it.

## Measurement (design §5.2)

| Measure (macOS arm64 host unless noted) | p50 | p95 | p99 | max |
| --- | --- | --- | --- | --- |
| One engine child, sequential (40), wall in the Rust parent: admission, cache check, spawn, execution, parse (`engine_child_campaign`) | 142.9 ms | 157.2 ms | 164.8 ms | 164.8 ms |
| Same, inside the native campaign (40) | 141.7 ms | 150.0 ms | 151.7 ms | 151.7 ms |
| The child's own time, request read to result written | 80 ms | — | — | 101 ms |
| One wave of two concurrent children (20 waves) | 139.9–152.1 ms | — | 147.5–158.1 ms | 158.1 ms |
| Whole authorized path: `run_query` + child + `release_result` + receipt (20, TLS PostgreSQL in Docker) | 247.1 ms | 259.7 ms | — | 260.5 ms |
| The image's child under `--network=none --read-only`, linux/arm64 container, `subprocess` wall (40) | 115.9 ms | 139.4 ms | — | 152.5 ms |

Against the gate (D5): three waves of two at about 0.16 s each take about 0.5 s, inside the 1.8 s per-operation and 2 s six-view p99 budgets of design §5.2; the per-operation cost is about 1.3–1.6 times the 100 ms warm assumption. **Decision: child per request (D7.10 as written); the broker is not adopted.** To re-check in APP-078 on Fargate (vCPU and cold page cache differ).

## Open questions

1. **Audit of operator source and work seeding.** `register-source-revision` and `create-bound-work` write no audit event because D7.8's operator kinds name none. A `source_revision.registered` / `work.seeded` operator kind is an ADR-007 amendment and a policy change in a migration; deferred with the other operator-kind work (APP-084). Until then the retained command output is the record.
2. **Routes and page (second half).** `POST /v1/work/{app}/{work}/queries/{view}` should map `Error::Denied` → 404, malformed parameters (`parse_parameters` = `None`) → 422, `QueryError::NarrowTheQuery` → 422 `narrow_the_query`, `Busy` → 429 `Retry-After: 2`, `SourceUnavailable`/`Failed` → the page's failed state (503 for the JSON route), and `Refused` → 422; `release_result` must run immediately before the response body is written. `Engine::new(EngineSettings::image())` belongs in `main.rs` with the data store.
3. **The seeded work `…0069` keeps its placeholder binding** and, with no object of that digest in the cache, is `SourceUnavailable` (tested). The demo must open the work `create-bound-work` writes; the seeded one can be labeled or left unlinked.
4. **Inventory revision `…0071`** still carries placeholder digests; `total` and `timeline` never read it (only the `destinations` lookup does, refused until PH02), but the work's authorization requires `source_read` on it.
5. **The image** was built and probed locally (linux/arm64) only; the x86_64 CI image, `check_image.py`/`qualify_image.py` against it and the Fargate behavior of `/run/genui/cache` and `/run/genui/query-scratch` on the task volume are APP-078's. `EngineSettings::image()` assumes `/usr/local/bin/python3` (the pinned `python:3.11.14-slim-bookworm` base).
6. **RSS sampling** is every 100 ms, so a 130 ms child is sampled about once; the recorded peak (≈ 56 MB) is a floor, not a maximum.

## Corrections after the independent review (2026-10-30)

Recorded by the review-fixes slice ([review-fixes/README.md](review-fixes/README.md)); the text above is left as delivered.

- **D4, `source_revisions`:** the receipt now lists every source dependency of the work (`work_source_dependencies`, in `input_id` order), as `contracts/validate.py` `validate_bundle`, both Northstar fixtures and `contracts/README.md` require (review M3). The engine still reads only the view's one input. `release_result` requires the same dependency list.
- **D4, `record_snapshot_sequence`:** `organization_sequences` is on the tip (migration 0007). Phase 1 keeps the work-scoped `max(commit_sequence)`, which identifies the same visible revisions as ADR-009 D9.8's organization-wide S while no query reads a record input; the switch to D9.8's S with record inputs is APP-085 item 6 (review m8).
- **D3:** a window bound is validated as a contract `TIMESTAMP` under the admission validator's rule (a real day in years 1 to 9999), not only by field ranges (review M2).
- **Open question 1:** carried to APP-084 item 6 (review m9).
- **Admission (D7.10(3)):** the queue of 20 is per organization (review M1); a request that expires in the queue is 429, not 503 (review m2).
