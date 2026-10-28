"""Read-only APP-007 snapshot comparison; execution-secret password stays in task."""
import hashlib
import json
import os
import re
import signal


SCOPE = "APP-007 foundation snapshot restore manifest"
ROLES = ("genui_owner", "genui_workspace", "genui_control", "genui_data", "genui_worker")
QUERIES = {
    "schema_version": "SELECT singleton, version FROM genui_meta.schema_version",
    "roles": """SELECT rolname AS name, rolsuper, rolcreatedb, rolcreaterole, rolreplication,
      rolbypassrls, rolcanlogin, rolinherit, rolconnlimit FROM pg_roles
      WHERE rolname = ANY(%s)""",
    "memberships": """SELECT parent.rolname AS role, member.rolname AS member,
      grantor.rolname AS grantor, m.admin_option, m.inherit_option, m.set_option
      FROM pg_auth_members m JOIN pg_roles parent ON parent.oid=m.roleid
      JOIN pg_roles member ON member.oid=m.member JOIN pg_roles grantor ON grantor.oid=m.grantor
      WHERE parent.rolname = ANY(%s) OR member.rolname = ANY(%s)""",
    "membership_closure": """SELECT member.rolname AS member, parent.rolname AS role
      FROM pg_roles member CROSS JOIN pg_roles parent
      WHERE member.rolname = ANY(%s) AND parent.oid<>member.oid
      AND pg_has_role(member.oid,parent.oid,'MEMBER')""",
    "database": """SELECT datname AS name, pg_get_userbyid(datdba) AS owner
      FROM pg_database WHERE datname=current_database()""",
    "database_grants": """SELECT pg_get_userbyid(a.grantor) AS grantor,
      CASE WHEN a.grantee=0 THEN 'PUBLIC' ELSE pg_get_userbyid(a.grantee) END AS grantee,
      a.privilege_type, a.is_grantable FROM pg_database d,
      LATERAL aclexplode(COALESCE(d.datacl,acldefault('d',d.datdba))) a
      WHERE d.datname=current_database()""",
    "schemas": """SELECT nspname AS name, pg_get_userbyid(nspowner) AS owner
      FROM pg_namespace WHERE nspname IN ('public','genui_meta')""",
    "schema_grants": """SELECT n.nspname AS schema, pg_get_userbyid(a.grantor) AS grantor,
      CASE WHEN a.grantee=0 THEN 'PUBLIC' ELSE pg_get_userbyid(a.grantee) END AS grantee,
      a.privilege_type, a.is_grantable FROM pg_namespace n,
      LATERAL aclexplode(COALESCE(n.nspacl,acldefault('n',n.nspowner))) a
      WHERE n.nspname IN ('public','genui_meta')""",
    "tables": """SELECT n.nspname AS schema, c.relname AS name, c.relkind AS kind,
      pg_get_userbyid(c.relowner) AS owner, c.relrowsecurity, c.relforcerowsecurity
      FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
      WHERE n.nspname='genui_meta' AND c.relkind IN ('r','p','v','m','f','S')""",
    "table_grants": """SELECT c.relname AS table, pg_get_userbyid(a.grantor) AS grantor,
      CASE WHEN a.grantee=0 THEN 'PUBLIC' ELSE pg_get_userbyid(a.grantee) END AS grantee,
      a.privilege_type, a.is_grantable FROM pg_class c
      JOIN pg_namespace n ON n.oid=c.relnamespace,
      LATERAL aclexplode(COALESCE(c.relacl,acldefault('r',c.relowner))) a
      WHERE n.nspname='genui_meta' AND c.relname='schema_version'""",
    "columns": """SELECT a.attname AS name, a.attnum AS position,
      pg_get_userbyid(c.relowner) AS table_owner, format_type(a.atttypid,a.atttypmod) AS type,
      a.attnotnull, a.attidentity, a.attgenerated, pg_get_expr(d.adbin,d.adrelid) AS default_expression
      FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid
      JOIN pg_namespace n ON n.oid=c.relnamespace
      LEFT JOIN pg_attrdef d ON d.adrelid=c.oid AND d.adnum=a.attnum
      WHERE n.nspname='genui_meta' AND c.relname='schema_version' AND a.attnum>0 AND NOT a.attisdropped""",
    "column_grants": """SELECT a.attname AS column, pg_get_userbyid(g.grantor) AS grantor,
      CASE WHEN g.grantee=0 THEN 'PUBLIC' ELSE pg_get_userbyid(g.grantee) END AS grantee,
      g.privilege_type, g.is_grantable FROM pg_attribute a
      JOIN pg_class c ON c.oid=a.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace,
      LATERAL aclexplode(COALESCE(a.attacl,'{}'::aclitem[])) g
      WHERE n.nspname='genui_meta' AND c.relname='schema_version' AND a.attnum>0 AND NOT a.attisdropped""",
    "constraints": """SELECT k.conname AS name, k.contype AS type,
      pg_get_constraintdef(k.oid,true) AS definition, k.convalidated AS validated
      FROM pg_constraint k JOIN pg_class c ON c.oid=k.conrelid
      JOIN pg_namespace n ON n.oid=c.relnamespace
      WHERE n.nspname='genui_meta' AND c.relname='schema_version'""",
    "effective_app_grants": """SELECT r.rolname AS role,
      has_database_privilege(r.oid,current_database(),'CONNECT') AS connect,
      has_database_privilege(r.oid,current_database(),'CREATE,TEMP') AS database_write,
      has_schema_privilege(r.oid,'genui_meta','USAGE') AS metadata_usage,
      has_schema_privilege(r.oid,'genui_meta','CREATE') AS metadata_create,
      has_schema_privilege(r.oid,'public','CREATE') AS public_create,
      has_table_privilege(r.oid,'genui_meta.schema_version','SELECT') AS metadata_select,
      has_table_privilege(r.oid,'genui_meta.schema_version','INSERT,UPDATE,DELETE,TRUNCATE,REFERENCES,TRIGGER') AS metadata_write
      FROM pg_roles r WHERE r.rolname = ANY(%s)""",
}


def require(condition):
    if not condition:
        raise ValueError("foundation_restore_manifest_refused")


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def validate(manifest):
    require(manifest["server_version_num"] == 160013)
    require(manifest["schema_version"] == [{"singleton": True, "version": 1}])
    require(len(manifest["roles"]) == 5 and {r["name"] for r in manifest["roles"]} == set(ROLES))
    for role in manifest["roles"]:
        require(not any(role[f] for f in ("rolsuper", "rolcreatedb", "rolcreaterole", "rolreplication", "rolbypassrls")))
        require(role["rolcanlogin"] == (role["name"] != "genui_owner"))
        require(role["rolinherit"] == (role["name"] == "genui_owner"))
    require({(m["member"], m["role"]) for m in manifest["membership_closure"]} ==
            {(name, "rds_iam") for name in ROLES[1:]})
    require(not any(m["member"] in ROLES and (m["admin_option"] or m["inherit_option"])
                    for m in manifest["memberships"]))
    require(manifest["database"] == [{"name": "genui", "owner": "genui_migration"}])
    require({s["name"] for s in manifest["schemas"]} == {"public", "genui_meta"})
    require(next(s["owner"] for s in manifest["schemas"] if s["name"] == "genui_meta") == "genui_owner")
    require(manifest["tables"] == [{"schema": "genui_meta", "name": "schema_version", "kind": "r",
                                   "owner": "genui_owner", "relrowsecurity": False, "relforcerowsecurity": False}])
    require({(c["name"], c["type"], c["position"], c["attnotnull"]) for c in manifest["columns"]} ==
            {("singleton", "boolean", 1, True), ("version", "integer", 2, True)})
    require(len(manifest["effective_app_grants"]) == 4)
    for row in manifest["effective_app_grants"]:
        require(row == {"role": row["role"], "connect": True, "database_write": False,
                        "metadata_usage": True, "metadata_create": False, "public_create": False,
                        "metadata_select": True, "metadata_write": False} and row["role"] in ROLES[1:])
    require(not any(g["grantee"] == "PUBLIC" for g in manifest["database_grants"]))


def collect(connection):
    connection.execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
    identity = connection.execute("""SELECT session_user::text AS session_user, current_user::text AS current_user,
      current_database() AS database, current_setting('server_version_num')::integer AS server_version_num,
      current_setting('transaction_read_only') AS read_only,
      current_setting('statement_timeout') AS statement_timeout,
      current_setting('lock_timeout') AS lock_timeout,
      (SELECT ssl FROM pg_stat_ssl WHERE pid=pg_backend_pid()) AS ssl""").fetchone()
    require(identity["session_user"] == identity["current_user"] == "genui_migration")
    require(identity["database"] == "genui" and identity["read_only"] == "on" and identity["ssl"] is True)
    require(identity["statement_timeout"] == "5s" and identity["lock_timeout"] == "2s")
    manifest = {"manifest_version": 1, "server_version_num": identity["server_version_num"]}
    for name, query in QUERIES.items():
        parameters = None
        if name in ("roles", "membership_closure"):
            parameters = (list(ROLES),)
        elif name == "memberships":
            parameters = (list(ROLES), list(ROLES))
        elif name == "effective_app_grants":
            parameters = (list(ROLES[1:]),)
        manifest[name] = sorted(connection.execute(query, parameters).fetchall(), key=canonical)
    validate(manifest)
    return {"scope": SCOPE, "passed": True,
            "assertions": {"uid": 65532, "database_identity_verified": True, "tls_verify_full": True,
                           "transaction_read_only": True, "foundation_state_verified": True},
            "manifest_sha256": hashlib.sha256(canonical(manifest).encode()).hexdigest(), "manifest": manifest}


def run():
    require(os.environ.get("GENUI_QUALIFY_SYNTHETIC") == "APP-007" and os.geteuid() == 65532)
    require(os.environ.get("PGUSER") == "genui_migration" and os.environ.get("PGDATABASE") == "genui")
    host = os.environ["PGHOST"]
    require(re.fullmatch(r"[a-z0-9.-]+\.us-east-1\.rds\.amazonaws\.com", host) is not None)
    require(os.environ.get("PGPORT", "5432") == "5432")
    require(os.environ.get("PGSSLROOTCERT") == "/etc/ssl/certs/rds-global-bundle.pem")
    import psycopg
    from psycopg.rows import dict_row
    with psycopg.connect(host=host, port=5432, dbname="genui", user="genui_migration",
                         password=os.environ["PGPASSWORD"], sslmode="verify-full",
                         sslrootcert=os.environ["PGSSLROOTCERT"], connect_timeout=5, row_factory=dict_row,
                         options="-c search_path=pg_catalog -c default_transaction_read_only=on "
                                 "-c statement_timeout=5000 -c lock_timeout=2000 -c idle_in_transaction_session_timeout=10000") as connection:
        try:
            return collect(connection)
        finally:
            connection.rollback()


def expired(*_):
    raise TimeoutError("foundation_restore_manifest_refused")


def main():
    signal.signal(signal.SIGALRM, expired)
    signal.alarm(60)
    try:
        result = run()
    except Exception:
        print(canonical({"scope": SCOPE, "passed": False, "error": "foundation_restore_manifest_refused"}), flush=True)
        return 1
    finally:
        signal.alarm(0)
    print(canonical(result), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
