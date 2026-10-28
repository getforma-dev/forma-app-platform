"""Focused Linux supervision controls; no DuckDB query or benchmark substitutes.

Runs against the exact processes.py. A supervisor-only run may supply a query
module stub with just the exception classes, because no engine operation is
used here; full qualification separately uses real query.py and DuckDB.
"""
import concurrent.futures
import json
import os
from pathlib import Path
import select
import signal
import socket
import shutil
import tempfile
import time

import processes as p
from query import Admission, Deadline, Refused


def _wait(predicate, timeout=2):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.005)
    raise AssertionError("control did not reach expected state")


def _gone(pid):
    return not Path(f"/proc/{pid}").exists()


def _clean(parent, before):
    assert set(parent.glob("supervised-*")) == before, "private query scratch leaked"


def _recovery():
    readiness = p.start_workers()
    assert readiness["broker_native_threads"] == 1
    assert p._supervise(p._write, {"recovery": True}) == {"recovery": True}
    return readiness


def run():
    records = []
    _recovery()
    parent = p._BROKER.directory.parent
    before = set(parent.glob("supervised-*"))
    try:
        # Child return status is preserved without arbitrary exception contents.
        try:
            p._supervise(p._fault_child, "exit")
        except Refused as exc:
            assert "exit_code=23" in str(exc)
            records.append({"control": "sanitized child exit status", "passed": True, "reason": str(exc)})
        else:
            raise AssertionError("nonzero child exit accepted")
        for fault in ("fifo", "symlink", "oversize"):
            started = time.monotonic()
            try:
                p._supervise(p._fault_child, fault)
            except (OSError, ValueError):
                pass
            else:
                raise AssertionError("unsafe result file accepted: " + fault)
            assert time.monotonic() - started < 1
            _clean(parent, before)
            assert p._supervise(p._write, {"healthy": True}) == {"healthy": True}
            records.append({"control": fault + " result refused with recovery", "passed": True})

        # A peer that never completes a frame cannot monopolize this broker.
        for payload in (b"partial", b"x" * 512 + b"\n", b"../../escape\n", b"ready\nextra\n"):
            with socket.socket(socket.AF_UNIX) as connection:
                connection.settimeout(0.4)
                connection.connect(str(p._BROKER.directory / "control"))
                connection.sendall(payload)
                try:
                    assert connection.recv(1024) == b""
                except ConnectionResetError:
                    pass
            assert p._supervise(p._write, {"healthy": True}) == {"healthy": True}
        records.append({"control": "partial oversized malformed request frames", "passed": True})

        with socket.socket(socket.AF_UNIX) as connection:
            connection.settimeout(0.4)
            connection.connect(str(p._BROKER.directory / "control"))
            started = time.monotonic()
            while time.monotonic() - started < 0.3:
                try:
                    connection.sendall(b"x")
                except BrokenPipeError:
                    break
                time.sleep(0.04)
                if select.select([connection], [], [], 0)[0]:
                    assert connection.recv(1024) == b""
                    break
            elapsed = time.monotonic() - started
            assert elapsed < 0.25, "per-byte traffic extended the broker's frame deadline"
        assert p._supervise(p._write, {"healthy": True}) == {"healthy": True}
        records.append({"control": "slow-trickle request has one absolute frame deadline", "passed": True, "observed_seconds": elapsed})

        for payload in (b"{", b"x" * 1024, b"{}\n{}\n"):
            sender, receiver = socket.socketpair()
            try:
                sender.sendall(payload)
                started = time.monotonic()
                try:
                    p._receive(receiver, started + 0.15)
                except (TimeoutError, Refused):
                    pass
                else:
                    raise AssertionError("malformed or incomplete result frame accepted")
                assert time.monotonic() - started < 0.4
            finally:
                sender.close(); receiver.close()
        records.append({"control": "partial oversized malformed response frames", "passed": True})

        # A stopped broker cannot accept or fork the queued request later.
        broker_pid = p._BROKER.process.pid
        os.kill(broker_pid, signal.SIGSTOP)
        started = time.monotonic()
        try:
            p._supervise(p._write, {"must_not_return": True})
        except Deadline as exc:
            elapsed = time.monotonic() - started
            assert 5 <= elapsed < 6.5, elapsed
        else:
            raise AssertionError("stopped broker request returned")
        assert _gone(broker_pid); _clean(parent, before)
        try:
            p._supervise(p._write, {})
        except Refused:
            pass
        else:
            raise AssertionError("broker failure silently restarted admission")
        recovery = _recovery()
        records.append({"control": "broker stopped before request acceptance", "passed": True, "elapsed_seconds": elapsed, "broker_reaped": True, "no_late_launch": True, "explicit_recovery": recovery})

        # Two live slots: stopping the broker also strands a live child. Both
        # callers must fail, group members must disappear, and slots must recover.
        admission = Admission()
        first = admission.submit(lambda start: p._supervise(p._unresponsive_child))
        _wait(lambda: bool(list(parent.glob("supervised-*/partial.tmp"))))
        marker = next(parent.glob("supervised-*/partial.tmp"))
        child_pid = int(marker.read_text()); broker_pid = p._BROKER.process.pid
        os.kill(child_pid, signal.SIGSTOP)
        os.kill(broker_pid, signal.SIGSTOP)
        second = admission.submit(lambda start: p._supervise(p._write, {"must_not_return": True}))
        started = time.monotonic()
        failures = []
        try:
            for future in (first, second):
                try:
                    future.result(timeout=6.5)
                except (Deadline, Refused) as exc:
                    failures.append(type(exc).__name__)
                else:
                    raise AssertionError("failed broker released a result")
            assert admission.active == 0
            assert len(failures) == 2 and _gone(child_pid) and _gone(broker_pid)
            _clean(parent, before)
            recovery = _recovery()
            assert admission.submit(lambda start: p._supervise(p._write, {"recovery": True})).result(timeout=2) == {"recovery": True}
        finally:
            admission.close()
        records.append({"control": "stopped broker and child with concurrent callers", "passed": True, "elapsed_seconds": time.monotonic() - started, "failures": failures, "process_group_reaped": True, "scratch_reclaimed": True, "slots_recovered": True, "explicit_recovery": recovery})

        # Killing only the broker exercises PDEATHSIG, not just killpg cleanup.
        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as executor:
            result = executor.submit(p._supervise, p._unresponsive_child)
            _wait(lambda: bool(list(parent.glob("supervised-*/partial.tmp"))))
            marker = next(parent.glob("supervised-*/partial.tmp"))
            child_pid = int(marker.read_text()); broker_pid = p._BROKER.process.pid
            os.kill(broker_pid, signal.SIGKILL)
            try:
                result.result(timeout=2)
            except Refused:
                pass
            else:
                raise AssertionError("broker death released a result")
            assert _gone(broker_pid) and _gone(child_pid)
        _clean(parent, before); _recovery()
        records.append({"control": "broker death while child running", "passed": True, "child_reaped": True, "explicit_recovery": True})

        # Verify the kernel parent-death control before invoking group cleanup.
        # This raw trusted protocol peer deliberately does not run _supervise,
        # so no supervisor can race to kill the group on the broker's EOF.
        scratch = Path(tempfile.mkdtemp(prefix="supervised-", dir=parent))
        (scratch / "request.json").write_text(json.dumps({"target": "_unresponsive_child", "args": []}))
        with p._BROKER.connection(time.monotonic() + 1) as connection:
            connection.sendall(scratch.name.encode() + b"\n")
            _wait(lambda: (scratch / "partial.tmp").exists())
            child_pid = int((scratch / "partial.tmp").read_text())
            os.kill(p._BROKER.process.pid, signal.SIGKILL)
            _wait(lambda: _gone(child_pid) or "State:\tZ" in Path(f"/proc/{child_pid}/status").read_text())
            p.stop_workers()
            assert _gone(child_pid)
        shutil.rmtree(scratch); _clean(parent, before); _recovery()
        records.append({"control": "PDEATHSIG before group cleanup", "passed": True, "terminated_before_group_cleanup": True, "adopted_child_reaped": True})
        records.append({"control": "stuck ordinary child backstop", "passed": True, **p.backstop_probe()})
        assert not list(parent.glob("supervised-*/partial.tmp"))
        return {"passed": True, "scope": "Linux process/IPC/cleanup controls, no engine performance claim", "controls": records}
    finally:
        p.stop_workers()


if __name__ == "__main__":
    print(json.dumps(run(), indent=2))
