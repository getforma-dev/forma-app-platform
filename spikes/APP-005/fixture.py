"""Trusted deterministic synthetic Parquet preparation, outside the query worker."""
import hashlib
import ipaddress
import json
from pathlib import Path

import duckdb

S02 = [
    (1, "2026-10-22 10:00:00", "203.0.113.10", 60000),
    (2, "2026-10-22 10:01:00", "203.0.113.20", 120000),
    (3, "2026-10-22 10:02:00", "198.51.100.7", 300000),
    (4, "2026-10-22 10:03:00", "198.51.100.7", 600000),
    (5, "2026-10-22 10:05:00", "203.0.113.10", 60000),
    (6, "2026-10-22 10:10:00", "198.51.100.99", 900000),
]


def file_digest(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        while block := stream.read(1024 * 1024):
            h.update(block)
    return "sha256:" + h.hexdigest()


def prepare(root):
    root = Path(root)
    root.mkdir(parents=True, exist_ok=True)
    connection = duckdb.connect(str(root / "empty.duckdb"))
    connection.execute("SET threads=2")
    connection.execute("CREATE TABLE IF NOT EXISTS fixture_identity (edition VARCHAR)")
    connection.close()
    connection = duckdb.connect()
    connection.execute("SET threads=2")
    connection.execute("CREATE TABLE events(event_id BIGINT, time TIMESTAMP, destination VARCHAR, bytes BIGINT)")
    connection.executemany("INSERT INTO events VALUES (?, ?, ?, ?)", S02)
    connection.execute("COPY events TO ? (FORMAT PARQUET, COMPRESSION ZSTD, ROW_GROUP_SIZE 100000)", [str(root / "s02.parquet")])
    connection.execute("""COPY (
      SELECT i::BIGINT AS event_id,
        TIMESTAMP '2026-10-22 10:00:00' + (i % 600) * INTERVAL 1 SECOND AS time,
        CASE i % 4 WHEN 0 THEN '203.0.113.10' WHEN 1 THEN '203.0.113.20'
          WHEN 2 THEN '198.51.100.7' ELSE '198.51.100.8' END AS destination,
        (i % 1000 + 1)::BIGINT AS bytes
      FROM range(1000000) t(i) ORDER BY i
    ) TO ? (FORMAT PARQUET, COMPRESSION ZSTD, ROW_GROUP_SIZE 100000)""", [str(root / "million.parquet")])
    groups = connection.execute("SELECT row_group_id, row_group_num_rows FROM parquet_metadata(?) WHERE column_id=0", [str(root / "million.parquet")]).fetchall()
    connection.close()
    # These expectations use independent Python integer loops, never DuckDB outputs.
    totals = {k: 0 for k in ("203.0.113.10", "203.0.113.20", "198.51.100.7", "198.51.100.8")}
    destinations = list(totals)
    minutes = [0] * 10
    for i in range(1000000):
        amount = i % 1000 + 1
        totals[destinations[i % 4]] += amount
        minutes[(i % 600) // 60] += amount
    manifest = {"fixture_version": 1, "duckdb_generator": duckdb.__version__, "source_semantics": "complete synthetic event bytes, UTC, half-open ten-minute window",
                "files": {name: {"digest": file_digest(root / name), "bytes": (root / name).stat().st_size} for name in ("empty.duckdb", "s02.parquet", "million.parquet")},
                "million": {"rows": 1000000, "total": sum(totals.values()), "minimum": 1, "maximum": 1000, "destinations": totals, "minute_totals": minutes, "row_groups": groups},
                "s02": {"included_rows": 5, "total": 1140000, "filtered_destination_total": 900000, "whole_window_bytes_per_minute": 114000, "timeline": [60000, 120000, 300000, 600000, 0, 60000, 0, 0, 0, 0]},
                "inventory_scope": "Exact/CIDR matching is a future registered operator, not implemented or timed by this spike."}
    (root / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (root / "escape-link.parquet").symlink_to("../canary-secret.parquet")
    return manifest


if __name__ == "__main__":
    import sys
    prepare(sys.argv[1])
