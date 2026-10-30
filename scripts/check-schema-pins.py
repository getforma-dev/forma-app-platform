#!/usr/bin/env python3
"""Refuse a partial schema-version or catalog-bound pin update; reads repository files only.

Every pool refuses admission (Error::Unavailable) unless the migration manifest,
the runtime constant, the native version check and both embedded catalogs name
the same database version (ADR-007 D7.12). The canonical catalog bound must be
one value in catalog-query.json (max_output_bytes and the SQL octet_length
literal), the native BOUND and runtime.py CATALOG_BOUND, or the projection is
refused (NULL body) or the embedded catalog is refused. Every ledger-era
catalog-vk.json (k >= 2) must record both version CHECKs as `version <= k`, and
consecutive catalogs must differ, so the runner can prove exactly one version.
No database, image or network use. Exits 0 only when every pin is found exactly
once and each agreement group agrees.

Deliberately not listed: the native campaign's fixture statements in
app/src/storage/qualification.rs that set unsupported or restored version rows;
they fail loudly in that campaign when stale.
"""
import argparse
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = "app/migrations/manifest.json"
MIGRATIONS = "app/migrations"
QUERY = "app/migrations/catalog-query.json"
POOL = "app/src/storage/pool.rs"
INCLUDES = (
    ("app/runtime/runtime.py DATABASE_SCHEMA_VERSION",
     "app/runtime/runtime.py", re.compile(r"^DATABASE_SCHEMA_VERSION = ([0-9]+)$", re.M)),
    ("app/src/storage/catalog.rs EXPECTED include",
     "app/src/storage/catalog.rs", re.compile(r'^const EXPECTED: &str = include_str!\("\.\./\.\./migrations/catalog-v([0-9]+)\.json"\);$', re.M)),
    ("app/src/storage/managed_qualification.rs CATALOG include",
     "app/src/storage/managed_qualification.rs", re.compile(r'^const CATALOG: &\[u8\] = include_bytes!\("\.\./\.\./migrations/catalog-v([0-9]+)\.json"\);$', re.M)),
)
BOUNDS = (
    ("app/runtime/runtime.py CATALOG_BOUND",
     "app/runtime/runtime.py", re.compile(r"^CATALOG_BOUND = ([0-9]+)$", re.M)),
    ("app/src/storage/catalog.rs BOUND",
     "app/src/storage/catalog.rs", re.compile(r"^const BOUND: usize = ([0-9_]+);$", re.M)),
)
SQL_BOUND = re.compile(r"octet_length\(body\)<=([0-9]+)")
VERSION_CHECK = re.compile(r"^async fn verify_schema_version\(.*?^\}$", re.M | re.S)
VERSION_LITERAL = re.compile(r"!= ([0-9]+)")
LEDGER_CHECKS = ("schema_version_version_check", "schema_migrations_version_check")


class Refused(ValueError):
    pass


def single(pattern, text, label):
    found = pattern.findall(text)
    if len(found) != 1:
        raise Refused(label + " must appear exactly once")
    return found[0]


def manifest_version(root):
    manifest = json.loads((root / MANIFEST).read_text())
    version = manifest.get("database_version") if isinstance(manifest, dict) else None
    if type(version) is not int:
        raise Refused(MANIFEST + " database_version must be an integer")
    return manifest, version


def pins(root):
    root = Path(root)
    _, version = manifest_version(root)
    found = {MANIFEST + " database_version": version}
    for label, name, pattern in INCLUDES:
        found[label] = int(single(pattern, (root / name).read_text(), label))
    body = single(VERSION_CHECK, (root / POOL).read_text(), POOL + " verify_schema_version")
    found[POOL + " verify_schema_version literal"] = int(single(VERSION_LITERAL, body, POOL + " verify_schema_version literal"))
    return found


def catalog_bound(root):
    """Second agreement group (runner review m1): one catalog byte bound."""
    root = Path(root)
    query = json.loads((root / QUERY).read_text())
    if not isinstance(query, dict) or type(query.get("max_output_bytes")) is not int or not isinstance(query.get("sql"), str):
        raise Refused(QUERY + " must carry an integer max_output_bytes and its SQL")
    found = {QUERY + " max_output_bytes": query["max_output_bytes"],
             QUERY + " SQL octet_length literal": int(single(SQL_BOUND, query["sql"], QUERY + " SQL octet_length literal"))}
    for label, name, pattern in BOUNDS:
        found[label] = int(single(pattern, (root / name).read_text(), label).replace("_", ""))
    return found


def catalog_version_checks(root):
    """Runner review m3: catalog-vk.json bounds both version CHECKs at k; neighbours differ."""
    root = Path(root)
    manifest, version = manifest_version(root)
    names = [item.get("file") if isinstance(item, dict) else None for item in manifest.get("catalog", [])]
    if names != ["catalog-query.json", *("catalog-v%d.json" % k for k in range(1, version + 1))]:
        raise Refused(MANIFEST + " catalog list must name catalog-query.json then catalog-v1..v%d.json" % version)
    documents = {k: json.loads((root / MIGRATIONS / ("catalog-v%d.json" % k)).read_text()) for k in range(1, version + 1)}
    checked = {}
    for k in range(2, version + 1):
        name = "catalog-v%d.json" % k
        if documents[k] == documents[k - 1]:
            raise Refused(name + " must differ from catalog-v%d.json" % (k - 1))
        rows = [row for row in documents[k].get("constraints", [])
                if isinstance(row, list) and len(row) == 8 and row[0] == "genui_meta" and row[2] in LEDGER_CHECKS]
        expected = "CHECK (version >= 1 AND version <= %d)" % k
        if sorted(row[2] for row in rows) != sorted(LEDGER_CHECKS) or any(row[7] != expected for row in rows):
            raise Refused(name + " must record both version CHECKs as " + expected)
        checked[name] = k
    return checked


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT, help="repository root to check (default: this checkout)")
    args = parser.parse_args(argv)
    try:
        found = pins(args.root)
        bound = catalog_bound(args.root)
        checks = catalog_version_checks(args.root)
    except (OSError, ValueError) as error:
        print(json.dumps({"agree": False, "refused": str(error)}))
        return 1
    agree = len(set(found.values())) == 1 and len(set(bound.values())) == 1
    print(json.dumps({"agree": agree, "pins": found, "catalog_bound": bound, "catalog_version_checks": checks}, indent=2))
    return 0 if agree else 1


if __name__ == "__main__":
    sys.exit(main())
