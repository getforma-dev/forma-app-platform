#!/usr/bin/env python3
"""Independent fixture oracle: reads original files, never generator row constants.

The small OOXML reader verifies this known fixture subset, not arbitrary uploads.
APP-004 remains the bounded untrusted-input parser. This is not an import service.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
import csv
from datetime import datetime, timezone
from fractions import Fraction
import hashlib
import ipaddress
import json
from pathlib import Path, PurePosixPath
import re
import zipfile
from xml.etree import ElementTree

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = Path(__file__).parent / "phase-one" / "v1"


class Refused(ValueError):
    pass


def sha(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def member(root, relative):
    root = root.resolve(strict=True)
    path = PurePosixPath(relative)
    if path.is_absolute() or ".." in path.parts or str(path) != relative:
        raise Refused("unsafe_fixture_path")
    result = root / relative
    if any(p.is_symlink() for p in [result, *result.parents] if p.is_relative_to(root)):
        raise Refused("symlink_fixture_path")
    if not result.is_file() or not result.resolve().is_relative_to(root.resolve()):
        raise Refused("missing_fixture_file")
    return result


def load_package(root=DEFAULT):
    root = Path(root)
    manifest = json.loads(member(root, "manifest.json").read_text())
    if manifest["edition"] != "app011-fixtures-v1" or manifest["synthetic"] is not True:
        raise Refused("fixture_edition")
    known = {"manifest.json"}
    ids = set()
    for entry in manifest["sources"] + manifest["metadata"]:
        path = entry["path"]
        if path in known:
            raise Refused("duplicate_fixture_path")
        known.add(path)
        actual = member(root, path)
        if actual.stat().st_size != entry["bytes"] or sha(actual) != entry.get("original_digest", entry.get("digest")):
            raise Refused("fixture_digest_mismatch:" + path)
        if "id" in entry:
            if entry["id"] in ids:
                raise Refused("duplicate_source_identity")
            ids.add(entry["id"])
    actual_paths = {p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file()}
    if known != actual_paths:
        raise Refused("fixture_inventory_mismatch")
    for entry in manifest["source_documents"]:
        if sha(member(ROOT, entry["path"])) != entry["digest"]:
            raise Refused("provenance_document_drift")
    return manifest


def table(root, entry):
    path = member(Path(root), entry["path"])
    if entry["format"] == "csv":
        with path.open(newline="", encoding="utf-8") as stream:
            cells = list(csv.reader(stream, strict=True))
    else:
        ns = {"s": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}
        with zipfile.ZipFile(path) as archive:
            if sum(x.file_size for x in archive.infolist()) > 50 * 1024 * 1024:
                raise Refused("fixture_reader_size")
            workbook = ElementTree.fromstring(archive.read("xl/workbook.xml"))
            sheets = workbook.findall("s:sheets/s:sheet", ns)
            if len(sheets) != 1 or sheets[0].attrib["name"] != entry["sheet"]:
                raise Refused("fixture_reader_sheet")
            sheet = ElementTree.fromstring(archive.read("xl/worksheets/sheet1.xml"))
            cells = []
            for row in sheet.findall("s:sheetData/s:row", ns):
                values = []
                for cell in row.findall("s:c", ns):
                    if cell.find("s:f", ns) is not None:
                        raise Refused("formula_rejected")
                    value = cell.find("s:is/s:t" if cell.get("t") == "inlineStr" else "s:v", ns)
                    values.append("" if value is None else value.text or "")
                cells.append(values)
    if not cells or cells[0] != entry["columns"] or len(cells) != entry["data_rows"] + 1:
        raise Refused("fixture_table_shape")
    if any(len(row) != len(cells[0]) for row in cells):
        raise Refused("fixture_row_width")
    return [dict(zip(cells[0], row)) for row in cells[1:]]


def timestamp(value):
    if not isinstance(value, str) or not re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ", value):
        raise Refused("timestamp_refused")
    try:
        return datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
    except ValueError as exc:
        raise Refused("timestamp_refused") from exc


def integer(value):
    if not isinstance(value, str) or not re.fullmatch(r"0|[1-9]\d{0,37}", value):
        raise Refused("integer_refused")
    return int(value)


def inventory_rows(rows):
    exact, networks, identities = {}, {}, set()
    for row in rows:
        identity = row["inventory_id"]
        if not identity or identity in identities:
            raise Refused("inventory_identity_refused")
        identities.add(identity)
        try:
            if row["kind"] == "exact":
                key = ipaddress.IPv4Address(row["address"])
                target = exact
            elif row["kind"] == "network":
                key = ipaddress.IPv4Network(row["address"], strict=True)
                target = networks
            else:
                raise Refused("inventory_kind_refused")
        except ValueError as exc:
            raise Refused("inventory_address_refused") from exc
        if key in target:
            raise Refused("inventory_ambiguous")
        target[key] = identity
    return exact, sorted(networks.items(), key=lambda pair: -pair[0].prefixlen)


def match(address, inventory):
    exact, networks = inventory
    if address in exact:
        return exact[address]
    return next((identity for network, identity in networks if address in network), None)


def event_oracle(rows, inventory, mapping, asset):
    if mapping.get("unit") != "byte":
        raise Refused("unit_refused")
    if mapping.get("timestamp_semantics") != "event_attributed_count":
        raise Refused("semantics_refused")
    if mapping.get("coverage") not in ("complete", "partial"):
        raise Refused("coverage_refused")
    window = mapping["window"]
    start, end = timestamp(window["start"]), timestamp(window["end"])
    seconds = int((end - start).total_seconds())
    if window.get("bounds") != "half_open" or seconds <= 0 or seconds % 60:
        raise Refused("window_refused")
    parsed = []
    identities = set()
    for row in rows:
        if not row["event_id"] or row["event_id"] in identities:
            raise Refused("event_identity_refused")
        identities.add(row["event_id"])
        time = timestamp(row["time"])
        value = integer(row["bytes"])
        try:
            destination = ipaddress.IPv4Address(row["destination"])
        except ValueError as exc:
            raise Refused("destination_refused") from exc
        if row["asset"] == asset and start <= time < end:
            parsed.append((row, time, value, destination))
    inv = inventory_rows(inventory)
    destinations, buckets, matches = defaultdict(int), {}, {}
    total = unmatched = 0
    for row, time, value, destination in parsed:
        total += value
        match_id = match(destination, inv)
        matches[row["event_id"]] = match_id
        if match_id is None:
            unmatched += value
        destinations[str(destination)] += value
        minute = int((time - start).total_seconds()) // 60
        buckets[minute] = buckets.get(minute, 0) + value
    complete = mapping["coverage"] == "complete"
    return {"included_ids": [row[0]["event_id"] for row in parsed], "total_bytes": str(total),
            "matched_bytes": str(total - unmatched), "unmatched_bytes": str(unmatched),
            "bytes_per_minute": str(Fraction(total * 60, seconds)) if complete else None,
            "unmatched_bytes_per_minute": str(Fraction(unmatched * 60, seconds)) if complete else None,
            "destinations": {key: str(value) for key, value in destinations.items()},
            "minute_bytes": [str(buckets[i]) if i in buckets else ("0" if complete else None) for i in range(seconds // 60)],
            "matches": matches, "coverage": mapping["coverage"], "total_scope": "observed selected asset and half-open window"}


def spend_oracle(rows, mapping):
    if mapping.get("currency") != "USD" or mapping.get("exponent") != 2 or mapping.get("unit") != "minor_unit":
        raise Refused("currency_refused")
    ids = set()
    threshold = integer(mapping["threshold_cents"])
    total = selected = 0
    queue = []
    for row in rows:
        if not row["expense_id"] or row["expense_id"] in ids:
            raise Refused("expense_identity_refused")
        ids.add(row["expense_id"])
        if row["currency"] != "USD":
            raise Refused("currency_refused")
        value = integer(row["amount_cents"])
        total += value
        if value >= threshold:
            queue.append(row["expense_id"])
            selected += value
    return {"total_cents": str(total), "queue_cents": str(selected), "queue_ids": queue,
            "currency": "USD", "exponent": 2, "initial_decisions": []}


def subset(actual, expected):
    for key, value in expected.items():
        if actual.get(key) != value:
            raise AssertionError(f"oracle disagreement for {key}: {actual.get(key)!r} != {value!r}")


def verify(root=DEFAULT):
    root = Path(root)
    manifest = load_package(root)
    mapping = json.loads((root / "mappings.json").read_text())
    expected = json.loads((root / "expected.json").read_text())
    datasets = {}
    equivalence = []
    for entry in manifest["sources"]:
        if entry["parser_expect"] != "accepted":
            continue
        rows = table(root, entry)
        if entry["dataset"] in datasets:
            if rows != datasets[entry["dataset"]]:
                raise AssertionError("CSV/XLSX disagree: " + entry["dataset"])
            equivalence.append(entry["dataset"])
        datasets[entry["dataset"]] = rows
    results = {}
    for name, source, inv, asset in (("northstar", "northstar-events", "northstar-inventory", "build-worker-17"), ("ops-runner", "ops-runner-events", "northstar-inventory", "ops-runner-04"), ("canary", "canary-events", "canary-inventory", "canary-worker"), ("boundaries", "window-boundaries", "northstar-inventory", "build-worker-17")):
        results[name] = event_oracle(datasets[source], datasets[inv], mapping["events"], asset)
        subset(results[name], expected[name])
    overlap = event_oracle(datasets["northstar-events"], datasets["inventory-overlap"], mapping["events"], "build-worker-17")
    if overlap["matches"] != expected["inventory-overlap"] or overlap["total_bytes"] != "1140000":
        raise AssertionError("exact/longest matching or counted-once result disagrees")
    results["inventory-overlap"] = overlap
    results["spend-review"] = spend_oracle(datasets["spend-review"], mapping["spend"])
    subset(results["spend-review"], expected["spend-review"])
    results["refusals"] = {}
    for name, reason in expected["invalid_mapping"].items():
        try:
            if name == "mixed-currency":
                spend_oracle(datasets[name], mapping["spend"])
            else:
                event_oracle(datasets[name], datasets["northstar-inventory"], mapping["events"], "build-worker-17")
        except Refused as exc:
            if str(exc) != reason:
                raise
            results["refusals"][name] = str(exc)
        else:
            raise AssertionError("invalid mapping admitted: " + name)
    for variant in mapping["interpretation_variants"]:
        changed = {**mapping["events"], **variant["events_override"]}
        try:
            result = event_oracle(datasets["northstar-events"], datasets["northstar-inventory"], changed, "build-worker-17")
        except Refused as exc:
            if str(exc) != variant["expect"]:
                raise
            results["refusals"][variant["id"]] = str(exc)
        else:
            if variant["id"] != "partial-coverage" or result["bytes_per_minute"] is not None or result["minute_bytes"][4] is not None or result["total_bytes"] != "1140000":
                raise AssertionError("partial coverage certified unobserved time")
            results["partial-coverage"] = result
    results["paired_csv_xlsx_datasets"] = equivalence
    results["source_count"] = len(manifest["sources"])
    results["scope"] = "independent fixture oracle; no application or unsupported query-operator acceptance"
    return results


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=DEFAULT)
    args = parser.parse_args()
    print(json.dumps(verify(args.root), indent=2, sort_keys=True))
