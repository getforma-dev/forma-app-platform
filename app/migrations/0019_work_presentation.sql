-- APP-155–157: same-work adoption of an admitted presentation-only publication.
-- No new role, SECURITY DEFINER surface, record write or cross-tenant grant.
SET LOCAL ROLE genui_owner;
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK(version BETWEEN 1 AND 19);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK(version BETWEEN 1 AND 19);
GRANT UPDATE(publication_id,version) ON genui.saved_work TO genui_workspace;
GRANT UPDATE(publication_id) ON genui.record_collections,genui.work_bindings TO genui_workspace;
RESET ROLE;
