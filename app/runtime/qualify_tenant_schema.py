"""Bounded local PostgreSQL 16.13 schema qualification; no AWS/IAM acceptance.

Run with the existing locked runtime + contract validator environments. A unique
Docker database publishes only a random loopback port and is removed in finally.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import shutil
import tempfile
import threading
import time
import uuid

import psycopg
from psycopg import sql
import runtime
from tenant_seed import ORGANIZATIONS, make_bundles, seed, canonical

IMAGE = "postgres@sha256:5d143123fdf80462d1778cd4f24b9f7ca13c87174bca19141fb194c5a1ebca59"
MIGRATIONS = Path(__file__).resolve().parents[1] / 'migrations'
P = '00000000-0000-4000-8000-000000000066'
APP = '00000000-0000-4000-8000-000000000065'
WORK = '00000000-0000-4000-8000-000000000069'
PUB = '00000000-0000-4000-8000-000000000068'


def command(args, check=True):
    p = subprocess.run(args, capture_output=True, text=True, timeout=60)
    if check and p.returncode: raise RuntimeError('local Docker operation failed: '+p.stderr[-1200:])
    return p


class LocalPostgres:
    def __init__(self):
        self.name = 'genui-app008-schema-' + uuid.uuid4().hex[:12]
        self.port = None
        self.removed = False

    def __enter__(self):
        try:
            command(['docker','run','--detach','--rm','--name',self.name,'--publish','127.0.0.1::5432',
                     '--memory=512m','--cpus=1','--tmpfs=/var/lib/postgresql/data:rw,noexec,nosuid,size=256m',
                     '--env=POSTGRES_HOST_AUTH_METHOD=trust',IMAGE])
            address = command(['docker','port',self.name,'5432/tcp']).stdout.strip()
            assert address.startswith('127.0.0.1:') and '\n' not in address
            self.port = int(address.split(':')[1])
            for _ in range(60):
                if command(['docker','exec',self.name,'pg_isready','--host=127.0.0.1','--username=postgres'], False).returncode == 0: break
                time.sleep(.25)
            else: raise RuntimeError('local PostgreSQL startup deadline')
            with self.connect('postgres','postgres',autocommit=True) as c:
                assert c.execute('SHOW server_version').fetchone()[0].split()[0] == '16.13'
                c.execute('CREATE ROLE genui_migration LOGIN INHERIT CREATEDB CREATEROLE NOSUPERUSER NOBYPASSRLS')
                c.execute('CREATE ROLE rds_iam NOLOGIN')
                c.execute('GRANT rds_iam TO genui_migration WITH ADMIN TRUE')
                c.execute('CREATE DATABASE genui OWNER genui_migration')
            return self
        except BaseException:
            self.__exit__(None,None,None)
            raise

    def __exit__(self,*_):
        self.removed = command(['docker','rm','--force','--volumes',self.name],False).returncode == 0

    def connect(self, role='genui_migration', database='genui', autocommit=False):
        return psycopg.connect(host='127.0.0.1',port=self.port,dbname=database,user=role,sslmode='disable',
                              connect_timeout=3,autocommit=autocommit,
                              options='-c statement_timeout=5000 -c lock_timeout=3000 -c idle_in_transaction_session_timeout=10000')

    def digest(self):
        raw = command(['docker','exec',self.name,'pg_dump','--username=postgres','--dbname=genui','--no-owner','--no-comments']).stdout
        raw = '\n'.join(x for x in raw.splitlines() if not x.startswith(('\\restrict ','\\unrestrict ')))
        return hashlib.sha256(raw.encode()).hexdigest()


def scope(c, org=ORGANIZATIONS[0], principal=P, epoch=1):
    c.execute("SELECT set_config('genui.organization_id',%s,true),set_config('genui.principal_id',%s,true),set_config('genui.membership_epoch',%s,true)", (org,principal,str(epoch)))


def qualify():
    report={'schema_version':1,'scope':'APP-008 real local PostgreSQL schema/ACL qualification',
            'managed_acceptance':False,'image':IMAGE,'server_version':'16.13','network':'random loopback-only published port',
            'checks':[],'passed':False,'limits':['Local trust authentication is not RDS IAM/TLS qualification.',
            'Operator-seeded contract shapes are not source import, hosted identity, admission or successful application business writes.',
            'Native pool cancellation/reuse and actual managed trials remain separate.']}
    def passed(name, **facts): report['checks'].append({'name':name,'passed':True,**facts})
    catalog_query=json.loads((MIGRATIONS/'catalog-query.json').read_text())['sql']
    # Version, relation and pinned-file expectations follow the manifest and its
    # latest reviewed catalog; never a constant that a new migration must edit.
    manifest=runtime.migration_manifest(MIGRATIONS)
    sources=runtime.migration_sources(MIGRATIONS)
    latest=len(sources)
    tenant_tables=sorted(row[1] for row in runtime.catalog_resources(MIGRATIONS)[1][latest]['relations']
                         if row[0]=='genui' and row[2] in ('r','p'))
    def drift_refused(pg, name, statements):
        # These mutations are visible to the actual runner in one transaction.
        # A Python Refused must leave that drift untouched; only the test rolls
        # it back afterwards. Committed reproductions are checked separately.
        with pg.connect() as c:
            for statement in statements: c.execute(statement)
            c.execute('RESET ROLE; SET LOCAL search_path=pg_catalog')
            before=c.execute(catalog_query).fetchall()
            version=c.execute('SELECT * FROM genui_meta.schema_version').fetchall()
            try: runtime.apply_migrations(c,MIGRATIONS)
            except runtime.Refused: pass
            else: raise AssertionError('catalog drift accepted: '+name)
            assert c.execute(catalog_query).fetchall()==before
            assert c.execute('SELECT * FROM genui_meta.schema_version').fetchall()==version
            c.rollback()
        passed(name+' refuses without repairing drift')
    report['committed_drift_containers_removed']=[]
    for label, statement, legacy in (
        ('missing real table', 'SET LOCAL ROLE genui_owner; DROP TABLE genui.organization_permissions', False),
        ('disabled tenant RLS', 'SET LOCAL ROLE genui_owner; ALTER TABLE genui.organizations DISABLE ROW LEVEL SECURITY', False),
        ('extra application UPDATE', 'SET LOCAL ROLE genui_owner; GRANT UPDATE ON genui.publications TO genui_workspace', False),
        ('preexisting history1 with role drift', 'ALTER ROLE genui_workspace INHERIT', True),
    ):
        drift=LocalPostgres()
        try:
            with drift:
                with drift.connect() as c:
                    if legacy:
                        c.execute((MIGRATIONS/'0001_role_foundation.sql').read_text())
                        runtime.initialize_migration_history(c,runtime.INITIAL_MIGRATION_SHA256)
                    else: runtime.apply_migrations(c,MIGRATIONS)
                    c.execute(statement)
                # Mutation is committed before opening the real migration call.
                before=drift.digest()
                with drift.connect() as c:
                    c.execute('SET LOCAL search_path=pg_catalog')
                    catalog_before=c.execute(catalog_query).fetchall()
                try:
                    with drift.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
                except runtime.Refused: pass
                else: raise AssertionError('committed drift accepted: '+label)
                assert drift.digest()==before
                with drift.connect() as c:
                    c.execute('SET LOCAL search_path=pg_catalog')
                    assert c.execute(catalog_query).fetchall()==catalog_before
                passed('committed '+label+' refuses and preserves exact drifted state')
        finally: report['committed_drift_containers_removed'].append(drift.removed)
    replica=LocalPostgres()
    try:
        with replica:
            with replica.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            for parameter, value in (('application_name','unexpected-all-role-default'),('session_replication_role','replica')):
                with replica.connect('postgres') as c:
                    c.execute(sql.SQL('ALTER DATABASE genui SET {} TO {}').format(sql.Identifier(parameter),sql.Literal(value)))
                before=replica.digest()
                for role in ('genui_migration','genui_workspace'):
                    with replica.connect(role) as c:
                        expected_mode='replica' if parameter=='session_replication_role' else 'origin'
                        assert c.execute('SHOW session_replication_role').fetchone()==(expected_mode,)
                        c.execute('SET LOCAL search_path=pg_catalog');catalog_before=c.execute(catalog_query).fetchall()
                        try: runtime.verify_schema_catalog(c,MIGRATIONS)
                        except runtime.Refused: pass
                        else: raise AssertionError('all-role database default accepted: '+parameter)
                        assert c.execute('SHOW session_replication_role').fetchone()==(expected_mode,)
                        assert c.execute(catalog_query).fetchall()==catalog_before
                try:
                    with replica.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
                except runtime.Refused: pass
                else: raise AssertionError('migration accepted committed all-role default: '+parameter)
                assert replica.digest()==before
                with replica.connect('postgres') as c:
                    c.execute(sql.SQL('ALTER DATABASE genui RESET {}').format(sql.Identifier(parameter)))
                passed('committed all-role '+parameter+' default refuses on fresh app/operator connections without repair')
            with replica.connect('postgres') as c:
                c.execute('GRANT SET ON PARAMETER session_replication_role TO genui_workspace')
            before=replica.digest()
            with replica.connect('genui_workspace') as c:
                assert c.execute('SHOW session_replication_role').fetchone()==('origin',)
                assert c.execute("SELECT has_parameter_privilege(current_user,'session_replication_role','SET')").fetchone()==(True,)
                c.execute('SET LOCAL search_path=pg_catalog');catalog_before=c.execute(catalog_query).fetchall()
                try: runtime.verify_schema_catalog(c,MIGRATIONS)
                except runtime.Refused: pass
                else: raise AssertionError('application FK-bypass parameter authority accepted')
                assert c.execute(catalog_query).fetchall()==catalog_before
                assert c.execute("SELECT has_parameter_privilege(current_user,'session_replication_role','SET')").fetchone()==(True,)
            try:
                with replica.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            except runtime.Refused: pass
            else: raise AssertionError('migration accepted application FK-bypass parameter grant')
            assert replica.digest()==before
            with replica.connect('postgres') as c:
                c.execute('REVOKE SET ON PARAMETER session_replication_role FROM genui_workspace')
            passed('committed application replica-parameter SET grant refuses while session remains origin without repair')
            # Isolated operator-enabled session mutation, independently of the
            # DB default. The permission is fixture setup, not an application grant.
            with replica.connect('postgres') as c:
                c.execute('GRANT SET ON PARAMETER session_replication_role TO genui_migration')
            with replica.connect() as c:
                c.execute('SET LOCAL session_replication_role=replica')
                for verify in (lambda:runtime.verify_schema_catalog(c,MIGRATIONS),lambda:runtime.apply_migrations(c,MIGRATIONS)):
                    try: verify()
                    except runtime.Refused: pass
                    else: raise AssertionError('replica session accepted')
                    assert c.execute('SHOW session_replication_role').fetchone()==('replica',)
                c.rollback()
            passed('operator-enabled current replica session refuses both verifier and migration without SET repair')
    finally: report['replica_drift_container_removed']=replica.removed
    compatibility=LocalPostgres()
    try:
        with compatibility:
            with compatibility.connect() as c: c.execute((MIGRATIONS/'0001_role_foundation.sql').read_text())
            def remove_creator_bookkeeping():
                with compatibility.connect('postgres') as c:
                    rows=c.execute("""SELECT parent.rolname,grantor.rolname FROM pg_auth_members a
                      JOIN pg_roles parent ON parent.oid=a.roleid JOIN pg_roles member ON member.oid=a.member
                      JOIN pg_roles grantor ON grantor.oid=a.grantor
                      WHERE member.rolname='genui_migration' AND parent.rolname IN
                      ('genui_owner','genui_lock','genui_workspace','genui_control','genui_data','genui_worker')
                      AND a.admin_option AND NOT a.inherit_option AND NOT a.set_option""").fetchall()
                    assert rows
                    for role,grantor in rows:
                        c.execute(sql.SQL('REVOKE {} FROM genui_migration GRANTED BY {} CASCADE').format(sql.Identifier(role),sql.Identifier(grantor)))
                        # PostgreSQL may cascade the creator's dependent explicit
                        # grant. Retain precisely the separately required operator
                        # owner/helper access under the fixture administrator.
                        if role=='genui_owner': c.execute('GRANT genui_owner TO genui_migration WITH ADMIN FALSE, INHERIT TRUE, SET TRUE')
                        elif role=='genui_lock': c.execute('GRANT genui_lock TO genui_migration WITH ADMIN FALSE, INHERIT FALSE, SET TRUE')
            remove_creator_bookkeeping()
            with compatibility.connect() as c:
                runtime.verify_schema_catalog(c,MIGRATIONS,1)
                runtime.apply_migrations(c,MIGRATIONS)
            passed('managed-style absence of optional creator-only grants permits exact version1 verification and upgrade')
            remove_creator_bookkeeping()
            with compatibility.connect() as c:
                runtime.verify_schema_catalog(c,MIGRATIONS,latest)
                runtime.apply_migrations(c,MIGRATIONS)
            passed('managed-style absence of optional creator-only grants permits exact version'+str(latest)+' repeat')
            with compatibility.connect('postgres') as c:
                c.execute('REVOKE genui_owner FROM genui_migration CASCADE')
            with compatibility.connect() as c:
                try: runtime.verify_schema_catalog(c,MIGRATIONS,latest)
                except runtime.Refused: pass
                else: raise AssertionError('required explicit migration-owner grant was normalized away')
            passed('required explicit migration-owner access remains exact and cannot be normalized away')
    finally: report['compatibility_container_removed']=compatibility.removed
    shadow=LocalPostgres()
    try:
        with shadow:
            with shadow.connect() as c:
                c.execute((MIGRATIONS/'0001_role_foundation.sql').read_text())
                c.execute("""CREATE SCHEMA app008_untrusted;
                  CREATE SEQUENCE app008_untrusted.marker;
                  CREATE FUNCTION app008_untrusted.current_database() RETURNS name LANGUAGE plpgsql AS
                  $$ BEGIN PERFORM pg_catalog.nextval('app008_untrusted.marker'); RETURN 'genui'::name; END $$;
                  CREATE FUNCTION app008_untrusted.to_regclass(text) RETURNS regclass LANGUAGE plpgsql AS
                  $$ BEGIN PERFORM pg_catalog.nextval('app008_untrusted.marker'); RETURN pg_catalog.to_regclass($1); END $$;
                  CREATE FUNCTION app008_untrusted.pg_advisory_xact_lock(integer,integer) RETURNS void LANGUAGE plpgsql AS
                  $$ BEGIN PERFORM pg_catalog.nextval('app008_untrusted.marker'); END $$;
                  CREATE FUNCTION app008_untrusted.version_marker() RETURNS integer LANGUAGE plpgsql AS
                  $$ BEGIN PERFORM pg_catalog.nextval('app008_untrusted.marker'); RETURN 1; END $$""")
            for version in (1,latest):
                with shadow.connect('postgres') as c:
                    c.execute("ALTER DATABASE genui SET search_path=app008_untrusted,pg_catalog")
                with shadow.connect() as c:
                    assert c.execute('SHOW search_path').fetchone()==('app008_untrusted, pg_catalog',)
                    try: runtime.apply_migrations(c,MIGRATIONS)
                    except runtime.Refused: pass
                    else: raise AssertionError('untrusted database search_path accepted')
                    assert c.execute('SELECT is_called FROM app008_untrusted.marker').fetchone()==(False,)
                    assert c.execute('SHOW search_path').fetchone()==('pg_catalog, pg_temp',)
                with shadow.connect('postgres') as c:
                    assert c.execute("SELECT setconfig FROM pg_db_role_setting WHERE setrole=0 AND setdatabase=(SELECT oid FROM pg_database WHERE datname='genui')").fetchone()==(['search_path=app008_untrusted, pg_catalog'],)
                    c.execute('ALTER DATABASE genui RESET search_path')
                passed('version'+str(version)+' migration refuses unsafe database search_path without executing shadow functions or repairing default')
                if version==1:
                    with shadow.connect() as c:
                        c.execute('DROP TABLE genui_meta.schema_version')
                        c.execute('CREATE VIEW genui_meta.schema_version AS SELECT true AS singleton,app008_untrusted.version_marker() AS version')
                        try: runtime.apply_migrations(c,MIGRATIONS)
                        except runtime.Refused: pass
                        else: raise AssertionError('shadow version view accepted before catalog proof')
                        assert c.execute('SELECT is_called FROM app008_untrusted.marker').fetchone()==(False,)
                        c.rollback()
                    passed('legacy version view refuses before reading its marker expression')
                    with shadow.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            with shadow.connect() as c:
                c.execute("""CREATE TEMP SEQUENCE review_marker;
                  CREATE FUNCTION pg_temp.review_marker() RETURNS boolean LANGUAGE plpgsql VOLATILE AS
                  $$ BEGIN PERFORM pg_catalog.nextval('pg_temp.review_marker'); RETURN true; END $$;
                  CREATE TEMP VIEW pg_namespace AS SELECT n.* FROM pg_catalog.pg_namespace n
                    WHERE pg_temp.review_marker()""")
                assert c.execute('SELECT is_called FROM pg_temp.review_marker').fetchone()==(False,)
                # Positive control: omitted pg_temp really does take precedence
                # for catalog relation names and executes the hostile expression.
                c.execute('SET LOCAL search_path=pg_catalog')
                c.execute(catalog_query).fetchall()
                assert c.execute('SELECT is_called FROM pg_temp.review_marker').fetchone()==(True,)
                c.execute("SELECT pg_catalog.setval('pg_temp.review_marker',1,false)")
                runtime.verify_schema_catalog(c,MIGRATIONS)
                assert c.execute('SHOW search_path').fetchone()==('pg_catalog, pg_temp',)
                assert c.execute('SELECT is_called FROM pg_temp.review_marker').fetchone()==(False,)
                # Restart from the vulnerable path to prove apply_migrations
                # fixes its own first metadata lookup, not just the verifier's.
                c.execute('SET LOCAL search_path=pg_catalog')
                runtime.apply_migrations(c,MIGRATIONS)
                assert c.execute('SELECT is_called FROM pg_temp.review_marker').fetchone()==(False,)
                assert c.execute('SHOW search_path').fetchone()==('pg_catalog, pg_temp',)
                c.rollback()
            passed('positive-control temporary catalog view executes only under implicit temp-first path; verifier and repeat leave zero marker')
            with shadow.connect() as c:
                c.execute("""CREATE SEQUENCE app008_untrusted.probe_marker;
                  CREATE FUNCTION app008_untrusted.probe_version_marker() RETURNS integer
                  LANGUAGE plpgsql VOLATILE SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS
                  $$ BEGIN PERFORM pg_catalog.nextval('app008_untrusted.probe_marker'); RETURN %d; END $$;
                  DROP TABLE genui_meta.schema_version;
                  CREATE VIEW genui_meta.schema_version AS SELECT true AS singleton,app008_untrusted.probe_version_marker() AS version;
                  GRANT SELECT ON genui_meta.schema_version TO genui_workspace""" % latest)
            with shadow.connect('genui_workspace') as c:
                # This is a real view expression, reachable by the exact probe
                # role. The read-write control proves the sequence marker fires.
                assert c.execute('SELECT version FROM genui_meta.schema_version WHERE singleton=true').fetchone()==(latest,)
            with shadow.connect() as c:
                assert c.execute('SELECT is_called FROM app008_untrusted.probe_marker').fetchone()==(True,)
                c.execute("SELECT pg_catalog.setval('app008_untrusted.probe_marker',1,false)")
            with shadow.connect('genui_workspace') as c:
                c.execute('SET TRANSACTION READ ONLY')
                try: c.execute('SELECT version FROM genui_meta.schema_version WHERE singleton=true').fetchone()
                except psycopg.Error as error: assert error.sqlstate=='25006'
                else: raise AssertionError('read-only control unexpectedly permitted marker nextval')
                c.rollback()
            with shadow.connect('genui_workspace') as c:
                c.execute('SET TRANSACTION READ ONLY')
                try: runtime.probe_database(c,'genui_workspace')
                except runtime.Refused: pass
                else: raise AssertionError('runtime probe accepted malicious version view')
                assert c.execute('SHOW transaction_read_only').fetchone()==('on',)
                assert c.execute('SHOW search_path').fetchone()==('pg_catalog, pg_temp',)
            with shadow.connect() as c:
                assert c.execute('SELECT is_called FROM app008_untrusted.probe_marker').fetchone()==(False,)
            passed('actual read-only workspace probe refuses malicious version view before evaluation; read-write and read-only controls prove reachability',read_only_control_sqlstate='25006')
    finally: report['shadow_path_container_removed']=shadow.removed
    fresh=LocalPostgres()
    try:
        with fresh:
            before=fresh.digest()
            with fresh.connect(autocommit=True) as c:
                try: runtime.apply_migrations(c,MIGRATIONS)
                except runtime.Refused: pass
                else: raise AssertionError('autocommit migration was accepted')
            assert fresh.digest()==before
            passed('autocommit migration refuses before changing fresh database')
            with fresh.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            before=fresh.digest()
            with fresh.connect() as c:
                runtime.apply_migrations(c,MIGRATIONS)
                assert c.execute('SELECT singleton,version FROM genui_meta.schema_version').fetchall()==[(True,latest)]
                assert c.execute('SELECT version,sha256 FROM genui_meta.schema_migrations ORDER BY version').fetchall()==[(v,h) for v,h,_ in runtime.migration_sources(MIGRATIONS)]
            assert fresh.digest()==before
            passed('fresh checksummed version'+str(latest)+' install and exact repeat')
    finally:
        report['fresh_container_removed']=fresh.removed
    pg=LocalPostgres()
    try:
        with pg:
            # Exact checksummed source can still have a SQL failure. Both fresh
            # install and upgrade must roll back roles, schema and history whole.
            with tempfile.TemporaryDirectory(prefix='genui-app008-fault-') as tmp:
                fault=Path(tmp)
                for file in MIGRATIONS.iterdir():
                    if file.is_file(): shutil.copyfile(file,fault/file.name)
                # Interrupt the last migration, after every earlier one ran.
                faulted=json.loads((fault/'manifest.json').read_text())
                target=fault/faulted['migrations'][-1]['file']
                target.write_text(target.read_text()+"\nDO $$ BEGIN RAISE EXCEPTION 'synthetic migration interruption' USING ERRCODE='P0001'; END $$;\n")
                faulted['migrations'][-1]['sha256']=hashlib.sha256(target.read_bytes()).hexdigest()
                (fault/'manifest.json').write_text(json.dumps(faulted))
                for initial in ('fresh','version1'):
                    before=pg.digest()
                    try:
                        with pg.connect() as c: runtime.apply_migrations(c,fault)
                    except psycopg.Error as e: assert e.sqlstate=='P0001'
                    else: raise AssertionError('interrupted migration passed')
                    assert pg.digest()==before
                    with pg.connect('postgres') as c:
                        assert c.execute("SELECT count(*) FROM pg_roles WHERE rolname='genui_lock'").fetchone()==(0,)
                    passed(initial+' migration interruption rolls schema/history/lock-role back')
                    if initial=='fresh':
                        with pg.connect() as c: c.execute((MIGRATIONS/'0001_role_foundation.sql').read_text())
            for label, statements in (
                ('legacy metadata extra column',["SET LOCAL ROLE genui_owner", "ALTER TABLE genui_meta.schema_version ADD COLUMN unknown text"]),
                ('legacy metadata wrong constraint',["SET LOCAL ROLE genui_owner", "ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check"]),
                ('legacy metadata PUBLIC write',["SET LOCAL ROLE genui_owner", "GRANT UPDATE ON genui_meta.schema_version TO PUBLIC"]),
                ('legacy application inheritance drift',["ALTER ROLE genui_workspace INHERIT"]),
                ('legacy default privilege drift',["SET LOCAL ROLE genui_owner", "ALTER DEFAULT PRIVILEGES GRANT UPDATE ON TABLES TO genui_workspace"]),
            ): drift_refused(pg,label,statements)
            for inheritance in (False,True):
                with pg.connect() as c:
                    runtime.initialize_migration_history(c,runtime.INITIAL_MIGRATION_SHA256)
                    if inheritance: c.execute('ALTER ROLE genui_workspace INHERIT')
                    c.execute('SET LOCAL search_path=pg_catalog')
                    before=c.execute(catalog_query).fetchall()
                    try: runtime.apply_migrations(c,MIGRATIONS)
                    except runtime.Refused: pass
                    else: raise AssertionError('preexisting history1 was accepted')
                    assert c.execute(catalog_query).fetchall()==before
                    assert c.execute('SELECT version FROM genui_meta.schema_version').fetchone()==(1,)
                    c.rollback()
                passed('preexisting history1'+(' plus role drift' if inheritance else '')+' refuses without bootstrap bypass')
            with pg.connect() as c: c.execute('CREATE TABLE genui_meta.unexpected_partial_state(n integer)')
            before=pg.digest()
            try:
                with pg.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            except runtime.Refused: pass
            else: raise AssertionError('partial version1 foundation accepted')
            assert pg.digest()==before
            with pg.connect() as c: c.execute('DROP TABLE genui_meta.unexpected_partial_state')
            passed('partial version1 state refuses without inventing migration history')
            with pg.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            passed('transactional verified version1-to-version'+str(latest)+' migration')
            before=pg.digest()
            with pg.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            assert pg.digest()==before
            passed('checksummed repeat has identical schema/data dump')
            with pg.connect() as c: c.execute("UPDATE genui_meta.schema_migrations SET sha256=repeat('0',64) WHERE version=%s",(latest,))
            before=pg.digest()
            try:
                with pg.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            except runtime.Refused: pass
            else: raise AssertionError('altered recorded checksum accepted')
            assert pg.digest()==before
            with pg.connect() as c:
                c.execute('UPDATE genui_meta.schema_migrations SET sha256=%s WHERE version=%s',(sources[-1][1],latest))
            passed('altered recorded checksum refuses without altering database')
            for label, statements in (
                ('missing real table',["SET LOCAL ROLE genui_owner", "DROP TABLE genui.organization_permissions"]),
                ('missing real column',["SET LOCAL ROLE genui_owner", "ALTER TABLE genui.organizations DROP COLUMN display_name"]),
                ('changed column default',["SET LOCAL ROLE genui_owner", "ALTER TABLE genui.organizations ALTER COLUMN status SET DEFAULT 'active'"]),
                ('missing check constraint',["SET LOCAL ROLE genui_owner", "ALTER TABLE genui.organizations DROP CONSTRAINT organizations_status_check"]),
                ('extra index',["SET LOCAL ROLE genui_owner", "CREATE INDEX unexpected_status_idx ON genui.organizations(status)"]),
                ('disabled RLS',["SET LOCAL ROLE genui_owner", "ALTER TABLE genui.organizations DISABLE ROW LEVEL SECURITY"]),
                ('disabled FORCE RLS',["SET LOCAL ROLE genui_owner", "ALTER TABLE genui.organizations NO FORCE ROW LEVEL SECURITY"]),
                ('permissive tenant policy',["SET LOCAL ROLE genui_owner", "ALTER POLICY tenant_scope ON genui.organizations USING (true)"]),
                ('changed table owner',["ALTER TABLE genui.organizations OWNER TO genui_migration"]),
                ('extra workspace UPDATE',["SET LOCAL ROLE genui_owner", "GRANT UPDATE ON genui.publications TO genui_workspace"]),
                ('extra workspace column UPDATE',["SET LOCAL ROLE genui_owner", "GRANT UPDATE(display_name) ON genui.organizations TO genui_workspace"]),
                ('PUBLIC business read',["SET LOCAL ROLE genui_owner", "GRANT SELECT ON genui.internal_records TO PUBLIC"]),
                ('application inheritance drift',["ALTER ROLE genui_workspace INHERIT"]),
                ('application owner membership',["GRANT genui_owner TO genui_workspace"]),
                ('noncreator operator grant flags',["GRANT genui_workspace TO genui_migration WITH ADMIN FALSE, INHERIT FALSE, SET TRUE"]),
                ('unknown incoming owner member',["CREATE ROLE unexpected_catalog_incoming NOLOGIN", "GRANT genui_owner TO unexpected_catalog_incoming"]),
                ('transitive IAM parent membership',["CREATE ROLE unexpected_catalog_group NOLOGIN", "GRANT unexpected_catalog_group TO rds_iam"]),
                ('unexpected incoming helper membership',["GRANT genui_lock TO genui_data"]),
                ('application database CREATE',["GRANT CREATE ON DATABASE genui TO genui_workspace"]),
                ('helper schema CREATE',["SET LOCAL ROLE genui_owner", "GRANT CREATE ON SCHEMA genui_private TO genui_lock"]),
                ('PUBLIC function execute',["SET LOCAL ROLE genui_lock", "GRANT EXECUTE ON FUNCTION genui_private.lock_member_authority(uuid,uuid,bigint) TO PUBLIC"]),
                ('unsafe helper search path',["SET LOCAL ROLE genui_lock", "ALTER FUNCTION genui_private.lock_member_authority(uuid,uuid,bigint) SET search_path=public,genui"]),
                ('changed helper body',["SET LOCAL ROLE genui_owner", "GRANT CREATE ON SCHEMA genui_private TO genui_lock", "SET LOCAL ROLE genui_lock", "CREATE OR REPLACE FUNCTION genui_private.lock_member_authority(p_org uuid,p_principal uuid,p_epoch bigint) RETURNS TABLE(organization_active boolean,member_active boolean,membership_epoch bigint) LANGUAGE sql SECURITY DEFINER AS 'SELECT true,true,1::bigint'", "SET LOCAL ROLE genui_owner", "REVOKE CREATE ON SCHEMA genui_private FROM genui_lock"]),
                ('role database setting',["ALTER ROLE genui_workspace IN DATABASE genui SET row_security=off"]),
                ('extra schema object',["SET LOCAL ROLE genui_owner", "CREATE VIEW genui.unexpected_view AS SELECT 1 AS n"]),
            ): drift_refused(pg,label,statements)
            with pg.connect('postgres') as c:
                c.execute('ALTER TABLE genui.work_permissions DISABLE TRIGGER ALL')
            before=pg.digest()
            with pg.connect() as c:
                c.execute('SET LOCAL search_path=pg_catalog');catalog_before=c.execute(catalog_query).fetchall()
            try:
                with pg.connect() as c: runtime.apply_migrations(c,MIGRATIONS)
            except runtime.Refused: pass
            else: raise AssertionError('disabled internal FK triggers accepted')
            assert pg.digest()==before
            with pg.connect() as c:
                c.execute('SET LOCAL search_path=pg_catalog');assert c.execute(catalog_query).fetchall()==catalog_before
            with pg.connect('postgres') as c:
                c.execute('ALTER TABLE genui.work_permissions ENABLE TRIGGER ALL')
            passed('committed disabled internal FK triggers refuse without repair')
            for role in ('genui_workspace','genui_control','genui_data','genui_worker'):
                with pg.connect(role) as c: runtime.verify_schema_catalog(c,MIGRATIONS)
            passed('one canonical complete catalog projection is readable by all four application roles')
            bundles=make_bundles()
            before=pg.digest()
            with pg.connect(autocommit=True) as c:
                try: seed(c,bundles)
                except ValueError: pass
                else: raise AssertionError('autocommit seed was accepted')
            assert pg.digest()==before
            passed('autocommit seed refuses before tenant writes')
            with pg.connect() as c: seed(c,bundles)
            before=pg.digest()
            with pg.connect() as c: seed(c,bundles)
            assert pg.digest()==before
            passed('validated operator seed idempotence',bundle_count=len(bundles),seed_sha256=hashlib.sha256(canonical(bundles)).hexdigest())

            with pg.connect('postgres') as c:
                tables=c.execute("SELECT tablename FROM pg_tables WHERE schemaname='genui' ORDER BY tablename").fetchall()
                assert len(tables)==len(tenant_tables) and sorted(table for (table,) in tables)==tenant_tables
                assert c.execute("""SELECT count(*) FROM pg_class t JOIN pg_namespace n ON n.oid=t.relnamespace
                  WHERE n.nspname='genui' AND t.relkind='r' AND
                  (NOT t.relrowsecurity OR NOT t.relforcerowsecurity OR pg_get_userbyid(t.relowner)<>'genui_owner')""").fetchone()==(0,)
                assert c.execute("""SELECT count(*) FROM pg_index i JOIN pg_class t ON t.oid=i.indrelid JOIN pg_namespace n ON n.oid=t.relnamespace
                  JOIN pg_attribute a ON a.attrelid=t.oid AND a.attnum=i.indkey[0]
                  WHERE n.nspname='genui' AND a.attname<>'organization_id'""").fetchone()==(0,)
                assert c.execute("""SELECT count(*) FROM pg_constraint k JOIN pg_class t ON t.oid=k.conrelid JOIN pg_namespace n ON n.oid=t.relnamespace
                  JOIN pg_attribute a ON a.attrelid=t.oid AND a.attnum=k.conkey[1]
                  JOIN pg_attribute b ON b.attrelid=k.confrelid AND b.attnum=k.confkey[1]
                  WHERE n.nspname='genui' AND k.contype='f' AND (a.attname<>'organization_id' OR b.attname<>'organization_id')""").fetchone()==(0,)
                assert c.execute("SELECT count(*) FROM pg_policies WHERE schemaname='genui' AND (qual IS NULL OR with_check IS NULL)").fetchone()==(0,)
                lock=c.execute("SELECT rolsuper,rolcreatedb,rolcreaterole,rolreplication,rolbypassrls,rolcanlogin,rolinherit FROM pg_roles WHERE rolname='genui_lock'").fetchone()
                assert lock==(False,)*7
                assert c.execute("SELECT count(*) FROM pg_auth_members WHERE member=(SELECT oid FROM pg_roles WHERE rolname='genui_lock')").fetchone()==(0,)
                assert c.execute("SELECT has_schema_privilege('genui_lock','genui_private','CREATE'),has_schema_privilege('genui_lock','genui','CREATE')").fetchone()==(False,False)
                functions=c.execute("""SELECT p.proname,pg_get_userbyid(p.proowner),p.prosecdef,p.proconfig
                  FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname='genui_private' ORDER BY p.proname""").fetchall()
                assert len(functions)==2 and all(r[1:] == ('genui_lock',True,['search_path=pg_catalog, genui, pg_temp']) for r in functions)
                assert c.execute("""SELECT count(*) FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace,
                  LATERAL aclexplode(coalesce(p.proacl,acldefault('f',p.proowner))) a
                  WHERE n.nspname='genui_private' AND a.grantee=0 AND a.privilege_type='EXECUTE'""").fetchone()==(0,)
                assert c.execute("SELECT bool_and(proisstrict) FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname='genui_private'").fetchone()==(True,)
                assert c.execute("""SELECT table_name,column_name FROM information_schema.column_privileges
                  WHERE grantee='genui_lock' AND privilege_type='UPDATE' ORDER BY table_name,column_name""").fetchall()==[('memberships','membership_epoch'),('organizations','status'),('saved_work','version')]
            passed('all real tenant relations have owner/FORCE-RLS/policies/tenant-leading indexes and FKs',table_count=len(tables))
            passed('lock definer is non-login non-inheriting minimal role with exact three UPDATE columns')

            def denied(name, role, statement, params=(), expected='42501', org=ORGANIZATIONS[0]):
                try:
                    with pg.connect(role) as c:
                        scope(c,org)
                        if role=='genui_migration': c.execute('SET LOCAL ROLE genui_owner')
                        c.execute(statement,params)
                except psycopg.Error as e:
                    assert e.sqlstate==expected,(name,e.sqlstate)
                    passed(name,sqlstate=e.sqlstate)
                else: raise AssertionError('expected refusal: '+name)

            for role in ('genui_workspace','genui_control','genui_data','genui_worker'):
                with pg.connect(role) as c:
                    assert c.execute('SELECT count(*) FROM genui.organizations').fetchone()==(0,)
                    for org in ORGANIZATIONS:
                        scope(c,org)
                        assert c.execute('SELECT organization_id::text FROM genui.organizations').fetchall()==[(org,)]
                        assert c.execute('SELECT count(*) FROM genui.organizations WHERE organization_id=%s',(ORGANIZATIONS[1-ORGANIZATIONS.index(org)],)).fetchone()==(0,)
                passed(role+' actual own-tenant positive and missing/foreign scope denial')
                accessible=[]
                with pg.connect(role) as c:
                    for (table,) in tables:
                        allowed=c.execute('SELECT has_table_privilege(current_user,%s,\'SELECT\')',('genui.'+table,)).fetchone()[0]
                        if not allowed: continue
                        accessible.append(table)
                        for org in ORGANIZATIONS:
                            scope(c,org)
                            assert c.execute(sql.SQL('SELECT DISTINCT organization_id::text FROM genui.{}').format(sql.Identifier(table))).fetchall()==[(org,)]
                        c.execute("SELECT set_config('genui.organization_id','',true)")
                        assert c.execute(sql.SQL('SELECT count(*) FROM genui.{}').format(sql.Identifier(table))).fetchone()==(0,)
                        assert c.execute('SELECT has_table_privilege(current_user,%s,\'INSERT,UPDATE,DELETE,TRUNCATE,REFERENCES,TRIGGER\')',('genui.'+table,)).fetchone()==(False,)
                passed(role+' RLS on every granted real relation',tables=accessible)
                for target in ('genui_owner','genui_lock','genui_migration'):
                    denied(role+' cannot SET ROLE '+target,role,'SET ROLE '+target)
                for table in ('organizations','memberships','publications','saved_work','internal_records','internal_record_revisions'):
                    denied(role+' no direct UPDATE '+table,role,'UPDATE genui.'+table+' SET organization_id=organization_id')
                for table in ('publications','internal_records','internal_record_revisions'):
                    denied(role+' no direct INSERT '+table,role,'INSERT INTO genui.'+table+' DEFAULT VALUES')
                denied(role+' cannot TRUNCATE publications',role,'TRUNCATE genui.publications')
                denied(role+' cannot bypass RLS',role,'SET LOCAL row_security=off; SELECT * FROM genui.organizations')
                denied(role+' malformed scope fails closed',role,"SELECT set_config('genui.organization_id','malformed',true); SELECT * FROM genui.organizations",expected='22P02')
                if role=='genui_worker':
                    denied('worker cannot execute authority fence',role,'SELECT * FROM genui_private.lock_member_authority(%s,%s,1)',(ORGANIZATIONS[0],P))
                else:
                    with pg.connect(role) as c:
                        scope(c)
                        assert c.execute('SELECT * FROM genui_private.lock_member_authority(%s,%s,1)',(ORGANIZATIONS[0],P)).fetchone()==(True,True,1)
                        assert c.execute('SELECT * FROM genui_private.lock_member_authority(%s,%s,2)',(ORGANIZATIONS[0],P)).fetchall()==[]
                    passed(role+' permitted bounded member fence, mismatched epoch refuses')
                if role!='genui_data':
                    denied(role+' cannot execute work fence',role,'SELECT * FROM genui_private.lock_work_revision(%s,%s,%s,%s,1)',(ORGANIZATIONS[0],APP,WORK,PUB))
            with pg.connect('genui_data') as c:
                scope(c)
                assert c.execute('SELECT publication_id::text,work_version FROM genui_private.lock_work_revision(%s,%s,%s,%s,1)',(ORGANIZATIONS[0],APP,WORK,PUB)).fetchone()==(PUB,1)
                assert c.execute('SELECT count(*) FROM genui.internal_records').fetchone()==(1,)
            passed('data has real record read and bounded work lock, no UPDATE authority')
            with pg.connect('genui_data') as c:
                scope(c)
                member_args=[ORGANIZATIONS[0],P,1]
                work_args=[ORGANIZATIONS[0],APP,WORK,PUB,1]
                for index in range(len(member_args)):
                    args=member_args.copy();args[index]=None
                    assert c.execute('SELECT * FROM genui_private.lock_member_authority(%s,%s,%s)',args).fetchall()==[]
                for index in range(len(work_args)):
                    args=work_args.copy();args[index]=None
                    assert c.execute('SELECT * FROM genui_private.lock_work_revision(%s,%s,%s,%s,%s)',args).fetchall()==[]
                for epoch in (0,9007199254740992):
                    scope(c,epoch=epoch)
                    assert c.execute('SELECT * FROM genui_private.lock_member_authority(%s,%s,%s)',(ORGANIZATIONS[0],P,epoch)).fetchall()==[]
                    assert c.execute('SELECT * FROM genui_private.lock_work_revision(%s,%s,%s,%s,1)',work_args[:4]).fetchall()==[]
                scope(c)
                for setting in ('genui.principal_id','genui.membership_epoch','genui.organization_id'):
                    c.execute('SELECT set_config(%s,\'\',true)',(setting,))
                    assert c.execute('SELECT * FROM genui_private.lock_work_revision(%s,%s,%s,%s,1)',work_args[:4]).fetchall()==[]
                    scope(c)
                assert c.execute('SELECT * FROM genui_private.lock_work_revision(%s,%s,%s,%s,2)',work_args[:4]).fetchall()==[]
                assert c.execute('SELECT * FROM genui_private.lock_work_revision(%s,%s,%s,%s,1)',(ORGANIZATIONS[0],APP,WORK,str(uuid.uuid4()))).fetchall()==[]
            passed('NULL or incomplete scope, unsafe epochs and stale work pin fail closed')
            for setting in ('genui.principal_id','genui.membership_epoch'):
                denied('malformed '+setting+' refuses work fence','genui_data',
                       'SELECT set_config(\''+setting+'\',\'malformed\',true); SELECT * FROM genui_private.lock_work_revision(\''+ORGANIZATIONS[0]+'\',\''+APP+'\',\''+WORK+'\',\''+PUB+'\',1)',expected='22P02')
            with pg.connect() as c:
                c.execute('SET LOCAL ROLE genui_owner');scope(c)
                assert c.execute('SELECT count(*) FROM genui.organizations').fetchone()==(1,)
                assert c.execute('SELECT count(*) FROM genui.organizations WHERE organization_id=%s',(ORGANIZATIONS[1],)).fetchone()==(0,)
            passed('actual NOLOGIN table owner remains subject to FORCE RLS')
            denied('real FK refuses foreign-only app','genui_migration',
                   "INSERT INTO genui.saved_work VALUES (%s,%s,%s,%s,1,'invalid')",
                   (ORGANIZATIONS[0],'00000000-0000-4000-8000-0000000000c9',str(uuid.uuid4()),'00000000-0000-4000-8000-0000000000cc'),expected='23503')
            denied('owner tenant policy refuses cross-organization write','genui_migration',
                   "INSERT INTO genui.spaces VALUES (%s,%s,'invalid')",(ORGANIZATIONS[1],str(uuid.uuid4())),expected='42501')
            denied('unknown permission check rejects even operator seed','genui_migration',
                   "INSERT INTO genui.app_permissions VALUES (%s,%s,%s,'unknown')",(ORGANIZATIONS[0],P,APP),expected='23514')
            denied('record binding refuses NULL revision policy','genui_migration',
                   "UPDATE genui.work_bindings SET revision_policy=NULL WHERE organization_id=%s AND kind='records'",(ORGANIZATIONS[0],),expected='23514')
            denied('source binding refuses changed normalized digest','genui_migration',
                   "UPDATE genui.work_bindings SET source_digest='sha256:'||repeat('f',64) WHERE organization_id=%s AND kind='source'",(ORGANIZATIONS[0],),expected='23503')
            denied('source dependency cannot name a different input binding','genui_migration',
                   "UPDATE genui.work_source_dependencies SET input_id='unbound_input' WHERE organization_id=%s",(ORGANIZATIONS[0],),expected='23503')
            denied('publication refuses wrong definition digest','genui_migration',
                   "UPDATE genui.publications SET definition_digest='sha256:'||repeat('f',64) WHERE organization_id=%s",(ORGANIZATIONS[0],),expected='23503')

            # Barrier is the observed pg_stat_activity lock wait, not a timing guess.
            def race(writer_first, organization=False):
                with pg.connect() as writer, pg.connect('genui_data') as reader, pg.connect('postgres',autocommit=True) as observer:
                    writer.execute('SET LOCAL ROLE genui_owner');scope(writer)
                    epoch=writer.execute('SELECT membership_epoch FROM genui.memberships WHERE organization_id=%s AND principal_id=%s',(ORGANIZATIONS[0],P)).fetchone()[0]
                    scope(reader,epoch=epoch)
                    errors=[]; result=[]; finished=threading.Event()
                    def revoke():
                        try:
                            if organization:
                                writer.execute('SELECT organization_id FROM genui.organizations WHERE organization_id=%s FOR UPDATE',(ORGANIZATIONS[0],))
                                writer.execute("UPDATE genui.organizations SET status='suspended' WHERE organization_id=%s",(ORGANIZATIONS[0],))
                            else:
                                writer.execute('SELECT organization_id FROM genui.organizations WHERE organization_id=%s FOR SHARE',(ORGANIZATIONS[0],))
                                writer.execute('SELECT principal_id FROM genui.memberships WHERE organization_id=%s AND principal_id=%s FOR UPDATE',(ORGANIZATIONS[0],P))
                                writer.execute("UPDATE genui.memberships SET status='revoked',membership_epoch=membership_epoch+1 WHERE organization_id=%s AND principal_id=%s",(ORGANIZATIONS[0],P))
                        except BaseException as e: errors.append(type(e).__name__)
                        finally: finished.set()
                    def read():
                        try: result.extend(reader.execute('SELECT * FROM genui_private.lock_member_authority(%s,%s,%s)',(ORGANIZATIONS[0],P,epoch)).fetchall())
                        except BaseException as e: errors.append(type(e).__name__)
                        finally: finished.set()
                    if writer_first:
                        revoke(); assert not errors;finished.clear();thread=threading.Thread(target=read);pid=reader.info.backend_pid
                    else:
                        read();assert result==[(True,True,epoch)];finished.clear();thread=threading.Thread(target=revoke);pid=writer.info.backend_pid
                    thread.start()
                    deadline=time.monotonic()+2
                    while time.monotonic()<deadline:
                        state=observer.execute('SELECT wait_event_type FROM pg_stat_activity WHERE pid=%s',(pid,)).fetchone()
                        if state==('Lock',): break
                        if finished.is_set(): raise AssertionError('fence did not block')
                        time.sleep(.01)
                    else: raise AssertionError('lock-wait observation deadline')
                    if writer_first: writer.commit()
                    else: reader.commit()
                    thread.join(4);assert not thread.is_alive() and not errors,errors
                    if writer_first: assert result==([(False,True,epoch)] if organization else [])
                    writer.commit();reader.commit()
                with pg.connect() as c:
                    c.execute('SET LOCAL ROLE genui_owner');scope(c)
                    c.execute("UPDATE genui.organizations SET status='active' WHERE organization_id=%s",(ORGANIZATIONS[0],))
                    c.execute("UPDATE genui.memberships SET status='active',membership_epoch=membership_epoch+1 WHERE organization_id=%s AND principal_id=%s",(ORGANIZATIONS[0],P))
                passed(('organization suspension' if organization else 'membership revocation')+(' writer-first' if writer_first else ' reader-first')+' actual lock ordering')
            for org in (False,True):
                for first in (False,True): race(first,org)
            def work_race(writer_first):
                with pg.connect() as writer, pg.connect('genui_data') as reader, pg.connect('postgres',autocommit=True) as observer:
                    writer.execute('SET LOCAL ROLE genui_owner');scope(writer)
                    epoch=writer.execute('SELECT membership_epoch FROM genui.memberships WHERE organization_id=%s AND principal_id=%s',(ORGANIZATIONS[0],P)).fetchone()[0]
                    version=writer.execute('SELECT version FROM genui.saved_work WHERE organization_id=%s AND app_id=%s AND work_id=%s',(ORGANIZATIONS[0],APP,WORK)).fetchone()[0]
                    scope(reader,epoch=epoch)
                    errors=[];result=[];finished=threading.Event()
                    def advance():
                        try:
                            writer.execute('SELECT organization_id FROM genui.organizations WHERE organization_id=%s FOR SHARE',(ORGANIZATIONS[0],))
                            writer.execute('SELECT principal_id FROM genui.memberships WHERE organization_id=%s AND principal_id=%s FOR SHARE',(ORGANIZATIONS[0],P))
                            writer.execute('SELECT work_id FROM genui.saved_work WHERE organization_id=%s AND app_id=%s AND work_id=%s FOR UPDATE',(ORGANIZATIONS[0],APP,WORK))
                            writer.execute('UPDATE genui.saved_work SET version=version+1 WHERE organization_id=%s AND app_id=%s AND work_id=%s',(ORGANIZATIONS[0],APP,WORK))
                        except BaseException as e: errors.append(type(e).__name__)
                        finally: finished.set()
                    def read_work():
                        try:
                            assert reader.execute('SELECT * FROM genui_private.lock_member_authority(%s,%s,%s)',(ORGANIZATIONS[0],P,epoch)).fetchone()==(True,True,epoch)
                            result.extend(reader.execute('SELECT publication_id::text,work_version FROM genui_private.lock_work_revision(%s,%s,%s,%s,%s)',(ORGANIZATIONS[0],APP,WORK,PUB,version)).fetchall())
                        except BaseException as e: errors.append(type(e).__name__)
                        finally: finished.set()
                    if writer_first:
                        advance();assert not errors;finished.clear();thread=threading.Thread(target=read_work);pid=reader.info.backend_pid
                    else:
                        read_work();assert result==[(PUB,version)];finished.clear();thread=threading.Thread(target=advance);pid=writer.info.backend_pid
                    thread.start();deadline=time.monotonic()+2
                    while time.monotonic()<deadline:
                        if observer.execute('SELECT wait_event_type FROM pg_stat_activity WHERE pid=%s',(pid,)).fetchone()==('Lock',): break
                        if finished.is_set(): raise AssertionError('work fence did not block')
                        time.sleep(.01)
                    else: raise AssertionError('work lock-wait observation deadline')
                    if writer_first: writer.commit()
                    else: reader.commit()
                    thread.join(4);assert not thread.is_alive() and not errors,errors
                    if writer_first: assert result==[]
                    writer.commit();reader.commit()
                passed('saved-work version advance '+('writer-first' if writer_first else 'reader-first')+' actual lock ordering')
            for first in (False,True): work_race(first)
            before=pg.digest()
            try:
                with pg.connect() as c: seed(c,bundles)
            except ValueError: pass
            else: raise AssertionError('seed reset an existing changed membership epoch')
            assert pg.digest()==before
            passed('operator seed refuses changed existing epoch without overwriting authority')
            report['source_sha256']={str(p.relative_to(Path(__file__).resolve().parents[2])):hashlib.sha256(p.read_bytes()).hexdigest()
                                    for p in (Path(runtime.__file__),Path(__file__),Path(__file__).with_name('tenant_seed.py'),MIGRATIONS/'manifest.json',
                                              *(MIGRATIONS/item['file'] for item in manifest['migrations']+manifest['catalog']))}
            report['passed']=True
    finally:
        report['container_removed']=pg.removed
    return report


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path);args=parser.parse_args()
    result=qualify(); rendered=json.dumps(result,indent=2)+'\n'
    if args.output: args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(rendered)
    print(rendered,end='')
