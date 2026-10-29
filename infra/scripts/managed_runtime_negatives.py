"""Opt-in APP-007 one-off; never service startup, auth, or business acceptance.

Root binds this source to the reviewed 8f8c34b app task/image, using entryPoint
["python3", "-c"] and this complete source as its sole command argument.
Importing has no I/O. Only main() refreshes credentials or contacts managed stores.
"""
import copy
import hashlib
import http.client
import importlib.util
import json
import logging
import os
from pathlib import Path
import re
import selectors
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import time


RUNTIME_SHA256 = "83df42285e43af81118b055a29a0b2f2e99aea1d37d6a7ead5fa7354c38a84d4"
ROLES = ("workspace", "control", "data", "worker")
CASES = ("missing_file", "wrong_storage_region", "expired_file", "unreachable_database")


class Refused(Exception):
    """Fixed failure classification; never carries diagnostic data."""


def require(condition):
    if not condition:
        raise Refused()


def interrupted(_signum, _frame):
    raise Refused()


def private_directory(path):
    metadata = Path(path).lstat()
    require(stat.S_ISDIR(metadata.st_mode) and metadata.st_uid == os.geteuid()
            and stat.S_IMODE(metadata.st_mode) == 0o700)


def private_file(path):
    metadata = Path(path).lstat()
    require(stat.S_ISREG(metadata.st_mode) and metadata.st_uid == os.geteuid()
            and stat.S_IMODE(metadata.st_mode) == 0o600 and metadata.st_nlink == 1)


def child_environment(path):
    # Keep task identity/config for the real helper, but permit no optional auth
    # configuration, injected helper/binary, logging override, or public listener.
    require(not any(key.startswith("GENUI_AUTH_QUALIFICATION_") for key in os.environ))
    env = dict(os.environ)
    env.update(GENUI_RUNTIME_CONFIG_FILE=str(path), GENUI_HELPER_PATH="/app/runtime.py",
               GENUI_PYTHON=sys.executable, FORMA_BIND_ADDRESS="127.0.0.1", FORMA_PORT="0",
               RUST_LOG="off", PYTHONDONTWRITEBYTECODE="1")
    return env


class Native:
    def __init__(self, path):
        self.process = subprocess.Popen(["/app/genui"], env=child_environment(path),
                                        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                        start_new_session=True)
        self.port = None

    def listen(self):
        # Read only a bounded startup marker; never print/copy native diagnostics.
        deadline = time.monotonic() + 5
        output = b""
        with selectors.DefaultSelector() as selector:
            selector.register(self.process.stdout, selectors.EVENT_READ)
            while time.monotonic() < deadline:
                require(self.process.poll() is None)
                if not selector.select(max(0.001, deadline - time.monotonic())):
                    continue
                part = os.read(self.process.stdout.fileno(), 4097 - len(output))
                require(part)
                output += part
                require(len(output) <= 4096)
                if b"\n" in output:
                    marker = output.split(b"\n", 1)[0]
                    match = re.fullmatch(rb"FORMA_LISTENING http://127\.0\.0\.1:([0-9]{1,5})", marker)
                    require(match is not None)
                    self.port = int(match[1])
                    require(0 < self.port <= 65535)
                    return
        raise Refused()

    def status(self):
        require(self.process.poll() is None and self.port is not None)
        connection = http.client.HTTPConnection("127.0.0.1", self.port, timeout=1)
        try:
            connection.request("GET", "/health/ready")
            response = connection.getresponse()
            require(response.status in (200, 503) and response.read(1) == b"")
            return response.status
        finally:
            connection.close()

    def ready(self):
        # Existing native loop retries after15s and bounds each real helper25s.
        # This local wait is subordinate to the entire trial's110s work alarm.
        deadline = time.monotonic() + 27
        while time.monotonic() < deadline:
            status = self.status()
            if status == 200:
                return status
            time.sleep(0.1)
        raise Refused()

    def close(self):
        # The process group includes any in-flight native readiness helper.
        # Kill the group even if its leader already exited; never leave a probe.
        try:
            try:
                os.killpg(self.process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                pass
        finally:
            try:
                os.killpg(self.process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            self.process.wait(timeout=2)
            self.process.stdout.close()


def candidate(baseline, name):
    require(name in CASES)
    if name == "missing_file":
        return None
    value = copy.deepcopy(baseline)
    if name == "wrong_storage_region":
        value["storage"]["region"] = "us-west-2"
    elif name == "expired_file":
        value["expires_at"] = value["issued_at"] - 1
    else:
        value["database"]["host"] = "127.0.0.1"
    return value


def helper_positive(runtime, path, baseline):
    value = runtime.load(path)
    require(value == baseline)
    result = runtime.probe(path, baseline["generation"])
    require(result["generation"] == baseline["generation"]
            and result["file_sha256"] == baseline["_file_sha256"]
            and result["expires_at"] == baseline["expires_at"]
            and value["issued_at"] <= result["checked_at"] <= int(time.time())
            and runtime.load(path) == baseline)


def helper_negative(runtime, path, generation, name):
    require(name in CASES)
    try:
        runtime.probe(path, generation)
    except FileNotFoundError:
        require(name == "missing_file" and not path.exists())
    except runtime.Refused:
        require(name in ("wrong_storage_region", "expired_file"))
    except Exception as error:
        import psycopg
        # A reserved, non-listening loopback socket prevents another service from
        # receiving the original credentials. Require actual connection refusal,
        # not an arbitrary authentication/TLS/DNS error. Never export the text.
        require(name == "unreachable_database" and isinstance(error, psycopg.OperationalError)
                and getattr(error, "sqlstate", None) is None)
        detail = str(error).lower()
        require("connection refused" in detail and "127.0.0.1" in detail and "5432" in detail)
    else:
        raise Refused()


def exercise(runtime, directory, report, native_factory=Native):
    baseline_path = Path(directory) / "baseline.json"
    active_path = Path(directory) / "active.json"
    native = None
    try:
        runtime.refresh(baseline_path)
        private_file(baseline_path)
        baseline = runtime.load(baseline_path)
        require(baseline["expires_at"] > int(time.time()) + 120)
        document = {key: value for key, value in baseline.items() if key != "_file_sha256"}
        runtime.atomic_write(active_path, document)
        private_file(active_path)
        helper_positive(runtime, active_path, baseline)
        native = native_factory(active_path)
        native.listen()
        require(native.ready() == 200)
        report["checks"].append({"name": "initial_managed_baseline", "passed": True,
                                 "http_status": 200, "database_roles": 4,
                                 "storage_prefixes": 2, "kms_keys": 2})
        for name in CASES:
            check = {"name": name, "passed": False}
            report["checks"].append(check)
            started = time.monotonic()
            reservation = None
            try:
                if name == "unreachable_database":
                    reservation = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                    reservation.bind(("127.0.0.1", 5432))  # Deliberately never listen.
                changed = candidate(document, name)
                if changed is None:
                    active_path.unlink()
                else:
                    runtime.atomic_write(active_path, changed)
                    private_file(active_path)
                helper_negative(runtime, active_path, baseline["generation"], name)
                check["helper_refused"] = True
                check["http_status"] = native.status()
                require(check["http_status"] == 503)
            finally:
                if reservation is not None:
                    reservation.close()
                # Restore only our isolated active copy, even when a check fails.
                runtime.atomic_write(active_path, document)
            helper_positive(runtime, active_path, baseline)
            require(runtime.load(baseline_path) == baseline)
            check["baseline_helper_passed"] = True
            check["restored_http_status"] = native.ready()
            require(check["restored_http_status"] == 200)
            check["elapsed_ms"] = round((time.monotonic() - started) * 1000)
            check["passed"] = True
    finally:
        # Reserve up to10s for native group termination and private-dir cleanup.
        # Work is interrupted after110s, so the complete runner remains<=120s.
        signal.alarm(10)
        if native is not None:
            native.close()
            report["native_process_stopped"] = True


def main():
    report = {"schema_version": 1, "scope": "APP-007 isolated managed runtime negatives",
              "passed": False, "service_changed": False, "llm_requests": 0,
              "auth_enabled": False, "wrong_tenant_key_repeated": False,
              "checks": [], "private_files_removed": False, "native_process_stopped": False}
    started = time.monotonic()
    directory = None
    previous_handlers = {}
    previous_logging = logging.root.manager.disable
    try:
        require(os.environ.get("GENUI_QUALIFY_SYNTHETIC") == "APP-007" and os.geteuid() == 65532)
        require(not any(key.startswith("GENUI_AUTH_QUALIFICATION_") for key in os.environ))
        private_directory("/run/genui")
        for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGALRM):
            previous_handlers[sig] = signal.signal(sig, interrupted)
        signal.alarm(110)
        logging.disable(logging.CRITICAL)
        sys.dont_write_bytecode = True
        helper = Path("/app/runtime.py")
        require(hashlib.sha256(helper.read_bytes()).hexdigest() == RUNTIME_SHA256)
        spec = importlib.util.spec_from_file_location("app007_runtime", helper)
        runtime = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(runtime)
        require(runtime.ROLES == ROLES)
        with tempfile.TemporaryDirectory(prefix=".app007-negatives-", dir="/run/genui") as directory:
            private_directory(directory)
            exercise(runtime, directory, report)
        require(not Path(directory).exists())
        report["private_files_removed"] = True
        report["passed"] = True
    except BaseException as error:
        report["failure"] = "assertion_or_interrupt" if isinstance(error, Refused) else "operation_failed"
    finally:
        signal.alarm(0)
        for sig, handler in previous_handlers.items():
            signal.signal(sig, handler)
        logging.disable(previous_logging)
        if directory is not None:
            report["private_files_removed"] = not Path(directory).exists()
        report["elapsed_ms"] = round((time.monotonic() - started) * 1000)
        print(json.dumps(report, separators=(",", ":")), flush=True)
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
