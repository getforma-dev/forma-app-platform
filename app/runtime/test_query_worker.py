"""APP-012 engine child checks (ADR-007 D7.10): the real `runtime.py query` child
over the operator-shaped Northstar revision, against the APP-011 oracles.

Needs `duckdb` 2.0.0 (and, for the cache fetch, `botocore`), as in the image;
the locked operator environment provides both. No AWS call: the fetch uses
botocore's Stubber.
"""
import copy
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[1]
sys.path.insert(0, str(REPOSITORY / "fixtures"))
import verify  # noqa: E402
import query_engine  # noqa: E402
import runtime  # noqa: E402

ORG = "00000000-0000-4000-8000-000000000001"
OTHER = "00000000-0000-4000-8000-000000000002"
SOURCE = "00000000-0000-4000-8000-00000000006e"
REVISION = "00000000-0000-4000-8000-00000000006f"
# evidence/APP-077: the uploaded part.parquet of the Northstar events revision.
UPLOADED_NORMALIZED = "sha256:0c2582d801005bd49ed4173b86bf8d03b9e681cbcfcca8a8387b5eb32237a082"
WINDOW = {"start": "2026-10-22T10:00:00Z", "end": "2026-10-22T10:10:00Z", "bounds": "half_open"}
EXPECTED = json.loads((REPOSITORY / "fixtures/phase-one/v1/expected.json").read_text())["northstar"]


def seeded_views(edition="impl-2026-10-28.1", name="northstar"):
    path = REPOSITORY / "contracts/fixtures" / (edition + "/" if edition else "") / (name + ".json")
    return {v["id"]: v for v in json.loads(path.read_text())["definition"]["content"]["views"]}


def northstar_parquet(target):
    """The operator's `normalize` over the independent CSV reader (APP-077 adapter)."""
    import duckdb
    root = REPOSITORY / "fixtures/phase-one/v1"
    manifest = json.loads((root / "manifest.json").read_text())
    item = [s for s in manifest["sources"] if s["id"] == "northstar-events-csv"][0]
    rows = [r for r in verify.table(root, item) if r["asset"] == "build-worker-17"]
    typed = sorted(((verify.timestamp(r["time"]).replace(tzinfo=None), r["destination"], verify.integer(r["bytes"]), i)
                    for i, r in enumerate(rows, 1)), key=lambda x: (x[0], x[3]))
    connection = duckdb.connect()
    try:
        connection.execute("SET threads=1")
        connection.execute("CREATE TABLE events(time TIMESTAMP NOT NULL, destination VARCHAR NOT NULL, bytes BIGINT NOT NULL, original INTEGER NOT NULL)")
        connection.executemany("INSERT INTO events VALUES (?, ?, ?, ?)", typed)
        connection.execute("COPY (SELECT time, destination, bytes FROM events ORDER BY time, original) TO ? "
                           "(FORMAT PARQUET, COMPRESSION ZSTD, ROW_GROUP_SIZE 100000)", [str(target)])
    finally:
        connection.close()
    return "sha256:" + hashlib.sha256(Path(target).read_bytes()).hexdigest()


def windowed(query, destination=None):
    query = copy.deepcopy(query)
    query["window"] = dict(WINDOW)
    if destination is not None:
        query["operations"].insert(0, {"type": "filter", "field": {"input_id": "events", "field_id": "destination"},
                                       "comparison": "eq", "value": {"type": "text", "value": destination}})
    return query


class EngineChild(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.directory = Path(tempfile.mkdtemp(prefix="app012-"))
        cls.cache = cls.directory / "cache"
        (cls.cache / "o" / ORG).mkdir(parents=True)
        (cls.cache / "o" / OTHER).mkdir(parents=True)
        cls.path = cls.cache / "o" / ORG / f"{SOURCE}.{REVISION}.parquet"
        cls.digest = northstar_parquet(cls.path)
        cls.views = seeded_views()

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.directory)

    def request(self, query, organization=ORG, digest=None, deadline_ms=1800, source=SOURCE, revision=REVISION):
        scratch = Path(tempfile.mkdtemp(prefix="scratch-", dir=self.directory))
        self.addCleanup(shutil.rmtree, scratch, True)
        return {"organization_id": organization, "query": query, "deadline_ms": deadline_ms,
                "cache_root": str(self.cache), "scratch": str(scratch),
                "source": {"source_id": source, "revision_id": revision, "normalized_digest": digest or self.digest,
                           "s3_key": f"o/{organization}/s/{source}/r/{revision}/part.parquet"}}

    def child(self, request):
        """The image's invocation: `python3 -I runtime.py query`, empty environment."""
        completed = subprocess.run([sys.executable, "-I", str(HERE / "runtime.py"), "query"], input=json.dumps(request).encode(),
                                   capture_output=True, env={}, timeout=10)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        lines = completed.stdout.decode().splitlines()
        self.assertEqual(len(lines), 1, "exactly one result line")
        return json.loads(lines[0])

    def test_normalized_revision_is_the_uploaded_object(self):
        self.assertEqual(self.digest, UPLOADED_NORMALIZED)

    def test_query_worker_matches_app011_oracles(self):
        total = self.child(self.request(windowed(self.views["total"]["query"])))
        self.assertEqual(total["outcome"], "rows")
        self.assertEqual(total["rows"], [[EXPECTED["total_bytes"]]])
        self.assertEqual(total["rows"], [["1140000"]])
        self.assertEqual((total["partial"], total["truncated_page"], total["total_scope"]), (False, False, "full_authorized_filtered_input"))
        filtered = self.child(self.request(windowed(self.views["total"]["query"], "198.51.100.7")))
        self.assertEqual(filtered["rows"], [[EXPECTED["destinations"]["198.51.100.7"]]])
        self.assertEqual(filtered["rows"], [["900000"]])
        timeline = self.child(self.request(windowed(self.views["timeline"]["query"])))
        self.assertEqual(timeline["columns"], ["minute", "destination", "bytes"])
        minutes = {}
        for minute, _, size in timeline["rows"]:
            minutes[minute] = minutes.get(minute, 0) + int(size)
        expected = {f"2026-10-22T10:{i:02d}:00Z": int(v) for i, v in enumerate(EXPECTED["minute_bytes"]) if v != "0"}
        self.assertEqual(minutes, expected)
        self.assertEqual(sum(minutes.values()), 1140000)

    def test_the_window_parameter_is_what_excludes_e6(self):
        # Without the S02 window the seeded `total` view sums all six events.
        whole = self.child(self.request(self.views["total"]["query"]))
        self.assertEqual(whole["rows"], [["2040000"]])

    def test_impossible_window_bounds_are_refused_before_duckdb(self):
        # APP-012 review M2 (probe P6): the Rust parent refuses these first; the child
        # re-checks both bounds with strptime so neither 2026-02-31 nor year 0000 runs.
        for start, end in [("2026-02-31T00:00:00Z", "2026-03-01T00:00:00Z"), ("0000-01-01T00:00:00Z", "0000-01-01T10:10:00Z"),
                           ("2026-04-31T00:00:00Z", "2026-05-01T00:00:00Z"), ("2026-10-22T10:10:00Z", "2026-10-22T10:00:00Z"),
                           ("2026-10-22T10:00:00+00:00", "2026-10-22T10:10:00Z")]:
            query = windowed(self.views["total"]["query"])
            query["window"].update({"start": start, "end": end})
            result = self.child(self.request(query))
            self.assertEqual((result["outcome"], result["reason"]), ("refused", "window_refused"), (start, end))
            self.assertNotIn("rows", result)
        leap = windowed(self.views["total"]["query"])
        leap["window"].update({"start": "2028-02-29T00:00:00Z", "end": "2028-03-01T00:00:00Z"})
        self.assertEqual(self.child(self.request(leap))["outcome"], "rows")

    def test_measure_ids_are_local_identifiers(self):
        # APP-012 review m5 (probe P7): a measure id becomes an SQL alias, so only
        # [a-z][a-z0-9_]{0,63} is interpolated; anything else is refused before DuckDB.
        for bad in ['x" FROM (SELECT 1 AS x, 2 AS y) --', "Total", "a" * 65, 7]:
            query = windowed(self.views["total"]["query"])
            query["operations"][-1]["measures"][0]["id"] = bad
            result = self.child(self.request(query))
            self.assertEqual((result["outcome"], result["reason"]), ("refused", "query_shape_refused"), bad)
            self.assertNotIn("rows", result)

    def test_ratio_lookup_and_money_views_stay_refused(self):
        current = seeded_views(edition="", name="northstar")
        rate = self.child(self.request(current["rate"]["query"]))
        self.assertEqual((rate["outcome"], rate["reason"]), ("refused", "operator_not_qualified"))
        lookup = self.child(self.request(self.views["destinations"]["query"]))
        self.assertEqual(lookup["outcome"], "refused")
        queue = seeded_views(name="spend-review")["queue"]["query"]
        self.assertEqual(self.child(self.request(queue))["outcome"], "refused")
        money = windowed(self.views["total"]["query"])
        money["operations"].insert(0, {"type": "filter", "field": {"input_id": "events", "field_id": "bytes"}, "comparison": "gte",
                                       "value": {"type": "money", "minor_units": "20000", "currency": "USD", "minor_unit_exponent": 2}})
        refused = self.child(self.request(money))
        self.assertEqual((refused["outcome"], refused["reason"]), ("refused", "literal_not_qualified"))
        self.assertNotIn("rows", refused)

    def test_worker_cannot_read_outside_organization_cache(self):
        # The other organization has no copy; this organization's file is not reachable from it.
        foreign = self.child(self.request(windowed(self.views["total"]["query"]), organization=OTHER))
        self.assertEqual((foreign["outcome"], foreign["reason"]), ("refused", "source_unavailable"))
        link = self.cache / "o" / OTHER / f"{SOURCE}.{REVISION}.parquet"
        link.symlink_to(self.path)
        self.addCleanup(link.unlink)
        linked = self.child(self.request(windowed(self.views["total"]["query"]), organization=OTHER))
        self.assertEqual(linked["reason"], "source_unavailable")
        wrong = self.child(self.request(windowed(self.views["total"]["query"]), digest="sha256:" + "0" * 64))
        self.assertEqual(wrong["reason"], "source_digest_mismatch")

    def test_engine_has_no_external_access_and_locked_configuration(self):
        scratch = Path(tempfile.mkdtemp(prefix="direct-", dir=self.directory))
        self.addCleanup(shutil.rmtree, scratch)
        connection = query_engine.connect(scratch, [self.path])
        try:
            settings = dict(connection.execute("SELECT name, value FROM duckdb_settings() WHERE name IN "
                                               "('enable_external_access','lock_configuration','autoload_known_extensions','threads','memory_limit')").fetchall())
            self.assertEqual(settings["enable_external_access"], "false")
            self.assertEqual(settings["lock_configuration"], "true")
            self.assertEqual(settings["autoload_known_extensions"], "false")
            self.assertEqual(settings["threads"], "2")
            with self.assertRaises(Exception):
                connection.execute("SET enable_external_access=true")
            secret = self.directory / "outside.csv"
            secret.write_text("a\n1\n")
            with self.assertRaises(Exception):
                connection.execute("SELECT * FROM read_csv(?)", [str(secret)]).fetchall()
            with self.assertRaises(Exception):
                connection.execute("INSTALL httpfs")
            self.assertEqual(connection.execute("SELECT sum(bytes) FROM read_parquet(?)", [str(self.path)]).fetchone()[0], 2040000)
        finally:
            connection.close()

    def test_deadline_reports_failed_never_zero(self):
        result = self.child(self.request(windowed(self.views["total"]["query"]), deadline_ms=1))
        self.assertEqual((result["outcome"], result["reason"]), ("failed", "deadline"))
        self.assertNotIn("rows", result)

    def test_request_shape_is_closed(self):
        request = self.request(windowed(self.views["total"]["query"]))
        request["sql"] = "SELECT 1"
        self.assertEqual(self.child(request)["reason"], "request_refused")
        request = self.request(windowed(self.views["total"]["query"]))
        request["source"]["s3_key"] = f"o/{OTHER}/s/{SOURCE}/r/{REVISION}/part.parquet"
        self.assertEqual(self.child(request)["reason"], "request_refused")

    def test_per_request_child_latency(self):
        # Recorded in evidence/APP-012 from the Rust measurement; this keeps a local bound visible.
        started = time.monotonic()
        for _ in range(3):
            self.assertEqual(self.child(self.request(windowed(self.views["total"]["query"])))["outcome"], "rows")
        self.assertLess((time.monotonic() - started) / 3, 1.8)


class RevisionCache(unittest.TestCase):
    def test_fetch_verifies_digest_and_renames_read_only(self):
        from botocore.response import StreamingBody
        from botocore.stub import Stubber
        import boto3
        data = b"PAR1-synthetic"
        directory = Path(tempfile.mkdtemp(prefix="cache-"))
        self.addCleanup(shutil.rmtree, directory)
        os.chmod(directory, 0o700)
        client = boto3.client("s3", region_name="us-east-1", aws_access_key_id="unit", aws_secret_access_key="unit")
        request = {"organization_id": ORG, "source_id": SOURCE, "revision_id": REVISION,
                   "normalized_digest": "sha256:" + hashlib.sha256(data).hexdigest(), "cache_root": str(directory)}
        key = f"o/{ORG}/s/{SOURCE}/r/{REVISION}/part.parquet"
        with Stubber(client) as stub:
            stub.add_response("get_object", {"Body": StreamingBody(io.BytesIO(data), len(data)), "ContentLength": len(data)},
                              {"Bucket": "bucket", "Key": key})
            self.assertEqual(runtime.fetch_revision(client, "bucket", request), {"cached": True, "bytes": len(data)})
        target = directory / "o" / ORG / f"{SOURCE}.{REVISION}.parquet"
        self.assertEqual(target.read_bytes(), data)
        self.assertEqual(target.stat().st_mode & 0o777, 0o400)
        wrong = dict(request, revision_id="00000000-0000-4000-8000-000000000070", normalized_digest="sha256:" + "1" * 64)
        with Stubber(client) as stub:
            stub.add_response("get_object", {"Body": StreamingBody(io.BytesIO(data), len(data)), "ContentLength": len(data)},
                              {"Bucket": "bucket", "Key": runtime.revision_key(wrong)})
            with self.assertRaises(runtime.Refused):
                runtime.fetch_revision(client, "bucket", wrong)
        self.assertEqual(sorted(p.name for p in (directory / "o" / ORG).iterdir()), [target.name])

    def test_cache_request_is_closed(self):
        with self.assertRaises(runtime.Refused):
            runtime.cache_request(json.dumps({"organization_id": ORG}).encode())
        with self.assertRaises(runtime.Refused):
            runtime.cache_request(json.dumps({"organization_id": "../x", "source_id": SOURCE, "revision_id": REVISION,
                                              "normalized_digest": "sha256:" + "0" * 64, "cache_root": "/c"}).encode())


if __name__ == "__main__":
    unittest.main()
