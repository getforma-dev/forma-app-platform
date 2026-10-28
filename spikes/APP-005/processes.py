"""Fresh Linux query children from a data-free, single-threaded fork server."""
import json
import multiprocessing
import os
from pathlib import Path
import shutil
import stat
import tempfile
import time

from query import ContractError, Deadline, Refused, execute_query

CONTEXT = multiprocessing.get_context("forkserver")
CONTEXT.set_forkserver_preload(["query", "processes"])
MAX_RESPONSE = 1024 * 1024


def _write(scratch, value):
    data = json.dumps(value).encode()
    if len(data) > MAX_RESPONSE:
        raise Refused("worker response exceeds 1 MiB")
    (Path(scratch) / "response.json").write_bytes(data)


def _ready_child(scratch):
    # The server imports code only: no queries, connections or tenant state.
    native_threads = len(list(Path(f"/proc/{os.getppid()}/task").iterdir()))
    assert native_threads == 1, "fork server must be single threaded"
    _write(scratch, {"fork_server_native_threads": native_threads,
                     "start_method": CONTEXT.get_start_method()})


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
    (Path(scratch) / "partial.tmp").write_text("synthetic cleanup control")
    time.sleep(30)


def _supervise(target, *args):
    scratch = Path(tempfile.mkdtemp(prefix="supervised-", dir=os.environ.get("FORMA_QUERY_TMP")))
    process = CONTEXT.Process(target=target, args=(str(scratch), *args))
    try:
        process.start()
        process.join(timeout=5)
        if process.is_alive():
            process.kill()
            process.join()
            raise Deadline("unresponsive query worker killed and reaped by 5-second backstop")
        if process.exitcode != 0:
            raise Refused("query worker failed without a valid result")
        # Read only after exit, with a byte cap and no symlink following. A pipe
        # with a partial frame could otherwise block the supervisor past its timer.
        fd = os.open(scratch / "response.json", os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(fd, "rb") as response:
            if not stat.S_ISREG(os.fstat(response.fileno()).st_mode):
                raise Refused("worker response is not a regular file")
            data = response.read(MAX_RESPONSE + 1)
        if len(data) > MAX_RESPONSE:
            raise Refused("worker response exceeds 1 MiB")
        return json.loads(data)
    finally:
        if process.pid is not None:
            if process.is_alive():
                process.kill()
                process.join()
            process.close()
        shutil.rmtree(scratch)


def start_workers():
    """Call before admission threads or database operations; readiness is measured."""
    started = time.monotonic()
    value = _supervise(_ready_child)
    return {**value, "startup_seconds": time.monotonic() - started}


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
        return {"elapsed_seconds": elapsed, "reason": str(exc),
                "process_reaped": True, "scratch_reclaimed": True}
    raise AssertionError("unresponsive child unexpectedly returned")
