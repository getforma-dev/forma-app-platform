"""APP-108 box data normalizer: a confirmed CSV of a box to the engine's Parquet.

The `runtime.py normalize` child (no credential, empty environment, its three
pipes; spawned by the Rust binary per registration under
`supervise::run_confined`) reads one JSON request on stdin, `{"contract":
<input contract>, "csv": "<canonical CSV text>", "target": "<absolute path>"}`,
writes the normalized Parquet to `target` and prints one JSON line
`{"normalized_digest": "sha256:…", "row_count": n}`; any refusal prints
`{"refused": "<code>"}` and exits 1.

The normalization is exactly `library_sources.normalize` (ADR-011 pitch,
APP-094; the operator's `normalize`): DuckDB, one thread, `FORMAT PARQUET,
COMPRESSION ZSTD, ROW_GROUP_SIZE 100000`, rows ordered by the `time`
timestamp field then the original record, the same typing table (money and
decimals as BIGINT coefficients, never a float), and the round-trip check.
It is copied here because the service image ships `runtime.py`,
`query_engine.py` and this file only; `test_library.py` proves both produce
the same bytes for every library CSV. The Rust caller builds the canonical
CSV from the sealed parser's cells under the member's confirmed types
(`boxes::inspect::canonical_csv`); this child re-checks every cell.
"""
from __future__ import annotations

import csv
import datetime
import hashlib
import io
import json
from pathlib import Path
import re
import sys

INTEGER = re.compile(r"-?(0|[1-9][0-9]{0,37})")
DECIMAL = re.compile(r"(-?)(0|[1-9][0-9]*)(?:\.([0-9]+))?")
TIMESTAMP = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z")
DATE = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}")
LOCAL_ID = re.compile(r"[a-z][a-z0-9_]{0,63}")
MAX_CSV_BYTES = 64 * 1024 * 1024
MAX_FIELDS = 32


class Refused(ValueError):
    pass


def require(condition, reason):
    if not condition:
        raise Refused(reason)


def sha256(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def column_type(value_type):
    kind = value_type["type"]
    if kind in ("text", "enum"):
        return "VARCHAR"
    if kind == "boolean":
        return "BOOLEAN"
    if kind in ("integer", "money", "decimal"):
        return "BIGINT"
    if kind == "date":
        return "DATE"
    if kind == "timestamp":
        return "TIMESTAMP"
    raise Refused("field_type_refused")


def scaled_integer(text, scale):
    match = DECIMAL.fullmatch(text)
    require(match is not None, "decimal_refused")
    sign, whole, fraction = match.group(1), match.group(2), match.group(3) or ""
    require(len(fraction) <= scale, "decimal_scale_refused")
    coefficient = int(whole) * 10**scale + int(fraction.ljust(scale, "0"))
    require(coefficient < 2**63, "integer_range_refused")
    return -coefficient if sign else coefficient


def typed_value(field, text):
    kind = field["value_type"]["type"]
    if text == "":
        require(field.get("nullable", False), "null_refused")
        return None
    if kind == "text":
        return text
    if kind == "enum":
        require(text in field["value_type"]["values"], "enum_refused")
        return text
    if kind == "boolean":
        require(text in ("true", "false"), "boolean_refused")
        return text == "true"
    if kind == "integer":
        require(INTEGER.fullmatch(text) is not None, "integer_refused")
        value = int(text)
        require(-(2**63) <= value < 2**63, "integer_range_refused")
        return value
    if kind == "money":
        require(field["value_type"]["currency"] == "USD" and field["value_type"]["minor_unit_exponent"] == 2, "money_refused")
        require(DECIMAL.fullmatch(text) is not None and len((DECIMAL.fullmatch(text).group(3) or "")) == 2, "money_refused")
        return scaled_integer(text, 2)
    if kind == "decimal":
        return scaled_integer(text, field["value_type"]["scale"])
    if kind == "date":
        require(DATE.fullmatch(text) is not None, "date_refused")
        return datetime.date.fromisoformat(text)
    if kind == "timestamp":
        require(TIMESTAMP.fullmatch(text) is not None, "timestamp_refused")
        return datetime.datetime.strptime(text, "%Y-%m-%dT%H:%M:%SZ")
    raise Refused("field_type_refused")


def check_contract(contract):
    require(isinstance(contract, dict) and contract.get("kind") == "source", "contract_refused")
    fields = contract.get("fields")
    require(isinstance(fields, list) and 0 < len(fields) <= MAX_FIELDS, "contract_refused")
    names = [f.get("id") for f in fields]
    require(all(isinstance(n, str) and LOCAL_ID.fullmatch(n) for n in names) and len(set(names)) == len(names), "contract_refused")
    for f in fields:
        column_type(f["value_type"])
    return fields


def rows_of(contract, data):
    fields = contract["fields"]
    reader = csv.reader(io.StringIO(data.decode("utf-8"), newline=""))
    header = next(reader, None)
    require(header == [f["id"] for f in fields], "header_refused")
    rows = []
    for record in reader:
        require(len(record) == len(fields), "row_shape_refused")
        rows.append(tuple(typed_value(field, cell) for field, cell in zip(fields, record)))
    require(0 < len(rows) <= 1000000, "row_count_refused")
    return rows


def normalize(contract, data, target):
    """library_sources.normalize, verbatim in behavior: (normalized_digest, row_count)."""
    import duckdb
    fields = contract["fields"]
    rows = rows_of(contract, data)
    names = [f["id"] for f in fields]
    time_index = next((i for i, f in enumerate(fields) if f["id"] == "time" and f["value_type"]["type"] == "timestamp"), None)
    ordered = sorted(enumerate(rows, 1), key=lambda item: ((item[1][time_index], item[0]) if time_index is not None else (item[0],)))
    declarations = ", ".join('"%s" %s%s' % (f["id"], column_type(f["value_type"]), "" if f.get("nullable") else " NOT NULL") for f in fields)
    quoted = ", ".join('"%s"' % n for n in names)
    target = Path(target)
    connection = duckdb.connect()
    try:
        connection.execute("SET threads=1")
        connection.execute("CREATE TABLE source(%s, original INTEGER NOT NULL)" % declarations)
        connection.executemany("INSERT INTO source VALUES (%s)" % ", ".join("?" * (len(names) + 1)), [(*row, index) for index, row in ordered])
        connection.execute("COPY (SELECT %s FROM source ORDER BY original) TO ? (FORMAT PARQUET, COMPRESSION ZSTD, ROW_GROUP_SIZE 100000)" % quoted,
                           [str(target)])
        back = connection.execute("SELECT %s FROM read_parquet(?)" % quoted, [str(target)]).fetchall()
        columns = [row[0] for row in connection.execute("DESCRIBE SELECT * FROM read_parquet(?)", [str(target)]).fetchall()]
    finally:
        connection.close()
    require(columns == names and back == [row for _, row in ordered], "normalized_round_trip_refused")
    return sha256(target.read_bytes()), len(rows)


def main(stdin=None, stdout=None):
    stdin = stdin or sys.stdin.buffer
    stdout = stdout or sys.stdout
    try:
        raw = stdin.read(MAX_CSV_BYTES + 65537)
        require(len(raw) <= MAX_CSV_BYTES + 65536, "request_refused")
        request = json.loads(raw)
        require(isinstance(request, dict) and set(request) == {"contract", "csv", "target"}, "request_refused")
        target = Path(request["target"])
        require(target.is_absolute() and not target.exists() and target.parent.is_dir(), "target_refused")
        contract = request["contract"]
        check_contract(contract)
        digest, count = normalize(contract, request["csv"].encode("utf-8"), target)
        print(json.dumps({"normalized_digest": digest, "row_count": count}), file=stdout)
        return 0
    except Refused as refusal:
        print(json.dumps({"refused": str(refusal)}), file=stdout)
        return 1
    except Exception:
        print(json.dumps({"refused": "normalize_failed"}), file=stdout)
        return 1


if __name__ == "__main__":
    sys.exit(main())
