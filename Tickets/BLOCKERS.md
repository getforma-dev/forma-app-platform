# Cross-team blockers

GenUI owns the APP consumers. FormaStack owns the upstream FORMA work. Clear the consumer gate only after receiving and checking its evidence; assignment is not completion.

| FormaStack ticket | Gate state | Blocked GenUI ticket | Unblock condition |
|---|---|---|---|
| FORMA-246 | cleared | [APP-003](4-done/APP-003.md) | Native composition conformance evidence plus exact consumable ref and GenUI integration checks. |
| FORMA-232 | cleared | [APP-003](4-done/APP-003.md) | Shipped budget support OR measured no-override-needed consumer disposition; broader feature may remain open. |
| FORMA-247 | cleared | [APP-006](4-done/APP-006.md) | Cleared 2026-10-29 by founder disposition on reviewed, merged, CI-passing fAuth source (gatewasm PR18/19 at dfba787, PR20 at 1bf5d3b) consumed through APP-006. The deployed-auth, three-browser, canary-isolation and rollback proof it originally required is FORMA-365 / APP-079 and blocks any real organization (ADR-008). |
| FORMA-248 | cleared | [APP-058](9-blocked/APP-058.md) | Cleared 2026-10-29 by founder disposition on merged key-custody source (gatewasm PR17). The production custody/rotation/outage campaign moved to FORMA-365, which keeps blocking APP-058 before real-organization sign-in (ADR-008). |
| FORMA-365 | open | [APP-058](9-blocked/APP-058.md), [APP-079](9-blocked/APP-079.md) | Deferred hosted identity acceptance: the joint rehearsal with deployed negatives, KMS custody and A→B→C rotation campaign, unknown-kid live refetch, three-browser callback matrix and Canary-versus-Northstar resource check, with receipts. Required before any real organization signs in (ADR-008). |
| FORMA-366 | open | [APP-076](9-blocked/APP-076.md) | Qualified fAuth image carrying the merged mail provider slice (gatewasm PR20, 1bf5d3b) and the controlled-inbox trial receipt over the chosen transport (FormaStack prefers Amazon SES SMTP through fAuth's existing SMTP secret; GenUI creates the getforma.dev DKIM/SPF/DMARC records). FormaStack scoped the gate to image qualification and the inbox trial on 2026-10-29; the deployment itself is FORMA-367 (ADR-006, ADR-008). |
| FORMA-367 | open | [APP-076](9-blocked/APP-076.md) | Lean always-on fAuth deployment for the beta (FormaStack ticket FORMA-367, P1; Victor approved it with SES SMTP on 2026-10-29 and FormaStack deploys once its image is qualified): an image at or after 1bf5d3b, ECR, KMS keys and secrets including the SMTP secret, three-stack deploy, custom domain (GenUI creates the DNS records), Northstar/Canary tenants, the beta client with the exact callback https://beta.getforma.dev/auth/callback and per-organization return_entry_url, a smoke test, and private delivery of the issuer identity and ten_/usr_ ids. FormaStack's recomputed always-on cost is about $31–38/month (ADR-008). |

Read the [handoff](../docs/planning/CROSS-TEAM-HANDOFF.md). Source: `external-dependencies.json`; edit the register and ticket evidence, then regenerate.
