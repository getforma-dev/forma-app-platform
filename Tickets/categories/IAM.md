# IAM: Identity and access

Authenticated principals, tenant boundaries and service authority.

| Order | Ticket | Phase | Sprint | Category | Priority | Size | Status | Depends on |
|---|---|---|---|---|---|---|---|---|
| 6 | [APP-006: Revalidate fAuth contract and integration prerequisites](../9-blocked/APP-006.md) | [PH01](../phases/PH01.md) | S01 | IAM | P0 | M | Blocked | APP-001 |
| 8 | [APP-008: Establish tenant-bearing schema, database roles and deny-by-default authorization](../2-in-progress/APP-008.md) | [PH01](../phases/PH01.md) | S02 | IAM | P0 | L | In progress | APP-007 |
| 9 | [APP-073: Establish bounded invite and trial admission with owner-issued codes](../0-backlog/APP-073.md) | [PH01](../phases/PH01.md) | S02 | IAM | P0 | L | Backlog | APP-008 |
| 10 | [APP-009: Implement fAuth-backed session lifecycle](../0-backlog/APP-009.md) | [PH01](../phases/PH01.md) | S02 | IAM | P0 | L | Backlog | APP-006, APP-008, APP-073 |
| 11 | [APP-074: Qualify invite-only signup and public beta access](../0-backlog/APP-074.md) | [PH01](../phases/PH01.md) | S02 | IAM | P0 | L | Backlog | APP-006, APP-009, APP-073 |
| 62 | [APP-060: Authenticate service calls with constrained role identities](../0-backlog/APP-060.md) | [PH05](../phases/PH05.md) | S11 | IAM | P0 | L | Backlog | APP-059 |
