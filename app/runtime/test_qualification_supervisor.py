"""APP-075 supervisor unit tests. No AWS, RDS or Docker; refresh is replaced
by a synthetic envelope writer and the child by a recorded fake executable."""
import hashlib
import io
import json
import os
from pathlib import Path
import signal
import stat
import sys
import tempfile
import textwrap
import threading
import time
import unittest
from unittest import mock
import uuid

import qualification_supervisor as supervisor
import runtime

RUN = "00000000-0000-4000-8000-000000000aaa"
REGISTRY_VECTOR = (
    '{"bindings":[{"issuer":"app008-fixture-only","organization_id":"00000000-0000-4000-8000-000000000001",'
    '"provider_tenant_id":"ten_00000000-0000-4000-8000-000000000001"},'
    '{"issuer":"app008-fixture-only","organization_id":"00000000-0000-4000-8000-000000000002",'
    '"provider_tenant_id":"ten_00000000-0000-4000-8000-000000000002"}],'
    '"registry_version":"00000000-0000-4000-8000-000000000aaa","schema_version":1}')
SECRET = "synthetic-secret-" + "f00d" * 4
MOUNTINFO = textwrap.dedent("""\
    101 90 0:50 / / ro,relatime master:1 - overlay overlay ro
    102 101 0:53 / /proc rw,nosuid,nodev,noexec,relatime - proc proc rw
    103 101 0:54 / /dev rw,nosuid - tmpfs tmpfs rw,size=65536k,mode=755
    110 101 254:1 /docker/containers/x/resolv.conf /etc/resolv.conf rw,relatime - ext4 /dev/vda1 rw
    111 101 254:1 /docker/containers/x/hostname /etc/hostname rw,relatime - ext4 /dev/vda1 rw
    112 101 254:1 /docker/containers/x/hosts /etc/hosts rw,relatime - ext4 /dev/vda1 rw
    113 101 254:1 /docker/volumes/v/_data /run/genui rw,relatime - ext4 /dev/vda1 rw
    """)


def envelope(now=None):
    now = int(time.time()) if now is None else now
    roles = {name: {"username": "genui_" + name, "token": "synthetic-" + name, "expires_at": now + 600} for name in runtime.ROLES}
    tenants = [{"organization_id": org, "role_arn": "arn:aws:iam::877573132739:role/t%d" % i,
                "kms_key_arn": "arn:aws:kms:us-east-1:877573132739:key/%036d" % i, "prefix": "o/" + org + "/",
                "access_key_id": "synthetic", "secret_access_key": "synthetic", "session_token": "synthetic", "expires_at": now + 600}
               for i, org in enumerate(supervisor.ORGANIZATIONS)]
    return {"schema_version": 1, "generation": str(uuid.uuid4()), "issued_at": now, "expires_at": now + 600,
            "database": {"host": supervisor.HOST, "port": 5432, "name": "genui", "roles": roles},
            "storage": {"region": "us-east-1", "bucket": "synthetic-bucket", "tenants": tenants}}


class Harness:
    """Private run directory, CA, identity, mountinfo and a fake executable."""

    def __init__(self, body="pass", ca=b"synthetic reviewed CA bytes\n"):
        self.temporary = tempfile.TemporaryDirectory(prefix="genui-app075-supervisor-")
        root = Path(self.temporary.name).resolve()
        self.run = root / "run"
        self.run.mkdir(mode=0o700)
        os.chmod(self.run, 0o700)
        self.ca = root / "ca.pem"
        self.ca.write_bytes(ca)
        os.chmod(self.ca, 0o444)
        self.ca_sha = hashlib.sha256(ca).hexdigest()
        self.record = root / "record.json"
        self.executable = root / "genui-storage-tests"
        self.executable.write_text(self.script(body))
        os.chmod(self.executable, 0o755)
        self.identity = root / "build-identity.json"
        self.identity.write_text(json.dumps({"schema_version": 1, "executable": str(supervisor.EXECUTABLE),
                                             "executable_sha256": hashlib.sha256(self.executable.read_bytes()).hexdigest()}))
        self.mountinfo = root / "mountinfo"
        self.mountinfo.write_text(MOUNTINFO.replace("/run/genui", str(self.run)))
        self.refreshes = []
        self.out = io.StringIO()

    def close(self):
        self.temporary.cleanup()

    def script(self, body):
        result = {"schema_version": 1, "passed": True, "error": None, "checks": [{"name": "synthetic", "passed": True}] * 6,
                  "cleanup_applicable": True, "cleanup_confirmed": True}
        bodies = {
            "pass": "emit(%r)\nsys.exit(0)" % json.dumps(result),
            "five": "emit(%r)\nsys.exit(0)" % json.dumps({**result, "checks": result["checks"][:5]}),
            "unpassed": "emit(%r)\nsys.exit(0)" % json.dumps({**result, "checks": result["checks"][:5] + [{"name": "x", "passed": False}]}),
            "none": "emit(%r)\nsys.exit(0)" % json.dumps({k: v for k, v in result.items() if k != "checks"}),
            "fail": "emit(%r)\nsys.exit(101)" % json.dumps({**result, "passed": False, "error": "case_refused", "checks": []}),
            "missing": "sys.exit(101)",
            "double": "emit(%r)\nemit(%r)\nsys.exit(0)" % (json.dumps(result), json.dumps(result)),
            "slow": "time.sleep(1.2)\nemit(%r)\nsys.exit(0)" % json.dumps(result),
            "hang": "time.sleep(60)",
            "flood": "sys.stderr.write('x' * (4 * 1024 * 1024))\nsys.stderr.flush()\nemit(%r)\nsys.exit(0)" % json.dumps(result),
        }
        return textwrap.dedent("""\
            #!{python}
            import json, os, stat, sys, time
            from pathlib import Path
            run = Path({run!r})
            files = {{}}
            for name in ("app008-managed-run.json", "app008-managed-registry.json", "credentials.json"):
                st = os.lstat(run / name)
                files[name] = [stat.S_IMODE(st.st_mode), (run / name).read_text()]
            Path({record!r}).write_text(json.dumps({{"argv": sys.argv[1:], "env": dict(os.environ), "files": files}}))
            run_id = json.loads(files["app008-managed-run.json"][1])["run_id"]
            def emit(text):
                value = json.loads(text)
                value["run_id"] = run_id
                sys.stdout.write("\\nAPP008_MANAGED_RESULT " + json.dumps(value) + "\\n")
                sys.stdout.flush()
            sys.stdout.write("\\nrunning 1 test\\ntest storage::managed_qualification::managed_readonly_entry ... APP008 check: x\\n")
            """).format(python=sys.executable, run=str(self.run), record=str(self.record)) + bodies[body] + "\n"

    def manifest(self, **changes):
        now = int(time.time())
        value = {"schema_version": 1, "scope": "APP-008 isolated application-data-read-only qualification", "armed": True,
                 "run_id": RUN, "issued_at": now, "expires_at": now + 600, "database_host": supervisor.HOST,
                 "database_port": 5432, "database_name": "genui", "ca_sha256": self.ca_sha}
        value.update(changes)
        return json.dumps(value)

    def environ(self, **changes):
        value = {supervisor.ARM: "APP-008", supervisor.MANIFEST_ENV: self.manifest(),
                 "GENUI_DATABASE_HOST": supervisor.HOST, "AWS_REGION": "us-east-1"}
        value.update(changes)
        return {k: v for k, v in value.items() if v is not None}

    def refresh(self, path):
        self.refreshes.append(time.time())
        runtime.atomic_write(path, envelope())

    def supervise(self, argv=("run",), environ=None, **overrides):
        options = dict(refresh=self.refresh, reviewed_ca=self.ca_sha, executable=self.executable, identity=self.identity,
                       run_directory=self.run, ca=self.ca, mountinfo=self.mountinfo, euid=os.geteuid(), uid=os.geteuid(),
                       out=self.out)
        options.update(overrides)
        self.status = supervisor.supervise(list(argv), self.environ() if environ is None else environ, **options)
        lines = self.out.getvalue().splitlines()
        summaries = [line for line in lines if line.startswith(supervisor.SUMMARY_PREFIX)]
        assert len(summaries) == 1, lines
        self.summary = json.loads(summaries[0][len(supervisor.SUMMARY_PREFIX):])
        self.results = [line for line in lines if line.startswith("APP008_MANAGED_RESULT ")]
        return self.status

    def launched(self):
        return self.record.exists()

    def run_files(self):
        return sorted(p.name for p in self.run.iterdir())


class SupervisorTests(unittest.TestCase):
    def setUp(self):
        self.harness = Harness()
        self.addCleanup(self.harness.close)

    def refused(self, harness=None, **kwargs):
        harness = harness or self.harness
        self.assertEqual(harness.supervise(**kwargs), 2)
        self.assertEqual(harness.summary["error"], "configuration_refused")
        self.assertFalse(harness.summary["launched"])
        self.assertFalse(harness.launched())
        self.assertEqual(harness.refreshes, [])
        self.assertEqual(harness.run_files(), [])
        self.assertTrue(harness.summary["files_removed"])

    def test_registry_bytes_match_the_rust_vector(self):
        self.assertEqual(supervisor.registry_bytes(RUN).decode(), REGISTRY_VECTOR)

    def test_reviewed_ca_pin_is_the_packaged_bundle(self):
        bundle = Path(__file__).resolve().parent.parent / "certs" / "rds-global-bundle.pem"
        self.assertEqual(hashlib.sha256(bundle.read_bytes()).hexdigest(), supervisor.REVIEWED_CA_SHA256)

    def test_refuses_every_other_argument_list_including_serve(self):
        for argv in ([], ["serve"], ["migrate"], ["refresh"], ["probe"], ["run", "--exact"], ["run", "run"], ["RUN"]):
            with self.subTest(argv=argv):
                harness = Harness()
                self.addCleanup(harness.close)
                self.refused(harness, argv=argv)

    def test_refuses_operator_password_and_migration_channel(self):
        for name in ("PGPASSWORD", "PGHOST", "PGPASSFILE", "PGUSER", "GENUI_MIGRATION_PASSWORD", "db_password"):
            with self.subTest(name=name):
                harness = Harness()
                self.addCleanup(harness.close)
                self.refused(harness, environ=harness.environ(**{name: SECRET}))
                self.assertNotIn(SECRET, harness.out.getvalue())

    def test_refuses_every_variable_outside_the_allowlist(self):
        for name in ("AWS_ENDPOINT_URL", "AWS_ENDPOINT_URL_STS", "AWS_CA_BUNDLE", "SSL_CERT_FILE", "SSL_CERT_DIR",
                     "REQUESTS_CA_BUNDLE", "PYTHONPATH", "PYTHONHOME", "PYTHONSTARTUP", "LD_PRELOAD", "LD_LIBRARY_PATH",
                     "AWS_CONTAINER_CREDENTIALS_FULL_URI", "AWS_CONTAINER_AUTHORIZATION_TOKEN", "AWS_PROFILE",
                     "AWS_ACCESS_KEY_ID", "HTTPS_PROXY", "GENUI_RDS_CA_FILE", "GENUI_RUNTIME_CONFIG_FILE"):
            with self.subTest(name=name):
                harness = Harness()
                self.addCleanup(harness.close)
                self.refused(harness, environ=harness.environ(**{name: "/run/genui/" + SECRET}))
                self.assertNotIn(SECRET, harness.out.getvalue())
        for name in ("AWS_CONFIG_FILE", "AWS_SHARED_CREDENTIALS_FILE"):
            with self.subTest(name=name):
                harness = Harness()
                self.addCleanup(harness.close)
                self.refused(harness, environ=harness.environ(**{name: "/run/genui/aws"}))

    def test_accepts_the_task_platform_and_image_environment(self):
        environ = self.harness.environ(
            GENUI_DATABASE_PORT="5432", GENUI_DATABASE_NAME="genui", GENUI_EVIDENCE_BUCKET="synthetic-bucket",
            GENUI_DATABASE_ROLES_JSON="{}", GENUI_TENANTS_JSON="[]", PYTHONDONTWRITEBYTECODE="1", PYTHONUNBUFFERED="1",
            AWS_CONTAINER_CREDENTIALS_RELATIVE_URI="/v2/credentials/x", AWS_DEFAULT_REGION="us-east-1",
            AWS_EXECUTION_ENV="AWS_ECS_FARGATE", ECS_CONTAINER_METADATA_URI="http://169.254.170.2/v3/x",
            ECS_CONTAINER_METADATA_URI_V4="http://169.254.170.2/v4/x", ECS_AGENT_URI="http://169.254.170.2/api/x",
            PATH="/usr/local/bin:/usr/bin:/bin", HOME="/", HOSTNAME="ip-10-0-0-1", LANG="C.UTF-8", GPG_KEY="synthetic",
            PYTHON_VERSION="3.11.14", PYTHON_SHA256="synthetic", AWS_CONFIG_FILE="/dev/null",
            AWS_SHARED_CREDENTIALS_FILE="/dev/null")
        self.assertEqual(self.harness.supervise(environ=environ), 0, self.harness.out.getvalue())
        record = json.loads(self.harness.record.read_text())
        environment = {k: v for k, v in record["env"].items() if k not in ("__CF_USER_TEXT_ENCODING", "LC_CTYPE")}
        self.assertEqual(environment, {supervisor.ARM: "APP-008"})

    def test_refuses_unarmed_injected_auth_and_wrong_uid(self):
        for changes in ({supervisor.ARM: None}, {supervisor.ARM: "app-008"}, {supervisor.ARM: "APP-075"},
                        {runtime.QUALIFICATION_JSON_ENV: "{}"}, {runtime.QUALIFICATION_FILE_ENV: "/run/genui/x"},
                        {"GENUI_DATABASE_HOST": "localhost"}, {"GENUI_DATABASE_HOST": None}):
            with self.subTest(changes=changes):
                harness = Harness()
                self.addCleanup(harness.close)
                self.refused(harness, environ=harness.environ(**changes))
        harness = Harness()
        self.addCleanup(harness.close)
        self.refused(harness, euid=0)
        self.assertEqual(supervisor.UID, 65532)

    def test_refuses_absent_open_or_out_of_bounds_manifests(self):
        now = int(time.time())
        harness = Harness()
        self.addCleanup(harness.close)
        duplicate = harness.manifest().replace('"armed": true', '"armed": true, "armed": true')
        for manifest in (None, "", "[]", "{", duplicate, "x" * 9000, harness.manifest(run_id="not-a-uuid"),
                         harness.manifest(run_id="00000000-0000-7000-8000-000000000aaa"),
                         harness.manifest(issued_at=now - 1, expires_at=now + 600),
                         harness.manifest(issued_at=now - 545, expires_at=now + 55),
                         harness.manifest(issued_at=now + 10, expires_at=now + 300),
                         harness.manifest(issued_at=str(now)), harness.manifest(armed=False),
                         harness.manifest(database_host="localhost"), harness.manifest(ca_sha256="0" * 64)):
            with self.subTest(manifest=None if manifest is None else manifest[:60]):
                harness = Harness()
                self.addCleanup(harness.close)
                self.refused(harness, environ=harness.environ(**{supervisor.MANIFEST_ENV: manifest}))

    def test_refuses_ca_executable_mount_and_preexisting_file_substitution(self):
        harness = Harness()
        self.addCleanup(harness.close)
        os.chmod(harness.ca, 0o644)
        harness.ca.write_bytes(b"other CA\n")
        self.refused(harness)
        harness = Harness()
        self.addCleanup(harness.close)
        os.chmod(harness.executable, 0o755)
        with open(harness.executable, "a") as stream:
            stream.write("# changed\n")
        self.refused(harness)
        for point in ("/etc/ssl/certs", "/etc/ssl/certs/rds-global-bundle.pem", "/etc/ssl", "/app",
                      "/app/qualification/genui-storage-tests", "/usr/local/lib/python3.11", "RUN/app008-managed-run.json",
                      "RUN/app008-managed-registry.json", "RUN/credentials.json"):
            with self.subTest(point=point):
                harness = Harness()
                self.addCleanup(harness.close)
                point = point.replace("RUN", str(harness.run))
                harness.mountinfo.write_text(MOUNTINFO + "120 101 254:1 /x %s ro - ext4 /dev/vda1 ro\n" % point)
                self.refused(harness)
        for name in (supervisor.MANIFEST_NAME, supervisor.REGISTRY_NAME, supervisor.CREDENTIALS_NAME):
            with self.subTest(name=name):
                harness = Harness()
                self.addCleanup(harness.close)
                planted = harness.run / name
                planted.write_text("planted")
                os.chmod(planted, 0o600)
                self.assertEqual(harness.supervise(), 2)
                self.assertFalse(harness.launched())
                self.assertEqual(planted.read_text(), "planted")  # never replaced or removed
        harness = Harness()
        self.addCleanup(harness.close)
        os.chmod(harness.run, 0o750)
        self.refused(harness)

    def test_launches_exact_executable_and_arguments_with_private_files_and_no_secrets(self):
        status = self.harness.supervise()
        self.assertEqual(status, 0, self.harness.out.getvalue())
        record = json.loads(self.harness.record.read_text())
        self.assertEqual(record["argv"], list(supervisor.ARGUMENTS))
        self.assertEqual(record["argv"], ["--exact", "storage::managed_qualification::managed_readonly_entry",
                                          "--ignored", "--nocapture", "--test-threads=1"])
        environment = {k: v for k, v in record["env"].items() if k not in ("__CF_USER_TEXT_ENCODING", "LC_CTYPE")}
        self.assertEqual(environment, {supervisor.ARM: "APP-008"})
        files = record["files"]
        self.assertEqual({name: mode for name, (mode, _) in files.items()},
                         {"app008-managed-run.json": 0o600, "app008-managed-registry.json": 0o600, "credentials.json": 0o600})
        self.assertEqual(files["app008-managed-registry.json"][1], REGISTRY_VECTOR)
        self.assertEqual(json.loads(files["app008-managed-run.json"][1])["run_id"], RUN)
        self.assertEqual(len(self.harness.results), 1)
        result = json.loads(self.harness.results[0][len("APP008_MANAGED_RESULT "):])
        self.assertTrue(result["passed"] and result["cleanup_confirmed"])
        summary = self.harness.summary
        self.assertEqual((summary["launched"], summary["result_retained"], summary["passed"], summary["child_exit"]), (True, True, True, 0))
        self.assertEqual(summary["run_id"], RUN)
        self.assertEqual(summary["refreshes"], 1)
        self.assertTrue(summary["files_removed"])
        self.assertEqual(self.harness.run_files(), [])
        self.assertNotIn("synthetic-workspace", self.harness.out.getvalue())

    def test_a_partly_written_run_file_is_removed(self):
        real = os.fsync
        for failing in (1, 2):
            with self.subTest(failing=failing):
                harness = Harness()
                self.addCleanup(harness.close)
                calls = []
                def fsync(fd):
                    calls.append(fd)
                    if len(calls) == failing:
                        raise OSError("synthetic fsync failure")
                    return real(fd)
                with mock.patch("os.fsync", fsync):
                    self.refused(harness)

    def test_sigterm_kills_the_child_removes_files_and_reports(self):
        harness = Harness("hang")
        self.addCleanup(harness.close)
        previous = signal.getsignal(signal.SIGTERM)
        self.addCleanup(signal.signal, signal.SIGTERM, previous)
        def stop_task():
            deadline = time.time() + 10
            while not harness.launched() and time.time() < deadline:
                time.sleep(0.05)
            os.kill(os.getpid(), signal.SIGTERM)
        sender = threading.Thread(target=stop_task)
        sender.start()
        started = time.time()
        status = harness.supervise(handle_sigterm=True)
        sender.join()
        self.assertEqual(status, 143)
        self.assertLess(time.time() - started, 10)
        self.assertEqual(harness.summary["error"], "terminated")
        self.assertTrue(harness.summary["launched"])
        self.assertFalse(harness.summary["passed"])
        self.assertTrue(harness.summary["files_removed"])
        self.assertEqual(harness.run_files(), [])
        self.assertEqual(harness.results, [])
        # The handler is restored for the caller; a second signal during
        # cleanup was ignored rather than interrupting it.
        self.assertIs(signal.getsignal(signal.SIGTERM), previous)

    def test_refreshes_until_the_executable_exits(self):
        harness = Harness("slow")
        self.addCleanup(harness.close)
        self.assertEqual(harness.supervise(refresh_interval=0.2), 0, harness.out.getvalue())
        self.assertGreaterEqual(harness.summary["refreshes"], 4)
        self.assertEqual(harness.summary["refresh_failures"], 0)

    def test_refresh_failures_are_sanitized(self):
        def failing(path):
            raise RuntimeError("token " + SECRET)
        harness = Harness()
        self.addCleanup(harness.close)
        stderr = io.StringIO()
        original, sys.stderr = sys.stderr, stderr
        try:
            self.assertEqual(harness.supervise(refresh=failing), 3)
        finally:
            sys.stderr = original
        self.assertFalse(harness.launched())
        self.assertEqual(stderr.getvalue(), "credential_refresh_unavailable\n")
        self.assertNotIn(SECRET, harness.out.getvalue())
        self.assertEqual(harness.run_files(), [])

    def test_missing_ambiguous_or_failed_results_never_pass(self):
        for body, status in (("missing", 4), ("double", 4), ("fail", 1)):
            with self.subTest(body=body):
                harness = Harness(body)
                self.addCleanup(harness.close)
                self.assertEqual(harness.supervise(), status)
                self.assertFalse(harness.summary["passed"])
                self.assertEqual(harness.run_files(), [])
        self.assertEqual(harness.summary["error"], "case_refused")

    def test_a_pass_requires_six_passed_checks(self):
        for body in ("five", "unpassed", "none"):
            with self.subTest(body=body):
                harness = Harness(body)
                self.addCleanup(harness.close)
                self.assertEqual(harness.supervise(), 1)
                self.assertFalse(harness.summary["passed"])
                self.assertEqual(harness.summary["error"], "checks_incomplete")
                self.assertEqual(harness.run_files(), [])

    def test_kills_the_child_at_the_original_deadline(self):
        harness = Harness("hang")
        self.addCleanup(harness.close)
        real = time.time
        calls = []
        def clock():
            calls.append(1)
            return real() if len(calls) <= 2 else real() + 1000
        started = real()
        self.assertEqual(harness.supervise(clock=clock), 4)
        self.assertLess(real() - started, 10)
        self.assertTrue(harness.summary["deadline_kill"])
        self.assertFalse(harness.summary["passed"])
        self.assertEqual(harness.run_files(), [])

    def test_drains_stderr_without_forwarding_it(self):
        harness = Harness("flood")
        self.addCleanup(harness.close)
        self.assertEqual(harness.supervise(), 0, harness.out.getvalue()[-400:])
        self.assertGreaterEqual(harness.summary["stderr_bytes"], 4 * 1024 * 1024)
        self.assertNotIn("xxxx", harness.out.getvalue())


if __name__ == "__main__":
    unittest.main()
