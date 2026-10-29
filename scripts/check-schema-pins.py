#!/usr/bin/env python3
"""Refuse a partial schema-version pin update; reads repository files only.

Every pool refuses admission (Error::Unavailable) unless the migration manifest,
the runtime constant, the native version check and both embedded catalogs name
the same database version (ADR-007 D7.12). No database, image or network use.
Exits 0 only when all five pins are found exactly once and agree.
"""
import argparse
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = "app/migrations/manifest.json"
POOL = "app/src/storage/pool.rs"
INCLUDES = (
    ("app/runtime/runtime.py DATABASE_SCHEMA_VERSION",
     "app/runtime/runtime.py", re.compile(r"^DATABASE_SCHEMA_VERSION = ([0-9]+)$", re.M)),
    ("app/src/storage/catalog.rs EXPECTED include",
     "app/src/storage/catalog.rs", re.compile(r'^const EXPECTED: &str = include_str!\("\.\./\.\./migrations/catalog-v([0-9]+)\.json"\);$', re.M)),
    ("app/src/storage/managed_qualification.rs CATALOG include",
     "app/src/storage/managed_qualification.rs", re.compile(r'^const CATALOG: &\[u8\] = include_bytes!\("\.\./\.\./migrations/catalog-v([0-9]+)\.json"\);$', re.M)),
)
VERSION_CHECK = re.compile(r"^async fn verify_schema_version\(.*?^\}$", re.M | re.S)
VERSION_LITERAL = re.compile(r"!= ([0-9]+)")


class Refused(ValueError):
    pass


def single(pattern, text, label):
    found = pattern.findall(text)
    if len(found) != 1:
        raise Refused(label + " must appear exactly once")
    return found[0]


def pins(root):
    root = Path(root)
    manifest = json.loads((root / MANIFEST).read_text())
    version = manifest.get("database_version") if isinstance(manifest, dict) else None
    if type(version) is not int:
        raise Refused(MANIFEST + " database_version must be an integer")
    found = {MANIFEST + " database_version": version}
    for label, name, pattern in INCLUDES:
        found[label] = int(single(pattern, (root / name).read_text(), label))
    body = single(VERSION_CHECK, (root / POOL).read_text(), POOL + " verify_schema_version")
    found[POOL + " verify_schema_version literal"] = int(single(VERSION_LITERAL, body, POOL + " verify_schema_version literal"))
    return found


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT, help="repository root to check (default: this checkout)")
    args = parser.parse_args(argv)
    try:
        found = pins(args.root)
    except (OSError, ValueError) as error:
        print(json.dumps({"agree": False, "refused": str(error)}))
        return 1
    agree = len(set(found.values())) == 1
    print(json.dumps({"agree": agree, "pins": found}, indent=2))
    return 0 if agree else 1


if __name__ == "__main__":
    sys.exit(main())
