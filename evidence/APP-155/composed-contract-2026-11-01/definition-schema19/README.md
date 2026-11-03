# Schema19 definition qualifier alignment

Both old-candidate tenant CI runs failed the historical assertion that Workspace cannot update a saved-work version. Migration19 intentionally grants `UPDATE(publication_id, version)` for fenced presentation adoption. The production behavior matched that reviewed migration; the old test expectation was stale.

The repaired fixture still denies Workspace context changes and explicitly denies Control/Data version writes. A separate probe changes exactly one local work through the new columns, observes the incremented version, proves foreign-tenant rows remain invisible, rolls back, and verifies the original version and complete footprint. It changes no production code, migration, expected record value or authorization predicate. Creator RLS is an INSERT policy; current-owner adoption checks remain separately qualified by the composer campaign.

`just campaign definition` passed all 19 checks in 22.32 seconds against a disposable real PostgreSQL fixture. The rest of the campaign retains cross-tenant binding refusal, publication races, revocation, re-admission, restart and audit-chain checks. The peer independently reviewed this bounded fixture change. [Observed failure/repair and hashes](repair.json) and the [passing report](definition.json) retain the evidence without publishing raw private logs.
