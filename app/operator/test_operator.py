"""APP-077 operator package tests. No AWS, registry or RDS use.

PackageTests and S3Tests need only the operator lock's packages; S3Tests drive
the real boto3 client against local_s3.Stub on loopback. AdapterTests run the
real parser-host and WASM guest when a local parser build is named by
GENUI_OPERATOR_TEST_PARSER (a directory holding release/parser-host and
wasm32-unknown-unknown/release/parser_guest.wasm, i.e. parser/target), else
skip. DatabaseTests start one disposable PostgreSQL 16.13 container with a
throwaway TLS CA and drive the real command line as genui_migration over
verify-full TLS; they skip without Docker and OpenSSL and remove the container.

Run: python -m unittest discover -s app/operator -v (with the operator lock installed).
"""
import base64
import contextlib
import copy
import hashlib
import io
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock
import uuid

HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[1]
SCRIPT = HERE / "operator.py"
spec = importlib.util.spec_from_file_location("genui_operator", SCRIPT)
op = importlib.util.module_from_spec(spec)
spec.loader.exec_module(op)
sys.path.insert(0, str(HERE))
import local_s3  # noqa: E402

NORTHSTAR, CANARY = "00000000-0000-4000-8000-000000000001", "00000000-0000-4000-8000-000000000002"
KEYS = {NORTHSTAR: "arn:aws:kms:us-east-1:123456789012:key/00000000-0000-4000-8000-0000000000a1",
        CANARY: "arn:aws:kms:us-east-1:123456789012:key/00000000-0000-4000-8000-0000000000a2"}
BUCKET = "genui-app077-local-stub"
# The OperatorRole grants of infra/src/stack.ts, mirrored for the stub (review M1,
# m3): kms:GenerateDataKey and kms:Decrypt on each tenant key (both bound to S3
# and that tenant's o/<tenant>/s/* encryption context), and PutObject only with
# If-None-Match: *. infra/test/security.test.ts asserts the same set.
OPERATOR_KMS = ("kms:Decrypt", "kms:GenerateDataKey")
SEEDED_REVISION = "00000000-0000-4000-8000-00000000006f"
REGISTRATION_KEYS = ["content_digest", "contract_digest", "normalized_digest", "object_digest", "organization_id", "revision_id",
                     "row_count", "source_id"]
POSTGRES = "postgres@sha256:5d143123fdf80462d1778cd4f24b9f7ca13c87174bca19141fb194c5a1ebca59"


def tenants_json(keys=KEYS):
    return json.dumps([{"organization_id": org, "kms_key_arn": arn, "prefix": "o/%s/" % org} for org, arn in keys.items()])


def cli(*args, env_extra=None, clear_pg=True, timeout=120):
    env = {k: v for k, v in os.environ.items() if not (clear_pg and k.startswith(("PG", "GENUI_", "AWS_")))}
    env.update(env_extra or {})
    return subprocess.run([sys.executable, "-I", str(SCRIPT), *args], capture_output=True, text=True, timeout=timeout, env=env,
                          cwd=tempfile.gettempdir())


def refusal(completed):
    lines = completed.stderr.strip().splitlines()
    return json.loads(lines[-1])["refused"] if lines else None


def synthetic_plan(original=b"event_id,asset\nE1,a\n", normalized=b"PAR1-synthetic"):
    source, revision = "00000000-0000-4000-8000-00000000006e", "00000000-0000-4000-8000-00000000006f"
    return [{"organization_id": NORTHSTAR, "source_id": source, "revision_id": revision, "content_digest": op.sha256(original),
             "normalized_digest": op.sha256(normalized),
             "objects": [{"name": name, "key": op.object_key(NORTHSTAR, source, revision, name), "sha256": op.sha256(data),
                          "bytes": len(data), "content_type": op.CONTENT_TYPES[name], "data": data}
                         for name, data in (("original", original), ("part.parquet", normalized))]}]


class PackageTests(unittest.TestCase):
    def test_subcommands_are_closed(self):
        self.assertEqual(op.COMMANDS, ("migrate", "verify-catalog", "seed", "admission", "upload-sources"))
        for argv in ([], ["sql", "SELECT 1"], ["migrate", "--exact"], ["seed", "extra"], ["verify-catalog", "--sql", "x"],
                     ["upload-sources", "--key", "o/x"], ["admission"], ["serve"], ["refresh"], ["--help"], ["migrate", "--help"]):
            with self.subTest(argv=argv), self.assertRaises(op.Refused):
                op.parse(argv)
        self.assertEqual(op.parse(["upload-sources", "--plan"]).plan, True)
        self.assertEqual(op.parse(["admission", "list", "--organization", NORTHSTAR]).rest, ["list", "--organization", NORTHSTAR])

    def test_refusals_print_fixed_codes_and_never_echo_values(self):
        secret = "app077-not-a-password-" + uuid.uuid4().hex
        for args, env, code in (
            (["drop", secret], {}, "usage"),
            (["migrate"], {"PGHOST": "/var/run/" + secret, "PGUSER": "genui_migration", "PGSSLROOTCERT": "/x", "PGPASSWORD": secret}, "socket_host_refused"),
            (["seed"], {"PGHOST": "localhost", "PGUSER": "genui_workspace", "PGSSLROOTCERT": "/x", "PGPASSWORD": secret}, "migration_identity_required"),
            (["verify-catalog"], {"PGHOST": "localhost", "PGUSER": "genui_migration", "PGDATABASE": secret, "PGSSLROOTCERT": "/x"}, "database_refused"),
            (["verify-catalog"], {"PGHOST": "localhost", "PGUSER": "genui_migration", "PGSSLMODE": "disable", "PGSSLROOTCERT": "/x"}, "tls_mode_refused"),
            (["verify-catalog"], {"PGHOST": "localhost", "PGUSER": "genui_migration", "PGPASSWORD": secret}, "ca_required"),
            (["upload-sources"], {"GENUI_EVIDENCE_BUCKET": BUCKET, "GENUI_OPERATOR_TENANTS_JSON": secret}, "tenants_refused"),
            (["upload-sources"], {"GENUI_EVIDENCE_BUCKET": BUCKET, "GENUI_OPERATOR_TENANTS_JSON": tenants_json(), "AWS_REGION": "us-east-1",
                                  "AWS_ENDPOINT_URL_S3": "http://" + secret}, "aws_override_refused"),
            (["upload-sources"], {"GENUI_EVIDENCE_BUCKET": "Bad_" + secret}, "bucket_refused"),
        ):
            with self.subTest(code=code):
                completed = cli(*args, env_extra=env)
                self.assertEqual(completed.returncode, 1)
                self.assertEqual(completed.stdout, "")
                self.assertEqual(refusal(completed), code)
                self.assertNotIn(secret, completed.stderr)

    def test_database_environment_is_checked_before_any_connection(self):
        with mock.patch.dict(os.environ, {"PGHOST": "localhost", "PGUSER": "genui_admin", "PGSSLROOTCERT": "/x"}, clear=False), \
                mock.patch("psycopg.connect", side_effect=AssertionError("connected")):
            with self.assertRaises(op.Refused):
                op.connect()

    def test_documented_overrides_fit_ecs_and_match_the_entry_point(self):
        document = json.loads(op.OVERRIDES_FILE.read_text())
        self.assertEqual(document["entryPoint"], ["python3", "-I", "-u", "/operator/app/operator/operator.py"])
        self.assertEqual(document["container"], "operator")
        self.assertEqual(document["max_override_bytes"], 8192)
        names = [item["name"] for item in document["overrides"]]
        self.assertEqual(len(names), len(set(names)))
        covered, admission_covered = set(), set()
        import admission_operator
        for item in document["overrides"]:
            with self.subTest(name=item["name"]):
                overrides = item["overrides"]
                self.assertEqual(list(overrides), ["containerOverrides"])
                self.assertEqual(len(overrides["containerOverrides"]), 1)
                container = overrides["containerOverrides"][0]
                self.assertEqual(set(container), {"name", "command"})  # command only: no environment, no secrets
                self.assertEqual(container["name"], "operator")
                self.assertLessEqual(len(json.dumps(overrides, separators=(",", ":")).encode()), 8192)
                args = op.parse(container["command"])
                covered.add(args.command)
                if args.command == "admission":
                    parsed = admission_operator.validate(admission_operator.parser().parse_args(args.rest))
                    self.assertNotIn(parsed.command, op.MANAGED_REFUSED)
                    admission_covered.add(parsed.command)
        self.assertEqual(covered, set(op.COMMANDS))
        self.assertEqual(op.parse(document["default_command"]).command, "verify-catalog")
        self.assertEqual([item["command"][:2] for item in document["not_ecs_overrides"]][0], ["admission", "issue"])
        # Every admission subcommand is either documented (review m4) or refused in a managed task (M3).
        excluded = {item["command"][1] for item in document["not_ecs_overrides"]}
        self.assertEqual(excluded, set(op.MANAGED_REFUSED))
        self.assertEqual(admission_covered | excluded, set(admission_operator.parser()._subparsers._group_actions[0].choices))
        self.assertEqual(admission_covered & excluded, set())

    def test_issue_and_reconcile_refuse_in_a_managed_task(self):
        # Review M3: a Fargate task's stdout is the awslogs group; the raw code must never reach it.
        org = NORTHSTAR
        for variable in ("ECS_CONTAINER_METADATA_URI_V4", "ECS_CONTAINER_METADATA_URI"):
            for argv in (["admission", "issue", "--organization", org, "--issued-by", "ops", "--grant", "x"],
                         ["admission", "issue", "--organization", org, "--issued-by", "ops", "--grant", "x", "--provider-email", "a@b.test"],
                         ["admission", "reconcile", "--organization", org]):
                with self.subTest(variable=variable, argv=argv[:3]):
                    completed = cli(*argv, env_extra={variable: "http://169.254.170.2/v4/x", "PGHOST": "db.example.test",
                                                      "PGUSER": "genui_migration", "PGSSLROOTCERT": str(SCRIPT)})
                    self.assertEqual((completed.returncode, completed.stdout, refusal(completed)), (1, "", "refused_in_managed_task"))
        # The guard reads the subcommand as admission_operator's own parser does; a form that parser
        # does not accept as `issue` never reaches it.
        completed = cli("admission", "--", "issue", "--organization", org, "--issued-by", "ops",
                        env_extra={"ECS_CONTAINER_METADATA_URI_V4": "x"})
        self.assertEqual((completed.returncode, completed.stdout, refusal(completed)), (1, "", "usage"))
        # Outside a managed task the guard does not apply (the identity check refuses next, before any connection).
        completed = cli("admission", "reconcile", "--organization", org, env_extra={"PGHOST": "localhost", "PGUSER": "genui_workspace",
                                                                                  "PGSSLROOTCERT": str(SCRIPT)})
        self.assertEqual(refusal(completed), "migration_identity_required")
        # A documented admission command still runs in a managed task (here it reaches the host check).
        completed = cli("admission", "list", "--organization", org, env_extra={"ECS_CONTAINER_METADATA_URI_V4": "x", "PGHOST": "@abstract"})
        self.assertEqual(refusal(completed), "socket_host_refused")

    def test_admission_checks_the_database_environment_before_connecting(self):
        # Review m1: the passthrough runs the same pre-connect identity checks as every database command.
        for environment, code in (({"PGHOST": "localhost", "PGUSER": "genui_workspace", "PGDATABASE": "postgres", "PGSSLROOTCERT": str(SCRIPT)},
                                   "migration_identity_required"),
                                  ({"PGHOST": "localhost", "PGUSER": "genui_migration", "PGDATABASE": "postgres", "PGSSLROOTCERT": str(SCRIPT)},
                                   "database_refused"),
                                  ({"PGHOST": "db.example.test,/var/run/postgresql", "PGUSER": "genui_migration", "PGSSLROOTCERT": str(SCRIPT)},
                                   "host_refused")):
            with self.subTest(code=code), mock.patch.dict(os.environ, environment, clear=True), \
                    mock.patch("psycopg.connect", side_effect=AssertionError("connected")) as connect, \
                    contextlib.redirect_stderr(io.StringIO()) as stderr:
                self.assertEqual(op.main(["admission", "list", "--organization", NORTHSTAR]), 1)
                self.assertEqual(json.loads(stderr.getvalue().strip().splitlines()[-1]), {"refused": code})
                connect.assert_not_called()
        # `defaults` needs no database and no environment.
        with mock.patch.dict(os.environ, {}, clear=True), contextlib.redirect_stdout(io.StringIO()) as stdout:
            self.assertEqual(op.main(["admission", "defaults"]), 0)
        self.assertIn("code_expiry_days_max", json.loads(stdout.getvalue()))

    def test_host_and_ca_guards_refuse_non_tls_and_unpinned_trust(self):
        # Review m2: libpq treats '@' as an abstract Unix socket and ',' as a host list; sslmode does not apply to sockets.
        base = {"PGUSER": "genui_migration", "PGSSLROOTCERT": str(SCRIPT)}
        for host, code in (("/var/run/postgresql", "socket_host_refused"), ("@abstract", "socket_host_refused"),
                           ("db.example.test,/var/run/postgresql", "host_refused"), ("db.example.test,@x", "host_refused"),
                           ("db example.test", "host_refused"), ("-db.example.test", "host_refused"), ("db.example.test.", "host_refused"),
                           ("[::1]", "host_refused"), ("", "socket_host_refused")):
            with self.subTest(host=host), mock.patch.dict(os.environ, dict(base, PGHOST=host), clear=True), self.assertRaises(op.Refused) as refused:
                op.database_environment()
            self.assertEqual(str(refused.exception), code)
        for ca in ("system", "relative/ca.pem", str(HERE), str(HERE / "missing.pem")):
            with self.subTest(ca=ca), mock.patch.dict(os.environ, dict(base, PGHOST="db.example.test", PGSSLROOTCERT=ca), clear=True), \
                    self.assertRaises(op.Refused) as refused:
                op.database_environment()
            self.assertEqual(str(refused.exception), "ca_refused")
        with mock.patch.dict(os.environ, dict(base, PGHOST="genui.abc123.us-east-1.rds.amazonaws.com"), clear=True):
            self.assertEqual(op.database_environment(), ("genui.abc123.us-east-1.rds.amazonaws.com", 5432))
        # Inside a managed task only the image's pinned RDS bundle is a trust root.
        with mock.patch.dict(os.environ, dict(base, PGHOST="db.example.test", ECS_CONTAINER_METADATA_URI_V4="x"), clear=True), \
                self.assertRaises(op.Refused) as refused:
            op.database_environment()
        self.assertEqual(str(refused.exception), "ca_refused")
        self.assertEqual(op.PINNED_CA, "/etc/ssl/certs/rds-global-bundle.pem")

    def test_upload_revision_is_derived_and_never_the_seeded_placeholder(self):
        # Review M2 (coordinated decision): a new revision id, uuid5 over the organization id and the
        # original content digest, stamped version 4 so the contract's v4/v7 identifier shape accepts it.
        digest = op.SOURCES[0]["original_digest"]
        revision = op.derived_revision_id(NORTHSTAR, digest)
        self.assertEqual(revision, str(uuid.UUID(bytes=uuid.uuid5(uuid.UUID(NORTHSTAR), digest).bytes, version=4)))
        self.assertEqual(revision, op.derived_revision_id(NORTHSTAR, digest))
        self.assertIsNotNone(op.UUID.fullmatch(revision))
        self.assertNotEqual(revision, SEEDED_REVISION)
        self.assertNotEqual(revision, op.derived_revision_id(CANARY, digest))
        self.assertNotEqual(revision, op.derived_revision_id(NORTHSTAR, "sha256:" + "0" * 64))
        for bad in (("not-a-uuid", digest), (NORTHSTAR, "sha256:" + "0" * 63), (NORTHSTAR, digest.upper())):
            with self.subTest(bad=bad), self.assertRaises(op.Refused):
                op.derived_revision_id(*bad)

    def test_dockerfile_entry_point_and_default_command_match(self):
        text = (HERE / "Dockerfile").read_text()
        document = json.loads(op.OVERRIDES_FILE.read_text())
        self.assertIn("ENTRYPOINT " + json.dumps(document["entryPoint"]), text)
        self.assertIn("CMD " + json.dumps(document["default_command"]), text)
        self.assertIn("USER 65532:65532", text)
        for line in text.splitlines():
            if line.startswith("FROM ") and " AS inputs" not in line:
                self.assertRegex(line, r"@sha256:[0-9a-f]{64}( AS \w+)?$")

    def test_requirements_lock_is_the_verbatim_union_of_reviewed_locks(self):
        def blocks(path):
            result, current = {}, None
            for line in Path(path).read_text().splitlines():
                if not line.strip() or line.startswith("#"):
                    continue
                if not line.startswith(" "):
                    current = line.split("==")[0]
                    result[current] = []
                result[current].append(line)
            return result
        lock = blocks(HERE / "requirements.lock")
        union = {}
        for path in ("app/runtime/requirements.lock", "contracts/requirements.lock", "spikes/APP-005/requirements.lock"):
            for name, block in blocks(REPOSITORY / path).items():
                self.assertEqual(union.setdefault(name, block), block, name)
        self.assertEqual(lock, union)
        for name, block in lock.items():
            self.assertRegex(block[0], r"^[a-z0-9-]+==[0-9][0-9a-z.]* \\$")
            self.assertTrue(all(re.fullmatch(r"    --hash=sha256:[0-9a-f]{64}( \\)?", line) for line in block[1:]), name)
        self.assertIn("duckdb", lock)

    def test_dockerignore_is_an_exact_allowlist_of_tracked_files(self):
        sys.path.insert(0, str(HERE))
        import check_image
        names = check_image.allowlist()
        tracked = set(subprocess.run(["git", "ls-files", "--cached", "--others", "--exclude-standard"], cwd=REPOSITORY,
                                     capture_output=True, text=True, check=True).stdout.splitlines())
        self.assertEqual(sorted(set(names) - tracked), [])
        parser = sorted(name for name in tracked if name.startswith("parser/"))
        self.assertEqual(sorted(name for name in names if name.startswith("parser/")), parser)
        for required in ("app/operator/operator.py", "app/runtime/runtime.py", "app/runtime/tenant_seed.py", "app/runtime/admission_operator.py",
                         "app/runtime/provider_management.py", "contracts/validate.py", "contracts/schema.json", "fixtures/qualify.py",
                         "fixtures/verify.py", "fixtures/phase-one/v1/sources/northstar-events.csv", "app/migrations/manifest.json",
                         "app/certs/rds-global-bundle.pem", "app/operator/requirements.lock"):
            self.assertIn(required, names)
        for excluded in ("app/operator/test_operator.py", "app/operator/local_s3.py", "app/operator/check_image.py",
                         "app/runtime/qualification_supervisor.py", "app/runtime/fake_fauth.py"):
            self.assertNotIn(excluded, names)
        migrations = sorted("app/migrations/" + p.name for p in (REPOSITORY / "app/migrations").iterdir())
        self.assertEqual(sorted(name for name in names if name.startswith("app/migrations/")), migrations)
        for source in check_image.dockerfile_sources():
            self.assertTrue(source in names or any(name.startswith(source.rstrip("/") + "/") for name in names), source)

    def test_object_keys_are_the_adr_009_b1_layout(self):
        source, revision = str(uuid.uuid4()), str(uuid.uuid4())
        self.assertEqual(op.object_key(NORTHSTAR, source, revision, "original"), "o/%s/s/%s/r/%s/original" % (NORTHSTAR, source, revision))
        self.assertEqual(op.object_key(NORTHSTAR, source, revision, "part.parquet"), "o/%s/s/%s/r/%s/part.parquet" % (NORTHSTAR, source, revision))
        for bad in (("../x", source, revision, "original"), (NORTHSTAR, source, revision, "inspection.json"),
                    (source.upper(), source, revision, "original"), (NORTHSTAR, source + "/", revision, "original")):
            with self.subTest(bad=bad), self.assertRaises(op.Refused):
                op.object_key(*bad)

    def test_tenant_configuration_is_exact(self):
        self.assertEqual(sorted(op.load_tenants(tenants_json())), [NORTHSTAR, CANARY])
        good = json.loads(tenants_json())
        for mutate in (lambda v: v[0].update(role_arn="arn:aws:iam::123456789012:role/x"), lambda v: v[0].update(prefix="o/"),
                       lambda v: v[0].update(kms_key_arn="alias/x"), lambda v: v.append(dict(v[0])), lambda v: v.clear(),
                       lambda v: v[0].update(organization_id="not-a-uuid")):
            value = copy.deepcopy(good)
            mutate(value)
            with self.subTest(value=str(value)[:60]), self.assertRaises(op.Refused):
                op.load_tenants(json.dumps(value))
        with self.assertRaises(op.Refused):
            op.load_tenants(None)

    def test_seeded_source_is_the_northstar_events_revision(self):
        source, contract_digest = op.seeded_source(op.SOURCES[0])
        self.assertEqual((source["source_id"], source["revision_id"]), ("00000000-0000-4000-8000-00000000006e", "00000000-0000-4000-8000-00000000006f"))
        self.assertEqual(source["input_contract"]["contract_id"], "events_v1")
        self.assertEqual(contract_digest, "sha256:b92d4b96754e5e9de1863dff09e615880ded8d57de22e7546b1f59f2f9178f64")

    def test_fixture_bytes_are_pinned(self):
        item, path, data = op.fixture_entry(op.SOURCES[0])
        self.assertEqual(op.sha256(data), op.SOURCES[0]["original_digest"])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "v1"
            shutil.copytree(op.FIXTURES, root)
            target = root / item["path"]
            target.write_bytes(data.replace(b"60000", b"60001", 1))
            with self.assertRaises(op.Refused) as refused:
                op.fixture_entry(op.SOURCES[0], root)
            self.assertEqual(str(refused.exception), "fixture_digest_refused")


def operator_stub(kms_actions=OPERATOR_KMS):
    return local_s3.Stub(BUCKET, {"o/%s/" % org: arn for org, arn in KEYS.items()},
                         kms_grants={arn: set(kms_actions) for arn in KEYS.values()}, require_if_none_match=True)


class S3Tests(unittest.TestCase):
    def stub(self, kms_actions=OPERATOR_KMS):
        return operator_stub(kms_actions)

    def test_upload_writes_b1_keys_with_sse_kms_under_the_tenant_key(self):
        plan = synthetic_plan()
        with self.stub() as stub:
            receipts = op.upload(stub.client(), BUCKET, op.load_tenants(tenants_json()), plan)
            puts = stub.puts()
        self.assertEqual([p["key"] for p in puts], [item["key"] for item in plan[0]["objects"]])  # original first
        for put, item in zip(puts, plan[0]["objects"]):
            self.assertEqual(put["headers"]["x-amz-server-side-encryption"], "aws:kms")
            self.assertEqual(put["headers"]["x-amz-server-side-encryption-aws-kms-key-id"], KEYS[NORTHSTAR])
            self.assertEqual(put["headers"]["x-amz-checksum-sha256"], base64.b64encode(hashlib.sha256(item["data"]).digest()).decode())
            self.assertTrue(put["headers"]["authorization"].startswith("AWS4-HMAC-SHA256 "))
            self.assertEqual(put["headers"]["if-none-match"], "*")  # conditional: a revision key is written once
            self.assertEqual("sha256:" + put["body_sha256"], item["sha256"])
        objects = receipts[0]["objects"]
        self.assertEqual([o["state"] for o in objects], ["created", "created"])
        self.assertTrue(all(o["version_id"] and o["kms_key_arn"] == KEYS[NORTHSTAR] for o in objects))
        self.assertEqual(receipts[0]["normalized_digest"], plan[0]["normalized_digest"])
        self.assertNotIn("data", json.dumps(receipts))

    def test_rerun_leaves_equal_objects_unchanged(self):
        plan = synthetic_plan()
        with self.stub() as stub:
            client, tenants = stub.client(), op.load_tenants(tenants_json())
            first = op.upload(client, BUCKET, tenants, plan)
            again = op.upload(client, BUCKET, tenants, plan)
            # The rerun's conditional PUTs answer 412 and store no second version.
            self.assertEqual({key: len(versions) for key, versions in stub.objects.items()}, {item["key"]: 1 for item in plan[0]["objects"]})
        self.assertEqual([o["state"] for o in again[0]["objects"]], ["unchanged", "unchanged"])
        self.assertEqual([o["version_id"] for o in again[0]["objects"]], [o["version_id"] for o in first[0]["objects"]])

    def test_an_existing_different_object_refuses_and_is_not_overwritten(self):
        with self.stub() as stub:
            client, tenants = stub.client(), op.load_tenants(tenants_json())
            op.upload(client, BUCKET, tenants, synthetic_plan())
            with self.assertRaises(op.Refused) as refused:
                op.upload(client, BUCKET, tenants, synthetic_plan(original=b"other bytes\n"))
            self.assertEqual(str(refused.exception), "existing_object_differs")
            self.assertEqual({key: len(versions) for key, versions in stub.objects.items()},
                             {item["key"]: 1 for item in synthetic_plan()[0]["objects"]})
            self.assertEqual(stub.objects[synthetic_plan()[0]["objects"][0]["key"]][0]["body"], synthetic_plan()[0]["objects"][0]["data"])

    def test_a_partial_upload_completes_on_rerun(self):
        plan = synthetic_plan()
        with self.stub() as stub:
            client, tenants = stub.client(), op.load_tenants(tenants_json())
            stub.fail_puts.add(2)  # the part.parquet PUT fails once
            with self.assertRaises(op.Refused) as refused:
                op.upload(client, BUCKET, tenants, plan)
            self.assertEqual(str(refused.exception), "object_write_refused")
            self.assertEqual(sorted(stub.objects), [plan[0]["objects"][0]["key"]])  # the original only
            again = op.upload(client, BUCKET, tenants, plan)
        self.assertEqual([o["state"] for o in again[0]["objects"]], ["unchanged", "created"])

    def test_a_rerun_without_kms_decrypt_refuses(self):
        # Review M1: a checksum-mode HeadObject of an SSE-KMS object needs kms:Decrypt. With only
        # kms:GenerateDataKey (the grant before the fix) the first run succeeds and every rerun refuses.
        plan = synthetic_plan()
        with self.stub(kms_actions=("kms:GenerateDataKey",)) as stub:
            client, tenants = stub.client(), op.load_tenants(tenants_json())
            first = op.upload(client, BUCKET, tenants, plan)
            self.assertEqual([o["state"] for o in first[0]["objects"]], ["created", "created"])
            with self.assertRaises(op.Refused) as refused:
                op.upload(client, BUCKET, tenants, plan)
            self.assertEqual(str(refused.exception), "object_write_refused")
            heads = [r for r in stub.requests if r["method"] == "HEAD"]
            self.assertEqual(heads[0]["headers"].get("x-amz-checksum-mode"), "ENABLED")
        with self.stub(kms_actions=("kms:Decrypt",)) as stub:  # and no write without GenerateDataKey
            with self.assertRaises(op.Refused):
                op.upload(stub.client(), BUCKET, op.load_tenants(tenants_json()), plan)
            self.assertEqual(stub.objects, {})

    def test_an_unconditional_put_is_denied(self):
        # Review m3: OperatorRole allows PutObject only with s3:if-none-match = *.
        from botocore.exceptions import ClientError
        with self.stub() as stub:
            key = synthetic_plan()[0]["objects"][0]["key"]
            with self.assertRaises(ClientError) as denied:
                stub.client().put_object(Bucket=BUCKET, Key=key, Body=b"x", ServerSideEncryption="aws:kms", SSEKMSKeyId=KEYS[NORTHSTAR])
            self.assertEqual(denied.exception.response["Error"]["Code"], "AccessDenied")
            self.assertEqual(stub.objects, {})

    def test_the_bucket_policy_denies_another_tenants_key(self):
        swapped = {NORTHSTAR: KEYS[CANARY], CANARY: KEYS[NORTHSTAR]}
        with self.stub() as stub:
            with self.assertRaises(op.Refused) as refused:
                op.upload(stub.client(), BUCKET, op.load_tenants(tenants_json(swapped)), synthetic_plan())
            self.assertEqual(str(refused.exception), "object_write_refused")
            self.assertEqual(stub.objects, {})

    def test_a_tenant_without_configuration_refuses_before_any_request(self):
        with self.stub() as stub:
            with self.assertRaises(op.Refused) as refused:
                op.upload(stub.client(), BUCKET, op.load_tenants(tenants_json({CANARY: KEYS[CANARY]})), synthetic_plan())
            self.assertEqual(str(refused.exception), "tenant_missing")
            self.assertEqual(stub.requests, [])


def local_parser():
    root = os.environ.get("GENUI_OPERATOR_TEST_PARSER")
    if not root:
        return None
    host, wasm = Path(root) / "release/parser-host", Path(root) / "wasm32-unknown-unknown/release/parser_guest.wasm"
    return op.ParserBinding.from_files(host, wasm) if host.is_file() and wasm.is_file() else None


@unittest.skipUnless(local_parser(), "set GENUI_OPERATOR_TEST_PARSER to a local parser/target build")
class AdapterTests(unittest.TestCase):
    def test_plan_normalizes_through_the_parser_and_records_real_digests(self):
        import duckdb
        plans = op.plan_sources(local_parser())
        self.assertEqual(len(plans), 1)
        plan = plans[0]
        original, normalized = (item["data"] for item in plan["objects"])
        self.assertEqual(plan["content_digest"], op.sha256(original))
        self.assertEqual(plan["content_digest"], op.SOURCES[0]["original_digest"])
        self.assertEqual(plan["normalized_digest"], op.sha256(normalized))
        placeholder = plan["seeded_placeholder_not_written"]
        self.assertEqual(placeholder["revision_id"], SEEDED_REVISION)
        self.assertNotEqual(plan["normalized_digest"], placeholder["normalized_digest"])
        self.assertNotEqual(plan["content_digest"], placeholder["content_digest"])
        # Review M2: the objects go to a new revision derived from the original, never the placeholder's keys.
        revision = op.derived_revision_id(NORTHSTAR, plan["content_digest"])
        self.assertEqual(plan["revision_id"], revision)
        self.assertEqual([item["key"] for item in plan["objects"]],
                         [op.object_key(NORTHSTAR, plan["source_id"], revision, name) for name in ("original", "part.parquet")])
        self.assertFalse(any(SEEDED_REVISION in item["key"] for item in plan["objects"]))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "part.parquet"
            path.write_bytes(normalized)
            connection = duckdb.connect()
            columns = [row[0] for row in connection.execute("DESCRIBE SELECT * FROM read_parquet(?)", [str(path)]).fetchall()]
            rows = connection.execute("SELECT time, destination, bytes FROM read_parquet(?)", [str(path)]).fetchall()
            # APP-011's oracle: the half-open S02 window [10:00, 10:10) totals 1,140,000 bytes.
            window = connection.execute("SELECT sum(bytes) FROM read_parquet(?) WHERE time >= TIMESTAMP '2026-10-22 10:00:00' "
                                        "AND time < TIMESTAMP '2026-10-22 10:10:00'", [str(path)]).fetchone()[0]
            compression = {row[0] for row in connection.execute("SELECT compression FROM parquet_metadata(?)", [str(path)]).fetchall()}
            connection.close()
        expected = json.loads((op.FIXTURES / "expected.json").read_text())
        self.assertEqual(columns, ["time", "destination", "bytes"])
        self.assertEqual(len(rows), 6)
        self.assertEqual(rows, sorted(rows, key=lambda row: row[0]))
        self.assertEqual(str(window), expected["northstar"]["total_bytes"])
        self.assertEqual(compression, {"ZSTD"})

    def test_plan_bytes_are_deterministic(self):
        first, second = op.public(op.plan_sources(local_parser())), op.public(op.plan_sources(local_parser()))
        self.assertEqual(first, second)

    def test_a_parser_reader_disagreement_refuses(self):
        binding = local_parser()
        real = binding.run

        def tampered(source, scratch):
            report = real(source, scratch)
            for cell in report["result"]["result"]["cells"]:
                if cell["original"] == "300000":
                    cell["original"] = "300001"
            return report
        with mock.patch.object(binding, "run", side_effect=tampered), self.assertRaises(op.Refused) as refused:
            op.plan_sources(binding)
        self.assertEqual(str(refused.exception), "parser_reader_disagreement")

    def test_a_different_guest_refuses(self):
        binding = local_parser()
        binding.guest_sha256 = "sha256:" + "0" * 64
        with self.assertRaises(op.Refused) as refused:
            op.plan_sources(binding)
        self.assertEqual(str(refused.exception), "parser_binding_refused")

    def test_upload_sources_prints_one_registration_record(self):
        # Review M2: exactly the record APP-012's register-source-revision consumes, after both objects are stored.
        with operator_stub() as stub, \
                mock.patch.dict(os.environ, {"GENUI_EVIDENCE_BUCKET": BUCKET, "GENUI_OPERATOR_TENANTS_JSON": tenants_json()}), \
                mock.patch.object(op.ParserBinding, "from_manifest", return_value=local_parser()), \
                mock.patch.object(op, "s3_client", return_value=stub.client()):
            for _ in range(2):  # a rerun prints the same record and stores nothing new
                with contextlib.redirect_stdout(io.StringIO()) as stdout:
                    self.assertEqual(op.main(["upload-sources"]), 0)
                lines = stdout.getvalue().splitlines()
                self.assertEqual(len(lines), 1)
                record = json.loads(lines[0])
                self.assertEqual(sorted(record), REGISTRATION_KEYS)
                stored = {key: versions[-1]["body"] for key, versions in stub.objects.items()}
                self.assertEqual({key: len(versions) for key, versions in stub.objects.items()}, {key: 1 for key in stored})
        plan = op.plan_sources(local_parser())[0]
        self.assertEqual(record, {"organization_id": NORTHSTAR, "source_id": "00000000-0000-4000-8000-00000000006e",
                                  "revision_id": op.derived_revision_id(NORTHSTAR, plan["content_digest"]),
                                  "content_digest": op.SOURCES[0]["original_digest"], "normalized_digest": plan["normalized_digest"],
                                  "object_digest": plan["normalized_digest"], "row_count": 6,
                                  "contract_digest": "sha256:b92d4b96754e5e9de1863dff09e615880ded8d57de22e7546b1f59f2f9178f64"})
        self.assertEqual(sorted(stored), sorted(item["key"] for item in plan["objects"]))
        self.assertEqual(op.sha256(stored[plan["objects"][1]["key"]]), record["object_digest"])

    def test_planned_objects_upload_to_the_stub(self):
        plans = op.plan_sources(local_parser())
        with operator_stub() as stub:
            receipts = op.upload(stub.client(), BUCKET, op.load_tenants(tenants_json()), plans)
            stored = {key: versions[-1]["body"] for key, versions in stub.objects.items()}
        self.assertEqual({item["key"]: item["data"] for item in plans[0]["objects"]}, stored)
        self.assertEqual(receipts[0]["normalized_digest"], plans[0]["normalized_digest"])


def run(args, check=True, **kw):
    completed = subprocess.run(args, capture_output=True, text=True, timeout=120, **kw)
    if check and completed.returncode:
        raise RuntimeError("local fixture command failed: " + " ".join(args[:3]) + ": " + completed.stderr[-800:])
    return completed


@unittest.skipUnless(shutil.which("docker") and shutil.which("openssl"), "requires local Docker and OpenSSL")
class DatabaseTests(unittest.TestCase):
    """Fresh PostgreSQL 16.13 with TLS and the bare migration identity; tests run in name order."""

    @classmethod
    def setUpClass(cls):
        import psycopg
        cls.psycopg = psycopg
        cls.directory = tempfile.mkdtemp(prefix="genui-app077-")
        cls.name = "genui-app077-operator-" + uuid.uuid4().hex[:12]
        cls.started = False
        try:
            d = cls.directory
            run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", "ca.key", "-out", "ca.pem", "-days", "1",
                 "-subj", "/CN=APP077 operator test CA", "-addext", "basicConstraints=critical,CA:TRUE"], cwd=d)
            run(["openssl", "req", "-newkey", "rsa:2048", "-nodes", "-keyout", "server.key", "-out", "server.csr", "-subj", "/CN=localhost"], cwd=d)
            Path(d, "server.ext").write_text("subjectAltName=DNS:localhost\nbasicConstraints=critical,CA:FALSE\n"
                                             "keyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n")
            run(["openssl", "x509", "-req", "-in", "server.csr", "-CA", "ca.pem", "-CAkey", "ca.key", "-CAcreateserial",
                 "-out", "server.pem", "-days", "1", "-extfile", "server.ext"], cwd=d)
            run(["docker", "run", "--detach", "--rm", "--name", cls.name, "--publish", "127.0.0.1::5432", "--memory=512m", "--cpus=1",
                 "--tmpfs=/var/lib/postgresql/data:rw,noexec,nosuid,size=256m", "--env=POSTGRES_HOST_AUTH_METHOD=trust", POSTGRES])
            cls.started = True
            cls.port = int(run(["docker", "port", cls.name, "5432/tcp"]).stdout.strip().split(":")[1])
            for _ in range(80):
                if run(["docker", "exec", cls.name, "pg_isready", "--host=127.0.0.1", "--username=postgres"], check=False).returncode == 0:
                    break
                time.sleep(.25)
            else:
                raise RuntimeError("local PostgreSQL startup deadline")
            for f in ("server.key", "server.pem"):
                run(["docker", "cp", str(Path(d, f)), cls.name + ":/var/lib/postgresql/" + f])
            run(["docker", "exec", "--user=0", cls.name, "chown", "postgres:postgres", "/var/lib/postgresql/server.key", "/var/lib/postgresql/server.pem"])
            run(["docker", "exec", "--user=0", cls.name, "chmod", "600", "/var/lib/postgresql/server.key"])
            with cls.connect("postgres", "postgres", autocommit=True) as c:
                assert c.execute("SHOW server_version").fetchone()[0].split()[0] == "16.13"
                c.execute("ALTER SYSTEM SET ssl='on'")
                c.execute("ALTER SYSTEM SET ssl_cert_file='/var/lib/postgresql/server.pem'")
                c.execute("ALTER SYSTEM SET ssl_key_file='/var/lib/postgresql/server.key'")
                c.execute("SELECT pg_reload_conf()")
                c.execute("CREATE ROLE genui_migration LOGIN INHERIT CREATEDB CREATEROLE NOSUPERUSER NOBYPASSRLS")
                c.execute("CREATE ROLE rds_iam NOLOGIN")
                c.execute("GRANT rds_iam TO genui_migration WITH ADMIN TRUE")
                c.execute("CREATE DATABASE genui OWNER genui_migration")
        except BaseException:
            cls.tearDownClass()
            raise

    @classmethod
    def tearDownClass(cls):
        if cls.started:
            run(["docker", "rm", "--force", "--volumes", cls.name], check=False)
            cls.started = False
        shutil.rmtree(cls.directory, ignore_errors=True)

    @classmethod
    def connect(cls, role="postgres", database="genui", autocommit=False):
        # Fixture-side inspection only; the command under test uses verify-full TLS.
        return cls.psycopg.connect(host="127.0.0.1", port=cls.port, dbname=database, user=role, sslmode="disable",
                                   connect_timeout=3, autocommit=autocommit)

    def operator(self, *args, expect=0, user="genui_migration"):
        completed = cli(*args, env_extra={"PGHOST": "localhost", "PGPORT": str(self.port), "PGUSER": user, "PGDATABASE": "genui",
                                          "PGSSLROOTCERT": str(Path(self.directory, "ca.pem"))})
        self.assertEqual(completed.returncode, expect, completed.stderr[-400:])
        if expect == 0:
            self.assertEqual(completed.stdout.count("\n"), 1)
            return json.loads(completed.stdout)
        self.assertEqual(completed.stdout, "")
        return refusal(completed)

    def query(self, statement, params=()):
        with self.connect() as c:
            return c.execute(statement, params).fetchall()

    def dump(self):
        raw = run(["docker", "exec", self.name, "pg_dump", "--username=postgres", "--dbname=genui", "--data-only"]).stdout
        return "\n".join(x for x in raw.splitlines() if not x.startswith(("\\restrict ", "\\unrestrict ")))

    def test_1_verify_catalog_refuses_before_migration_and_seed_refuses_too(self):
        self.assertEqual(self.operator("verify-catalog", expect=1), "catalog_refused")
        self.assertEqual(self.operator("seed", expect=1), "catalog_refused")
        self.assertEqual(self.query("SELECT count(*) FROM pg_namespace WHERE nspname LIKE 'genui%%'"), [(0,)])

    def test_2_migrate_applies_every_landed_migration_and_a_repeat_is_unchanged(self):
        sys.path.insert(0, str(REPOSITORY / "app/runtime"))
        import runtime
        manifest = runtime.migration_manifest(runtime.migration_directory())
        out = self.operator("migrate")
        self.assertEqual(out["database_version"], manifest["database_version"])
        self.assertEqual(out["migrations"], [{"version": m["version"], "sha256": m["sha256"]} for m in manifest["migrations"]])
        history = self.query("SELECT version,sha256 FROM genui_meta.schema_migrations ORDER BY version")
        self.assertEqual(history, [(m["version"], m["sha256"]) for m in manifest["migrations"]])
        before = self.dump()
        self.assertEqual(self.operator("migrate"), out)
        self.assertEqual(self.dump(), before)
        self.assertEqual(self.query("SELECT version,sha256 FROM genui_meta.schema_migrations ORDER BY version"), history)
        self.assertEqual(self.operator("verify-catalog"), {"command": "verify-catalog", "catalog_version": manifest["database_version"], "read_only": True})

    def test_3_verify_catalog_refuses_drift_and_writes_nothing(self):
        with self.connect() as c:
            c.execute("CREATE TABLE genui.app077_drift (x int)")
        try:
            self.assertEqual(self.operator("verify-catalog", expect=1), "catalog_refused")
            self.assertEqual(self.operator("seed", expect=1), "catalog_refused")
        finally:
            with self.connect() as c:
                c.execute("DROP TABLE genui.app077_drift")
        self.assertEqual(self.operator("verify-catalog")["read_only"], True)

    def test_4_a_seed_that_meets_a_changed_row_refuses_whole_and_writes_nothing(self):
        # The canary organization is seeded after Northstar; a differing row there
        # must roll back every earlier insert of the same transaction.
        with self.connect() as c:
            c.execute("INSERT INTO genui.organizations VALUES (%s,'Changed by operator','active')", (CANARY,))
        before = self.dump()
        self.assertEqual(self.operator("seed", expect=1), "seed_refused")
        self.assertEqual(self.dump(), before)
        self.assertEqual(self.query("SELECT count(*) FROM genui.organizations WHERE organization_id=%s", (NORTHSTAR,)), [(0,)])
        # Restore the exact seeded value: an organization row cannot be deleted
        # once audit_events may reference it.
        with self.connect() as c:
            c.execute("UPDATE genui.organizations SET display_name='Synthetic organization 0002' WHERE organization_id=%s", (CANARY,))

    def test_5_seed_is_exact_match_idempotent(self):
        out = self.operator("seed")
        self.assertEqual(out["bundles"], 3)
        self.assertEqual(out["organizations"], [NORTHSTAR, CANARY])
        before = self.dump()
        self.assertEqual(self.operator("seed"), out)
        self.assertEqual(self.dump(), before)
        self.assertEqual(self.query("SELECT organization_id::text FROM genui.organizations ORDER BY 1"), [(NORTHSTAR,), (CANARY,)])

    def test_6_admission_passes_through_to_the_operator_channel(self):
        out = self.operator("admission", "backfill", "--organization", NORTHSTAR)
        self.assertEqual(sorted(out["created"]), ["admission", "control", "data", "operator", "workspace"])
        self.assertEqual(self.operator("admission", "verify", "--organization", NORTHSTAR)["verified"], True)
        self.assertEqual(self.operator("admission", "list", "--organization", NORTHSTAR)["codes"], [])
        # admission_operator's own closed parser and refusals apply unchanged.
        self.assertEqual(self.operator("admission", "sql", "SELECT 1", expect=1), "usage")
        self.assertEqual(self.operator("verify-catalog")["read_only"], True)

    def test_7_another_identity_is_refused_before_connecting(self):
        self.assertEqual(self.operator("migrate", user="postgres", expect=1), "migration_identity_required")
        self.assertEqual(self.operator("seed", user="genui_workspace", expect=1), "migration_identity_required")


if __name__ == "__main__":
    unittest.main()
