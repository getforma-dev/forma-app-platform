"""Disposable PG16.13/TLS qualification of the emitted APP-006 operator command."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(ROOT / 'app/runtime'))
from qualify_tenant_schema import LocalPostgres, IMAGE, MIGRATIONS
import runtime
import psycopg

SOURCE = ROOT / 'infra/scripts/auth_schema_preflight.py'
spec = importlib.util.spec_from_file_location('auth_schema_preflight', SOURCE)
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)


def command(args):
    p = subprocess.run(args, capture_output=True, text=True, timeout=30)
    if p.returncode:
        raise AssertionError('local fixture command failed')
    return p.stdout


def qualify():
    report = {'scope': 'APP-006 schema-one preflight local PostgreSQL/TLS qualification',
              'managed_acceptance': False, 'image': IMAGE, 'checks': [], 'passed': False}
    def passed(name):
        report['checks'].append({'name': name, 'passed': True})
    source_before = hashlib.sha256(SOURCE.read_bytes()).hexdigest()
    override = helper.emit_override()
    assert override == helper.emit_override() and len(override.encode()) <= 8192
    document = json.loads(override)
    entry = document['containerOverrides'][0]
    assert entry['name'] == 'migration' and entry['command'][0] == '-c'
    # Import the exact emitted command with only __name__ changed: no CLI/target
    # bypass is added to production. Local fixture connections are test-owned.
    emitted = {'__name__': 'local_emitted_preflight'}
    exec(entry['command'][1], emitted)
    query, expected = emitted['resources']()
    assert emitted['SOURCE_SHA256'] == source_before
    report['override_bytes'] = len(override.encode())
    report['override_sha256'] = hashlib.sha256(override.encode()).hexdigest()
    report['source_sha256'] = source_before
    report['resource_sha256'] = {name: hashlib.sha256((MIGRATIONS / name).read_bytes()).hexdigest()
                                for name in ['catalog-query.json', 'catalog-v1.json', '0001_role_foundation.sql']}
    passed('deterministic exact emitted override fits8192 and validates embedded SHA-bound resources')
    process = subprocess.run([sys.executable, *entry['command']], env={'PGPASSWORD': 'SYNTHETIC_NOT_FOR_OUTPUT'},
                             capture_output=True, text=True, timeout=35)
    assert process.returncode == 1 and process.stderr == '' and 'SYNTHETIC_NOT_FOR_OUTPUT' not in process.stdout
    assert json.loads(process.stdout) == {'scope': helper.SCOPE, 'passed': False, 'error': 'auth_schema_preflight_refused'}
    passed('actual python3 entryPoint plus emitted command fails closed with bounded sanitized JSON before network')
    original = emitted['EMBEDDED_RESOURCES']
    emitted['EMBEDDED_RESOURCES'] = (original[0] + ' ', original[1])
    try:
        emitted['resources']()
    except emitted['Refused']:
        pass
    else:
        raise AssertionError('modified resource accepted')
    emitted['EMBEDDED_RESOURCES'] = original
    passed('embedded query byte tampering refuses before connection')
    env = {'PGHOST': helper.HOST, 'PGPORT': '5432', 'PGDATABASE': 'genui', 'PGUSER': 'genui_migration',
           'PGSSLMODE': 'verify-full', 'PGSSLROOTCERT': helper.CA, 'PGPASSWORD': 'local-synthetic-only'}
    with patch.dict('os.environ', env, clear=True):
        args = emitted['connection_arguments']()
        assert args['options'] == helper.OPTIONS and args['sslmode'] == 'verify-full'
        assert 'PGPASSWORD' not in emitted['os'].environ
    passed('production connection fixes identity target TLS safe startup bounds and consumes secret env')
    for key, value in [('PGHOST', 'localhost'), ('PGPORT', '5433'), ('PGUSER', 'postgres'),
                       ('PGDATABASE', 'postgres'), ('PGSSLMODE', 'disable'), ('PGSSLROOTCERT', '/tmp/ca'),
                       ('PGHOSTADDR', '127.0.0.1'), ('PGSERVICE', 'alternate'), ('PGOPTIONS', '-c search_path=public')]:
        with patch.dict('os.environ', {**env, key: value}, clear=True):
            try:
                emitted['connection_arguments']()
            except emitted['Refused']:
                pass
            else:
                raise AssertionError('alternate connection channel accepted')
        passed('production refuses alternate ' + key)
    pg = LocalPostgres()
    with tempfile.TemporaryDirectory(prefix='genui-app006-preflight-') as private:
        private = Path(private)
        cert, key = private / 'server.pem', private / 'server.key'
        command(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1',
                 '-subj', '/CN=localhost', '-addext', 'subjectAltName=IP:127.0.0.1,DNS:localhost',
                 '-keyout', str(key), '-out', str(cert)])
        try:
            with pg:
                for path in (cert, key):
                    command(['docker', 'cp', str(path), pg.name + ':/var/lib/postgresql/' + path.name])
                command(['docker', 'exec', '--user', 'root', pg.name, 'chown', 'postgres:postgres',
                         '/var/lib/postgresql/server.pem', '/var/lib/postgresql/server.key'])
                command(['docker', 'exec', '--user', 'root', pg.name, 'chmod', '600', '/var/lib/postgresql/server.key'])
                with pg.connect('postgres', autocommit=True) as c:
                    for setting in ["ssl='on'", "ssl_cert_file='/var/lib/postgresql/server.pem'", "ssl_key_file='/var/lib/postgresql/server.key'"]:
                        c.execute('ALTER SYSTEM SET ' + setting)
                    c.execute('SELECT pg_reload_conf()')
                with pg.connect() as c:
                    c.execute((MIGRATIONS / '0001_role_foundation.sql').read_text())
                def connect():
                    return psycopg.connect(host='127.0.0.1', port=pg.port, dbname='genui', user='genui_migration',
                                           sslmode='verify-full', sslrootcert=str(cert), connect_timeout=3,
                                           options=helper.OPTIONS)
                def refuse():
                    with connect() as c:
                        try:
                            emitted['collect'](c)
                        except emitted['Refused']:
                            c.rollback()
                            return
                        raise AssertionError('drift accepted')
                before = pg.digest()
                with connect() as c:
                    result = emitted['collect'](c)
                    assert result['passed'] and result['database_version'] == 1
                    assert result['assertions']['tls'] and result['assertions']['read_only']
                    try:
                        c.execute('UPDATE genui_meta.schema_version SET version=1')
                    except psycopg.errors.ReadOnlySqlTransaction:
                        c.rollback()
                    else:
                        raise AssertionError('read-only operation wrote state')
                assert pg.digest() == before
                report['local_success_receipt'] = result
                passed('exact emitted collector succeeds on actual PG16.13 verify-full TLS version1 without state changes')
                passed('actual operator UPDATE is denied SQLSTATE25006 in the preflight transaction')
                with pg.connect() as c:
                    c.execute('ALTER ROLE genui_workspace INHERIT')
                drifted = pg.digest()
                refuse()
                assert pg.digest() == drifted
                with pg.connect() as c:
                    assert c.execute("SELECT rolinherit FROM pg_catalog.pg_roles WHERE rolname='genui_workspace'").fetchone() == (True,)
                    c.execute('ALTER ROLE genui_workspace NOINHERIT')
                passed('catalog role drift refuses and leaves the observed changed role untouched')
                # An operator-only temp view can shadow unqualified catalog
                # relations when pg_temp is omitted from search_path.
                with connect() as c:
                    c.execute('SET default_transaction_read_only=off'); c.commit()
                    c.execute("""CREATE TEMP SEQUENCE marker;
                      CREATE FUNCTION pg_temp.preflight_marker() RETURNS boolean LANGUAGE plpgsql VOLATILE AS
                      $$ BEGIN PERFORM pg_catalog.nextval('pg_temp.marker'); RETURN true; END $$;
                      CREATE TEMP VIEW pg_namespace AS SELECT n.* FROM pg_catalog.pg_namespace n WHERE pg_temp.preflight_marker()""")
                    c.execute('SET search_path=pg_catalog')
                    c.execute(query['sql']).fetchall()
                    assert c.execute('SELECT is_called FROM pg_temp.marker').fetchone() == (True,)
                    c.execute("SELECT pg_catalog.setval('pg_temp.marker',1,false)")
                    c.execute("SELECT pg_catalog.set_config('search_path','pg_catalog,pg_temp',false)")
                    c.execute('SET default_transaction_read_only=on'); c.commit()
                    assert emitted['collect'](c)['passed']
                    assert c.execute('SELECT is_called FROM pg_temp.marker').fetchone() == (False,)
                    c.rollback()
                passed('temp catalog shadow positive control fires; exact preflight path never evaluates it')
                with pg.connect() as c:
                    c.execute("""CREATE SCHEMA app006_probe;
                      CREATE SEQUENCE app006_probe.marker;
                      CREATE FUNCTION app006_probe.version_marker() RETURNS integer LANGUAGE plpgsql VOLATILE AS
                      $$ BEGIN PERFORM pg_catalog.nextval('app006_probe.marker'); RETURN 1; END $$;
                      DROP TABLE genui_meta.schema_version;
                      CREATE VIEW genui_meta.schema_version AS SELECT true AS singleton,app006_probe.version_marker() AS version""")
                    assert c.execute('SELECT version FROM genui_meta.schema_version').fetchone() == (1,)
                    assert c.execute('SELECT is_called FROM app006_probe.marker').fetchone() == (True,)
                    c.execute("SELECT pg_catalog.setval('app006_probe.marker',1,false)")
                with connect() as c:
                    try:
                        c.execute('SELECT version FROM genui_meta.schema_version').fetchone()
                    except psycopg.errors.ReadOnlySqlTransaction:
                        c.rollback()
                    else:
                        raise AssertionError('metadata expression positive control was not reached')
                drifted = pg.digest()
                refuse()
                assert pg.digest() == drifted
                with pg.connect() as c:
                    assert c.execute('SELECT is_called FROM app006_probe.marker').fetchone() == (False,)
                passed('metadata view RW control fires and RO control reaches25006; collector refuses catalog before expression')
                with pg.connect('postgres') as c:
                    c.execute('ALTER DATABASE genui SET session_replication_role=replica')
                with connect() as c:
                    assert c.execute('SHOW session_replication_role').fetchone() == ('replica',)
                    c.rollback()
                refuse()
                with pg.connect('postgres') as c:
                    assert c.execute("SELECT setconfig FROM pg_catalog.pg_db_role_setting WHERE setrole=0 AND setdatabase=(SELECT oid FROM pg_catalog.pg_database WHERE datname='genui')").fetchone() == (['session_replication_role=replica'],)
                passed('committed all-role replica default refuses immediately and is not silently repaired')
                with pg.connect('postgres') as c:
                    c.execute('ALTER DATABASE genui RESET session_replication_role')
                with pg.connect() as c:
                    c.execute('DROP VIEW genui_meta.schema_version; DROP SCHEMA app006_probe CASCADE')
                    c.execute((MIGRATIONS / '0001_role_foundation.sql').read_text())
                    runtime.apply_migrations(c, MIGRATIONS)
                drifted = pg.digest()
                refuse()
                assert pg.digest() == drifted
                with pg.connect() as c:
                    assert c.execute('SELECT version FROM genui_meta.schema_version').fetchone() == (2,)
                passed('real schema2 installation refuses and remains unchanged; no migration or downgrade path')
        finally:
            report['container_removed'] = pg.removed
    assert pg.removed and source_before == hashlib.sha256(SOURCE.read_bytes()).hexdigest()
    report['passed'] = True
    report['limits'] = ['Disposable local operator/trust authentication with verified TLS is not actual RDS identity or IAM acceptance.',
                        'Production endpoint/UID/env guards are checked locally; production run was not connected to AWS.',
                        'No cloud, hosted identity, migration, shared database reset or secret read occurred.']
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = qualify()
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'passed': result['passed'], 'checks': len(result['checks']),
                      'override_bytes': result['override_bytes'], 'container_removed': result['container_removed']}))
