"""Contract checks for synthetic fixture qualification, not an authorization service."""
from __future__ import annotations

import hashlib
import json
import math
import re
from datetime import date, datetime, timedelta
from pathlib import Path

from jsonschema import Draft202012Validator, FormatChecker

ROOT = Path(__file__).resolve().parent
SCHEMA = json.loads((ROOT / "schema.json").read_text())
Draft202012Validator.check_schema(SCHEMA)
FORMATS = FormatChecker()


@FORMATS.checks("date-time", raises=ValueError)
def valid_timestamp(value):
    if not isinstance(value, str):
        return True
    datetime.fromisoformat(value.replace("Z", "+00:00"))
    return True


@FORMATS.checks("date", raises=ValueError)
def valid_date(value):
    if not isinstance(value, str):
        return True
    date.fromisoformat(value)
    return True


VALIDATOR = Draft202012Validator(SCHEMA, format_checker=FORMATS)
# Edition impl-2026-10-29.2 adds normalized-rows (ADR-009 D9.3) and job-payload (genui.jobs.payload_digest, D9.4).
DOMAINS = {name: f"forma:{name}:v1" for name in (
    "app-definition", "input-contract", "record-schema", "query", "query-receipt",
    "command-payload", "source-binding", "result", "normalized-rows", "job-payload")}
REGISTRY_EDITION = 1  # The operator and renderer registry edition every admitted definition names.
MAX_RATIONAL_DIGITS = 38


def _part(name):
    return Draft202012Validator({"$ref": "#/$defs/" + name, "$defs": SCHEMA["$defs"]}, format_checker=FORMATS)


PROBLEM_VALIDATOR = _part("problem")
RATIONAL_VALIDATOR = _part("exact_rational")


class ContractError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise ContractError(message)


def _duplicates(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "duplicate object key: " + key)
        result[key] = value
    return result


def loads(raw):
    """Decode bounded UTF-8 JSON before schema validation, refusing ambiguous input."""
    if isinstance(raw, bytes):
        require(len(raw) <= 1024 * 1024, "document exceeds 1 MiB")
        raw = raw.decode("utf-8", errors="strict")
    require(len(raw.encode("utf-8")) <= 1024 * 1024, "document exceeds 1 MiB")
    try:
        value = json.loads(raw, object_pairs_hook=_duplicates,
                           parse_float=lambda _: (_ for _ in ()).throw(ContractError("floating-point number refused")),
                           parse_constant=lambda _: (_ for _ in ()).throw(ContractError("non-finite number refused")))
    except RecursionError as exc:
        raise ContractError("JSON nesting exceeds decoder limit") from exc
    canonical(value)
    return value


def canonical(value):
    """forma-json-v1: restricted JSON; deliberately not RFC 8785/JCS."""
    def check(item, depth=0):
        require(depth <= 32, "JSON nesting exceeds 32")
        if item is None or isinstance(item, bool):
            return
        if isinstance(item, str):
            try:
                item.encode("utf-8", errors="strict")
            except UnicodeEncodeError as exc:
                raise ContractError("unpaired Unicode surrogate") from exc
        elif type(item) is int:
            require(abs(item) <= 9007199254740991, "unsafe JSON integer")
        elif isinstance(item, list):
            for child in item:
                check(child, depth + 1)
        elif isinstance(item, dict):
            for key, child in item.items():
                require(isinstance(key, str) and re.fullmatch(r"[a-z][a-z0-9_]*", key), "non-ASCII or invalid object key")
                check(child, depth + 1)
        else:
            raise ContractError("unsupported JSON type (floats are refused)")
    check(value)
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


def digest(domain, value):
    require(domain in DOMAINS, "unregistered digest domain")
    return "sha256:" + hashlib.sha256(DOMAINS[domain].encode("ascii") + b"\x00" + canonical(value)).hexdigest()


def domain_name(domain):
    """The short registry name of a full `forma:<name>:v1` domain."""
    names = {full: name for name, full in DOMAINS.items()}
    require(domain in names, "unregistered digest domain")
    return names[domain]


def rows_digest(rows):
    """forma:normalized-rows:v1 over the forma-json-v1 array of typed rows in their sorted order (ADR-009 D9.3)."""
    require(isinstance(rows, list), "normalized rows are an array")
    return digest("normalized-rows", rows)


def exact_rational(numerator, denominator):
    """Reduce exact integers (each at most 38 digits) to the canonical exact_rational value."""
    require(type(numerator) is int and type(denominator) is int, "exact_rational takes integers")
    require(denominator != 0, "zero denominator")
    require(max(len(str(abs(numerator))), len(str(abs(denominator)))) <= MAX_RATIONAL_DIGITS, "exact_rational exceeds 38 digits")
    if denominator < 0:
        numerator, denominator = -numerator, -denominator
    divisor = math.gcd(numerator, denominator)
    return {"type": "exact_rational", "numerator": str(numerator // divisor), "denominator": str(denominator // divisor)}


def validate_exact_rational(value):
    errors = sorted(RATIONAL_VALIDATOR.iter_errors(value), key=lambda e: str(list(e.path)))
    if errors:
        raise ContractError("schema: " + errors[0].message)
    require(math.gcd(int(value["numerator"]), int(value["denominator"])) == 1, "exact_rational must be in lowest terms")
    return value


def validate_problem(body):
    """application/problem+json bodies for the codes this edition registers."""
    canonical(body)
    require(all(type(v) is int for k, v in body.items() if not isinstance(v, str) and k not in ("recovery_view", "current_revision")),
            "problem members are text or integers, or the registered recovery view and current revision objects")
    errors = sorted(PROBLEM_VALIDATOR.iter_errors(body), key=lambda e: str(list(e.path)))
    if errors:
        raise ContractError("schema: " + errors[0].message)
    code = body["code"]
    require(code != "payload_too_large" or body["status"] == 413, "payload_too_large is 413")
    require(code != "limit_exceeded" or body["status"] == 429, "limit_exceeded is 429")
    names = code == "limit_exceeded" and "limit" in body and "retry_after_seconds" in body
    # Edition impl-2026-10-29.3 (ADR-007 D7.9): in_flight and busy are 409 with Retry-After and no limit.
    waiting = code in ("in_flight", "busy")
    require(not waiting or (body["status"] == 409 and "retry_after_seconds" in body and "limit" not in body),
            "in_flight and busy are 409 with Retry-After seconds")
    require((body["status"] == 429) == names and ("limit" in body) == names and ("retry_after_seconds" in body) == (names or waiting),
            "a 429 names its limit and Retry-After seconds, and only a 429, in_flight or busy does")
    # D7.15: the recovery view travels only on a 409 conflict, the current revision only on a 412 stale.
    require((code == "conflict") == ("recovery_view" in body) and (code != "conflict" or body["status"] == 409),
            "a conflict is 409 with the recovery view, and only a conflict carries one")
    require((code == "stale") == ("current_revision" in body) and (code != "stale" or body["status"] == 412),
            "stale is 412 with the current revision, and only stale carries one")
    if "recovery_view" in body:
        validate(body["recovery_view"])
    return body


def _seconds(instant):
    return int(datetime.fromisoformat(instant.replace("Z", "+00:00")).timestamp())


def _sorted_identities(items, key, label):
    names = [item[key] for item in items]
    require(names == sorted(set(names)), label + " identities must be sorted and unique")


def _operator_identities(queries):
    return [{"id": name, "version": REGISTRY_EDITION} for name in sorted({op["type"] for query in queries for op in query["operations"]})]


def payload_digest(command):
    return digest("command-payload", {k: v for k, v in command.items() if k != "idempotency_key"})


def unique(items, key, label):
    values = [item[key] for item in items]
    require(len(set(values)) == len(values), "duplicate " + label)
    return {item[key]: item for item in items}


def compatible(value, field):
    if value["type"] == "null":
        return field["nullable"]
    expected = field["value_type"]
    if value["type"] != expected["type"]:
        return False
    if expected["type"] == "enum":
        return value["value"] in expected["values"]
    if expected["type"] == "reference":
        return value["input_id"] == expected["target_input"]
    return all(value.get(k) == v for k, v in expected.items() if k != "type")


GUARD_ORDERED = ("integer", "decimal", "money", "date", "timestamp")


def _guard(guard, fields):
    """APP-107: a declared guard compares one field of the same record schema with a typed, non-null literal."""
    field = fields.get(guard["field_id"])
    require(field is not None, "unknown guard field")
    require(guard["value"]["type"] != "null" and compatible(guard["value"], field), "guard value type/unit mismatch")
    require(guard["comparison"] in ("eq", "ne") or field["value_type"]["type"] in GUARD_ORDERED, "guard comparison requires an ordered field")


def guard_holds(guard, state):
    """APP-107: a guard over a record state ({field_id: value}); an absent or null value never satisfies it."""
    value = state.get(guard["field_id"])
    if value is None or value["type"] == "null":
        return False
    literal = guard["value"]
    if value["type"] in ("money",):
        a, b = int(value["minor_units"]), int(literal["minor_units"])
    elif value["type"] == "decimal":
        a, b = int(value["coefficient"]), int(literal["coefficient"])
    elif value["type"] == "integer":
        a, b = int(value["value"]), int(literal["value"])
    else:
        a, b = value.get("value"), literal.get("value")
    return {"eq": a == b, "ne": a != b, "lt": a < b, "lte": a <= b, "gt": a > b, "gte": a >= b}[guard["comparison"]]


def _definition(content):
    inputs = unique(content["inputs"], "id", "input id")
    for item in inputs.values():
        if "coverage" in item and item["kind"] == "records":
            require(item["coverage"] == "complete", "records input requires complete coverage")
    fields = {name: unique(item["fields"], "id", "input field") for name, item in inputs.items()}
    records = unique(content["record_schemas"], "id", "record schema")
    record_fields = {name: unique(item["fields"], "id", "record field") for name, item in records.items()}
    views = unique(content["views"], "id", "view id")
    commands = unique(content["commands"], "id", "command id")
    for collection in [*inputs.values(), *records.values()]:
        for field in collection["fields"]:
            if field["value_type"]["type"] == "reference":
                require(field["value_type"]["target_input"] in inputs, "unknown reference target")
    for command in commands.values():
        require(command["record_schema_id"] in records, "unknown record schema")
        writable = record_fields[command["record_schema_id"]]
        require(set(command["writable_fields"]) <= writable.keys(), "unknown writable field")
        for transition in command["transitions"]:
            field = writable.get(transition["field_id"])
            require(field is not None and field["value_type"]["type"] == "enum", "transition requires enum field")
            require(transition["field_id"] in command["writable_fields"], "transition field is not writable")
            require({transition["from"], transition["to"]} <= set(field["value_type"]["values"]), "unknown transition value")
            if "when" in transition:
                _guard(transition["when"], writable)
        # APP-107 amendment: the starting value of an enum field on a create, chosen by declared guards.
        if "initial" in command:
            require(command["operation"] == "create" and not command["transitions"], "initial values belong to a filing create")
            for rule in command["initial"]:
                field = writable.get(rule["field_id"])
                require(field is not None and field["value_type"]["type"] == "enum", "initial requires enum field")
                require(rule["field_id"] in command["writable_fields"], "initial field is not writable")
                require(rule["value"] in field["value_type"]["values"], "unknown initial value")
                if "when" in rule:
                    _guard(rule["when"], writable)
    for view in views.values():
        if view["kind"] == "form":
            require(view.get("command_id") in commands and "query" not in view, "form requires declared command without query")
        else:
            require("command_id" not in view, "only a form invokes a command")
        if view["kind"] == "note":
            require("text" in view and "query" not in view, "note requires literal text")
        else:
            require("text" not in view, "literal text only belongs to notes")
        if view["kind"] not in ("form", "note"):
            require("query" in view, "data view requires query")
        if "query" not in view:
            continue
        query = view["query"]
        require(set(query["inputs"]) <= inputs.keys(), "query names unknown input")
        if "window" in query:
            require(query["window"]["start"] < query["window"]["end"], "time window must be increasing")
        measures = set()
        def lookup(field):
            require(field["input_id"] in query["inputs"], "field uses undeclared query input")
            require(field["field_id"] in fields[field["input_id"]], "unknown query field")
            return fields[field["input_id"]][field["field_id"]]
        for op in query["operations"]:
            kind = op["type"]
            if kind == "project":
                for field in op["fields"]:
                    lookup(field)
            elif kind == "filter":
                require(compatible(op["value"], lookup(op["field"])), "filter value type/unit mismatch")
            elif kind == "aggregate":
                for field in op["group_by"]:
                    lookup(field)
                unique(op["measures"], "id", "measure id")
                for measure in op["measures"]:
                    field = lookup(measure["field"])
                    if measure["operation"] == "sum":
                        require(field["value_type"]["type"] in ("integer", "decimal", "money"), "sum requires numeric field")
                    require(measure["id"] not in measures, "duplicate derived measure")
                    measures.add(measure["id"])
            elif kind == "sort":
                for field in op["fields"]:
                    lookup(field["field"])
                lookup(op["stable_tiebreak"])
            elif kind == "time_bucket":
                require(lookup(op["field"])["value_type"]["type"] == "timestamp", "time bucket requires timestamp")
            elif kind == "lookup":
                left, right = lookup(op["left"]), lookup(op["right"])
                require(left["value_type"] == right["value_type"], "lookup type mismatch")
                if op["rule"] == "cidr_exact_then_longest":
                    require(left["value_type"]["type"] == "text", "CIDR requires text fields")
            elif kind == "ratio":
                require(op["numerator"] in measures, "ratio requires prior declared numerator")
                if op["denominator"]["type"] == "measure":
                    require(op["denominator"]["id"] in measures, "ratio requires prior declared denominator")
                if op["denominator"]["type"] == "window_minutes":
                    require("window" in query, "window_minutes denominator requires a window")
                    span = _seconds(query["window"]["end"]) - _seconds(query["window"]["start"])
                    require(span % 60 == 0, "window must be a whole number of minutes")
                require(op["output_field"] not in measures, "duplicate derived measure")
                measures.add(op["output_field"])
    layout = unique(content["default_layout"], "view_id", "layout view")
    require(layout.keys() == views.keys(), "layout must name each view exactly once")
    for position in layout.values():
        require(position["column"] + position["width"] <= 12, "layout exceeds grid")
    for link in content["links"]:
        require(link["from_view"] in views and link["to_view"] in views and link["input_id"] in inputs, "unknown link target")


def validate(entity):
    raw = canonical(entity)
    require(len(raw) <= 1024 * 1024, "document exceeds 1 MiB")
    errors = sorted(VALIDATOR.iter_errors(entity), key=lambda e: str(list(e.path)))
    if errors:
        raise ContractError("schema: " + errors[0].message)
    kind = entity["kind"]
    if kind == "app_definition":
        require(len(canonical(entity["content"])) <= 256 * 1024, "definition exceeds 256 KiB")
        _definition(entity["content"])
        require(entity["digest"] == digest("app-definition", entity["content"]), "definition digest mismatch")
    elif kind == "publication":
        if entity["schema_version"] == 2:
            _sorted_identities(entity["operators"], "id", "operator")
            _sorted_identities(entity["renderers"], "kind", "renderer")
        if entity["policy"]["kind"] == "independent_review":
            require(entity["author_id"] != entity["approver_id"], "independent reviewer must differ from author")
        unique(entity["input_contract_digests"], "input_id", "publication input")
        unique(entity["record_schema_digests"], "schema_id", "publication record schema")
    elif kind == "saved_work":
        unique(entity["bindings"], "input_id", "binding input")
        unique(entity["record_collections"], "id", "record collection")
        unique(entity["source_dependencies"], "input_id", "source dependency")
    elif kind == "source_revision":
        require(entity["input_contract"]["kind"] == "source", "source revision needs source contract")
        if entity["schema_version"] == 2:
            require(entity["coverage"] == entity["input_contract"]["coverage"], "source coverage mismatch")
        unique(entity["input_contract"]["fields"], "id", "source field")
        require(entity["row_count"] <= (1000000 if entity["format"] == "csv" else 10000), "source row cap exceeded")
        for coord in entity["provenance_sample"]:
            require(coord["type"] == entity["format"], "coordinate format mismatch")
            if coord["type"] == "csv":
                require(coord["physical_line_start"] <= coord["physical_line_end"], "inverted physical span")
            else:
                require(coord["row"] <= 10000 and coord["column"] <= 32, "XLSX coordinate out of bounds")
    elif kind == "query_receipt":
        require((entity["mode"] == "published") == ("publication_id" in entity), "only published receipt names publication")
        if entity["schema_version"] == 2:
            _sorted_identities(entity["operators"], "id", "operator")
        unique(entity["source_revisions"], "input_id", "receipt source")
        unique(entity["parameters"], "field_id", "receipt parameter")
        require(entity["digest"] == digest("query-receipt", {k: v for k, v in entity.items() if k != "digest"}), "receipt digest mismatch")
    elif kind == "command_request":
        unique(entity["values"], "field_id", "command field")
    elif kind == "command_outcome":
        start = datetime.fromisoformat(entity["committed_at"].replace("Z", "+00:00"))
        end = datetime.fromisoformat(entity["retry_expires_at"].replace("Z", "+00:00"))
        require(end - start == timedelta(hours=24), "retry horizon must be 24 hours")
    elif kind == "recovery_view":
        # Edition impl-2026-10-29.3: an absent record has version 0 and no values; a key conflict names the
        # stored outcome under the same scope and key, a version conflict names none.
        require((entity["current_record_version"] == 0) == (entity["current_values"] is None), "current values iff the record exists")
        if entity["current_values"] is not None:
            unique(entity["current_values"], "field_id", "recovery value")
        stored = entity["stored_outcome"]
        require((entity["reason"] == "expected_version") == (stored is None), "a key conflict names the stored outcome; a version conflict names none")
        if stored is not None:
            validate(stored)
            require(all(stored[k] == entity[k] for k in ("organization_id", "app_id", "work_id", "command_id", "idempotency_key", "record_id")),
                    "stored outcome scope mismatch")
    elif kind == "job_state":
        require((entity["state"] == "done") == ("result_digest" in entity), "done iff committed result reference exists")
        require((entity["state"] == "failed") == ("failure_code" in entity), "failed iff failure code exists")
        if entity["schema_version"] == 2:
            queued = entity["state"] == "queued"
            # A claimed job reads queued until its start commits (D9.4 steps 2-3): no position, and a cancel
            # request may already be pending. Only a queued job ever carries position and wait.
            require(("queue_position" in entity) == ("expected_wait_seconds" in entity), "queue position and expected wait travel together")
            require(queued or "queue_position" not in entity, "only a queued job has a queue position")
            require(entity["state"] != "cancelled" or entity["cancel_requested"], "cancelled requires a cancel request")
    return entity


def validate_bundle(bundle):
    """Cross-artifact consistency for one synthetic fixture, not live authority."""
    require(set(bundle) == {"context", "definition", "publication", "work", "sources", "receipt", "command", "outcome"}, "unexpected bundle members")
    context = bundle["context"]
    require(set(context) == {"organization_id", "principal_id"}, "invalid trusted fixture context")
    entities = [bundle[k] for k in ("definition", "publication", "work", "receipt", "command", "outcome")] + bundle["sources"]
    for item in entities:
        validate(item)
        if "organization_id" in item:
            require(item["organization_id"] == context["organization_id"], "cross-organization reference")
    definition, pub, work = (bundle[k] for k in ("definition", "publication", "work"))
    receipt, command, outcome = (bundle[k] for k in ("receipt", "command", "outcome"))
    for item in (pub, work, receipt, command, outcome):
        require(item["app_id"] == definition["app_id"], "cross-app reference")
    for item in (receipt, command, outcome):
        require(item["work_id"] == work["work_id"], "cross-work reference")
    for item in (work, receipt, command):
        require(item["publication_id"] == pub["publication_id"], "publication pin mismatch")
    require(pub["definition_revision_id"] == definition["revision_id"] and pub["definition_digest"] == definition["digest"] == receipt["definition_digest"], "definition pin mismatch")
    content = definition["content"]
    inputs = {x["id"]: x for x in content["inputs"]}
    expected_inputs = {name: digest("input-contract", item) for name, item in inputs.items()}
    require({x["input_id"]: x["digest"] for x in pub["input_contract_digests"]} == expected_inputs, "publication input contract mismatch")
    schemas = {x["id"]: x for x in content["record_schemas"]}
    require({x["schema_id"]: x["digest"] for x in pub["record_schema_digests"]} == {name: digest("record-schema", item) for name, item in schemas.items()}, "publication record schema mismatch")
    collections = unique(work["record_collections"], "schema_id", "collection schema")
    require(collections.keys() == schemas.keys(), "work must have one collection per declared record schema")
    require(all(item["schema_version"] == schemas[name]["version"] for name, item in collections.items()), "collection schema version mismatch")
    bindings = {x["input_id"]: x for x in work["bindings"]}
    require(bindings.keys() == inputs.keys(), "work must bind each input")
    sources = unique(bundle["sources"], "revision_id", "source revision")
    required_sources = []
    for name, binding in bindings.items():
        require(binding["kind"] == inputs[name]["kind"] and binding["contract_digest"] == expected_inputs[name], "binding contract mismatch")
        if binding["kind"] == "source":
            source = sources.get(binding["source_revision_id"])
            require(source is not None, "source revision missing")
            require(source["source_id"] == binding["source_id"] and source["normalized_digest"] == binding["source_digest"], "source identity mismatch")
            require(digest("input-contract", source["input_contract"]) == expected_inputs[name], "source schema mismatch")
            require("deleted_at" not in source, "deleted source cannot satisfy current fixture")
            required_sources.append({k: binding[k] for k in ("input_id", "source_id", "source_revision_id", "source_digest")})
        else:
            require(any(c["id"] == binding["collection_id"] and c["schema_id"] == binding["schema_id"] and c["schema_version"] == binding["schema_version"] for c in work["record_collections"]), "record collection mismatch")
            require(binding["schema_id"] in schemas and schemas[binding["schema_id"]]["fields"] == inputs[name]["fields"], "record input schema mismatch")
    receipt_sources = [{k: v for k, v in item.items() if k != "mapping_version"} for item in receipt["source_revisions"]]
    require(work["source_dependencies"] == required_sources == receipt_sources, "source evidence dependency mismatch")
    views = {x["id"]: x for x in content["views"]}
    require(receipt["view_id"] in views and "query" in views[receipt["view_id"]], "receipt must name query view")
    require(receipt["provenance"]["query_digest"] == digest("query", views[receipt["view_id"]]["query"]), "receipt query mismatch")
    queries = [view["query"] for view in content["views"] if "query" in view]
    if definition["schema_version"] == 2:
        # An edition-two definition's publication and receipts carry the identity lists, mapping versions and partial.
        require(pub["schema_version"] == 2 and receipt["schema_version"] == 2, "edition-two definition requires edition-two publication and receipt")
    if pub["schema_version"] == 2:
        require(pub["operator_version"] == pub["renderer_version"] == REGISTRY_EDITION, "publication registry edition mismatch")
        require(pub["operators"] == _operator_identities(queries), "publication operator identities mismatch")
        renderers = [{"kind": kind, "version": REGISTRY_EDITION} for kind in sorted({view["kind"] for view in content["views"]})]
        require(pub["renderers"] == renderers, "publication renderer identities mismatch")
    if receipt["schema_version"] == 2:
        view = views[receipt["view_id"]]
        require(receipt["operator_version"] == REGISTRY_EDITION, "receipt registry edition mismatch")
        require(receipt["operators"] == _operator_identities([view["query"]]), "receipt operator identities mismatch")
        require(receipt["renderers"] == [{"kind": view["kind"], "version": REGISTRY_EDITION}], "receipt renderer identities mismatch")
        for item in receipt["source_revisions"]:
            require(item["mapping_version"] == sources[item["source_revision_id"]]["mapping_version"], "receipt mapping version mismatch")
        read = [sources[binding["source_revision_id"]] for name, binding in bindings.items() if binding["kind"] == "source" and name in view["query"]["inputs"]]
        partial = any(source.get("coverage") == "partial" for source in read)
        require(receipt["partial"] == partial, "receipt partial must equal the partial coverage of the inputs it reads")
    declaration = next((x for x in content["commands"] if x["id"] == command["command_id"]), None)
    require(declaration is not None and declaration["record_schema_id"] == command["schema_id"], "command is undeclared")
    # Edition impl-2026-10-29.3 amendment (APP-013 review M1): a create command expects the absent record, or an
    # existing version only when its declaration has transitions and the command carries a declared transition's
    # target (storage requires that transition to fire from the current value; no command writes a no-op revision).
    targets = {(t["field_id"], t["to"]) for t in declaration.get("transitions", [])}
    fires = any((item["field_id"], item["value"].get("value")) in targets for item in command["values"])
    existing = command["expected_record_version"] >= 1
    require(declaration["operation"] != "create" or not existing or fires,
            "create expects absent record, or an existing version along a declared transition")
    require(declaration["operation"] != "update" or existing, "update expects an existing version")
    fields = {x["id"]: x for x in schemas[command["schema_id"]]["fields"]}
    for item in command["values"]:
        require(item["field_id"] in declaration["writable_fields"], "command field is not writable")
        require(compatible(item["value"], fields[item["field_id"]]), "command value type/unit mismatch")
    if not existing and "initial" in declaration:
        state = {item["field_id"]: item["value"] for item in command["values"]}
        for field_id in dict.fromkeys(rule["field_id"] for rule in declaration["initial"]):
            chosen = next((rule["value"] for rule in declaration["initial"] if rule["field_id"] == field_id and ("when" not in rule or guard_holds(rule["when"], state))), None)
            require(chosen is not None and state.get(field_id) == {"type": "enum", "value": chosen}, "create must carry the declared initial value")
    require(command["expected_work_version"] == work["version"], "work version mismatch")
    require(outcome["payload_digest"] == payload_digest(command), "command payload mismatch")
    require(outcome["idempotency_key"] == command["idempotency_key"] and outcome["command_id"] == command["command_id"] and outcome["record_id"] == command["record_id"], "outcome command identity mismatch")
    require(outcome["record_version"] == command["expected_record_version"] + 1, "record version mismatch")
    require(outcome["work_version"] == work["version"], "outcome work version mismatch")
    require(outcome["principal_id"] == receipt["principal_id"] == context["principal_id"], "cross-principal reference")
    return bundle
