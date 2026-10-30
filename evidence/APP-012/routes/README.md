# APP-012 route and page half: author evidence

**Author evidence, not acceptance.** This covers plan Task 9's second half on top of the storage and worker half ([../README.md](../README.md), merged at `6f0d1c3`): the JSON query route, the work page with its view states and receipt inspector, and the engine wired into the binary. No independent review has run (Tier A for authorization at delivery, Tier B for the page and the receipt encoding, ADR-010 §4.1), so no acceptance box is checked and no ticket was moved. Nothing touched AWS. Everything ran on one Apple-silicon Mac (Darwin arm64) with local Docker; every container this slice started was removed. No `Tickets/` or `docs/` file was edited.

Controlling text: [APP-012](../../../Tickets/0-backlog/APP-012.md); plan Task 9; ADR-007 D7.9, D7.10, D7.14, D7.15; design §7.2, §9, §11.3; ADR-010 §1 act 4 and §4.5 scope locks (`total` and `timeline` only; SSR pages; no new island); the storage half's open question 2 (status mapping).

## What was built

| File | What |
| --- | --- |
| `app/src/query/routes.rs` (new) | `POST /v1/work/{app}/{work}/queries/{view}`, `GET /w/{app}/{work}`, `POST /w/{app}/{work}`; the `Executor` trait (the engine in the binary, a dispatch counter in tests); the status mapping. |
| `app/src/query/page.rs` (new) | Typed `PageProps` of the work page and `card()`, the one place a view's outcome becomes what the page shows; `SsrWorkPages` renders a complete `PageResponse`. |
| `app/admin/src/work/app.tsx` (new), `app/admin/build.ts`, `app/admin/src/styles/app.css` | Forma 3.2 native page `work` at route `/w/{app}/{work}`: run form, one card per runnable view, receipt inspector (`<details>`: every receipt field plus the receipt JSON). No JavaScript, no island. |
| `app/admin/src/forma-types.d.ts` (new) | Consumer-only `createList` declaration for `tsc` (as `spikes/APP-003`); no runtime code. |
| `app/src/storage/queries.rs` | `Data::work_outline`: the same precheck, fence and pinned premise as `run_query`, the stored definition re-admitted, returning the views whose query reads exactly one input (the page lists only what the qualified subset can run; nothing is chosen by view or tenant name). |
| `app/src/auth/csrf.rs` | `Form::Query`, the run form's synchronizer token (tied to the session cookie). |
| `app/src/main.rs` (additive) | Requires the `/w/{app}/{work}` SSR route; binds `work` before `finalize`; `Engine::new(EngineSettings::image())` whenever the data store is configured (startup refuses if `/run/genui/cache` or `/run/genui/query-scratch` cannot be made private); merges the routes into the product router only (never beside the qualification harness). Binary test for the page. |
| `app/src/query/route_tests.rs` (new, cfg(test)) | Unit tests, the dispatch counter (`Counting`, with a one-shot pause between execution and release) and the recording page renderer. |
| `app/src/storage/query_qualification.rs` | `native_query_campaign` gains the route checks over real APP-009 sessions (below). |
| `app/src/auth/mod.rs`, `app/src/auth/tests.rs` | The APP-009 test harness becomes crate-visible (`pub(crate)`), and `issue_code_in`/`invited_signup_in`/`member_in` take the organization (the old helpers delegate), so the query campaign signs members in over the same routes. Test-only. |
| `.dockerignore`, `app/qualification/Dockerfile.dockerignore`, `app/src/storage/managed_qualification.rs` | New sources allowlisted in both contexts; `SOURCES` binds `query/page.rs`, `query/routes.rs`, `query/route_tests.rs` (66 files). [image-contexts.json](image-contexts.json): nothing missing from either exported context. |

## Decisions (for review)

1. **Status mapping** (storage half's open question 2, design §9, D7.9): uniform 404 for no, unknown, ended or refused sessions and for `Error::Denied` at run or at release; 503 `unavailable` only when the workspace or data store itself is unavailable or saturated (an identity-independent condition); 422 `invalid_parameters` for a query string or a body outside `{"parameters": {window?, destination?, filter_epoch?}}`; `NarrowTheQuery` → 422 `narrow_the_query`; `Refused` → 422 `query_refused` (the engine's code is not echoed); `Busy` → 429 `too_many_requests` with `Retry-After: 2`; `SourceUnavailable` → 503 `source_unavailable`; `Failed` → 503 `query_failed` (no `Retry-After`, so a browser never retries an execution failure). `query_refused`, `source_unavailable`, `query_failed` and `invalid_parameters` are new stable codes; design §9 already has these classes.
2. **Refusal order**: fetch metadata (403) → bounded body (64 KiB JSON, 4 KiB form; 413) → session (404/503) → run-form token (403) → parameters (422) → `run_query` → engine → receipt → `release_result` → body. Nothing reaches the engine before authorization; `release_result` runs after the body is serialized and immediately before it is returned (JSON) or rendered (page).
3. **The page is no-JS SSR** (ADR-010 scope lock): `GET` renders every runnable view `idle` and runs nothing; the run form `POST`s window and destination in its body (never the URL; a query string on a page route is the uniform 404). The window is required on the page (the S02 numbers need it); the JSON route accepts no window and then computes the whole revision (2,040,000, receipted with no parameters), which the campaign records.
4. **View states**: `idle`, `loading` (the engine admission is full: nothing computed yet, run again), `ready`, `empty` (a released result with no row or only unknown cells: "No rows matched", never 0), `failed` (with its reason), `denied` (authority lost at run or at release while the page itself was authorized). Only `ready` carries a value; only `ready` and `empty` carry a receipt. Values are the engine's exact decimal strings with digit grouping (`1140000` → `1,140,000`); an unknown cell shows `unknown`.
5. **Runs on the page are concurrent** (one task per view; the engine admits two per organization) so two views stay well inside the 5 s handler deadline; each result is released after all views computed.
6. **The `loading` state is server-side**: with no island, a request in flight cannot be shown; `loading` is what an admitted-but-queued-out run renders. A future island would render it while its `POST /v1/...` is pending.

## Tests and results (Rust 1.93.1, `CARGO_BUILD_JOBS=2`; final source hashes below)

`PY` = the scratch Python 3.11.0 venv installed from `app/operator/requirements.lock` (duckdb 2.0.0, jsonschema, psycopg, boto3), as for the storage half.

| Command | Result |
| --- | --- |
| in `app/admin`: `npm ci && npm run typecheck && npm run build` | exit 0. `work.ir` 3,573 B, 0 islands; `/w/{app}/{work}` "no interactive region, no JavaScript"; `/` Forma 26,514 of 28,420 B, JavaScript 28,168 of 76,800 B ([size-report.json](size-report.json)). The first typecheck needed the `createList` declaration. |
| in `app/`: `cargo fmt --check`; `cargo clippy --locked --all-targets -- -D warnings`, and `--features qualification-harness` | exit 0 all (one `large_enum_variant` fixed) |
| in `app/`: `cargo test --locked` | lib **96 passed / 10 ignored**, bin 3, doctests 3 |
| in `app/`: `cargo test --locked --features qualification-harness` | lib 107 passed / 10 ignored, bin 3, doctests 3 |
| in `app/`: `CARGO_TARGET_DIR=target/msrv cargo +1.85.0 check --locked --lib --tests --bins` (also with the feature); `cargo check --locked --release --bin forma-app-platform` | exit 0 |
| in `app/`: `GENUI_NATIVE_FIXTURE_PYTHON=PY GENUI_QUERY_PYTHON=PY GENUI_QUERY_REPORT=… cargo test --locked --lib storage::query_qualification::native_query_campaign -- --ignored --exact --nocapture` | **1 passed, 14 checks** ([query-campaign.json](query-campaign.json)): the 8 storage-half checks unchanged plus the 6 route checks below. Authorized path p50 247.9 ms, p95 251.9 ms (20 samples). |
| in `app/`: `GENUI_NATIVE_FIXTURE_PYTHON=PY GENUI_SESSION_REPORT=… cargo test --locked --lib auth::tests::native_session_campaign -- --ignored --exact --nocapture` | 1 passed, 17 checks ([session-campaign.json](session-campaign.json)): the APP-009 harness refactor changed nothing |
| `PY -m unittest discover -s app/runtime`; `GENUI_OPERATOR_TEST_PARSER=<main>/parser/target PY -m unittest discover -s app/operator -p 'test_*.py'`; `PY -m unittest discover -s scripts -p 'test_*.py'` | 132 OK (incl. the Docker PostgreSQL operator tests); 36 OK; 17 OK |
| `python3 scripts/check-schema-pins.py` | `"agree": true` (schema 6; no migration in this slice) |
| `PY app/runtime/smoke_http.py --binary app/target/debug/forma-app-platform` | passed: native HTML count 7, protected routes uniform 404, startup refusals unchanged |
| BuildKit context exports of the root and qualification `.dockerignore` (`FROM scratch; COPY . /`) compared with the sources | nothing missing ([image-contexts.json](image-contexts.json)) |
| `python3 Tickets/_tools/board.py check`; `git diff --check` | OK; clean |

Unit tests (no Docker): `anonymous_cross_site_and_unknown_session_requests_never_reach_the_engine` (403 first; anonymous, forged session, pre-access cookie, URL filter, other methods and undecodable paths all the uniform 404; 413 at 64 KiB + 1 and at 4 KiB + 1; dispatch counter 0, no page rendered), `refusal_mapping_matches_design_section_nine`, `filters_travel_in_a_closed_body`, `the_page_requires_the_window_and_reads_the_form_body_only`, `view_states_loading_empty_failed_denied_never_zero`, `display_groups_exact_decimals_only`, `storage::queries::tests::outline_lists_only_single_input_query_views`; binary `work_page_renders_every_view_state_without_a_fabricated_zero` (the real SSR module: each state's section, no `class="value"` and no `>0<` outside `ready`, receipts only on `ready`/`empty`, no script, CSP and `no-store`; [work-page.html](work-page.html) is its rendered output with synthetic props).

Route checks in the campaign (members signed in through the APP-009 routes against the `cfg(test)` synthetic loopback issuer; the APP-012 routes behind the dispatch counter wrapping the real confined engine):

1. `route_authorized_member_gets_exact_result_and_receipt_digest`: 1,140,000 for [10:00, 10:10), 900,000 for `198.51.100.7` at filter epoch 1, the five timeline buckets; each receipt contract-valid (`contracts/validate.py`), digest and `result_digest` recomputed, pinning her principal, the new work and the uploaded revision's real digest; without the window 2,040,000 with no parameters.
2. Work page: `GET` lists `total` and `timeline` idle and runs nothing; the run form gives the same numbers with contract-valid receipts, 900,000 for the destination, `empty` for a destination with no event; no window → 422 with every view idle; a forged token → 403; neither reaches the engine.
3. Filters in bodies only: a query string → 422 (JSON) / uniform 404 (page); unclosed, foreign-scoped or malformed bodies → 422; 64 KiB + 1 → 413; no dispatch.
4. Foreign, revoked, pre-access and anonymous: 32 requests (anonymous, a pre-access session, a pre-access cookie as a session cookie, a canary member replaying the Northstar ids, a member without work or source access, a revoked membership, the member after her own `source_read` revocation) on the JSON route, the page and the run form (with the session's own valid token) are all the uniform 404; the dispatch counter does not move.
5. Revocation between execution and delivery: the engine paused after computing 1,140,000; a `source_read` revocation turns the JSON response into the uniform 404 with no number (one dispatch); on the page a membership revocation in the same gap renders both views `denied` with no value or receipt, and the next request finds no session.
6. 429 under load: with the organization's engine admission full (2 running, 20 waiting; 1.5 s fake children), an authorized JSON request is 429 `too_many_requests`, `Retry-After: 2`, `no-store`; the page renders both views `loading`; the engine rejected exactly those 3; at most 2 ran at once; once drained the same request is 200.

A campaign finding fixed on the way: the storage checks' `Stores` must close before the route checks open theirs, because pool capacity (2 per role, 8 in all) is process-wide; with both open the page's second concurrent `run_query` waited past the handler deadline (503). One production `Stores` per task is unaffected.

File SHA-256 (final source):

```
7a45026f462523b8657c92c4835f2a2cf0d121541e740e6e6e5c0927c17d75be  app/src/query/routes.rs
d5fe40eeeff393c5b5cb6087dfb340a77dc3505889b43f944b3ef37f6f11510c  app/src/query/page.rs
be475eaf8287ecefcc24671ce8d9fb5df75f7a72a39ffa13175a7d2cb05ceb06  app/src/query/route_tests.rs
455a59736cf41597d707705b63be4053fa875812e67072633981aa173eec42ac  app/src/storage/queries.rs
eb0fc79ec8dd522f8fc93d8105f58e37cb07260b88ca2d3b88cbdd0b4d7f32bb  app/admin/src/work/app.tsx
4bb44f6ad9b1cf0c3f028b4acaedd2a9e3dadcd20350e84db4065bd05a1ac0ec  app/src/main.rs
```

Docker: `docker ps -a` showed only `gatewasm-db-test-1` before and after each campaign. Another session's `genui-native-…` containers appeared between runs; this slice waited for them to finish (one campaign at a time) and did not touch them.

## Limits

- Sessions come from the simulated loopback issuer, not the deployed fAuth; the campaign renders through a recording renderer (the SSR page itself is the binary test). No browser was used; installed Chrome and the hosted path are APP-076's.
- No service image was built or qualified in this slice (`qualify_image.py`, the arm64/x86_64 image with the engine directories on the task volume, `check_image.py`); APP-078 rebuilds the candidate. `EngineSettings::image()` creates `/run/genui/cache` and `/run/genui/query-scratch` at startup when the store is configured.
- The `loading` state is server-rendered (decision 6); the page has no reload-restore of the last run (parameters are not persisted; a saved filter state is APP-025's).
- No receipt is stored (`query_receipts` is 0007, APP-013); the receipt travels with its result only.

## Open questions

1. **New problem codes** `invalid_parameters`, `query_refused`, `source_unavailable`, `query_failed`: confirm, or fold them into fewer codes; the ADR-007 D7.15 contract edition (APP-013) could list them.
2. **Store saturation → 503** (decision 1) differs from `GET /workspace`, which answers any `resolve_session` error with 404. Align APP-009's route, or keep 404 there.
3. **The JSON route without a window** computes the whole revision (2,040,000, truthfully receipted). Should the route require a window for views without one, as the page does?
4. **`POST /w/{app}/{work}`** (the no-JS run form) is a route the plan did not name; it carries the same refusals and a synchronizer token. Confirm, or replace it with an island when one is qualified.
5. The demo must open the work `create-bound-work` writes (storage half open question 3); `GET /workspace` does not yet link it (the home/work listing is outside this slice).
