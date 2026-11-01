# Shared authentication entry repair — November 1, 2026

This repairs an existing APP-139/154 acceptance path. It does not implement the later composed-app direction or claim a new hosted deployment/signup success.

The genuine guest continuation stopped on the shared app page. The original harness recorded **responses**, so its GET-only report did not prove that no POST was issued. The browser retained a shared-flow cookie; subsequent reconciliation found one issued broker invitation and no guest provider account. Preserve that invitation and the original failure packet. Do not reset it or blindly repeat registration to make the trial appear clean.

Read-only anonymous HTML and JSON showed both entry buttons and `can_sign_up=true`. The deployed HTML's CSP was `form-action 'self'`. A native form could POST to the local start route, receive the external provider's 303 handoff and then have that redirect blocked by the browser. The existing workspace invitation/sign-in renderer already handled this redirect-hop policy; the shared renderer did not. Its client controller does not remove anonymous signup forms.

## Repair

- `SsrSharedPages` receives only the trusted configured provider origin. Pages with `needs_sign_in` use the existing `PolicyExtensions::form_action` facility for that exact origin. Accepted/pending-acceptance shared pages and the account canvas keep self-only forms. Complete native PageResponse/CSP, nonce and no-store handling remain intact.
- Auth configuration now explicitly rejects wildcard hosts. URL normalization alone accepted `https://*.example.test`, which is inappropriate for an exact CSP origin. Paths, credentials, queries and non-HTTPS origins remain refused.
- A separate entry defect used the presence of any raw workspace cookie to suppress signup, while POST treated failed lookup as anonymous. GET and POST now share a fallible home-session resolver: absent/ended sessions may use recipient signup; a live session preserves its own tenant; lookup errors refuse rather than falling back to another tenant. Raw-cookie CSRF binding and live home cookies are preserved. This was a source-review finding, not the cause of the observed hosted CSP interruption.

## Evidence

The [browser report](browser-report.json) contains five passing cases on Chromium 156.0.8078.74. [browser.mjs](browser.mjs) serves actual native fixture HTML/headers and the actual built island assets through an owned loopback HTTPS server on port 7697. It performs real browser form submission and 303 navigation: self-only reproduces the failure; signup, sign-in and verification-pending sign-in reach the exact configured synthetic provider; a foreign origin remains blocked. Port 7697 was free before this run and the server was stopped afterward. The self-signed local certificate is generated temporarily and removed; TLS verification is relaxed only in this disposable test context. No real provider, mailbox, account, invitation or owner profile is used by the final browser test.

The [identity report](identity-report.json) records eight passing production-storage/HTTP checks, including the two added entry cases. Actual PostgreSQL session lookup covers absent, malformed, expired, revoked and missing-vault cookies; the native API offers signup and the subsequent CSRF-protected POST selects the exact recipient tenant. A real held membership-row lock causes GET to hide signup and POST to refuse with no flow, broker or PAR growth. The existing signed synthetic-provider journey still preserves a live other-organization home session, rejects hostile callback identities, handles account caps/signout, and creates no membership by guest acceptance. This is local synthetic-provider qualification, not hosted mail or signup proof.

The native regression also checks the configured/absent-origin policy, anonymous and verification-pending buttons, wrong-account neutral entry, accepted shared page and shared canvas. Unit coverage rejects non-exact configured origins and distinguishes unavailable home storage from anonymous identity. The affected check run passed: fmt, clippy, clippy-dev, 364 library tests, 8 native binary tests, 3 doctests, HTTP smoke and Rust 1.85 MSRV. Locked admin native build, browser syntax, board and diff checks also passed. This is the affected local check set, not a claim that every remote or deployment gate ran. Independent source review by the separate frontend/auth reviewer found no remaining blocker in the combined policy/session change; the expanded campaign passed afterward.

Reproduce from the repository root after its normal locked setup and admin build:

```sh
mkdir -p .cache/reports/shared-auth-entry
GENUI_SHARED_ENTRY_FIXTURE="$PWD/.cache/reports/shared-auth-entry/native.json" cargo test --locked -p forma-app-platform --bin forma-app-platform shared_auth_entry_keeps_the_exact_provider_form_policy -- --nocapture
PLAYWRIGHT_MODULE=/absolute/path/to/test-installation/playwright/index.mjs node evidence/APP-154/shared-auth-entry-2026-11-01/browser.mjs .cache/reports/shared-auth-entry/native.json .cache/reports/shared-auth-entry/browser.json
just campaign shared-identity
just check fmt clippy clippy-dev test http-smoke msrv
```

The browser helper uses an existing repository testing installation supplied explicitly; it adds no product dependency. Do not run it if another process owns port 7697. The native fixture contains synthetic values and a fresh public CSP nonce; it is generated locally rather than checked in.

## Intermediate findings and limits

- The first native negative test incorrectly expected the renderer to reject an origin path; origin validation belongs to AuthConfig. Moving the check to that boundary exposed and fixed the real wildcard-host gap rather than weakening the test.
- An initial Playwright request-interception prototype could reproduce the CSP block but failed its allowed redirect with a browser network error. It used a synthetic authorize path and no credentials; it did not establish network confinement or provider behavior. The retained test replaces interception with real loopback HTTPS endpoints and passes both directions.
- The first expanded database campaign failed its missing-vault case because test setup omitted awaiting vault removal. The assertion caught that live token, the setup was corrected, and the full campaign passed. No product authorization expectation was relaxed.
- The local source fix and these results do not complete APP-139's intended guest signup, accept/connect/live/revoke journey or final recording. Reconcile the already-issued real invitation, deploy a separately approved qualified candidate, then continue the genuine browser flow. No hosted retry, infrastructure mutation, source push or deployment was performed by this repair task.
