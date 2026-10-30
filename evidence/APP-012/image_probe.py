"""APP-012 in-image probe: run inside the service image with --network=none.

Mounted read-only at /probe with the cached Northstar revision at
/probe/cache/o/<org>/<source>.<revision>.parquet. It checks the image's
`python3 -I /app/runtime.py query` child with an empty environment against
the oracle and measures its wall time per request. Prints one JSON report.
Not part of the image; evidence tooling only.
"""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

ORG = "00000000-0000-4000-8000-000000000001"
SOURCE = "00000000-0000-4000-8000-00000000006e"
REVISION = "9161c1e4-27c0-4cbb-addf-eb7ce504bf45"
CACHE = Path(sys.argv[1])
VIEWS = json.loads(Path(sys.argv[2]).read_text())
WINDOW = {"start": "2026-10-22T10:00:00Z", "end": "2026-10-22T10:10:00Z", "bounds": "half_open"}
path = CACHE / "o" / ORG / f"{SOURCE}.{REVISION}.parquet"
digest = "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def child(query):
    scratch = Path(tempfile.mkdtemp(prefix="q-", dir="/run/genui"))
    request = {"organization_id": ORG, "query": query, "deadline_ms": 1800, "cache_root": str(CACHE), "scratch": str(scratch),
               "source": {"source_id": SOURCE, "revision_id": REVISION, "normalized_digest": digest,
                          "s3_key": f"o/{ORG}/s/{SOURCE}/r/{REVISION}/part.parquet"}}
    started = time.monotonic()
    done = subprocess.run(["/usr/local/bin/python3", "-I", "/app/runtime.py", "query"], input=json.dumps(request).encode(),
                          capture_output=True, env={}, timeout=10)
    wall = time.monotonic() - started
    shutil.rmtree(scratch)
    return json.loads(done.stdout), wall


def windowed(view, destination=None):
    query = dict(VIEWS[view], window=WINDOW)
    if destination:
        query["operations"] = [{"type": "filter", "field": {"input_id": "events", "field_id": "destination"}, "comparison": "eq",
                                "value": {"type": "text", "value": destination}}] + query["operations"]
    return query


total, _ = child(windowed("total"))
filtered, _ = child(windowed("total", "198.51.100.7"))
timeline, _ = child(windowed("timeline"))
walls = sorted(child(windowed("total"))[1] for _ in range(40))
import duckdb  # noqa: E402  (the image's pinned engine)
report = {"digest": digest, "duckdb": duckdb.__version__, "total": total["rows"], "destination_198_51_100_7": filtered["rows"],
          "timeline": timeline["rows"], "timeline_columns": timeline["columns"],
          "child_wall_ms": {"samples": len(walls), "p50": round(walls[19] * 1000, 1), "p95": round(walls[37] * 1000, 1),
                            "max": round(walls[-1] * 1000, 1)}}
print(json.dumps(report, sort_keys=True))
