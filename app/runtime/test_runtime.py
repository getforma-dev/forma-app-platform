"""Unit-only failure-path checks. No AWS/RDS calls or managed acceptance."""
import copy
import contextlib
import io
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import signal
import stat
import subprocess
import sys
import threading
import time
import unittest
from unittest.mock import patch
import uuid
import runtime


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
        with patch.dict(os.environ, {"GENUI_RDS_CA_FILE": "/unit-only/ca.pem"}), patch('psycopg.connect', return_value=Connection()) as connect, patch.object(runtime, 'scoped_session') as storage:
            with self.assertRaises(runtime.Refused): runtime.probe(self.path, self.value["generation"])
            self.assertEqual(connect.call_args.kwargs['sslmode'], 'verify-full')
            self.assertEqual(connect.call_args.kwargs['user'], 'genui_workspace')
            storage.assert_not_called()

    def test_rds_superuser_membership_refused_even_with_false_superuser_flags(self):
        class Connection:
            def __enter__(self): return self
            def __exit__(self, *args): pass
            def execute(self, *args): return self
            def fetchone(self):
                return ('genui_workspace','genui_workspace','genui',False,False,False,False,True,False,True,True,1)
            def fetchall(self): return [('rds_superuser',)]
        with patch.dict(os.environ, {'GENUI_RDS_CA_FILE':'/unit-only/ca.pem'}), patch('psycopg.connect',return_value=Connection()), patch.object(runtime,'scoped_session') as storage:
            with self.assertRaises(runtime.Refused): runtime.probe(self.path,self.value['generation'])
            storage.assert_not_called()

    def test_generation_change_refused_before_network(self):
        with patch('psycopg.connect') as connect:
            with self.assertRaises(runtime.Refused): runtime.probe(self.path, str(uuid.uuid4()))
            connect.assert_not_called()


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

    def test_transport_consumption_byte_limit_and_no_duplicate_auth_policy(self):
        for raw in (self.PAYLOAD, '\u00e9' * 4096, '{invalid-json-for-rust-to-refuse'):
            with patch.dict(os.environ, {runtime.QUALIFICATION_JSON_ENV: raw}, clear=True):
                self.assertEqual(runtime.take_qualification_json('serve'), raw.encode('utf-8'))
                self.assertNotIn(runtime.QUALIFICATION_JSON_ENV, os.environ)
        for raw in ('', ' \n ', 'x' * 8193, '\u00e9' * 4097, '\udcff'):
            with self.subTest(kind=len(raw)), patch.dict(os.environ, {runtime.QUALIFICATION_JSON_ENV: raw}, clear=True):
                with self.assertRaises(runtime.Refused) as error:
                    runtime.take_qualification_json('serve')
                self.assertEqual(str(error.exception), 'runtime configuration refused')
                self.assertNotIn(runtime.QUALIFICATION_JSON_ENV, os.environ)
        with patch.dict(os.environ, {}, clear=True):
            self.assertIsNone(runtime.take_qualification_json('serve'))
        with patch.dict(os.environ, {runtime.QUALIFICATION_JSON_ENV: self.PAYLOAD, runtime.QUALIFICATION_FILE_ENV: ''}, clear=True):
            with self.assertRaises(runtime.Refused): runtime.take_qualification_json('serve')
            self.assertNotIn(runtime.QUALIFICATION_JSON_ENV, os.environ)

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

    def test_failed_preparation_does_not_refresh_launch_or_delete_existing_credentials(self):
        self.path.write_text('existing-preserve')
        self.directory.chmod(0o755)
        with patch.dict(os.environ, {runtime.QUALIFICATION_JSON_ENV: self.PAYLOAD}, clear=True), \
             patch.object(runtime.signal, 'signal'), patch.object(runtime, 'refresh') as refresh, patch.object(runtime.subprocess, 'Popen') as child:
            with self.assertRaises(runtime.Refused): runtime.serve(self.path)
            refresh.assert_not_called(); child.assert_not_called()
            self.assertNotIn(runtime.QUALIFICATION_JSON_ENV, os.environ)
        self.directory.chmod(0o700)
        self.assertEqual(self.path.read_text(), 'existing-preserve')
        self.assertEqual(self.auth_files(), [])

    def child_script(self, exit_code=0, wait_for_termination=False):
        script = self.directory / 'child.py'
        report = self.directory / 'child-report.json'
        source = f'''#!{sys.executable}
import json, os, signal, stat, time
from pathlib import Path
if {wait_for_termination!r}: signal.signal(signal.SIGTERM, signal.SIG_IGN)
path = os.environ.get({runtime.QUALIFICATION_FILE_ENV!r})
info = None if path is None else Path(path).stat()
content_ok = path is None or Path(path).read_text() == {self.PAYLOAD!r}
result = {{'raw_env_absent': {runtime.QUALIFICATION_JSON_ENV!r} not in os.environ,
           'path_present': path is not None, 'content_ok': content_ok,
           'mode': None if info is None else stat.S_IMODE(info.st_mode),
           'owned': info is None or info.st_uid == os.geteuid()}}
Path({str(report)!r}).write_text(json.dumps(result))
if {wait_for_termination!r}:
    while True: time.sleep(0.05)
raise SystemExit({exit_code})
'''
        script.write_text(source); script.chmod(0o700)
        return script, report

    def test_real_successful_and_failed_children_receive_only_private_path(self):
        for exit_code in (0, 7):
            with self.subTest(exit_code=exit_code):
                script, report = self.child_script(exit_code)
                env = {runtime.QUALIFICATION_JSON_ENV: self.PAYLOAD, 'GENUI_BINARY': str(script), 'GENUI_RUNTIME_CONFIG_FILE': str(self.path)}
                with patch.dict(os.environ, env, clear=True), patch.object(runtime.signal, 'signal'), \
                     patch.object(runtime, 'refresh', side_effect=lambda path: runtime.atomic_write(path, self.value)) as refresh:
                    self.assertEqual(runtime.serve(self.path), exit_code)
                    self.assertNotIn(runtime.QUALIFICATION_JSON_ENV, os.environ)
                    self.assertNotIn(runtime.QUALIFICATION_FILE_ENV, os.environ)
                    refresh.assert_called_once()
                self.assertEqual(json.loads(report.read_text()), {'raw_env_absent': True, 'path_present': True, 'content_ok': True, 'mode': 0o600, 'owned': True})
                self.assertEqual(self.auth_files(), [])
                self.assertFalse(self.path.exists())

    def test_absent_json_preserves_default_and_explicit_path_is_never_owned(self):
        for explicit in (False, True):
            with self.subTest(explicit=explicit):
                script, report = self.child_script()
                existing = self.directory / 'explicit.json'
                existing.write_text(self.PAYLOAD); existing.chmod(0o600)
                env = {'GENUI_BINARY': str(script), 'GENUI_RUNTIME_CONFIG_FILE': str(self.path)}
                if explicit: env[runtime.QUALIFICATION_FILE_ENV] = str(existing)
                with patch.dict(os.environ, env, clear=True), patch.object(runtime.signal, 'signal'), \
                     patch.object(runtime, 'refresh', side_effect=lambda path: runtime.atomic_write(path, self.value)):
                    self.assertEqual(runtime.serve(self.path), 0)
                value = json.loads(report.read_text())
                self.assertTrue(value['raw_env_absent'])
                self.assertEqual(value['path_present'], explicit)
                self.assertEqual(existing.read_text(), self.PAYLOAD)
                self.assertEqual(self.auth_files(), [])

    def test_startup_failure_and_cancel_after_write_clean_file_without_secret_output(self):
        for failure in ('refresh', 'launch', 'interrupt'):
            with self.subTest(failure=failure):
                stop = threading.Event()
                output = io.StringIO()
                def refresh(path):
                    self.assertNotIn(runtime.QUALIFICATION_JSON_ENV, os.environ)
                    self.assertEqual(len(self.auth_files()), 1)
                    if failure == 'refresh':
                        stop.set(); raise OSError(self.PAYLOAD)
                    if failure == 'interrupt': raise KeyboardInterrupt()
                    runtime.atomic_write(path, self.value)
                def launch(*args, **kwargs):
                    self.assertNotIn(runtime.QUALIFICATION_JSON_ENV, kwargs['env'])
                    stop.set(); raise OSError(self.PAYLOAD)
                with patch.dict(os.environ, {runtime.QUALIFICATION_JSON_ENV: self.PAYLOAD}, clear=True), \
                     patch.object(runtime.signal, 'signal'), patch.object(runtime.threading, 'Event', return_value=stop), \
                     patch.object(runtime, 'refresh', side_effect=refresh), patch.object(runtime.subprocess, 'Popen', side_effect=launch), \
                     contextlib.redirect_stderr(output):
                    if failure == 'interrupt':
                        with self.assertRaises(KeyboardInterrupt): runtime.serve(self.path)
                    else: self.assertEqual(runtime.serve(self.path), 0)
                self.assertNotIn(self.PAYLOAD, output.getvalue())
                self.assertEqual(self.auth_files(), [])

    def test_real_cancelled_child_ignoring_term_is_killed_and_cleanup_is_bounded(self):
        script, report = self.child_script(wait_for_termination=True)
        stop = threading.Event()
        def cancel_when_started():
            deadline = time.monotonic() + 3
            while not report.exists() and time.monotonic() < deadline: time.sleep(0.01)
            stop.set()
        watcher = threading.Thread(target=cancel_when_started, daemon=True)
        env = {runtime.QUALIFICATION_JSON_ENV: self.PAYLOAD, 'GENUI_BINARY': str(script), 'GENUI_RUNTIME_CONFIG_FILE': str(self.path)}
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

    def test_actual_nonserve_commands_refuse_injection_before_managed_operations(self):
        for mode in ('refresh', 'probe', 'migrate', 'qualify', 'cleanup'):
            with self.subTest(mode=mode):
                process = subprocess.run([sys.executable, str(Path(runtime.__file__).resolve()), mode],
                                         env={runtime.QUALIFICATION_JSON_ENV: self.PAYLOAD},
                                         capture_output=True, text=True, timeout=3)
                self.assertEqual(process.returncode, 1)
                self.assertEqual(process.stdout, '')
                self.assertEqual(process.stderr, 'managed_dependency_operation_refused\n')
                self.assertNotIn(self.PAYLOAD, process.stdout + process.stderr)


if __name__ == '__main__': unittest.main()
