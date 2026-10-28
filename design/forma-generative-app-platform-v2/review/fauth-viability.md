# fAuth for the demo: viability check and the hurt-or-wow question

Checked 2026-10-22 by the lead on the head's condition (intake K4): fAuth's current build must be
viable for the demo, judged together on whether it hurts or wows. Evidence files are in
`fauth-viability/`. Nothing in the head's working trees was touched; the clone and build live
under `~/dev/Forma/work/fauth-demo-check/`.

## What was checked, and the result

| Check | Method | Result |
|---|---|---|
| Source state | `getforma-dev/gatewasm` cloned fresh; `HEAD` = `b750ca53`, the pin design A recorded; CI on `main` green (eight runs, about 6 min each, last 2026-08-08) | at the pin, no later commits |
| Build at the pin | `cargo build -p gatewasm-server` (forma-ir and forma-server 0.2.0, Forma 2.x); `npm run build:ssr` for the admin UI (core 2.0.0, compiler 0.3.5, build 0.2.0) | both build; the 2.x compiler prints warning-only SSR degradations on console pages, none on the login page |
| Runs locally | Docker PostgreSQL 16 (`npm run db`), Mailpit, the server with an ephemeral Ed25519 key; `npm run db:seed` | up in under a second; migrations apply; health `ok` |
| Independent test signal | the repository's own Playwright smoke suite at the pin (`npx playwright test --grep @smoke`), after installing its browser | **44 passed, 0 failed, 30.2 s** (`fauth-viability/smoke-suite-at-pin.log`) |
| Live service | `https://auth.getforma.dev` | up: `/health` ok, `/auth/jwks` serves one EdDSA key, the login page answers in 0.16 s; **but it runs an older build** than the pin: it still says "GateWASM" and shows the earlier light design, while the pin says "Secured by fAuth" with the new design |
| Tenant login | `POST /auth/login` with `X-Tenant-Slug` | 200; access token (EdDSA, `kid`) with `sub`, `email`, `name`, `tenant_id`, `roles`, `auth_method`, `mfa_verified`; refresh cookie `gw_rt_<slug>` (HttpOnly; `__Host-`, `Secure`, `SameSite=None`, `Path=/` in production) |
| Offline verification | the token's signature checked with OpenSSL against the raw Ed25519 key from `/auth/jwks`; a tampered payload | verifies; the tampered token fails. The platform can verify fAuth tokens with no call to fAuth |
| Organization and users for the demo | `POST /platform/tenants` (platform key) created `northstar` and returned its management key; `PATCH /manage/config` set the app name, the primary color and the redirect; `signup_mode: invite_only`; `POST /manage/users` created Maya (admin), Sarah (admin) and Jordan (user) | all 201/200; the branded page reads "Northstar Security — Sign In", shows the invite-only notice, uses the color (`fauth-viability/local-pin-northstar-branded-login.png`) |
| Invite-only | open `POST /auth/register` on `northstar` | refused, `invite_required` |
| Isolation | Maya's password at the `acme` tenant; Maya's Northstar token with `X-Tenant-Slug: acme` on `/auth/me` and `/admin/stats` | the login is refused (401); the token is accepted but the data returned is Northstar's own (3 users; acme has 1): authenticated scope comes from the token's `tenant_id`, the header is ignored. No cross-tenant data was reachable |
| MFA | `POST /auth/mfa/setup`, a TOTP computed from the secret, `POST /auth/mfa/verify` | enrols; the verified token carries `mfa_verified: true`; with the tenant's `mfa_policy: off` a later password login does not demand it |
| Passkeys | routes `/auth/webauthn/register/*`, `/credentials`, `/login/begin` and `/login/complete` exist; the hosted login page and the tenant login app make no call to them; the onboarding wizard labels Passkey "Coming soon" | server-ready, no sign-in button |
| Session handoff to another app | read `admin/src/tenant/login/store.ts` and `handlers/token.rs` | after a password login the hosted page sets the refresh cookie on fAuth's origin and redirects to the tenant's `login_redirect_url`; the app then obtains an access token with `POST /auth/token {grant_type: refresh_token}` sending that cookie (the bridge's silent refresh). OAuth providers additionally use a PKCE code flow against a registered `oauth_clients` row; the hosted page is itself that client (`<slug>-hosted`). There is no API to register another client; a demo environment adds one row |

Not checked: the live deployment's exact commit (no version endpoint); Google and GitHub sign-in
(no provider credentials configured; with `allowed_providers: ["email"]` the buttons do not render);
the invite-link flow end to end; load beyond one user.

## Does it hurt or wow?

**It can wow, for about ten seconds, and it is viable today.** The moment is act 1: the audience
sees a Northstar-branded sign-in page, invite-only, "Secured by fAuth", and lands in the company
home. It says, without a word, that the company runs on a managed identity service Forma also
built, that organizations are real boundaries, and that membership is administered, not
hard-coded. Everything the demo needs from it works at the pin: organizations, invite-only
membership, roles, branded login, a token the platform verifies offline, MFA available but not
in the way.

**Where it would hurt, and the guard for each:**

| Risk | Why it hurts | Guard |
|---|---|---|
| The live service is behind the pin (old name, old design) | the audience would see "GateWASM" | redeploy `auth.getforma.dev` at `b750ca53` before rehearsal (the pin is what passed here) |
| Two visual languages | fAuth's page is serif-and-gold; the platform's shell is not | brand the `northstar` tenant with the platform's palette and the Northstar logo through `/manage/config` (shown working); the pin's page keeps "Secured by fAuth" small |
| Cross-site cookies | the session handoff rides fAuth's cookie; Safari and strict browsers refuse third-party cookies | serve the platform on the same site, `app.getforma.dev` beside `auth.getforma.dev` (same registrable domain, so the `SameSite=None` cookie is first-party), and rehearse in the browser the demo uses; the fallback is a server-side PKCE exchange with one `oauth_clients` row per environment |
| A round trip in the demo | leaving the platform for the login page and back | it is one redirect each way; the hosted page answers in under a second; keep MFA policy off and provider buttons hidden unless configured |
| Rate limits in rehearsal | repeated logins trip the login limiter | raise `LOGIN_RATE_LIMIT_BURST` for the demo environment, as the test suite does |
| No passkey button | the "Touch ID" moment the pitch could use is not wired | optional: a sign-in button over the existing `/auth/webauthn/login` routes, a small fAuth change; not required for viability |
| It is a 2.x Forma application | the compiler prints SSR degradations on console pages | irrelevant to the login page; fAuth's own migration (FORMA-150, FORMA-152) is not a demo prerequisite |

**Recommendation.** Use it. The adapter boundary in the design stays as K4 states: one verified-
principal check in the platform, the provider behind it. Concretely, the platform verifies the
EdDSA token against `/auth/jwks` with key rotation by `kid`, takes the organization from
`tenant_id` and the person from `sub`, never from a header, seeds membership roles from `roles`,
and keeps its own session and its own authorization; fAuth's token lifetime (15 minutes by
default) is not the platform's revocation path. The items above are filed as FORMA-247 on the
work board so they are not lost; the head rules on hurt-or-wow at the phase-1 summary.

## Reproduction, in short

```
gh repo clone getforma-dev/gatewasm ~/dev/Forma/work/fauth-demo-check/gatewasm
cd ~/dev/Forma/work/fauth-demo-check/gatewasm && npm install && (cd admin && npm install)
npm run db && npm run build:ssr && cargo build -p gatewasm-server
./target/debug/gatewasm-server &          # with a local .env: DATABASE_URL on port 5434, PLATFORM_ADMIN_KEY
npm run db:seed
curl -X POST localhost:3001/platform/tenants -H 'Authorization: Bearer plat_local_dev_key' \
  -d '{"name":"Northstar","slug":"northstar","owner_email":"maya@northstar.example","allowed_providers":["email"]}'
curl -X PATCH localhost:3001/manage/config -H 'Authorization: Bearer mgmt_…' \
  -d '{"branding_app_name":"Northstar Security","branding_primary_color":"#0f766e","signup_mode":"invite_only"}'
curl -X POST localhost:3001/manage/users -H 'Authorization: Bearer mgmt_…' \
  -d '{"email":"maya@northstar.example","password":"…","name":"Maya Chen","role":"admin"}'
npm run db:test && (cd admin && npx playwright install chromium && npx playwright test --grep @smoke)
```

The `northstar` tenant was deleted afterwards through the platform API (fAuth soft-deletes: the row
remains with status `deleted`), the containers were stopped and Docker Desktop quit, since it was
not running before the check.

## Ruling (2026-10-22)

The head delegated the call to the lead. Ruled: **in, as a quiet real moment.** The hosted page on
camera for ten seconds, Northstar-branded, "Secured by fAuth" small, no narration, no passkey
button. It is not the wow; the wow is what rests on it: the canary denied before data and
revocation within 5 s, both proven on a real tenant boundary the platform did not build for the
demo, with no password ever reaching the platform. The hurts are all guarded (the redeploy at the
pin, the branded tenant, the platform's client row, the rehearsal rate limit; FORMA-247), and the
same-site guard fell away when the platform became a registered client (its section 10, TH3). The
minimal login stays the fallback behind the same adapter.
