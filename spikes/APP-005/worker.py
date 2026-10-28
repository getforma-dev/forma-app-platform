"""Qualification worker; direct SQL controls are trusted tests, never user APIs."""
import argparse
import copy
import json
import os
from pathlib import Path
import resource
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time

from query import Admission, ContractError, Deadline, Refused, authorized_source, compile_query, connect, digest, execute_query, field_ref, measure, run_sql, typed_query
import duckdb


def supervised_query(root, name, query, manifest, epoch=1, start=None):
    """Every scheduled query gets an independently killable process backstop."""
    start = start or time.monotonic()
    if time.monotonic() - start >= 1.8:
        raise Deadline("deadline expired in admission queue")
    request = {"name": name, "query": query, "epoch": epoch, "start": start}
    scratch = Path(tempfile.mkdtemp(prefix="supervised-", dir=os.environ.get("FORMA_QUERY_TMP")))
    process = subprocess.Popen([sys.executable, __file__, str(root), "--query-request"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env={**os.environ, "FORMA_QUERY_TMP": str(scratch)})
    try:
        try:
            out, err = process.communicate(json.dumps(request), timeout=5)
        except subprocess.TimeoutExpired as exc:
            process.kill(); process.communicate()
            raise Deadline("unresponsive query worker killed and reaped by 5-second backstop") from exc
    finally:
        shutil.rmtree(scratch)
    if process.returncode:
        failure = json.loads(out) if out else {"error": "engine_failure", "reason": err[:500]}
        if failure["error"] == "deadline":
            raise Deadline(failure["reason"])
        raise Refused(failure["error"] + ": " + failure["reason"])
    value = json.loads(out)
    if time.monotonic() - start >= 1.8:
        raise Deadline("result arrived after deadline and was withheld")
    value["elapsed_seconds"] = time.monotonic() - start
    return value


def workloads():
    return [typed_query([measure("sum")]), typed_query([measure("count", "event_id")]),
            typed_query([measure("sum", group=["destination"])]),
            typed_query([{"type": "time_bucket", "field": field_ref("time"), "seconds": 60, "timezone": "UTC", "output_field": "time"}, measure("sum", group=["time"])]),
            typed_query([measure("min")]), typed_query([measure("max")])]


def expected_workloads(manifest):
    m = manifest["million"]
    return [[[str(m["total"])]], [[str(m["rows"])]], [[name, str(value)] for name, value in sorted(m["destinations"].items())],
            [[f"2026-10-22T10:{i:02d}:00Z", str(value)] for i, value in enumerate(m["minute_totals"])], [["1"]], [["1000"]]]


def deadline_probe(root):
    """The parent independently kills this entire process if interrupt cannot finish."""
    manifest = json.loads((root / "manifest.json").read_text())
    before = set(Path(os.environ.get("FORMA_QUERY_TMP", tempfile.gettempdir())).glob("query-*"))
    started = time.monotonic()
    try:
        run_sql(root, [root / "million.parquet"], "SELECT sum(a.i*b.i) FROM range(1000000000) a(i) CROSS JOIN range(1000000000) b(i)", [])
        raise AssertionError("heavy negative control unexpectedly completed")
    except Deadline as exc:
        elapsed = time.monotonic() - started
        assert 1.7 <= elapsed < 3.5, elapsed
        after = set(Path(os.environ.get("FORMA_QUERY_TMP", tempfile.gettempdir())).glob("query-*"))
        assert after == before, "scratch not reclaimed"
        recovery = execute_query(root, "s02.parquet", typed_query([measure("sum")]), manifest)
        assert recovery["rows"] == [["1140000"]]
        return {"passed": True, "deadline_seconds": 1.8, "elapsed_seconds": elapsed, "reason": str(exc), "scratch_reclaimed": True, "recovery_result": recovery["rows"]}


def security(root, manifest, container):
    allowed = authorized_source(root, "s02.parquet", manifest)
    secret = Path("/protected/canary.csv") if container else root.parent / "canary.csv"
    assert secret.read_text() == "secret\n987654321\n", "negative control must exist before refusal tests"
    probes = {
        "other_organization_file": ("SELECT * FROM read_csv(?,header=true)", [str(secret)]),
        "path_traversal": ("SELECT * FROM read_csv(?,header=true)", [str(root / ".." / "canary.csv")]),
        "symlink_escape": ("SELECT * FROM read_parquet(?)", [str(root / "escape-link.parquet")]),
        "host_environment": ("SELECT * FROM read_csv('/proc/self/environ')", []),
        "URL": ("SELECT * FROM read_parquet('https://example.com/private.parquet')", []),
        "glob_outside": ("SELECT * FROM glob('/etc/*')", []),
        "attach": ("ATTACH '/tmp/escape.duckdb' AS escape", []),
        "copy": ("COPY (SELECT 1) TO '/tmp/escape.csv'", []),
        "install": ("INSTALL httpfs", []),
        "load": ("LOAD httpfs", []),
        "unlock": ("SET lock_configuration=false", []),
        "external_access": ("SET enable_external_access=true", []),
        "allow_directory": ("SET allowed_directories=['/']", []),
        "allow_config": ("SET allowed_configs=['enable_external_access']", []),
        "autoload": ("SET autoload_known_extensions=true", []),
        "create_table": ("CREATE TABLE forbidden(i INT)", []),
    }
    results = []
    for name, (sql, params) in probes.items():
        try:
            run_sql(root, [allowed], sql, params)
        except duckdb.Error as exc:
            message = str(exc)
            # A missing file/function is not evidence of the security setting.
            assert any(word in message.lower() for word in ("disabled", "permission", "locked", "read-only", "read only")), (name, message)
            results.append({"name": name, "refused": True, "class": type(exc).__name__, "reason": message[:500]})
        else:
            raise AssertionError(f"dangerous direct-engine control admitted: {name}")
    legitimate = execute_query(root, "s02.parquet", typed_query([measure("sum")]), manifest)
    assert legitimate["rows"] == [["1140000"]]
    scratch = Path(tempfile.mkdtemp(prefix="settings-", dir=os.environ.get("FORMA_QUERY_TMP")))
    c = connect(root, [allowed], scratch)
    settings = dict(c.execute("SELECT name,value FROM duckdb_settings() WHERE name IN ('threads','memory_limit','max_temp_directory_size','enable_external_access','allowed_paths','allowed_directories','allowed_configs','lock_configuration','autoinstall_known_extensions','autoload_known_extensions','allow_unsigned_extensions')").fetchall())
    directories, files = c.execute("SELECT current_setting('allowed_directories'), current_setting('allowed_paths')").fetchone()
    # DuckDB 2.0 adds its database/WAL files and spill directory itself. Verify the
    # effective policy, including those private resources, not just requested args.
    assert {Path(p).resolve() for p in directories} == {scratch.resolve()}, directories
    assert all(Path(p).resolve() == allowed.resolve() or Path(p).resolve().parent == scratch.resolve() for p in files), files
    c.close(); shutil.rmtree(scratch)
    assert settings["threads"] == "2" and settings["lock_configuration"] == "true" and settings["enable_external_access"] == "false"
    assert settings["allowed_configs"] == "[]"
    assert settings["memory_limit"] == "1.0 GiB" and settings["max_temp_directory_size"] == "2.0 GiB"
    assert settings["autoinstall_known_extensions"] == settings["autoload_known_extensions"] == settings["allow_unsigned_extensions"] == "false"
    return {"direct_engine_probes": results, "settings": settings, "extension_observation": "LOAD/INSTALL httpfs refused; extension filesystem enumeration is itself unavailable under disabled external access", "legitimate_query_after_refusals": legitimate["rows"]}


def spill_probe(root):
    """A stricter 16 MiB diagnostic forces real engine spill; production stays 1 GiB."""
    scratch = Path(tempfile.mkdtemp(prefix="spill-", dir=os.environ.get("FORMA_QUERY_TMP")))
    c = connect(root, [root / "million.parquet"], scratch, memory_limit="16MiB")
    finished = threading.Event(); peak = [0]; seen = set()
    def observe():
        while not finished.wait(0.01):
            size = 0
            for path in scratch.rglob("*"):
                try:
                    if path.is_file() and "duckdb_temp" in path.name:
                        size += path.stat().st_size; seen.add(path.name)
                except FileNotFoundError:
                    pass
            peak[0] = max(peak[0], size)
    observer = threading.Thread(target=observe); observer.start()
    timer = threading.Timer(1.8, c.interrupt); timer.start(); started = time.monotonic()
    state = "completed"
    try:
        value = c.execute("SELECT sum(rn) FROM (SELECT row_number() OVER (ORDER BY repeat(md5(i::VARCHAR),4)) rn FROM range(300000) t(i))").fetchone()[0]
        assert value == 300000 * 300001 // 2
    except duckdb.InterruptException:
        state = "interrupted"
    except duckdb.OutOfMemoryException:
        state = "memory_limit_refused"
    finally:
        timer.cancel(); timer.join(); finished.set(); observer.join(); c.close(); shutil.rmtree(scratch)
    assert peak[0] > 0, "forced-spill control produced no observed spill"
    assert not scratch.exists()
    return {"passed": True, "state": state, "elapsed_seconds": time.monotonic() - started, "test_memory_limit": "16MiB (tighter diagnostic; ordinary queries use 1GiB)", "peak_observed_spill_bytes": peak[0], "observed_files": sorted(seen), "scratch_removed": True}


def typed_refusals(root, manifest):
    queries = []
    base = workloads()[0]
    for key, value in (("sql", "SELECT * FROM read_csv('/etc/passwd')"), ("path", "/etc/passwd"), ("organization_id", "canary")):
        q = copy.deepcopy(base); q[key] = value; queries.append((key, q))
    q = copy.deepcopy(base); q["operations"][0]["measures"][0]["field"]["field_id"] = 'bytes); SELECT 1 --'; queries.append(("identifier_injection", q))
    q = copy.deepcopy(base); q["operations"][0]["type"] = "sql"; queries.append(("unknown_operation", q))
    q = typed_query([{"type": "lookup", "left": field_ref("destination"), "right": field_ref("destination"), "rule": "cidr_exact_then_longest", "cardinality": "at_most_one"}]); queries.append(("unqualified_CIDR_lookup", q))
    q = typed_query([{"type": "project", "fields": [field_ref("event_id")]}, measure("sum")]); queries.append(("field_removed_before_use", q))
    q = typed_query([{"type": "limit", "rows": 1}, measure("sum")]); queries.append(("pagination_before_total", q))
    records = []
    for name, q in queries:
        try:
            execute_query(root, "s02.parquet", q, manifest)
        except (Refused, ContractError) as exc:
            records.append({"name": name, "refused": True, "reason": str(exc)[:300]})
        else:
            raise AssertionError(f"typed boundary admitted {name}")
    for name in ("../canary.csv", "escape-link.parquet", "https://example.com/private.parquet"):
        try:
            execute_query(root, name, base, manifest)
        except Refused as exc:
            records.append({"name": name, "refused": True, "reason": str(exc)})
        else:
            raise AssertionError("source name escaped trusted registry")
    literal = typed_query([{"type": "filter", "field": field_ref("destination"), "comparison": "eq", "value": {"type": "text", "value": "x' OR 1=1; --"}}, measure("count", "event_id")])
    assert execute_query(root, "s02.parquet", literal, manifest)["rows"] == [["0"]]
    try:
        execute_query(root, "million.parquet", typed_query([{"type": "project", "fields": [field_ref("event_id")]}]), manifest)
    except Refused:
        records.append({"name": "unbounded_result_page", "refused": True})
    else:
        raise AssertionError("unbounded result was silently truncated")
    changed = copy.deepcopy(manifest); changed["files"]["s02.parquet"]["digest"] = "sha256:" + "0" * 64
    try:
        execute_query(root, "s02.parquet", base, changed)
    except Refused:
        records.append({"name": "source_digest_mismatch", "refused": True})
    else:
        raise AssertionError("changed evidence accepted")
    return records


def admission_probe(root, manifest):
    gate = threading.Event()
    broker = Admission()
    futures = []
    def held_query(start):
        assert gate.wait(5)
        return supervised_query(root, "s02.parquet", workloads()[0], manifest, start=start)
    try:
        for _ in range(24):
            try:
                futures.append(broker.submit(held_query))
            except Refused:
                pass
        assert broker.accepted == 22 and broker.rejected == 2
        gate.set()
        completed = 0; expired = 0
        for future in futures:
            try:
                assert future.result(timeout=10)["rows"] == [["1140000"]]
                completed += 1
            except Deadline:
                expired += 1
        assert completed > 0 and completed + expired == 22
        assert broker.max_active == 2 and broker.active == 0
        assert broker.submit(supervised_query, root, "s02.parquet", workloads()[0], manifest).result(timeout=5)["rows"] == [["1140000"]]
        return {"initial_admitted": 22, "initial_rejected": 2, "completed_with_correct_result": completed, "expired_without_result": expired, "max_running": broker.max_active, "queue_cap": 20, "active_after": broker.active, "recovery_admitted": True}
    finally:
        gate.set(); broker.close()


def queued_deadline_probe(root, manifest):
    broker = Admission(); gate = threading.Event(); launched = [0]
    started = time.monotonic()
    def occupy(start):
        # Trusted test-only hold, not a production operation.
        assert gate.wait(4)
    def should_not_launch(start):
        launched[0] += 1
        return supervised_query(root, "s02.parquet", workloads()[0], manifest, start=start)
    try:
        held = [broker.submit(occupy) for _ in range(2)]
        queued = broker.submit(should_not_launch)
        time.sleep(2.05)
        gate.set()
        for task in held:
            task.result(timeout=2)
        try:
            queued.result(timeout=2)
        except Deadline as exc:
            reason = str(exc)
        else:
            raise AssertionError("expired queued work was allowed to run")
        assert launched[0] == 0 and broker.active == 0
        assert broker.submit(supervised_query, root, "s02.parquet", workloads()[0], manifest).result(timeout=5)["rows"] == [["1140000"]]
        return {"queue_hold_seconds": 2.05, "elapsed_seconds": time.monotonic() - started, "deadline_origin": "enqueue time", "refusal": reason, "worker_launches_for_expired_request": launched[0], "slots_recovered": True}
    finally:
        gate.set(); broker.close()


def os_probe():
    assert os.getuid() == 65532
    assert not any(key.startswith(("AWS_", "AZURE_", "GOOGLE_APPLICATION_CREDENTIALS")) for key in os.environ)
    status = Path("/proc/self/status").read_text()
    assert "CapEff:\t0000000000000000" in status and "NoNewPrivs:\t1" in status
    try:
        Path("/root-write-probe").write_text("no")
    except OSError as exc:
        assert exc.errno == 30
    else:
        raise AssertionError("root filesystem writable")
    s = socket.socket(); s.settimeout(0.2)
    try:
        s.connect(("1.1.1.1", 443))
    except OSError as exc:
        assert exc.errno in (101, 113), repr(exc)
        network_reason = str(exc)
    else:
        raise AssertionError("network unexpectedly reachable")
    finally:
        s.close()
    return {"uid": os.getuid(), "capabilities": "none", "no_new_privileges": True, "root_read_only": True, "no_cloud_credential_environment": True, "network_denied": network_reason}


def suite(root, container=False):
    manifest = json.loads((root / "manifest.json").read_text())
    report = {"duckdb": duckdb.__version__, "python": sys.version, "fixture": manifest, "qualification_scope": "local authorized immutable revision; no service/auth/cloud/network overhead; no full Northstar CIDR workload"}
    checks = []
    def check(name, function):
        try:
            value = function(); checks.append({"name": name, "passed": True, "evidence": value})
        except Exception as exc:
            checks.append({"name": name, "passed": False, "error": type(exc).__name__ + ": " + str(exc)})
    if container:
        check("OS confinement", os_probe)
    check("closed typed refusals", lambda: typed_refusals(root, manifest))
    check("engine defense in depth", lambda: security(root, manifest, container))
    check("admission and resource recovery", lambda: admission_probe(root, manifest))
    check("queued deadline expires before worker launch", lambda: queued_deadline_probe(root, manifest))
    def timeout():
        started = time.monotonic()
        proc = subprocess.Popen([sys.executable, __file__, str(root), "--deadline-probe"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            out, err = proc.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill(); out, err = proc.communicate()
            raise AssertionError("1.8-second interrupt failed; outer 5-second process backstop killed worker: " + err)
        assert proc.returncode == 0, out + err
        value = json.loads(out); value["outer_process_seconds"] = time.monotonic() - started; value["process_exited"] = True
        return value
    check("real deadline and outer process backstop", timeout)
    def spill():
        p = subprocess.run([sys.executable, __file__, str(root), "--spill-probe"], text=True, capture_output=True, timeout=5)
        assert p.returncode == 0, p.stdout + p.stderr
        return json.loads(p.stdout)
    check("actual engine spill reclamation", spill)
    def forced_backstop():
        p = subprocess.Popen([sys.executable, "-c", "import time; print('ready',flush=True); time.sleep(10)"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            p.communicate(timeout=0.5)
            raise AssertionError("unresponsive-worker negative control unexpectedly exited")
        except subprocess.TimeoutExpired:
            p.kill(); out, err = p.communicate(timeout=2)
            assert "ready" in out and p.returncode == -9
        return {"forced_control_killed": True, "timeout_seconds": 0.5, "process_reaped": p.poll() is not None}
    check("unresponsive worker kill control", forced_backstop)
    def exact():
        total = execute_query(root, "s02.parquet", workloads()[0], manifest)
        assert total["rows"] == [["1140000"]]
        selected = typed_query([{"type": "filter", "field": field_ref("destination"), "comparison": "eq", "value": {"type": "text", "value": "198.51.100.7"}}, measure("sum")])
        filtered = execute_query(root, "s02.parquet", selected, manifest)
        assert filtered["rows"] == [["900000"]]
        count = execute_query(root, "s02.parquet", workloads()[1], manifest)
        assert count["rows"] == [["5"]]
        timeline = execute_query(root, "s02.parquet", workloads()[3], manifest)
        actual = {int(t[14:16]): int(value) for t, value in timeline["rows"]}
        assert [actual.get(i, 0) for i in range(10)] == manifest["s02"]["timeline"]
        # Zero fill is qualified only for this explicitly complete coverage fixture.
        page = typed_query([{"type": "project", "fields": [field_ref("event_id"), field_ref("bytes")]}, {"type": "sort", "fields": [{"field": field_ref("event_id"), "direction": "asc"}], "stable_tiebreak": field_ref("event_id")}, {"type": "limit", "rows": 1}])
        paged = execute_query(root, "s02.parquet", page, manifest)
        assert paged["rows"] == [["1", "60000"]] and paged["receipt"]["truncated_page"] is True
        assert execute_query(root, "s02.parquet", workloads()[0], manifest)["receipt"] == total["receipt"]
        assert total["receipt"]["digest"] != filtered["receipt"]["digest"]
        return {"total": total, "selected": filtered, "included_count": count["rows"], "timeline": timeline["rows"], "whole_window_rate_oracle": 1140000 // 10, "pagination_does_not_redefine_total": True, "CIDR_matching": "not qualified"}
    check("S02 exact arithmetic and receipt identity", exact)
    expected = expected_workloads(manifest)
    cold = []
    for sample in range(5):
        started = time.monotonic()
        p = subprocess.run([sys.executable, __file__, str(root), "--cold-sample"], capture_output=True, text=True, timeout=5)
        if p.returncode:
            cold.append({"sample": sample, "passed": False, "error": p.stderr or p.stdout})
        else:
            value = json.loads(p.stdout)
            cold.append({"sample": sample, "passed": value["rows"] == expected[0], "process_wall_seconds": time.monotonic() - started, **value})
    warm = []
    # One explicitly excluded warm-up; every measured operation still gets its own engine.
    execute_query(root, "million.parquet", workloads()[0], manifest)
    for sample in range(5):
        broker = Admission(); started = time.monotonic()
        futures = [broker.submit(supervised_query, root, "million.parquet", q, manifest, start=started) for q in workloads()]
        values = []
        try:
            for i, future in enumerate(futures):
                try:
                    result = future.result(timeout=5)
                    values.append({"passed": result["rows"] == expected[i], **result})
                except Exception as exc:
                    values.append({"passed": False, "error": type(exc).__name__ + ": " + str(exc)})
        finally:
            broker.close()
        warm.append({"sample": sample, "elapsed_seconds": time.monotonic() - started, "max_running": broker.max_active, "views": values})
    report.update({"checks": checks, "cold_samples": cold, "warm_six_view_samples": warm,
                   "peak_rss_native_units": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
                   "passed": all(x["passed"] for x in checks + cold) and all(v["passed"] for x in warm for v in x["views"]),
                   "latency_definition": "Cold: fresh process/engine and empty result cache, immutable revision already local, OS page cache NOT reset; process wall separately. Warm: primed local revision, six declared analytical views with two active slots, independently supervised fresh process/engine per query and no result cache; elapsed includes process startup, digest verification, contract validation, queue, engine work, receipt validation and delivery. This is not the full Northstar six-view or managed client p99."})
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(); parser.add_argument("root", type=Path); parser.add_argument("--container", action="store_true"); parser.add_argument("--deadline-probe", action="store_true"); parser.add_argument("--cold-sample", action="store_true"); parser.add_argument("--spill-probe", action="store_true"); parser.add_argument("--query-request", action="store_true")
    args = parser.parse_args(); root = args.root.resolve()
    if args.query_request:
        request = json.load(sys.stdin)
        try:
            value = execute_query(root, request["name"], request["query"], json.loads((root / "manifest.json").read_text()), epoch=request["epoch"], start=request["start"])
        except (Refused, ContractError, Deadline) as exc:
            print(json.dumps({"error": "deadline" if isinstance(exc, Deadline) else "narrow_the_query", "reason": str(exc)[:500]}))
            raise SystemExit(2)
    elif args.deadline_probe:
        value = deadline_probe(root)
    elif args.spill_probe:
        value = spill_probe(root)
    elif args.cold_sample:
        value = execute_query(root, "million.parquet", workloads()[0], json.loads((root / "manifest.json").read_text()))
    else:
        value = suite(root, args.container)
    print(json.dumps(value, indent=2))
    if "passed" in value and not value["passed"]:
        raise SystemExit(1)
