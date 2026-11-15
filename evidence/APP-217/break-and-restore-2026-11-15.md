# APP-217: themed error pages for browser navigations (builder, 2026-11-15)

Branch `feat/app-217-error-pages`, built on `origin/main` `36f6acac` and rebased onto `164dd573` (the APP-216 ticket and docs; no code). After the rebase:

| Commit | Content | Before the rebase |
| --- | --- | --- |
| `3649cf6a` | The feature | `e106040b` |
| `fafc5614` | The HTTP smoke probe | `d906f61f` |
| `943b81df` | The surge-ring tweak | `a49eb07b` |
| `a980ae24` | The image allowlists | — |

The breaks and the first full check ran on the pre-rebase ids named below. The decisions are the coordinator's brief for [APP-217](../../Tickets/0-backlog/APP-217.md).

## Security invariant and how it is held

ADR-006 makes the uniform 404 indistinguishable whether a path is unknown, protected, called with a wrong method or undecodable. The page keeps that property for the same request headers:

- The layer passes the renderer an `ErrorView` and nothing else. That view holds the refusal (from status and code), the viewer variant (from cookie presence, 404 only) and the 429 wait (from the refusal's own `Retry-After`). The path, query, method, ids and names never reach it.
- The status never changes. The refusal's headers stay, and `finish` still runs last, so HSTS is present and `Allow` is absent. The page adds its type, its nonce CSP, its `Permissions-Policy` and its opener policy. `Cache-Control`, `nosniff` and `Referrer-Policy` keep the refusal's values.
- Anything the layer does not recognize, and any failed render, is the original answer byte for byte.

## Tests (written with the code, run before the proofs)

| Test | What it holds |
| --- | --- |
| `http::errors::tests::a_navigation_is_navigate_to_a_document_or_an_old_browser_asking_for_html` | The navigation test, over 19 header sets: duplicated, case-changed, iframe, frame, image, cors and same-origin metadata; the old-browser fallback; `POST` without metadata. |
| `http::errors::tests::the_wait_is_plain_words_from_retry_after` | 1 s "a few seconds" through 300 s "about 5 minutes" to days; an HTTP date or a negative value is "a little while". |
| `http::errors::tests::navigations_get_the_page_and_everyone_else_the_exact_refusal` | A counting fake renderer over `uniform`. Five cookie cases each give the right variant, a duplicated session cookie counts as signed out, and seven ways to miss give the same answer. Five non-navigation header sets get headers and bytes equal to the same router without the layer. |
| `http::errors::tests::the_other_refusals_keep_their_status_and_headers_and_name_no_viewer` | 403, 413, 429 (300 s and 2 s) and 503 keep their status, `no-store`, `no-referrer` and `Retry-After`, and name no viewer even with a member cookie. |
| `http::errors::tests::unrecognized_answers_and_failed_renders_are_the_original_refusal` | 409, another code, extra members, a bare 404, a JSON 200, a 4,097-byte body and a streamed body all pass through unread: the renderer is never called. A failing renderer and a renderer that answers a 500 both leave the original refusal. |
| `tests::browser_navigations_get_one_themed_404_whatever_the_path` (binary, real FMIR) | Through the real `org_admin` router: three variants by six requests, byte-identical apart from the nonce, with the full page policy, the copy and buttons per variant, and no marker. Four non-navigation header sets get exact JSON. Also covers the old-browser fallback and an untouched served route. |
| `tests::the_other_refusals_are_their_own_page_for_everyone` (binary, real FMIR) | 403, 413, 429 and 503 with their titles and words. The member and guest pages equal the signed-out page apart from the nonce. `Retry-After: 300` gives "about 5 minutes". The JSON callers are unchanged. |
| `app/runtime/smoke_http.py` | The release binary over a socket: a navigation to `/workspace?probe=zq7` is the 404 page with the full headers and nothing from the request, and the mascot is served as `image/svg+xml`. The JSON probes are unchanged. |

## Break and restore

Each break was applied by a script that asserted the replaced span was unique. The named tests were then run, and the file was restored. Break 1 was restored by copying back the saved original and checking its SHA-256. Breaks 2–4 ran on the committed head `d906f61f` and were restored with `git checkout --`, followed by an empty `git diff --exit-code`. After the last restore, `cargo test -p forma-app-platform --lib http::` passed 16 of 16, and the two binary tests passed.

| Break | File: change | Command | Caught | What failed | Restored |
| --- | --- | --- | --- | --- | --- |
| B1: the layer is off | `app/src/http/mod.rs`: `.layer(middleware::from_fn(errors::error_pages))` removed from `uniform` | `cargo test -p forma-app-platform --lib http::errors`; `cargo test -p forma-app-platform --bin forma-app-platform -- themed_404 their_own_page` | yes, 4 tests | `navigations_get_the_page_and_everyone_else_the_exact_refusal` (`errors.rs:620`: got `{"code":"not_found","status":404}`, expected the page), `the_other_refusals_keep_their_status_and_headers_and_name_no_viewer` (`errors.rs:712`: the 403 JSON), `browser_navigations_get_one_themed_404_whatever_the_path` (`main.rs:764`: `application/problem+json`, expected `text/html; charset=utf-8`), `the_other_refusals_are_their_own_page_for_everyone` (`main.rs:894`, same) | yes, SHA-256 `9858325a…` matched |
| B2: the viewer logic is broken | `app/src/auth/cookies.rs`: `if read(headers, SESSION).is_some()` became `if headers.contains_key(header::COOKIE)`, so any cookie means member | the same two commands | yes, 2 tests | `navigations_get_the_page_and_everyone_else_the_exact_refusal` (`errors.rs:620`: `Some(Member)` for the guest cookie, expected `Some(Guest)`); `browser_navigations_get_one_themed_404_whatever_the_path` (`main.rs:788`: the guest's words "Ask the person who shared it for a new link." were missing) | yes, blob `c3979f52` = `HEAD` |
| B3: a partial body | `app/src/http/errors.rs`: the fallback `Response::from_parts(parts, Body::from(bytes))` became `Body::empty()` | `cargo test -p forma-app-platform --lib http::errors` | yes | `unrecognized_answers_and_failed_renders_are_the_original_refusal` (`errors.rs:748`: `/other-code` answered its headers with an empty body instead of `{"code":"missing","status":404}`) | yes, `git diff` empty |
| B4: a non-page render is sent | `app/src/http/errors.rs`: `.filter(is_page)` removed, so a renderer's 500 is sent as the page | `cargo test -p forma-app-platform --lib http::errors` | yes | `unrecognized_answers_and_failed_renders_are_the_original_refusal` (`errors.rs:766`: the injected renderer's `text/plain` "Internal Server Error" went out under the 404, instead of the original JSON) | yes, `git diff` empty |

## Screenshots

These come from one Playwright pass: the project's pinned Playwright 1.64.1 (`spikes/APP-003/admin`, `npm ci` in this worktree, no browser download) driving installed Chrome headless with `reducedMotion: 'reduce'`. It ran against `target/debug/forma-app-platform` on `127.0.0.1` with an OS-chosen port and the HTTP smoke's synthetic configuration (no registry, no database, no provider). The dev stand was not used, because its persona gate adds a session cookie to every browser request and so shows only the member variant. The 429 is real: after 15 failed session lookups from one address in a minute, `GET /workspace` with a synthetic session cookie, sent as a request header because Chrome refuses a `__Host-` cookie on plain http, answered 429 with `Retry-After: 59`, and the page said "Wait about a minute". All six documents loaded the mascot. No page had horizontal scroll at 1280, 375 or 320 wide. The only console line was Chrome logging the document's own 404 or 429 status.

| File | What it shows |
| --- | --- |
| [404-signed-out-1280.png](404-signed-out-1280.png) | 404, signed out, 1280 × 900, light |
| [404-signed-out-375.png](404-signed-out-375.png) | 404, signed out, 375 × 812 |
| [404-signed-out-dark-1280.png](404-signed-out-dark-1280.png) | 404, signed out, 1280, `prefers-color-scheme: dark` |
| [404-surge-1280.png](404-surge-1280.png) | 404 after clicking the mascot: the surge, static under reduced motion |
| [429-1280.png](429-1280.png) | 429, 1280 × 900 |
| [429-375.png](429-375.png) | 429, 375 × 812 |

The launcher and the Playwright script were one-off files in the session scratchpad, not committed. They used the smoke's configuration shape and one `node` run.

## Commands and results

Local macOS. The toolchains came from `just setup` in the worktree `forma-errors`. Docker Desktop was up. Each command ran once unless noted. CI was not observed by the builder.

- `just check` (every step, 7 min 40 s, at `a49eb07b` with the ticket and board uncommitted). 19 steps ok: fmt, admin (typecheck, tests, strict build), clippy, clippy-dev, http-smoke (with the new navigation probe), msrv (Rust 1.85.0), catalog, infra, py-scripts, py-runtime, shared-broker, py-app-scripts, py-ops, py-promotion, py-infra, schema-pins, contracts, board and bash-n. Two failed, with one cause: the new `app/src/http/errors.rs` was missing from the image allowlists.
  - `test`: `storage::managed_qualification::tests::source_bundle_covers_every_compiled_source`, because the source bundle lacked `errors.rs`.
  - `py-qualification`: `test_service_context_admits_every_compiled_source_and_only_tracked_files`, because the root `.dockerignore` lacked `errors.rs` and, checked next, the three admin files.
- **The fix** (commit "Admit the error page files in the image allowlists"):
  - The root `.dockerignore` admits `app/src/http/errors.rs`, `app/admin/src/error/app.tsx`, `app/admin/src/styles/error.css` and `app/admin/src/brand/forma-isotope.svg`.
  - `app/qualification/Dockerfile.dockerignore` admits `app/src/http/errors.rs`.
  - `SOURCES` in `storage/managed_qualification.rs` binds `errors.rs` (236 → 237).

  After the fix, `just check test py-qualification py-promotion py-ops contracts` was all ok: the library had 513 passed and 40 ignored, the binary 11 passed and doctests 3 passed, with 16 qualification tests.
- The **deployment contract digest** (`ops.promotion.candidate.deployment_contract_digest`) was `3493b0a68b08b686a245be7ffa602817772895c0f5cc8983c06ad78b7a3e2dd9` at `origin/main` `164dd573`. At the branch it is `8578d6ac6fc95d40cda7055a3724b194a0a3314c7344bd295b8aeb2c324b6c48`. The only contract files changed are `.dockerignore` (+10 lines) and `app/qualification/Dockerfile.dockerignore` (+1 line). Nothing changed under `app/migrations`, `infra/src`, the Dockerfiles, the operator dockerignore or `runtime.py`. The branch therefore needs a reviewed contract-only release (APP-210) to promote. This builder did not create one.
- Campaigns, each on its own Docker fixture and each passing with exit 0:
  - `just campaign session`
  - `just campaign invite` (15 checks)
  - `just campaign org-admin-routes` (5 checks)
  - `just campaign shared-identity`

  These compose `uniform` without the extension, so they prove that the routes and their refusals are unchanged.
- `python3 Tickets/_tools/board.py render` and `check`: OK, 215 tickets.
- The Playwright pass is under **Screenshots** above.
