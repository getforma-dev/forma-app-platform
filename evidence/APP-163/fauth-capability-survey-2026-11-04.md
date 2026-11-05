# fAuth email capability survey for APP-163 (2026-11-04)

APP-163 says to inspect the provider's real verification and email-delivery contract before implementation, and to assume no upstream capability. This record is that inspection.

It is a read-only source review of `getforma-dev/gatewasm` `main` at `bbe03f9`, done by a Claude subagent on 2026-11-04. No build ran and no service was contacted. Three findings were re-checked by hand: the `Claims` struct has no `email_verified` field, staging mail is SES sandbox, and there is no magic-link or passwordless code. Paths below are relative to `crates/server/src` in gatewasm unless stated.

## What fAuth has

- **One verification link, tied to an account.** It is issued at registration (`handlers/auth.rs:562-563,684-689`; hosted signup `handlers/hosted_code.rs:433-442`). The token is `vt_` plus 32 random bytes, and only its SHA-256 is stored (`jwt/signing.rs:76-86`). It lasts 24 hours (`handlers/auth.rs:1051`). GET only shows a confirmation page; POST consumes it under a row lock, bound to tenant, address and expiry, and creates no session (`handlers/auth.rs:1929-2014`). Tests cover wrong tenant, replay, expiry, a changed address and an 8-way race (`tests/cases/mail_verification.rs`).
- **Resend needs the account's own access token** and has a 60-second cooldown (`handlers/auth.rs:1053,2074-2088`).
- **Rate limits are per IP only**, burst 5 then one request every 12 seconds, on verify, register, reset and resend (`handlers/mod.rs:88-94,160-173`; `middleware/rate_limit.rs:24-37`). The comments at `rate_limit.rs:24` and `mod.rs:87` describe a different rate than tower_governor's `per_second(n)` gives.
- **`email_verified` is backend data, not a JWT claim.** It comes on code exchange and refresh (`handlers/token.rs:315-324`), `/auth/me`, the management users API and a `user.email_verified` webhook. The app already reads it this way (`app/src/auth/provider.rs:25-49` in this repository).
- **Mail through Resend or SMTP** (`email.rs:72-117`), with fixed fAuth templates for verification, password reset, tenant invite and team invite (`email.rs:225-462`). Each send reports suppressed, unconfigured or provider-accepted.

## What fAuth does not have

- A way to prove control of an address that has no account, or across tenants.
- A short-lived one-time code or sign-in link, and a per-proof attempt counter.
- A discovery mail template, and any per-recipient or global send quota.
- An email-change flow or event.
- An email-to-workspaces lookup. Users are unique per tenant (`db/pg_migrations/001_schema.sql:749`); cross-tenant search exists only for the platform owner.
- Delivery to arbitrary recipients on staging: SES SMTP in sandbox mode, accepted for the MVP ([mail trial record](../APP-006/upstream-auth-source/fauth-mail-trial-2026-10-29.md)).

Password reset (`handlers/password_reset.rs`) is not a model for the new proof. Its token is stored in plaintext, it logs the address, and it sends only for existing accounts during the request.

## Outcome

Victor decided on 2026-11-04 that the mailbox proof is fAuth work, filed as FormaStack FORMA-368. The email-to-workspace mapping stays in this app ([ADR-016 amendment](../../docs/architecture/ADR-016-WORKSPACE-SIGNIN.md#amendment-2026-11-04-the-mailbox-proof-is-fauth-work)). APP-163 is Blocked on FORMA-368.
