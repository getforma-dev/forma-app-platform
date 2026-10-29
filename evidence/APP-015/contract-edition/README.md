# Contract edition impl-2026-10-29.2 (APP-015 preparation, ADR-009 D9.6)

Task 0.3 of [PHASE-2-EXECUTION-PLAN](../../../docs/planning/PHASE-2-EXECUTION-PLAN.md). Base commit `623fcd5`. This is the edition README that D9.6 opens. It records synthetic contract qualification only: no jobs, sources, receipts or problem responses exist at runtime yet.

## What the edition is

Edition `impl-2026-10-29.2` adds exactly D9.6's items. It also adds the plan's `payload_too_large` (D9.9 B17) and the digest domain `forma:job-payload:v1`. [contracts/README.md](../../../contracts/README.md), section "Edition impl-2026-10-29.2", maps each item to the contract.

- **Versioning.** A document's `schema_version` names its kind's shape:
  - Version 1 is edition `impl-2026-10-28.1`'s shape, unchanged and still admitted.
  - Version 2 is the closed edition-two shape. It applies to `app_definition`, `publication`, `source_revision`, `query_receipt` and `job_state`.
  - `saved_work`, `command_request` and `command_outcome` did not change and stay at version 1.
  - An edition-two definition requires an edition-two publication and receipt.
- **Retention (ADR-002).**
  - Edition one's four fixture files are kept byte-identically in `contracts/fixtures/impl-2026-10-28.1/`. The lock's `retained_editions` records their original identities, and `lock.py check` refuses any drift.
  - Every edition-one canonical vector is copied verbatim into the edition-two `canonical-vectors.json`.
  - All 57 edition-one negative vectors still run against the edition-one bundles: in Python, and in Rust for the 19 that touch a definition.
  - Stored edition-one definition digests admit unchanged at schema version 1.
- **Changed vectors.** Two edition-one negative vectors change meaning in edition two. They are not repeated in the edition-two file, but they still run from the retained file.
  - "unknown version refused" uses schema version 2, which is now registered. It is still refused, now because the edition-one document lacks `coverage`.
  - "partial total refused": `partial` is now a boolean whose meaning is partial coverage.
  - Their edition-two counterparts are "unregistered schema version refused" and "partial receipt over complete coverage refused".
- **Rust.** `admission::admit_definition` admits schema versions 1 and 2. Version 2 requires `coverage` on every input, with records inputs complete. It also admits the `window_minutes` ratio denominator, which requires a whole-minute window.
  - `admission::exact_rational`/`check_exact_rational` reproduce validate.py's reduction and checks.
  - `digest::NORMALIZED_ROWS`, `digest::JOB_PAYLOAD` and the streaming `digest::normalized_rows` reproduce the new domains.
- **PH01 seed.** `app/runtime/tenant_seed.py` now reads the retained edition-one fixtures. PH01 operator-seeded data is edition one: its revisions have NULL `rows_digest` (D9.3), and nothing rebinds (C05). Without this change, the seed would have stored edition-two content as schema version 1.

## Erratum

**`forma:job-payload:v1`.** D9.4 names `genui.jobs.payload_digest` but gives it no domain. The Phase 2 plan adds this domain: `payload_digest = sha256("forma:job-payload:v1" ‖ 0x00 ‖ forma-json-v1(payload))` over the complete job payload. Nothing else changes.

## Decided here (for review)

- **Row-stream framing.** `forma:normalized-rows:v1` frames the row stream as one forma-json-v1 array of typed rows in sorted order. The identity is therefore the ordinary `digest` of a value, and Rust hashes it row by row. Each row is an object mapping field id to contract value. The unknown-value representation for out-of-int64 text (D9.3) is left to APP-016/018.
- **`queue_position`** is the organization position `enqueue_job` returns, starting at 1. A claimed-but-not-started job reads `queued` without a position and may carry a pending cancel request (D9.4 steps 2–3).
- **Problem bodies.**
  - Within this edition's bodies, a 429 is `limit_exceeded`, naming one of the five limits (`organization_queue`, `task_queue`, `imports_per_day`, `stored_original_bytes`, `queries_per_hour`) and `retry_after_seconds`.
  - The HTTP status of `source_deleted`, `source_quarantined`, `unknown_value_in_aggregate`, `lookup_ambiguous` and `resource_limit` is not fixed by D9.6 and is left to their tickets.
- **Identity lists.** Per-operator and per-renderer versions equal the registry edition (1) in this edition, so the lists are derivable from the definition until a later edition raises one version.

## Not expressible, or not expressed, in this edition

- **`stale`** is a display state, a card older than its ADR-002 authorization freshness. It is defined in the README; no contract document carries it.
- **Parser refusal codes** are carried verbatim through the open `failure_code` identifier grammar. They are not a closed enumeration, because the parser is a spike until APP-017.
- **HTTP status** for five of the new problem codes, as noted above.

## Commands and results

`red-python.txt`, `red-javascript.txt` and `red-rust.txt` record the failing state, taken after the tests were written and before the implementation:

- Python: `ImportError` (`domain_name` and the other edition-two names are absent).
- JavaScript: `AssertionError: normalized-rows vectors`.
- Rust: `cannot find value NORMALIZED_ROWS / JOB_PAYLOAD`, `normalized_rows`, `exact_rational`, `check_exact_rational`.

The commands were:

- `contracts/.venv/bin/python -m unittest discover -s contracts -p 'test_*.py'`
- `node contracts/verify_vectors.mjs`
- `cargo +1.93.1 test --locked --lib admission::`

`green.txt` records every final check with its exact command and exit status, all `exit=0`:

| Check | Result |
| --- | --- |
| Contracts unittest | 23 tests OK |
| `verify_vectors.mjs` | 6 edition-one and 13 edition-two vectors match in JavaScript |
| `lock.py check` | 22 artifacts, 4 retained edition-one artifacts, both frozen sources |
| PH01 seed bundle build | 3 bundles, schema version 1, original digests |
| `cargo +1.93.1 fmt --check` | clean |
| `clippy --locked --all-targets -- -D warnings` | clean |
| `cargo +1.93.1 test --locked --lib admission::` | 11 passed |
| `cargo +1.93.1 test --locked` | 29 passed, 2 ignored, plus 1 and 1 |
| `CARGO_TARGET_DIR=target/msrv cargo +1.85.0 check --locked --lib --tests` | ok |

Environment notes:

- Python 3.11.0, with `contracts/.venv` installed from `requirements.lock` with `--require-hashes`.
- Node v22.21.0.
- `CARGO_BUILD_JOBS=2` for every cargo command.
- `app/admin` was built first with `npm ci && npm run build`.

The two ignored tests are the operator-only managed qualification and the Docker-backed `native_postgres_campaign`. Neither was run. The campaign seeds through `tenant_seed.py`; its edition-one input is shown unchanged by the seed check above.

Python, JavaScript and Rust produce identical bytes and digests for every canonical vector. Rust also reproduces both editions' fixture definition, input-contract, query, receipt and command-payload digests.

## Review

[independent-review.md](independent-review.md) has the independent review's findings and their dispositions.
