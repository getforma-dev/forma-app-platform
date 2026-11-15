# APP-223: site footer and the Beta terms page (builder, 2026-11-15)

Branch `feat/app-223-site-footer`, built on `origin/main` `b1c7fcd6` and rebased onto `18d67818` (release D close-out and the observability work; no app code). After the rebase:

| Commit | Content |
| --- | --- |
| `b3194676` | The footer, the notice page, the canvas links and their tests |
| `ab6a7a92` | The HTTP smoke probe for `/notice` |
| `7760cc03` | The service image allowlist |
| `05ddad24` | The page-set test names the page whose footer is missing (found by B1) |
| `742e592e` | The canvas's Feedback, Beta terms and About under one Help menu (found by the canvas measurement below) |
| `3834ae11` | The coordinator's scope addition: Contact mails support@, a Security link, the privacy@ sentence on `/notice`, and `/.well-known/security.txt` |

The decisions are the coordinator's brief for [APP-223](../../Tickets/0-backlog/APP-223.md) and its scope addition the same day.

## The invariant and how it is held

ADR-006 makes the uniform 404 one answer whether a path is unknown, protected, called with a wrong method or undecodable. APP-217's error page keeps that for the same request headers. The footer must not break it:

- `SiteFooter` takes no props. It is the same bytes on every page and every error variant, so it cannot carry anything from the request.
- The APP-217 test still compares six ways to miss, for each of three viewers, byte for byte apart from the nonce. It now also checks that the 404 page's footer equals the one the renderer puts on the sign-in page, and judges the page's own copy (the buttons, no `://`) with the footer taken out. The footer's external links are the only absolute URLs on the page.
- `/notice` reads no session and sets no cookie. Like `/about`, it is a `get` route under `uniform`, so every other method is the uniform 404 with the unknown path's headers.

## Tests (written with the code, run before the proofs)

| Test | What it holds |
| --- | --- |
| `tests::document_pages_carry_the_one_site_footer_and_the_canvas_none` | 19 renders through the real FMIR: `/`, `/about` and `/notice` through the binary's own routes (`site()` under `uniform`); sign-in with and without a handle, invite, pending, denied, record, admin, work and shared app; the error page for the 404 of each viewer and for 403, 413, 429 and 503. Each has exactly one footer, the same bytes, after the page's `</main>`, with one `aria-label="Footer"`. `/canvas`, `/shared` and `/workspace` have none. The canvas has the two `target="_blank" rel="noopener"` links once each. |
| `tests::the_site_footer_links_are_pinned` | The nine links, in order, as exact markup. The count of `href=` equals the count of links. No `target`, form, script, image, input, button or id. The three group labels and the bottom line. |
| `tests::notice_page_is_public_and_shows_the_one_beta_notice` | `GET /notice` is 200 `text/html` with the nonce CSP (`default-src 'none'`, `frame-ancestors 'none'`, `form-action 'self'`), `no-store`, `nosniff`, HSTS and no `Set-Cookie`. It has one `<h1>`, the sentence with its mail link, `data-notice-version` and "Beta notice, version" from `NOTICE_VERSION`, the title, and every paragraph of `NOTICE_PARAGRAPHS` in order, each exactly as the invitation page renders it. `HEAD` has no body. `POST`, `PUT`, `PATCH` and `DELETE` on `/notice` and on `/about` give the uniform 404 JSON with no `Allow` and the unknown path's headers. |
| `tests::browser_navigations_get_one_themed_404_whatever_the_path` (APP-217, adjusted) | As before, plus the footer equality and the page-without-footer checks above. |
| `tests::security_txt_is_the_exact_static_file` | `product::security_txt`, under `uniform` and the error-page layer as the binary composes it: `GET` gives the exact four lines (pinned in the test as well as in the constant) as `text/plain; charset=utf-8` with `no-store`, `nosniff`, `no-referrer` and HSTS, and no cookie or page policy. A browser navigation gets the same text, not the themed page. `HEAD` has no body. `POST`, `PUT`, `PATCH` and `DELETE` give the uniform 404 JSON with the unknown path's headers; a browser's `POST` gets the themed 404. |
| `app/runtime/smoke_http.py` | The binary over a socket: `GET /notice` is the no-store page with the policy, no cookie, the notice version and the footer; `POST /notice` is the uniform 404 JSON. `GET /.well-known/security.txt` is the exact bytes with `nosniff` and `no-referrer`, so the product router mounts it; `POST` on it is the uniform 404 JSON. |

## Break and restore

Each break was applied by a script that asserted the replaced span was unique. The admin bundle was rebuilt (`npm run build` in `app/admin`) where the break was in a page source. The named tests were then run and the file restored. B1 and B2 were restored with `git checkout --` and an empty `git diff --exit-code`. B3 was restored by copying back the saved file, because the tree also held the uncommitted test change that B1 prompted, and its SHA-256 matched (`b281c54b…`). After the last restore and a rebuild, `cargo test -p forma-app-platform --bin forma-app-platform` passed 14 of 14.

| Break | File: change | Command | Caught | What failed | Restored |
| --- | --- | --- | --- | --- | --- |
| B1: no footer on `/signin` | `app/admin/src/signin/app.tsx`: the `<SiteFooter />` line removed | `cargo test -p forma-app-platform --bin forma-app-platform -- site_footer themed_404` | yes, 2 tests | `document_pages_carry_the_one_site_footer_and_the_canvas_none` (`main.rs:1219`: "/signin: one site footer", left 0, right 1); `browser_navigations_get_one_themed_404_whatever_the_path` (`main.rs:1024`, the reference footer it takes from the sign-in page is missing). The first run, before `05ddad24`, failed in the helper without naming the page; the test now names it. | yes, SHA-256 `33318e44…` before and after |
| B2: an external link without `rel="noopener"` | `app/admin/src/site/footer.tsx`: the Blog link lost `rel="noopener"` | the same command | yes, 1 test | `the_site_footer_links_are_pinned` (`main.rs:1279`: the sixth link was `href="https://getforma.dev/blog">Blog`). The page-set test passed, as it should: the footer was still one and the same everywhere. | yes, SHA-256 `f9b6eb37…` before and after |
| B3: `/notice` for any method | `app/src/main.rs`: `.route("/notice", get(notice))` became `any(notice)` | `cargo test -p forma-app-platform --bin forma-app-platform -- notice_page` | yes | `notice_page_is_public_and_shows_the_one_beta_notice` (`main.rs:1380`: `POST /notice` answered 200 with the page, expected 404 and the uniform JSON) | yes, SHA-256 `b281c54b…` matched |

## Screenshots

Pinned Playwright 1.64.1 (`spikes/APP-003/admin`, `npm ci` in this worktree with no browser download), driving installed Chrome headless with `reducedMotion: 'reduce'`. The pages come from `target/debug/forma-app-platform` on `127.0.0.1` with an OS-chosen port and the HTTP smoke's synthetic configuration (no registry, no database, no provider), after `3834ae11`. The full pages are captured, so the footer shows.

| File | What it shows |
| --- | --- |
| [home-1280.png](home-1280.png) | `/`, 1280 wide, light |
| [home-375.png](home-375.png) | `/`, 375 wide: the groups stacked, their links in a row |
| [notice-1280.png](notice-1280.png) | `/notice`, 1280 wide |
| [notice-375.png](notice-375.png) | `/notice`, 375 wide |
| [notice-1280-dark.png](notice-1280-dark.png) | `/notice`, 1280 wide, `prefers-color-scheme: dark` |
| [404-1280.png](404-1280.png) | The signed-out 404 for `/no-such-page-zq7`, 1280 wide: the footer at the card's width |
| [404-375.png](404-375.png) | The same 404, 375 wide |
| [canvas-help-1280.png](canvas-help-1280.png) | The canvas top bar on the dev stand (Alex Rivera, clean profile), 1280 wide, the Help menu open |
| [canvas-help-375.png](canvas-help-375.png) | The same at 375 wide: the list fixed under the header, above the first-run name prompt |

Measured in the same pass:

- `/`, `/about`, `/notice`, `/signin`, `/invite` and the 404, each at 320, 375 and 1280 wide: no horizontal scroll (`scrollWidth` equals `clientWidth`), exactly one `footer.site-footer`, and it sits outside `<main>` (18 of 18).
- The footer's text, links and group labels against the page background: 6.21:1 in the light scheme and 9.21:1 in the dark one, on `/`, `/notice` and the 404.

### The canvas top bar

The brief put "Beta terms" and "About" as two more links beside Feedback. The first pass measured the canvas top bar on this worktree's dev stand (`just dev clean -d`, Alex Rivera, an organization admin with no name yet) at 761, 900, 1024, 1280 and 1440 wide. Its row already overflows on `main`: at 1280 the avatar and Sign out are cut off, and only at 1440 does everything fit, with no spare width. The two links widened the bar by about 125 px. At 1280 the name menu was then cut off as well, and at 1440 Sign out was cut off where `main` fits.

So `742e592e` puts Feedback, Beta terms and About under one "Help" `<details>` menu in place of the Feedback link. A short recheck on the same stand (canvas only, no page screenshots) compared the bar's `scrollWidth` with the Help menu against the same page with the menu swapped back to `main`'s single Feedback link:

| Width | Help menu | `main`'s Feedback link | Cut off with Help | Cut off on `main` |
| --- | --- | --- | --- | --- |
| 761 | 1036 | 1063 | Workspace, Help, Your name, avatar, Sign out | Workspace, Feedback, Your name, avatar, Sign out |
| 900 | 1061 | 1087 | Your name, avatar, Sign out | Your name, avatar, Sign out |
| 1024 | 1082 | 1108 | Sign out | avatar, Sign out |
| 1280 | 1323 | 1346 | Sign out | Sign out |
| 1440 | 1440 | 1440 | none | none |

At 375, 900, 1024, 1280 and 1440 the menu opened from the keyboard (focus, then Enter), and its three links were inside the viewport and on top (`elementFromPoint`). At 761 the menu's summary is in the part of the row that is already cut off, as Feedback is on `main`. At 375 the header row scrolls sideways (keyboard focus scrolls the summary into view), so the list is `position: fixed`. The first-run name prompt covered it until `:root:has(.cv-help-menu[open]) .cv-name-prompt` dropped the prompt below the top bar while the menu is open. The live chip's width changes with its state ("Live" or "Reconnecting"), so the screenshot, taken after a reload, shows more of the row cut off than the table's measurement.

The top bar's own overflow on `main` (Sign out cut off between 761 and about 1400 px) is outside this ticket. It is reported to the coordinator as a follow-up.

The launcher, the two Playwright scripts and the dev stand were one-off; the scripts lived in the session scratchpad and are not committed. The dev stand was stopped afterwards with `just dev clean stop`, which keeps its data volume.

## Commands and results

Local macOS, toolchains from `just setup` in the worktree `forma-footer`, Docker Desktop up. CI was not observed by the builder.

- `just check` at `7760cc03`'s pre-rebase twin (`107185d0`, on `b1c7fcd6`): 20 steps ok (fmt, admin, clippy, clippy-dev, test with the library at 514 passed and 40 ignored and the binary at 14 passed, http-smoke, msrv with Rust 1.85.0, catalog, infra, py-scripts, py-runtime, shared-broker, py-qualification, py-app-scripts, py-ops, py-infra, schema-pins, contracts, board, bash-n). One failed: `py-promotion`'s `test_committed_release_is_bound_to_this_tree`, because that base still held release D's committed record (`ops/promotion/schema-release.json`), which binds `main`'s contract digest and so refuses any change to it. `origin/main` `18d67818` closed release D and removed that record. After the rebase, `just check py-promotion` passed (281 tests, 5 skipped).
- `just check` at the code head `3834ae11` (after the rebase and the scope addition): all 21 steps ok (fmt, admin, clippy, clippy-dev, test with the library at 514 passed and 40 ignored, the binary at 15 passed and doctests 3 passed, http-smoke, msrv, catalog, infra, py-scripts, py-runtime, shared-broker, py-qualification, py-app-scripts, py-ops, py-promotion, py-infra, schema-pins, contracts, board, bash-n).
- Campaigns after the rebase, each on its own Docker fixture, each exit 0: `just campaign session`; `just campaign invite` (15 checks); `just campaign org-admin-routes` (5 checks); `just campaign shared-identity`.
- The break-and-restore commands are in the table above.
- `python3 Tickets/_tools/board.py render` and `check`: OK, 217 tickets.
- The **deployment contract digest** (`ops.promotion.candidate.deployment_contract_digest`) is `8578d6ac6fc95d40cda7055a3724b194a0a3314c7344bd295b8aeb2c324b6c48` at `origin/main` `18d67818` (and at `b1c7fcd6`). At the branch head it is `79263fbbdbf4c59ced39902f76b2a352f443c3a35dfad6a926b7e23912f081b1`. The only contract file changed is the root `.dockerignore` (+7 lines). Nothing changed under `app/migrations`, `infra/src`, the Dockerfiles, `app/qualification/Dockerfile.dockerignore`, the operator context or `runtime.py`. The branch therefore needs a reviewed contract-only release (APP-210) to promote. This builder did not create one.
