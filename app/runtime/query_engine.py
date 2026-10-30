"""APP-012 confined typed-query engine child (ADR-007 D7.10). Arbitrary SQL is never accepted.

Source identity: copied from `spikes/APP-005/query.py` at `25f92d6` (sha256
d7d724b9e34c3bdb73316eb9f3017c082dfedc3a7b79503cea584d67e06cf187; the
qualified APP-005 executor). `POLICY`, `connect`, `encode_rows`, `run_sql` and
`compile_query` are that file's code; the deviations, each recorded in
evidence/APP-012/README.md, are:

1. `FIELDS` is the contract `events_v1` shape (`time`, `destination`, `bytes`),
   the three fields the operator's normalized Parquet holds (APP-077 open
   question 3); APP-005's `event_id` is not a contract field.
2. `time_bucket` may name a new output field (the seeded Northstar `timeline`
   view buckets `time` as `minute`): the bucket replaces the time column in
   place under that name, and the time field is no longer addressable.
3. APP-005's `validate_query` (whole-query schema validation in the child) is
   not copied. The stored definition is re-admitted by the Rust process
   (APP-010) before it builds this request, and the run parameters the Rust
   process adds to the stored view query (the window, the destination filter)
   are judged there against the contract's rules (a window bound is a contract
   `TIMESTAMP` naming a real day). The effective query is therefore not
   schema-validated here; `compile_query` still refuses every operator,
   literal and shape outside the qualified subset, re-checks both window
   bounds with `strptime` (defense in depth, APP-012 review M2) and requires
   every measure id it interpolates as an SQL alias to be a local identifier
   (review m5).
4. The source is not chosen from a fixture manifest: the request names one
   revision of the request's organization, whose file the credentialed Rust
   step already placed in the read-only cache; this child re-checks that it is
   a regular file directly inside `<cache>/o/<organization>/` and that its
   SHA-256 equals the revision's `normalized_digest`.
5. The empty read-only database is created inside the private scratch
   directory the Rust parent made for this request (no prepared file), and the
   receipt is built and digested by the Rust process (`query::receipt`);
   admission (2 per organization, 4 per task, 20 queued) is the Rust
   parent's, so `Admission` is not copied.
6. (ADR-011 pitch library, APP-094.) The compiled field set is no longer the
   fixed `events_v1` shape: the request's `source` names the view's one input
   (`input_id`) and carries that input contract's `fields` verbatim, taken by
   the Rust parent from the re-admitted definition (never from the client), and
   `compile_query` types every column, literal and measure from them under the
   normalization table of `library_sources.py` (text/enum VARCHAR, boolean,
   integer/money/decimal BIGINT, date, timestamp). A query window applies to
   the field named `time` and requires it to be a timestamp; the Rust parent
   refuses a window or destination parameter for an input without that
   field. Everything else (POLICY, the one-file allowlist, the read-only
   private database, the closed request, the single result line, the 500-row
   page, the deadline and the fixed refusal codes) is unchanged.

Protocol: one JSON request on stdin, exactly one JSON line on stdout. The
child runs with an empty environment, its three pipes and nothing else
(`supervise::run_confined`).
"""
from __future__ import annotations

import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sys
import threading
import time

# The seeded Northstar `events_v1` shape, kept as the reference example of a field list (deviation 6).
FIELDS = [{"id": "time", "value_type": {"type": "timestamp"}, "nullable": False},
          {"id": "destination", "value_type": {"type": "text"}, "nullable": False},
          {"id": "bytes", "value_type": {"type": "integer", "unit": "byte"}, "nullable": False}]
# Contract value types a source column may have (library_sources.py's table); `reference` never is.
COLUMN_TYPES = ("text", "enum", "boolean", "integer", "money", "decimal", "date", "timestamp")
# Measures that need a numeric field; count/min/max take any column type.
NUMERIC = ("integer", "money", "decimal")
LOCAL_ID = re.compile(r"[a-z][a-z0-9_]{0,63}")
MAX_FIELDS = 32
POLICY = {"threads": 2, "memory_limit": "1GiB", "max_temp_directory_size": "2GiB", "enable_external_access": False,
          "autoinstall_known_extensions": False, "autoload_known_extensions": False, "allow_unsigned_extensions": False,
          "allow_community_extensions": False, "allow_persistent_secrets": False,
          "allowed_directories": [], "allowed_configs": []}
# APP-005's interactive deadline; the parent kills at its 5 s backstop.
DEADLINE_SECONDS = 1.8
MAX_REQUEST_BYTES = 65536
UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}")
DIGEST = re.compile(r"sha256:[0-9a-f]{64}")
REQUEST_KEYS = {"organization_id", "source", "query", "deadline_ms", "cache_root", "scratch"}
SOURCE_KEYS = {"source_id", "revision_id", "normalized_digest", "s3_key", "input_id", "fields"}


class Refused(ValueError):
    pass


class Deadline(TimeoutError):
    pass


def require(condition, reason):
    if not condition:
        raise Refused(reason)


def parse_fields(fields):
    """The input contract's field list, re-checked here (defense in depth): id -> value_type."""
    require(isinstance(fields, list) and 0 < len(fields) <= MAX_FIELDS, "fields_refused")
    typed = {}
    for item in fields:
        require(isinstance(item, dict) and set(item) == {"id", "value_type", "nullable"}, "fields_refused")
        name, value_type = item["id"], item["value_type"]
        require(isinstance(name, str) and LOCAL_ID.fullmatch(name) is not None and name not in typed, "fields_refused")
        require(isinstance(value_type, dict) and value_type.get("type") in COLUMN_TYPES and isinstance(item["nullable"], bool), "fields_refused")
        typed[name] = value_type
    return typed


def literal_parameter(value, value_type):
    """A filter literal as a bound parameter plus its SQL cast, or a refusal; the type must be the column's."""
    require(isinstance(value, dict) and value.get("type") == value_type["type"], "literal type is not in the qualified query subset")
    kind = value["type"]
    if kind in ("text", "enum"):
        require(isinstance(value.get("value"), str), "literal_not_qualified")
        return value["value"], ""
    if kind == "boolean":
        require(isinstance(value.get("value"), bool), "literal_not_qualified")
        return value["value"], ""
    if kind == "timestamp":
        require(isinstance(value.get("value"), str), "literal_not_qualified")
        try:
            datetime.datetime.strptime(value["value"], "%Y-%m-%dT%H:%M:%SZ")
        except ValueError:
            raise Refused("literal_not_qualified") from None
        return value["value"], "::TIMESTAMP"
    if kind == "date":
        require(isinstance(value.get("value"), str), "literal_not_qualified")
        try:
            datetime.date.fromisoformat(value["value"])
        except ValueError:
            raise Refused("literal_not_qualified") from None
        return value["value"], "::DATE"
    if kind == "integer":
        require(value.get("unit") == value_type.get("unit") and isinstance(value.get("value"), str), "literal_not_qualified")
        parameter = int(value["value"])
    elif kind == "money":
        require(value.get("currency") == value_type.get("currency") == "USD" and value.get("minor_unit_exponent") == value_type.get("minor_unit_exponent") == 2
                and isinstance(value.get("minor_units"), str), "literal_not_qualified")
        parameter = int(value["minor_units"])
    elif kind == "decimal":
        require(value.get("unit") == value_type.get("unit") and value.get("scale") == value_type.get("scale")
                and isinstance(value.get("coefficient"), str), "literal_not_qualified")
        parameter = int(value["coefficient"])
    else:
        raise Refused("literal type is not in the qualified query subset")
    if not -(2**63) <= parameter < 2**63:
        raise Refused("this qualification stores signed 64-bit integer columns")
    return parameter, ""


def compile_query(query, source_path, input_id="events", fields=FIELDS):
    require(isinstance(query, dict) and set(query) <= {"operator_version", "inputs", "operations", "null_policy", "window"}
            and query.get("operator_version") == 1 and query.get("null_policy") == "unknown"
            and isinstance(query.get("operations"), list) and 0 < len(query["operations"]) <= 16, "query_shape_refused")
    require(isinstance(input_id, str) and LOCAL_ID.fullmatch(input_id) is not None, "request_refused")
    types = parse_fields(fields)
    if query["inputs"] != [input_id]:
        raise Refused("only the request's one source input is qualified")
    params = [str(source_path)]
    sql = "SELECT * FROM read_parquet(?)"
    available = set(types)
    aggregated = False
    if "window" in query:
        require(query["window"].get("bounds") == "half_open", "window_refused")
        for bound in ("start", "end"):
            # Review M2: an impossible date (2026-02-31) or year 0000 never reaches DuckDB.
            try:
                datetime.datetime.strptime(query["window"][bound], "%Y-%m-%dT%H:%M:%SZ")
            except (TypeError, ValueError):
                raise Refused("window_refused") from None
        require(query["window"]["start"] < query["window"]["end"], "window_refused")
        # Deviation 6: the window is the half-open range of the `time` timestamp field.
        require(types.get("time", {}).get("type") == "timestamp", "window_refused")
        sql += ' WHERE "time" >= ?::TIMESTAMP AND "time" < ?::TIMESTAMP'
        params += [query["window"]["start"], query["window"]["end"]]

    def column(ref):
        if not isinstance(ref, dict) or ref.get("input_id") != input_id or ref.get("field_id") not in available:
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
            parameter, cast = literal_parameter(op["value"], types[op["field"]["field_id"]])
            sql = f"SELECT * FROM {source} WHERE {name} {comparison} ?{cast}"
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
                if operator == "sum" and types[item["field"]["field_id"]]["type"] not in NUMERIC:
                    raise Refused("query_shape_refused")
                # The measure id becomes an SQL alias: a local identifier only (review m5);
                # literals are parameter-bound.
                require(isinstance(item["id"], str) and LOCAL_ID.fullmatch(item["id"]) is not None, "query_shape_refused")
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
            require(types[op["field"]["field_id"]]["type"] == "timestamp", "time_bucket_refused")
            require(op.get("timezone") == "UTC" and op.get("seconds") in (60, 300, 3600, 86400), "time_bucket_refused")
            output = op["output_field"]
            require(isinstance(output, str) and LOCAL_ID.fullmatch(output) is not None, "time_bucket_refused")
            if output == op["field"]["field_id"]:
                sql = f"SELECT * REPLACE (time_bucket(INTERVAL '{op['seconds']} seconds', {name}) AS {name}) FROM {source}"
            else:
                # Deviation 2: the bucket replaces the time column in place under the new name.
                require(output not in available, "time_bucket_refused")
                sql = (f"SELECT * RENAME ({name} AS \"{output}\") FROM "
                       f"(SELECT * REPLACE (time_bucket(INTERVAL '{op['seconds']} seconds', {name}) AS {name}) FROM {source}) AS b{index}")
                available = (available - {op["field"]["field_id"]}) | {output}
        elif kind == "sort":
            names = [column(f["field"]) + " " + {"asc": "ASC", "desc": "DESC"}[f["direction"]] for f in op["fields"]]
            names.append(column(op["stable_tiebreak"]) + " ASC")
            sql = f"SELECT * FROM {source} ORDER BY " + ", ".join(names)
        elif kind == "limit":
            if index != len(query["operations"]) - 1:
                raise Refused("pagination must be last; totals use full filtered input")
            sql = f"SELECT * FROM {source} LIMIT {int(op['rows']) + 1}"
        else:
            raise Refused(f"operator {kind} is not qualified (no fallback)")
    # Bound transferred rows independently of full-input totals.
    return "SELECT * FROM (" + sql + ") AS bounded_result LIMIT 501", params


def file_digest(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        while block := stream.read(1024 * 1024):
            h.update(block)
    return "sha256:" + h.hexdigest()


def cached_source(cache_root, organization, source):
    """Deviation 4: the one cached revision of this organization, digest re-checked."""
    root = Path(cache_root).resolve(strict=True) / "o" / organization
    require(root.is_dir() and not root.is_symlink(), "source_unavailable")
    path = root / f"{source['source_id']}.{source['revision_id']}.parquet"
    require(path.exists() and not path.is_symlink() and path.is_file() and path.resolve(strict=True).parent == root.resolve(strict=True),
            "source_unavailable")
    require(file_digest(path) == source["normalized_digest"], "source_digest_mismatch")
    return path


def connect(scratch, allowed, memory_limit=None):
    import duckdb
    runtime = {"enable_external_access", "allowed_paths", "allowed_directories", "allowed_configs"}
    config = {**{k: v for k, v in POLICY.items() if k not in runtime}, "temp_directory": str(scratch)}
    if memory_limit is not None:
        config["memory_limit"] = memory_limit  # Trusted tighter-limit resource test only.
    # Deviation 5: a private empty database for this one operation, opened read-only.
    database = Path(scratch) / "query.duckdb"
    duckdb.connect(str(database), config={"threads": 1, "enable_external_access": False}).close()
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
        if isinstance(value, datetime.date):
            return value.isoformat()
        if type(value) is int:
            return str(value)
        if value is None or isinstance(value, (str, bool)):
            return value
        raise Refused("unqualified result type")
    return [[encode(v) for v in row] for row in rows]


def run_sql(scratch, allowed, sql, params, deadline_seconds=DEADLINE_SECONDS, start=None):
    import duckdb
    start = start or time.monotonic()
    remaining = deadline_seconds - (time.monotonic() - start)
    if remaining <= 0:
        raise Deadline("deadline exhausted before engine admission")
    connection = None
    timer = None
    fired = threading.Event()
    try:
        connection = connect(scratch, allowed)

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
            columns = [item[0] for item in cursor.description]
            rows = cursor.fetchall()
        except duckdb.InterruptException as exc:
            raise Deadline("query interrupted at caller deadline") from exc
        if fired.is_set() or time.monotonic() - start >= deadline_seconds:
            raise Deadline("result not released after deadline")
        return columns, encode_rows(rows)
    finally:
        if timer:
            timer.cancel()
            timer.join()
        if connection:
            connection.close()


def parse_request(raw):
    require(len(raw) <= MAX_REQUEST_BYTES, "request_refused")
    request = json.loads(raw)
    require(isinstance(request, dict) and set(request) == REQUEST_KEYS, "request_refused")
    source = request["source"]
    require(isinstance(source, dict) and set(source) == SOURCE_KEYS, "request_refused")
    organization = request["organization_id"]
    require(isinstance(organization, str) and UUID.fullmatch(organization) is not None, "request_refused")
    require(all(isinstance(source[k], str) and UUID.fullmatch(source[k]) for k in ("source_id", "revision_id")), "request_refused")
    require(isinstance(source["normalized_digest"], str) and DIGEST.fullmatch(source["normalized_digest"]) is not None, "request_refused")
    require(isinstance(source["input_id"], str) and LOCAL_ID.fullmatch(source["input_id"]) is not None, "request_refused")
    try:
        parse_fields(source["fields"])
    except Refused:
        raise Refused("request_refused") from None
    require(source["s3_key"] == f"o/{organization}/s/{source['source_id']}/r/{source['revision_id']}/part.parquet", "request_refused")
    deadline = request["deadline_ms"]
    require(type(deadline) is int and 1 <= deadline <= int(DEADLINE_SECONDS * 1000), "request_refused")
    for key in ("cache_root", "scratch"):
        require(isinstance(request[key], str) and Path(request[key]).is_absolute(), "request_refused")
    return request


def execute(request, start):
    scratch = Path(request["scratch"])
    require(scratch.is_dir() and not scratch.is_symlink() and not any(scratch.iterdir()), "scratch_refused")
    path = cached_source(request["cache_root"], request["organization_id"], request["source"])
    query = request["query"]
    sql, params = compile_query(query, path, request["source"]["input_id"], request["source"]["fields"])
    columns, rows = run_sql(scratch, [path], sql, params, deadline_seconds=request["deadline_ms"] / 1000, start=start)
    last = query["operations"][-1]
    limit = last.get("rows") if last["type"] == "limit" else None
    truncated = limit is not None and len(rows) > limit
    if limit is not None:
        rows = rows[:limit]
    elif len(rows) > 500:
        raise Refused("narrow_the_query")
    if time.monotonic() - start >= request["deadline_ms"] / 1000:
        raise Deadline("result withheld after deadline")
    return {"outcome": "rows", "columns": columns, "rows": rows, "truncated_page": truncated, "partial": False,
            "total_scope": "full_authorized_filtered_input"}


def refusal_code(reason):
    """A fixed code: the copied compiler's prose messages never leave the child."""
    if re.fullmatch(r"[a-z_]{1,64}", reason):
        return reason
    if "literal" in reason or "integer columns" in reason:
        return "literal_not_qualified"
    if reason.startswith("operator "):
        return "operator_not_qualified"
    return "query_not_qualified"


def main(stdin=None, stdout=None):
    start = time.monotonic()
    stdin = stdin or sys.stdin.buffer
    stdout = stdout or sys.stdout
    try:
        request = parse_request(stdin.read(MAX_REQUEST_BYTES + 1))
        result = execute(request, start)
    except Deadline:
        result = {"outcome": "failed", "reason": "deadline"}
    except Refused as refusal:
        result = {"outcome": "refused", "reason": refusal_code(str(refusal))}
    except Exception:
        # Engine errors can carry paths or values; nothing raw leaves the child.
        result = {"outcome": "failed", "reason": "engine"}
    result["elapsed_ms"] = int((time.monotonic() - start) * 1000)
    stdout.write(json.dumps(result, separators=(",", ":"), sort_keys=True) + "\n")
    stdout.flush()
    return 0


if __name__ == "__main__":
    sys.exit(main())
