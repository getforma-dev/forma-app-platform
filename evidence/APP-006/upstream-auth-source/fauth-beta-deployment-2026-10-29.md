# fAuth beta deployment received (FORMA-367), 2026-10-29

FormaStack reported FORMA-367 Done: fAuth runs at `https://auth-staging.getforma.dev` from gatewasm `main` `1bf5d3b` (image digest `sha256:d1365238…`) on one EC2 host with Caddy, after AWS refused new App Runner services for the account. GenUI created the DNS record (Porkbun A `44.220.97.130`, record id 599164375).

GenUI's own public checks, 2026-10-29 ~21:05 UTC:

| Check | Result |
| --- | --- |
| `GET /health` | `status ok`, `db_status ok`, `signer_current true`, `source_revision 1bf5d3b2fb515c8ed82b0c5a044f7846915e28d0` |
| `GET /auth/jwks` | two OKP/Ed25519/EdDSA keys, kids `g0jSyT5oFq8` (current) and `hKBBWz5abqI` (next) |
| `GET /ssf/jwks` | one key `Po1CZYz1-d8`, separate from the access keys |
| TLS | Let's Encrypt `YE2`, subject `auth-staging.getforma.dev`, valid until 2027-01-27 |

FormaStack-reported, not repeated by GenUI: provisioning run twice with identical results (Northstar three users, Canary one user; clients `northstar-platform` and `canary-platform` with the exact callback `https://beta.getforma.dev/auth/callback`); a deployed PAR → authorize → PKCE exchange whose token verifies offline against the public JWKS (iss `gatewasm`, aud `gatewasm-app`, ≤ 900 s); wrong verifier and replay 400; logout 204; an invited signup in a throwaway tenant. Private identifiers are held outside this repository in the FormaStack journey folder (`private/fauth-beta-identifiers.json`, mode 0600); no identifier, credential or password is copied here.

Still open: mail is wired to Amazon SES in the sandbox (verified recipients only); SES production access and the controlled-inbox trial are FORMA-366. Per-organization `return_entry_url` registrations wait for APP-009's sign-in handles. Deferred hosted acceptance remains FORMA-365 / APP-079.
