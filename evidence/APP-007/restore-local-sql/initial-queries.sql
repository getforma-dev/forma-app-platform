BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SET LOCAL search_path=pg_catalog; SET LOCAL statement_timeout=5000; SET LOCAL lock_timeout=2000;
SELECT json_build_object('identity', json_build_object('session_user', session_user, 'current_user', current_user, 'database', current_database(), 'read_only', current_setting('transaction_read_only'), 'server_version_num', current_setting('server_version_num')::integer));
SELECT json_build_object('name', 'schema_version', 'rows', COALESCE(json_agg(row_to_json(q)), '[]'::json)) FROM (SELECT singleton, version FROM genui_meta.schema_version) q;
SELECT json_build_object('name', 'roles', 'rows', COALESCE(json_agg(row_to_json(q)), '[]'::json)) FROM (SELECT rolname AS name, rolsuper, rolcreatedb, rolcreaterole, rolreplication,
      rolbypassrls, rolcanlogin, rolinherit, rolconnlimit FROM pg_roles
      WHERE rolname = ANY(ARRAY['genui_owner','genui_workspace','genui_control','genui_data','genui_worker']::text[])) q;
SELECT json_build_object('name', 'memberships', 'rows', COALESCE(json_agg(row_to_json(q)), '[]'::json)) FROM (SELECT parent.rolname AS role, member.rolname AS member,
      grantor.rolname AS grantor, m.admin_option, m.inherit_option, m.set_option
      FROM pg_auth_members m JOIN pg_roles parent ON parent.oid=m.roleid
      JOIN pg_roles member ON member.oid=m.member JOIN pg_roles grantor ON grantor.oid=m.grantor
      WHERE parent.rolname = ANY(ARRAY['genui_owner','genui_workspace','genui_control','genui_data','genui_worker']::text[]) OR member.rolname = ANY(ARRAY['genui_owner','genui_workspace','genui_control','genui_data','genui_worker']::text[])) q;
SELECT json_build_object('name', 'membership_closure', 'rows', COALESCE(json_agg(row_to_json(q)), '[]'::json)) FROM (SELECT member.rolname AS member, parent.rolname AS role
      FROM pg_roles member CROSS JOIN pg_roles parent
      WHERE member.rolname = ANY(ARRAY['genui_owner','genui_workspace','genui_control','genui_data','genui_worker']::text[]) AND parent.oid<>member.oid
      AND pg_has_role(member.oid,parent.oid,'MEMBER')) q;
SELECT json_build_object('name', 'database', 'rows', COALESCE(json_agg(row_to_json(q)), '[]'::json)) FROM (SELECT datname AS name, pg_get_userbyid(datdba) AS owner
      FROM pg_database WHERE datname=current_database()) q;
SELECT json_build_object('name', 'database_grants', 'rows', COALESCE(json_agg(row_to_json(q)), '[]'::json)) FROM (SELECT pg_get_userbyid(a.grantor) AS grantor,
      CASE WHEN a.grantee=0 THEN 'PUBLIC' ELSE pg_get_userbyid(a.grantee) END AS grantee,
      a.privilege_type, a.is_grantable FROM pg_database d,
      LATERAL aclexplode(COALESCE(d.datacl,acldefault('d',d.datdba))) a
      WHERE d.datname=current_database()) q;
SELECT json_build_object('name', 'schemas', 'rows', COALESCE(json_agg(row_to_json(q)), '[]'::json)) FROM (SELECT nspname AS name, pg_get_userbyid(nspowner) AS owner
      FROM pg_namespace WHERE nspname IN ('public','genui_meta')) q;
SELECT json_build_object('name', 'schema_grants', 'rows', COALESCE(json_agg(row_to_json(q)), '[]'::json)) FROM (SELECT n.nspname AS schema, pg_get_userbyid(a.grantor) AS grantor,
      CASE WHEN a.grantee=0 THEN 'PUBLIC' ELSE pg_get_userbyid(a.grantee) END AS grantee,
      a.privilege_type, a.is_grantable FROM pg_namespace n,
      LATERAL aclexplode(COALESCE(n.nspacl,acldefault('n',n.nspowner))) a
      WHERE n.nspname IN ('public','genui_meta')) q;
SELECT json_build_object('name', 'tables', 'rows', COALESCE(json_agg(row_to_json(q)), '[]'::json)) FROM (SELECT n.nspname AS schema, c.relname AS name, c.relkind AS kind,
      pg_get_userbyid(c.relowner) AS owner, c.relrowsecurity, c.relforcerowsecurity
      FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
      WHERE n.nspname='genui_meta' AND c.relkind IN ('r','p','v','m','f','S')) q;
SELECT json_build_object('name', 'table_grants', 'rows', COALESCE(json_agg(row_to_json(q)), '[]'::json)) FROM (SELECT c.relname AS table, pg_get_userbyid(a.grantor) AS grantor,
      CASE WHEN a.grantee=0 THEN 'PUBLIC' ELSE pg_get_userbyid(a.grantee) END AS grantee,
      a.privilege_type, a.is_grantable FROM pg_class c
      JOIN pg_namespace n ON n.oid=c.relnamespace,
      LATERAL aclexplode(COALESCE(c.relacl,acldefault('r',c.relowner))) a
      WHERE n.nspname='genui_meta' AND c.relname='schema_version') q;
SELECT json_build_object('name', 'columns', 'rows', COALESCE(json_agg(row_to_json(q)), '[]'::json)) FROM (SELECT a.attname AS name, a.attnum AS position,
      pg_get_userbyid(c.relowner) AS table_owner, format_type(a.atttypid,a.atttypmod) AS type,
      a.attnotnull, a.attidentity, a.attgenerated, pg_get_expr(d.adbin,d.adrelid) AS default_expression
      FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid
      JOIN pg_namespace n ON n.oid=c.relnamespace
      LEFT JOIN pg_attrdef d ON d.adrelid=c.oid AND d.adnum=a.attnum
      WHERE n.nspname='genui_meta' AND c.relname='schema_version' AND a.attnum>0 AND NOT a.attisdropped) q;
SELECT json_build_object('name', 'column_grants', 'rows', COALESCE(json_agg(row_to_json(q)), '[]'::json)) FROM (SELECT a.attname AS column, pg_get_userbyid(g.grantor) AS grantor,
      CASE WHEN g.grantee=0 THEN 'PUBLIC' ELSE pg_get_userbyid(g.grantee) END AS grantee,
      g.privilege_type, g.is_grantable FROM pg_attribute a
      JOIN pg_class c ON c.oid=a.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace,
      LATERAL aclexplode(COALESCE(a.attacl,'{}'::aclitem[])) g
      WHERE n.nspname='genui_meta' AND c.relname='schema_version' AND a.attnum>0 AND NOT a.attisdropped) q;
SELECT json_build_object('name', 'constraints', 'rows', COALESCE(json_agg(row_to_json(q)), '[]'::json)) FROM (SELECT k.conname AS name, k.contype AS type,
      pg_get_constraintdef(k.oid,true) AS definition, k.convalidated AS validated
      FROM pg_constraint k JOIN pg_class c ON c.oid=k.conrelid
      JOIN pg_namespace n ON n.oid=c.relnamespace
      WHERE n.nspname='genui_meta' AND c.relname='schema_version') q;
SELECT json_build_object('name', 'effective_app_grants', 'rows', COALESCE(json_agg(row_to_json(q)), '[]'::json)) FROM (SELECT r.rolname AS role,
      has_database_privilege(r.oid,current_database(),'CONNECT') AS connect,
      has_database_privilege(r.oid,current_database(),'CREATE,TEMP') AS database_write,
      has_schema_privilege(r.oid,'genui_meta','USAGE') AS metadata_usage,
      has_schema_privilege(r.oid,'genui_meta','CREATE') AS metadata_create,
      has_schema_privilege(r.oid,'public','CREATE') AS public_create,
      has_table_privilege(r.oid,'genui_meta.schema_version','SELECT') AS metadata_select,
      has_table_privilege(r.oid,'genui_meta.schema_version','INSERT,UPDATE,DELETE,TRUNCATE,REFERENCES,TRIGGER') AS metadata_write
      FROM pg_roles r WHERE r.rolname = ANY(ARRAY['genui_workspace','genui_control','genui_data','genui_worker']::text[])) q;
ROLLBACK;
