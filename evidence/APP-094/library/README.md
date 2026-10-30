# APP-094 prebuilt app library (ADR-011 pitch): author evidence

**Author evidence, not acceptance.** Built on the clone's sub-branch `pivot/library` (from `pivot/pitch-mvp` 9cf52c2); not merged. No independent review has run and no ticket was moved. Nothing touched AWS or `forma-app-platform`. Everything ran on one Apple-silicon Mac (Darwin arm64) with local Docker; every container this slice started was removed by its campaign. Data is labeled synthetic throughout (`contracts/fixtures/library/catalog.json` `synthetic: true`); no customer evidence, credential, address or key appears in this folder.

Controlling text: [APP-094](../../../Tickets/0-backlog/APP-094.md); [ADR-011](../../../docs/architecture/ADR-011-NOV-2-EMERGENCY-PIVOT.md) §3 invariants (verified identity, tenant isolation, receipts on every governed number, idempotent audited commands, model output as data through admission, no credential in evidence); APP-012's engine and its qualified subset ([../../APP-012/README.md](../../APP-012/README.md)).

## What the library is

Thirteen prebuilt business apps the composer's router can pick from, each a typed `app_definition` of edition `impl-2026-10-28.1` (the edition the operator seed places and the store re-admits, APP-010 part 1), admitted by the real `admission::admit_definition` (test `storage::queries::tests::every_library_definition_is_admitted_and_in_the_sidecar`), over one `source` input and the shared `decisions` record input with a `record_decision` form, using only the nine renderer kinds and the engine-qualified operators. Each runs on realistic synthetic data through the governed query path (`run_query` → confined engine child → `release_result` → contract-valid receipt).

| Library id | Title | Views (kind) | Source (rows): fields |
| --- | --- | --- | --- |
| `expense-review` | Expense review queue | totals (summary), queue (table), by_category (chart), by_status (status), decision (form), caution (note) | `expense_claims` (240): expense_id:text, time:timestamp, employee:text, vendor:text, category:enum, amount:money, status:enum |
| `vendor-spend` | Vendor spend breakdown | totals (summary), by_vendor (chart), by_category (chart), unpaid (table), decision (form) | `vendor_invoices` (180): invoice_id:text, time:timestamp, vendor:text, category:enum, amount:money, due:date, paid:boolean |
| `outbound-traffic` | Outbound traffic investigation | total (summary), by_destination (chart), timeline (timeline), largest (table), record_observation (form), caution (note) | `traffic` (360): time:timestamp, asset:text, destination:text, bytes:integer, protocol:enum |
| `incident-timeline` | Incident timeline | timeline (timeline), by_severity (chart), open (table), totals (summary), review (form) | `incidents` (96): incident_id:text, time:timestamp, severity:enum, service:text, status:enum, minutes_to_resolve:integer (nullable) |
| `access-review` | Access review tracker | pending (table), by_system (chart), admin_requests (summary), by_status (status), approval (form) | `access_requests` (150): request_id:text, time:timestamp, requester:text, system:text, access_level:enum, status:enum, approver:text (nullable) |
| `sales-pipeline` | Sales pipeline by stage | by_stage (chart), by_owner (chart), largest (table), totals (summary), review (form) | `deals` (120): deal_id:text, time:timestamp, account:text, owner:text, stage:enum, amount:money, close:date |
| `inventory-comparison` | Inventory comparison | by_warehouse (chart), by_product (chart), low_stock (table), totals (summary), reorder (form) | `inventory` (42): sku:text, product:text, warehouse:enum, on_hand:integer, reorder_point:integer, unit_cost:money |
| `support-queue` | Support ticket queue | open (table), by_channel (chart), timeline (timeline), totals (summary), triage (form) | `tickets` (260): ticket_id:text, time:timestamp, channel:enum, priority:enum, status:enum, customer:text, first_response_minutes:integer |
| `budget-vs-actual` | Budget vs actual | by_department (chart), by_month (chart), lines (table), totals (summary), variance_review (form) | `budget_lines` (28): department:enum, month:date, budget:money, actual:money |
| `audit-trail` | Audit trail viewer | recent (table), by_action (chart), denied (summary), timeline (timeline), review (form) | `audit_log` (320): time:timestamp, actor:text, action:enum, resource:text, outcome:enum |
| `cash-flow` | Cash-flow statement | by_direction (chart), timeline (timeline), recent (table), largest_debits (table), reconcile (form) | `transactions` (210): transaction_id:text, time:timestamp, description:text, counterparty:text, direction:enum, amount:money, balance:money |
| `hiring-funnel` | Hiring funnel | by_stage (chart), by_source (chart), by_role (chart), recent (table), screening (form) | `candidates` (170): candidate_id:text, time:timestamp, role:text, stage:enum, source:enum |
| `subscription-revenue` | Subscription revenue | by_plan (chart), by_status (status), active (summary), top_customers (table), review (form) | `subscriptions` (36): subscription_id:text, customer:text, plan:enum, mrr:money, started:date, status:enum |

Mined from the prior repositories (Victor, 2026-10-30): forma-generative-ui's widget registry gave the router block shape (`description`, `intent_examples`, `negative_examples`, `capabilities`) and the finance shapes (statement transactions with debit/credit/balance, daily cash-flow timeline, money-in/money-out KPI, reviewed tables and grouped charts); forma-vector-studio's module library is consumer-shaped (flip cards, forms) and carries no business search metadata worth reusing, so nothing was taken from it. Every library app is a typed definition through the platform's admission and the governed engine; no HTML or widget artifact was carried over.

## Files

| File | What |
| --- | --- |
| `contracts/make_library.py` (new) | Deterministic generator (seeded) of the whole `contracts/fixtures/library/` tree: CSVs, the 13 edition-1 bundles (definition, publication under the `owner_publish` policy row the seed already places, saved work, source revision with the REAL `content_digest`/`normalized_digest`, receipt, seeded decision command and outcome), and the catalog; `--check` refuses a stale tree. |
| `contracts/fixtures/library/data/*.csv` (new, 13) | Labeled synthetic rows in the normalizer's cell grammar (money as two-place dollars, `YYYY-MM-DDTHH:MM:SSZ` timestamps). |
| `contracts/fixtures/library/<app>.json` (new, 13) | The bundles; `validate_bundle` passes each. |
| `contracts/fixtures/library/catalog.json` (new) | The router sidecar (the catalog contract below). Pinned as `seed_library_sha256` in `managed_qualification.rs` SEED_INPUTS and embedded by `storage::queries` (so also in SOURCES). |
| `app/runtime/library_sources.py` (new) | CSV → Parquet normalization per contract type (text/enum VARCHAR, boolean, integer/money/decimal BIGINT, date, timestamp), DuckDB 2.0.0 one thread ZSTD, ordered by `time` then original record; `build --organization --cache-root` places `<cache>/o/<org>/<source>.<revision>.parquet` (0400) refusing any digest that differs from the catalog; `plan` prints the ADR-009 B1 object keys for the deploy. Never touches AWS. |
| `app/runtime/query_engine.py` | Deviation 6: the request's `source` carries `input_id` and the input contract's `fields` (from the re-admitted definition, never the client); `compile_query` types every column, literal and measure from them; `sum` needs a numeric field; a window needs the `time` timestamp field; date values encode as `YYYY-MM-DD`. POLICY, the one-file allowlist, the read-only private database, the closed request, the single result line, the 500-row page, the deadline and the fixed refusal codes are unchanged. |
| `app/runtime/tenant_seed.py` | `library_catalog()` (edition, synthetic label, per-fixture digest), `rewrite_organization`, `make_library_bundles(org)`; `seed()` itself unchanged. |
| `app/runtime/admission_operator.py` | `seed-library --organization <org>`: every library bundle through `tenant_seed.seed` in one transaction under the organization FOR SHARE; exact-match idempotent, no audit event (D7.8 names none), output is the record. Override `admission-seed-library` documented in `app/operator/overrides.json`. |
| `app/src/storage/queries.rs` | `QueryRun.fields()`; `view_query` returns the one source input's fields (a records-input view is `Denied`); `Parameters::apply(query, fields)` refuses a `window` without a `time` timestamp field and a `destination` without a `destination` text field (refused, never dropped: receipts record only what ran) and names the view's own input in the filter; `outline` lists only single-source-input qualified views; `LibraryEntry` and `Data::library_catalog` (openable works whose definition digest the embedded sidecar names, with its router metadata; same authorization clause as `openable_works`). |
| `app/src/query/worker.rs` | The child request carries `input_id` and `fields`; `typed_integer_columns(query, fields)` (integer, money and decimal columns and count/sum measures are digit-grouped); `integer_columns` unchanged for the events shape. |
| `app/src/query/routes.rs` | `GET /v1/library` → `{"schema_version": 1, "synthetic": true, "apps": [...]}` for the signed-in member; uniform 404 without a session or for a refused member; 503 on a store outage. |
| `app/src/storage/query_qualification.rs` | The native query campaign gained the library section (two checks, below). |
| `app/src/storage/managed_qualification.rs`, `app/qualification/test_tools.py`, `app/qualification/Dockerfile.dockerignore`, `app/operator/Dockerfile.dockerignore` | SOURCES (+catalog.json, lexicographic), SEED_INPUTS (+`seed_library_sha256`, the re-pinned `tenant_seed.py`), the image allowlists. |
| `app/runtime/test_library.py` (new), `app/runtime/test_query_worker.py`, `app/runtime/test_admission_operator.py`, `app/src/query/tests.rs`, `app/src/storage/queries.rs` tests | Below. |

## The catalog contract (what the composer reads)

`contracts/fixtures/library/catalog.json`, also served per organization by `GET /v1/library` (only the apps the member may open now, as `library_catalog` lists them):

```
{"schema_version": 1, "synthetic": true, "edition": "impl-2026-10-28.1", "window_field": "time", "destination_field": "destination",
 "apps": [{"library_id", "fixture", "fixture_sha256", "title", "purpose",
           "routing": {"description", "intent_examples": [≥4], "negative_examples": [≥3], "capabilities": [...]},
           "app_id", "definition_revision_id", "definition_digest", "publication_id", "work_id", "collection_id",
           "inputs": [{"id", "kind", "contract_id", "fields": [{"id", "type", "nullable", unit?|values?|currency?|scale?}]}],
           "views": [{"id", "kind", "title", "runnable", input?, "operators"?, "parameters"? ("window"/"destination"), command_id?}],
           "commands": [{"id", "operation", "writable_fields"}],
           "sources": [{"input_id", "source_id", "revision_id", "csv", "content_digest", "normalized_digest", "row_count", "input_contract"}]}]}
```

The route's entries carry `library_id`, `app_id`, `work_id`, `title`, `description`, `definition_digest`, `purpose`, `routing`, `inputs`, `views`, `can_write`. A view runs at `POST /v1/work/{app}/{work}/queries/{view}` with the closed parameters (`window` only when the input has `time`; `destination` only when it has `destination`; `filter_epoch`). Metadata only: nothing in the catalog is a grant, a result or an admission verdict.

## Engine coverage added (within the contract's operators; no new operator kind)

- `table`: `project` + `sort` (by projected fields, direction, stable tiebreak) + `limit` (page of ≤ 500, `truncated_page` exact).
- Grouped breakdown (`chart`/`status`): `aggregate` with `group_by` on declared fields, measures `count` (NULL-skipping), `sum` (integer/money; HUGEINT-exact as digit strings), `min`/`max` (any type, including date and timestamp), ordered by group key.
- `filter` literals typed against the column: text, enum, boolean, integer (unit-checked), money (USD, exponent 2 → minor units), decimal (scale-checked), date, timestamp; a mistyped literal is `literal_not_qualified`.
- `timeline`: `time_bucket` replacing `time` in place (60/300/3600/86400 s) then `aggregate` grouped by `time`; the window parameter still applies.
- Nullable columns: an all-NULL `max` is an unknown cell (`null`), never zero.
- Two contract facts the fixtures respect: admission resolves `sort` keys and `group_by` fields against the input's declared fields, so a grouped result cannot be sorted by a measure and a renamed bucket cannot be grouped (hence in-place buckets). `avg` is not in the contract's measure set and was not added.

## Tests and results (worktree at the commit named in the ticket log; Rust 1.93.1; `PY` = a scratch venv from `app/operator/requirements.lock`, `--require-hashes --only-binary=:all:`)

| Command | Result |
| --- | --- |
| `PY contracts/make_library.py && PY contracts/make_library.py --check` | 27 files, 13 apps; deterministic (`--check` exit 0 on a regenerate) |
| in `app/`: `cargo +1.93.1 fmt --check`; `cargo +1.93.1 clippy --locked --all-targets -- -D warnings` | exit 0; exit 0 |
| in `app/`: `cargo +1.93.1 test --locked` | lib **131 passed / 13 ignored**; bin 5; doctests 3. New/changed: `every_library_definition_is_admitted_and_in_the_sidecar` (13 admitted at edition 1, outline = sidecar's runnable views, ≥ 3 governed views each), `outline_lists_only_single_input_query_views` (records-input view not listed), `effective_query_adds_window_and_destination_only` (refusals without `time`/`destination`), `integer_columns_follow_the_typed_query` (typed money/count/sum), `seed_catalog_and_migration_pins_match_repository_files` (catalog pin, per-fixture digests), `source_bundle_covers_every_compiled_source` |
| `PY -m unittest discover -s app/runtime` | **172 passed** (174 s; Docker PostgreSQL operator tests included). New `test_library.py` (11): every fixture validates and the catalog names it; the generator is current; CSV grammar refusals; the 13 normalized revisions equal the catalog pins (0400, repeat build no-op, differing CSV refused); **every runnable view of every library app (52 runs) through the real `python3 -I runtime.py query` child with `env={}` equals an independent Python evaluation of the typed query (columns, rows, truncation)**; table ordering/paging exact (50 of ~108 submitted claims, truncated; 10 largest debits truncated; 28 budget lines not); grouped vendor spend exact and summing to 180 invoices; count/sum/min/max over money and nullable integers (open incidents skip NULL; an all-NULL group is unknown); boolean/integer/enum/money/date/timestamp filters exact and mistyped literals refused; daily buckets with and without a window; window on an input without `time` → `window_refused`; destination filter shape; foreign cache, wrong digest, other input, unpaged 320 rows served, undeclared sort field, text sum, deadline; `plan` prints B1 keys without AWS. `test_query_worker.py`: requests carry `input_id`/`fields`; closed field list refusals. `test_admission_operator.py`: `seed-library` places 13 apps once in Northstar (definition bytes = fixture canonical content, source revisions pin the real digests, author grants), repeat `changed: false` with an identical dump, a tampered row refuses whole, backfill + verify hold afterwards |
| `GENUI_OPERATOR_TEST_PARSER=<main>/parser/target PY -m unittest discover -s app/operator -p 'test_*.py'` | **39 passed** (allowlist is an exact list of tracked files with the library fixtures; `seed-library` is not a managed-form refusal) |
| `PY -m unittest test_tools` (app/qualification) | 7 passed (SEED_INPUTS = the entry table with `seed_library_sha256`; the catalog names the pinned edition and every fixture digest) |
| `python3 Tickets/_tools/board.py check` | OK: 97 tickets |
| in `app/`: `GENUI_NATIVE_FIXTURE_PYTHON=PY GENUI_QUERY_REPORT=… cargo +1.93.1 test --locked --lib storage::query_qualification::native_query_campaign -- --ignored --exact --nocapture` | **1 passed, 22 checks** ([query-campaign.json](query-campaign.json), `library_receipts`): `seed-library` 13 apps once, `changed: false` on the repeat, no audit event; `library_sources.build` caches the 13 revisions under the pinned digests; `library_catalog` lists 13 entries with router metadata for the granted member and none for the reviewer; expense `totals` (240 claims; count/sum/max over money, integer columns), `queue` (50 rows, truncated, amount descending), `by_category` (claims sum to 240), vendor `unpaid` (boolean filter), tickets `timeline` with a window (receipt `parameters` = 2), traffic `total` with a destination (receipt parameter and `filter_epoch` 2), each through `run_query`, the confined child, `release_result` and a receipt `contracts/validate.py` accepts; a window or destination on `inventory-comparison` and an ungranted member are `Denied` before any engine. The APP-012 checks and latency (child p50 ≈ 140 ms, authorized path p50 ≈ 255 ms) are unchanged |
| in `app/`: `native_definition_campaign`, `native_admission_campaign` (same flags) | 1 passed each |

Measured on macOS arm64 with the scratch venv; the earlier run of the operator suite through the session's `op-python` wrapper showed 7 failures that reproduce identically on the untouched main checkout (the wrapper's PYTHONPATH does not survive the suite's `python3 -I`, so `jsonschema` is missing): an environment artifact, not a code finding.

## Left out, honestly

- No `avg`, no measure-ordered breakdowns, no renamed-bucket grouping: the contract has no `avg` and admission resolves sort/group fields against declared inputs; a change there is a contract edition (ADR-002), not this slice.
- The library's Parquet objects are not in the bucket: `library_sources.py plan` prints the B1 keys and `build` writes the cache; the upload is the deploy's (APP-096) and this slice never touched AWS. Until then a live view on a library app is `source_unavailable` (no number, by design).
- `seed-library` grants the library apps to the Northstar fixture author (`…066`) only, as the other seeds do; a real member needs `grant-permission` (`discover`, `run` on each app; `record_read` on each work; `source_read` on each source), which audits each grant. No audit event for the seed itself (D7.8 names no operator seed kind; APP-084 item 6).
- The SSR work page renders `chart`/`status`/`timeline` library results as tables (its summary/table dichotomy is unchanged); the canvas renderers are `pivot/canvas`'s.
- The service image copies no `contracts/` directory although `app/src` embeds contract files at compile time (pre-existing for `northstar.json`; the catalog adds one more file to that situation) — noted for APP-089/APP-096, not changed here.
- Two `genui-native-*` Docker containers belonging to parallel sessions were running during these campaigns and were not touched; the campaigns here removed their own.
