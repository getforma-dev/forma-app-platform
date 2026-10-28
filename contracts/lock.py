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


def sha(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()


def expected():
    assert sha(SOURCE) == SOURCE_SHA, "Frozen shared source changed"
    original = json.loads((ROOT / DEMO_LOCK).read_text())
    assert original["contract_edition"] == "prep-2026-10-22.1" and original["sha256"] == SOURCE_SHA, "Frozen companion lock changed"
    files = sorted(str(p.relative_to(ROOT)) for p in (ROOT / "contracts").glob("*") if p.is_file() and p.name != LOCK.name)
    files += sorted(str(p.relative_to(ROOT)) for p in (ROOT / "contracts/fixtures").glob("*.json"))
    files += ["docs/architecture/ADR-002-CONTRACT-EDITION.md", "evidence/APP-002/dependencies.json", ".github/workflows/contracts.yml"]
    return {"edition": "impl-2026-10-28.1", "source_edition": "prep-2026-10-22.1", "status": "synthetic contract qualification; not runtime authorization or persistence",
            "canonical_encoding": "forma-json-v1", "sources": {SOURCE: SOURCE_SHA, DEMO_LOCK: sha(DEMO_LOCK)},
            "artifacts": {path: sha(path) for path in sorted(files)}}


if __name__ == "__main__":
    if len(sys.argv) != 2 or sys.argv[1] not in ("write", "check"):
        raise SystemExit("Usage: python contracts/lock.py write|check")
    value = expected()
    if sys.argv[1] == "write":
        LOCK.write_text(json.dumps(value, indent=2) + "\n")
        print("Wrote implementation edition lock")
    else:
        assert json.loads(LOCK.read_text()) == value, "Contract lock stale or artifact drifted; review changes before regenerating"
        print(f"Contract edition lock matches {len(value['artifacts'])} artifacts and both frozen source identities")
