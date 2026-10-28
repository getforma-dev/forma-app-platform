# Cross-team blockers

GenUI owns the APP consumers. FormaStack owns the upstream FORMA work. Clear the consumer gate only after receiving and checking its evidence; assignment is not completion.

| FormaStack ticket | Gate state | Blocked GenUI ticket | Unblock condition |
|---|---|---|---|
| FORMA-246 | open | [APP-003](9-blocked/APP-003.md) | Native composition conformance evidence plus exact consumable ref and GenUI integration checks. |
| FORMA-232 | open | [APP-003](9-blocked/APP-003.md) | Shipped budget support OR measured no-override-needed consumer disposition; broader feature may remain open. |
| FORMA-247 | open | [APP-006](9-blocked/APP-006.md) | Deployed auth revision, client, PKCE, tenant claims, canary and rollback evidence. |
| FORMA-248 | open | [APP-058](9-blocked/APP-058.md) | Production key custody/rotation/outage evidence before real-organization sign-in. |

Read the [handoff](../docs/planning/CROSS-TEAM-HANDOFF.md). Source: `external-dependencies.json`; edit the register and ticket evidence, then regenerate.
