# APP-222 part A and APP-219: break and restore (builder, 2026-11-15)

Branch `feat/app-222-ai-usage`, built on `origin/main` `b1c7fcd6`. Each break below was made in the working tree after the tests were written and passing, run with the one test that guards it, and then undone by copying back the file saved before the break. The restored file was compared byte for byte with the saved copy (`cmp`), and the same test was run again and passed. No break was committed.

Command form for every run: `cargo test --locked -p forma-app-platform --lib -- <test name>`.

## Break 1: drop the usage-threshold once-guard

- **Guard**: `Marks::pass` in `app/src/composer/model.rs` reports a threshold only when its bit is not yet set (`if reached && self.0 & flag == 0`), and sets the bit under the cap's lock.
- **Break**: the condition became `if reached`, so every admitted call at or past 80 % reported again.
- **Test**: `composer::model::tests::usage_thresholds_fire_once_per_scope_per_day_under_concurrency` (process limit 10, member allowance 5, 30 concurrent calls by two members on each of two UTC days).
- **Result while broken**: FAILED. `left` held repeated `(80, "member", "ana", 5)` rows, one per call past the threshold, against the expected single row per scope, threshold and day.
- **Restored**: the test passed.

## Break 2: log the raw request path instead of the route template

- **Guard**: `http::server_errors` in `app/src/http/mod.rs` logs `route` from axum's `MatchedPath`, or `unmatched`.
- **Break**: `route` became the request's `uri().path()` whenever a route matched.
- **Test**: `http::tests::a_server_error_logs_the_route_template_never_the_raw_path` (a marker in the requested path).
- **Result while broken**: FAILED at `no raw path is logged`. The captured line was `"route":"/v1/boom/zq7-path-marker-4417"`.
- **Restored**: the test passed.

## Break 3: put the prompt into the `model.call` line

- **Guard**: the `model.call` and `model.call_failed` lines carry the source, outcome, latency, token counts, model names and member ids, and never the request's text.
- **Break**: `prompt = request.user` was added to the `ok` line in `log_ok`.
- **Test**: `composer::tests::each_source_logs_one_model_call_with_its_tokens_and_never_the_prompt` (a unique marker in the system and user text of every source's call).
- **Result while broken**: FAILED at `no prompt text is logged`.
- **Restored**: the test passed, and `app/src/composer/model.rs` matched its saved copy byte for byte.

## Limits

These are unit-level proofs on the stub model and in-process routers. They do not show a deployed log line, a CloudWatch metric filter or an alarm. APP-219's deployed acceptance (the filters match a synthetic event and each alarm reaches the inbox once) remains open.
