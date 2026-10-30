"""Unit-only failure-path checks. No AWS/RDS calls or managed acceptance."""
import copy
import contextlib
import hashlib
import io
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import signal
import shutil
import stat
import subprocess
import sys
import threading
import time
import types
import unittest
from unittest.mock import patch
import uuid
import runtime

MIGRATIONS = Path(runtime.__file__).resolve().parent.parent / 'migrations'
LEDGER_READ = 'SELECT version,sha256 FROM genui_meta.schema_migrations ORDER BY version'
VERSION_READ = 'SELECT singleton,version FROM genui_meta.schema_version'
WRITES = ('INSERT ', 'UPDATE ', 'SET LOCAL ROLE ')


def synthetic_versions(target, latest):
    """Reviewed versions 1-2 plus tiny unit-only versions 3..latest.

    Only the reviewed 0001/0002 files, catalog-query.json and catalog-v1/v2 are
    copied, whatever later reviewed migrations exist, so the runner bookkeeping
    is exercised on a fixed shape. Each synthetic migration is a comment and a
    SELECT; each synthetic catalog adds one marker relation row and keeps the
    reviewed sections. Nothing here is schema SQL or reaches PostgreSQL.
    """
    target = Path(target)
    manifest = json.loads((MIGRATIONS / 'manifest.json').read_text())
    manifest['migrations'] = manifest['migrations'][:2]
    manifest['catalog'] = manifest['catalog'][:3]
    for name in ('catalog-query.json', 'catalog-v1.json', 'catalog-v2.json', *(item['file'] for item in manifest['migrations'])):
        shutil.copyfile(MIGRATIONS / name, target / name)
    catalog = json.loads((target / 'catalog-v2.json').read_text())
    for version in range(3, latest + 1):
        name = f'{version:04d}_unit_only_marker.sql'
        source = f'-- unit-only synthetic migration {version}; never reaches PostgreSQL\nSELECT {version};\n'.encode()
        (target / name).write_bytes(source)
        catalog = copy.deepcopy(catalog)
        catalog['relations'].append(['genui', f'unit_only_marker_{version}', 'r', 'genui_owner', True, True, 'p', False, None])
        raw = json.dumps(catalog, sort_keys=True).encode()
        (target / f'catalog-v{version}.json').write_bytes(raw)
        manifest['migrations'].append({'version': version, 'file': name, 'sha256': hashlib.sha256(source).hexdigest()})
        manifest['catalog'].append({'file': f'catalog-v{version}.json', 'sha256': hashlib.sha256(raw).hexdigest()})
    manifest['database_version'] = latest
    (target / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    return target


class MigrationRecorder:
    """Unit-only stand-in for one migration transaction; never a database.

    It answers the runner's fixed statements from modeled facts: the reviewed
    catalog the projection shows, the ledger rows and the version row. Running
    reviewed migration k's exact text makes the projection show catalog-vk;
    `drift` replaces what the projection shows at a version. Any other
    statement fails the test.
    """
    def __init__(self, directory, version, ledger=True, created=True):
        directory = Path(directory)
        manifest = json.loads((directory / 'manifest.json').read_text())
        self.query = json.loads((directory / 'catalog-query.json').read_text())['sql']
        self.catalogs = {v: (directory / f'catalog-v{v}.json').read_text() for v in range(1, manifest['database_version'] + 1)}
        self.migrations = {(directory / item['file']).read_bytes().decode('utf-8'): item['version'] for item in manifest['migrations']}
        self.digests = {item['version']: item['sha256'] for item in manifest['migrations']}
        self.autocommit = False
        self.created, self.ledger = created, ledger
        self.shown = self.version = version if created else None
        self.history = [(v, self.digests[v]) for v in range(1, version + 1)] if ledger else []
        self.drift = {}
        self.statements = []
        self.rows = []

    def applied(self):
        return [self.migrations[s] for s in self.statements if s in self.migrations]

    def writes(self):
        return [s for s in self.statements if s in self.migrations or s.startswith(WRITES)]

    def execute(self, statement, params=None):
        self.statements.append(statement)
        fixed = {
            'SHOW session_replication_role': [('origin',)],
            'SET LOCAL search_path=pg_catalog, pg_temp': [],
            'SELECT session_user::text,current_user::text,current_database()': [('genui_migration', 'genui_migration', 'genui')],
            'SELECT pg_advisory_xact_lock(1819409017, 2)': [('',)],
            "SELECT to_regclass('genui_meta.schema_version') IS NOT NULL": [(self.created,)],
            "SELECT to_regclass('genui_meta.schema_migrations') IS NOT NULL": [(self.ledger,)],
            "SELECT count(*) FROM pg_namespace WHERE nspname IN ('genui_meta','genui','genui_private')": [(3 if self.created else 0,)],
            LEDGER_READ: list(self.history),
            VERSION_READ: [(True, self.version)],
            'SELECT version FROM genui_meta.schema_version': [(self.version,)],
        }
        if statement in fixed: self.rows = fixed[statement]
        elif statement == self.query: self.rows = [(self.drift.get(self.shown, self.catalogs.get(self.shown, '{}')),)]
        elif statement in self.migrations:
            self.shown = self.migrations[statement]
            if self.shown == 1: self.created, self.version = True, 1
            self.rows = []
        elif statement.startswith('SET LOCAL ROLE genui_owner;') and 'CREATE TABLE genui_meta.schema_migrations' in statement:
            self.ledger, self.rows = True, []
        elif statement == 'INSERT INTO genui_meta.schema_migrations(version,sha256) VALUES (1,%s)':
            self.history.append((1, params[0])); self.rows = []
        elif statement == 'INSERT INTO genui_meta.schema_migrations(version,sha256) VALUES (%s,%s)':
            self.history.append(tuple(params)); self.rows = []
        elif statement == 'UPDATE genui_meta.schema_version SET version=%s WHERE singleton':
            self.version = params[0]; self.rows = []
        else: raise AssertionError('unexpected unit-only statement: ' + statement[:80])
        return self

    def fetchone(self): return self.rows[0] if self.rows else None
    def fetchall(self): return list(self.rows)


def fixture():
    now = int(time.time())
    return {"schema_version": 1, "generation": str(uuid.uuid4()), "issued_at": now, "expires_at": now + 500,
        "database": {"host": "synthetic.invalid", "port": 5432, "name": "genui", "roles": {
            name: {"username": "genui_" + name, "token": "test-only-" + name, "expires_at": now + 500} for name in runtime.ROLES}},
        "storage": {"region": "us-east-1", "bucket": "unit-only-bucket", "tenants": [
            {"organization_id": tenant, "prefix": "o/" + tenant + "/", "role_arn": "unit-role-" + str(i), "kms_key_arn": "unit-key-" + str(i),
             "access_key_id": "unit-only-id", "secret_access_key": "unit-only-secret", "session_token": "unit-only-session", "expires_at": now + 500}
            for i, tenant in enumerate(["00000000-0000-4000-8000-000000000001", "00000000-0000-4000-8000-000000000002"]) ]}}


class RuntimeTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="genui-runtime-unit-")
        self.path = Path(self.tmp.name) / "credentials.json"
        self.value = fixture()
        runtime.atomic_write(self.path, self.value)
    def tearDown(self): self.tmp.cleanup()

    def test_private_atomic_rotation_changes_identity(self):
        initial = runtime.load(self.path)
        self.value["generation"] = str(uuid.uuid4())
        runtime.atomic_write(self.path, self.value)
        after = runtime.load(self.path)
        self.assertNotEqual(initial["generation"], after["generation"])
        self.assertNotEqual(initial["_file_sha256"], after["_file_sha256"])
        self.assertEqual(self.path.stat().st_mode & 0o777, 0o600)
        self.assertEqual(sorted(p.name for p in self.path.parent.iterdir()), ["credentials.json"])

    def test_expired_future_overlong_and_role_interchange_refused(self):
        cases = [lambda v: v.update(expires_at=v["issued_at"]), lambda v: v.update(issued_at=v["issued_at"] + 60),
                 lambda v: v.update(expires_at=v["issued_at"] + 700),
                 lambda v: v["database"]["roles"]["data"].update(username="genui_migration"),
                 lambda v: v["database"]["roles"]["data"].update(token="test-only-worker"),
                 lambda v: v["storage"]["tenants"][0].update(prefix="o/another/"),
                 lambda v: v["storage"]["tenants"][0].update(expires_at=v["issued_at"])]
        for edit in cases:
            with self.subTest(edit=edit):
                value = copy.deepcopy(self.value); edit(value); runtime.atomic_write(self.path, value)
                with self.assertRaises(runtime.Refused): runtime.load(self.path)

    def test_permission_and_symlink_denial(self):
        self.path.chmod(0o644)
        with self.assertRaises(runtime.Refused): runtime.load(self.path)
        self.path.chmod(0o600)
        link = self.path.parent / "link.json"; link.symlink_to(self.path)
        with self.assertRaises(OSError): runtime.load(link)
        self.path.parent.chmod(0o755)
        with self.assertRaises(runtime.Refused): runtime.load(self.path)
        self.path.parent.chmod(0o700)

    def test_same_generation_byte_change_has_distinct_fingerprint(self):
        first = runtime.load(self.path)
        self.value["database"]["roles"]["data"]["token"] = "rotated-unit-token"
        runtime.atomic_write(self.path, self.value)
        second = runtime.load(self.path)
        self.assertEqual(first["generation"], second["generation"])
        self.assertNotEqual(first["_file_sha256"], second["_file_sha256"])

    def test_fifo_and_oversized_file_are_refused_without_blocking(self):
        fifo = self.path.parent / "fifo"
        os.mkfifo(fifo, 0o600)
        started = time.monotonic()
        with self.assertRaises(runtime.Refused): runtime.load(fifo)
        self.assertLess(time.monotonic() - started, 0.5)
        self.path.write_bytes(b"x" * 65537)
        with self.assertRaises(runtime.Refused): runtime.load(self.path)

    def test_mutating_qualification_is_not_a_readiness_side_effect(self):
        with patch.dict(os.environ, {}, clear=True), patch.object(runtime, 'refresh') as refresh:
            with self.assertRaises(runtime.Refused): runtime.qualification(self.path)
            refresh.assert_not_called()

    def test_cleanup_rejects_an_unrelated_bucket_before_aws_client(self):
        report = self.path.parent / "qualification.json"
        run_id = str(uuid.uuid4())
        report.write_text(json.dumps({'schema_version':1,'scope':'APP-007 synthetic managed qualification','run_id':run_id,'cleanup':[{'Bucket':'unrelated','Key':'o/unrelated/probe','VersionId':'unit-version'}]}))
        env = {'GENUI_QUALIFY_SYNTHETIC':'APP-007','GENUI_EVIDENCE_BUCKET':'unit-only-bucket','GENUI_TENANTS_JSON':json.dumps(self.value['storage']['tenants'])}
        with patch.dict(os.environ,env), patch('boto3.Session') as session:
            with self.assertRaises(runtime.Refused): runtime.cleanup(report)
            session.assert_not_called()

    def test_real_probe_refuses_unexpected_db_identity_before_storage(self):
        class Connection:
            def __enter__(self): return self
            def __exit__(self, *args): pass
            def execute(self, *args): return self
            def fetchone(self): return ("genui_migration",) * 12
        with patch.dict(os.environ, {"GENUI_RDS_CA_FILE": "/unit-only/ca.pem"}), patch('psycopg.connect', return_value=Connection()) as connect, patch.object(runtime, 'scoped_session') as storage, patch.object(runtime, 'verify_schema_catalog'):
            with self.assertRaises(runtime.Refused): runtime.probe(self.path, self.value["generation"])
            self.assertEqual(connect.call_args.kwargs['sslmode'], 'verify-full')
            self.assertEqual(connect.call_args.kwargs['user'], 'genui_workspace')
            self.assertIn('-c search_path=pg_catalog,pg_temp',connect.call_args.kwargs['options'])
            storage.assert_not_called()

    def test_rds_superuser_membership_refused_even_with_false_superuser_flags(self):
        class Connection:
            def __enter__(self): return self
            def __exit__(self, *args): pass
            def execute(self, *args): return self
            def fetchone(self):
                return ('genui_workspace','genui_workspace','genui',False,False,False,False,True,False,True,True,runtime.DATABASE_SCHEMA_VERSION)
            def fetchall(self): return [('rds_superuser',)]
        with patch.dict(os.environ, {'GENUI_RDS_CA_FILE':'/unit-only/ca.pem'}), patch('psycopg.connect',return_value=Connection()), patch.object(runtime,'scoped_session') as storage, patch.object(runtime,'verify_schema_catalog'):
            with self.assertRaises(runtime.Refused): runtime.probe(self.path,self.value['generation'])
            storage.assert_not_called()

    def test_real_probe_requires_exact_image_schema_version_before_storage(self):
        current = runtime.DATABASE_SCHEMA_VERSION
        class Connection:
            def __init__(self, user, version): self.user, self.version = user, version
            def __enter__(self): return self
            def __exit__(self, *args): pass
            def execute(self, *args): return self
            def fetchone(self): return (self.user,self.user,'genui',False,False,False,False,True,False,True,True,self.version)
            def fetchall(self): return []
        class Session:
            def client(self, *args, **kwargs): return self
            def list_objects_v2(self, **kwargs): return {}
            def encrypt(self, **kwargs): return {'CiphertextBlob':b'unit-only'}
            def decrypt(self, **kwargs): return {'Plaintext':b'genui-foundation-readiness-v1'}
        for version in (1,current-1,current,current+1):
            with self.subTest(version=version), patch.dict(os.environ, {'GENUI_RDS_CA_FILE':'/unit-only/ca.pem'}), \
                 patch('psycopg.connect',side_effect=lambda **kw: Connection(kw['user'],version)), \
                 patch.object(runtime,'verify_schema_catalog'), \
                 patch.object(runtime,'scoped_session',return_value=Session()) as storage:
                if version==current:
                    result=runtime.probe(self.path,self.value['generation'])
                    self.assertEqual(result['generation'],self.value['generation'])
                    self.assertEqual(storage.call_count,2)
                else:
                    with self.assertRaises(runtime.Refused): runtime.probe(self.path,self.value['generation'])
                    storage.assert_not_called()

    def test_probe_catalog_refusal_precedes_identity_and_storage(self):
        class Connection:
            def __enter__(self): return self
            def __exit__(self,*args): pass
            def execute(self,*args): raise AssertionError('metadata read before catalog proof')
        with patch.dict(os.environ,{'GENUI_RDS_CA_FILE':'/unit-only/ca.pem'}), \
             patch('psycopg.connect',return_value=Connection()), \
             patch.object(runtime,'verify_schema_catalog',side_effect=runtime.Refused()) as catalog, \
             patch.object(runtime,'scoped_session') as storage:
            with self.assertRaises(runtime.Refused): runtime.probe(self.path,self.value['generation'])
            catalog.assert_called_once();storage.assert_not_called()

    def test_generation_change_refused_before_network(self):
        with patch('psycopg.connect') as connect:
            with self.assertRaises(runtime.Refused): runtime.probe(self.path, str(uuid.uuid4()))
            connect.assert_not_called()

    def test_production_entry_points_refuse_a_manifest_other_than_the_image_pin(self):
        # Runner review M1: the image's DATABASE_SCHEMA_VERSION is load-bearing
        # for the two production entry points, before any connection is made.
        self.assertEqual(runtime.migration_manifest(MIGRATIONS)['database_version'], runtime.DATABASE_SCHEMA_VERSION)
        env = {'GENUI_RDS_CA_FILE': '/unit-only/ca.pem', 'PGDATABASE': 'genui', 'PGUSER': 'genui_migration',
               'PGHOST': 'synthetic.invalid', 'PGPASSWORD': 'unit-only', 'PGSSLROOTCERT': '/unit-only/ca.pem'}
        with tempfile.TemporaryDirectory() as tmp:
            directory = synthetic_versions(tmp, runtime.DATABASE_SCHEMA_VERSION + 1)
            for name, call in (('probe', lambda: runtime.probe(self.path, self.value['generation'])), ('migrate', runtime.migrate)):
                with self.subTest(entry=name), patch.dict(os.environ, env), \
                     patch.object(runtime, 'migration_directory', return_value=directory), \
                     patch('psycopg.connect') as connect, patch.object(runtime, 'scoped_session') as storage:
                    with self.assertRaises(runtime.Refused): call()
                    connect.assert_not_called(); storage.assert_not_called()


class MigrationSourceTests(unittest.TestCase):
    def test_exact_reviewed_migrations_and_original_bytes_are_required(self):
        source=Path(runtime.__file__).resolve().parent.parent/'migrations'
        reviewed=runtime.migration_manifest(source)['database_version']
        self.assertEqual([row[0] for row in runtime.migration_sources(source)],list(range(1,reviewed+1)))
        for case in ('unknown_version','reorder','extra_sql','changed_second','changed_catalog','changed_query','changed_original_even_with_new_hash'):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as tmp:
                target=Path(tmp)
                for p in source.iterdir():
                    if p.is_file(): shutil.copyfile(p,target/p.name)
                manifest=json.loads((target/'manifest.json').read_text())
                if case=='unknown_version': manifest['database_version']=reviewed+1
                elif case=='reorder': manifest['migrations'].reverse()
                elif case=='extra_sql': (target/f'{reviewed+1:04d}_unreviewed.sql').write_text('SELECT 1;')
                elif case=='changed_second': (target/'0002_tenant_foundation.sql').write_text('SELECT 1;')
                elif case=='changed_catalog': (target/'catalog-v2.json').write_text('{}')
                elif case=='changed_query': (target/'catalog-query.json').write_text('{"sql":"SELECT 1"}')
                else:
                    p=target/'0001_role_foundation.sql';p.write_text(p.read_text()+'\n')
                    manifest['migrations'][0]['sha256']=runtime.hashlib.sha256(p.read_bytes()).hexdigest()
                (target/'manifest.json').write_text(json.dumps(manifest))
                with self.assertRaises(runtime.Refused): runtime.migration_sources(target)

    def test_migration_sources_accepts_three_pinned_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = synthetic_versions(tmp, 3)
            manifest = json.loads((directory / 'manifest.json').read_text())
            sources = runtime.migration_sources(directory)
            self.assertEqual([(version, digest) for version, digest, _ in sources],
                             [(item['version'], item['sha256']) for item in manifest['migrations']])
            self.assertEqual(sources[0][1], runtime.INITIAL_MIGRATION_SHA256)
            self.assertEqual(sources[2][2], (directory / '0003_unit_only_marker.sql').read_text())
            query, catalogs = runtime.catalog_resources(directory)
            self.assertEqual(query, json.loads((directory / 'catalog-query.json').read_text()))
            self.assertEqual(sorted(catalogs), [1, 2, 3])
            for version in (1, 2, 3):
                self.assertEqual(catalogs[version], json.loads((directory / f'catalog-v{version}.json').read_text()))
        # Exact pins, numbering, stray-file and section-key rules hold for N files.
        for case in ('changed_third', 'unpinned_fourth', 'misnumbered_third', 'unnumbered_third', 'catalog_sections_differ',
                     'missing_catalog_pin', 'version_count_mismatch', 'extra_manifest_key'):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as tmp:
                directory = synthetic_versions(tmp, 3)
                manifest = json.loads((directory / 'manifest.json').read_text())
                third = manifest['migrations'][2]
                if case == 'changed_third': (directory / third['file']).write_text('SELECT 4;\n')
                elif case == 'unpinned_fourth': (directory / '0004_unit_only_marker.sql').write_text('SELECT 4;\n')
                elif case in ('misnumbered_third', 'unnumbered_third'):
                    name = '0004_unit_only_marker.sql' if case == 'misnumbered_third' else 'unit_only_marker.sql'
                    (directory / third['file']).rename(directory / name); third['file'] = name
                elif case == 'catalog_sections_differ':
                    catalog = json.loads((directory / 'catalog-v3.json').read_text()); catalog['unreviewed_section'] = []
                    raw = json.dumps(catalog).encode(); (directory / 'catalog-v3.json').write_bytes(raw)
                    manifest['catalog'][3]['sha256'] = hashlib.sha256(raw).hexdigest()
                elif case == 'missing_catalog_pin': manifest['catalog'].pop()
                elif case == 'version_count_mismatch': manifest['database_version'] = 4
                else: manifest['unreviewed'] = True
                (directory / 'manifest.json').write_text(json.dumps(manifest))
                with self.assertRaises(runtime.Refused): runtime.migration_sources(directory)


class MigrationRunnerTests(unittest.TestCase):
    """Runner bookkeeping against MigrationRecorder; real SQL is qualify_tenant_schema.py."""

    def test_apply_migrations_upgrades_v2_to_v3_and_repeat_is_noop(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = synthetic_versions(tmp, 3)
            manifest = json.loads((directory / 'manifest.json').read_text())
            database = MigrationRecorder(directory, 2)
            runtime.apply_migrations(database, directory)
            self.assertEqual(database.applied(), [3])
            self.assertEqual(database.history, [(item['version'], item['sha256']) for item in manifest['migrations']])
            self.assertEqual((database.version, database.shown), (3, 3))
            statements = database.statements
            self.assertEqual(statements[:2], ['SHOW session_replication_role', 'SET LOCAL search_path=pg_catalog, pg_temp'])
            self.assertIn('SELECT pg_advisory_xact_lock(1819409017, 2)', statements)
            # The recorded version is proven by the catalog before any ledger or
            # version read; the last statement is the final catalog-v3 proof.
            self.assertLess(statements.index(database.query), min(statements.index(LEDGER_READ), statements.index(VERSION_READ)))
            self.assertLess(statements.index(VERSION_READ), statements.index((directory / '0003_unit_only_marker.sql').read_text()))
            self.assertEqual(statements[-1], database.query)
            # Already at N: exact-match repeat with the same lock and no writes.
            before = (list(database.history), database.version, database.shown)
            database.statements = []
            runtime.apply_migrations(database, directory)
            self.assertEqual((database.history, database.version, database.shown), before)
            self.assertEqual(database.writes(), [])
            self.assertIn('SELECT pg_advisory_xact_lock(1819409017, 2)', database.statements)
            self.assertEqual(database.statements.count(database.query), 1)

    def test_new_ledger_starts_at_version_two_and_every_step_is_proven(self):
        # ADR-007 D7.12: the runner creates the ledger at the historical bound and
        # each later migration ALTERs both version CHECKs, so a fresh or legacy
        # install passes through every reviewed catalog, proven step by step.
        self.assertEqual(runtime.LEDGER_VERSION, 2)
        bound = f'CHECK (version BETWEEN 1 AND {runtime.LEDGER_VERSION})'
        with tempfile.TemporaryDirectory() as tmp:
            directory = synthetic_versions(tmp, 3)
            third = (directory / '0003_unit_only_marker.sql').read_text()
            for created in (False, True):  # fresh database, then preserved legacy version 1
                with self.subTest(created=created):
                    database = MigrationRecorder(directory, 1, ledger=False, created=created)
                    runtime.apply_migrations(database, directory)
                    ledger, = [s for s in database.statements if s.startswith('SET LOCAL ROLE genui_owner;')]
                    self.assertEqual((ledger.count('BETWEEN 1 AND '), ledger.count(bound)), (2, 2))
                    self.assertEqual(database.applied(), [2, 3] if created else [1, 2, 3])
                    self.assertEqual(([version for version, _ in database.history], database.version), ([1, 2, 3], 3))
                    # Proof of version 1, of version 2 before 0003 runs, and finally of version 3.
                    proofs = [index for index, s in enumerate(database.statements) if s == database.query]
                    self.assertEqual(len(proofs), 3)
                    self.assertLess(proofs[1], database.statements.index(third))
                    self.assertEqual(database.statements[-1], database.query)
                    # A step whose catalog differs is refused before the next migration runs.
                    database = MigrationRecorder(directory, 1, ledger=False, created=created)
                    database.drift[2] = json.dumps({'unreviewed': []})
                    with self.assertRaises(runtime.Refused): runtime.apply_migrations(database, directory)
                    self.assertEqual(database.applied(), [2] if created else [1, 2])
        # The reviewed manifest: the same bound, then one proof per version 1..N.
        reviewed = runtime.migration_manifest(MIGRATIONS)['database_version']
        database = MigrationRecorder(MIGRATIONS, 1, ledger=False)
        runtime.apply_migrations(database, MIGRATIONS)
        ledger, = [s for s in database.statements if s.startswith('SET LOCAL ROLE genui_owner;')]
        self.assertEqual((ledger.count('BETWEEN 1 AND '), ledger.count(bound)), (2, 2))
        self.assertEqual((database.version, database.statements.count(database.query)), (reviewed, reviewed))
        self.assertEqual(database.applied(), list(range(2, reviewed + 1)))

    def test_apply_migrations_proves_each_intermediate_version_before_the_next(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = synthetic_versions(tmp, 4)
            database = MigrationRecorder(directory, 2)
            runtime.apply_migrations(database, directory)
            self.assertEqual((database.applied(), [v for v, _ in database.history], database.version), ([3, 4], [1, 2, 3, 4], 4))
            # Proof of version 2 before metadata, of version 3, and finally of version 4.
            self.assertEqual(database.statements.count(database.query), 3)
            database = MigrationRecorder(directory, 2)
            database.drift[3] = json.dumps({'unreviewed': []})
            with self.assertRaises(runtime.Refused): runtime.apply_migrations(database, directory)
            self.assertEqual(database.applied(), [3])

    def test_apply_migrations_refuses_unproven_or_altered_history_before_writes(self):
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / 'three').mkdir(); (Path(tmp) / 'four').mkdir()
            directory = synthetic_versions(Path(tmp) / 'three', 3)
            newer = (synthetic_versions(Path(tmp) / 'four', 4) / 'catalog-v4.json').read_text()
            cases = {
                'altered_recorded_checksum': lambda d: d.history.__setitem__(1, (2, '0' * 64)),
                'ledger_ahead_of_catalog': lambda d: d.history.append((3, d.digests[3])),
                'version_row_disagrees': lambda d: setattr(d, 'version', 3),
                'unreviewed_catalog': lambda d: d.drift.__setitem__(2, json.dumps({'unreviewed': []})),
                'catalog_newer_than_manifest': lambda d: d.drift.__setitem__(2, newer),
            }
            for case, edit in cases.items():
                with self.subTest(case=case):
                    database = MigrationRecorder(directory, 2)
                    edit(database)
                    with self.assertRaises(runtime.Refused): runtime.apply_migrations(database, directory)
                    self.assertIn('SELECT pg_advisory_xact_lock(1819409017, 2)', database.statements)
                    self.assertEqual(database.statements.count(database.query), 1)
                    self.assertEqual(database.writes(), [])
                    if case in ('unreviewed_catalog', 'catalog_newer_than_manifest'):
                        # Catalog proof precedes every metadata read.
                        self.assertNotIn(LEDGER_READ, database.statements)
                        self.assertNotIn(VERSION_READ, database.statements)

    def test_probe_and_default_catalog_proof_follow_the_manifest_version(self):
        class Connection:
            def __init__(self, version): self.version = version
            def execute(self, *args): return self
            def fetchone(self): return ('genui_workspace','genui_workspace','genui',False,False,False,False,True,False,True,True,self.version)
            def fetchall(self): return []
        with tempfile.TemporaryDirectory() as tmp:
            directory = synthetic_versions(tmp, 3)
            for version in (2, 3):
                with self.subTest(version=version), patch.object(runtime, 'migration_directory', return_value=directory), \
                     patch.object(runtime, 'verify_schema_catalog') as catalog:
                    connection = Connection(version)
                    if version == 3: runtime.probe_database(connection, 'genui_workspace')
                    else:
                        with self.assertRaises(runtime.Refused): runtime.probe_database(connection, 'genui_workspace')
                    catalog.assert_called_once_with(connection, directory)
            # Without an explicit version the proof requires the manifest's latest.
            runtime.verify_schema_catalog(MigrationRecorder(directory, 3), directory)
            with self.assertRaises(runtime.Refused): runtime.verify_schema_catalog(MigrationRecorder(directory, 2), directory)
            runtime.verify_schema_catalog(MigrationRecorder(directory, 2), directory, 2)


class GeneratorTests(unittest.TestCase):
    """The real generator over a unit-only stand-in for local PostgreSQL 16.13."""

    def test_generator_refuses_snapshot_over_80_percent_cap(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'migrations').mkdir()
            directory = synthetic_versions(root / 'migrations', 3)
            query = json.loads((directory / 'catalog-query.json').read_text())['sql']
            feed, statements = [], []
            class Connection:
                def __enter__(self): return self
                def __exit__(self, *args): return False
                def execute(self, statement, params=None):
                    statements.append(statement)
                    self.row = (feed.pop(0),) if statement == query else None
                    return self
                def fetchone(self): return self.row
            class LocalPostgres:
                removed = False
                def __enter__(self): return self
                def __exit__(self, *args): self.removed = True
                def connect(self): return Connection()
            stand_in = types.ModuleType('qualify_tenant_schema')
            stand_in.LocalPostgres, stand_in.MIGRATIONS, stand_in.IMAGE = LocalPostgres, directory, 'unit-only-image'
            with patch.dict(sys.modules, {'qualify_tenant_schema': stand_in}):
                sys.modules.pop('generate_schema_catalog', None)
                generator = importlib.import_module('generate_schema_catalog')
                small = generator.readable({'relations': [['genui', 'unit_only', 'r']]})
                base = len(generator.readable({'relations': [['pad', '']]}).encode())
                def padded(size): return generator.readable({'relations': [['pad', 'x' * (size - base)]]})
                # ADR-007 D7.12: 80 % of the moved 524,288-byte bound.
                self.assertEqual((runtime.CATALOG_BOUND, runtime.CATALOG_BOUND * 4 // 5), (524288, 419430))
                self.assertEqual(len(padded(419430).encode()), 419430)
                cases = (('at_cap', padded(419430), None),
                         # Compact body under the guard, readable file over it.
                         ('file_over_cap', json.dumps(json.loads(padded(419431)), separators=(',', ':')),
                          r'catalog-v3 candidate file length 419431 exceeds 419430 bytes'),
                         ('projection_over_cap', padded(419430) + ' ', r'catalog-v3 candidate body length 419431 exceeds 419430 bytes'))
                for case, third, refusal in cases:
                    with self.subTest(case=case):
                        feed[:] = [small, small, third]; statements.clear()
                        output = root / ('out-' + case)
                        if refusal is None:
                            receipt = generator.generate(output)
                            self.assertEqual(sorted(p.name for p in output.iterdir()),
                                             ['catalog-v1.json', 'catalog-v2.json', 'catalog-v3.json', 'generation.json'])
                            self.assertEqual(len((output / 'catalog-v3.json').read_bytes()), 419430)
                            self.assertEqual(sorted(receipt['candidate_sha256']), ['catalog-v1.json', 'catalog-v2.json', 'catalog-v3.json'])
                        else:
                            with self.assertRaisesRegex(ValueError, refusal): generator.generate(output)
                            self.assertFalse(output.exists())
                        # One snapshot after every migration, all taken before any refusal.
                        self.assertEqual(statements.count(query), 3)
                        self.assertEqual(feed, [])
                        # As in the runner, the upgrade to version 2 creates the ledger at
                        # its historical bound, so catalog-v2 is the real version-2 state.
                        ledger, = [s for s in statements if s.startswith('SET LOCAL ROLE genui_owner;')]
                        self.assertEqual(ledger.count('CHECK (version BETWEEN 1 AND 2)'), 2)
                        second = (directory / '0002_tenant_foundation.sql').read_bytes().decode('utf-8')
                        self.assertLess(statements.index(ledger), statements.index(second))


class IdentityRegistryFileTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory(prefix='genui-empty-registry-unit-')
        self.directory=Path(self.tmp.name);self.directory.chmod(0o700)
        self.path=self.directory/'credentials.json'

    def tearDown(self):
        self.directory.chmod(0o700);self.tmp.cleanup()

    def files(self): return list(self.directory.glob('.identity-registry-*.json'))

    def test_new_owned_empty_registry_has_closed_shape_and_cleanup(self):
        owned=runtime.EmptyIdentityRegistryFile(self.path)
        value=json.loads(owned.path.read_text())
        self.assertEqual(set(value),{'schema_version','registry_version','bindings'})
        self.assertEqual(value['schema_version'],1);self.assertEqual(value['bindings'],[])
        self.assertEqual(str(uuid.UUID(value['registry_version'])),value['registry_version'])
        self.assertEqual(stat.S_IMODE(owned.path.stat().st_mode),0o600)
        self.assertEqual(owned.path.stat().st_uid,os.geteuid())
        self.assertFalse(os.get_inheritable(owned.file_fd))
        owned.check_file();owned.close();owned.close()
        self.assertEqual(self.files(),[])

    def test_real_children_receive_private_empty_or_exact_explicit_registry(self):
        report=self.directory/'report.json';script=self.directory/'child.py'
        script.write_text(f'''#!{sys.executable}
import json,os,stat
from pathlib import Path
p=Path(os.environ[{runtime.IDENTITY_REGISTRY_FILE_ENV!r}])
Path({str(report)!r}).write_text(json.dumps({{'path':str(p),'value':json.loads(p.read_text()),'mode':stat.S_IMODE(p.stat().st_mode),'raw_auth_absent':{runtime.QUALIFICATION_JSON_ENV!r} not in os.environ}}))
''');script.chmod(0o700)
        explicit=self.directory/'operator-registry.json'
        explicit_payload=json.dumps({'schema_version':1,'registry_version':str(uuid.uuid4()),'bindings':[]})
        explicit.write_text(explicit_payload);explicit.chmod(0o600)
        for supplied in (False,True):
            with self.subTest(supplied=supplied):
                env={'GENUI_BINARY':str(script)}
                if supplied: env[runtime.IDENTITY_REGISTRY_FILE_ENV]=str(explicit)
                with patch.dict(os.environ,env,clear=True),patch.object(runtime.signal,'signal'), \
                     patch.object(runtime,'refresh',side_effect=lambda p:runtime.atomic_write(p,fixture())):
                    self.assertEqual(runtime.serve(self.path),0)
                    self.assertEqual(runtime.IDENTITY_REGISTRY_FILE_ENV in os.environ,supplied)
                value=json.loads(report.read_text())
                self.assertEqual(value['mode'],0o600);self.assertTrue(value['raw_auth_absent'])
                self.assertEqual(value['value']['bindings'],[])
                if supplied:
                    self.assertEqual(value['path'],str(explicit))
                    self.assertEqual(value['value'],json.loads(explicit_payload))
                else: self.assertTrue(Path(value['path']).name.startswith('.identity-registry-'))
                self.assertEqual(explicit.read_text(),explicit_payload)
                self.assertEqual(self.files(),[]);self.assertFalse(self.path.exists())

    def test_explicit_empty_path_is_preserved_for_native_refusal(self):
        with patch.dict(os.environ,{runtime.IDENTITY_REGISTRY_FILE_ENV:''},clear=True), \
             patch.object(runtime.signal,'signal'),patch.object(runtime,'refresh'), \
             patch.object(runtime.subprocess,'Popen') as launch:
            launch.return_value.poll.return_value=7;launch.return_value.returncode=7
            self.assertEqual(runtime.serve(self.path),7)
            self.assertEqual(launch.call_args.kwargs['env'][runtime.IDENTITY_REGISTRY_FILE_ENV],'')
        self.assertEqual(self.files(),[])

    def test_replaced_registry_is_refused_before_launch_and_preserved(self):
        def refresh(_):
            path,=self.files();path.unlink();path.write_text('unrelated replacement')
        with patch.dict(os.environ,{},clear=True),patch.object(runtime.signal,'signal'), \
             patch.object(runtime,'refresh',side_effect=refresh),patch.object(runtime.subprocess,'Popen') as launch:
            with self.assertRaises(runtime.Refused): runtime.serve(self.path)
            launch.assert_not_called()
        path,=self.files();self.assertEqual(path.read_text(),'unrelated replacement')

    def test_existing_collision_and_unsafe_directory_are_never_replaced(self):
        fixed=uuid.UUID('00000000-0000-4000-8000-000000000001')
        collision=self.directory/('.identity-registry-'+fixed.hex+'.json')
        collision.write_text('existing operator data')
        with patch.object(runtime.uuid,'uuid4',return_value=fixed):
            with self.assertRaises(FileExistsError): runtime.EmptyIdentityRegistryFile(self.path)
        self.assertEqual(collision.read_text(),'existing operator data')
        self.directory.chmod(0o755)
        with self.assertRaises(runtime.Refused): runtime.EmptyIdentityRegistryFile(self.path)

    def test_refresh_failure_cleans_owned_registry(self):
        stop=threading.Event()
        def refresh(_):
            self.assertEqual(len(self.files()),1);stop.set();raise OSError('synthetic')
        with patch.dict(os.environ,{},clear=True),patch.object(runtime.signal,'signal'), \
             patch.object(runtime.threading,'Event',return_value=stop),patch.object(runtime,'refresh',side_effect=refresh), \
             contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(runtime.serve(self.path),0)
        self.assertEqual(self.files(),[])


class QualificationSecretTests(unittest.TestCase):
    PAYLOAD = '{"operator_token":"synthetic-only-not-a-real-credential"}'

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="genui-qualification-secret-unit-")
        self.directory = Path(self.tmp.name)
        self.directory.chmod(0o700)
        self.path = self.directory / "credentials.json"
        self.value = fixture()

    def tearDown(self):
        self.directory.chmod(0o700)
        self.tmp.cleanup()

    def auth_files(self):
        return list(self.directory.glob('.auth-qualification-*'))

    def test_injection_is_removed_and_refused_in_every_mode(self):
        """APP-009 review m6 (ADR-007 D7.13): the harness configuration is
        refused, never written to a file; the raw value leaves the environment."""
        for env in ({runtime.QUALIFICATION_JSON_ENV: self.PAYLOAD}, {runtime.QUALIFICATION_JSON_ENV: ''},
                    {runtime.QUALIFICATION_FILE_ENV: '/run/genui/x'},
                    {runtime.QUALIFICATION_JSON_ENV: self.PAYLOAD, runtime.QUALIFICATION_FILE_ENV: ''}):
            with self.subTest(env=sorted(env)), patch.dict(os.environ, env, clear=True):
                with self.assertRaises(runtime.Refused) as error:
                    runtime.refuse_qualification_injection()
                self.assertEqual(str(error.exception), 'runtime configuration refused')
                self.assertNotIn(runtime.QUALIFICATION_JSON_ENV, os.environ)
        with patch.dict(os.environ, {}, clear=True):
            self.assertIsNone(runtime.refuse_qualification_injection())

    def test_new_real_private_file_is_exact_and_cleanup_preserves_other_files(self):
        self.path.write_text('unrelated-existing-credential-file')
        owned = runtime.QualificationFile(self.path, self.PAYLOAD.encode())
        path = owned.path
        self.assertEqual(path.parent, self.directory)
        self.assertEqual(path.read_bytes(), self.PAYLOAD.encode())
        info = path.stat()
        self.assertEqual(stat.S_IMODE(info.st_mode), 0o600)
        self.assertEqual(info.st_uid, os.geteuid())
        self.assertEqual(info.st_nlink, 1)
        self.assertFalse(os.get_inheritable(owned.file_fd))
        owned.check_file()
        owned.close()
        owned.close()
        self.assertFalse(path.exists())
        self.assertIsNone(owned.file_fd)
        self.assertEqual(self.path.read_text(), 'unrelated-existing-credential-file')

    def test_existing_regular_symlink_hardlink_and_fifo_names_are_not_clobbered(self):
        fixed = uuid.UUID(int=0)
        collision = self.directory / ('.auth-qualification-' + fixed.hex + '.json')
        target = self.directory / 'unrelated.json'
        target.write_text('preserve-me')
        for kind in ('regular', 'symlink', 'hardlink', 'fifo'):
            with self.subTest(kind=kind):
                if kind == 'regular': collision.write_text('existing-file')
                elif kind == 'symlink': collision.symlink_to(target)
                elif kind == 'hardlink': os.link(target, collision)
                else: os.mkfifo(collision, 0o600)
                before = collision.lstat()
                started = time.monotonic()
                with patch.object(runtime.uuid, 'uuid4', return_value=fixed):
                    with self.assertRaises(OSError): runtime.QualificationFile(self.path, self.PAYLOAD.encode())
                after = collision.lstat()
                self.assertEqual((before.st_dev, before.st_ino, before.st_mode), (after.st_dev, after.st_ino, after.st_mode))
                self.assertEqual(target.read_text(), 'preserve-me')
                self.assertLess(time.monotonic() - started, 0.5)
                collision.unlink()

    def test_unsafe_missing_or_linked_parent_refuses_without_creating_files(self):
        self.directory.chmod(0o755)
        with self.assertRaises(runtime.Refused): runtime.QualificationFile(self.path, self.PAYLOAD.encode())
        self.directory.chmod(0o700)
        link = self.directory / 'linked'
        link.symlink_to(self.directory, target_is_directory=True)
        with self.assertRaises(OSError): runtime.QualificationFile(link / 'credentials.json', self.PAYLOAD.encode())
        missing = self.directory / 'missing'
        with self.assertRaises(OSError): runtime.QualificationFile(missing / 'credentials.json', self.PAYLOAD.encode())
        self.assertFalse(missing.exists())
        with self.assertRaises(runtime.Refused): runtime.QualificationFile(Path('relative/credentials.json'), self.PAYLOAD.encode())
        self.assertEqual(self.auth_files(), [])

    def test_cleanup_refuses_replaced_or_newly_linked_file_without_deleting_it(self):
        for kind in ('regular', 'symlink', 'hardlink'):
            with self.subTest(kind=kind):
                owned = runtime.QualificationFile(self.path, self.PAYLOAD.encode())
                target = self.directory / ('target-' + kind)
                if kind == 'hardlink':
                    os.link(owned.path, target)
                else:
                    owned.path.unlink()
                    target.write_text('unrelated-preserve')
                    if kind == 'regular': owned.path.write_text('replacement-preserve')
                    else: owned.path.symlink_to(target)
                before = owned.path.lstat()
                with self.assertRaises(runtime.Refused): owned.close()
                after = owned.path.lstat()
                self.assertEqual((before.st_dev, before.st_ino), (after.st_dev, after.st_ino))
                self.assertIsNone(owned.directory_fd)
                self.assertIsNone(owned.file_fd)
                self.assertTrue(target.exists())
                owned.path.unlink(); target.unlink()

    def test_write_failure_removes_only_new_file(self):
        self.path.write_text('existing-preserve')
        with patch.object(runtime.os, 'fsync', side_effect=OSError('synthetic-write-failure')):
            with self.assertRaises(OSError): runtime.QualificationFile(self.path, self.PAYLOAD.encode())
        self.assertEqual(self.auth_files(), [])
        self.assertEqual(self.path.read_text(), 'existing-preserve')

    def test_serve_refuses_injection_before_refresh_launch_or_any_file(self):
        self.path.write_text('existing-preserve')
        for env in ({runtime.QUALIFICATION_JSON_ENV: self.PAYLOAD}, {runtime.QUALIFICATION_FILE_ENV: str(self.directory / 'x.json')}):
            with self.subTest(env=sorted(env)), patch.dict(os.environ, env, clear=True), \
                 patch.object(runtime.signal, 'signal'), patch.object(runtime, 'refresh') as refresh, \
                 patch.object(runtime.subprocess, 'Popen') as child:
                with self.assertRaises(runtime.Refused): runtime.serve(self.path)
                refresh.assert_not_called(); child.assert_not_called()
                self.assertNotIn(runtime.QUALIFICATION_JSON_ENV, os.environ)
        self.assertEqual(self.path.read_text(), 'existing-preserve')
        self.assertEqual(self.auth_files(), [])
        self.assertEqual(sorted(p.name for p in self.directory.iterdir()), ['credentials.json'])

    def child_script(self, exit_code=0, wait_for_termination=False):
        script = self.directory / 'child.py'
        report = self.directory / 'child-report.json'
        source = f'''#!{sys.executable}
import json, os, signal, time
from pathlib import Path
if {wait_for_termination!r}: signal.signal(signal.SIGTERM, signal.SIG_IGN)
result = {{'raw_env_absent': {runtime.QUALIFICATION_JSON_ENV!r} not in os.environ,
           'path_present': {runtime.QUALIFICATION_FILE_ENV!r} in os.environ}}
Path({str(report)!r}).write_text(json.dumps(result))
if {wait_for_termination!r}:
    while True: time.sleep(0.05)
raise SystemExit({exit_code})
'''
        script.write_text(source); script.chmod(0o700)
        return script, report

    def test_real_children_receive_no_qualification_handoff(self):
        for exit_code in (0, 7):
            with self.subTest(exit_code=exit_code):
                script, report = self.child_script(exit_code)
                env = {'GENUI_BINARY': str(script), 'GENUI_RUNTIME_CONFIG_FILE': str(self.path)}
                with patch.dict(os.environ, env, clear=True), patch.object(runtime.signal, 'signal'), \
                     patch.object(runtime, 'refresh', side_effect=lambda path: runtime.atomic_write(path, self.value)) as refresh:
                    self.assertEqual(runtime.serve(self.path), exit_code)
                    refresh.assert_called_once()
                self.assertEqual(json.loads(report.read_text()), {'raw_env_absent': True, 'path_present': False})
                self.assertEqual(self.auth_files(), [])
                self.assertFalse(self.path.exists())

    def test_startup_failure_and_cancel_leave_no_files_or_secret_output(self):
        for failure in ('refresh', 'launch', 'interrupt'):
            with self.subTest(failure=failure):
                stop = threading.Event()
                output = io.StringIO()
                def refresh(path):
                    if failure == 'refresh':
                        stop.set(); raise OSError(self.PAYLOAD)
                    if failure == 'interrupt': raise KeyboardInterrupt()
                    runtime.atomic_write(path, self.value)
                def launch(*args, **kwargs):
                    self.assertNotIn(runtime.QUALIFICATION_FILE_ENV, kwargs['env'])
                    stop.set(); raise OSError(self.PAYLOAD)
                with patch.dict(os.environ, {}, clear=True), \
                     patch.object(runtime.signal, 'signal'), patch.object(runtime.threading, 'Event', return_value=stop), \
                     patch.object(runtime, 'refresh', side_effect=refresh), patch.object(runtime.subprocess, 'Popen', side_effect=launch), \
                     contextlib.redirect_stderr(output):
                    if failure == 'interrupt':
                        with self.assertRaises(KeyboardInterrupt): runtime.serve(self.path)
                    else: self.assertEqual(runtime.serve(self.path), 0)
                self.assertNotIn(self.PAYLOAD, output.getvalue())
                self.assertEqual(self.auth_files(), [])
                self.assertEqual(list(self.directory.glob('.identity-registry-*')), [])

    def test_real_cancelled_child_ignoring_term_is_killed_and_cleanup_is_bounded(self):
        script, report = self.child_script(wait_for_termination=True)
        stop = threading.Event()
        def cancel_when_started():
            deadline = time.monotonic() + 3
            while not report.exists() and time.monotonic() < deadline: time.sleep(0.01)
            stop.set()
        watcher = threading.Thread(target=cancel_when_started, daemon=True)
        env = {'GENUI_BINARY': str(script), 'GENUI_RUNTIME_CONFIG_FILE': str(self.path)}
        started = time.monotonic()
        watcher.start()
        with patch.dict(os.environ, env, clear=True), patch.object(runtime.signal, 'signal'), \
             patch.object(runtime.threading, 'Event', return_value=stop), \
             patch.object(runtime, 'refresh', side_effect=lambda path: runtime.atomic_write(path, self.value)):
            self.assertEqual(runtime.serve(self.path), 0)
        watcher.join(timeout=1)
        self.assertTrue(report.exists())
        self.assertTrue(json.loads(report.read_text())['raw_env_absent'])
        self.assertLess(time.monotonic() - started, 8)
        self.assertEqual(self.auth_files(), [])
        self.assertFalse(self.path.exists())

    def test_actual_commands_refuse_injection_before_managed_operations(self):
        for mode in ('refresh', 'probe', 'migrate', 'qualify', 'cleanup', 'serve'):
            for env in ({runtime.QUALIFICATION_JSON_ENV: self.PAYLOAD}, {runtime.QUALIFICATION_FILE_ENV: '/run/genui/x'}):
                with self.subTest(mode=mode, env=sorted(env)):
                    process = subprocess.run([sys.executable, str(Path(runtime.__file__).resolve()), mode],
                                             env=dict(env, GENUI_RUNTIME_CONFIG_FILE=str(self.path), GENUI_BINARY='/nonexistent'),
                                             capture_output=True, text=True, timeout=5)
                    self.assertEqual(process.returncode, 1)
                    self.assertEqual(process.stdout, '')
                    self.assertEqual(process.stderr, 'managed_dependency_operation_refused\n')
                    self.assertNotIn(self.PAYLOAD, process.stdout + process.stderr)
        self.assertEqual(sorted(self.directory.iterdir()), [])


def capability(org=None, invitation=None, **changes):
    """One synthetic capability entry: (id, record)."""
    org, invitation = org or str(uuid.uuid4()), invitation or str(uuid.uuid4())
    record = {'format': runtime.CAPABILITY_FORMAT, 'organization_id': org, 'invitation_id': invitation,
              'remote_invitation_id': 'tinv_' + str(uuid.uuid4()), 'provider_expires_at': '2026-11-03T00:00:00Z',
              'invite_token': 'tinv_' + str(uuid.uuid4()), 'code_digest': 'sha256:' + 'ab' * 32}
    record.update(changes)
    return '%s.%s' % (org, invitation), {k: v for k, v in record.items() if v is not None}


class CapabilityInjectionTests(unittest.TestCase):
    """ADR-003 amendment 2026-10-30 (APP-074 review M1): the injected secret becomes 0600 files in a
    fresh 0700 directory under the runtime volume that only the service child is told about."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='genui-capabilities-unit-')
        self.directory = Path(self.tmp.name); self.directory.chmod(0o700)
        self.path = self.directory / 'credentials.json'

    def tearDown(self):
        self.tmp.cleanup()

    def secret(self, *entries):
        return json.dumps(dict(entries))

    def test_the_map_is_validated_whole(self):
        good = capability()
        self.assertEqual(runtime.capability_map(self.secret(good)), dict([good]))
        self.assertEqual(runtime.capability_map('{}'), {})
        self.assertEqual(runtime.capability_map(self.secret(capability(code_digest=None))).popitem()[1].get('code_digest'), None)
        key, record = capability()
        other_org = capability()[1]['organization_id']
        bad = [
            'not json', '[]', '{"a": 1}',
            json.dumps({key.upper(): record}),
            json.dumps({'%s.%s' % (other_org, record['invitation_id']): record}),
            self.secret(capability(extra='x')), self.secret(capability(format='other')),
            self.secret(capability(invite_token='short')), self.secret(capability(remote_invitation_id=None)),
            self.secret(capability(provider_expires_at='2026-11-03T00:00:00.000Z')),
            self.secret(capability(code_digest='sha256:AB')),
            '{"%s": %s, "%s": %s}' % (key, json.dumps(record), key, json.dumps(record)),  # a duplicated id
            self.secret(*(capability() for _ in range(runtime.CAPABILITIES_MAX + 1))),
            json.dumps({key: dict(record, invite_token=record['remote_invitation_id'])}),
            'x' * (runtime.CAPABILITY_SECRET_BYTES_MAX + 1),
        ]
        for text in bad:
            with self.subTest(text=text[:60]), self.assertRaises((runtime.Refused, ValueError)):
                runtime.capability_map(text)

    def test_files_are_private_exact_and_never_replace_a_name(self):
        entries = [capability(), capability()]
        target = self.directory / 'capabilities'; target.mkdir(mode=0o700); target.chmod(0o700)
        written = runtime.write_capabilities(str(target), self.secret(*entries))
        self.assertEqual(sorted(written), sorted(key + '.capability' for key, _ in entries))
        for key, record in entries:
            path = target / (key + '.capability')
            info = path.lstat()
            self.assertEqual((stat.S_IMODE(info.st_mode), info.st_uid, info.st_nlink), (0o600, os.geteuid(), 1))
            self.assertEqual(json.loads(path.read_bytes()), record)
        with self.assertRaises(FileExistsError):
            runtime.write_capabilities(str(target), self.secret(entries[0]))
        loose = self.directory / 'loose'; loose.mkdir(); loose.chmod(0o755)
        with self.assertRaises(runtime.Refused):
            runtime.write_capabilities(str(loose), self.secret(capability()))
        link = self.directory / 'link'; link.symlink_to(target, target_is_directory=True)
        with self.assertRaises(OSError):
            runtime.write_capabilities(str(link), self.secret(capability()))
        self.assertEqual(list(loose.iterdir()), [])

    def child(self):
        report = self.directory / 'report.json'; script = self.directory / 'child.py'
        script.write_text(f'''#!{sys.executable}
import json,os,stat
from pathlib import Path
d=os.environ.get({runtime.CAPABILITY_DIR_ENV!r})
out={{'dir':d,'raw_absent':{runtime.CAPABILITIES_JSON_ENV!r} not in os.environ}}
if d:
    p=Path(d)
    out.update(mode=stat.S_IMODE(p.lstat().st_mode),parent=str(p.parent),files={{n:[stat.S_IMODE((p/n).lstat().st_mode),json.loads((p/n).read_text())] for n in os.listdir(p)}})
Path({str(report)!r}).write_text(json.dumps(out))
'''); script.chmod(0o700)
        return script, report

    def test_serve_materializes_the_injected_secret_for_its_child_only(self):
        script, report = self.child()
        entries = [capability(), capability()]
        text = self.secret(*entries)
        for injected in (text, '{}', None):
            with self.subTest(injected=injected and injected[:20]):
                env = {'GENUI_BINARY': str(script)}
                if injected is not None:
                    env[runtime.CAPABILITIES_JSON_ENV] = injected
                with patch.dict(os.environ, env, clear=True), patch.object(runtime.signal, 'signal'), \
                     patch.object(runtime, 'refresh', side_effect=lambda p: runtime.atomic_write(p, fixture())):
                    self.assertEqual(runtime.serve(self.path), 0)
                    self.assertNotIn(runtime.CAPABILITIES_JSON_ENV, os.environ, 'removed from the supervisor itself')
                    self.assertNotIn(runtime.CAPABILITY_DIR_ENV, os.environ)
                value = json.loads(report.read_text())
                self.assertTrue(value['raw_absent'], 'the child never inherits the secret value')
                if injected is None:
                    self.assertIsNone(value['dir'])
                else:
                    self.assertEqual((value['mode'], value['parent']), (0o700, str(self.directory)))
                    self.assertTrue(Path(value['dir']).name.startswith(runtime.CapabilityDirectory.PREFIX))
                    expected = dict(entries) if injected == text else {}
                    self.assertEqual(value['files'], {key + '.capability': [0o600, record] for key, record in expected.items()})
                    self.assertFalse(Path(value['dir']).exists(), 'removed when serve ends')
                self.assertEqual(sorted(p.name for p in self.directory.iterdir()), ['child.py', 'report.json'])

    def test_a_malformed_secret_or_an_inherited_directory_refuses_before_launch(self):
        for env in ({runtime.CAPABILITIES_JSON_ENV: self.secret(capability(extra='x'))},
                    {runtime.CAPABILITIES_JSON_ENV: 'not json'},
                    {runtime.CAPABILITY_DIR_ENV: str(self.directory)}):
            with self.subTest(env=sorted(env)):
                with patch.dict(os.environ, env, clear=True), patch.object(runtime.signal, 'signal'), \
                     patch.object(runtime, 'refresh') as refresh, patch.object(runtime.subprocess, 'Popen') as launch:
                    with self.assertRaises((runtime.Refused, ValueError)): runtime.serve(self.path)
                    launch.assert_not_called(); refresh.assert_not_called()
                    self.assertNotIn(runtime.CAPABILITIES_JSON_ENV, os.environ)
                self.assertEqual(list(self.directory.iterdir()), [])

    def test_every_mode_removes_the_injection(self):
        with patch.dict(os.environ, {runtime.CAPABILITIES_JSON_ENV: '{}'}, clear=True):
            self.assertEqual(runtime.take_capability_injection(), '{}')
            self.assertNotIn(runtime.CAPABILITIES_JSON_ENV, os.environ)
            self.assertIsNone(runtime.take_capability_injection())
        process = subprocess.run([sys.executable, str(Path(runtime.__file__).resolve()), 'refresh'],
                                 env={runtime.CAPABILITIES_JSON_ENV: self.secret(capability()),
                                      'GENUI_RUNTIME_CONFIG_FILE': str(self.path)},
                                 capture_output=True, text=True, timeout=10)
        self.assertEqual((process.returncode, process.stdout), (1, ''))
        self.assertEqual(process.stderr, 'managed_dependency_operation_refused\n')

if __name__ == '__main__': unittest.main()
