-- Operator-only initial migration on the isolated GenUI database.
-- Tenant tables and FORCE RLS policies belong to APP-008.
DO $$
DECLARE role_name text;
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'genui_owner') THEN
    CREATE ROLE genui_owner NOLOGIN NOSUPERUSER NOBYPASSRLS;
  END IF;
  ALTER ROLE genui_owner NOLOGIN NOSUPERUSER NOBYPASSRLS;
  FOREACH role_name IN ARRAY ARRAY['genui_workspace','genui_control','genui_data','genui_worker'] LOOP
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = role_name) THEN
      EXECUTE format('CREATE ROLE %I LOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOBYPASSRLS', role_name);
    END IF;
    EXECUTE format('ALTER ROLE %I LOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOBYPASSRLS', role_name);
  END LOOP;
END $$;
GRANT genui_owner TO genui_migration;
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
GRANT rds_iam TO genui_workspace, genui_control, genui_data, genui_worker;
-- No table privileges, owner membership or elevated permissions for app roles.
REVOKE ALL ON DATABASE genui FROM PUBLIC;
GRANT CONNECT ON DATABASE genui TO genui_workspace, genui_control, genui_data, genui_worker;
CREATE SCHEMA IF NOT EXISTS genui_meta AUTHORIZATION genui_owner;
CREATE TABLE IF NOT EXISTS genui_meta.schema_version (
  singleton boolean PRIMARY KEY CHECK (singleton), version integer NOT NULL CHECK (version = 1)
);
ALTER TABLE genui_meta.schema_version OWNER TO genui_owner;
INSERT INTO genui_meta.schema_version VALUES (true, 1) ON CONFLICT (singleton) DO NOTHING;
GRANT USAGE ON SCHEMA genui_meta TO genui_workspace, genui_control, genui_data, genui_worker;
GRANT SELECT ON genui_meta.schema_version TO genui_workspace, genui_control, genui_data, genui_worker;
