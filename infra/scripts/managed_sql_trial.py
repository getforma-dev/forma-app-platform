"""APP-007 operator-invoked SQL qualification; never startup or APP-008 acceptance.

Run as the reviewed application task with GENUI_QUALIFY_SYNTHETIC=APP-007.
This source can be the sole command argument to entryPoint ["python3", "-c"].
Only main() obtains fresh task credentials; importing this file has no I/O.
"""
import importlib.util
import ipaddress
import json
import os
from pathlib import Path
import re
import signal
import sys
import tempfile
import uuid


ROLES = ("workspace", "control", "data", "worker")


class Refused(Exception):
    """A failed assertion with no external diagnostic attached."""


def require(condition):
    if not condition:
        raise Refused()


def sqlstate(error):
    value = getattr(error, "sqlstate", None)
    return value if isinstance(value, str) and re.fullmatch(r"[0-9A-Z]{5}", value) else None


class Trial:
    def __init__(self):
        self.report = {"schema_version": 1, "scope": "APP-007 managed SQL role qualification",
                       "run_id": str(uuid.uuid4()), "app008_acceptance": False,
                       "passed": False, "checks": []}
        self.role = None

    def begin(self, name):
        check = {"name": name, "passed": False}
        if self.role:
            check["role"] = self.role
        self.report["checks"].append(check)
        return check

    def denied(self, connection, psycopg, name, statement):
        check = self.begin(name)
        # Autocommit applies only to observations. No mutation may run outside
        # this explicit transaction, and even unexpected success is rolled back.
        connection.execute("BEGIN READ WRITE")
        try:
            require(connection.execute("SHOW transaction_read_only").fetchone() == ("off",))
            try:
                connection.execute(statement)
            except psycopg.Error as error:
                check["sqlstate"] = sqlstate(error)
            else:
                check["unexpectedly_allowed"] = True
        finally:
            connection.rollback()
            require(connection.info.transaction_status == psycopg.pq.TransactionStatus.IDLE)
            check["rolled_back"] = True
        # SET ROLE is transaction-scoped here: rollback must restore identity.
        require(connection.execute("SELECT current_user::text").fetchone() == (self.role,))
        require(check.get("sqlstate") == "42501")
        check["read_only"] = False
        check["passed"] = True

    def role_checks(self, psycopg, db, name, ca_file):
        self.role = "genui_" + name
        check = self.begin("connect_tls_verify_full")
        args = dict(host=db["host"], port=5432, dbname="genui", user=self.role,
                    password=db["roles"][name]["token"], sslmode="verify-full",
                    sslrootcert=ca_file, ssl_min_protocol_version="TLSv1.2",
                    gssencmode="disable", passfile="/dev/null", connect_timeout=3,
                    application_name="app007-managed-sql-trial", autocommit=True,
                    options="-c default_transaction_read_only=off -c statement_timeout=3000 "
                            "-c lock_timeout=1000 -c idle_in_transaction_session_timeout=5000 "
                            "-c search_path=pg_catalog")
        connection = psycopg.connect(**args)
        try:
            require(connection.pgconn.ssl_in_use)
            require(connection.execute("SELECT ssl FROM pg_stat_ssl WHERE pid=pg_backend_pid()").fetchone() == (True,))
            require(connection.info.get_parameters().get("sslmode") == "verify-full")
            check["passed"] = True

            check = self.begin("identity_flags_noinherit_write_enabled")
            row = connection.execute("""SELECT session_user::text, current_user::text, current_database(),
                rolsuper, rolcreatedb, rolcreaterole, rolreplication, rolbypassrls, rolcanlogin, rolinherit,
                current_setting('default_transaction_read_only'), current_setting('transaction_read_only')
                FROM pg_roles WHERE rolname=current_user""").fetchone()
            require(row == (self.role, self.role, "genui", False, False, False, False, False, True, False, "off", "off"))
            check["passed"] = True

            check = self.begin("connect_usage_select_and_create_temp_absence")
            row = connection.execute("""SELECT
                has_database_privilege(current_user,'genui','CONNECT'),
                has_database_privilege(current_user,'genui','CREATE'),
                has_database_privilege(current_user,'genui','TEMP'),
                has_schema_privilege(current_user,'genui_meta','USAGE'),
                has_schema_privilege(current_user,'genui_meta','CREATE'),
                has_schema_privilege(current_user,'public','CREATE'),
                has_table_privilege(current_user,'genui_meta.schema_version','SELECT')""").fetchone()
            require(row == (True, False, False, True, False, False, True))
            require(connection.execute("SELECT singleton, version FROM genui_meta.schema_version").fetchall() == [(True, 1)])
            check["passed"] = True

            check = self.begin("metadata_table_and_column_mutation_grants_absent")
            row = connection.execute("""SELECT
                has_table_privilege(current_user,'genui_meta.schema_version','INSERT'),
                has_table_privilege(current_user,'genui_meta.schema_version','UPDATE'),
                has_table_privilege(current_user,'genui_meta.schema_version','DELETE'),
                has_table_privilege(current_user,'genui_meta.schema_version','TRUNCATE'),
                has_table_privilege(current_user,'genui_meta.schema_version','REFERENCES'),
                has_table_privilege(current_user,'genui_meta.schema_version','TRIGGER'),
                has_any_column_privilege(current_user,'genui_meta.schema_version','INSERT'),
                has_any_column_privilege(current_user,'genui_meta.schema_version','UPDATE'),
                has_any_column_privilege(current_user,'genui_meta.schema_version','REFERENCES')""").fetchone()
            require(row == (False,) * 9)
            check["passed"] = True

            check = self.begin("only_rds_iam_membership_without_admin_or_inherit")
            require(connection.execute("SELECT pg_has_role(current_user,'rds_iam','MEMBER')").fetchone() == (True,))
            require(connection.execute("""SELECT count(*) FROM pg_roles WHERE rolname NOT IN (current_user,'rds_iam')
                AND pg_has_role(current_user,oid,'MEMBER')""").fetchone() == (0,))
            require(connection.execute("""SELECT count(*) FROM pg_auth_members m JOIN pg_roles r ON r.oid=m.member
                WHERE r.rolname=current_user AND (m.admin_option OR m.inherit_option)""").fetchone() == (0,))
            check["passed"] = True

            check = self.begin("tls_hostname_mismatch_rejected")
            address = str(ipaddress.ip_address(connection.info.hostaddr))
            try:
                bad = psycopg.connect(**{**args, "hostaddr": address, "host": "app007.invalid"})
            except psycopg.OperationalError as error:
                # Match only libpq's specific hostname-verification failure.
                # DNS, timeout, authentication and arbitrary TLS failures fail.
                diagnostic = str(error)
                require(sqlstate(error) is None and "server certificate" in diagnostic
                        and "does not match host name" in diagnostic and '"app007.invalid"' in diagnostic)
            else:
                bad.close()
                raise Refused()
            check["passed"] = True

            targets = ("genui_owner", "genui_migration", "rds_superuser") + tuple(
                "genui_" + sibling for sibling in ROLES if sibling != name)
            check = self.begin("forbidden_role_targets_exist")
            require(connection.execute("SELECT count(*) FROM pg_roles WHERE rolname = ANY(%s)", (list(targets),)).fetchone() == (6,))
            check["passed"] = True
            for target in targets:
                self.denied(connection, psycopg, "set_role_" + target,
                            psycopg.sql.SQL("SET ROLE {}").format(psycopg.sql.Identifier(target)))

            # Identifiers are synthetic, unique per role/run, and below 63 bytes.
            obj = "app007_" + uuid.uuid4().hex
            for label, statement in (
                ("create_schema", f'CREATE SCHEMA "{obj}"'),
                ("create_temporary_table", f'CREATE TEMP TABLE "{obj}" (n integer)'),
                ("create_public_table", f'CREATE TABLE public."{obj}" (n integer)'),
                ("create_metadata_table", f'CREATE TABLE genui_meta."{obj}" (n integer)'),
                ("insert_metadata", "INSERT INTO genui_meta.schema_version SELECT true, 1 WHERE false"),
                ("update_metadata", "UPDATE genui_meta.schema_version SET version=version WHERE singleton"),
                ("delete_metadata", "DELETE FROM genui_meta.schema_version WHERE singleton"),
            ):
                self.denied(connection, psycopg, label, statement)
            check = self.begin("metadata_unchanged_after_rollback")
            require(connection.execute("SELECT singleton, version FROM genui_meta.schema_version").fetchall() == [(True, 1)])
            check["passed"] = True
        finally:
            # Closing also aborts any transaction if rollback/transport failed.
            connection.close()
        self.role = None


def interrupted(_signum, _frame):
    raise Refused()


def main():
    trial = Trial()
    try:
        check = trial.begin("explicit_invocation_and_runtime")
        require(os.environ.get("GENUI_QUALIFY_SYNTHETIC") == "APP-007")
        require(os.geteuid() == 65532)
        sys.dont_write_bytecode = True
        signal.signal(signal.SIGTERM, interrupted)
        signal.signal(signal.SIGINT, interrupted)
        signal.signal(signal.SIGALRM, interrupted)
        signal.alarm(120)
        spec = importlib.util.spec_from_file_location("app007_runtime", "/app/runtime.py")
        runtime = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(runtime)
        import psycopg
        require(runtime.ROLES == ROLES)
        check["passed"] = True

        # Never reuse or overwrite a running service's credential generation.
        # The helper assumes all six roles; only the four SQL tokens are used.
        with tempfile.TemporaryDirectory(prefix=".app007-sql-", dir="/run/genui") as directory:
            path = Path(directory) / "credentials.json"
            check = trial.begin("fresh_private_credentials")
            runtime.refresh(path)
            value = runtime.load(path)
            check["passed"] = True
            for name in ROLES:
                trial.role_checks(psycopg, value["database"], name, os.environ["GENUI_RDS_CA_FILE"])
            check = trial.begin("same_valid_credential_generation")
            final = runtime.load(path)
            require(final["_file_sha256"] == value["_file_sha256"])
            check["passed"] = True
        check = trial.begin("private_credential_file_removed")
        require(not path.exists() and not Path(directory).exists())
        check["passed"] = True
        trial.report["passed"] = True
    except BaseException as error:
        # No exception text, connection info, host, token, or AWS data escapes.
        trial.report["failure"] = "assertion_or_interrupt" if isinstance(error, Refused) else "operation_failed"
        if sqlstate(error):
            trial.report["sqlstate"] = sqlstate(error)
    finally:
        signal.alarm(0)
        print(json.dumps(trial.report, separators=(",", ":")), flush=True)
    return 0 if trial.report["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
