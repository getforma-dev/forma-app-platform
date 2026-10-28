# FORMA-247: fAuth as the Northstar demo's login: redeploy at the pin, demo tenant provisioning, a client row for the platform, optional passkey sign-in

| Field | Value |
|---|---|
| Epic | E9 |
| Status | Backlog |
| Priority | P2 |
| Release | fAuth deployment and small fAuth changes; no Forma Stack change |
| Repos | getforma-dev/gatewasm (fAuth) at `b750ca53`; the demo environment |
| Decision | Founder 2026-10-22 (platform v2 intake K4): fAuth is the first identity adapter if its current build passes a demo-viability check, judged with the head on hurt-or-wow. The check passed (`design-doc-factory/tasks/forma-generative-app-platform-v2/review/fauth-viability.md`); ruled in on 2026-10-23 (delegated to the lead): a quiet real moment, no passkey button, so item 4 is not taken. Filed so the guards are not lost |
| Depends on | none |
| Assignee | Unassigned |
| Branch / PR | |

## Why

The lead's viability check on 2026-10-22 built and ran fAuth at the pin: its smoke suite passed
44/44; tenant login, EdDSA tokens verified offline against `/auth/jwks`, invite-only tenants,
management-API users with roles, branding through `/manage/config`, tenant isolation and MFA all
work. Four things stand between that and a clean demo moment, none of them a Forma Stack change.

## What changes

1. **Redeploy `auth.getforma.dev` at `b750ca53`.** The live service runs an older build: it still
   reads "GateWASM" and shows the previous design; the pin reads "Secured by fAuth". Confirm the
   deployed commit after the deploy (a version in `/health` would help; optional).
2. **A demo provisioning script** (checked in beside the demo, not in fAuth): create the
   `northstar` tenant and a canary tenant with the platform key; set `branding_app_name`,
   `branding_primary_color`, `branding_logo_url`, `login_redirect_url`, `signup_mode: invite_only`
   through `/manage/config`; create Maya (admin), Sarah (admin), Jordan (user) through
   `/manage/users`; raise `LOGIN_RATE_LIMIT_BURST` for rehearsal; keep `mfa_policy: off`; hide
   provider buttons unless Google or GitHub credentials are configured for the tenant.
3. **One `oauth_clients` row for the platform.** The platform v2 design (its section 10, TH3 and
   SR2, 2026-10-23) signs in as a registered OAuth client of fAuth with the authorization-code flow
   and PKCE S256, the code exchanged on the back channel, so no token passes through browser script
   and same-site hosting is no longer required (the refresh-cookie bridge is the fallback). fAuth
   has no endpoint to register a client, so the row is inserted by hand per environment (the demo's
   provisioning step, labeled), or `POST /manage/oauth-clients` is added (optional).
4. **Optional, the wow:** a "Sign in with a passkey" button on the hosted login page over the
   existing `/auth/webauthn/login/begin` and `/login/complete` routes; registration already exists
   in the console's Passkeys view. Small; only if the head wants the Touch ID moment.

The platform's side is in the design: verify the token against `/auth/jwks` by `kid`, take the
organization from `tenant_id` and the person from `sub` (never from a header), seed membership
from `roles`, keep the platform's own session and authorization.

## Acceptance tests

- [ ] `auth.getforma.dev/t/northstar/login` shows the Northstar branding and "Secured by fAuth" at
  the deployed pin; `/health` is ok.
- [ ] The provisioning script runs twice without error (idempotent) and the three users log in.
- [ ] From the platform's origin, a rehearsal login completes in Chrome, Safari and Firefox through
  the code flow against the platform's client row, and the platform rejects a token from the
  canary tenant for a Northstar resource.
- [ ] If item 4 is taken: a passkey registered in the console signs Maya in on the hosted page in
  Chrome and Safari; the token carries `auth_method: webauthn`.

## Migration and rollback

- **Migration:** none for fAuth's data; the demo tenants are additive.
- **Rollback:** the platform's own minimal membership login behind the same adapter boundary
  (intake K4's fallback); the demo loses the branded sign-in moment, nothing else.

## Log

- 2026-10-22: Filed from the platform v2 fAuth viability check. Evidence in that task's
  `review/fauth-viability/`.
- 2026-10-23: Item 3 changed from same-site hosting to a registered client row: the platform v2 design's threat model (TH3, SR2) makes sign-in the authorization-code flow with PKCE on the back channel. The signing key's custody in KMS is FORMA-248.
- 2026-10-23: K4 ruled in (quiet; no passkey button). Items 1 to 3 stand; item 4 is not taken.
