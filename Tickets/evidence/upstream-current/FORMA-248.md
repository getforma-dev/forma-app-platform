# FORMA-248: fAuth signing key held in KMS, current and next keys published in the key set

| Field | Value |
|---|---|
| Epic | E9 |
| Status | Backlog |
| Priority | P2 |
| Release | fAuth change; no Forma Stack change; before any real organization signs in through the platform |
| Repos | getforma-dev/gatewasm (fAuth) at `b750ca53` |
| Decision | Platform v2 design record, section 10 (2026-10-23): threat TH2, whoever holds fAuth's signing or management key mints a token for any `sub` and `tenant_id`; mitigation M5 is fAuth's and is deferred, accepted risk AR4 until it lands |
| Depends on | FORMA-247 (the demo tenants and client row) |
| Assignee | FormaStack team |
| Team | FormaStack |
| Blocks team | GenUI |
| Blocks | [APP-058](../../../FormaBuiltApps/forma-app-platform/Tickets/9-blocked/APP-058.md) — real-organization pilot signoff |
| Branch / PR | |

## Why

At the pin the EdDSA signing key is a secret fAuth's process can read and export, on one
deployment run by the same founder as the platform. The platform verifies tokens offline against
`/auth/jwks` by `kid`, so key rotation is already possible on the platform's side (an unknown
`kid` refetches the key set once a minute, then refuses). What is missing is custody: a key that
cannot leave the key management service, and a published next key so rotation never breaks a
live session.

## What changes

1. The signing key becomes a KMS asymmetric key (Ed25519 or, if KMS lacks it, ECDSA P-256 with
   the platform's verifier extended); fAuth signs through the KMS API and never holds the private
   key in memory or on disk.
2. `/auth/jwks` publishes the current and the next key, each with its `kid`; rotation promotes
   the next key and keeps the previous one published until the longest token lifetime has passed.
3. The management key (`mgmt_` per tenant) and the platform key are stored in the secrets manager
   with rotation on demand and an audit event per use.

## Acceptance tests

- [ ] Tokens issued after the change verify offline against `/auth/jwks` by `kid`; the private key
  is not readable from the fAuth host or its database.
- [ ] A rotation during a live session: the session's next refresh succeeds, the old `kid` keeps
  verifying until its tokens expire, then is removed from the key set.
- [ ] The platform's unknown-`kid` path refetches once a minute and refuses otherwise (platform
  SR2).

## Migration and rollback

- **Migration:** generate the KMS key, publish it as the next key, promote it, retire the file key.
- **Rollback:** promote a file key again; the platform's verifier does not change.

## Log
- 2026-10-23: Filed from the platform v2 design's threat model (TH2, M5, AR4).

- 2026-10-28 (GenUI cross-team handoff): Assigned to the FormaStack team to coordinate fAuth key custody; status remains Backlog and no implementation has started. This blocks GenUI APP-058 real-organization pilot signoff and remains a hard prerequisite before any real organization signs in. It may remain deferred for an explicitly synthetic demonstration. Deliver non-exportable signing-key custody, current/next key publication, rotation and management/platform-secret evidence from this ticket; GenUI verifies its unknown-kid/rotation integration. FORMA-247 remains the existing predecessor. Independent GenUI work continues in parallel. See the [cross-team journey](../journeys/genui-platform-2026-10-28/README.md).
