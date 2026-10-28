"""Exact PostgreSQL16.13 catalog-query regression; no TLS/RDS/restore assertion."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import time
import uuid

ROOT = Path(__file__).resolve().parents[3]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output-dir', type=Path, default=ROOT / '.cache/APP-007/restore-sql')
OUT = parser.parse_args().output_dir
OUT.mkdir(parents=True, exist_ok=True)
IMAGE = "postgres@sha256:5d143123fdf80462d1778cd4f24b9f7ca13c87174bca19141fb194c5a1ebca59"
SOURCE = ROOT / "infra/scripts/foundation_restore_manifest.py"
MIGRATION = ROOT / "app/migrations/0001_role_foundation.sql"
name = "genui-app007-restore-sql-" + uuid.uuid4().hex[:12]
report = {"scope": "APP-007 local restore-manifest SQL/catalog compatibility", "passed": False,
          "managed_acceptance": False, "image": IMAGE, "container": name, "network": "none",
          "published_ports": [], "commands": [], "validator_sha256": hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
          "migration_sha256": hashlib.sha256(MIGRATION.read_bytes()).hexdigest()}


def command(args, source=None, check=True):
    result = subprocess.run(args, input=source, text=True, capture_output=True, timeout=60)
    report["commands"].append({"argv": args, "stdin_sha256": hashlib.sha256(source.encode()).hexdigest() if source else None,
                               "exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr})
    if check and result.returncode:
        raise RuntimeError("local_command_failed")
    return result


def sql(source, user="genui_migration", database="genui"):
    return command(["docker", "exec", "-i", name, "psql", "--no-psqlrc", "--set=ON_ERROR_STOP=1",
                    "--set=VERBOSITY=verbose", "--quiet", "--tuples-only", "--no-align", "--username", user,
                    "--dbname", database, "--file=-"], source).stdout


spec = importlib.util.spec_from_file_location("restore_manifest_under_test", SOURCE)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
bootstrap = """CREATE ROLE genui_migration LOGIN INHERIT CREATEDB CREATEROLE NOSUPERUSER NOBYPASSRLS;
CREATE ROLE rds_iam NOLOGIN;
GRANT rds_iam TO genui_migration WITH ADMIN TRUE;
CREATE DATABASE genui OWNER genui_migration;
"""
(OUT / "bootstrap.sql").write_text(bootstrap)
# The only bound parameters are the source's constant foundation-role names.
# This substitution preserves the actual SELECT text and produces typed arrays
# for psql, which has no psycopg-style parameter binding interface.
def array(names):
    return "ARRAY[" + ",".join("'" + n.replace("'", "''") + "'" for n in names) + "]::text[]"


statements = ["BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;",
              "SET LOCAL search_path=pg_catalog; SET LOCAL statement_timeout=5000; SET LOCAL lock_timeout=2000;",
              "SELECT json_build_object('identity', json_build_object('session_user', session_user, 'current_user', current_user, 'database', current_database(), 'read_only', current_setting('transaction_read_only'), 'server_version_num', current_setting('server_version_num')::integer));"]
for key, original in module.QUERIES.items():
    query = original
    if key == "effective_app_grants":
        query = query.replace("%s", array(module.ROLES[1:]))
    else:
        query = query.replace("%s", array(module.ROLES))
    statements.append("SELECT json_build_object('name', '" + key + "', 'rows', COALESCE(json_agg(row_to_json(q)), '[]'::json)) FROM (" + query + ") q;")
statements.append("ROLLBACK;")
queries = "\n".join(statements) + "\n"
(OUT / "queries.sql").write_text(queries)


try:
    command(["docker", "run", "--detach", "--rm", "--name", name, "--network=none", "--memory=512m", "--cpus=1",
             "--tmpfs=/var/lib/postgresql/data:rw,noexec,nosuid,size=256m", "--env=POSTGRES_HOST_AUTH_METHOD=trust", IMAGE])
    for _ in range(60):
        if command(["docker", "exec", name, "pg_isready", "--host=127.0.0.1", "--username=postgres"], check=False).returncode == 0:
            break
        time.sleep(0.25)
    else:
        raise RuntimeError("local_startup_timeout")
    sql(bootstrap, "postgres", "postgres")
    sql("BEGIN;\n" + MIGRATION.read_text() + "\nCOMMIT;\n")
    def manifest_from_sql():
        records = [json.loads(line) for line in sql(queries).splitlines() if line.strip()]
        identity = records[0]["identity"]
        assert identity == {"session_user": "genui_migration", "current_user": "genui_migration", "database": "genui", "read_only": "on", "server_version_num": 160013}
        assert {r["name"] for r in records[1:]} == set(module.QUERIES)
        manifest = {"manifest_version": 1, "server_version_num": identity["server_version_num"]}
        manifest.update({r["name"]: sorted(r["rows"], key=module.canonical) for r in records[1:]})
        return manifest

    manifests = []
    for _ in range(2):
        manifest = manifest_from_sql()
        module.validate(manifest)
        manifests.append(manifest)
    assert manifests[0] == manifests[1]
    null_acl = sql("SELECT count(*) FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='genui_meta' AND c.relname='schema_version' AND a.attnum>0 AND NOT a.attisdropped AND a.attacl IS NULL;").strip()
    assert null_acl == "2" and manifests[0]["column_grants"] == []
    assert sql("SELECT count(*) FROM aclexplode(NULL::aclitem[]);").strip() == "0"
    sql("GRANT UPDATE(version) ON genui_meta.schema_version TO genui_data;")
    changed = manifest_from_sql()
    assert changed["column_grants"] == [{"column": "version", "grantor": "genui_owner", "grantee": "genui_data", "privilege_type": "UPDATE", "is_grantable": False}]
    original_digest = hashlib.sha256(module.canonical(manifests[0]).encode()).hexdigest()
    changed_digest = hashlib.sha256(module.canonical(changed).encode()).hexdigest()
    assert original_digest != changed_digest
    try:
        module.validate(changed)
    except ValueError:
        pass
    else:
        raise AssertionError("unexpected column UPDATE grant was accepted")
    sql("REVOKE UPDATE(version) ON genui_meta.schema_version FROM genui_data;")
    restored = manifest_from_sql()
    module.validate(restored)
    assert restored == manifests[0]
    report.update(passed=True, server_version_num=160013, actual_query_count=len(module.QUERIES), repeated_manifest_equal=True,
                  manifest_sha256=original_digest, manifest=manifests[0], null_column_acl_count=2,
                  column_update_trial={"query_reported_grant": True, "validation_refused": True, "changed_manifest_sha256": changed_digest,
                                       "grant": changed["column_grants"], "all_queries_pass_after_revoke": True, "restored_original_manifest": True},
                  limitations=["Simulated rds_iam; local Unix-socket trust authentication.", "Calls actual QUERIES plus validate/canonical only, not run() or collect().", "No verified TLS, ECS secret channel, RDS, snapshot or restore acceptance."])
except Exception as error:
    report["failure_type"] = type(error).__name__
    raise
finally:
    removed = command(["docker", "rm", "--force", "--volumes", name], check=False)
    remaining = command(["docker", "ps", "-a", "--filter", "name=^/" + name + "$", "--format", "{{.Names}}"])
    report["cleanup_confirmed"] = removed.returncode == 0 and not remaining.stdout.strip()
    report["passed"] = report["passed"] and report["cleanup_confirmed"]
    (OUT / "results.json").write_text(json.dumps(report, indent=2) + "\n")

if not report["cleanup_confirmed"]:
    raise RuntimeError("owned_fixture_cleanup_failed")
print(json.dumps({key: report[key] for key in ("passed", "actual_query_count", "server_version_num", "manifest_sha256", "cleanup_confirmed")}))
