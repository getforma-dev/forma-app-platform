"""Verify edition artifacts without modifying frozen design evidence."""
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "contracts/implementation-lock.json"
SOURCE = "design/forma-generative-app-platform-v2/SHARED-CONTRACTS.md"
DEMO_LOCK = "design/forma-northstar-demo/platform-contract-lock.json"
SOURCE_SHA = "f209ccb05aa895080c6fd424fe979d65f276959536ee7d57b371428e1d049fd5"
EDITION = "impl-2026-10-29.3"
# Each prior edition's lock file and the fixture identities it locked. Its fixtures are retained
# byte-identically under contracts/fixtures/<edition>/ and must keep these identities (ADR-002).
RETAINED = {
    "impl-2026-10-29.2": {
        "lock_sha256": "c1c1c80d7e327c53e2558da37befb97c9dbf6900bed56225e17518f79d952923",
        "artifacts": {
            "contracts/fixtures/impl-2026-10-29.2/canonical-vectors.json": "eb9a4501813c8c90d8fbb756179a64074dd2df0a31883f39e995601afa9354f9",
            "contracts/fixtures/impl-2026-10-29.2/negative-vectors.json": "315a9d32e91dbeaf21c0ad3ec6f8e882315e46ef23b7db4c3c47f4b161ae1cbc",
            "contracts/fixtures/impl-2026-10-29.2/northstar.json": "d9acfd8f399d20ea6dbcabc72dca1437c802a1565ca7119c6dd53587b14e1480",
            "contracts/fixtures/impl-2026-10-29.2/spend-review.json": "186162da87532f368ed3c74faafc46c04780cc7331c095dc92f729bead0f0851"
        },
    },
    "impl-2026-10-28.1": {
        "lock_sha256": "9bf0bc694bd75ea7a370c9b852809f53c4a312978fe31bb4ce61e5a4e6569a45",
        "artifacts": {
            "contracts/fixtures/impl-2026-10-28.1/canonical-vectors.json": "33b46ff5970bdcd5259f23bdb5e9a7fcda4c5eeffebe93e0fedfebf5f661203d",
            "contracts/fixtures/impl-2026-10-28.1/negative-vectors.json": "089e9a3017adda324a2a150fb68d5aa497889cd104d2f7096b9128d520764e75",
            "contracts/fixtures/impl-2026-10-28.1/northstar.json": "45639ea369193d60f45fbafbe6c3aad648881017df57615bdf89bd501dfdd592",
            "contracts/fixtures/impl-2026-10-28.1/spend-review.json": "4a4581f5a7fcfec867070584a00b0fbebdf33558fffe538eec35f97f72aa5650",
        },
    },
}


def sha(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def expected():
    assert sha(SOURCE) == SOURCE_SHA, "Frozen shared source changed"
    original = json.loads((ROOT / DEMO_LOCK).read_text())
    assert original["contract_edition"] == "prep-2026-10-22.1" and original["sha256"] == SOURCE_SHA, "Frozen companion lock changed"
    for edition in RETAINED.values():
        for path, identity in edition["artifacts"].items():
            assert sha(path) == identity, f"Retained edition artifact changed: {path}"
    files = sorted(str(p.relative_to(ROOT)) for p in (ROOT / "contracts").glob("*") if p.is_file() and p.name != LOCK.name)
    files += sorted(str(p.relative_to(ROOT)) for p in (ROOT / "contracts/fixtures").rglob("*.json"))
    files += ["docs/architecture/ADR-002-CONTRACT-EDITION.md", "docs/architecture/ADR-009-PHASE-2-DATA-PLANE.md",
              "evidence/APP-002/dependencies.json", ".github/workflows/contracts.yml"]
    return {"edition": EDITION, "source_edition": "prep-2026-10-22.1", "status": "synthetic contract qualification; not runtime authorization or persistence",
            "canonical_encoding": "forma-json-v1", "sources": {SOURCE: SOURCE_SHA, DEMO_LOCK: sha(DEMO_LOCK)},
            "retained_editions": RETAINED, "artifacts": {path: sha(path) for path in sorted(files)}}


if __name__ == "__main__":
    if len(sys.argv) != 2 or sys.argv[1] not in ("write", "check"):
        raise SystemExit("Usage: python contracts/lock.py write|check")
    value = expected()
    if sys.argv[1] == "write":
        LOCK.write_text(json.dumps(value, indent=2) + "\n")
        print("Wrote implementation edition lock")
    else:
        assert json.loads(LOCK.read_text()) == value, "Contract lock stale or artifact drifted; review changes before regenerating"
        print(f"Contract edition {EDITION} lock matches {len(value['artifacts'])} artifacts, "
              f"{sum(len(e['artifacts']) for e in RETAINED.values())} retained edition-one artifacts and both frozen source identities")
