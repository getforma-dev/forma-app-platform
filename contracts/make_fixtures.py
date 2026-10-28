"""Regenerate clearly synthetic contract examples; no parser/query execution is claimed."""
import copy
import hashlib
import json
from pathlib import Path

from validate import digest, payload_digest

ROOT = Path(__file__).resolve().parent
def uid(n):
    return f"00000000-0000-4000-8000-{n:012x}"

def raw_hash(label):
    return "sha256:" + hashlib.sha256(label.encode()).hexdigest()

def field(name, kind, **extras):
    return {"id": name, "value_type": {"type": kind, **extras}, "nullable": False}

def f(input_id, field_id):
    return {"input_id": input_id, "field_id": field_id}

def query(inputs, operations):
    return {"operator_version": 1, "inputs": inputs, "operations": operations, "null_policy": "unknown"}

def bundle(name, offset, input_specs, views, command_values):
    org, app, principal, revision, publication, work, collection = map(uid, range(offset, offset + 7))
    decisions = [field("status", "enum", values=["needs_context", "reviewed"]), field("note", "text")]
    records = {"id": "decision", "version": 1, "fields": decisions}
    inputs = [{"id": k, "contract_id": k + "_v1", "contract_version": 1, "kind": "source", "fields": v} for k, v in input_specs]
    inputs.append({"id": "decisions", "contract_id": "decisions_v1", "contract_version": 1, "kind": "records", "fields": decisions})
    content = {"title": name, "description": "Synthetic contract example; no runtime or query result is claimed.", "inputs": inputs, "views": views,
               "record_schemas": [records], "commands": [{"id": "record_decision", "record_schema_id": "decision", "operation": "create", "writable_fields": ["status", "note"], "transitions": [{"field_id": "status", "from": "needs_context", "to": "reviewed"}]}],
               "links": [], "default_layout": [{"view_id": v["id"], "column": 0, "row": i, "width": 12} for i, v in enumerate(views)]}
    definition = {"kind": "app_definition", "schema_version": 1, "organization_id": org, "app_id": app, "revision_id": revision, "content": content, "digest": digest("app-definition", content)}
    pub = {"kind": "publication", "schema_version": 1, "organization_id": org, "app_id": app, "publication_id": publication, "definition_revision_id": revision, "definition_digest": definition["digest"],
           "input_contract_digests": [{"input_id": i["id"], "digest": digest("input-contract", i)} for i in inputs],
           "record_schema_digests": [{"schema_id": "decision", "digest": digest("record-schema", records)}],
           "renderer_version": 1, "operator_version": 1, "admission_digest": raw_hash(name + ":synthetic-admission-placeholder"),
           "policy": {"id": uid(offset + 8), "version": 1, "kind": "independent_review"}, "author_id": principal,
           "approver_id": uid(offset + 9), "approver_epoch": 1, "published_at": "2026-10-28T12:00:00Z"}
    sources, bindings, dependencies = [], [], []
    for n, contract in enumerate(inputs):
        if contract["kind"] == "records":
            bindings.append({"input_id": contract["id"], "kind": "records", "contract_digest": digest("input-contract", contract), "collection_id": collection, "schema_id": "decision", "schema_version": 1, "revision_policy": "committed_snapshot"})
            continue
        source_id, source_rev = uid(offset + 10 + n * 2), uid(offset + 11 + n * 2)
        normalized = raw_hash(name + ":" + contract["id"] + ":synthetic-normalized-placeholder")
        sources.append({"kind": "source_revision", "schema_version": 1, "organization_id": org, "source_id": source_id, "revision_id": source_rev,
                        "content_digest": raw_hash(name + ":" + contract["id"] + ":synthetic-original-placeholder"), "normalized_digest": normalized,
                        "input_contract": copy.deepcopy(contract), "parser_version": 1, "mapping_version": 1, "format": "csv", "formula_policy": "reject", "row_count": 5,
                        "timezone": "UTC", "time_semantics": "event" if contract["id"] == "events" else "none", "units_confirmed": True,
                        "created_at": "2026-10-28T11:00:00Z", "provenance_sample": [{"type": "csv", "logical_record": 1, "physical_line_start": 2, "physical_line_end": 2}]})
        dependency = {"input_id": contract["id"], "source_id": source_id, "source_revision_id": source_rev, "source_digest": normalized}
        dependencies.append(dependency)
        bindings.append({**dependency, "kind": "source", "contract_digest": digest("input-contract", contract)})
    saved_work = {"kind": "saved_work", "schema_version": 1, "organization_id": org, "app_id": app, "work_id": work, "publication_id": publication, "version": 1,
                  "context": "Contract-shape validation only", "bindings": bindings, "record_collections": [{"id": collection, "schema_id": "decision", "schema_version": 1}], "source_dependencies": dependencies}
    receipt = {"kind": "query_receipt", "schema_version": 1, "organization_id": org, "app_id": app, "work_id": work, "mode": "published", "definition_digest": definition["digest"],
               "publication_id": publication, "view_id": views[0]["id"], "operator_version": 1, "source_revisions": dependencies, "record_snapshot_sequence": 0, "parameters": [],
               "principal_id": principal, "membership_epoch": 1, "policy_epoch": 1, "filter_epoch": 0, "result_digest": digest("result", {"example": name}), "partial": False, "truncated_page": False,
               "total_scope": "full_authorized_filtered_input", "provenance": {"query_digest": digest("query", views[0]["query"]), "predicate_digest": digest("query", views[0]["query"])}}
    receipt["digest"] = digest("query-receipt", receipt)
    command = {"kind": "command_request", "schema_version": 1, "app_id": app, "work_id": work, "publication_id": publication, "command_id": "record_decision", "schema_id": "decision", "schema_version_expected": 1,
               "record_id": uid(offset + 40), "expected_record_version": 0, "expected_work_version": 1, "values": command_values, "idempotency_key": "0123456789abcdef0123456789abcdef"}
    outcome = {"kind": "command_outcome", "schema_version": 1, "organization_id": org, "app_id": app, "work_id": work, "principal_id": principal, "command_id": "record_decision", "idempotency_key": command["idempotency_key"],
               "payload_digest": payload_digest(command), "state": "committed", "record_id": command["record_id"], "record_version": 1, "work_version": 1, "commit_sequence": 1,
               "audit_event_id": uid(offset + 41), "committed_at": "2026-10-28T12:01:00Z", "retry_expires_at": "2026-10-29T12:01:00Z"}
    return {"context": {"organization_id": org, "principal_id": principal}, "definition": definition, "publication": pub, "work": saved_work, "sources": sources, "receipt": receipt, "command": command, "outcome": outcome}

def view(name, kind, q=None, **extra):
    return {"id": name, "kind": kind, "renderer_version": 1, "title": name.replace("_", " "), **({"query": q} if q else {}), **extra}

def main():
    status_values = [{"field_id": "status", "value": {"type": "enum", "value": "needs_context"}}, {"field_id": "note", "value": {"type": "text", "value": "Review required — synthetic only"}}]
    northstar = bundle("Northstar investigation", 100,
        [("events", [field("time", "timestamp"), field("destination", "text"), field("bytes", "integer", unit="byte")]), ("inventory", [field("address", "text")])],
        [view("total", "summary", query(["events"], [{"type": "aggregate", "group_by": [], "measures": [{"id": "total_bytes", "operation": "sum", "field": f("events", "bytes")}]}])),
         view("destinations", "table", query(["events", "inventory"], [{"type": "lookup", "left": f("events", "destination"), "right": f("inventory", "address"), "rule": "cidr_exact_then_longest", "cardinality": "at_most_one"}, {"type": "project", "fields": [f("events", "destination"), f("events", "bytes")]}])),
         view("timeline", "timeline", query(["events"], [{"type": "time_bucket", "field": f("events", "time"), "seconds": 60, "timezone": "UTC", "output_field": "minute"}])),
         view("record_observation", "form", command_id="record_decision"), view("caution", "note", text="An unmatched destination does not establish compromise.")], status_values)
    spend = bundle("Spend Review", 200,
        [("expenses", [field("expense_id", "text"), field("amount", "money", currency="USD", minor_unit_exponent=2)])],
        [view("expenses", "table", query(["expenses"], [{"type": "project", "fields": [f("expenses", "expense_id"), f("expenses", "amount")]}])),
         view("queue", "detail", query(["expenses"], [{"type": "filter", "field": f("expenses", "amount"), "comparison": "gte", "value": {"type": "money", "minor_units": "20000", "currency": "USD", "minor_unit_exponent": 2}}])),
         view("decision", "form", command_id="record_decision"),
         view("status", "status", query(["decisions"], [{"type": "project", "fields": [f("decisions", "status")]}]))], status_values)
    (ROOT / "fixtures").mkdir(exist_ok=True)
    for name, value in (("northstar", northstar), ("spend-review", spend)):
        (ROOT / "fixtures" / (name + ".json")).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")

if __name__ == "__main__":
    main()
