-- APP-141: a generated app is one saved work. Renaming that work must not
-- rename siblings which reuse its authoring container. Display-only fields
-- do not change publication, work version, bindings, records or share pins.
SET LOCAL ROLE genui_owner;
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK(version BETWEEN 1 AND 18);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK(version BETWEEN 1 AND 18);
ALTER TABLE genui.saved_work
 ADD COLUMN display_name text CHECK(display_name IS NULL OR (length(display_name) BETWEEN 1 AND 60 AND display_name=btrim(display_name))),
 ADD COLUMN name_version bigint NOT NULL DEFAULT 0 CHECK(name_version BETWEEN 0 AND 9007199254740991);
GRANT UPDATE(display_name,name_version) ON genui.saved_work TO genui_workspace;
RESET ROLE;
