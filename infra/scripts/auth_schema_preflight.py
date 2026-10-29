"""Emit a bounded read-only APP-006 schema-one operator command; no AWS calls.

Only --emit-override is accepted from the source checkout. The emitted command
contains the exact reviewed query/baseline and needs no repository files. It
requires a reviewed migration-derived task with entryPoint=['python3']; its
existing execution-secret channel is preserved. No host secret is read.
"""
import base64
import hashlib
import json
import os
from pathlib import Path
import signal
import sys
import zlib

SCOPE = "APP-006 read-only schema-one auth preflight"
HOST = "genuisyntheticfoundation-databaseb269d8bb-pta9s49jelmg.cg58i06y2r4e.us-east-1.rds.amazonaws.com"
RESOURCE_ID = "db-KW4LRMTPI5YXHZXF4LELQZKTAU"
CA = "/etc/ssl/certs/rds-global-bundle.pem"
QUERY_SHA256 = "e17ceb5e62dc6f03dfb9dc26f34018d3f2977e1b93ee5910ed16f03364a63963"
EXPECTED_SHA256 = "4182e1425f2c7fc9d9540f2837cb92f07cd2521c86adc81f3add6e606e92aac9"
OPTIONS = ("-c search_path=pg_catalog,pg_temp -c default_transaction_read_only=on "
           "-c statement_timeout=5000 -c lock_timeout=2000 "
           "-c idle_in_transaction_session_timeout=10000")
EMBEDDED_RESOURCES = None
SOURCE_SHA256 = None


class Refused(Exception):
    pass


def require(condition):
    if not condition:
        raise Refused("auth_schema_preflight_refused")


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def resources():
    require(isinstance(EMBEDDED_RESOURCES, tuple) and len(EMBEDDED_RESOURCES) == 2)
    query_raw, expected_raw = EMBEDDED_RESOURCES
    for raw, digest in ((query_raw, QUERY_SHA256), (expected_raw, EXPECTED_SHA256)):
        require(isinstance(raw, str) and 0 < len(raw.encode()) <= 262144)
        require(hashlib.sha256(raw.encode()).hexdigest() == digest)
    query, expected = json.loads(query_raw), json.loads(expected_raw)
    require(set(query) == {"schema_version", "search_path", "max_output_bytes", "section_row_limit", "sql"})
    require(query["schema_version"] == 1 and query["search_path"] == "pg_catalog, pg_temp"
            and query["max_output_bytes"] == 262144 and query["section_row_limit"] == 1025
            and isinstance(query["sql"], str) and 0 < len(query["sql"].encode()) <= 65536)
    require(isinstance(expected, dict) and expected
            and all(isinstance(rows, list) and len(rows) < 1025 for rows in expected.values()))
    return query, expected


def collect(connection):
    """Caller supplies a fresh TLS operator connection; this never commits."""
    require(not connection.autocommit)
    query, expected = resources()
    # Startup options establish this order before libpq sends the first SQL.
    # Fully qualified identity functions also avoid operator pg_temp shadows.
    connection.execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
    require(connection.execute("SHOW session_replication_role").fetchone() == ("origin",))
    require(connection.execute("SHOW search_path").fetchone() == ("pg_catalog,pg_temp",))
    identity = connection.execute("""SELECT session_user::text, current_user::text,
      pg_catalog.current_database(), pg_catalog.current_setting('server_version_num'),
      pg_catalog.current_setting('transaction_read_only'),
      pg_catalog.current_setting('default_transaction_read_only'),
      pg_catalog.current_setting('statement_timeout'), pg_catalog.current_setting('lock_timeout'),
      pg_catalog.current_setting('idle_in_transaction_session_timeout'),
      (SELECT ssl FROM pg_catalog.pg_stat_ssl WHERE pid=pg_catalog.pg_backend_pid())""").fetchone()
    require(identity == ("genui_migration", "genui_migration", "genui", "160013", "on", "on", "5s", "2s", "10s", True))
    rows = connection.execute(query["sql"]).fetchall()
    require(len(rows) == 1 and isinstance(rows[0][0], str)
            and 0 < len(rows[0][0].encode()) <= query["max_output_bytes"])
    observed = json.loads(rows[0][0])
    require(observed == expected)
    # No application relation, including a substituted view, is evaluated until
    # the exact canonical catalog proves the version-one table and privileges.
    require(connection.execute("SELECT singleton, version FROM genui_meta.schema_version").fetchall() == [(True, 1)])
    return {"scope": SCOPE, "passed": True, "database_version": 1,
            "source_sha256": SOURCE_SHA256,
            "query_sha256": QUERY_SHA256, "expected_sha256": EXPECTED_SHA256,
            "catalog_sha256": hashlib.sha256(canonical(observed).encode()).hexdigest(),
            "assertions": {"fixed_operator_identity": True, "postgres_16_13": True,
                           "tls": True, "read_only": True, "origin": True,
                           "catalog_before_metadata": True},
            "limits": ["Point-in-time SQL proof; preserve operator/schema quiescence for the auth window.",
                       "No hosted identity, deployment, invitation, application access or schema-two acceptance."]}


def connection_arguments():
    # Reject libpq's alternate address/service/options channels before connecting.
    allowed = {"PGHOST", "PGPORT", "PGDATABASE", "PGUSER", "PGSSLROOTCERT", "PGSSLMODE", "PGPASSWORD"}
    require(not {key for key in os.environ if key.startswith("PG")} - allowed)
    require(os.environ.get("PGHOST") == HOST and os.environ.get("PGPORT") == "5432")
    require(os.environ.get("PGUSER") == "genui_migration" and os.environ.get("PGDATABASE") == "genui")
    require(os.environ.get("PGSSLMODE") == "verify-full" and os.environ.get("PGSSLROOTCERT") == CA)
    password = os.environ.pop("PGPASSWORD", None)
    require(isinstance(password, str) and 0 < len(password) <= 4096 and "\0" not in password)
    return dict(host=HOST, port=5432, dbname="genui", user="genui_migration", password=password,
                sslmode="verify-full", sslrootcert=CA, connect_timeout=5, options=OPTIONS)


def run():
    require(os.geteuid() == 65532)
    resources()
    arguments = connection_arguments()
    import psycopg
    with psycopg.connect(**arguments) as connection:
        try:
            result = collect(connection)
        finally:
            # Successful context-manager exit must never commit this operation.
            connection.rollback()
    result["target"] = {"host": HOST, "resource_id_from_reviewed_control_plane": RESOURCE_ID}
    result["assertions"]["tls_verify_full"] = True
    return result


def expired(*_):
    raise Refused("auth_schema_preflight_refused")


def emit_override():
    require(EMBEDDED_RESOURCES is None)
    source_path = Path(__file__).resolve()
    directory = source_path.parents[2] / "app" / "migrations"
    raw = tuple((directory / name).read_text() for name in ("catalog-query.json", "catalog-v1.json"))
    source = source_path.read_text()
    source_hash = hashlib.sha256(source.encode()).hexdigest()
    embedded = source.replace("EMBEDDED_RESOURCES = None", "EMBEDDED_RESOURCES = " + repr(raw), 1)
    embedded = embedded.replace("SOURCE_SHA256 = None", "SOURCE_SHA256 = " + repr(source_hash), 1)
    # Source, query and expected bytes are all fixed, not operator arguments.
    namespace = {"__name__": "auth_preflight_packaging"}
    exec(compile(embedded, "<auth-schema-preflight>", "exec"), namespace)
    namespace["resources"]()
    payload = embedded.encode()
    packed = base64.b85encode(zlib.compress(payload, 9)).decode()
    bootstrap = ("import base64,hashlib,zlib;"
                 "s=zlib.decompress(base64.b85decode(" + repr(packed) + "));"
                 "assert len(s)==" + str(len(payload)) + ";"
                 "assert hashlib.sha256(s).hexdigest()==" + repr(hashlib.sha256(payload).hexdigest()) + ";"
                 "exec(compile(s,'<auth-schema-preflight>','exec'))")
    override = {"containerOverrides": [{"name": "migration", "command": ["-c", bootstrap]}]}
    encoded = canonical(override)
    require(len(encoded.encode()) <= 8192)
    return encoded


def main():
    signal.signal(signal.SIGALRM, expired)
    signal.alarm(30)
    try:
        if EMBEDDED_RESOURCES is None:
            require(sys.argv[1:] == ["--emit-override"])
            print(emit_override(), flush=True)
        else:
            require(sys.argv[1:] == [])
            print(canonical(run()), flush=True)
        return 0
    except Exception:
        print(canonical({"scope": SCOPE, "passed": False, "error": "auth_schema_preflight_refused"}), flush=True)
        return 1
    finally:
        signal.alarm(0)


if __name__ == "__main__":
    raise SystemExit(main())
