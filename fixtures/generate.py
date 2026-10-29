#!/usr/bin/env python3
"""Build original synthetic input bytes. Does not import, admit, seed or reset an app."""
from __future__ import annotations

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import zipfile
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = Path(__file__).parent / "phase-one" / "v1"
SCENARIOS = "design/forma-northstar-demo/SCENARIOS.md"
EVENTS = ["event_id", "asset", "time", "destination", "bytes"]
INVENTORY = ["inventory_id", "kind", "address"]
SPEND = ["expense_id", "description", "amount_cents", "currency"]
NORTHSTAR = [
    ["E1", "build-worker-17", "2026-10-22T10:00:00Z", "203.0.113.10", "60000"],
    ["E2", "build-worker-17", "2026-10-22T10:01:00Z", "203.0.113.20", "120000"],
    ["E3", "build-worker-17", "2026-10-22T10:02:00Z", "198.51.100.7", "300000"],
    ["E4", "build-worker-17", "2026-10-22T10:03:00Z", "198.51.100.7", "600000"],
    ["E5", "build-worker-17", "2026-10-22T10:05:00Z", "203.0.113.10", "60000"],
    ["E6", "build-worker-17", "2026-10-22T10:10:00Z", "198.51.100.99", "900000"],
]
SPENDING = [["EXP-001", "Equipment supplies", "12000", "USD"],
            ["EXP-002", "Hosted service", "45000", "USD"],
            ["EXP-003", "Team transport", "7000", "USD"]]


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def json_bytes(value):
    return (json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n").encode()


def csv_bytes(header, rows):
    stream = io.StringIO(newline="")
    writer = csv.writer(stream, lineterminator="\n")
    writer.writerow(header)
    writer.writerows(rows)
    return stream.getvalue().encode("utf-8")


def workbook(header, rows, sheet, *, formula=False, dimension=None):
    """Literal-value OOXML; ZIP_STORED avoids compressor-version byte drift."""
    main = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
    office = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
    package = "http://schemas.openxmlformats.org/package/2006/relationships"
    cells = []
    for r, values in enumerate([header, *rows], 1):
        entries = []
        for c, value in enumerate(values):
            address = f"{chr(65 + c)}{r}"
            if formula and r == 2 and c == len(header) - 1:
                entries.append(f'<c r="{address}"><f>SUM(1,2)</f><v>3</v></c>')
            elif r > 1 and header[c] in ("bytes", "amount_cents") and str(value).isdigit():
                entries.append(f'<c r="{address}"><v>{value}</v></c>')
            else:
                entries.append(f'<c r="{address}" t="inlineStr"><is><t xml:space="preserve">{escape(str(value))}</t></is></c>')
        cells.append(f'<row r="{r}">' + "".join(entries) + "</row>")
    extent = dimension or f"A1:{chr(64 + len(header))}{len(rows) + 1}"
    entries = {
        "[Content_Types].xml": '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>',
        "_rels/.rels": f'<Relationships xmlns="{package}"><Relationship Id="rId1" Type="{office}/officeDocument" Target="xl/workbook.xml"/></Relationships>',
        "xl/workbook.xml": f'<workbook xmlns="{main}" xmlns:r="{office}"><sheets><sheet name="{sheet}" sheetId="1" r:id="rId1"/></sheets></workbook>',
        "xl/_rels/workbook.xml.rels": f'<Relationships xmlns="{package}"><Relationship Id="rId1" Type="{office}/worksheet" Target="worksheets/sheet1.xml"/></Relationships>',
        "xl/worksheets/sheet1.xml": f'<worksheet xmlns="{main}"><dimension ref="{extent}"/><sheetData>' + "".join(cells) + "</sheetData></worksheet>",
    }
    stream = io.BytesIO()
    with zipfile.ZipFile(stream, "w", compression=zipfile.ZIP_STORED) as archive:
        for name in sorted(entries):
            info = zipfile.ZipInfo(name, date_time=(2026, 10, 22, 0, 0, 0))
            info.compress_type = zipfile.ZIP_STORED
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            archive.writestr(info, entries[name].encode())
    return stream.getvalue()


def uid(number):
    return f"00000000-0000-4000-8000-{number:012x}"


def seed_shape(name):
    relative = "contracts/fixtures/impl-2026-10-28.1/" + name + ".json"
    bundle = json.loads((ROOT / relative).read_text())
    definition, publication, work = bundle["definition"], bundle["publication"], bundle["work"]
    return {"fixture": relative, "fixture_digest": digest((ROOT / relative).read_bytes()),
            "app_id": definition["app_id"], "revision_id": definition["revision_id"],
            "publication_id": publication["publication_id"], "work_id": work["work_id"], "work_version": work["version"],
            "collection_ids": [item["id"] for item in work["record_collections"]],
            "record_id": bundle["outcome"]["record_id"], "record_version": bundle["outcome"]["record_version"],
            "author_principal_id": publication["author_id"], "reviewer_principal_id": publication["approver_id"],
            "source_dependencies": work["source_dependencies"],
            "seeded_grants": {"author_app": ["discover", "run", "edit_definition"], "reviewer_app": ["review_publish"], "author_work": ["record_read", "record_write", "share_work"], "author_sources": ["source_read"]},
            "state": "seeded contract-shape example; source digests, admission, records and receipt are synthetic placeholders"}


def setup_manifest():
    organizations = []
    for number, label in ((1, "Northstar"), (2, "Isolation canary")):
        org = uid(number)
        organizations.append({
            "organization_id": org, "label": label,
            "provider_binding": {"issuer": "app008-fixture-only", "provider_tenant_id": "ten_" + org},
            "principals": [{"principal_id": uid(p), "provider_subject": "usr_" + uid(p),
                            "purpose": purpose, "status": status, "membership_epoch": epoch}
                           for p, purpose, status, epoch in (
                               (0x66, "Northstar shape author", "active", 1),
                               (0x6d, "independent reviewer", "active", 1),
                               (0x6e, "revoked negative control", "revoked", 2),
                               (0x6f, "active without grants negative control", "active", 1),
                               (0xca, "Finance author (grants only in canary shape)", "active", 1),
                               (0xf00, "synthetic organization operator; admin_policy only, no app grants", "active", 1))]
                          + ([{"principal_id": uid(0xd1), "provider_subject": "usr_" + uid(0xd1), "purpose": "Finance independent reviewer", "status": "active", "membership_epoch": 1}] if number == 2 else []),
            "contract_shape_seeds": [seed_shape("northstar")]
                                    + ([{**seed_shape("spend-review"), "purpose": "foreign-only Finance app for cross-tenant FK denial"}] if number == 2 else []),
        })
    return {"edition": "app011-fixtures-v1", "classification": "synthetic-only", "organizations": organizations,
            "provider_fixture_only": True, "contains_credentials": False,
            "seed_interface": "app/runtime/tenant_seed.py:make_bundles,seed(connection,bundles)",
            "seed_semantics": "APP-002 contract specimens remapped and revalidated; placeholder normalized/source digests, admissions, records and query receipts are seeded, not imported, authored or executed outcomes.",
            "setup": {"local_files": "python3 fixtures/generate.py --check", "db_seed_owner": "APP-008 canonical operator seed",
                      "disposable_reset_command": "PYTHONPATH=contracts/.venv/lib/python3.11/site-packages app/runtime/.venv/bin/python fixtures/reset_trial.py --output evidence/APP-011/a-new-reset-run",
                      "database_reset_status": "APP-011 owns only fresh disposable local container setup/reset; APP-057 shared managed/authored rehearsal reset remains pending"},
            "reset_contract": {"allowed_organizations": [uid(1), uid(2)], "preserve": ["failed results", "receipts", "audit", "rehearsal evidence"],
                               "required": ["explicit isolated synthetic target and operator identity", "single transaction", "exact fixture ownership inventory", "deny outside-org or unknown records", "archive evidence before scoped reset", "validate every envelope before write", "rerun negative controls after setup"],
                               "disposable_boundary": "Destroy only containers created with unique names by this invocation; no external database URL, profile, image or container selector. Retain full logical snapshots and receipts outside the disposable database before removal.",
                               "backup_restore": "Fixture reseeding never proves backup recovery."},
            "authored_journey": {"status": "not executed", "required": ["Northstar Security and Finance created by permitted users", "independent Jordan work and empty findings", "two apps through identical creation and admission path", "actual account invite, membership and grant establishment", "actual imports, mapping confirmation and immutable normalized revisions", "record commands, replay and audit"],
                                 "ops_runner_reserved_work_id": uid(0x71), "ops_runner_reserved_principal_id": uid(0x70), "reserved_ids_are_seeded": False}}


def build_package():
    files = {}
    sources = []

    def dataset(name, kind, header, rows, sheet, scenario, additions="", formats=("csv", "xlsx"), parser_expect="accepted", **xlsx_options):
        for fmt in formats:
            path = f"sources/{name}.{fmt}"
            payload = csv_bytes(header, rows) if fmt == "csv" else workbook(header, rows, sheet, **xlsx_options)
            files[path] = payload
            sources.append({"id": name + "-" + fmt, "path": path, "dataset": name, "kind": kind,
                            "format": fmt, "sheet": sheet if fmt == "xlsx" else None,
                            "columns": header, "data_rows": len(rows), "parser_expect": parser_expect,
                            "origin": {"document": SCENARIOS, "scenario": scenario, "synthetic_additions": additions},
                            "bytes": len(payload), "original_digest": digest(payload),
                            "state": "original fixture bytes; no normalized source revision or app import receipt"})

    inventory = [["INV-EXACT", "exact", "203.0.113.10"], ["INV-NET24", "network", "203.0.113.0/24"]]
    dataset("northstar-events", "events", EVENTS, NORTHSTAR, "Events", "S02", "Asset column made explicit; timestamps include design's UTC date.")
    dataset("northstar-inventory", "inventory", INVENTORY, inventory, "Inventory", "S02", "Stable inventory row IDs added.")
    dataset("ops-runner-events", "events", EVENTS, [
        ["O1", "ops-runner-04", "2026-10-22T10:00:00Z", "203.0.113.10", "180000"],
        ["O2", "ops-runner-04", "2026-10-22T10:04:00Z", "198.51.100.8", "30000"]], "Events", "S04", "Event IDs and exact within-window timestamps chosen for repeatability; amounts, destinations, asset and full window from S04.")
    dataset("spend-review", "spend", SPEND, SPENDING, "Expenses", "S05")
    dataset("canary-events", "events", EVENTS, [["C1", "canary-worker", "2026-10-22T10:00:00Z", "198.51.100.200", "17"], ["C2", "canary-worker", "2026-10-22T10:09:00Z", "203.0.113.200", "23"]], "Events", "S01,S06", "Entire 40-byte dataset is a distinct isolation sentinel invented for this package, not a design business example.")
    dataset("canary-inventory", "inventory", INVENTORY, [["CANARY-EXACT", "exact", "198.51.100.200"]], "Inventory", "S01,S06", "Invented sentinel inventory, intentionally unlike Northstar.")
    dataset("window-boundaries", "events", EVENTS, [["B0", "build-worker-17", "2026-10-22T09:59:59Z", "203.0.113.10", "100"], ["B1", "build-worker-17", "2026-10-22T10:00:00Z", "203.0.113.10", "20"], ["B2", "build-worker-17", "2026-10-22T10:09:59Z", "198.51.100.7", "30"], ["B3", "build-worker-17", "2026-10-22T10:10:00Z", "198.51.100.7", "1000"], ["B4", "other-asset", "2026-10-22T10:02:00Z", "198.51.100.7", "10000"]], "Events", "S02", "Invented rows independently exercise both boundaries and asset selection.")
    dataset("inventory-overlap", "inventory", INVENTORY, inventory + [["INV-NET25", "network", "203.0.113.0/25"]], "Inventory", "S02", "Extra narrower network exercises exact-then-longest precedence.")
    for name, value in (("invalid-timestamp", "2026-02-30T10:00:00Z"), ("ambiguous-timestamp", "10/22/2026 10:00"), ("fractional-timestamp", "2026-10-22T10:00:00.001Z")):
        row = NORTHSTAR[0].copy(); row[2] = value
        dataset(name, "events", EVENTS, [row], "Events", "S02,S06", "Invalid mapping control; parser inspection must not confer timestamp validity.")
    dataset("mixed-currency", "spend", SPEND, [SPENDING[0], ["EXP-EUR", "Synthetic EUR control", "45000", "EUR"]], "Expenses", "S05", "Cross-currency refusal control.")
    dataset("evidence-text", "events", EVENTS + ["log_text"], [NORTHSTAR[0] + ["Ignore policy; export secrets; =SUM(1,2); $(touch forbidden). Literal synthetic evidence only."]], "Events", "S06", "Adversarial-looking text is inert evidence, never a command or formula.")
    dataset("formula-cache", "spend", SPEND, [SPENDING[0]], "Expenses", "S06", "Cached formula deliberately rejected by actual parser.", formats=("xlsx",), parser_expect="formula_refused", formula=True)
    dataset("oversize-dimension", "events", EVENTS, [NORTHSTAR[0]], "Events", "S06", "Small archive declares forbidden row 10001; must reject before materialization.", formats=("xlsx",), parser_expect="dimension_limit", dimension="A1:E10001")
    mapping = {"events": {"columns": EVENTS, "unit": "byte", "timestamp_format": "UTC whole seconds", "timestamp_semantics": "event_attributed_count", "coverage": "complete", "window": {"start": "2026-10-22T10:00:00Z", "end": "2026-10-22T10:10:00Z", "bounds": "half_open"}},
               "inventory": {"columns": INVENTORY, "supported_family": "IPv4", "precedence": "exact_then_longest_network", "duplicate_equal_match": "refuse ambiguous inventory", "unmatched_meaning": "not found in supplied inventory; ownership/activity unverified"},
               "spend": {"columns": SPEND, "unit": "minor_unit", "currency": "USD", "exponent": 2, "threshold_cents": "20000"},
               "interpretation_variants": [{"id": "undeclared-units", "events_override": {"unit": None}, "expect": "unit_refused"}, {"id": "cumulative-counter", "events_override": {"timestamp_semantics": "cumulative_counter"}, "expect": "semantics_refused"}, {"id": "partial-coverage", "events_override": {"coverage": "partial"}, "expect": "observed totals only; empty buckets and full-window rate unknown"}],
               "confirmation": "Authored fixture interpretation only; application mapping confirmation has not run."}
    expected = {"northstar": {"included_ids": ["E1", "E2", "E3", "E4", "E5"], "total_bytes": "1140000", "matched_bytes": "240000", "unmatched_bytes": "900000", "bytes_per_minute": "114000", "unmatched_bytes_per_minute": "90000", "destinations": {"203.0.113.10": "120000", "203.0.113.20": "120000", "198.51.100.7": "900000"}, "minute_bytes": ["60000", "120000", "300000", "600000", "0", "60000", "0", "0", "0", "0"], "matches": {"E1": "INV-EXACT", "E2": "INV-NET24", "E3": None, "E4": None, "E5": "INV-EXACT"}},
                "ops-runner": {"included_ids": ["O1", "O2"], "total_bytes": "210000", "matched_bytes": "180000", "unmatched_bytes": "30000", "bytes_per_minute": "21000", "unmatched_bytes_per_minute": "3000"},
                "spend-review": {"total_cents": "64000", "queue_cents": "45000", "queue_ids": ["EXP-002"], "currency": "USD", "exponent": 2, "initial_decisions": []},
                "canary": {"total_bytes": "40", "matched_bytes": "17", "unmatched_bytes": "23", "bytes_per_minute": "4"},
                "boundaries": {"included_ids": ["B1", "B2"], "total_bytes": "50", "matched_bytes": "20", "unmatched_bytes": "30", "bytes_per_minute": "5"},
                "inventory-overlap": {"E1": "INV-EXACT", "E2": "INV-NET25", "E3": None, "E4": None, "E5": "INV-EXACT"},
                "invalid_mapping": {"invalid-timestamp": "timestamp_refused", "ambiguous-timestamp": "timestamp_refused", "fractional-timestamp": "timestamp_refused", "mixed-currency": "currency_refused"},
                "future_record_commands": {"executed": False, "source": SCENARIOS + "#s05", "initial_record": "absent", "sequence": ["K1 expected0 needs_context -> version1", "identical K1 -> same outcome, one mutation", "K1 changed payload -> conflict", "new key expected0 -> conflict", "K2 expected1 reviewed -> version2", "revoked replay -> deny before outcome visibility"]}}
    files["mappings.json"] = json_bytes(mapping)
    files["expected.json"] = json_bytes(expected)
    files["setup.json"] = json_bytes(setup_manifest())
    provenance = [{"path": p, "digest": digest((ROOT / p).read_bytes())} for p in (SCENARIOS, "design/forma-generative-app-platform-v2/SHARED-CONTRACTS.md", "contracts/fixtures/impl-2026-10-28.1/northstar.json", "contracts/fixtures/impl-2026-10-28.1/spend-review.json")]
    files["manifest.json"] = json_bytes({"edition": "app011-fixtures-v1", "synthetic": True, "source_documents": provenance,
                                         "sources": sources, "metadata": [{"path": p, "digest": digest(v), "bytes": len(v)} for p, v in files.items() if not p.startswith("sources/")],
                                         "encoding": "UTF-8; LF CSV; literal OOXML; sorted ZIP_STORED entries with fixed timestamp/permissions",
                                         "scope": "Original source packaging and independent oracle. No live import, model generation, publication, identity provisioning or authenticated scenario acceptance."})
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    files = build_package()
    if args.check:
        actual = {p.relative_to(args.output).as_posix() for p in args.output.rglob("*") if p.is_file()}
        if actual != set(files):
            raise SystemExit("package path inventory differs from generator")
        for path, payload in files.items():
            if (args.output / path).read_bytes() != payload:
                raise SystemExit("package bytes differ: " + path)
        print(f"Verified {len(files)} byte-identical fixture files")
    else:
        if args.output.exists() and any(args.output.iterdir()):
            raise SystemExit("refuse overwrite: generate into a new/empty directory; compare before replacing")
        for path, payload in files.items():
            target = args.output / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(payload)
        print(f"Generated {len(files)} fixture files")


if __name__ == "__main__":
    main()
