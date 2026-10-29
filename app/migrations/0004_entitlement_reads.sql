-- ADR-007 D7.3 (APP-073 Task 5): the entitlement reads that the shared member
-- precheck and its fence re-read need. D7.3's SELECT column assigns
-- entitlements to genui_workspace, genui_control and genui_data; reads are
-- granted by column, exactly the columns the precheck, the fence and the
-- session's entitlement reference read. No write, no new relation, no policy:
-- entitlements keeps 0003's ENABLE/FORCE RLS and tenant_scope. Executed only
-- by the checksummed, transactional migration runner (D7.12: numbered in
-- landing order; ADR-007 planned 0004 for APP-010, which renumbers).
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 4);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 4);

GRANT SELECT(organization_id,principal_id,entitlement_id,status,expires_at) ON genui.entitlements
 TO genui_workspace,genui_control,genui_data;
RESET ROLE;
