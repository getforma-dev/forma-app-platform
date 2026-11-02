"""Confined full Parquet-to-record export. Never touches original source bytes."""
import datetime
import hashlib
import json
from pathlib import Path
import re
import sys

MAX_ROWS = 100000
MAX_BYTES = 64 * 1024 * 1024


def require(test):
    if not test:
        raise ValueError("data_copy_refused")


def scalar(spec, value, nullable):
    if value is None:
        require(nullable)
        return {"type": "null"}
    kind = spec["type"]
    result = {"type": kind}
    if kind == "text":
        require(isinstance(value, str) and len(value) <= 2000)
        result["value"] = value
    elif kind == "enum":
        require(value in spec["values"])
        result["value"] = value
    elif kind == "boolean":
        require(type(value) is bool)
        result["value"] = value
    elif kind in ("integer", "decimal", "money"):
        require(type(value) is int)
        result[{"integer": "value", "decimal": "coefficient", "money": "minor_units"}[kind]] = str(value)
        for key in {"integer": ("unit",), "decimal": ("scale", "unit"), "money": ("currency", "minor_unit_exponent")}[kind]:
            result[key] = spec[key]
    elif kind == "date":
        require(type(value) is datetime.date)
        result["value"] = value.isoformat()
    elif kind == "timestamp":
        require(type(value) is datetime.datetime and value.microsecond == 0)
        result["value"] = value.isoformat(timespec="seconds") + "Z"
    else:
        raise ValueError("data_copy_refused")
    return result


def export(request):
    import duckdb
    require(set(request) == {"path", "digest", "fields", "rows"})
    path = Path(request["path"])
    require(path.is_absolute() and not path.is_symlink() and path.is_file())
    require(0 <= request["rows"] <= MAX_ROWS)
    require(hashlib.sha256(path.read_bytes()).hexdigest() == request["digest"].removeprefix("sha256:"))
    fields = request["fields"]
    require(0 < len(fields) <= 32)
    require(all(re.fullmatch(r"[a-z][a-z0-9_]{0,63}", field["id"]) for field in fields))
    connection = duckdb.connect(config={"threads": 1, "memory_limit": "512MB"})
    # Trusted path is bound as a value and field identifiers come from admission.
    cursor = connection.execute('SELECT ' + ','.join('"' + field['id'] + '"' for field in fields) + ' FROM read_parquet(?)', [str(path)])
    output, size = [], 0
    while True:
        chunk = cursor.fetchmany(1000)
        if not chunk:
            break
        for row in chunk:
            values = [{"field_id": field["id"], "value": scalar(field["value_type"], value, field.get("nullable", False))} for field, value in zip(fields, row)]
            size += len(json.dumps(values, ensure_ascii=False, separators=(",", ":")).encode())
            require(size <= MAX_BYTES and len(output) < MAX_ROWS)
            output.append(values)
    require(len(output) == request["rows"])
    return output


def main():
    try:
        request = json.loads(sys.stdin.buffer.read(262145))
        print(json.dumps({"rows": export(request)}, ensure_ascii=False, separators=(",", ":")))
        return 0
    except Exception:
        print('{"refused":"data_copy_refused"}')
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
