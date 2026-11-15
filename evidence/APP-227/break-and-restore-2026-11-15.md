# APP-227: notice v2 (builder, 2026-11-15)

Branch `feat/app-227-notice-v2`, built on `origin/main` `d00b9395` (release F's record). Release G's content: `NOTICE_VERSION = 2` and the approved v2 text in `app/src/auth/notice.rs`, the one source every renderer reads (`/invite` through `InviteProps::notice()` in `app/src/pages.rs`, the public `/notice` page in `app/src/main.rs`, the reservation in `app/src/auth/invite.rs:401`); the ADR-008 amendment "continuation after the class"; the class guide's code email; runbook §10 and §11.4 retitled to deletion on request, with the operator note for Victor's pre-v2 accounts in §11.5; the readiness table's "by Dec 14" row; the docs index row. No migration, no contract file.

The decisions are Victor's of 2026-11-15 (keep everyone by default; the free plan with a small AI allowance plus own AI through MCP; deletion on request at privacy@getforma.dev, confirmed within 7 days and done within 30; keep-and-anonymize for entries in other owners' apps; the beta planned to Jan 15 through release H). The text is the plan's "Notice v2 draft", verbatim.

## Where version 1 was pinned, and how each pin now follows the one source

| Pin | Before | After |
| --- | --- | --- |
| `app/src/auth/notice.rs` module test | `NOTICE_VERSION == 1`, "December 14, 2026", "feedback@getforma.dev", "Copy out", the v1 "Your data" paragraph word for word | `the_notice_is_version_two_and_general`: `NOTICE_VERSION == 2`; the title and checkbox label unchanged; "privacy@getforma.dev"; the v1 phrases refused ("December 14", "feedback@getforma.dev", "deleted after the test", "end of the test", "Copy out"); the cohort words refused (class, student, teacher, Victor); 18 v2 phrases; the "After a trial period" and "Your data" paragraphs word for word; the last paragraph |
| `app/runtime/smoke_http.py:90` (`just check http-smoke`) | `b'data-notice-version="1"'`, a pasted number | the version read from `app/src/auth/notice.rs` by a regular expression on `pub const NOTICE_VERSION: i32 = N;`, then `data-notice-version="N"` |
| `app/src/main.rs` `notice_page_is_public_and_shows_the_one_beta_notice` | already `NOTICE_VERSION`, `NOTICE_TITLE`, every `NOTICE_PARAGRAPHS` entry in order, as the invitation renders it | unchanged; passes with the seven v2 paragraphs |
| `app/src/auth/invite_tests.rs:1038, :1117, :1578` (the invite campaign) | already `notice::NOTICE_VERSION`: the reservation's `notice_version`, and after the redeeming sign-in exactly one `genui.member_consents` row at `NOTICE_VERSION` for the member, naming its `admission.consented` event | unchanged; with the constant at 2 the campaign proves a new redemption records `notice_version = 2` |
| `app/src/storage/org_admin_campaign.rs:184, :239, :247, :393` (the org-admin campaign) | already `NOTICE_VERSION`: the reservation, the consent row's count, the `admission.consented` digest `forma:notice-consent:v1` over the version (the digest's domain tag, not the notice version; `admit_redemption` in migration 0025 uses the same tag for every version), `/admin`'s `consent_version` | unchanged |
| `app/src/storage/admission_qualification.rs:37` | already `crate::auth::notice::NOTICE_VERSION` | unchanged |
| `app/src/main.rs:3398, :3436` | an `/admin` rendering fixture reads `Notice v1, 2026-11-29` for a synthetic member | left as is: a display fixture, not a pin of the current version; `/admin` renders `Notice v{version}` from each member's stored consent (`app/src/org_admin.rs:730`) |

Existing members: `app/src/storage/sessions.rs` never reads `genui.member_consents`, and no route compares a member's version with `NOTICE_VERSION`; nothing re-asks them, and no re-consent flow was added. Victor's pre-v2 accounts take an operator note (runbook §11.5, exact wording given there).

## Break and restore

The file was copied to the session scratchpad before the break; the break was applied by a script that asserted the replaced span was unique; the file was restored by copying back, and its SHA-256 matched.

| Break | File: change | Command | Caught | What failed | Restored |
| --- | --- | --- | --- | --- | --- |
| B1: version 1 with the v2 text | `app/src/auth/notice.rs`: `pub const NOTICE_VERSION: i32 = 2;` became `= 1;` | `cargo test --locked -p forma-app-platform --lib -- auth::notice` | yes, exit 101 | `auth::notice::tests::the_notice_is_version_two_and_general` panicked at `app/src/auth/notice.rs:55:9`: `left: 1`, `right: 2` | yes, SHA-256 `00976d46…2c5d74bd` before and after; `NOTICE_VERSION` reads `2` |

Before the break, the same command passed: `test auth::notice::tests::the_notice_is_version_two_and_general ... ok` (1 passed, 571 filtered out).

## Screenshots

Pinned Playwright 1.64.1 (`spikes/APP-003/admin`, `npm ci` in this worktree with no browser download), driving installed Chrome headless with `reducedMotion: 'reduce'`, the light scheme, as APP-223 did. The pages come from `target/release/forma-app-platform` on `127.0.0.1` with an OS-chosen port and the HTTP smoke's synthetic configuration (`app/runtime/smoke_http.py`: no registry, no database, no provider). Full pages. The launcher and the script were one-off in the session scratchpad and are not committed.

| File | What it shows |
| --- | --- |
| [invite-1280.png](invite-1280.png) | `/invite`, 1280 wide: the v2 notice above the code field and the consent checkbox |
| [invite-375.png](invite-375.png) | `/invite`, 375 wide |
| [notice-1280.png](notice-1280.png) | `/notice`, 1280 wide: "Beta notice, version 2" |
| [notice-375.png](notice-375.png) | `/notice`, 375 wide |

Checked in the same pass on all four (status 200): `data-notice-version="2"` on the notice section; the title; the "After a trial period" paragraph with "continue on Forma's free plan"; "privacy@getforma.dev"; the sentence "Entries you made in apps that other people own stay with those apps, without your name."; no "December 14" and no "feedback@getforma.dev" in the page text; no horizontal scroll (`scrollWidth` equals `clientWidth`).

## Commands and results

Local macOS, toolchains from `just setup` in the worktree `forma-notice`, Docker Desktop up. CI was not observed by the builder.

- `cargo fmt -p forma-app-platform && just check && just campaign invite && just campaign session && just campaign org-admin-routes`, one chained run at the final tree, exit 0:
  - `just check`: all 21 steps ok (fmt 1s, admin 21s, clippy 69s, clippy-dev 60s, test 45s, http-smoke 37s, msrv 63s, catalog, infra 8s, py-scripts 5s, py-runtime 243s, shared-broker 10s, py-qualification 2s, py-app-scripts 1s, py-ops 35s, py-promotion 14s, py-infra 5s, schema-pins, contracts 7s, board 1s, bash-n). `http-smoke` is the modified smoke, reading the version from the source.
  - `just campaign invite`: `APP074 invite campaign: 15 checks passed`; `native_invite_campaign ... ok` (31.6s). Its D5 check is the one that proves a redemption stores `member_consents.notice_version` at `NOTICE_VERSION`, now 2.
  - `just campaign session`: `native_session_campaign ... ok` (30.5s).
  - `just campaign org-admin-routes`: `org-admin routes campaign: 5 checks passed`; `native_org_admin_routes_campaign ... ok` (7.7s).
- `python3 Tickets/_tools/board.py check`: `OK: 225 tickets; 9 phases; 15 sprints; 80 requirements; design digests and active links verified.` A first run refused the Markdown links to the APP-227 to APP-232 tickets and to `CLASS-CONTINUATION-PLAN.md`, which PR #156 carries and this branch does not; those references are plain text until #156 merges.
- The **deployment contract digest** (`ops.promotion.candidate.deployment_contract_digest`) at `origin/main` `d00b9395` is `bd3c6437682bcc295260b476e069fd6c49f21956ae383ea5acbe539df2c38d44`. At the branch head it is `bd3c6437682bcc295260b476e069fd6c49f21956ae383ea5acbe539df2c38d44`, the same value as `origin/main` `d00b9395`: no contract file changed, so release G promotes without a contract-only release.
