"""Local PostgreSQL 16 regression only; simulated rds_iam is not managed evidence."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time
import uuid


DEFAULT_IMAGE = "postgres@sha256:5d143123fdf80462d1778cd4f24b9f7ca13c87174bca19141fb194c5a1ebca59"
MIGRATION = Path(__file__).resolve().parents[1] / "migrations/0001_role_foundation.sql"
ROLES = ("genui_workspace", "genui_control", "genui_data", "genui_worker")


def qualify(image):
    name = "genui-app007-migration-" + uuid.uuid4().hex[:12]
    report = {"scope": "APP-007 local PostgreSQL migration regression", "managed_acceptance": False,
              "image": image, "container": name, "network": "none", "published_ports": [],
              "migration_sha256": hashlib.sha256(MIGRATION.read_bytes()).hexdigest(), "checks": [], "passed": False}

    def command(args, *, source=None, check=True):
        result = subprocess.run(args, input=source, text=True, capture_output=True, timeout=60)
        if check and result.returncode:
            raise RuntimeError(f"Command failed: {args!r}\n{result.stderr}")
        return result

    def sql(source, *, user="genui_migration", database="genui", transaction=True, check=True):
        args = ["docker", "exec", "-i", name, "psql", "--no-psqlrc", "--set=ON_ERROR_STOP=1",
                "--set=VERBOSITY=verbose", "--tuples-only", "--no-align", "--username", user,
                "--dbname", database, "--file=-"]
        if transaction:
            args.append("--single-transaction")
        return command(args, source=source, check=check)

    def denied(label, source):
        result = sql(source, check=False)
        assert result.returncode != 0 and "42501" in result.stderr, (label, result.stdout, result.stderr)
        report["checks"].append({"name": label, "denied": True, "sqlstate": "42501"})
        return result.stderr.strip()

    def snapshot():
        return sql("""SELECT json_agg(row_to_json(r) ORDER BY rolname) FROM
          (SELECT rolname, rolsuper, rolcreatedb, rolcreaterole, rolreplication, rolbypassrls,
                  rolcanlogin, rolinherit FROM pg_roles WHERE rolname LIKE 'genui_%') r;
          SELECT json_agg(row_to_json(m) ORDER BY roleid, member, grantor) FROM
          (SELECT roleid, member, grantor, admin_option, inherit_option, set_option
             FROM pg_auth_members) m;
          SELECT json_agg(row_to_json(v)) FROM genui_meta.schema_version v;""").stdout

    try:
        command(["docker", "run", "--detach", "--rm", "--name", name, "--network=none",
                 "--memory=512m", "--cpus=1", "--tmpfs=/var/lib/postgresql/data:rw,noexec,nosuid,size=256m",
                 "--env=POSTGRES_HOST_AUTH_METHOD=trust", image])
        for _ in range(60):
            # The image's temporary initialization server accepts Unix-socket
            # connections, then stops. Only the final server listens on TCP.
            if command(["docker", "exec", name, "pg_isready", "--host=127.0.0.1", "--username=postgres"], check=False).returncode == 0:
                break
            time.sleep(0.25)
        else:
            raise RuntimeError("Local PostgreSQL startup timeout")
        report["server_version"] = sql("SHOW server_version;", user="postgres", database="postgres").stdout.strip()
        assert report["server_version"].startswith("16."), report["server_version"]
        sql("""CREATE ROLE genui_migration LOGIN INHERIT CREATEDB CREATEROLE NOSUPERUSER NOBYPASSRLS;
          CREATE ROLE rds_iam NOLOGIN;
          GRANT rds_iam TO genui_migration WITH ADMIN TRUE;
          CREATE DATABASE genui OWNER genui_migration;""", user="postgres", database="postgres", transaction=False)
        identity = sql("SELECT session_user, current_user, rolsuper, rolcreaterole, rolbypassrls FROM pg_roles WHERE rolname=current_user;").stdout.strip()
        assert identity == "genui_migration|genui_migration|f|t|f", identity
        report["administrator"] = identity
        report["legacy_failures"] = []
        for attribute in ("NOSUPERUSER", "NOBYPASSRLS"):
            failure = denied("legacy ALTER " + attribute,
                             f"CREATE ROLE legacy_role NOLOGIN; ALTER ROLE legacy_role {attribute};")
            report["legacy_failures"].append({"attribute": attribute, "diagnostic": failure})
            assert sql("SELECT count(*) FROM pg_roles WHERE rolname='legacy_role';").stdout.strip() == "0"

        migration = MIGRATION.read_text()
        sql("CREATE ROLE genui_workspace LOGIN SUPERUSER;", user="postgres")
        denied("fresh migration refuses existing elevated role", migration)
        assert sql("""SELECT rolsuper FROM pg_roles WHERE rolname='genui_workspace';
          SELECT count(*) FROM pg_roles WHERE rolname IN ('genui_owner','genui_control','genui_data','genui_worker');
          SELECT count(*) FROM pg_namespace WHERE nspname='genui_meta';""").stdout.strip() == "t\n0\n0"
        sql("DROP ROLE genui_workspace;", user="postgres")
        sql(migration)
        report["checks"].append({"name": "first migration as non-superuser administrator", "passed": True})
        first = snapshot()
        sql(migration)
        assert snapshot() == first
        report["checks"].append({"name": "repeat migration preserves role state and version", "passed": True})
        assert sql("SELECT version FROM genui_meta.schema_version;").stdout.strip() == "1"

        report["role_grants"] = []
        for role in ROLES:
            facts = sql("""SELECT session_user, current_user,
              has_database_privilege(current_user,'genui','CONNECT'),
              has_database_privilege(current_user,'genui','CREATE'),
              has_database_privilege(current_user,'genui','TEMP'),
              has_schema_privilege(current_user,'genui_meta','USAGE'),
              has_schema_privilege(current_user,'genui_meta','CREATE'),
              has_schema_privilege(current_user,'public','CREATE'),
              has_table_privilege(current_user,'genui_meta.schema_version','SELECT'),
              has_table_privilege(current_user,'genui_meta.schema_version','INSERT'),
              has_table_privilege(current_user,'genui_meta.schema_version','UPDATE'),
              has_table_privilege(current_user,'genui_meta.schema_version','DELETE'),
              (SELECT version FROM genui_meta.schema_version);""", user=role).stdout.strip()
            assert facts == f"{role}|{role}|t|f|f|t|f|f|t|f|f|f|1", facts
            report["role_grants"].append(facts)
            for label, statement in (("owner role", "SET ROLE genui_owner"),
                                     ("migration role", "SET ROLE genui_migration"),
                                     ("sibling role", "SET ROLE " + ROLES[(ROLES.index(role) + 1) % 4]),
                                     ("schema creation", "CREATE SCHEMA forbidden_schema"),
                                     ("temporary table", "CREATE TEMP TABLE forbidden_temp (n integer)"),
                                     ("metadata write", "UPDATE genui_meta.schema_version SET version=version")):
                result = sql(statement, user=role, check=False)
                assert result.returncode != 0 and "42501" in result.stderr, (role, label, result.stderr)
                report["checks"].append({"name": role + " denied " + label, "denied": True, "sqlstate": "42501"})

        sql("CREATE ROLE unexpected_group NOLOGIN;", user="postgres")
        drifts = [
            ("app " + flag, "ALTER ROLE genui_workspace " + flag, "ALTER ROLE genui_workspace " + reset)
            for flag, reset in (("SUPERUSER", "NOSUPERUSER"), ("BYPASSRLS", "NOBYPASSRLS"),
                                ("CREATEDB", "NOCREATEDB"), ("CREATEROLE", "NOCREATEROLE"),
                                ("REPLICATION", "NOREPLICATION"), ("INHERIT", "NOINHERIT"), ("NOLOGIN", "LOGIN"))
        ] + [
            ("owner login", "ALTER ROLE genui_owner LOGIN", "ALTER ROLE genui_owner NOLOGIN"),
            ("owner privilege", "ALTER ROLE genui_owner BYPASSRLS", "ALTER ROLE genui_owner NOBYPASSRLS"),
            ("app unexpected membership", "GRANT unexpected_group TO genui_workspace", "REVOKE unexpected_group FROM genui_workspace"),
            ("owner unexpected membership", "GRANT unexpected_group TO genui_owner", "REVOKE unexpected_group FROM genui_owner"),
            ("IAM grant admin option", "GRANT rds_iam TO genui_workspace WITH ADMIN TRUE", "GRANT rds_iam TO genui_workspace WITH ADMIN FALSE"),
            ("IAM grant inherit option", "GRANT rds_iam TO genui_workspace WITH INHERIT TRUE", "GRANT rds_iam TO genui_workspace WITH INHERIT FALSE"),
        ]
        for label, introduce, restore in drifts:
            sql(introduce, user="postgres")
            before = snapshot()
            denied("migration refuses " + label, migration)
            assert snapshot() == before, label + " was changed by refused migration"
            sql(restore, user="postgres")
        sql(migration)
        report["checks"].append({"name": "safe migration remains repeatable after test cleanup", "passed": True})
        report["passed"] = True
    finally:
        result = command(["docker", "rm", "--force", "--volumes", name], check=False)
        report["container_removed"] = result.returncode == 0
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", default=DEFAULT_IMAGE)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = qualify(args.image)
    rendered = json.dumps(result, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered)
    print(rendered, end="")
