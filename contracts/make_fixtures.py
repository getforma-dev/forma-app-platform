"""Regenerate clearly synthetic contract examples; no parser/query execution is claimed.

Edition impl-2026-10-29.3 specimens: the edition-two bundles and vectors unchanged, plus the APP-013
recovery view, claim and command problem bodies (fixtures/command-recovery.json). Edition
impl-2026-10-28.1's and impl-2026-10-29.2's specimens and vectors are retained byte-identically in
fixtures/impl-2026-10-28.1/ and fixtures/impl-2026-10-29.2/ and are never regenerated.
"""
import copy
import hashlib
import json
from pathlib import Path

from validate import canonical, digest, exact_rational, payload_digest, rows_digest

ROOT = Path(__file__).resolve().parent
RETAINED = ROOT / "fixtures" / "impl-2026-10-28.1"


def uid(n):
    return f"00000000-0000-4000-8000-{n:012x}"

def raw_hash(label):
    return "sha256:" + hashlib.sha256(label.encode()).hexdigest()

def field(name, kind, **extras):
    return {"id": name, "value_type": {"type": kind, **extras}, "nullable": False}

def f(input_id, field_id):
    return {"input_id": input_id, "field_id": field_id}

def query(inputs, operations, window=None):
    return {"operator_version": 1, "inputs": inputs, "operations": operations, "null_policy": "unknown", **({"window": window} if window else {})}

def operator_identities(queries):
    return [{"id": name, "version": 1} for name in sorted({op["type"] for q in queries for op in q["operations"]})]

# Typed normalized rows of the Northstar events source, sorted by time then original coordinate (D9.3):
# five events, 1,140,000 bytes in the half-open ten-minute window, 114,000 bytes per full-window minute.
EVENT_ROWS = [{"time": {"type": "timestamp", "value": time}, "destination": {"type": "text", "value": destination},
               "bytes": {"type": "integer", "value": amount, "unit": "byte"}}
              for time, destination, amount in (("2026-10-22T10:00:00Z", "10.0.0.5", "60000"), ("2026-10-22T10:01:00Z", "10.0.1.7", "120000"),
                                                ("2026-10-22T10:02:30Z", "203.0.113.9", "300000"), ("2026-10-22T10:03:00Z", "198.51.100.4", "600000"),
                                                ("2026-10-22T10:09:59Z", "10.0.0.5", "60000"))]

def synthetic_rows(contract):
    """Deterministic placeholder rows for every other synthetic source; their values claim nothing."""
    if contract["id"] == "events":
        return EVENT_ROWS
    def value(item, n):
        kind = item["value_type"]["type"]
        if kind == "money":
            return {"type": "money", "minor_units": str(10000 * n), "currency": "USD", "minor_unit_exponent": 2}
        return {"type": "text", "value": f"{contract['id']}-{n}"}
    return [{item["id"]: value(item, n) for item in contract["fields"]} for n in range(1, 6)]

def bundle(name, offset, input_specs, views, command_values, coverage="complete"):
    org, app, principal, revision, publication, work, collection = map(uid, range(offset, offset + 7))
    decisions = [field("status", "enum", values=["needs_context", "reviewed"]), field("note", "text")]
    records = {"id": "decision", "version": 1, "fields": decisions}
    first = input_specs[0][0]
    inputs = [{"id": k, "contract_id": k + "_v1", "contract_version": 1, "kind": "source", "fields": v, "coverage": coverage if k == first else "complete"} for k, v in input_specs]
    inputs.append({"id": "decisions", "contract_id": "decisions_v1", "contract_version": 1, "kind": "records", "fields": decisions, "coverage": "complete"})
    content = {"title": name, "description": "Synthetic contract example; no runtime or query result is claimed.", "inputs": inputs, "views": views,
               "record_schemas": [records], "commands": [{"id": "record_decision", "record_schema_id": "decision", "operation": "create", "writable_fields": ["status", "note"], "transitions": [{"field_id": "status", "from": "needs_context", "to": "reviewed"}]}],
               "links": [], "default_layout": [{"view_id": v["id"], "column": 0, "row": i, "width": 12} for i, v in enumerate(views)]}
    definition = {"kind": "app_definition", "schema_version": 2, "organization_id": org, "app_id": app, "revision_id": revision, "content": content, "digest": digest("app-definition", content)}
    queries = [v["query"] for v in views if "query" in v]
    pub = {"kind": "publication", "schema_version": 2, "organization_id": org, "app_id": app, "publication_id": publication, "definition_revision_id": revision, "definition_digest": definition["digest"],
           "input_contract_digests": [{"input_id": i["id"], "digest": digest("input-contract", i)} for i in inputs],
           "record_schema_digests": [{"schema_id": "decision", "digest": digest("record-schema", records)}],
           "renderer_version": 1, "operator_version": 1, "operators": operator_identities(queries),
           "renderers": [{"kind": kind, "version": 1} for kind in sorted({v["kind"] for v in views})],
           "admission_digest": raw_hash(name + ":synthetic-admission-placeholder"),
           "policy": {"id": uid(offset + 8), "version": 1, "kind": "independent_review"}, "author_id": principal,
           "approver_id": uid(offset + 9), "approver_epoch": 1, "published_at": "2026-10-28T12:00:00Z"}
    sources, bindings, dependencies = [], [], []
    for n, contract in enumerate(inputs):
        if contract["kind"] == "records":
            bindings.append({"input_id": contract["id"], "kind": "records", "contract_digest": digest("input-contract", contract), "collection_id": collection, "schema_id": "decision", "schema_version": 1, "revision_policy": "committed_snapshot"})
            continue
        source_id, source_rev = uid(offset + 10 + n * 2), uid(offset + 11 + n * 2)
        normalized = raw_hash(name + ":" + contract["id"] + ":synthetic-normalized-placeholder")
        sources.append({"kind": "source_revision", "schema_version": 2, "organization_id": org, "source_id": source_id, "revision_id": source_rev,
                        "content_digest": raw_hash(name + ":" + contract["id"] + ":synthetic-original-placeholder"), "normalized_digest": normalized,
                        "rows_digest": rows_digest(synthetic_rows(contract)), "coverage": contract["coverage"],
                        "input_contract": copy.deepcopy(contract), "parser_version": 1, "mapping_version": 1, "format": "csv", "formula_policy": "reject", "row_count": 5,
                        "timezone": "UTC", "time_semantics": "event" if contract["id"] == "events" else "none", "units_confirmed": True,
                        "created_at": "2026-10-28T11:00:00Z", "provenance_sample": [{"type": "csv", "logical_record": 1, "physical_line_start": 2, "physical_line_end": 2}]})
        dependency = {"input_id": contract["id"], "source_id": source_id, "source_revision_id": source_rev, "source_digest": normalized}
        dependencies.append(dependency)
        bindings.append({**dependency, "kind": "source", "contract_digest": digest("input-contract", contract)})
    saved_work = {"kind": "saved_work", "schema_version": 1, "organization_id": org, "app_id": app, "work_id": work, "publication_id": publication, "version": 1,
                  "context": "Contract-shape validation only", "bindings": bindings, "record_collections": [{"id": collection, "schema_id": "decision", "schema_version": 1}], "source_dependencies": dependencies}
    view = views[0]
    read = {s["input_contract"]["id"]: s for s in sources}
    receipt = {"kind": "query_receipt", "schema_version": 2, "organization_id": org, "app_id": app, "work_id": work, "mode": "published", "definition_digest": definition["digest"],
               "publication_id": publication, "view_id": view["id"], "operator_version": 1, "operators": operator_identities([view["query"]]),
               "renderers": [{"kind": view["kind"], "version": 1}], "source_revisions": [{**d, "mapping_version": 1} for d in dependencies],
               "record_snapshot_sequence": 0, "parameters": [],
               "principal_id": principal, "membership_epoch": 1, "policy_epoch": 1, "filter_epoch": 0, "result_digest": digest("result", {"example": name}),
               "partial": any(read[i]["coverage"] == "partial" for i in view["query"]["inputs"] if i in read), "truncated_page": False,
               "total_scope": "full_authorized_filtered_input", "provenance": {"query_digest": digest("query", view["query"]), "predicate_digest": digest("query", view["query"])}}
    receipt["digest"] = digest("query-receipt", receipt)
    command = {"kind": "command_request", "schema_version": 1, "app_id": app, "work_id": work, "publication_id": publication, "command_id": "record_decision", "schema_id": "decision", "schema_version_expected": 1,
               "record_id": uid(offset + 40), "expected_record_version": 0, "expected_work_version": 1, "values": command_values, "idempotency_key": "0123456789abcdef0123456789abcdef"}
    outcome = {"kind": "command_outcome", "schema_version": 1, "organization_id": org, "app_id": app, "work_id": work, "principal_id": principal, "command_id": "record_decision", "idempotency_key": command["idempotency_key"],
               "payload_digest": payload_digest(command), "state": "committed", "record_id": command["record_id"], "record_version": 1, "work_version": 1, "commit_sequence": 1,
               "audit_event_id": uid(offset + 41), "committed_at": "2026-10-28T12:01:00Z", "retry_expires_at": "2026-10-29T12:01:00Z"}
    return {"context": {"organization_id": org, "principal_id": principal}, "definition": definition, "publication": pub, "work": saved_work, "sources": sources, "receipt": receipt, "command": command, "outcome": outcome}

def view(name, kind, q=None, **extra):
    return {"id": name, "kind": kind, "renderer_version": 1, "title": name.replace("_", " "), **({"query": q} if q else {}), **extra}

STATUS_VALUES = [{"field_id": "status", "value": {"type": "enum", "value": "needs_context"}}, {"field_id": "note", "value": {"type": "text", "value": "Review required — synthetic only"}}]
WINDOW = {"start": "2026-10-22T10:00:00Z", "end": "2026-10-22T10:10:00Z", "bounds": "half_open"}

def northstar(coverage="complete"):
    total = {"type": "aggregate", "group_by": [], "measures": [{"id": "total_bytes", "operation": "sum", "field": f("events", "bytes")}]}
    return bundle("Northstar investigation", 100,
        [("events", [field("time", "timestamp"), field("destination", "text"), field("bytes", "integer", unit="byte")]), ("inventory", [field("address", "text")])],
        [view("total", "summary", query(["events"], [total])),
         view("destinations", "table", query(["events", "inventory"], [{"type": "lookup", "left": f("events", "destination"), "right": f("inventory", "address"), "rule": "cidr_exact_then_longest", "cardinality": "at_most_one"}, {"type": "project", "fields": [f("events", "destination"), f("events", "bytes")]}])),
         view("timeline", "timeline", query(["events"], [{"type": "time_bucket", "field": f("events", "time"), "seconds": 60, "timezone": "UTC", "output_field": "minute"}])),
         view("record_observation", "form", command_id="record_decision"), view("caution", "note", text="An unmatched destination does not establish compromise."),
         view("rate", "summary", query(["events"], [total, {"type": "ratio", "numerator": "total_bytes", "denominator": {"type": "window_minutes"}, "output_field": "bytes_per_minute",
                                                              "output_unit": "byte_per_minute", "zero_denominator": "error", "rounding": "exact_rational"}], WINDOW))],
        STATUS_VALUES, coverage)

def spend_review():
    return bundle("Spend Review", 200,
        [("expenses", [field("expense_id", "text"), field("amount", "money", currency="USD", minor_unit_exponent=2)])],
        [view("expenses", "table", query(["expenses"], [{"type": "project", "fields": [f("expenses", "expense_id"), f("expenses", "amount")]}])),
         view("queue", "detail", query(["expenses"], [{"type": "filter", "field": f("expenses", "amount"), "comparison": "gte", "value": {"type": "money", "minor_units": "20000", "currency": "USD", "minor_unit_exponent": 2}}])),
         view("decision", "form", command_id="record_decision"),
         view("status", "status", query(["decisions"], [{"type": "project", "fields": [f("decisions", "status")]}]))], STATUS_VALUES)

def vector(name, value, domain=None, **extra):
    short = domain[len("forma:"):-len(":v1")] if domain else "result"
    return {"name": name, **({"domain": domain} if domain else {}), **extra, "value": value, "utf8_hex": canonical(value).hex(), "digest": digest(short, value)}

def canonical_vectors():
    """Edition one's vectors verbatim, then edition two's new domains and the exact rational value."""
    vectors = json.loads((RETAINED / "canonical-vectors.json").read_text())
    vectors.append(vector("northstar_event_rows", EVENT_ROWS, "forma:normalized-rows:v1"))
    vectors.append(vector("empty_row_stream", [], "forma:normalized-rows:v1"))
    vectors.append(vector("import_job_payload", {"upload_id": uid(0x501), "content_digest": raw_hash("synthetic-staged-original-placeholder"), "format": "csv", "byte_length": 4096},
                          "forma:job-payload:v1"))
    largest = 10 ** 38 - 1
    for name, numerator, denominator in (("full_window_rate", 1140000, 10), ("fractional_rate", 50, 40), ("negative_rate", 6, -4), ("widest_rate", largest, largest - 1)):
        vectors.append(vector(name, exact_rational(numerator, denominator), rational={"numerator": str(numerator), "denominator": str(denominator)}))
    return vectors

def command_recovery():
    """Edition impl-2026-10-29.3 (ADR-007 D7.9/D7.15, APP-013): the record command route's refusal bodies.

    The Spend Review S05 sequence after ik-1 committed version 1: a stale edit expecting version 0 (409 with
    the recovery view naming the current record), ik-1 reused with another payload (409 naming the stored
    outcome), a moved work premise (412 naming the current revision), and the two waits. Synthetic: the
    outcome is the bundle's, not a committed row.
    """
    b = spend_review()
    command, outcome, work = b["command"], b["outcome"], b["work"]
    scope = {"organization_id": outcome["organization_id"], "app_id": command["app_id"], "work_id": command["work_id"],
             "publication_id": command["publication_id"], "work_version": work["version"], "record_id": command["record_id"], "command_id": command["command_id"]}
    current = [{"field_id": "status", "value": {"type": "enum", "value": "needs_context"}},
               {"field_id": "note", "value": {"type": "text", "value": "Review required — synthetic only"}}]
    stale_edit = {"kind": "recovery_view", "schema_version": 1, **scope, "idempotency_key": "fedcba9876543210fedcba9876543210", "reason": "expected_version",
                  "expected_record_version": 0, "current_record_version": 1, "current_values": current, "stored_outcome": None}
    reused_key = {"kind": "recovery_view", "schema_version": 1, **scope, "idempotency_key": command["idempotency_key"], "reason": "payload_mismatch",
                  "expected_record_version": 0, "current_record_version": 1, "current_values": current, "stored_outcome": outcome}
    absent = {"kind": "recovery_view", "schema_version": 1, **scope, "idempotency_key": "00000000000000000000000000000009", "reason": "expected_version",
              "expected_record_version": 3, "current_record_version": 0, "current_values": None, "stored_outcome": None}
    claim = {"kind": "command_claim", "schema_version": 1, "organization_id": outcome["organization_id"], "app_id": command["app_id"], "work_id": command["work_id"],
             "principal_id": outcome["principal_id"], "command_id": command["command_id"], "idempotency_key": command["idempotency_key"],
             "payload_digest": outcome["payload_digest"], "claimed_at": outcome["committed_at"]}
    problems = [
        {"status": 409, "code": "conflict", "recovery_view": stale_edit},
        {"status": 409, "code": "conflict", "recovery_view": reused_key},
        {"status": 412, "code": "stale", "current_revision": {"publication_id": command["publication_id"], "definition_revision_id": b["definition"]["revision_id"],
                                                               "definition_digest": b["definition"]["digest"], "work_version": work["version"]}},
        {"status": 409, "code": "in_flight", "retry_after_seconds": 2},
        {"status": 409, "code": "busy", "retry_after_seconds": 2},
        {"status": 400, "code": "missing_idempotency_key"},
        {"status": 400, "code": "malformed_idempotency_key"},
        {"status": 400, "code": "malformed_command"},
        {"status": 503, "code": "unknown_outcome"},
    ]
    return {"recovery_views": [stale_edit, reused_key, absent], "claim": claim, "problems": problems}

def main():
    (ROOT / "fixtures").mkdir(exist_ok=True)
    for name, value in (("northstar", northstar()), ("spend-review", spend_review()), ("canonical-vectors", canonical_vectors()),
                        ("command-recovery", command_recovery())):
        (ROOT / "fixtures" / (name + ".json")).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")

if __name__ == "__main__":
    main()
