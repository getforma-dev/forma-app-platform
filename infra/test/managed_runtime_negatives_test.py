"""Local synthetic boundaries only; never managed qualification."""
import contextlib
import copy
import hashlib
import http.server
import importlib.util
import io
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time
import types
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


trial = module("runtime_negatives", ROOT / "infra/scripts/managed_runtime_negatives.py")
runtime = module("local_runtime_fixture", ROOT / "app/runtime/runtime.py")


def fixture():
    now = int(time.time())
    tenants = []
    for i in (1, 2):
        identifier = "00000000-0000-4000-8000-00000000000" + str(i)
        tenants.append(dict(organization_id=identifier, prefix="o/" + identifier + "/",
                            kms_key_arn="unit-key-" + str(i), role_arn="unit-role-" + str(i),
                            expires_at=now + 600, access_key_id="unit-access",
                            secret_access_key="unit-SECRET", session_token="unit-TOKEN"))
    return dict(schema_version=1, generation="00000000-0000-4000-8000-000000000001",
                issued_at=now, expires_at=now + 600,
                database=dict(host="unit.invalid", port=5432, name="genui", roles={
                    role: dict(username="genui_" + role, token="unit-PASSWORD-" + role,
                               expires_at=now + 600) for role in trial.ROLES}),
                storage=dict(region="us-east-1", bucket="unit-only", tenants=tenants))


class OperationalError(Exception):
    sqlstate = None


class LocalRuntime:
    Refused = runtime.Refused
    atomic_write = staticmethod(runtime.atomic_write)
    load = staticmethod(runtime.load)

    def __init__(self):
        self.calls = []
        self.document = fixture()

    def refresh(self, path):
        self.calls.append("refresh")
        self.atomic_write(path, self.document)

    def probe(self, path, generation):
        self.calls.append("probe")
        value = self.load(path)
        if value["database"]["host"] == "127.0.0.1":
            raise OperationalError("127.0.0.1 port 5432: connection refused unit-SECRET")
        assert value["generation"] == generation
        return dict(generation=generation, file_sha256=value["_file_sha256"],
                    expires_at=value["expires_at"], checked_at=int(time.time()))


class LocalNative:
    instances = []

    def __init__(self, path):
        self.path = path
        self.original = path.read_bytes()
        self.closed = False
        self.instances.append(self)

    def listen(self):
        pass

    def status(self):
        return 200 if self.path.exists() and self.path.read_bytes() == self.original else 503

    ready = status

    def close(self):
        self.closed = True


class RuntimeNegatives(unittest.TestCase):
    def setUp(self):
        LocalNative.instances = []
        self.local = LocalRuntime()
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.report = {"checks": []}
        self.alarm = patch.object(trial.signal, "alarm")
        self.alarm.start()
        self.addCleanup(self.alarm.stop)
        self.psycopg = patch.dict(sys.modules, {"psycopg": types.SimpleNamespace(OperationalError=OperationalError)})
        self.psycopg.start()
        self.addCleanup(self.psycopg.stop)
        # Exercise sequencing uses an ephemeral local reservation; only the
        # managed runner targets fixed5432. No unit test claims that deployment.
        class Reservation:
            def __init__(self, *_args):
                self.actual = socket.socket()

            def bind(self, address):
                assert address == ("127.0.0.1", 5432)
                self.actual.bind(("127.0.0.1", 0))

            def close(self):
                self.actual.close()

        self.sockets = patch.object(trial, "socket", types.SimpleNamespace(
            AF_INET=socket.AF_INET, SOCK_STREAM=socket.SOCK_STREAM, socket=Reservation))
        self.sockets.start()
        self.addCleanup(self.sockets.stop)

    def run_exercise(self):
        trial.exercise(self.local, self.directory.name, self.report, LocalNative)

    def test_full_local_sequence_preserves_original_and_never_exports_secrets(self):
        self.run_exercise()
        self.assertEqual(len(self.report["checks"]), 5)
        self.assertEqual(self.local.calls, ["refresh"] + ["probe"] * 9)
        self.assertTrue(all(check["passed"] for check in self.report["checks"]))
        self.assertTrue(LocalNative.instances[0].closed)
        for name in ("baseline.json", "active.json"):
            path = Path(self.directory.name) / name
            trial.private_file(path)
            value = self.local.load(path)
            del value["_file_sha256"]
            self.assertEqual(value, self.local.document)
        output = json.dumps(self.report)
        for fragment in ("PASSWORD", "SECRET", "TOKEN", "unit.invalid", "127.0.0.1", "generation"):
            self.assertNotIn(fragment, output)

    def test_each_mutation_only_changes_its_one_field_and_keeps_credentials(self):
        original = fixture()
        before = copy.deepcopy(original)
        self.assertIsNone(trial.candidate(original, "missing_file"))
        for name, group, field, expected in (
            ("wrong_storage_region", "storage", "region", "us-west-2"),
            ("unreachable_database", "database", "host", "127.0.0.1"),
        ):
            changed = trial.candidate(original, name)
            self.assertEqual(changed[group][field], expected)
            changed[group][field] = original[group][field]
            self.assertEqual(changed, original)
        changed = trial.candidate(original, "expired_file")
        self.assertLess(changed.pop("expires_at"), original["issued_at"])
        self.assertEqual(changed, {key: value for key, value in original.items() if key != "expires_at"})
        self.assertEqual(original, before)
        with self.assertRaises(trial.Refused):
            trial.candidate(original, "caller_selected_case")

    def test_unexpected_negative_success_fails_restores_and_stops_child(self):
        with patch.object(trial, "helper_negative", side_effect=trial.Refused):
            with self.assertRaises(trial.Refused):
                self.run_exercise()
        self.assertFalse(self.report["checks"][-1]["passed"])
        self.assertTrue(LocalNative.instances[0].closed)
        self.assertEqual((Path(self.directory.name) / "baseline.json").read_bytes(),
                         (Path(self.directory.name) / "active.json").read_bytes())

    def test_interrupt_after_mutation_also_restores_and_stops(self):
        with patch.object(trial, "helper_negative", side_effect=KeyboardInterrupt):
            with self.assertRaises(KeyboardInterrupt):
                self.run_exercise()
        self.assertTrue(LocalNative.instances[0].closed)
        self.assertEqual((Path(self.directory.name) / "baseline.json").read_bytes(),
                         (Path(self.directory.name) / "active.json").read_bytes())

    def test_failed_positive_restoration_never_advances_or_passes(self):
        original = trial.helper_positive
        count = 0

        def positive(*args):
            nonlocal count
            count += 1
            if count > 1:
                raise RuntimeError("unit-SECRET")
            return original(*args)

        with patch.object(trial, "helper_positive", side_effect=positive):
            with self.assertRaises(RuntimeError):
                self.run_exercise()
        self.assertEqual(len(self.report["checks"]), 2)
        self.assertFalse(self.report["checks"][-1]["passed"])
        self.assertTrue(LocalNative.instances[0].closed)

    def test_refusal_types_are_specific_and_arbitrary_errors_never_pass(self):
        path = Path(self.directory.name) / "missing.json"
        for error in (self.local.Refused("unit-secret"), RuntimeError("unit-secret")):
            with patch.object(self.local, "probe", side_effect=error):
                with self.assertRaises(trial.Refused):
                    trial.helper_negative(self.local, path, "unit", "missing_file")
        with patch.object(self.local, "probe", return_value={}):
            with self.assertRaises(trial.Refused):
                trial.helper_negative(self.local, path, "unit", "missing_file")
        for diagnostic in ("TLS refused", "password authentication failed", "timeout"):
            with patch.object(self.local, "probe", side_effect=OperationalError(diagnostic)):
                with self.assertRaises(trial.Refused):
                    trial.helper_negative(self.local, path, "unit", "unreachable_database")

    def test_loopback_port_is_reserved_without_listening(self):
        with socket.socket() as reserved:
            reserved.bind(("127.0.0.1", 0))
            with socket.socket() as client:
                client.settimeout(0.2)
                with self.assertRaises((ConnectionRefusedError, TimeoutError)):
                    client.connect(reserved.getsockname())

    def test_existing_loopback_service_refuses_case_without_connecting_to_it(self):
        original_socket = socket.socket

        class Occupied:
            def bind(self, _address):
                raise OSError("unit-SECRET")

            def close(self):
                pass

        # First three cases complete; refusal occurs before the fourth mutation.
        with patch.object(trial.socket, "socket", return_value=Occupied()):
            with self.assertRaises(OSError):
                self.run_exercise()
        self.assertEqual(len(self.report["checks"]), 5)
        self.assertFalse(self.report["checks"][-1]["passed"])
        self.assertTrue(LocalNative.instances[0].closed)
        self.assertIs(socket.socket, original_socket)

    def test_private_permissions_refuse_links_and_public_modes(self):
        directory = Path(self.directory.name)
        trial.private_directory(directory)
        path = directory / "file.json"
        self.local.atomic_write(path, {})
        trial.private_file(path)
        linked = directory / "linked.json"
        linked.symlink_to(path)
        with self.assertRaises(trial.Refused):
            trial.private_file(linked)
        path.chmod(0o644)
        with self.assertRaises(trial.Refused):
            trial.private_file(path)
        directory.chmod(0o755)
        with self.assertRaises(trial.Refused):
            trial.private_directory(directory)
        directory.chmod(0o700)

    def test_child_forces_loopback_helper_and_refuses_optional_auth(self):
        with patch.dict(os.environ, {"GENUI_HELPER_PATH": "caller.py", "FORMA_PORT": "3000",
                                     "FORMA_BIND_ADDRESS": "0.0.0.0"}, clear=True):
            env = trial.child_environment(Path("/unit/private.json"))
            self.assertEqual(env["FORMA_PORT"], "0")
            self.assertEqual(env["FORMA_BIND_ADDRESS"], "127.0.0.1")
            self.assertEqual(env["GENUI_HELPER_PATH"], "/app/runtime.py")
            self.assertEqual(os.environ["FORMA_PORT"], "3000")
            for suffix in ("FILE", "JSON", "UNRECOGNIZED"):
                with patch.dict(os.environ, {"GENUI_AUTH_QUALIFICATION_CONFIG_" + suffix: ""}):
                    with self.assertRaises(trial.Refused):
                        trial.child_environment(Path("/unit/private.json"))

    def test_main_refusal_is_fixed_and_performs_no_import_refresh_or_filesystem_write(self):
        for environment, uid in (({}, 65532), ({"GENUI_QUALIFY_SYNTHETIC": "APP-007"}, 0),
                                 ({"GENUI_QUALIFY_SYNTHETIC": "APP-007",
                                   "GENUI_AUTH_QUALIFICATION_CONFIG_JSON": "unit-SECRET"}, 65532)):
            output = io.StringIO()
            with patch.dict(os.environ, environment, clear=True), patch.object(os, "geteuid", return_value=uid), \
                    patch.object(trial, "private_directory") as filesystem, contextlib.redirect_stdout(output):
                self.assertEqual(trial.main(), 1)
                filesystem.assert_not_called()
            report = json.loads(output.getvalue())
            self.assertFalse(report["passed"])
            self.assertEqual(report["failure"], "assertion_or_interrupt")
            self.assertNotIn("unit-SECRET", output.getvalue())

    def test_native_close_kills_real_child_that_ignores_termination(self):
        child = subprocess.Popen([sys.executable, "-c", "import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); print('ready',flush=True); time.sleep(60)"],
                                 stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, start_new_session=True)
        native = object.__new__(trial.Native)
        native.process = child
        self.assertEqual(child.stdout.readline(), b"ready\n")
        started = time.monotonic()
        native.close()
        self.assertEqual(child.returncode, -signal.SIGKILL)
        self.assertLess(time.monotonic() - started, 6)

    def test_actual_loopback_http_status_rejects_redirect_and_body(self):
        replies = [(503, b""), (200, b""), (302, b""), (200, b"unexpected")]

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                self.server.paths.append(self.path)
                status, body = replies.pop(0)
                self.send_response(status)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *_args):
                pass

        server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
        server.paths = []
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        try:
            native = object.__new__(trial.Native)
            native.port = server.server_port
            native.process = types.SimpleNamespace(poll=lambda: None)
            self.assertEqual(native.status(), 503)
            self.assertEqual(native.status(), 200)
            for _ in range(2):
                with self.assertRaises(trial.Refused):
                    native.status()
            self.assertEqual(server.paths, ["/health/ready"] * 4)
        finally:
            server.shutdown()
            server.server_close()
            worker.join(timeout=2)

    def test_main_cleans_real_temp_files_after_success_and_interrupt(self):
        original_temporary_directory = tempfile.TemporaryDirectory
        directories = []
        self.local.ROLES = trial.ROLES
        spec = types.SimpleNamespace(loader=types.SimpleNamespace(exec_module=lambda _module: None))

        def temporary_directory(**kwargs):
            self.assertEqual(kwargs["dir"], "/run/genui")
            value = original_temporary_directory(prefix=kwargs["prefix"])
            directories.append(Path(value.name))
            return value

        for interrupt in (False, True):
            def exercise(local, directory, report):
                path = Path(directory) / "credentials.json"
                path.write_text("unit-SECRET")
                path.chmod(0o600)
                if interrupt:
                    raise KeyboardInterrupt("unit-SECRET")

            output = io.StringIO()
            with patch.dict(os.environ, {"GENUI_QUALIFY_SYNTHETIC": "APP-007"}, clear=True), \
                    patch.object(os, "geteuid", return_value=65532), \
                    patch.object(trial, "private_directory"), \
                    patch.object(trial.Path, "read_bytes", return_value=b"fixture"), \
                    patch.object(trial, "RUNTIME_SHA256", hashlib.sha256(b"fixture").hexdigest()), \
                    patch.object(trial.importlib.util, "spec_from_file_location", return_value=spec), \
                    patch.object(trial.importlib.util, "module_from_spec", return_value=self.local), \
                    patch.object(trial.tempfile, "TemporaryDirectory", side_effect=temporary_directory), \
                    patch.object(trial, "exercise", side_effect=exercise), \
                    contextlib.redirect_stdout(output):
                self.assertEqual(trial.main(), 1 if interrupt else 0)
            result = json.loads(output.getvalue())
            self.assertTrue(result["private_files_removed"])
            self.assertEqual(result["passed"], not interrupt)
            self.assertNotIn("unit-SECRET", output.getvalue())
        self.assertTrue(all(not path.exists() for path in directories))


if __name__ == "__main__":
    unittest.main()
