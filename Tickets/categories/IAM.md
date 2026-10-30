# IAM: Identity and access

Authenticated principals, tenant boundaries and service authority.

| Order | Ticket | Phase | Sprint | Category | Priority | Size | Status | Depends on |
|---|---|---|---|---|---|---|---|---|
| 6 | [APP-006: Revalidate fAuth contract and integration prerequisites](../4-done/APP-006.md) | [PH01](../phases/PH01.md) | S01 | IAM | P0 | M | Done | APP-001 |
| 8 | [APP-008: Establish tenant-bearing schema, database roles and deny-by-default authorization](../4-done/APP-008.md) | [PH01](../phases/PH01.md) | S02 | IAM | P0 | L | Done | APP-007 |
| 9 | [APP-073: Establish bounded invite and trial admission with owner-issued codes](../4-done/APP-073.md) | [PH01](../phases/PH01.md) | S02 | IAM | P0 | L | Done | APP-008 |
| 10 | [APP-009: Implement fAuth-backed session lifecycle](../3-in-review/APP-009.md) | [PH01](../phases/PH01.md) | S02 | IAM | P0 | L | In review | APP-008, APP-073 |
| 11 | [APP-074: Qualify invite-only signup and public beta entry locally](../0-backlog/APP-074.md) | [PH01](../phases/PH01.md) | S02 | IAM | P0 | L | Backlog | APP-006, APP-009, APP-073 |
| 63 | [APP-060: Authenticate service calls with constrained role identities](../0-backlog/APP-060.md) | [PH05](../phases/PH05.md) | S11 | IAM | P0 | L | Backlog | APP-059 |
| 76 | [APP-075: Finish the managed qualification entry, image and supervisor](../4-done/APP-075.md) | [PH01](../phases/PH01.md) | S02 | IAM | P0 | L | Done | APP-008 |
| 77 | [APP-077: Package the operator image and tools](../3-in-review/APP-077.md) | [PH01](../phases/PH01.md) | S02 | IAM | P0 | L | In review | APP-008 |
| 78 | [APP-078: Qualify the tenant foundation on managed RDS in one reserved window](../0-backlog/APP-078.md) | [PH01](../phases/PH01.md) | S02 | IAM | P0 | L | Backlog | APP-075, APP-077 |
| 80 | [APP-079: Hosted identity acceptance deferred from Phase 1](../9-blocked/APP-079.md) | [PH06](../phases/PH06.md) | S12 | IAM | P1 | L | Blocked | APP-076 |
| 82 | [APP-082: Prove route-level admission races and provider faults on the local public entry (deferred from APP-074)](../0-backlog/APP-082.md) | [PH07](../phases/PH07.md) | S13 | IAM | P1 | M | Backlog | APP-074 |
| 84 | [APP-084: Close the admission and provider review residuals (APP-073 reviews)](../0-backlog/APP-084.md) | [PH07](../phases/PH07.md) | S13 | IAM | P1 | M | Backlog | APP-073 |
