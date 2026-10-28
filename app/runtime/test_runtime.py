"""Unit-only failure-path checks. No AWS/RDS calls or managed acceptance."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import tempfile
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


if __name__ == '__main__': unittest.main()
