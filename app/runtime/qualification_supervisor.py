"""APP-075 private qualification supervisor (qualification image only).

Launches exactly one process: the cfg(test) library test executable baked into
the qualification image, with the five fixed libtest arguments selecting
``storage::managed_qualification::managed_readonly_entry``. It is never the
service supervisor (``runtime.serve``) and never runs ``serve``, ``migrate``
or any other mode.

Before launching it refuses: any argument list other than ``["run"]``; a
missing arming value; any ``PG*`` or ``*PASSWORD*`` variable (the migration
task's operator password channel) or auth-rehearsal injection; a UID other than 65532; a
manifest that is absent, oversized, not one closed JSON object, outside the
original-deadline bounds or not pinned to the reviewed RDS CA; a CA file that
differs from that pin; mounts over the CA, the image's program files or the
private run files; pre-existing run files; and an executable whose bytes
differ from the image's build identity.

It writes the run manifest and the canonical synthetic registry 0600 under
the 0700 ``/run/genui``, obtains credentials with ``runtime.refresh`` (which
validates through ``runtime.load`` and publishes with ``runtime.atomic_write``)
and refreshes them every 240 s until the executable exits. The child receives
only the arming variable: no AWS, database or operator material. Both pipes
are drained continuously (the entry's stdout-drain contract); the single
``APP008_MANAGED_RESULT`` line is retained and re-emitted verbatim, followed by
one ``APP075_SUPERVISOR`` summary. At the manifest's original ``expires_at``
the child is killed. Run files and credentials are removed on every path.

No raw exception, environment, token or credential is printed.
"""
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import threading
import time
import uuid

import runtime

ARGV = ("run",)
EXECUTABLE = Path("/app/qualification/genui-storage-tests")
IDENTITY = Path("/app/qualification/build-identity.json")
ARGUMENTS = ("--exact", "storage::managed_qualification::managed_readonly_entry",
             "--ignored", "--nocapture", "--test-threads=1")
RUN_DIRECTORY = Path("/run/genui")
MANIFEST_NAME = "app008-managed-run.json"
REGISTRY_NAME = "app008-managed-registry.json"
CREDENTIALS_NAME = "credentials.json"
CA = Path("/etc/ssl/certs/rds-global-bundle.pem")
REVIEWED_CA_SHA256 = "90a6f1a7978a1b1b096dca6c57511ca48d3b8cf052f8e704baf358f34ea2f21b"
ARM = "GENUI_APP008_MANAGED_QUALIFICATION"
ARM_VALUE = "APP-008"
MANIFEST_ENV = "GENUI_APP008_MANAGED_MANIFEST_JSON"
UID = 65532
HOST = "genuisyntheticfoundation-databaseb269d8bb-pta9s49jelmg.cg58i06y2r4e.us-east-1.rds.amazonaws.com"
ISSUER = "app008-fixture-only"
ORGANIZATIONS = ("00000000-0000-4000-8000-000000000001", "00000000-0000-4000-8000-000000000002")
RESULT_PREFIX = b"APP008_MANAGED_RESULT "
SUMMARY_PREFIX = "APP075_SUPERVISOR "
MANIFEST_LIMIT = 8192
RESULT_LIMIT = 8192 + len(RESULT_PREFIX) + 2
REFRESH_INTERVAL = 240
MAX_SPAN = 600
ISSUED_SKEW = 5
MIN_REMAINING = 60
# Mount points at or under these paths would let a launcher replace the CA,
# the executable, its identity, Python or the supervisor itself.
FORBIDDEN_MOUNTS = ("/etc/ssl", "/app", "/usr", "/bin", "/sbin", "/lib", "/opt")


class Refused(ValueError):
    pass


def require(condition):
    if not condition:
        raise Refused("qualification supervisor refused")


def registry_bytes(run_id):
    """Byte-identical to the Rust entry's registry_bytes (sorted, compact)."""
    bindings = [{"issuer": ISSUER, "provider_tenant_id": "ten_" + org, "organization_id": org} for org in ORGANIZATIONS]
    return json.dumps({"schema_version": 1, "registry_version": run_id, "bindings": bindings},
                      sort_keys=True, separators=(",", ":")).encode("utf-8")


def check_invocation(argv, environ, euid, uid=UID):
    require(tuple(argv) == ARGV)
    require(environ.get(ARM) == ARM_VALUE)
    require(not any(name.startswith("PG") or "PASSWORD" in name.upper() for name in environ))
    require(runtime.QUALIFICATION_JSON_ENV not in environ and runtime.QUALIFICATION_FILE_ENV not in environ)
    require(euid == uid)


def closed_object(pairs):
    keys = [key for key, _ in pairs]
    require(len(keys) == len(set(keys)))
    return dict(pairs)


def take_manifest(environ, now, reviewed_ca):
    """Remove the injected manifest before anything can inherit it.

    The Rust entry owns full validation; this repeats only what the
    supervisor itself relies on: run id, original deadline, target host and
    the reviewed CA pin.
    """
    raw = environ.pop(MANIFEST_ENV, None)
    require(raw is not None)
    try:
        payload = raw.encode("utf-8", errors="strict")
        manifest = json.loads(payload, object_pairs_hook=closed_object)
    except (UnicodeError, ValueError):
        raise Refused("qualification supervisor refused") from None
    require(0 < len(payload) <= MANIFEST_LIMIT and isinstance(manifest, dict))
    run_id, issued, expires = manifest.get("run_id"), manifest.get("issued_at"), manifest.get("expires_at")
    require(isinstance(run_id, str) and str(uuid.UUID(run_id)) == run_id and uuid.UUID(run_id).version == 4)
    require(type(issued) is int and type(expires) is int)
    require(issued <= now + ISSUED_SKEW and issued < expires <= issued + MAX_SPAN)
    require(expires - now >= MIN_REMAINING)
    require(manifest.get("database_host") == HOST and manifest.get("armed") is True)
    require(manifest.get("ca_sha256") == reviewed_ca)
    return payload, manifest


def check_mounts(mountinfo, run_directory):
    """Refuse mounts that could replace the CA, program files or run files."""
    run = str(run_directory).rstrip("/")
    for line in mountinfo.splitlines():
        fields = line.split(" ")
        require(len(fields) > 4)
        point = fields[4].encode("utf-8").decode("unicode_escape")
        for forbidden in FORBIDDEN_MOUNTS:
            require(point != forbidden and not point.startswith(forbidden + "/"))
        require(not point.startswith(run + "/"))


def sha256_file(path):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    with os.fdopen(fd, "rb") as stream:
        metadata = os.fstat(stream.fileno())
        require(stat.S_ISREG(metadata.st_mode) and metadata.st_nlink == 1 and metadata.st_mode & 0o022 == 0)
        return hashlib.sha256(stream.read()).hexdigest()


def check_executable(executable, identity):
    expected = json.loads(Path(identity).read_text())
    require(expected.get("schema_version") == 1 and expected.get("executable") == str(EXECUTABLE))
    require(sha256_file(executable) == expected.get("executable_sha256"))
    return expected["executable_sha256"]


def check_run_directory(run_directory, euid):
    metadata = os.lstat(run_directory)
    require(stat.S_ISDIR(metadata.st_mode) and metadata.st_uid == euid and stat.S_IMODE(metadata.st_mode) == 0o700)


def write_new_private(path, payload):
    """Create (never replace) one private 0600 file."""
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
    with os.fdopen(fd, "wb") as stream:
        os.fchmod(stream.fileno(), 0o600)
        stream.write(payload)
        stream.flush()
        os.fsync(stream.fileno())


class Drain(threading.Thread):
    """Reads one pipe to EOF in bounded chunks so the child never blocks."""

    def __init__(self, stream, retain):
        super().__init__(daemon=True)
        self.stream, self.retain = stream, retain
        self.results, self.lines, self.bytes, self.oversized = [], 0, 0, False

    def run(self):
        pending = b""
        while True:
            chunk = self.stream.read1(65536) if hasattr(self.stream, "read1") else self.stream.read(65536)
            if not chunk:
                break
            self.bytes += len(chunk)
            if not self.retain:
                continue
            pending += chunk
            *complete, pending = pending.split(b"\n")
            for line in complete:
                self.line(line)
            if len(pending) > RESULT_LIMIT:
                self.oversized, pending = True, b""
        if self.retain and pending:
            self.line(pending)

    def line(self, line):
        self.lines += 1
        if line.startswith(RESULT_PREFIX):
            if len(line) > RESULT_LIMIT:
                self.oversized = True
            else:
                self.results.append(line)


def remove(path):
    try:
        if stat.S_ISREG(os.lstat(path).st_mode):
            os.unlink(path)
    except FileNotFoundError:
        pass


def supervise(argv, environ, *, refresh=runtime.refresh, load=runtime.load, reviewed_ca=REVIEWED_CA_SHA256,
              executable=EXECUTABLE, identity=IDENTITY, run_directory=RUN_DIRECTORY, ca=CA,
              mountinfo=Path("/proc/self/mountinfo"), euid=None, uid=UID, clock=time.time,
              refresh_interval=REFRESH_INTERVAL, out=None):
    """Returns the process exit status; 0 only for a passed, cleaned-up run."""
    out = out or sys.stdout
    euid = os.geteuid() if euid is None else euid
    run_directory = Path(run_directory)
    manifest_path = run_directory / MANIFEST_NAME
    registry_path = run_directory / REGISTRY_NAME
    credentials_path = run_directory / CREDENTIALS_NAME
    summary = {"schema_version": 1, "scope": "APP-075 qualification supervisor", "run_id": None, "launched": False,
               "child_exit": None, "result_retained": False, "passed": False, "refreshes": 0,
               "refresh_failures": 0, "deadline_kill": False, "stdout_lines": 0, "stderr_bytes": 0,
               "files_removed": False, "error": None}
    written = []
    child = None
    try:
        try:
            check_invocation(argv, environ, euid, uid)
            payload, manifest = take_manifest(environ, int(clock()), reviewed_ca)
            summary["run_id"] = manifest["run_id"]
            require(environ.get("GENUI_DATABASE_HOST") == HOST)
            check_mounts(Path(mountinfo).read_text(), run_directory)
            require(sha256_file(ca) == reviewed_ca)
            check_executable(executable, identity)
            check_run_directory(run_directory, euid)
            for path in (manifest_path, registry_path, credentials_path):
                require(not os.path.lexists(path))
            write_new_private(manifest_path, payload)
            written.append(manifest_path)
            write_new_private(registry_path, registry_bytes(manifest["run_id"]))
            written.append(registry_path)
        except (Refused, OSError, ValueError, KeyError, TypeError):
            summary["error"] = "configuration_refused"
            return 2
        expires_at = manifest["expires_at"]
        written.append(credentials_path)
        try:
            refresh(credentials_path)
            load(credentials_path)
            summary["refreshes"] += 1
        except Exception:
            # SDK/driver errors can contain tokens or connection strings.
            print("credential_refresh_unavailable", file=sys.stderr, flush=True)
            summary["error"] = "credential_refresh_unavailable"
            return 3
        child = subprocess.Popen([str(executable), *ARGUMENTS], env={ARM: ARM_VALUE}, cwd="/",
                                 stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                 close_fds=True)
        summary["launched"] = True
        readers = (Drain(child.stdout, True), Drain(child.stderr, False))
        for reader in readers:
            reader.start()
        next_refresh = clock() + refresh_interval
        while child.poll() is None:
            now = clock()
            if now >= expires_at:
                child.kill()
                summary["deadline_kill"] = True
                break
            if now >= next_refresh:
                next_refresh = now + refresh_interval
                try:
                    refresh(credentials_path)
                    load(credentials_path)
                    summary["refreshes"] += 1
                except Exception:
                    summary["refresh_failures"] += 1
                    print("credential_refresh_unavailable", file=sys.stderr, flush=True)
            try:
                child.wait(timeout=max(0.05, min(1.0, next_refresh - now, expires_at - now)))
            except subprocess.TimeoutExpired:
                pass
        child.wait()
        summary["child_exit"] = child.returncode
        for reader in readers:
            reader.join(timeout=5)
        stdout_reader, stderr_reader = readers
        summary["stdout_lines"], summary["stderr_bytes"] = stdout_reader.lines, stderr_reader.bytes
        results = stdout_reader.results
        if len(results) != 1 or stdout_reader.oversized:
            summary["error"] = "result_missing_or_ambiguous"
            return 4
        line = results[0]
        try:
            result = json.loads(line[len(RESULT_PREFIX):], object_pairs_hook=closed_object)
        except (Refused, ValueError):
            summary["error"] = "result_missing_or_ambiguous"
            return 4
        summary["result_retained"] = True
        out.write(line.decode("utf-8", errors="replace") + "\n")
        passed = (isinstance(result, dict) and result.get("passed") is True and result.get("cleanup_confirmed") is True
                  and result.get("run_id") == manifest["run_id"] and child.returncode == 0
                  and not summary["deadline_kill"])
        summary["passed"] = passed
        if not passed:
            summary["error"] = result.get("error") if isinstance(result, dict) and isinstance(result.get("error"), str) else "qualification_failed"
        return 0 if passed else 1
    finally:
        if child is not None:
            if child.poll() is None:
                child.kill()
                child.wait()
            for stream in (child.stdout, child.stderr):
                try:
                    stream.close()
                except (OSError, ValueError):
                    pass
        for path in written:
            remove(path)
        summary["files_removed"] = not any(os.path.lexists(path) for path in written)
        out.write(SUMMARY_PREFIX + json.dumps(summary, sort_keys=True, separators=(",", ":")) + "\n")
        out.flush()


if __name__ == "__main__":
    try:
        status = supervise(sys.argv[1:], os.environ)
    except Exception:
        print("qualification_supervisor_refused", file=sys.stderr, flush=True)
        status = 1
    raise SystemExit(status)
