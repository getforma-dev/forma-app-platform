"""Closed typed-query spike. Arbitrary SQL is never accepted by execute_query."""
from __future__ import annotations

import concurrent.futures
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import threading
import time
import uuid

import duckdb

CONTRACTS = Path(os.environ["FORMA_CONTRACTS"]) if "FORMA_CONTRACTS" in os.environ else Path(__file__).resolve().parents[2] / "contracts"
sys.path.insert(0, str(CONTRACTS))
from validate import ContractError, canonical, digest, validate
from fixture import file_digest

FIELDS = {"event_id": {"type": "integer", "unit": "count"}, "time": {"type": "timestamp"}, "destination": {"type": "text"}, "bytes": {"type": "integer", "unit": "byte"}}
INPUT = {"id": "events", "contract_id": "connection_events_v1", "contract_version": 1, "kind": "source", "fields": [{"id": name, "value_type": value, "nullable": False} for name, value in FIELDS.items()]}
UUID = "00000000-0000-4000-8000-000000000001"
POLICY = {"threads": 2, "memory_limit": "1GiB", "max_temp_directory_size": "2GiB", "enable_external_access": False,
          "autoinstall_known_extensions": False, "autoload_known_extensions": False, "allow_unsigned_extensions": False,
          "allow_community_extensions": False, "allow_persistent_secrets": False,
          "allowed_directories": [], "allowed_configs": []}


class Refused(ValueError):
    pass


class Deadline(TimeoutError):
    pass


def field_ref(name):
    return {"input_id": "events", "field_id": name}


def typed_query(operations, window=True):
    value = {"operator_version": 1, "inputs": ["events"], "operations": operations, "null_policy": "unknown"}
    if window:
        value["window"] = {"start": "2026-10-22T10:00:00Z", "end": "2026-10-22T10:10:00Z", "bounds": "half_open"}
    return value


def measure(operation, field="bytes", group=None):
    return {"type": "aggregate", "group_by": [field_ref(x) for x in (group or [])],
            "measures": [{"id": "value", "operation": operation, "field": field_ref(field)}]}


def validate_query(query):
    content = {"title": "Bounded query qualification", "description": "Trusted synthetic harness", "inputs": [INPUT],
               "views": [{"id": "result", "kind": "table", "renderer_version": 1, "title": "Result", "query": query}],
               "record_schemas": [], "commands": [], "links": [], "default_layout": [{"view_id": "result", "column": 0, "row": 0, "width": 12}]}
    validate({"kind": "app_definition", "schema_version": 1, "organization_id": UUID, "app_id": UUID, "revision_id": UUID,
              "content": content, "digest": digest("app-definition", content)})
    if query["inputs"] != ["events"]:
        raise Refused("only the registered events input is qualified")
    return content


def compile_query(query, source_path):
    validate_query(query)
    params = [str(source_path)]
    sql = "SELECT * FROM read_parquet(?)"
    available = set(FIELDS)
    aggregated = False
    if "window" in query:
        sql += " WHERE time >= ?::TIMESTAMP AND time < ?::TIMESTAMP"
        params += [query["window"]["start"], query["window"]["end"]]

    def column(ref):
        if ref["input_id"] != "events" or ref["field_id"] not in available:
            raise Refused("field is unavailable at this pipeline stage")
        return '"' + ref["field_id"] + '"'

    for index, op in enumerate(query["operations"]):
        source = f"({sql}) AS q{index}"
        kind = op["type"]
        if kind == "filter":
            if aggregated:
                raise Refused("post-aggregate filters are not qualified")
            name = column(op["field"])
            comparison = {"eq": "=", "ne": "<>", "lt": "<", "lte": "<=", "gt": ">", "gte": ">="}[op["comparison"]]
            value = op["value"]
            if value["type"] in ("text", "timestamp", "date", "enum"):
                parameter = value["value"]
            elif value["type"] == "integer":
                parameter = int(value["value"])
                if not -(2**63) <= parameter < 2**63:
                    raise Refused("this qualification stores signed 64-bit integer columns")
            else:
                raise Refused("literal type is not in the qualified query subset")
            sql = f"SELECT * FROM {source} WHERE {name} {comparison} ?"
            params.append(parameter)
        elif kind == "project":
            if aggregated:
                raise Refused("post-aggregate projection is not qualified")
            names = [column(f) for f in op["fields"]]
            sql = f"SELECT {', '.join(names)} FROM {source}"
            available = {f["field_id"] for f in op["fields"]}
        elif kind == "aggregate":
            if aggregated:
                raise Refused("multiple aggregation stages are not qualified")
            groups = [column(f) for f in op["group_by"]]
            measures = []
            for item in op["measures"]:
                value = column(item["field"])
                operator = {"count": "count", "sum": "sum", "min": "min", "max": "max"}[item["operation"]]
                # Identifiers are schema-validated ASCII, literals are parameter-bound.
                measures.append(f'{operator}({value}) AS "{item["id"]}"')
            sql = f"SELECT {', '.join(groups + measures)} FROM {source}"
            if groups:
                sql += " GROUP BY " + ", ".join(groups) + " ORDER BY " + ", ".join(groups)
            available = {f["field_id"] for f in op["group_by"]} | {m["id"] for m in op["measures"]}
            aggregated = True
        elif kind == "time_bucket":
            if aggregated:
                raise Refused("time bucket after aggregate is not qualified")
            name = column(op["field"])
            # Replace the declared time column with its UTC bucket; the output ID
            # must match it because this initial schema has no derived-field refs.
            if op["output_field"] != op["field"]["field_id"]:
                raise Refused("qualified time bucket preserves the declared field ID")
            sql = f"SELECT * REPLACE (time_bucket(INTERVAL '{op['seconds']} seconds', {name}) AS {name}) FROM {source}"
        elif kind == "sort":
            names = [column(f["field"]) + " " + {"asc": "ASC", "desc": "DESC"}[f["direction"]] for f in op["fields"]]
            names.append(column(op["stable_tiebreak"]) + " ASC")
            sql = f"SELECT * FROM {source} ORDER BY " + ", ".join(names)
        elif kind == "limit":
            if index != len(query["operations"]) - 1:
                raise Refused("pagination must be last; totals use full filtered input")
            sql = f"SELECT * FROM {source} LIMIT {op['rows'] + 1}"
        else:
            raise Refused(f"operator {kind} is not qualified (no fallback)")
    # Bound transferred rows independently of full-input totals.
    return "SELECT * FROM (" + sql + ") AS bounded_result LIMIT 501", params


def authorized_source(root, name, manifest):
    root = Path(root).resolve(strict=True)
    if name not in ("s02.parquet", "million.parquet"):
        raise Refused("unknown immutable source revision")
    path = root / name
    if path.is_symlink() or path.resolve(strict=True).parent != root:
        raise Refused("source path is not a regular in-scope revision")
    if file_digest(path) != manifest["files"][name]["digest"]:
        raise Refused("source digest mismatch")
    return path


def connect(root, allowed, scratch, memory_limit=None):
    runtime = {"enable_external_access", "allowed_paths", "allowed_directories", "allowed_configs"}
    config = {**{k: v for k, v in POLICY.items() if k not in runtime}, "temp_directory": str(scratch)}
    if memory_limit is not None:
        config["memory_limit"] = memory_limit  # Trusted tighter-limit resource test only.
    # DuckDB otherwise shares a file's instance/configuration inside one process.
    # Each operation gets a private copy of the empty database, opened read-only.
    database = Path(scratch) / "query.duckdb"
    shutil.copyfile(Path(root) / "empty.duckdb", database)
    connection = duckdb.connect(str(database), read_only=True, config=config)
    try:
        connection.execute("SET allowed_paths=?", [[str(p) for p in allowed]])
        connection.execute("SET allowed_directories=[]")
        connection.execute("SET allowed_configs=[]")
        connection.execute("SET enable_logging=false")
        connection.execute("SET enable_external_access=false")
        connection.execute("SET lock_configuration=true")
        return connection
    except Exception:
        connection.close()
        raise


def encode_rows(rows):
    def encode(value):
        if isinstance(value, datetime.datetime):
            return value.strftime("%Y-%m-%dT%H:%M:%SZ")
        if type(value) is int:
            return str(value)
        if value is None or isinstance(value, (str, bool)):
            return value
        raise Refused("unqualified result type")
    return [[encode(v) for v in row] for row in rows]


def run_sql(root, allowed, sql, params, deadline_seconds=1.8, start=None):
    start = start or time.monotonic()
    remaining = deadline_seconds - (time.monotonic() - start)
    if remaining <= 0:
        raise Deadline("deadline exhausted before engine admission")
    scratch = Path(tempfile.mkdtemp(prefix="query-", dir=os.environ.get("FORMA_QUERY_TMP")))
    connection = None
    timer = None
    fired = threading.Event()
    try:
        connection = connect(root, allowed, scratch)
        def interrupt():
            fired.set()
            connection.interrupt()
        remaining = deadline_seconds - (time.monotonic() - start)
        if remaining <= 0:
            raise Deadline("deadline exhausted during engine setup")
        timer = threading.Timer(remaining, interrupt)
        timer.start()
        try:
            cursor = connection.execute(sql, params)
            rows = cursor.fetchall()
        except duckdb.InterruptException as exc:
            raise Deadline("query interrupted at caller deadline") from exc
        if fired.is_set() or time.monotonic() - start >= deadline_seconds:
            raise Deadline("result not released after deadline")
        return encode_rows(rows)
    finally:
        if timer:
            timer.cancel()
            timer.join()
        if connection:
            connection.close()
        shutil.rmtree(scratch)


def execute_query(root, name, query, manifest, epoch=1, start=None):
    start = start or time.monotonic()
    path = authorized_source(root, name, manifest)
    sql, params = compile_query(query, path)
    rows = run_sql(root, [path], sql, params, start=start)
    limit = query["operations"][-1].get("rows") if query["operations"][-1]["type"] == "limit" else None
    truncated = limit is not None and len(rows) > limit
    if limit is not None:
        rows = rows[:limit]
    elif len(rows) > 500:
        raise Refused("result exceeds qualified 500-row page; narrow the query")
    result = {"columns_are_query_defined": True, "rows": rows}
    receipt = {"kind": "query_receipt", "schema_version": 1, "organization_id": UUID, "app_id": UUID, "work_id": UUID,
               "mode": "synthetic_preview", "definition_digest": digest("app-definition", validate_query(query)), "view_id": "result", "operator_version": 1,
               "source_revisions": [{"input_id": "events", "source_id": UUID, "source_revision_id": str(uuid.UUID(bytes=bytes.fromhex(manifest["files"][name]["digest"][7:39]), version=4)), "source_digest": manifest["files"][name]["digest"]}],
               "record_snapshot_sequence": 0, "parameters": [], "principal_id": UUID, "membership_epoch": 1, "policy_epoch": 1, "filter_epoch": epoch,
               "result_digest": digest("result", result), "partial": False, "truncated_page": truncated, "total_scope": "full_authorized_filtered_input",
               "provenance": {"query_digest": digest("query", query), "predicate_digest": digest("query", query)}}
    receipt["digest"] = digest("query-receipt", receipt)
    validate(receipt)
    if time.monotonic() - start >= 1.8:
        raise Deadline("result validation exhausted deadline; result withheld")
    return {"rows": rows, "receipt": receipt, "elapsed_seconds": time.monotonic() - start}


class Admission:
    """Two running queries, at most twenty waiting; separate from authentication."""
    def __init__(self):
        self.executor = concurrent.futures.ThreadPoolExecutor(max_workers=2)
        self.slots = threading.BoundedSemaphore(22)
        self.lock = threading.Lock()
        self.active = 0
        self.max_active = 0
        self.accepted = 0
        self.rejected = 0

    def submit(self, function, *args, **kwargs):
        enqueued_at = time.monotonic()
        # A caller can supply an earlier deadline origin (e.g. a six-view burst),
        # but cannot reset it when a queued operation eventually gets a slot.
        kwargs["start"] = min(kwargs.get("start", enqueued_at), enqueued_at)
        if not self.slots.acquire(blocking=False):
            self.rejected += 1
            raise Refused("429: organization queue full (2 active + 20 queued)")
        self.accepted += 1
        def execute():
            with self.lock:
                self.active += 1
                self.max_active = max(self.max_active, self.active)
            try:
                if time.monotonic() - kwargs["start"] >= 1.8:
                    raise Deadline("deadline expired in admission queue; no worker launched")
                return function(*args, **kwargs)
            finally:
                with self.lock:
                    self.active -= 1
                self.slots.release()
        return self.executor.submit(execute)

    def close(self):
        self.executor.shutdown(wait=True)
