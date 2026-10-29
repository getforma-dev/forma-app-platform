# Cross-team blockers

GenUI owns the APP consumers. FormaStack owns the upstream FORMA work. Clear the consumer gate only after receiving and checking its evidence; assignment is not completion.

| FormaStack ticket | Gate state | Blocked GenUI ticket | Unblock condition |
|---|---|---|---|
| FORMA-246 | cleared | [APP-003](4-done/APP-003.md) | Native composition conformance evidence plus exact consumable ref and GenUI integration checks. |
| FORMA-232 | cleared | [APP-003](4-done/APP-003.md) | Shipped budget support OR measured no-override-needed consumer disposition; broader feature may remain open. |
| FORMA-247 | cleared | [APP-006](2-in-progress/APP-006.md) | Cleared 2026-10-29 by founder disposition on reviewed, merged, CI-passing fAuth source (gatewasm PR18/19 at dfba787, PR20 at 1bf5d3b) consumed through APP-006. The deployed-auth, three-browser, canary-isolation and rollback proof it originally required is FORMA-365 / APP-079 and blocks any real organization (ADR-008). |
| FORMA-248 | cleared | [APP-058](9-blocked/APP-058.md) | Cleared 2026-10-29 by founder disposition on merged key-custody source (gatewasm PR17). The production custody/rotation/outage campaign moved to FORMA-365, which keeps blocking APP-058 before real-organization sign-in (ADR-008). |
| FORMA-365 | open | [APP-058](9-blocked/APP-058.md), [APP-079](9-blocked/APP-079.md) | Deferred hosted identity acceptance: the joint rehearsal with deployed negatives, KMS custody and A→B→C rotation campaign, unknown-kid live refetch, three-browser callback matrix and Canary-versus-Northstar resource check, with receipts. Required before any real organization signs in (ADR-008). |
| FORMA-366 | open | [APP-076](9-blocked/APP-076.md) | Qualified fAuth image carrying the merged mail provider slice (gatewasm PR20, 1bf5d3b) deployed for the beta with a mail transport, the controlled-inbox trial receipt, and the beta client registration with return_entry_url https://beta.getforma.dev/signin/{handle} (ADR-006, ADR-008). |

Read the [handoff](../docs/planning/CROSS-TEAM-HANDOFF.md). Source: `external-dependencies.json`; edit the register and ticket evidence, then regenerate.
