"""Independent APP-008 review probe; owned local PostgreSQL only, never AWS.

Run from the repository root with the locked runtime and contracts environments.
The final JSON is stdout; redirect it to a new retained evidence filename.
This intentionally installs a temporary view on an operator-owned connection.
"""
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "app/runtime"))
import runtime
from qualify_tenant_schema import LocalPostgres, MIGRATIONS

report = {
    "scope": "Independent session-local pg_temp relation-shadow probe; not an exposed application exploit",
    "production_preconditions": "Requires a caller-supplied operator connection already containing temporary relations; normal migrate() opens a fresh connection and app roles have no TEMP",
    "source_sha256": {
        str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
        for p in (ROOT / "app/runtime/runtime.py", MIGRATIONS / "catalog-query.json", Path(__file__))
    },
    "observations": [],
}
pg = LocalPostgres()
try:
    with pg:
        with pg.connect() as connection:
            runtime.apply_migrations(connection, MIGRATIONS)
        with pg.connect() as connection:
            connection.execute("""
              CREATE TEMP SEQUENCE marker;
              CREATE FUNCTION pg_temp.review_marker() RETURNS boolean
              LANGUAGE plpgsql VOLATILE AS $$ BEGIN
                PERFORM pg_catalog.nextval('pg_temp.marker'); RETURN true;
              END $$;
              CREATE TEMP VIEW pg_namespace AS
                SELECT n.* FROM pg_catalog.pg_namespace n WHERE pg_temp.review_marker();
            """)
            for label, operation in (
                ("verify_schema_catalog", runtime.verify_schema_catalog),
                ("apply_migrations", runtime.apply_migrations),
            ):
                connection.execute("SELECT pg_catalog.setval('pg_temp.marker',1,false)")
                assert connection.execute("SELECT is_called FROM pg_temp.marker").fetchone() == (False,)
                accepted = True
                try:
                    operation(connection, MIGRATIONS)
                except runtime.Refused:
                    accepted = False
                called, value = connection.execute("SELECT is_called,last_value FROM pg_temp.marker").fetchone()
                path = connection.execute("SHOW search_path").fetchone()[0]
                report["observations"].append({"operation": label, "accepted": accepted,
                                               "shadow_marker_called": called,
                                               "marker_value": value, "search_path": path})
finally:
    report["owned_container_removed"] = pg.removed
    print(json.dumps(report, indent=2))
