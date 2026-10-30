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

## Integration onto `b497ede` (2026-10-29)

This branch (`3e17429`, `380b7d4`, on `6f0d1c3`) was merged with `--no-ff` onto the phase-one-spikes tip `b497ede`. The tip holds APP-013 (commands, migration 0007, the record page), APP-074 (invite entry, with the APP-009 review fixes) and the APP-078 preparation. The hashes and counts in the sections above are this branch as delivered. This section records the merged result. The JSON files beside this README (`query-campaign.json`, `session-campaign.json`, `size-report.json`, `image-contexts.json`) are re-recorded from the merged tree.

Conflicts and how they were resolved (both sides kept every time):

| File | Resolution |
| --- | --- |
| `app/src/auth/csrf.rs` | `Form::Command` (APP-013) and `Form::Query` (APP-012), each with its own name (`command`, `query`). |
| `app/src/auth/mod.rs` | `mod invite_tests` (APP-074) kept; `synthetic_issuer` and `tests` are `pub(crate)` (APP-012). |
| `app/src/auth/tests.rs` | APP-074's harness kept: the real `POST /invite/validate`, the operator-issued code with its provider capability, the `codes` map. That harness is now `pub(crate)` for the query campaign. `issue_code_in(organization)` picks the Northstar stand-in or the new optional `canary` stand-in, along with that tenant and slug. Both stand-ins share one capability directory, as in `invite_tests`. `member_in(organization, label)` signs in with the matching handle. APP-012's labeled local stand-in for `/invite/validate` (`invited_signup_in`) is gone: the code names its organization, so the real route serves both organizations. |
| `app/src/storage/query_qualification.rs` (auto-merged, then adapted) | The route campaign starts both loopback management stand-ins and passes `Auth::with_invitations` over their shared capability directory. Its Canary member comes through the real invitation route. |
| `app/src/main.rs` | Required SSR routes include `/record` and `/w/{app}/{work}`. APP-074's trusted proxies and invitations are kept on `Auth`. APP-013's `command_routes` take `auth_pages`. APP-012's `work_page` is bound before `finalize`, and `Engine::new(EngineSettings::image())` runs whenever the store is configured. The product router merges `auth`, `commands` and `work_routes`, and never sits beside the qualification harness. Both binary tests are kept (record page, work page). The work-page test also binds `invite` and `record`, so `finalize` judges the whole build. |
| `app/admin/build.ts` | Pages `/`, `/about`, `/invite`, `/signin`, `/auth/pending`, `/auth/callback`, `/workspace`, `/record`, `/w/{app}/{work}`. The `record` and `work` entry points and SSR entries are both kept. |
| `app/admin/src/styles/app.css` | Additive: APP-074's `.notice`/`.entry`/`.help`/`.entry-links` rules, then APP-012's work-page rules. No selector overlaps; the record page uses `.page`, `.notice`, `.record` and `.command`. |
| `.dockerignore` | Union of the allowlists: `record/app.tsx`, and `work/app.tsx` plus `forma-types.d.ts`. The qualification ignore file merged cleanly. |
| `app/src/storage/managed_qualification.rs` | `SOURCES` is the union, **79** entries: 76 on the tip plus `query/page.rs`, `query/routes.rs`, `query/route_tests.rs`. The `SEED_INPUTS` hashes were recomputed: all five equal the repository files (setup.json `7460…ec7c`, northstar.json `4563…d592`, spend-review.json `4a45…5650`, tenant_seed.py `73bd…fc2e`, validate.py `7096…b826`). No change was needed. |
| `app/src/query/route_tests.rs` (compile fix) | `NoPages` implements the new `Pages::invite` (APP-074). |

Open question 2 was decided and implemented, and open question 5 was assessed and not done (see Open questions). `GET /workspace` (`app/src/auth/routes.rs`) now answers `Err(Error::Denied)` with the uniform 404 and every other `resolve_session` error with 503 `unavailable`. That is the same rule as the query routes' `storage_refusal`, which was already correct and is unchanged. New campaign check 15 (`store_outage` in [query-campaign.json](query-campaign.json)) builds a second `Stores` against a closed loopback port, using `Fixture::unreachable_settings`, after the first `Stores` has closed. On `GET /workspace`, `POST /v1/work/{app}/{work}/queries/total`, `GET /w/{app}/{work}` and `POST /w/{app}/{work}`, the member's live cookie and a well-formed forged cookie both get 503 `unavailable` with `no-store`. All 8 requests finished in 1 ms, well before the 5 s deadline, so the 503 comes from the storage mapping and not the deadline. Without a cookie the answer is the uniform 404. The engine dispatch count stays 0.

Merged-tree results (Rust 1.93.1, `CARGO_BUILD_JOBS=2`; Node 22, npm 11). `PYC` is the runtime venv with the contracts site-packages, as in APP-013. `OP` is a Python 3.11.0 venv whose `pip freeze` equals `app/operator/requirements.lock` exactly.

| Command | Result |
| --- | --- |
| in `app/admin`: `npm ci && npm run typecheck && npm run build` | exit 0. `record.ir` 3,906 B and `work.ir` 3,573 B, 0 islands each. Every route except `/` has "no interactive region, no JavaScript". `/` is Forma 26,515 of 28,420 B and JavaScript 28,168 of 76,800 B. Page budgets: no route `over` and none `exceeded` ([size-report.json](size-report.json)). |
| in `app/`: `cargo fmt --check`; `cargo clippy --locked --all-targets -- -D warnings`, and with `--features qualification-harness` | exit 0 all |
| in `app/`: `cargo test --locked`; with `--features qualification-harness` | lib 112 passed / 12 ignored, bin 5, doctests 3; with the feature, lib 123 passed / 12 ignored, bin 5, doctests 3 |
| in `app/`: `CARGO_TARGET_DIR=target/msrv cargo +1.85.0 check --locked --lib --tests --bins` (also with the feature); `cargo check --locked --release --bin forma-app-platform` | exit 0 |
| `PYC -m unittest discover -s app/runtime` (includes the Docker PostgreSQL operator tests) | 132 OK |
| `GENUI_OPERATOR_TEST_PARSER=<main>/parser/target OP -m unittest discover -s app/operator -p 'test_*.py'` | 38 OK |
| `python3 -m unittest discover -s scripts -p 'test_*.py'`; `PYC -m unittest discover -s fixtures -p 'test_*.py'`; `OP -m unittest discover -s infra/test -p '*_test.py'` | 17 OK; 14 OK; 54 OK |
| in `infra/`: `npm ci && npm run typecheck && npm test` | exit 0; 26 of 26 pass |
| `python3 scripts/check-schema-pins.py` | `"agree": true`, every pin 7 |
| `<main>/contracts/.venv/bin/python contracts/lock.py check`; contracts unittest; `node contracts/verify_vectors.mjs` | lock matches 27 artifacts (edition `impl-2026-10-29.3`); 26 OK; 6 + 13 vectors match |
| `python3 Tickets/_tools/board.py check`; `git diff --check` | OK (88 tickets); clean |
| `PYC app/runtime/qualify_tenant_schema.py --output <scratch>` | `passed: true`, 359 checks, container removed |
| in `app/`, one at a time: `native_postgres_campaign`, `native_admission_campaign`, `native_definition_campaign`, `auth::tests::native_session_campaign`, `auth::invite_tests::native_invite_campaign`, `native_command_campaign` (`GENUI_NATIVE_FIXTURE_PYTHON=PYC`) | 1 passed each: 36, 27, 18, **18** ([session-campaign.json](session-campaign.json); APP-074's count on the tip), 11, 20 checks |
| in `app/`: `GENUI_NATIVE_FIXTURE_PYTHON=OP GENUI_QUERY_PYTHON=OP … native_query_campaign -- --ignored --exact` | 1 passed, **15 checks** ([query-campaign.json](query-campaign.json)): the 14 above plus the store-outage check. Authorized path p50 259.8 ms, p95 266.4 ms (20 samples). |
| in `app/`: `GENUI_QUERY_PYTHON=OP … query::tests::engine_child_campaign -- --ignored --exact` | 1 passed. The child alone: sequential p50 136.9 ms, p99 147.4 ms; in waves of two, p99 152.4 ms; peak RSS 54.5 MB. |
| `PYC fixtures/reset_trial.py --output <scratch>` | `passed: true`, all owned resources removed |
| `PYC app/runtime/smoke_http.py --binary app/target/debug/forma-app-platform` | passed: native HTML count 7, CSP nonces fresh, `no-store`, protected routes the uniform 404, startup refusals unchanged |
| BuildKit context exports: the root ignore file (`FROM scratch; COPY . /`) and the qualification Dockerfile's `inputs` target | All 79 `SOURCES` paths and every library source are in the qualification context. Every non-test tracked source is in the service context, and nothing leaks from `Tickets/`, `docs/`, `evidence/`, venvs or build output ([image-contexts.json](image-contexts.json)). The test-only files in the service context were already on the tip's allowlist, and this merge changed none of them. |

Docker: `docker ps -a` showed only `gatewasm-db-test-1`, another session's container, before and after every campaign. No image was built.

Merged-source SHA-256 for the files the merge changed (the others are as listed above):

```
2e55fbf89c0865c58eb89b208f5331d14b1631d0f733890c90104b44bba57850  app/src/query/route_tests.rs
dd220e4bffb11e174bd52b6c2189fb9a8ed1342c252e775664e2892d8adfef1a  app/src/main.rs
978a50b49cc44fc20cdacd6dd83a389a151e347216e67a31fe361c73206b0784  app/src/auth/routes.rs
e98ebaf8a6f3537ec5005d69635377d348b0484f35a4891dddac5b87457c1540  app/src/auth/tests.rs
bf7e20d0413bca448e047ea18e0cf1b529c1c5bd03df7d63075855032122e918  app/src/storage/query_qualification.rs
```

## Limits

- Sessions come from the simulated loopback issuer, not the deployed fAuth; the campaign renders through a recording renderer (the SSR page itself is the binary test). No browser was used; installed Chrome and the hosted path are APP-076's.
- No service image was built or qualified in this slice (`qualify_image.py`, the arm64/x86_64 image with the engine directories on the task volume, `check_image.py`); APP-078 rebuilds the candidate. `EngineSettings::image()` creates `/run/genui/cache` and `/run/genui/query-scratch` at startup when the store is configured.
- The `loading` state is server-rendered (decision 6); the page has no reload-restore of the last run (parameters are not persisted; a saved filter state is APP-025's).
- No receipt is stored (`query_receipts` is 0007, APP-013); the receipt travels with its result only.

## Open questions

1. **New problem codes** `invalid_parameters`, `query_refused`, `source_unavailable`, `query_failed`: confirm, or fold them into fewer codes; the ADR-007 D7.15 contract edition (APP-013) could list them.
2. **Store saturation → 503** (decision 1) differed from `GET /workspace`, which answered any `resolve_session` error with 404. **Aligned at integration (2026-10-29):** both now use one rule. The uniform 404 is for no cookie and for an invalid session (absent, ended, or `Error::Denied`). 503 `unavailable` is for every other storage error (`Unavailable`, `Deadline`, `Configuration`, `UnknownOutcome`). Those errors come from the store and are the same whoever asks, so a 503 says nothing about the cookie that was sent. Query campaign check 15 covers it. Still answering 404 for every `resolve_session` error, unchanged here: `POST /auth/signout-all` (APP-009) and the APP-013 command and record routes (`commands::routes::session`). Aligning them belongs to their owners.
3. **The JSON route without a window** computes the whole revision (2,040,000, truthfully receipted). Should the route require a window for views without one, as the page does?
4. **`POST /w/{app}/{work}`** (the no-JS run form) is a route the plan did not name; it carries the same refusals and a synchronizer token. Confirm, or replace it with an island when one is qualified.
5. The demo must open the work `create-bound-work` writes (storage half open question 3); `GET /workspace` does not yet link it (the home/work listing is outside this slice). **Not trivial at integration, so not done.** The link has to name a per-organization work ID that the member may open. That needs three things: an authorized listing in `storage` (works this member's grants can read, under the same fence as `work_outline`), a list prop on the native `workspace` page, and its tests. A link to a fixed ID would show the link to members who cannot open it, and it would be fixture-specific. This still needs an owner, as the work listing in APP-025 or a small follow-up.

## Corrections after the independent review (2026-10-30)

Recorded by the review-fixes slice ([../review-fixes/README.md](../review-fixes/README.md)); the text above is left as delivered.

- **Decision 1 (status mapping):** a request that expired in the admission queue is 429 `too_many_requests` with `Retry-After: 2` (nothing ran); a query interrupted at its deadline, late past it or killed at its memory line is 422 `narrow_the_query` (design §6.3); 503 `query_failed` is only an engine or protocol failure (review m2).
- **Open question 2 (store saturation → 503):** corrected. A lock wait past `lock_timeout` during session resolution is on this cookie's own member row, so it is not "the same whoever asks"; `resolve_session` answers it as `Error::Denied` (the uniform 404, ADR-007 D7.9) on `GET /workspace` and the three APP-012 routes alike. 503 `unavailable` stays for a genuine store outage (campaign check 15 unchanged; new lock-wait check). Review M4.
- **Open question 3:** no change; the whole-revision behavior is documented in the `routes.rs` header (review OQ3).
- **Decision 6 / m7:** the queued-out card keeps the `loading` state key but is labelled `busy`; an in-flight loading state needs an island (ADR-010 §4.5 defers it).
- **Limits, "no reload-restore":** superseded. The run form now answers `303` to the page with a run cookie, and the page runs the saved parameters under a fresh authorization (ADR-010 §1 act 4 "reload restores").
