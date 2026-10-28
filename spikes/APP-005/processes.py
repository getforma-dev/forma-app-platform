"""Linux-only, bounded IPC to a data-free single-threaded query fork broker.

Only the broker forks. It receives random scratch names, never query contents;
its fresh child reads the request and opens the engine. A failed broker is
retired as a whole process group. The controller adopts/reaps its children and
must explicitly establish readiness again before admitting subsequent work.
"""
import atexit
import ctypes
import json
import os
from pathlib import Path
import re
import select
import shutil
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import threading
import time

from query import ContractError, Deadline, Refused, execute_query

MAX_RESPONSE = 1024 * 1024
BACKSTOP = 5.0
REAP_GRACE = 1.0
_BROKER = None
_LIFECYCLE = threading.Lock()


def _prctl(option, value):
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.prctl(option, value, 0, 0, 0) != 0:
        raise OSError(ctypes.get_errno(), "required Linux process ownership unavailable")


def _parent_death(expected_parent):
    _prctl(1, signal.SIGKILL)  # PR_SET_PDEATHSIG; reset by fork, so set in each child.
    if os.getppid() != expected_parent:  # Parent may have died before prctl.
        os._exit(125)


def _read(path):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, "rb") as response:
        if not stat.S_ISREG(os.fstat(response.fileno()).st_mode):
            raise Refused("worker response is not a regular file")
        data = response.read(MAX_RESPONSE + 1)
    if len(data) > MAX_RESPONSE:
        raise Refused("worker message exceeds 1 MiB")
    return json.loads(data)


def _write(scratch, value):
    data = json.dumps(value).encode()
    if len(data) > MAX_RESPONSE:
        raise Refused("worker response exceeds 1 MiB")
    (Path(scratch) / "response.json").write_bytes(data)


def _query_child(scratch, root, name, query, epoch, start):
    os.environ["FORMA_QUERY_TMP"] = scratch
    try:
        value = execute_query(Path(root), name, query,
                              json.loads((Path(root) / "manifest.json").read_text()),
                              epoch=epoch, start=start)
    except (Refused, ContractError, Deadline) as exc:
        value = {"error": "deadline" if isinstance(exc, Deadline) else "narrow_the_query",
                 "reason": str(exc)[:500]}
    _write(scratch, value)


def _unresponsive_child(scratch):
    (Path(scratch) / "partial.tmp").write_text(str(os.getpid()))
    time.sleep(30)


def _fault_child(scratch, fault):
    """Trusted regression controls, never exposed as typed-query operations."""
    if fault == "exit":
        os._exit(23)
    if fault == "fifo":
        os.mkfifo(Path(scratch) / "response.json")
    elif fault == "symlink":
        (Path(scratch) / "response.json").symlink_to("/etc/passwd")
    elif fault == "oversize":
        (Path(scratch) / "response.json").write_bytes(b" " * (MAX_RESPONSE + 1))
    else:
        raise AssertionError("unknown fault control")


_TARGETS = {fn.__name__: fn for fn in (_query_child, _write, _unresponsive_child, _fault_child)}


def _send(connection, value):
    connection.sendall(json.dumps(value).encode() + b"\n")


def _broker_main(directory, expected_parent):
    _parent_death(expected_parent)
    native_threads = len(list(Path("/proc/self/task").iterdir()))
    if native_threads != 1:
        raise Refused("query preloader must be single threaded")
    listener = socket.socket(socket.AF_UNIX)
    listener.bind(str(directory / "control")); listener.listen(4)
    active = {}
    while True:
        # This one thread owns all fork/waitpid operations. No concurrent
        # multiprocessing global child registry or shared database connection.
        for pid, connection in list(active.items()):
            exited, status = os.waitpid(pid, os.WNOHANG)
            if exited:
                try:
                    _send(connection, {"exit_code": os.waitstatus_to_exitcode(status)})
                except OSError:
                    pass
                connection.close(); del active[pid]
        if not select.select([listener], [], [], 0.005)[0]:
            continue
        connection, _ = listener.accept(); connection.settimeout(0.1)
        try:
            frame_deadline = time.monotonic() + 0.1
            message = b""
            while b"\n" not in message and len(message) <= 128:
                connection.settimeout(_remaining(frame_deadline))
                part = connection.recv(129 - len(message))
                if not part:
                    raise Refused("incomplete broker request")
                message += part
            name = message.decode("ascii")
            if name == "ready\n":
                _send(connection, {"broker_native_threads": native_threads,
                                   "start_method": "owned_linux_fork_broker"})
                connection.close(); continue
            if not re.fullmatch(r"supervised-[a-z0-9_]+\n", name):
                raise Refused("invalid broker request")
            if len(active) >= 2:
                _send(connection, {"refused": "broker active capacity exhausted"})
                connection.close(); continue
            scratch = directory.parent / name.rstrip("\n")
            parent = os.getpid()
            pid = os.fork()
            if pid == 0:
                # No query has been read in the broker. The inherited descriptor
                # set must not keep other requests alive after broker death.
                _parent_death(parent)
                listener.close(); connection.close()
                for other in active.values():
                    other.close()
                try:
                    request = _read(scratch / "request.json")
                    _TARGETS[request["target"]](str(scratch), *request["args"])
                    os._exit(0)
                except BaseException:
                    # Never put arbitrary query data, paths or credentials into
                    # child failure diagnostics; the parent gets a fixed code.
                    os._exit(70)
            active[pid] = connection
        except (OSError, ValueError):
            connection.close()


class _Broker:
    def __init__(self):
        self.directory = Path(tempfile.mkdtemp(prefix="broker-", dir=os.environ.get("FORMA_QUERY_TMP")))
        self.broken = False
        self.cleaned = threading.Event()
        self.cleanup_error = None
        try:
            self.process = subprocess.Popen(
                [sys.executable, str(Path(__file__).resolve()), "--broker", str(self.directory), str(os.getpid())],
                stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                start_new_session=True,
            )
        except OSError:
            shutil.rmtree(self.directory)
            raise Refused("could not launch the owned query broker") from None

    def connection(self, deadline):
        if self.broken:
            raise Refused("query broker unavailable; explicit readiness required")
        connection = socket.socket(socket.AF_UNIX)
        try:
            connection.settimeout(_remaining(deadline))
            connection.connect(str(self.directory / "control"))
            return connection
        except BaseException:
            connection.close(); raise


def _remaining(deadline):
    value = deadline - time.monotonic()
    if value <= 0:
        raise TimeoutError("supervisor deadline")
    return value


def _receive(connection, deadline):
    data = b""
    while b"\n" not in data:
        connection.settimeout(_remaining(deadline))
        part = connection.recv(1024 - len(data))
        if not part:
            raise Refused("query broker exited before acknowledging a reaped child")
        data += part
        if len(data) >= 1024:
            raise Refused("broker response exceeds protocol bound")
    if not data.endswith(b"\n") or data.count(b"\n") != 1:
        raise Refused("invalid broker response frame")
    return json.loads(data)


def _retire(broker):
    # Only one caller kills/reaps this group. Other requests wait for that bounded
    # cleanup before removing their scratch, and fail closed rather than restart.
    with _LIFECYCLE:
        owner = not broker.broken
        broker.broken = True
    if owner:
        try:
            try:
                os.killpg(broker.process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            deadline = time.monotonic() + REAP_GRACE
            broker.process.wait(timeout=_remaining(deadline))
            while True:
                try:
                    pid, _ = os.waitpid(-broker.process.pid, os.WNOHANG)
                except ChildProcessError:
                    break
                if not pid:
                    _remaining(deadline); time.sleep(0.005)
            shutil.rmtree(broker.directory)
        except BaseException as exc:
            broker.cleanup_error = type(exc).__name__
        finally:
            broker.cleaned.set()
    elif not broker.cleaned.wait(REAP_GRACE + 0.1):
        raise Refused("broker cleanup did not finish inside its grace period")
    if broker.cleanup_error:
        raise Refused("broker group cleanup failed: " + broker.cleanup_error)


def stop_workers():
    if _BROKER is not None:
        _retire(_BROKER)


atexit.register(stop_workers)


def start_workers():
    """Controller-only readiness, initially or after draining a failed broker."""
    global _BROKER
    if sys.platform != "linux" or threading.current_thread() is not threading.main_thread():
        raise Refused("broker readiness requires the Linux controller main thread")
    if _BROKER is not None:
        _retire(_BROKER)
    _prctl(36, 1)  # PR_SET_CHILD_SUBREAPER: reap owned descendants after broker death.
    started = time.monotonic(); deadline = started + BACKSTOP
    _BROKER = broker = _Broker()
    try:
        while True:
            try:
                with broker.connection(deadline) as connection:
                    connection.settimeout(_remaining(deadline))
                    connection.sendall(b"ready\n")
                    value = _receive(connection, deadline)
                break
            except (FileNotFoundError, ConnectionRefusedError):
                if broker.process.poll() is not None:
                    raise Refused("broker initialization failed; exit_code=" + str(broker.process.returncode))
                _remaining(deadline); time.sleep(0.005)
        return {**value, "startup_seconds": time.monotonic() - started}
    except BaseException:
        _retire(broker); raise


def _supervise(target, *args):
    deadline = time.monotonic() + BACKSTOP
    broker = _BROKER
    if broker is None or broker.broken:
        raise Refused("query broker unavailable; explicit readiness required")
    scratch = Path(tempfile.mkdtemp(prefix="supervised-", dir=broker.directory.parent))
    try:
        if target.__name__ not in _TARGETS or _TARGETS[target.__name__] is not target:
            raise Refused("unsupported trusted worker target")
        request = json.dumps({"target": target.__name__, "args": args}).encode()
        if len(request) > MAX_RESPONSE:
            raise Refused("worker request exceeds 1 MiB")
        (scratch / "request.json").write_bytes(request)
        try:
            with broker.connection(deadline) as connection:
                connection.settimeout(_remaining(deadline))
                connection.sendall(scratch.name.encode() + b"\n")
                status = _receive(connection, deadline)
        except TimeoutError as exc:
            _retire(broker)
            raise Deadline("5-second request/start/run/IPC backstop; broker group killed and reaped") from exc
        except (OSError, ValueError) as exc:
            _retire(broker)
            raise Refused("broker IPC failed; owned group killed and reaped: " + type(exc).__name__) from exc
        if "refused" in status:
            raise Refused(status["refused"])
        if type(status.get("exit_code")) is not int:
            _retire(broker)
            raise Refused("broker did not return a child exit status")
        if status["exit_code"] != 0:
            raise Refused("query worker failed; exit_code=" + str(status["exit_code"]))
        value = _read(scratch / "response.json")
        if time.monotonic() >= deadline:
            raise Deadline("result reading exceeded supervisor deadline; withheld")
        return value
    finally:
        # A concurrent request may have retired our broker; join its cleanup
        # before deleting paths that any killed child could still have open.
        if broker.broken:
            _retire(broker)
        shutil.rmtree(scratch)


def supervised_query(root, name, query, manifest, epoch=1, start=None):
    start = time.monotonic() if start is None else start
    if time.monotonic() - start >= 1.8:
        raise Deadline("deadline expired in admission queue")
    value = _supervise(_query_child, str(root), name, query, epoch, start)
    if "error" in value:
        if value["error"] == "deadline":
            raise Deadline(value["reason"])
        raise Refused(value["error"] + ": " + value["reason"])
    if time.monotonic() - start >= 1.8:
        raise Deadline("result arrived after deadline and was withheld")
    value["elapsed_seconds"] = time.monotonic() - start
    return value


def backstop_probe():
    parent = Path(os.environ.get("FORMA_QUERY_TMP", tempfile.gettempdir()))
    before = set(parent.glob("supervised-*"))
    started = time.monotonic()
    try:
        _supervise(_unresponsive_child)
    except Deadline as exc:
        elapsed = time.monotonic() - started
        assert 5 <= elapsed < 6.5, elapsed
        assert set(parent.glob("supervised-*")) == before
        recovery = start_workers()
        assert _supervise(_write, {"recovery": True}) == {"recovery": True}
        return {"elapsed_seconds": elapsed, "reason": str(exc),
                "process_group_reaped": True, "scratch_reclaimed": True,
                "explicit_readiness_recovered": recovery}
    raise AssertionError("unresponsive child unexpectedly returned")


if __name__ == "__main__":
    if len(sys.argv) == 4 and sys.argv[1] == "--broker":
        _broker_main(Path(sys.argv[2]), int(sys.argv[3]))
    else:
        raise SystemExit("internal broker entrypoint only")
