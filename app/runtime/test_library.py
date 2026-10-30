"""The prebuilt app library (ADR-011 pitch, APP-094): fixtures, data, engine oracles, catalog.

Every library definition validates under the contract (`validate_bundle`, the same rule the
operator seed applies), every CSV normalizes to the digest its fixture pins, and every runnable
view of every library app runs through the real `python3 -I runtime.py query` child over the
normalized Parquet with an empty environment, its rows compared cell for cell with an
independent Python evaluation of the same typed query over the typed CSV rows (the oracle).
The engine-side refusals the library convention relies on are checked too.

Needs `duckdb` 2.0.0 and `jsonschema`, as in the operator environment.
"""
import copy
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[1]
sys.path.insert(0, str(REPOSITORY / "contracts"))
import library_sources  # noqa: E402
import query_engine  # noqa: E402

ORG = "00000000-0000-4000-8000-000000000001"
OTHER = "00000000-0000-4000-8000-000000000002"
LIBRARY = REPOSITORY / "contracts" / "fixtures" / "library"
CATALOG = json.loads((LIBRARY / "catalog.json").read_text())
EPOCH = datetime.datetime(1970, 1, 1)


def encode(value):
    """`query_engine.encode_rows`'s encoding, applied to the oracle's typed values."""
    if isinstance(value, datetime.datetime):
        return value.strftime("%Y-%m-%dT%H:%M:%SZ")
    if isinstance(value, datetime.date):
        return value.isoformat()
    if isinstance(value, bool) or value is None:
        return value
    if isinstance(value, int):
        return str(value)
    return value


def literal(value):
    kind = value["type"]
    if kind in ("text", "enum"):
        return value["value"]
    if kind == "boolean":
        return value["value"]
    if kind == "integer":
        return int(value["value"])
    if kind == "money":
        return int(value["minor_units"])
    if kind == "date":
        return datetime.date.fromisoformat(value["value"])
    if kind == "timestamp":
        return datetime.datetime.strptime(value["value"], "%Y-%m-%dT%H:%M:%SZ")
    raise AssertionError(kind)


def evaluate(query, contract, typed_rows, window=None):
    """An independent evaluation of a qualified typed query: (columns, encoded rows, truncated).

    Rows are dicts in the normalized order (time, then original record). Aggregates follow SQL:
    count ignores NULL, sum/min/max ignore NULL and are NULL over no value, groups order by key.
    """
    names = [f["id"] for f in contract["fields"]]
    rows = [dict(zip(names, row)) for row in typed_rows]
    columns = list(names)
    if window:
        start, end = (datetime.datetime.strptime(window[k], "%Y-%m-%dT%H:%M:%SZ") for k in ("start", "end"))
        rows = [r for r in rows if start <= r["time"] < end]
    limit = None
    compare = {"eq": lambda a, b: a == b, "ne": lambda a, b: a != b, "lt": lambda a, b: a < b, "lte": lambda a, b: a <= b,
               "gt": lambda a, b: a > b, "gte": lambda a, b: a >= b}
    for op in query["operations"]:
        kind = op["type"]
        if kind == "filter":
            name, value = op["field"]["field_id"], literal(op["value"])
            rows = [r for r in rows if r[name] is not None and compare[op["comparison"]](r[name], value)]
        elif kind == "project":
            columns = [f["field_id"] for f in op["fields"]]
            rows = [{c: r[c] for c in columns} for r in rows]
        elif kind == "time_bucket":
            name, width = op["field"]["field_id"], op["seconds"]
            for r in rows:
                seconds = int((r[name] - EPOCH).total_seconds())
                r[name] = EPOCH + datetime.timedelta(seconds=seconds - seconds % width)
        elif kind == "aggregate":
            groups = [g["field_id"] for g in op["group_by"]]
            buckets = {}
            for r in rows:
                buckets.setdefault(tuple(r[g] for g in groups), []).append(r)
            out = []
            for key in sorted(buckets):
                record = dict(zip(groups, key))
                for m in op["measures"]:
                    values = [r[m["field"]["field_id"]] for r in buckets[key] if r[m["field"]["field_id"]] is not None]
                    if m["operation"] == "count":
                        record[m["id"]] = len(values)
                    elif not values:
                        record[m["id"]] = None
                    else:
                        record[m["id"]] = {"sum": sum, "min": min, "max": max}[m["operation"]](values)
                out.append(record)
            if not groups and not rows:
                out = [{m["id"]: (0 if m["operation"] == "count" else None) for m in op["measures"]}]
            rows, columns = out, groups + [m["id"] for m in op["measures"]]
        elif kind == "sort":
            keys = [(f["field"]["field_id"], f["direction"]) for f in op["fields"]] + [(op["stable_tiebreak"]["field_id"], "asc")]
            for name, direction in reversed(keys):
                rows.sort(key=lambda r: (r[name] is None, r[name]), reverse=(direction == "desc"))
        elif kind == "limit":
            limit = op["rows"]
    truncated = limit is not None and len(rows) > limit
    if limit is not None:
        rows = rows[:limit]
    return columns, [[encode(r[c]) for c in columns] for r in rows], truncated


class LibraryFixtures(unittest.TestCase):
    def test_every_library_fixture_validates_under_the_contract_and_the_catalog_names_it(self):
        from validate import digest, validate_bundle
        names = sorted(p.name for p in LIBRARY.glob("*.json") if p.name != "catalog.json")
        self.assertEqual(len(names), 20)
        self.assertEqual(names, sorted(a["fixture"] for a in CATALOG["apps"]))
        self.assertTrue(CATALOG["synthetic"])
        self.assertEqual((CATALOG["window_field"], CATALOG["destination_field"]), ("time", "destination"))
        seen_apps, seen_sources = set(), {}
        for entry in CATALOG["apps"]:
            bundle = json.loads((LIBRARY / entry["fixture"]).read_text())
            validate_bundle(bundle)
            definition = bundle["definition"]
            # Edition impl-2026-10-28.1 shapes: the edition the seed places and the store re-admits.
            self.assertEqual((definition["schema_version"], bundle["publication"]["schema_version"], bundle["receipt"]["schema_version"]), (1, 1, 1))
            self.assertEqual(definition["digest"], digest("app-definition", definition["content"]))
            self.assertEqual(entry["definition_digest"], definition["digest"])
            self.assertEqual(entry["fixture_sha256"], hashlib.sha256((LIBRARY / entry["fixture"]).read_bytes()).hexdigest())
            self.assertEqual((entry["app_id"], entry["work_id"], entry["publication_id"]),
                             (definition["app_id"], bundle["work"]["work_id"], bundle["publication"]["publication_id"]))
            self.assertNotIn(entry["app_id"], seen_apps)
            seen_apps.add(entry["app_id"])
            content = definition["content"]
            # One source input per app, one record input, a form, and only qualified operators over that one input.
            kinds = [i["kind"] for i in content["inputs"]]
            self.assertEqual(kinds, ["source", "records"])
            self.assertTrue(any(v["kind"] == "form" for v in content["views"]))
            for v in content["views"]:
                if "query" not in v:
                    continue
                self.assertEqual(v["query"]["inputs"], [content["inputs"][0]["id"]], v["id"])
                for op in v["query"]["operations"]:
                    self.assertIn(op["type"], ("filter", "project", "aggregate", "sort", "limit", "time_bucket"), v["id"])
                if any(op["type"] == "project" for op in v["query"]["operations"]) and v["kind"] in ("table", "detail"):
                    self.assertEqual(v["query"]["operations"][-1]["type"], "limit", "a row table pages (narrow_the_query otherwise)")
            for source in entry["sources"]:
                previous = seen_sources.setdefault(source["source_id"], source)
                self.assertEqual(previous, source, "a shared source has one revision")
            # The routing sidecar: what the composer's router reads.
            routing = entry["routing"]
            self.assertEqual(set(routing), {"description", "intent_examples", "negative_examples", "capabilities"})
            self.assertGreaterEqual(len(routing["intent_examples"]), 4)
            self.assertGreaterEqual(len(routing["negative_examples"]), 3)
            self.assertTrue(entry["purpose"] and entry["title"])
            views = {v["id"]: v for v in entry["views"]}
            for v in content["views"]:
                self.assertEqual(views[v["id"]]["runnable"], "query" in v)

    def test_the_generator_is_deterministic_and_the_tree_is_current(self):
        completed = subprocess.run([sys.executable, str(REPOSITORY / "contracts" / "make_library.py"), "--check"], capture_output=True, text=True)
        self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_csv_cells_outside_the_grammar_refuse(self):
        contract = {"fields": [{"id": "amount", "value_type": {"type": "money", "currency": "USD", "minor_unit_exponent": 2}, "nullable": False},
                               {"id": "when", "value_type": {"type": "timestamp"}, "nullable": True}]}
        self.assertEqual(library_sources.rows_of(contract, b"amount,when\n12.50,\n0.07,2026-10-01T10:00:00Z\n"),
                         [(1250, None), (7, datetime.datetime(2026, 10, 1, 10))])
        for bad in (b"amount,when\n12.5,\n", b"amount,when\n12.505,\n", b"amount,when\n1e3,\n", b"amount,when\n,\n",
                    b"amount,when\n1.00,2026-10-01 10:00:00\n", b"when,amount\n2026-10-01T10:00:00Z,1.00\n", b"amount,when\n1.00\n"):
            with self.assertRaises(library_sources.Refused, msg=bad):
                library_sources.rows_of(contract, bad)


class LibraryEngine(unittest.TestCase):
    """The real engine child over every library source, against the oracle."""

    @classmethod
    def setUpClass(cls):
        cls.directory = Path(tempfile.mkdtemp(prefix="library-"))
        cls.cache = cls.directory / "cache"
        cls.records = library_sources.build(ORG, cls.cache)
        (cls.cache / "o" / OTHER).mkdir(mode=0o700)
        cls.sources = {s["source_id"]: s for a in CATALOG["apps"] for s in a["sources"]}
        cls.rows = {}
        for source in cls.sources.values():
            contract = source["input_contract"]
            data = (LIBRARY / source["csv"]).read_bytes()
            typed = library_sources.rows_of(contract, data)
            if any(f["id"] == "time" and f["value_type"]["type"] == "timestamp" for f in contract["fields"]):
                index = [f["id"] for f in contract["fields"]].index("time")
                typed = [row for _, row in sorted(enumerate(typed, 1), key=lambda item: (item[1][index], item[0]))]
            cls.rows[source["source_id"]] = typed

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.directory)

    def request(self, source, query, organization=ORG, deadline_ms=1800, digest=None):
        scratch = Path(tempfile.mkdtemp(prefix="scratch-", dir=self.directory))
        self.addCleanup(shutil.rmtree, scratch, True)
        return {"organization_id": organization, "query": query, "deadline_ms": deadline_ms, "cache_root": str(self.cache), "scratch": str(scratch),
                "source": {"source_id": source["source_id"], "revision_id": source["revision_id"], "normalized_digest": digest or source["normalized_digest"],
                           "s3_key": f"o/{organization}/s/{source['source_id']}/r/{source['revision_id']}/part.parquet",
                           "input_id": source["input_id"], "fields": source["input_contract"]["fields"]}}

    def child(self, request):
        completed = subprocess.run([sys.executable, "-I", str(HERE / "runtime.py"), "query"], input=json.dumps(request).encode(),
                                   capture_output=True, env={}, timeout=10)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        lines = completed.stdout.decode().splitlines()
        self.assertEqual(len(lines), 1)
        return json.loads(lines[0])

    def test_normalized_revisions_match_the_catalog_pins_and_are_private(self):
        self.assertEqual(len(self.records), 19)
        for record in self.records:
            path = Path(record["path"])
            self.assertEqual(path.stat().st_mode & 0o777, 0o400)
            self.assertEqual("sha256:" + hashlib.sha256(path.read_bytes()).hexdigest(), self.sources[record["source_id"]]["normalized_digest"])
            self.assertEqual(record["row_count"], len(self.rows[record["source_id"]]))
        # A repeat build is a no-op over equal bytes, and a CSV whose bytes differ refuses before any write.
        again = library_sources.build(ORG, self.cache)
        self.assertEqual([r["normalized_digest"] for r in again], [r["normalized_digest"] for r in self.records])
        catalog = copy.deepcopy(CATALOG)
        catalog["apps"][0]["sources"][0]["content_digest"] = "sha256:" + "0" * 64
        with self.assertRaises(library_sources.Refused):
            library_sources.build(OTHER, self.directory / "other", catalog)

    def test_every_runnable_library_view_matches_the_oracle(self):
        runs = 0
        for app in CATALOG["apps"]:
            bundle = json.loads((LIBRARY / app["fixture"]).read_text())
            source = app["sources"][0]
            contract = source["input_contract"]
            for view in bundle["definition"]["content"]["views"]:
                if "query" not in view:
                    continue
                with self.subTest(app=app["library_id"], view=view["id"]):
                    result = self.child(self.request(source, view["query"]))
                    self.assertEqual(result["outcome"], "rows", result)
                    columns, rows, truncated = evaluate(view["query"], contract, self.rows[source["source_id"]])
                    self.assertEqual(result["columns"], columns)
                    self.assertEqual(result["rows"], rows)
                    self.assertEqual((result["truncated_page"], result["partial"], result["total_scope"]),
                                     (truncated, False, "full_authorized_filtered_input"))
                    self.assertLessEqual(len(result["rows"]), 500)
                    self.assertTrue(result["rows"], "every library view has at least one row on its data")
                    runs += 1
        self.assertGreaterEqual(runs, 50)

    def view(self, library_id, view_id):
        app = next(a for a in CATALOG["apps"] if a["library_id"] == library_id)
        bundle = json.loads((LIBRARY / app["fixture"]).read_text())
        return app["sources"][0], next(v for v in bundle["definition"]["content"]["views"] if v["id"] == view_id)["query"]

    def test_table_shapes_order_page_and_flag_truncation_exactly(self):
        # Expense queue: submitted claims, amount descending, expense_id tiebreak, first 50 of more.
        source, query = self.view("expense-review", "queue")
        result = self.child(self.request(source, query))
        self.assertEqual(result["columns"], ["expense_id", "time", "employee", "vendor", "category", "amount"])
        amounts = [int(r[5]) for r in result["rows"]]
        self.assertEqual(amounts, sorted(amounts, reverse=True))
        submitted = [r for r in self.rows[source["source_id"]] if r[6] == "submitted"]
        self.assertEqual((len(result["rows"]), result["truncated_page"]), (50, len(submitted) > 50))
        self.assertTrue(all(r[4] in ("travel", "software", "equipment", "meals", "services", "facilities") for r in result["rows"]))
        # A page that holds everything is not truncated: cash-flow's largest payments has exactly 10 rows.
        source, query = self.view("cash-flow", "largest_debits")
        result = self.child(self.request(source, query))
        self.assertEqual((len(result["rows"]), result["truncated_page"]), (10, True))
        source, query = self.view("budget-vs-actual", "lines")
        result = self.child(self.request(source, query))
        self.assertEqual((len(result["rows"]), result["truncated_page"]), (28, False))
        actuals = [int(r[3]) for r in result["rows"]]
        self.assertEqual(actuals, sorted(actuals, reverse=True))

    def test_grouped_breakdowns_and_aggregates_are_exact(self):
        source, query = self.view("vendor-spend", "by_vendor")
        result = self.child(self.request(source, query))
        self.assertEqual(result["columns"], ["vendor", "invoices", "spend"])
        expected = {}
        for row in self.rows[source["source_id"]]:
            count, spend = expected.get(row[2], (0, 0))
            expected[row[2]] = (count + 1, spend + row[4])
        self.assertEqual(result["rows"], [[vendor, str(count), str(spend)] for vendor, (count, spend) in sorted(expected.items())])
        self.assertEqual(sum(int(r[1]) for r in result["rows"]), 180)
        # count/sum/min/max over money and integers in one summary row; sum over BIGINT returns an exact integer string.
        source, query = self.view("incident-timeline", "totals")
        result = self.child(self.request(source, query))
        self.assertEqual(result["columns"], ["incidents", "resolved", "longest_minutes", "fastest_minutes"])
        rows = self.rows[source["source_id"]]
        minutes = [r[5] for r in rows if r[5] is not None]
        self.assertEqual(result["rows"], [[str(len(rows)), str(len(minutes)), str(max(minutes)), str(min(minutes))]])
        self.assertLess(len(minutes), len(rows), "open incidents have no resolution time: count skips NULL")
        # Nullable integer measures grouped: an all-NULL group yields an unknown cell, never zero.
        grouped = {"operator_version": 1, "inputs": ["incidents"], "null_policy": "unknown", "operations": [
            {"type": "aggregate", "group_by": [{"input_id": "incidents", "field_id": "status"}],
             "measures": [{"id": "longest", "operation": "max", "field": {"input_id": "incidents", "field_id": "minutes_to_resolve"}}]}]}
        result = self.child(self.request(source, grouped))
        self.assertEqual([r[0] for r in result["rows"]], ["mitigated", "open", "resolved"])
        self.assertIsNone(result["rows"][1][1])

    def test_filters_type_every_qualified_literal(self):
        source, query = self.view("vendor-spend", "unpaid")  # boolean literal
        result = self.child(self.request(source, query))
        unpaid = [r for r in self.rows[source["source_id"]] if r[6] is False]
        self.assertEqual(len(result["rows"]), min(50, len(unpaid)))
        dues = [r[3] for r in result["rows"]]
        self.assertEqual(dues, sorted(dues))
        source, query = self.view("inventory-comparison", "low_stock")  # integer literal with unit
        result = self.child(self.request(source, query))
        self.assertTrue(result["rows"] and all(int(r[3]) < 25 for r in result["rows"]))
        source, query = self.view("subscription-revenue", "active")  # enum filter then aggregate with a date min
        result = self.child(self.request(source, query))
        active = [r for r in self.rows[source["source_id"]] if r[5] == "active"]
        self.assertEqual(result["rows"], [[str(len(active)), str(sum(r[3] for r in active)), min(r[4] for r in active).isoformat()]])
        # Money, date and timestamp literals compare on their typed columns; a mistyped literal refuses.
        money = {"operator_version": 1, "inputs": ["vendor_invoices"], "null_policy": "unknown", "operations": [
            {"type": "filter", "field": {"input_id": "vendor_invoices", "field_id": "amount"}, "comparison": "gte",
             "value": {"type": "money", "minor_units": "500000", "currency": "USD", "minor_unit_exponent": 2}},
            {"type": "aggregate", "group_by": [], "measures": [{"id": "large", "operation": "count", "field": {"input_id": "vendor_invoices", "field_id": "invoice_id"}}]}]}
        source = next(a for a in CATALOG["apps"] if a["library_id"] == "vendor-spend")["sources"][0]
        result = self.child(self.request(source, money))
        self.assertEqual(result["rows"], [[str(sum(1 for r in self.rows[source["source_id"]] if r[4] >= 500000))]])
        date = copy.deepcopy(money)
        date["operations"][0] = {"type": "filter", "field": {"input_id": "vendor_invoices", "field_id": "due"}, "comparison": "lt", "value": {"type": "date", "value": "2026-10-15"}}
        result = self.child(self.request(source, date))
        self.assertEqual(result["rows"], [[str(sum(1 for r in self.rows[source["source_id"]] if r[5] < datetime.date(2026, 10, 15)))]])
        stamp = copy.deepcopy(money)
        stamp["operations"][0] = {"type": "filter", "field": {"input_id": "vendor_invoices", "field_id": "time"}, "comparison": "gte", "value": {"type": "timestamp", "value": "2026-10-20T00:00:00Z"}}
        result = self.child(self.request(source, stamp))
        self.assertEqual(result["rows"], [[str(sum(1 for r in self.rows[source["source_id"]] if r[1] >= datetime.datetime(2026, 10, 20)))]])
        for wrong in ({"type": "integer", "value": "500000", "unit": "cent"}, {"type": "text", "value": "5000.00"},
                      {"type": "money", "minor_units": "5000", "currency": "USD", "minor_unit_exponent": 3}):
            bad = copy.deepcopy(money)
            bad["operations"][0]["value"] = wrong
            result = self.child(self.request(source, bad))
            self.assertEqual((result["outcome"], result["reason"]), ("refused", "literal_not_qualified"), wrong)
        self.assertNotIn("rows", result)

    def test_integer_literals_are_ascii_digits_or_refused_with_a_code(self):
        """APP-093 review m4: an integer, money or decimal literal is `-?[0-9]+` (ASCII) or the fixed
        refusal `literal_not_qualified`, never `failed/engine` and never a Unicode digit read as a number."""
        money = {"operator_version": 1, "inputs": ["vendor_invoices"], "null_policy": "unknown", "operations": [
            {"type": "filter", "field": {"input_id": "vendor_invoices", "field_id": "amount"}, "comparison": "gte",
             "value": {"type": "money", "minor_units": "500000", "currency": "USD", "minor_unit_exponent": 2}},
            {"type": "aggregate", "group_by": [], "measures": [{"id": "large", "operation": "count", "field": {"input_id": "vendor_invoices", "field_id": "invoice_id"}}]}]}
        source = next(a for a in CATALOG["apps"] if a["library_id"] == "vendor-spend")["sources"][0]
        bad_texts = ("12.5", "", "-", "1e3", "\u0661\u0662\u0663", "\uff15", " 5", "5 ", "5\n", "+5", "--5", "0x10", "1_000")
        for text in bad_texts:
            bad = copy.deepcopy(money)
            bad["operations"][0]["value"]["minor_units"] = text
            result = self.child(self.request(source, bad))
            self.assertEqual((result["outcome"], result["reason"]), ("refused", "literal_not_qualified"), repr(text))
            self.assertNotIn("rows", result)
        signed = copy.deepcopy(money)
        signed["operations"][0]["value"]["minor_units"] = "-1"
        result = self.child(self.request(source, signed))
        self.assertEqual(result["rows"], [[str(len(self.rows[source["source_id"]]))]])
        # The integer (with unit) and decimal (with scale) literals share the grammar.
        cases = ((({"type": "integer", "unit": "each"}), "value"), (({"type": "decimal", "unit": "percent", "scale": 2}), "coefficient"))
        for value_type, key in cases:
            for text in bad_texts:
                with self.assertRaises(query_engine.Refused) as refused:
                    query_engine.literal_parameter(dict(value_type, **{key: text}), value_type)
                self.assertEqual(query_engine.refusal_code(str(refused.exception)), "literal_not_qualified", repr(text))
            self.assertEqual(query_engine.literal_parameter(dict(value_type, **{key: "-42"}), value_type), (-42, ""))

    def test_timelines_bucket_in_place_and_the_window_is_a_parameter(self):
        source, query = self.view("support-queue", "timeline")
        result = self.child(self.request(source, query))
        self.assertEqual(result["columns"], ["time", "tickets"])
        self.assertTrue(all(t.endswith("T00:00:00Z") for t, _ in result["rows"]))
        self.assertEqual(sum(int(n) for _, n in result["rows"]), 260)
        windowed = dict(query, window={"start": "2026-10-10T00:00:00Z", "end": "2026-10-13T00:00:00Z", "bounds": "half_open"})
        result = self.child(self.request(source, windowed))
        days = [t for t, _ in result["rows"]]
        self.assertTrue(set(days) <= {"2026-10-10T00:00:00Z", "2026-10-11T00:00:00Z", "2026-10-12T00:00:00Z"} and days)
        expected = sum(1 for r in self.rows[source["source_id"]] if datetime.datetime(2026, 10, 10) <= r[1] < datetime.datetime(2026, 10, 13))
        self.assertEqual(sum(int(n) for _, n in result["rows"]), expected)
        # An input with no `time` timestamp field cannot take a window: refused before DuckDB.
        source, query = self.view("inventory-comparison", "totals")
        result = self.child(self.request(source, dict(query, window={"start": "2026-10-10T00:00:00Z", "end": "2026-10-13T00:00:00Z", "bounds": "half_open"})))
        self.assertEqual((result["outcome"], result["reason"]), ("refused", "window_refused"))
        # The destination parameter shape: a text equality filter on the traffic input's destination.
        source, query = self.view("outbound-traffic", "total")
        filtered = copy.deepcopy(query)
        filtered["operations"].insert(0, {"type": "filter", "field": {"input_id": "traffic", "field_id": "destination"}, "comparison": "eq",
                                          "value": {"type": "text", "value": "198.51.100.7"}})
        result = self.child(self.request(source, filtered))
        rows = [r for r in self.rows[source["source_id"]] if r[2] == "198.51.100.7"]
        self.assertEqual(result["rows"], [[str(len(rows)), str(sum(r[3] for r in rows)), str(max(r[3] for r in rows))]])

    def test_the_lockdown_is_unchanged_for_library_sources(self):
        source, query = self.view("audit-trail", "recent")
        foreign = self.child(self.request(source, query, organization=OTHER))
        self.assertEqual((foreign["outcome"], foreign["reason"]), ("refused", "source_unavailable"))
        wrong = self.child(self.request(source, query, digest="sha256:" + "0" * 64))
        self.assertEqual(wrong["reason"], "source_digest_mismatch")
        other_input = dict(query, inputs=["tickets"])
        self.assertEqual(self.child(self.request(source, other_input))["outcome"], "refused")
        # An unpaged row list is served whole up to the 500-row page bound (320 audit rows) and refused past it.
        unpaged = {"operator_version": 1, "inputs": ["audit_log"], "null_policy": "unknown",
                   "operations": [{"type": "project", "fields": [{"input_id": "audit_log", "field_id": "actor"}]}]}
        served = self.child(self.request(source, unpaged))
        self.assertEqual((len(served["rows"]), served["truncated_page"]), (320, False))
        # A sort or group on a field the input does not declare is refused, as is a sum over text.
        bad_sort = {"operator_version": 1, "inputs": ["audit_log"], "null_policy": "unknown", "operations": [
            {"type": "sort", "fields": [{"field": {"input_id": "audit_log", "field_id": "amount"}, "direction": "asc"}], "stable_tiebreak": {"input_id": "audit_log", "field_id": "actor"}},
            {"type": "limit", "rows": 5}]}
        self.assertEqual(self.child(self.request(source, bad_sort))["outcome"], "refused")
        text_sum = {"operator_version": 1, "inputs": ["audit_log"], "null_policy": "unknown", "operations": [
            {"type": "aggregate", "group_by": [], "measures": [{"id": "s", "operation": "sum", "field": {"input_id": "audit_log", "field_id": "actor"}}]}]}
        self.assertEqual(self.child(self.request(source, text_sum))["reason"], "query_shape_refused")
        self.assertEqual(self.child(self.request(source, query, deadline_ms=1))["outcome"], "failed")

    def test_plan_lists_the_b1_object_keys_without_touching_storage(self):
        completed = subprocess.run([sys.executable, str(HERE / "library_sources.py"), "plan", "--organization", ORG], capture_output=True, text=True,
                                   env={k: v for k, v in os.environ.items() if not k.startswith("AWS")})
        self.assertEqual(completed.returncode, 0, completed.stderr)
        plan = json.loads(completed.stdout)
        self.assertEqual(len(plan["sources"]), 19)
        for source in plan["sources"]:
            prefix = f"o/{ORG}/s/{source['source_id']}/r/{source['revision_id']}/"
            self.assertEqual(source["objects"], {"original": prefix + "original", "part.parquet": prefix + "part.parquet"})
        refused = subprocess.run([sys.executable, str(HERE / "library_sources.py"), "plan", "--organization", "northstar"], capture_output=True, text=True)
        self.assertEqual((refused.returncode, json.loads(refused.stderr)), (1, {"refused": "organization_refused"}))


if __name__ == "__main__":
    unittest.main()


class BoxNormalizer(unittest.TestCase):
    """APP-108: the folder normalizer child writes exactly the library normalizer's bytes."""

    def test_box_normalize_matches_library_sources_for_every_library_csv(self):
        sources = {s["source_id"]: s for a in CATALOG["apps"] for s in a["sources"]}
        directory = Path(tempfile.mkdtemp(prefix="box-normalize-"))
        self.addCleanup(shutil.rmtree, directory, True)
        for source in sources.values():
            data = (LIBRARY / source["csv"]).read_bytes()
            target = directory / (source["source_id"] + ".parquet")
            request = {"contract": source["input_contract"], "csv": data.decode("utf-8"), "target": str(target)}
            completed = subprocess.run([sys.executable, "-I", str(HERE / "runtime.py"), "normalize"], input=json.dumps(request).encode(),
                                       capture_output=True, env={}, timeout=60)
            self.assertEqual(completed.returncode, 0, completed.stdout)
            answer = json.loads(completed.stdout)
            self.assertEqual(answer["normalized_digest"], source["normalized_digest"], source["csv"])
            self.assertEqual(answer["row_count"], source["row_count"])
            self.assertEqual("sha256:" + hashlib.sha256(target.read_bytes()).hexdigest(), source["normalized_digest"])

    def test_box_normalize_refuses_cells_outside_the_contract(self):
        directory = Path(tempfile.mkdtemp(prefix="box-normalize-"))
        self.addCleanup(shutil.rmtree, directory, True)
        contract = {"id": "q3", "contract_id": "q3_x", "contract_version": 1, "kind": "source",
                    "fields": [{"id": "revenue", "value_type": {"type": "money", "currency": "USD", "minor_unit_exponent": 2}, "nullable": False}]}
        for csv_text, code in (("revenue\n12.5\n", "money_refused"), ("other\n1.00\n", "header_refused"), ("revenue\n\n", "row_shape_refused")):
            request = {"contract": contract, "csv": csv_text, "target": str(directory / "x.parquet")}
            completed = subprocess.run([sys.executable, "-I", str(HERE / "runtime.py"), "normalize"], input=json.dumps(request).encode(),
                                       capture_output=True, env={}, timeout=60)
            self.assertEqual(completed.returncode, 1)
            self.assertEqual(json.loads(completed.stdout), {"refused": code})
            self.assertFalse((directory / "x.parquet").exists())
