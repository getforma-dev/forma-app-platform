"""Regenerate the pitch's prebuilt app library (ADR-011, APP-094): typed definitions, synthetic data, catalog.

Everything under `fixtures/library/` is produced by this script and nothing else:

- `fixtures/library/data/<source>.csv`: labeled synthetic rows (seeded generator, no customer
  data) for every library source, in the CSV cell grammar `app/runtime/library_sources.py`
  normalizes (money as two-place dollars, timestamps as `YYYY-MM-DDTHH:MM:SSZ`).
- `fixtures/library/<app>.json`: one edition impl-2026-10-28.1 fixture bundle per library app
  (definition, publication, saved work, source revisions with the REAL `content_digest` and
  `normalized_digest` of the generated data, a receipt, the seeded decision command and its
  outcome), the shape `app/runtime/tenant_seed.py` seeds and `validate_bundle` qualifies. The
  seed rewrites the organization; every other identity is fixed so a repeat is idempotent.
- `fixtures/library/catalog.json`: the router sidecar the composer reads (per app: purpose,
  routing intent/negative examples and capabilities in the shape of forma-generative-ui's
  widget registry `routing` block, the fields, the views and the sources with their digests).

The definitions use only what the platform admits and the governed engine runs: the nine
renderer kinds, the qualified operators `filter`, `project`, `aggregate` (count/sum/min/max,
grouped), `sort`, `limit` and `time_bucket`, one source input per data view, a `time` timestamp
field wherever a window applies. Admission resolves `sort` keys against the input's declared
fields, so a grouped breakdown is ordered by its group key (the engine's GROUP BY order), never by
a measure; row tables sort only by projected fields. No HTML, widget artifact, SQL or grant
appears anywhere.

Run with the operator Python (DuckDB 2.0.0 and jsonschema):

    <python> contracts/make_library.py            # writes fixtures/library/
    <python> contracts/make_library.py --check    # refuses if the tree would change
"""
from __future__ import annotations

import copy
import datetime
import hashlib
import json
import re
from pathlib import Path
import random
import sys
import tempfile

ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parent
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(REPOSITORY / "app" / "runtime"))
from validate import canonical, digest, payload_digest, validate_bundle  # noqa: E402
import library_sources  # noqa: E402

LIBRARY = ROOT / "fixtures" / "library"
DATA = LIBRARY / "data"
# The Northstar fixture organization and its investigation author (tenant_seed.ORGANIZATIONS[0];
# the seed rewrites the organization onto its target and keeps every other identity).
ORGANIZATION = "00000000-0000-4000-8000-000000000001"
PRINCIPAL = "00000000-0000-4000-8000-000000000066"
OWNER_PUBLISH_POLICY = "00000000-0000-4000-8000-0000000000e0"
EDITION = "impl-2026-10-28.1"
# Library identities start past every retained fixture id (northstar 0x64.., spend review 0xc8..,
# the seed's fixed actors and APP-012's bound work 0xf2): app n occupies 0x1000 + 0x80 n.
BASE, STRIDE = 0x1000, 0x80
PUBLISHED_AT, SEEDED_AT = "2026-10-30T09:00:00Z", "2026-10-30T09:05:00Z"
DAY = datetime.datetime(2026, 10, 1)


def uid(n):
    return f"00000000-0000-4000-8000-{n:012x}"


def raw_hash(label):
    return "sha256:" + hashlib.sha256(label.encode()).hexdigest()


def field(name, kind, nullable=False, **extras):
    return {"id": name, "value_type": {"type": kind, **extras}, "nullable": nullable}


def money(name, nullable=False):
    return field(name, "money", nullable, currency="USD", minor_unit_exponent=2)


def enum(name, *values, nullable=False):
    return field(name, "enum", nullable, values=list(values))


def integer(name, unit, nullable=False):
    return field(name, "integer", nullable, unit=unit)


def f(input_id, field_id):
    return {"input_id": input_id, "field_id": field_id}


def query(inputs, operations):
    return {"operator_version": 1, "inputs": inputs, "operations": operations, "null_policy": "unknown"}


def view(name, kind, title, q=None, **extra):
    return {"id": name, "kind": kind, "renderer_version": 1, "title": title, **({"query": q} if q else {}), **extra}


def filt(src, name, comparison, literal):
    return {"type": "filter", "field": f(src, name), "comparison": comparison, "value": literal}


def lit_money(dollars):
    return {"type": "money", "minor_units": str(dollars * 100), "currency": "USD", "minor_unit_exponent": 2}


def lit_enum(value):
    return {"type": "enum", "value": value}


def lit_int(value, unit):
    return {"type": "integer", "value": str(value), "unit": unit}


def measure(id_, operation, src, name):
    return {"id": id_, "operation": operation, "field": f(src, name)}


def aggregate(src, groups, measures):
    return {"type": "aggregate", "group_by": [f(src, g) for g in groups], "measures": measures}


def sort(src, keys, tiebreak):
    return {"type": "sort", "fields": [{"field": f(src, k), "direction": d} for k, d in keys], "stable_tiebreak": f(src, tiebreak)}


def limit(rows):
    return {"type": "limit", "rows": rows}


def bucket(src, seconds):
    # The bucket replaces `time` in place (output_field = the field itself): admission resolves a
    # following aggregate's group_by against the input's declared fields, so a renamed bucket could
    # not be grouped. The engine's REPLACE keeps the column name `time`.
    return {"type": "time_bucket", "field": f(src, "time"), "seconds": seconds, "timezone": "UTC", "output_field": "time"}


def stamp(day, hour, minute, second=0):
    return (DAY + datetime.timedelta(days=day, hours=hour, minutes=minute, seconds=second)).strftime("%Y-%m-%dT%H:%M:%SZ")


def date(day):
    return (DAY + datetime.timedelta(days=day)).strftime("%Y-%m-%d")


def dollars(cents):
    return f"{cents // 100}.{cents % 100:02d}"


# --- synthetic data ---------------------------------------------------------------------------

PEOPLE = ["Maya Chen", "Jordan Alvarez", "Priya Natarajan", "Samuel Okafor", "Lena Fischer", "Diego Ramos", "Aisha Bello", "Tomás Herrera",
          "Grace Park", "Noah Williams", "Ingrid Solberg", "Kwame Mensah", "Elena Petrova", "Ravi Shankar", "Sofia Rossi", "Liam O'Connor"]
VENDORS = ["Cloudline Hosting", "Northwind Travel", "Atlas Office Supply", "Beacon Legal LLP", "Granite Facilities", "Lumen Software",
           "Harbor Catering", "Pinecrest Insurance", "Vertex Recruiting", "Summit Telecom", "Meridian Analytics", "Orchard Print"]
CATEGORIES = ["travel", "software", "equipment", "meals", "services", "facilities"]
SERVICES = ["checkout-api", "billing-worker", "identity", "search", "notifications", "warehouse-sync", "reporting", "mobile-gateway"]
SYSTEMS = ["finance-ledger", "payroll", "customer-crm", "source-control", "production-database", "hr-records", "data-warehouse"]
ACCOUNTS = ["Brightwater Foods", "Kestrel Logistics", "Oakridge Health", "Juniper Media", "Copperline Energy", "Ravenna Retail",
            "Silverthread Textiles", "Marlow Robotics", "Fathom Marine", "Tidewater Insurance", "Alder Pharmaceuticals", "Quill Education"]
PRODUCTS = ["Standing desk frame", "Monitor arm", "USB-C dock", "Laptop stand", "Noise-cancelling headset", "Ergonomic chair", "Webcam 4K",
            "Mechanical keyboard", "Conference speaker", "Label printer", "Cable organizer", "Desk lamp", "Whiteboard 6ft", "Barcode scanner"]
WAREHOUSES = ["austin", "reno", "newark"]
CUSTOMERS = ["Harlow & Sons", "Nimbus Studio", "Peregrine Clinics", "Cobalt Works", "Ashgrove Partners", "Riverbend Dental", "Solstice Fitness",
             "Trellis Gardens", "Vantage Legal", "Willowmere School", "Ironleaf Outdoors", "Bluefin Seafood", "Canopy Books", "Delta Bakers"]
DESTINATIONS = ["203.0.113.10", "203.0.113.20", "203.0.113.44", "198.51.100.7", "198.51.100.8", "198.51.100.99", "192.0.2.15", "192.0.2.77",
                "203.0.113.5", "198.51.100.23"]
ASSETS = ["build-worker-17", "ops-runner-04", "ci-agent-02", "data-sync-09", "web-edge-01", "backup-node-03"]
DEPARTMENTS = ["engineering", "sales", "marketing", "finance", "operations", "people", "support"]
ROLES = ["Backend engineer", "Account executive", "Product designer", "Data analyst", "Support specialist", "Security engineer"]


def generate_sources():
    """Every library source: (contract, rows as CSV text cells). Deterministic (seeded)."""
    rng = random.Random(20261102)
    pick = rng.choice

    def times(count, days=30):
        return sorted(stamp(rng.randrange(days), rng.randrange(7, 20), rng.randrange(60), rng.randrange(60)) for _ in range(count))

    sources = {}

    rows, moments = [], times(240)
    for n, t in enumerate(moments, 1):
        category = pick(CATEGORIES)
        cents = {"travel": rng.randrange(8500, 240000), "software": rng.randrange(1900, 120000), "equipment": rng.randrange(4900, 320000),
                 "meals": rng.randrange(1200, 18000), "services": rng.randrange(25000, 480000), "facilities": rng.randrange(9000, 150000)}[category]
        status = rng.choices(["submitted", "approved", "rejected", "needs_receipt"], [45, 35, 8, 12])[0]
        rows.append([f"EXP-{1000 + n}", t, pick(PEOPLE), pick(VENDORS), category, dollars(cents), status])
    sources["expense_claims"] = ({"id": "expense_claims", "contract_id": "expense_claims_v1", "contract_version": 1, "kind": "source", "fields": [
        field("expense_id", "text"), field("time", "timestamp"), field("employee", "text"), field("vendor", "text"),
        enum("category", *CATEGORIES), money("amount"), enum("status", "submitted", "approved", "rejected", "needs_receipt")]}, rows, "event")

    rows = []
    for n, t in enumerate(times(180, 60), 1):
        vendor = pick(VENDORS)
        cents = rng.randrange(12000, 980000)
        issued = datetime.datetime.strptime(t, "%Y-%m-%dT%H:%M:%SZ")
        due = (issued + datetime.timedelta(days=30)).strftime("%Y-%m-%d")
        rows.append([f"INV-{20260 + n}", t, vendor, pick(CATEGORIES), dollars(cents), due, "true" if rng.random() < 0.62 else "false"])
    sources["vendor_invoices"] = ({"id": "vendor_invoices", "contract_id": "vendor_invoices_v1", "contract_version": 1, "kind": "source", "fields": [
        field("invoice_id", "text"), field("time", "timestamp"), field("vendor", "text"), enum("category", *CATEGORIES), money("amount"),
        field("due", "date"), field("paid", "boolean")]}, rows, "event")

    rows = []
    for t in times(360, 1):
        destination = rng.choices(DESTINATIONS, [12, 10, 6, 14, 4, 3, 8, 5, 9, 6])[0]
        size = rng.choice([rng.randrange(1200, 90000), rng.randrange(60000, 900000), rng.randrange(900000, 4800000)])
        rows.append([t, pick(ASSETS), destination, str(size), rng.choices(["tcp", "udp", "icmp"], [80, 17, 3])[0]])
    sources["traffic"] = ({"id": "traffic", "contract_id": "traffic_v1", "contract_version": 1, "kind": "source", "fields": [
        field("time", "timestamp"), field("asset", "text"), field("destination", "text"), integer("bytes", "byte"), enum("protocol", "tcp", "udp", "icmp")]},
        rows, "event")

    rows = []
    for n, t in enumerate(times(96), 1):
        severity = rng.choices(["sev1", "sev2", "sev3", "sev4"], [6, 22, 42, 30])[0]
        status = rng.choices(["open", "mitigated", "resolved"], [18, 14, 68])[0]
        minutes = "" if status == "open" else str(rng.randrange(8, 60 if severity == "sev1" else 720))
        rows.append([f"INC-{2600 + n}", t, severity, pick(SERVICES), status, minutes])
    sources["incidents"] = ({"id": "incidents", "contract_id": "incidents_v1", "contract_version": 1, "kind": "source", "fields": [
        field("incident_id", "text"), field("time", "timestamp"), enum("severity", "sev1", "sev2", "sev3", "sev4"), field("service", "text"),
        enum("status", "open", "mitigated", "resolved"), integer("minutes_to_resolve", "minute", nullable=True)]}, rows, "event")

    rows = []
    for n, t in enumerate(times(150), 1):
        status = rng.choices(["pending", "approved", "denied"], [30, 58, 12])[0]
        rows.append([f"REQ-{4100 + n}", t, pick(PEOPLE), pick(SYSTEMS), rng.choices(["read", "write", "admin"], [55, 35, 10])[0], status,
                     "" if status == "pending" else pick(PEOPLE[:5])])
    sources["access_requests"] = ({"id": "access_requests", "contract_id": "access_requests_v1", "contract_version": 1, "kind": "source", "fields": [
        field("request_id", "text"), field("time", "timestamp"), field("requester", "text"), field("system", "text"),
        enum("access_level", "read", "write", "admin"), enum("status", "pending", "approved", "denied"), field("approver", "text", nullable=True)]},
        rows, "event")

    rows = []
    stages = ["lead", "qualified", "proposal", "negotiation", "closed_won", "closed_lost"]
    for n, t in enumerate(times(120, 90), 1):
        created = datetime.datetime.strptime(t, "%Y-%m-%dT%H:%M:%SZ")
        close = (created + datetime.timedelta(days=rng.randrange(20, 120))).strftime("%Y-%m-%d")
        rows.append([f"DEAL-{700 + n}", t, pick(ACCOUNTS), pick(PEOPLE[:8]), rng.choices(stages, [25, 22, 18, 12, 14, 9])[0],
                     dollars(rng.randrange(480000, 24000000) // 100 * 100), close])
    sources["deals"] = ({"id": "deals", "contract_id": "deals_v1", "contract_version": 1, "kind": "source", "fields": [
        field("deal_id", "text"), field("time", "timestamp"), field("account", "text"), field("owner", "text"), enum("stage", *stages),
        money("amount"), field("close", "date")]}, rows, "event")

    rows = []
    for n, product in enumerate(PRODUCTS, 1):
        cost = rng.randrange(1500, 65000)
        for warehouse in WAREHOUSES:
            rows.append([f"SKU-{3000 + n}-{warehouse[:2].upper()}", product, warehouse, str(rng.randrange(0, 420)), str(rng.choice([20, 40, 60, 100])), dollars(cost)])
    sources["inventory"] = ({"id": "inventory", "contract_id": "inventory_levels_v1", "contract_version": 1, "kind": "source", "fields": [
        field("sku", "text"), field("product", "text"), enum("warehouse", *WAREHOUSES), integer("on_hand", "each"), integer("reorder_point", "each"),
        money("unit_cost")]}, rows, "none")

    rows = []
    for n, t in enumerate(times(260), 1):
        priority = rng.choices(["low", "normal", "high", "urgent"], [22, 48, 22, 8])[0]
        rows.append([f"TCK-{8800 + n}", t, rng.choices(["email", "chat", "phone"], [50, 38, 12])[0], priority,
                     rng.choices(["open", "pending", "solved"], [24, 16, 60])[0], pick(CUSTOMERS), str(rng.randrange(2, 30 if priority == "urgent" else 480))])
    sources["tickets"] = ({"id": "tickets", "contract_id": "support_tickets_v1", "contract_version": 1, "kind": "source", "fields": [
        field("ticket_id", "text"), field("time", "timestamp"), enum("channel", "email", "chat", "phone"), enum("priority", "low", "normal", "high", "urgent"),
        enum("status", "open", "pending", "solved"), field("customer", "text"), integer("first_response_minutes", "minute")]}, rows, "event")

    rows = []
    for department in DEPARTMENTS:
        base = rng.randrange(4000000, 42000000)
        for month in ("2026-07-01", "2026-08-01", "2026-09-01", "2026-10-01"):
            budget = base + rng.randrange(-200000, 200000)
            actual = int(budget * rng.uniform(0.82, 1.18)) // 100 * 100
            rows.append([department, month, dollars(budget // 100 * 100), dollars(actual)])
    sources["budget_lines"] = ({"id": "budget_lines", "contract_id": "budget_lines_v1", "contract_version": 1, "kind": "source", "fields": [
        enum("department", *DEPARTMENTS), field("month", "date"), money("budget"), money("actual")]}, rows, "none")

    rows = []
    actions = ["login", "export", "grant", "revoke", "publish", "delete"]
    for t in times(320, 14):
        action = rng.choices(actions, [50, 14, 10, 6, 12, 8])[0]
        outcome = "denied" if rng.random() < (0.3 if action in ("export", "delete", "grant") else 0.05) else "success"
        rows.append([t, pick(PEOPLE), action, pick(SYSTEMS) if action != "publish" else pick(["spend-review", "incident-timeline", "sales-pipeline"]), outcome])
    sources["audit_log"] = ({"id": "audit_log", "contract_id": "audit_log_v1", "contract_version": 1, "kind": "source", "fields": [
        field("time", "timestamp"), field("actor", "text"), enum("action", *actions), field("resource", "text"), enum("outcome", "success", "denied")]},
        rows, "event")

    rows, balance = [], 48250000
    for n, t in enumerate(times(210, 60), 1):
        direction = rng.choices(["debit", "credit"], [68, 32])[0]
        cents = rng.randrange(1200, 1850000) if direction == "debit" else rng.randrange(50000, 6500000)
        balance += cents if direction == "credit" else -cents
        party = pick(VENDORS) if direction == "debit" else pick(ACCOUNTS)
        rows.append([f"TXN-{51000 + n}", t, ("Payment to " if direction == "debit" else "Receipt from ") + party, party, direction, dollars(cents), dollars(balance)])
    sources["transactions"] = ({"id": "transactions", "contract_id": "statement_transactions_v1", "contract_version": 1, "kind": "source", "fields": [
        field("transaction_id", "text"), field("time", "timestamp"), field("description", "text"), field("counterparty", "text"),
        enum("direction", "debit", "credit"), money("amount"), money("balance")]}, rows, "event")

    rows = []
    stages = ["applied", "screen", "onsite", "offer", "hired", "rejected"]
    for n, t in enumerate(times(170, 60), 1):
        rows.append([f"CAND-{300 + n}", t, pick(ROLES), rng.choices(stages, [30, 24, 16, 6, 6, 18])[0], rng.choices(["referral", "inbound", "agency"], [28, 58, 14])[0]])
    sources["candidates"] = ({"id": "candidates", "contract_id": "candidates_v1", "contract_version": 1, "kind": "source", "fields": [
        field("candidate_id", "text"), field("time", "timestamp"), field("role", "text"), enum("stage", *stages), enum("source", "referral", "inbound", "agency")]},
        rows, "event")

    rows = []
    for n, customer in enumerate(CUSTOMERS + ACCOUNTS + [p + " Co." for p in PEOPLE[:10]], 1):
        plan = rng.choices(["starter", "growth", "enterprise"], [50, 35, 15])[0]
        mrr = {"starter": rng.randrange(4900, 19900, 100), "growth": rng.randrange(24900, 89900, 100), "enterprise": rng.randrange(120000, 480000, 1000)}[plan]
        started = date(-rng.randrange(30, 700))
        rows.append([f"SUB-{600 + n}", customer, plan, dollars(mrr), started, rng.choices(["active", "churned", "trial"], [72, 16, 12])[0]])
    sources["subscriptions"] = ({"id": "subscriptions", "contract_id": "subscriptions_v1", "contract_version": 1, "kind": "source", "fields": [
        field("subscription_id", "text"), field("customer", "text"), enum("plan", "starter", "growth", "enterprise"), money("mrr"), field("started", "date"),
        enum("status", "active", "churned", "trial")]}, rows, "none")
    notes_source(sources)
    return sources


# APP-109: the notes app's history (the team's pinned notes). A generator of its own, so every source above keeps its exact bytes.
NOTE_TOPICS = ["planning", "follow_up", "decision", "reminder", "idea"]
NOTE_TITLES = ["Quarter planning dates", "Send the auditor the Q3 ledger", "Vendor renewal decided", "Offsite headcount", "Try weekly demo hour",
               "Close the books by the 5th", "Hiring loop feedback due", "Laptop refresh order", "Customer call recap", "Security review booked"]


def notes_source(sources):
    rng = random.Random(20261109)
    rows = []
    moments = sorted(stamp(rng.randrange(30), rng.randrange(7, 20), rng.randrange(60), rng.randrange(60)) for _ in range(30))
    for n, t in enumerate(moments, 1):
        rows.append([f"NOTE-{300 + n}", t, rng.choice(PEOPLE), rng.choice(NOTE_TITLES), rng.choice(NOTE_TOPICS)])
    sources["team_notes"] = ({"id": "team_notes", "contract_id": "team_notes_v1", "contract_version": 1, "kind": "source", "fields": [
        field("note_id", "text"), field("time", "timestamp"), field("author", "text"), field("title", "text"), enum("topic", *NOTE_TOPICS)]}, rows, "event")


# --- the apps ---------------------------------------------------------------------------------

DECISION_NOTE = "Reviewer action; synthetic only"
# The default records collection of a library app: one review decision per record.
DECISION_RECORDS = {
    "input_id": "decisions", "contract_id": "decisions_v1", "schema_id": "decision",
    "fields": [field("status", "enum", values=["needs_context", "reviewed"]), field("note", "text")],
    "commands": [{"id": "record_decision", "record_schema_id": "decision", "operation": "create", "writable_fields": ["status", "note"],
                  "transitions": [{"field_id": "status", "from": "needs_context", "to": "reviewed"}]}],
    "seed": {"command_id": "record_decision", "values": [{"field_id": "status", "value": {"type": "enum", "value": "needs_context"}},
                                                         {"field_id": "note", "value": {"type": "text", "value": DECISION_NOTE}}]},
}
# ADR-011 scenarios (APP-093/095): the expense claims app. One claim per record; `submit` creates it (the action
# path always sets status `submitted`), `decide` moves it along the two declared transitions only (storage's
# compose rules: a create on an existing record must fire a declared transition; `decide` writes status and note
# alone, so it can never create a claim). Amounts are USD cents; the seeded claim is an older, already approved one.
EXPENSE_CLAIM_FIELDS = [money("amount"), field("merchant", "text"), enum("category", "meals", "travel", "software", "office", "other"),
                        field("spent_on", "date"), field("note", "text", nullable=True), enum("status", "submitted", "approved", "rejected")]
EXPENSE_CLAIMS_RECORDS = {
    "input_id": "claims", "contract_id": "expense_claim_v1", "schema_id": "expense_claim",
    "fields": EXPENSE_CLAIM_FIELDS,
    "commands": [
        {"id": "submit", "record_schema_id": "expense_claim", "operation": "create",
         "writable_fields": ["amount", "merchant", "category", "spent_on", "note", "status"], "transitions": []},
        {"id": "decide", "record_schema_id": "expense_claim", "operation": "create", "writable_fields": ["status", "note"],
         "transitions": [{"field_id": "status", "from": "submitted", "to": "approved"}, {"field_id": "status", "from": "submitted", "to": "rejected"}]},
    ],
    "seed": {"command_id": "submit", "values": [
        {"field_id": "amount", "value": {"type": "money", "minor_units": "4250", "currency": "USD", "minor_unit_exponent": 2}},
        {"field_id": "merchant", "value": {"type": "text", "value": "Blue Bottle Coffee"}},
        {"field_id": "category", "value": {"type": "enum", "value": "meals"}},
        {"field_id": "spent_on", "value": {"type": "date", "value": "2026-10-21"}},
        {"field_id": "note", "value": {"type": "text", "value": "Team coffee; synthetic only"}},
        {"field_id": "status", "value": {"type": "enum", "value": "approved"}}]},
}

# APP-109: quick notes. One note per record (a title and a plain-text body, at most 2,000 characters each, storage's text
# bound); `add` creates it, `edit` rewrites its title and body (an `update`: no status, no transition). Each member writes
# notes in a saved work of their own (`/v1/notes`), so a note is private to its author unless the work is shared.
NOTES_RECORDS = {
    "input_id": "notes", "contract_id": "note_v1", "schema_id": "note",
    "fields": [field("title", "text"), field("body", "text", nullable=True)],
    "commands": [
        {"id": "add", "record_schema_id": "note", "operation": "create", "writable_fields": ["title", "body"], "transitions": []},
        {"id": "edit", "record_schema_id": "note", "operation": "update", "writable_fields": ["title", "body"], "transitions": []},
    ],
    "seed": {"command_id": "add", "values": [
        {"field_id": "title", "value": {"type": "text", "value": "Welcome to notes"}},
        {"field_id": "body", "value": {"type": "text", "value": "Type /note and a sentence to keep a note on your canvas; synthetic only."}}]},
}


# APP-101 slash commands: per library app its command slug and a short alias
# list (`/slug rest-of-text` opens the app instantly in the canvas intent bar;
# `apps` is reserved for the list). Lowercase, [a-z][a-z0-9-]{1,31}, unique
# across the library.
COMMANDS = {
    "expense-review": ("expense-review", ["review-expenses"]),
    "vendor-spend": ("vendor-spend", ["spend", "vendors"]),
    "outbound-traffic": ("traffic", ["egress", "network"]),
    "incident-timeline": ("incidents", ["incident"]),
    "access-review": ("access", ["access-review"]),
    "sales-pipeline": ("pipeline", ["sales", "deals"]),
    "inventory-comparison": ("inventory", ["stock"]),
    "support-queue": ("support", ["tickets"]),
    "budget-vs-actual": ("budget", ["variance"]),
    "audit-trail": ("audit", ["audit-log"]),
    "cash-flow": ("cash", ["cash-flow"]),
    "hiring-funnel": ("hiring", ["candidates"]),
    "subscription-revenue": ("mrr", ["subscriptions", "revenue"]),
    "expense-claims": ("expense", ["expenses", "claim"]),
    "notes": ("note", ["notes"]),
}


def routing(description, intents, negatives, capabilities):
    return {"description": description, "intent_examples": intents, "negative_examples": negatives, "capabilities": capabilities}


def apps():
    """(library_id, title, purpose, source input id, views, routing[, records spec]). One source input per app."""
    s = "expense_claims"
    yield ("expense-review", "Expense review queue", "Review submitted expense claims: the queue, the totals and spend by category, with a decision form.", s, [
        view("totals", "summary", "Claims and total amount", query([s], [aggregate(s, [], [measure("claims", "count", s, "expense_id"), measure("total_amount", "sum", s, "amount"), measure("largest_claim", "max", s, "amount")])])),
        view("queue", "table", "Submitted claims awaiting review", query([s], [filt(s, "status", "eq", lit_enum("submitted")), {"type": "project", "fields": [f(s, "expense_id"), f(s, "time"), f(s, "employee"), f(s, "vendor"), f(s, "category"), f(s, "amount")]}, sort(s, [("amount", "desc")], "expense_id"), limit(50)])),
        view("by_category", "chart", "Spend by category", query([s], [aggregate(s, ["category"], [measure("claims", "count", s, "expense_id"), measure("amount", "sum", s, "amount")])])),
        view("by_status", "status", "Claims by status", query([s], [aggregate(s, ["status"], [measure("claims", "count", s, "expense_id")])])),
        view("decision", "form", "Record a review decision", command_id="record_decision"),
        view("caution", "note", "Scope", text="Amounts are the submitted claim values in USD; a decision here records the review, it pays nothing."),
    ], routing("Expense claim review queue with totals, spend by category and a decision form over the organization's submitted claims.",
               ["expense review queue", "approve expenses", "which expense claims are waiting", "spend by category this month", "employee reimbursement review"],
               ["vendor invoices", "bank statement", "sales pipeline", "stock price", "file my taxes"],
               ["finance.expenses.review", "finance.spend.by_category", "records.decision"]))

    s = "vendor_invoices"
    yield ("vendor-spend", "Vendor spend breakdown", "Spend by vendor and category from issued invoices, the unpaid invoices by due date, and a decision form.", s, [
        view("totals", "summary", "Invoices and total spend", query([s], [aggregate(s, [], [measure("invoices", "count", s, "invoice_id"), measure("total_spend", "sum", s, "amount"), measure("largest_invoice", "max", s, "amount")])])),
        view("by_vendor", "chart", "Spend by vendor", query([s], [aggregate(s, ["vendor"], [measure("invoices", "count", s, "invoice_id"), measure("spend", "sum", s, "amount")])])),
        view("by_category", "chart", "Spend by category", query([s], [aggregate(s, ["category"], [measure("spend", "sum", s, "amount")])])),
        view("unpaid", "table", "Unpaid invoices by due date", query([s], [filt(s, "paid", "eq", {"type": "boolean", "value": False}), {"type": "project", "fields": [f(s, "invoice_id"), f(s, "vendor"), f(s, "amount"), f(s, "due")]}, sort(s, [("due", "asc")], "invoice_id"), limit(50)])),
        view("decision", "form", "Record a vendor review decision", command_id="record_decision"),
    ], routing("Vendor spend breakdown: invoice totals, the top vendors and categories, and unpaid invoices ordered by due date.",
               ["vendor spend breakdown", "who are we paying the most", "unpaid invoices", "accounts payable by vendor", "supplier spend report"],
               ["employee expenses", "bank balance", "payroll", "stock chart", "customer revenue"],
               ["finance.vendors.spend", "finance.invoices.unpaid", "records.decision"]))

    s = "traffic"
    yield ("outbound-traffic", "Outbound traffic investigation", "Northstar's Security case: outbound bytes by destination and hour for the organization's assets, the largest transfers, and an observation form.", s, [
        view("total", "summary", "Total outbound bytes", query([s], [aggregate(s, [], [measure("connections", "count", s, "destination"), measure("total_bytes", "sum", s, "bytes"), measure("largest_transfer", "max", s, "bytes")])])),
        view("by_destination", "chart", "Bytes by destination", query([s], [aggregate(s, ["destination"], [measure("connections", "count", s, "destination"), measure("bytes", "sum", s, "bytes")])])),
        view("timeline", "timeline", "Bytes per hour", query([s], [bucket(s, 3600), aggregate(s, ["time"], [measure("bytes", "sum", s, "bytes")])])),
        view("largest", "table", "Largest transfers", query([s], [{"type": "project", "fields": [f(s, "time"), f(s, "asset"), f(s, "destination"), f(s, "protocol"), f(s, "bytes")]}, sort(s, [("bytes", "desc")], "time"), limit(25)])),
        view("record_observation", "form", "Record an observation", command_id="record_decision"),
        view("caution", "note", "Caution", text="An unmatched destination does not establish compromise."),
    ], routing("Outbound network traffic investigation: total bytes, bytes by destination and per hour, and the largest transfers for an asset and window.",
               ["outbound traffic investigation", "which destinations received the most bytes", "network egress by hour", "suspicious data transfer", "bytes sent by build-worker-17"],
               ["website analytics", "expense report", "firewall configuration", "block an ip address", "weather"],
               ["security.traffic.investigation", "security.traffic.timeline", "records.observation"]))

    s = "incidents"
    yield ("incident-timeline", "Incident timeline", "Incidents opened per day, by severity and service, the ones still open, and a review form.", s, [
        view("timeline", "timeline", "Incidents opened per day", query([s], [bucket(s, 86400), aggregate(s, ["time"], [measure("incidents", "count", s, "incident_id")])])),
        view("by_severity", "chart", "Incidents by severity", query([s], [aggregate(s, ["severity"], [measure("incidents", "count", s, "incident_id"), measure("longest_minutes", "max", s, "minutes_to_resolve")])])),
        view("open", "table", "Open and mitigated incidents", query([s], [filt(s, "status", "ne", lit_enum("resolved")), {"type": "project", "fields": [f(s, "incident_id"), f(s, "time"), f(s, "severity"), f(s, "service"), f(s, "status")]}, sort(s, [("severity", "asc"), ("time", "desc")], "incident_id"), limit(50)])),
        view("totals", "summary", "Incident totals", query([s], [aggregate(s, [], [measure("incidents", "count", s, "incident_id"), measure("resolved", "count", s, "minutes_to_resolve"), measure("longest_minutes", "max", s, "minutes_to_resolve"), measure("fastest_minutes", "min", s, "minutes_to_resolve")])])),
        view("review", "form", "Record an incident review", command_id="record_decision"),
    ], routing("Incident timeline: incidents opened per day, counts by severity, the open ones, and resolution-time extremes.",
               ["incident timeline", "how many incidents this week", "open sev1 incidents", "outage history by service", "postmortem review queue"],
               ["support tickets", "access requests", "status page design", "pager rotation", "server uptime graph"],
               ["ops.incidents.timeline", "ops.incidents.open", "records.decision"]))

    s = "access_requests"
    yield ("access-review", "Access review tracker", "Pending access requests by system and level, admin requests to scrutinize, and an approval decision form.", s, [
        view("pending", "table", "Pending requests", query([s], [filt(s, "status", "eq", lit_enum("pending")), {"type": "project", "fields": [f(s, "request_id"), f(s, "time"), f(s, "requester"), f(s, "system"), f(s, "access_level")]}, sort(s, [("time", "asc")], "request_id"), limit(50)])),
        view("by_system", "chart", "Requests by system", query([s], [aggregate(s, ["system"], [measure("requests", "count", s, "request_id")])])),
        view("admin_requests", "summary", "Admin-level requests", query([s], [filt(s, "access_level", "eq", lit_enum("admin")), aggregate(s, [], [measure("admin_requests", "count", s, "request_id"), measure("oldest", "min", s, "time")])])),
        view("by_status", "status", "Requests by status", query([s], [aggregate(s, ["status"], [measure("requests", "count", s, "request_id")])])),
        view("approval", "form", "Record an approval decision", command_id="record_decision"),
    ], routing("Access review tracker: pending access requests, requests by system and status, admin-level requests, with an approval decision form.",
               ["access review", "pending access requests", "who asked for admin access", "approvals tracker", "permission requests by system"],
               ["password reset", "login page", "incident list", "expense approvals", "grant access now"],
               ["security.access.review", "security.access.pending", "records.decision"]))

    s = "deals"
    yield ("sales-pipeline", "Sales pipeline by stage", "Pipeline value and deal count by stage and owner, the largest open deals, and a review form.", s, [
        view("by_stage", "chart", "Pipeline by stage", query([s], [aggregate(s, ["stage"], [measure("deals", "count", s, "deal_id"), measure("value", "sum", s, "amount")])])),
        view("by_owner", "chart", "Pipeline by owner", query([s], [aggregate(s, ["owner"], [measure("deals", "count", s, "deal_id"), measure("value", "sum", s, "amount")])])),
        view("largest", "table", "Largest deals in negotiation", query([s], [filt(s, "stage", "eq", lit_enum("negotiation")), {"type": "project", "fields": [f(s, "deal_id"), f(s, "account"), f(s, "owner"), f(s, "amount"), f(s, "close")]}, sort(s, [("amount", "desc")], "deal_id"), limit(20)])),
        view("totals", "summary", "Pipeline totals", query([s], [aggregate(s, [], [measure("deals", "count", s, "deal_id"), measure("pipeline_value", "sum", s, "amount"), measure("largest_deal", "max", s, "amount")])])),
        view("review", "form", "Record a deal review", command_id="record_decision"),
    ], routing("Sales pipeline by stage and owner: deal counts, pipeline value, the largest deals in negotiation and the totals.",
               ["sales pipeline by stage", "how much is in negotiation", "pipeline value by rep", "biggest open deals", "forecast review"],
               ["customer support tickets", "invoice payments", "marketing campaign", "stock market", "hiring pipeline"],
               ["sales.pipeline.by_stage", "sales.pipeline.largest", "records.decision"]))

    s = "inventory"
    yield ("inventory-comparison", "Inventory comparison", "Units on hand by warehouse and product, the low-stock SKUs, and a reorder decision form.", s, [
        view("by_warehouse", "chart", "Units on hand by warehouse", query([s], [aggregate(s, ["warehouse"], [measure("skus", "count", s, "sku"), measure("units", "sum", s, "on_hand")])])),
        view("by_product", "chart", "Units on hand by product", query([s], [aggregate(s, ["product"], [measure("units", "sum", s, "on_hand"), measure("lowest_warehouse", "min", s, "on_hand")])])),
        view("low_stock", "table", "SKUs under 25 units", query([s], [filt(s, "on_hand", "lt", lit_int(25, "each")), {"type": "project", "fields": [f(s, "sku"), f(s, "product"), f(s, "warehouse"), f(s, "on_hand"), f(s, "reorder_point"), f(s, "unit_cost")]}, sort(s, [("on_hand", "asc")], "sku"), limit(50)])),
        view("totals", "summary", "Inventory totals", query([s], [aggregate(s, [], [measure("skus", "count", s, "sku"), measure("units", "sum", s, "on_hand"), measure("highest_unit_cost", "max", s, "unit_cost")])])),
        view("reorder", "form", "Record a reorder decision", command_id="record_decision"),
    ], routing("Inventory comparison across warehouses: units on hand by warehouse and product, SKUs below a stock threshold and the totals.",
               ["inventory comparison", "compare stock across warehouses", "what is running low", "units on hand by product", "reorder list"],
               ["sales pipeline", "shipping rates", "supplier invoices", "product catalog design", "price list"],
               ["operations.inventory.compare", "operations.inventory.low_stock", "records.decision"]))

    s = "tickets"
    yield ("support-queue", "Support ticket queue", "Open tickets by priority, tickets per day and channel, first-response times, and a triage decision form.", s, [
        view("open", "table", "Open tickets by priority", query([s], [filt(s, "status", "eq", lit_enum("open")), {"type": "project", "fields": [f(s, "ticket_id"), f(s, "time"), f(s, "priority"), f(s, "channel"), f(s, "customer"), f(s, "first_response_minutes")]}, sort(s, [("priority", "desc"), ("time", "asc")], "ticket_id"), limit(50)])),
        view("by_channel", "chart", "Tickets by channel", query([s], [aggregate(s, ["channel"], [measure("tickets", "count", s, "ticket_id"), measure("slowest_first_response", "max", s, "first_response_minutes")])])),
        view("timeline", "timeline", "Tickets opened per day", query([s], [bucket(s, 86400), aggregate(s, ["time"], [measure("tickets", "count", s, "ticket_id")])])),
        view("totals", "summary", "Queue totals", query([s], [aggregate(s, [], [measure("tickets", "count", s, "ticket_id"), measure("slowest_first_response", "max", s, "first_response_minutes"), measure("fastest_first_response", "min", s, "first_response_minutes")])])),
        view("triage", "form", "Record a triage decision", command_id="record_decision"),
    ], routing("Support ticket queue: open tickets by priority, tickets by channel and per day, and first-response extremes.",
               ["support ticket queue", "open urgent tickets", "tickets by channel", "how many tickets came in today", "customer support backlog"],
               ["incident postmortem", "sales deals", "knowledge base article", "chat with a customer", "refund a payment"],
               ["support.tickets.queue", "support.tickets.timeline", "records.decision"]))

    s = "budget_lines"
    yield ("budget-vs-actual", "Budget vs actual", "Budget and actual spend by department and month, the totals, and a variance review form.", s, [
        view("by_department", "chart", "Budget and actual by department", query([s], [aggregate(s, ["department"], [measure("budget", "sum", s, "budget"), measure("actual", "sum", s, "actual")])])),
        view("by_month", "chart", "Budget and actual by month", query([s], [aggregate(s, ["month"], [measure("budget", "sum", s, "budget"), measure("actual", "sum", s, "actual")])])),
        view("lines", "table", "Budget lines by actual spend", query([s], [{"type": "project", "fields": [f(s, "department"), f(s, "month"), f(s, "budget"), f(s, "actual")]}, sort(s, [("actual", "desc")], "department"), limit(50)])),
        view("totals", "summary", "Totals", query([s], [aggregate(s, [], [measure("lines", "count", s, "department"), measure("budget", "sum", s, "budget"), measure("actual", "sum", s, "actual")])])),
        view("variance_review", "form", "Record a variance review", command_id="record_decision"),
    ], routing("Budget versus actual spend by department and month with totals and a variance review form.",
               ["budget vs actual", "which departments are over budget", "spend against plan by month", "variance report", "department budget review"],
               ["expense claims", "bank statement", "stock prices", "set a budget", "forecast next year"],
               ["finance.budget.variance", "finance.budget.by_department", "records.decision"]))

    s = "audit_log"
    yield ("audit-trail", "Audit trail viewer", "Recent audit events, events by action and outcome, denied actions, and a review form.", s, [
        view("recent", "table", "Recent events", query([s], [{"type": "project", "fields": [f(s, "time"), f(s, "actor"), f(s, "action"), f(s, "resource"), f(s, "outcome")]}, sort(s, [("time", "desc")], "actor"), limit(50)])),
        view("by_action", "chart", "Events by action", query([s], [aggregate(s, ["action"], [measure("events", "count", s, "actor")])])),
        view("denied", "summary", "Denied actions", query([s], [filt(s, "outcome", "eq", lit_enum("denied")), aggregate(s, [], [measure("denied", "count", s, "actor"), measure("latest", "max", s, "time")])])),
        view("timeline", "timeline", "Events per hour", query([s], [bucket(s, 3600), aggregate(s, ["time"], [measure("events", "count", s, "actor")])])),
        view("review", "form", "Record an audit review", command_id="record_decision"),
    ], routing("Audit trail viewer: recent audit events, counts by action, denied actions and events per hour.",
               ["audit trail", "who exported data", "denied actions last week", "audit log viewer", "recent admin activity"],
               ["server logs", "incident timeline", "access request approvals", "delete the audit log", "error monitoring"],
               ["security.audit.trail", "security.audit.denied", "records.decision"]))

    s = "transactions"
    yield ("cash-flow", "Cash-flow statement", "Money in and money out from the statement, the daily cash-flow timeline, recent activity, and a reconciliation note form.", s, [
        view("by_direction", "chart", "Money in and money out", query([s], [aggregate(s, ["direction"], [measure("transactions", "count", s, "transaction_id"), measure("total", "sum", s, "amount")])])),
        view("timeline", "timeline", "Daily cash flow", query([s], [bucket(s, 86400), aggregate(s, ["time", "direction"], [measure("total", "sum", s, "amount")])])),
        view("recent", "table", "Recent activity", query([s], [{"type": "project", "fields": [f(s, "time"), f(s, "description"), f(s, "direction"), f(s, "amount"), f(s, "balance")]}, sort(s, [("time", "desc")], "description"), limit(25)])),
        view("largest_debits", "table", "Largest payments", query([s], [filt(s, "direction", "eq", lit_enum("debit")), {"type": "project", "fields": [f(s, "time"), f(s, "counterparty"), f(s, "amount")]}, sort(s, [("amount", "desc")], "time"), limit(10)])),
        view("reconcile", "form", "Record a reconciliation note", command_id="record_decision"),
    ], routing("Cash-flow statement from bank transactions: money in versus money out, a daily timeline, recent activity and the largest payments.",
               ["cash flow statement", "money in and money out", "bank statement activity", "daily spending trend", "largest payments this month"],
               ["stock price", "currency conversion", "pay an invoice", "crypto chart", "vendor contracts"],
               ["finance.cashflow.summary", "finance.cashflow.timeline", "records.decision"]))

    s = "candidates"
    yield ("hiring-funnel", "Hiring funnel", "Candidates by stage, role and source, the recent applicants, and a screening decision form.", s, [
        view("by_stage", "chart", "Candidates by stage", query([s], [aggregate(s, ["stage"], [measure("candidates", "count", s, "candidate_id")])])),
        view("by_source", "chart", "Candidates by source", query([s], [aggregate(s, ["source"], [measure("candidates", "count", s, "candidate_id")])])),
        view("by_role", "chart", "Candidates by role", query([s], [aggregate(s, ["role"], [measure("candidates", "count", s, "candidate_id"), measure("latest_application", "max", s, "time")])])),
        view("recent", "table", "Recent applicants", query([s], [{"type": "project", "fields": [f(s, "candidate_id"), f(s, "time"), f(s, "role"), f(s, "stage"), f(s, "source")]}, sort(s, [("time", "desc")], "candidate_id"), limit(50)])),
        view("screening", "form", "Record a screening decision", command_id="record_decision"),
    ], routing("Hiring funnel: candidates by stage, source and role, and the most recent applicants.",
               ["hiring funnel", "recruiting pipeline", "how many candidates are at onsite", "applicants by source", "open roles pipeline"],
               ["sales pipeline", "employee directory", "payroll", "post a job ad", "interview scheduling"],
               ["people.hiring.funnel", "people.hiring.recent", "records.decision"]))

    s = "subscriptions"
    yield ("subscription-revenue", "Subscription revenue", "Monthly recurring revenue by plan and status, the active subscriptions, the top customers, and a review form.", s, [
        view("by_plan", "chart", "MRR by plan", query([s], [aggregate(s, ["plan"], [measure("subscriptions", "count", s, "subscription_id"), measure("mrr", "sum", s, "mrr")])])),
        view("by_status", "status", "Subscriptions by status", query([s], [aggregate(s, ["status"], [measure("subscriptions", "count", s, "subscription_id"), measure("mrr", "sum", s, "mrr")])])),
        view("active", "summary", "Active MRR", query([s], [filt(s, "status", "eq", lit_enum("active")), aggregate(s, [], [measure("active_subscriptions", "count", s, "subscription_id"), measure("active_mrr", "sum", s, "mrr"), measure("earliest_start", "min", s, "started")])])),
        view("top_customers", "table", "Top customers by MRR", query([s], [filt(s, "status", "ne", lit_enum("churned")), {"type": "project", "fields": [f(s, "customer"), f(s, "plan"), f(s, "mrr"), f(s, "started"), f(s, "status")]}, sort(s, [("mrr", "desc")], "customer"), limit(20)])),
        view("review", "form", "Record an account review", command_id="record_decision"),
    ], routing("Subscription revenue: MRR by plan and status, active MRR and the top customers by recurring revenue.",
               ["subscription revenue", "mrr by plan", "how much recurring revenue is active", "top customers by mrr", "churned accounts"],
               ["one-time invoices", "stock price", "payment gateway setup", "refund a customer", "marketing funnel"],
               ["finance.revenue.mrr", "finance.revenue.top_customers", "records.decision"]))

    # ADR-011 scenarios: the expense claims app the action path (`POST /v1/act`) files into. Its live claims are
    # the work's records (read by `GET /v1/work/{app}/{work}/records`); the two data views give the historical
    # context from the seeded claims source so the app never starts empty.
    s = "expense_claims"
    yield ("expense-claims", "Expense claims", "File an expense in your own words, send it to your manager for approval, and watch the decision land live: every claim is a typed record, every decision an audited command.", s, [
        view("totals", "summary", "Claims to date", query([s], [aggregate(s, [], [measure("claims", "count", s, "expense_id"), measure("total_amount", "sum", s, "amount"), measure("largest_claim", "max", s, "amount")])])),
        view("by_status", "status", "Past claims by status", query([s], [aggregate(s, ["status"], [measure("claims", "count", s, "expense_id"), measure("amount", "sum", s, "amount")])])),
        view("recent", "table", "Recent past claims", query([s], [{"type": "project", "fields": [f(s, "expense_id"), f(s, "time"), f(s, "employee"), f(s, "vendor"), f(s, "category"), f(s, "amount"), f(s, "status")]}, sort(s, [("time", "desc")], "expense_id"), limit(25)])),
        view("submit", "form", "File an expense claim", command_id="submit"),
        view("how", "note", "How approval works", text="A claim is filed as a typed record with status submitted. Your approver sees it live and decides; the decision is an audited command, never an edit."),
    ], routing("Expense claims with manager approval: file a claim (amount, merchant, category, date, note), share it with an approver, approve or reject it; the claims list updates live.",
               ["I spent $86 on a client dinner at Nobu, Sarah approves my expenses", "file an expense", "submit a reimbursement for my flight", "approve the Nobu dinner", "what is waiting for my approval"],
               ["vendor invoices", "payroll", "bank statement", "budget vs actual", "sales pipeline"],
               ["finance.expenses.claim", "finance.expenses.approve", "records.create", "records.transition"]),
           EXPENSE_CLAIMS_RECORDS)

    # APP-109: quick notes. The canvas shows each note as its own card (`/note`); the views give the team's pinned notes.
    s = "team_notes"
    yield ("notes", "Notes", "Quick notes: type /note and a sentence to keep a note as a card on your canvas; each note is a typed record of your own.", s, [
        view("count", "summary", "Team notes", query([s], [aggregate(s, [], [measure("notes", "count", s, "note_id")])])),
        view("by_topic", "status", "Team notes by topic", query([s], [aggregate(s, ["topic"], [measure("notes", "count", s, "note_id")])])),
        view("pinned", "table", "Recent team notes", query([s], [{"type": "project", "fields": [f(s, "note_id"), f(s, "time"), f(s, "author"), f(s, "title"), f(s, "topic")]}, sort(s, [("time", "desc")], "note_id"), limit(15)])),
        view("add", "form", "New note", command_id="add"),
    ], routing("Quick personal notes: write a short note with a title and a body and keep it as a card on the canvas.",
               ["take a note", "note to self", "remember to call the auditor Friday", "jot this down"],
               ["expense claims", "approve the Nobu dinner", "sales pipeline", "incident timeline", "budget vs actual", "outbound traffic"],
               ["utilities.notes", "records.create"]),
           NOTES_RECORDS)


def csv_bytes(contract, rows):
    import csv
    import io
    out = io.StringIO(newline="")
    writer = csv.writer(out, lineterminator="\n")
    writer.writerow([fld["id"] for fld in contract["fields"]])
    writer.writerows(rows)
    return out.getvalue().encode("utf-8")


def source_revision(contract, data, normalized, count, source_id, revision_id, time_semantics):
    return {"kind": "source_revision", "schema_version": 1, "organization_id": ORGANIZATION, "source_id": source_id, "revision_id": revision_id,
            "content_digest": library_sources.sha256(data), "normalized_digest": normalized, "input_contract": copy.deepcopy(contract),
            "parser_version": 1, "mapping_version": 1, "format": "csv", "formula_policy": "reject", "row_count": count, "timezone": "UTC",
            "time_semantics": time_semantics, "units_confirmed": True, "created_at": PUBLISHED_AT,
            "provenance_sample": [{"type": "csv", "logical_record": 1, "physical_line_start": 2, "physical_line_end": 2}]}


def bundle(index, library_id, title, purpose, source, views, revision, records_spec=None):
    """One bundle. `records_spec` (ADR-011 scenarios, APP-093/095) replaces the default decision schema with a
    typed records collection of its own: {input_id, contract_id, schema_id, fields, commands, seed: {command_id, values}}."""
    offset = BASE + STRIDE * index
    app, definition_revision, publication, work, collection, record, audit = (uid(offset + n) for n in range(1, 8))
    spec = records_spec or DECISION_RECORDS
    decisions = spec["fields"]
    records = {"id": spec["schema_id"], "version": 1, "fields": decisions}
    inputs = [copy.deepcopy(source), {"id": spec["input_id"], "contract_id": spec["contract_id"], "contract_version": 1, "kind": "records", "fields": decisions}]
    layout = []
    for i, v in enumerate(views):
        layout.append({"view_id": v["id"], "column": 0, "row": i, "width": 12})
    content = {"title": title, "description": purpose + " Prebuilt library app on labeled synthetic data (ADR-011 pitch).",
               "inputs": inputs, "views": views, "record_schemas": [records],
               "commands": copy.deepcopy(spec["commands"]),
               "links": [], "default_layout": layout}
    definition = {"kind": "app_definition", "schema_version": 1, "organization_id": ORGANIZATION, "app_id": app, "revision_id": definition_revision,
                  "content": content, "digest": digest("app-definition", content)}
    pub = {"kind": "publication", "schema_version": 1, "organization_id": ORGANIZATION, "app_id": app, "publication_id": publication,
           "definition_revision_id": definition_revision, "definition_digest": definition["digest"],
           "input_contract_digests": [{"input_id": i["id"], "digest": digest("input-contract", i)} for i in inputs],
           "record_schema_digests": [{"schema_id": records["id"], "digest": digest("record-schema", records)}],
           "renderer_version": 1, "operator_version": 1, "admission_digest": raw_hash(library_id + ":library-admission-placeholder"),
           "policy": {"id": OWNER_PUBLISH_POLICY, "version": 1, "kind": "owner_publish"}, "author_id": PRINCIPAL, "approver_id": PRINCIPAL,
           "approver_epoch": 1, "published_at": PUBLISHED_AT}
    dependency = {"input_id": source["id"], "source_id": revision["source_id"], "source_revision_id": revision["revision_id"],
                  "source_digest": revision["normalized_digest"]}
    saved_work = {"kind": "saved_work", "schema_version": 1, "organization_id": ORGANIZATION, "app_id": app, "work_id": work, "publication_id": publication,
                  "version": 1, "context": "Prebuilt library: " + title,
                  "bindings": [{**dependency, "kind": "source", "contract_digest": digest("input-contract", inputs[0])},
                               {"input_id": spec["input_id"], "kind": "records", "contract_digest": digest("input-contract", inputs[1]), "collection_id": collection,
                                "schema_id": records["id"], "schema_version": 1, "revision_policy": "committed_snapshot"}],
                  "record_collections": [{"id": collection, "schema_id": records["id"], "schema_version": 1}], "source_dependencies": [dependency]}
    first = next(v for v in views if "query" in v)
    receipt = {"kind": "query_receipt", "schema_version": 1, "organization_id": ORGANIZATION, "app_id": app, "work_id": work, "mode": "published",
               "definition_digest": definition["digest"], "publication_id": publication, "view_id": first["id"], "operator_version": 1,
               "source_revisions": [dependency], "record_snapshot_sequence": 0, "parameters": [], "principal_id": PRINCIPAL, "membership_epoch": 1,
               "policy_epoch": 1, "filter_epoch": 0, "result_digest": digest("result", {"example": library_id}), "partial": False, "truncated_page": False,
               "total_scope": "full_authorized_filtered_input",
               "provenance": {"query_digest": digest("query", first["query"]), "predicate_digest": digest("query", first["query"])}}
    receipt["digest"] = digest("query-receipt", receipt)
    command = {"kind": "command_request", "schema_version": 1, "app_id": app, "work_id": work, "publication_id": publication, "command_id": spec["seed"]["command_id"],
               "schema_id": records["id"], "schema_version_expected": 1, "record_id": record, "expected_record_version": 0, "expected_work_version": 1,
               "values": copy.deepcopy(spec["seed"]["values"]),
               "idempotency_key": f"{offset:032x}"}
    outcome = {"kind": "command_outcome", "schema_version": 1, "organization_id": ORGANIZATION, "app_id": app, "work_id": work, "principal_id": PRINCIPAL,
               "command_id": command["command_id"], "idempotency_key": command["idempotency_key"], "payload_digest": payload_digest(command), "state": "committed",
               "record_id": record, "record_version": 1, "work_version": 1, "commit_sequence": 1, "audit_event_id": audit, "committed_at": SEEDED_AT,
               "retry_expires_at": "2026-10-31T09:05:00Z"}
    return {"context": {"organization_id": ORGANIZATION, "principal_id": PRINCIPAL}, "definition": definition, "publication": pub, "work": saved_work,
            "sources": [revision], "receipt": receipt, "command": command, "outcome": outcome}


def field_summary(fld):
    summary = {"id": fld["id"], "type": fld["value_type"]["type"], "nullable": fld.get("nullable", False)}
    for key in ("unit", "values", "currency", "scale"):
        if key in fld["value_type"]:
            summary[key] = fld["value_type"][key]
    return summary


def catalog_entry(library_id, title, purpose, route, b, csv_name):
    content = b["definition"]["content"]
    source = content["inputs"][0]
    revision = b["sources"][0]
    views = []
    for v in content["views"]:
        entry = {"id": v["id"], "kind": v["kind"], "title": v["title"]}
        if "query" in v:
            entry["input"] = v["query"]["inputs"][0]
            entry["operators"] = sorted({op["type"] for op in v["query"]["operations"]})
            fields = {fld["id"]: fld for fld in source["fields"]}
            parameters = []
            if fields.get("time", {}).get("value_type", {}).get("type") == "timestamp":
                parameters.append("window")
            if fields.get("destination", {}).get("value_type", {}).get("type") == "text":
                parameters.append("destination")
            entry["parameters"] = parameters
            entry["runnable"] = True
        else:
            entry["runnable"] = False
            if "command_id" in v:
                entry["command_id"] = v["command_id"]
        views.append(entry)
    slug, aliases = COMMANDS[library_id]
    return {"library_id": library_id, "fixture": library_id + ".json", "title": title, "purpose": purpose, "routing": route,
            "command": {"slug": slug, "aliases": aliases},
            "app_id": b["definition"]["app_id"], "definition_revision_id": b["definition"]["revision_id"], "definition_digest": b["definition"]["digest"],
            "publication_id": b["publication"]["publication_id"], "work_id": b["work"]["work_id"], "collection_id": b["work"]["record_collections"][0]["id"],
            "inputs": [{"id": i["id"], "kind": i["kind"], "contract_id": i["contract_id"], "fields": [field_summary(fld) for fld in i["fields"]]} for i in content["inputs"]],
            "views": views,
            "commands": [{"id": c["id"], "record_schema_id": c["record_schema_id"], "operation": c["operation"], "writable_fields": c["writable_fields"],
                          "transitions": copy.deepcopy(c["transitions"])} for c in content["commands"]],
            "record_schemas": [{"id": r["id"], "version": r["version"], "fields": [field_summary(fld) for fld in r["fields"]]} for r in content["record_schemas"]],
            "sources": [{"input_id": source["id"], "source_id": revision["source_id"], "revision_id": revision["revision_id"], "csv": "data/" + csv_name,
                         "content_digest": revision["content_digest"], "normalized_digest": revision["normalized_digest"], "row_count": revision["row_count"],
                         "input_contract": copy.deepcopy(source)}]}


def build():
    """Every file of fixtures/library/, as (relative path, bytes)."""
    sources = generate_sources()
    files = {}
    revisions = {}
    with tempfile.TemporaryDirectory(prefix="library-") as scratch:
        for n, (name, (contract, rows, semantics)) in enumerate(sources.items()):
            data = csv_bytes(contract, rows)
            files["data/" + name + ".csv"] = data
            normalized, count = library_sources.normalize(contract, data, Path(scratch) / (name + ".parquet"))
            # Source identities: one pair per source, past every app's identity block.
            source_id, revision_id = uid(BASE + STRIDE * 64 + 2 * n), uid(BASE + STRIDE * 64 + 2 * n + 1)
            revisions[name] = source_revision(contract, data, normalized, count, source_id, revision_id, semantics)
    names = [name for slug, aliases in COMMANDS.values() for name in [slug, *aliases]]
    assert len(names) == len(set(names)) and "apps" not in names, "command slugs are unique"
    assert all(re.fullmatch(r"[a-z][a-z0-9-]{1,31}", name) for name in names), "command slug shape"
    entries = []
    for index, entry in enumerate(apps()):
        library_id, title, purpose, source, views, route = entry[:6]
        records_spec = entry[6] if len(entry) > 6 else None
        contract = sources[source][0]
        b = bundle(index, library_id, title, purpose, contract, views, revisions[source], records_spec)
        validate_bundle(b)
        text = json.dumps(b, ensure_ascii=False, indent=2) + "\n"
        files[library_id + ".json"] = text.encode("utf-8")
        entry = catalog_entry(library_id, title, purpose, route, b, source + ".csv")
        entry["fixture_sha256"] = hashlib.sha256(files[library_id + ".json"]).hexdigest()
        entries.append(entry)
    catalog = {"schema_version": 1, "synthetic": True, "edition": EDITION,
               "scope": "Prebuilt library of admitted app definitions for the ADR-011 pitch; synthetic data, no customer evidence. "
                        "Router metadata only: nothing here is a grant, a query result or an admission verdict.",
               "window_field": "time", "destination_field": "destination",
               "generator": "contracts/make_library.py", "normalizer": "app/runtime/library_sources.py", "apps": entries}
    files["catalog.json"] = (json.dumps(catalog, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    return files


def main(argv=None):
    check = "--check" in (argv or sys.argv[1:])
    files = build()
    stale = []
    for relative, data in files.items():
        path = LIBRARY / relative
        if check:
            if not path.exists() or path.read_bytes() != data:
                stale.append(relative)
            continue
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    if check:
        existing = {str(p.relative_to(LIBRARY)) for p in LIBRARY.rglob("*") if p.is_file()}
        stale.extend(sorted(existing - set(files)))
        if stale:
            print(json.dumps({"stale": stale}), file=sys.stderr)
            return 1
    print(json.dumps({"files": len(files), "apps": sum(1 for n in files if n.endswith(".json") and n != "catalog.json")}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
