"""Executable schema source; emit schema.json with `python3 contracts/schema.py`."""
import json
from pathlib import Path


def obj(properties, optional=()):
    return {"type": "object", "properties": properties,
            "required": [k for k in properties if k not in optional],
            "additionalProperties": False}


def arr(items, maximum, minimum=0):
    return {"type": "array", "items": items, "minItems": minimum, "maxItems": maximum}


def ref(name):
    return {"$ref": "#/$defs/" + name}


def enum(*values):
    return {"enum": list(values)}


def tagged(kind, fields):
    return obj({"type": {"const": kind}, **fields})


ID = {"type": "string", "pattern": "^[a-z][a-z0-9_]{0,63}(?![\\s\\S])"}
UUID = {"type": "string", "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}(?![\\s\\S])"}
DIGEST = {"type": "string", "pattern": "^sha256:[0-9a-f]{64}(?![\\s\\S])"}
INT = {"type": "integer", "minimum": 0, "maximum": 9007199254740991}
POS = {**INT, "minimum": 1}
INTEGER_TEXT = {"type": "string", "pattern": "^(0|-[1-9][0-9]{0,37}|[1-9][0-9]{0,37})(?![\\s\\S])"}
TEXT = {"type": "string", "maxLength": 2000}
TIME = {"type": "string", "pattern": "^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z(?![\\s\\S])", "format": "date-time"}
DATE = {"type": "string", "pattern": "^[0-9]{4}-[0-9]{2}-[0-9]{2}(?![\\s\\S])", "format": "date"}
CURRENCY = {"enum": ["USD"]}  # Initial admitted money contract; expansion is a reviewed edition change.
VERSION = {"const": 1}
SCOPE = {"organization_id": UUID, "app_id": UUID}
VALUE_TYPES = [tagged("text", {}), tagged("boolean", {}), tagged("integer", {"unit": ID}),
               tagged("decimal", {"unit": ID, "scale": {"type": "integer", "minimum": 0, "maximum": 18}}),
               tagged("date", {}), tagged("timestamp", {}),
               tagged("enum", {"values": {**arr(ID, 32, 1), "uniqueItems": True}}),
               tagged("reference", {"target_input": ID}),
               tagged("money", {"currency": CURRENCY, "minor_unit_exponent": {"const": 2}})]
VALUES = [tagged("text", {"value": TEXT}), tagged("boolean", {"value": {"type": "boolean"}}),
          tagged("integer", {"value": INTEGER_TEXT, "unit": ID}),
          tagged("decimal", {"coefficient": INTEGER_TEXT, "scale": {"type": "integer", "minimum": 0, "maximum": 18}, "unit": ID}),
          tagged("date", {"value": DATE}), tagged("timestamp", {"value": TIME}),
          tagged("enum", {"value": ID}), tagged("reference", {"input_id": ID, "record_id": UUID}),
          tagged("money", {"minor_units": INTEGER_TEXT, "currency": CURRENCY, "minor_unit_exponent": {"const": 2}}),
          tagged("null", {})]
FIELD_REF = obj({"input_id": ID, "field_id": ID})
AGG = obj({"id": ID, "operation": enum("count", "sum", "min", "max"), "field": FIELD_REF})
OPERATORS = [
    tagged("project", {"fields": arr(FIELD_REF, 32, 1)}),
    tagged("filter", {"field": FIELD_REF, "comparison": enum("eq", "ne", "lt", "lte", "gt", "gte"), "value": ref("value")}),
    tagged("aggregate", {"group_by": arr(FIELD_REF, 4), "measures": arr(AGG, 8, 1)}),
    tagged("sort", {"fields": arr(obj({"field": FIELD_REF, "direction": enum("asc", "desc")}), 4, 1), "stable_tiebreak": FIELD_REF}),
    tagged("limit", {"rows": {"type": "integer", "minimum": 1, "maximum": 500}}),
    tagged("time_bucket", {"field": FIELD_REF, "seconds": enum(60, 300, 3600, 86400), "timezone": {"const": "UTC"}, "output_field": ID}),
    tagged("ratio", {"numerator": ID, "denominator": {"oneOf": [tagged("measure", {"id": ID}), tagged("constant", {"value": {"type": "string", "pattern": "^[1-9][0-9]{0,17}(?![\\s\\S])"}, "unit": ID})]}, "output_field": ID, "output_unit": ID, "zero_denominator": {"const": "error"}, "rounding": {"const": "exact_rational"}}),
    tagged("lookup", {"left": FIELD_REF, "right": FIELD_REF, "rule": enum("exact", "cidr_exact_then_longest"), "cardinality": {"const": "at_most_one"}}),
]
RENDERERS = ("summary", "table", "detail", "timeline", "chart", "filter", "form", "note", "status")
DEFS = {
    "value": {"oneOf": VALUES},
    "field": obj({"id": ID, "value_type": {"oneOf": VALUE_TYPES}, "nullable": {"type": "boolean"}}),
    "input_contract": obj({"id": ID, "contract_id": ID, "contract_version": VERSION, "kind": enum("source", "records"), "fields": arr(ref("field"), 32, 1)}),
    "record_schema": obj({"id": ID, "version": VERSION, "fields": arr(ref("field"), 32, 1)}),
    "command_declaration": obj({"id": ID, "record_schema_id": ID, "operation": enum("create", "update"), "writable_fields": {**arr(ID, 32, 1), "uniqueItems": True}, "transitions": arr(obj({"field_id": ID, "from": ID, "to": ID}), 32)}),
    "query": obj({"operator_version": VERSION, "inputs": {**arr(ID, 8, 1), "uniqueItems": True}, "operations": arr({"oneOf": OPERATORS}, 8, 1), "null_policy": {"const": "unknown"}, "window": obj({"start": TIME, "end": TIME, "bounds": {"const": "half_open"}})}, optional=("window",)),
    "view": obj({"id": ID, "kind": enum(*RENDERERS), "renderer_version": VERSION, "title": TEXT, "query": ref("query"), "command_id": ID, "text": TEXT}, optional=("query", "command_id", "text")),
    "definition_content": obj({"title": TEXT, "description": TEXT, "inputs": arr(ref("input_contract"), 8), "views": arr(ref("view"), 24, 1), "record_schemas": arr(ref("record_schema"), 8), "commands": arr(ref("command_declaration"), 16), "links": arr(obj({"from_view": ID, "to_view": ID, "input_id": ID}), 48), "default_layout": arr(obj({"view_id": ID, "column": {"type": "integer", "minimum": 0, "maximum": 11}, "row": {"type": "integer", "minimum": 0, "maximum": 100}, "width": {"type": "integer", "minimum": 1, "maximum": 12}}), 24, 1)}),
    "binding": {"oneOf": [
        obj({"input_id": ID, "kind": {"const": "source"}, "contract_digest": DIGEST, "source_id": UUID, "source_revision_id": UUID, "source_digest": DIGEST}),
        obj({"input_id": ID, "kind": {"const": "records"}, "contract_digest": DIGEST, "collection_id": UUID, "schema_id": ID, "schema_version": VERSION, "revision_policy": {"const": "committed_snapshot"}})]},
    "source_dependency": obj({"input_id": ID, "source_id": UUID, "source_revision_id": UUID, "source_digest": DIGEST}),
    "named_value": obj({"field_id": ID, "value": ref("value")}),
    "source_coordinate": {"oneOf": [tagged("csv", {"logical_record": POS, "physical_line_start": POS, "physical_line_end": POS}), tagged("xlsx", {"sheet": TEXT, "row": POS, "column": POS})]},
}


def entity(kind, fields, optional=()):
    return obj({"kind": {"const": kind}, "schema_version": VERSION, **fields}, optional)


DEFS.update({
    "app_definition": entity("app_definition", {**SCOPE, "revision_id": UUID, "content": ref("definition_content"), "digest": DIGEST}),
    "publication": entity("publication", {**SCOPE, "publication_id": UUID, "definition_revision_id": UUID, "definition_digest": DIGEST, "input_contract_digests": arr(obj({"input_id": ID, "digest": DIGEST}), 8), "record_schema_digests": arr(obj({"schema_id": ID, "digest": DIGEST}), 8), "renderer_version": VERSION, "operator_version": VERSION, "admission_digest": DIGEST, "policy": obj({"id": UUID, "version": POS, "kind": enum("owner_publish", "independent_review")}), "author_id": UUID, "approver_id": UUID, "approver_epoch": POS, "published_at": TIME}),
    "saved_work": entity("saved_work", {**SCOPE, "work_id": UUID, "publication_id": UUID, "version": POS, "context": TEXT, "bindings": arr(ref("binding"), 8), "record_collections": arr(obj({"id": UUID, "schema_id": ID, "schema_version": VERSION}), 8), "source_dependencies": arr(ref("source_dependency"), 8)}),
    "source_revision": entity("source_revision", {"organization_id": UUID, "source_id": UUID, "revision_id": UUID, "content_digest": DIGEST, "normalized_digest": DIGEST, "input_contract": ref("input_contract"), "parser_version": VERSION, "mapping_version": POS, "format": enum("csv", "xlsx"), "formula_policy": {"const": "reject"}, "row_count": INT, "timezone": {"const": "UTC"}, "time_semantics": enum("event", "interval", "none"), "units_confirmed": {"const": True}, "created_at": TIME, "provenance_sample": arr(ref("source_coordinate"), 32), "deleted_at": TIME}, optional=("deleted_at",)),
    "query_receipt": entity("query_receipt", {**SCOPE, "work_id": UUID, "mode": enum("published", "synthetic_preview", "authorized_read_preview"), "definition_digest": DIGEST, "publication_id": UUID, "view_id": ID, "operator_version": VERSION, "source_revisions": arr(ref("source_dependency"), 8), "record_snapshot_sequence": INT, "parameters": arr(ref("named_value"), 32), "principal_id": UUID, "membership_epoch": POS, "policy_epoch": POS, "filter_epoch": INT, "result_digest": DIGEST, "partial": {"const": False}, "truncated_page": {"type": "boolean"}, "total_scope": {"const": "full_authorized_filtered_input"}, "provenance": obj({"query_digest": DIGEST, "predicate_digest": DIGEST}), "digest": DIGEST}, optional=("publication_id",)),
    "command_request": entity("command_request", {"app_id": UUID, "work_id": UUID, "publication_id": UUID, "command_id": ID, "schema_id": ID, "schema_version_expected": VERSION, "record_id": UUID, "expected_record_version": INT, "expected_work_version": POS, "values": arr(ref("named_value"), 32, 1), "idempotency_key": {"type": "string", "pattern": "^[0-9a-f]{32}(?![\\s\\S])"}}),
    "command_outcome": entity("command_outcome", {**SCOPE, "work_id": UUID, "principal_id": UUID, "command_id": ID, "idempotency_key": {"type": "string", "pattern": "^[0-9a-f]{32}(?![\\s\\S])"}, "payload_digest": DIGEST, "state": {"const": "committed"}, "record_id": UUID, "record_version": POS, "work_version": POS, "commit_sequence": POS, "audit_event_id": UUID, "committed_at": TIME, "retry_expires_at": TIME}),
    "job_state": entity("job_state", {"organization_id": UUID, "job_id": UUID, "state": enum("queued", "running", "failed", "done"), "claim_sequence": INT, "result_digest": DIGEST, "failure_code": ID}, optional=("result_digest", "failure_code")),
})
SCHEMA = {"$schema": "https://json-schema.org/draft/2020-12/schema", "$id": "https://getforma.dev/contracts/impl-2026-10-28.1/schema.json", "$defs": DEFS,
          "oneOf": [ref(n) for n in ("app_definition", "publication", "saved_work", "source_revision", "query_receipt", "command_request", "command_outcome", "job_state")]}

if __name__ == "__main__":
    Path(__file__).with_name("schema.json").write_text(json.dumps(SCHEMA, indent=2) + "\n")
