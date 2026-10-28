-- Operator-only initial migration on the isolated GenUI database.
-- Tenant tables and FORCE RLS policies belong to APP-008.
DO $$
DECLARE role_name text;
DECLARE role_state record;
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'genui_owner') THEN
    CREATE ROLE genui_owner NOLOGIN INHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
  END IF;
  FOREACH role_name IN ARRAY ARRAY['genui_workspace','genui_control','genui_data','genui_worker'] LOOP
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = role_name) THEN
      EXECUTE format('CREATE ROLE %I LOGIN NOINHERIT NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS', role_name);
    END IF;
  END LOOP;
  -- RDS administrators are not PostgreSQL superusers: even ALTER ROLE
  -- NOSUPERUSER/NOBYPASSRLS requires privileges they do not possess. Create
  -- constrained roles, then refuse drift instead of attempting a demotion.
  FOREACH role_name IN ARRAY ARRAY['genui_owner','genui_workspace','genui_control','genui_data','genui_worker'] LOOP
    SELECT * INTO STRICT role_state FROM pg_roles WHERE rolname = role_name;
    IF role_state.rolsuper OR role_state.rolcreatedb OR role_state.rolcreaterole
       OR role_state.rolreplication OR role_state.rolbypassrls
       OR role_state.rolcanlogin <> (role_name <> 'genui_owner')
       OR role_state.rolinherit <> (role_name = 'genui_owner') THEN
      RAISE EXCEPTION 'Unexpected attributes for foundation role %', role_name USING ERRCODE = '42501';
    END IF;
    IF EXISTS (
      SELECT 1 FROM pg_roles r
      WHERE r.rolname <> role_name
        AND (role_name = 'genui_owner' OR r.rolname <> 'rds_iam')
        AND pg_has_role(role_name, r.oid, 'MEMBER')
    ) OR EXISTS (
      SELECT 1 FROM pg_auth_members m
      WHERE m.member = role_state.oid AND (m.admin_option OR m.inherit_option)
    ) THEN
      RAISE EXCEPTION 'Unexpected memberships for foundation role %', role_name USING ERRCODE = '42501';
    END IF;
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
