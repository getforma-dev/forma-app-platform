"""Compare two synthesized GenuiFoundation templates (APP-077 evidence; no cloud call).

Usage: python3 template_diff.py BASE.json NEW.json OUT.json
"""
import hashlib
import json
import sys

BASE, NEW, OUT = sys.argv[1:4]
base, new = json.load(open(BASE)), json.load(open(NEW))
digest = lambda value: "sha256:" + hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
report = {"scope": "APP-077 CDK template comparison: GenuiFoundation('SecurityQualification') synthesized at d6986c2 (base) and at the APP-077 commit (new), same aws-cdk-lib 2.275.0; no cloud call",
          "sections": {}}
for section in ("Parameters", "Conditions", "Resources", "Outputs"):
    a, b = base.get(section, {}), new.get(section, {})
    report["sections"][section] = {
        "added": sorted(set(b) - set(a)), "removed": sorted(set(a) - set(b)),
        "changed": sorted(k for k in a if k in b and a[k] != b[k]),
        "unchanged_count": sum(1 for k in a if k in b and a[k] == b[k]),
        "unchanged_digest": digest({k: a[k] for k in sorted(a) if k in b and a[k] == b[k]}),
    }
prefixes = ("Task", "QualificationTask", "MigrationTask", "Service", "MigrationExecutionRole", "QualificationExecutionRole", "TaskRole", "MigrationRole")
named = {k: v for k, v in base["Resources"].items() if k.startswith(prefixes)}
report["named_existing_resources"] = {k: {"base": digest(v), "new": digest(new["Resources"].get(k))} for k, v in sorted(named.items())}
report["added_resource_conditions"] = {k: new["Resources"][k].get("Condition") for k in report["sections"]["Resources"]["added"]}
report["added_output_conditions"] = {k: new["Outputs"][k].get("Condition") for k in report["sections"]["Outputs"]["added"]}
json.dump(report, open(OUT, "w"), indent=2, sort_keys=True)
print(json.dumps({s: {k: v for k, v in r.items() if k in ("added", "removed", "changed")} for s, r in report["sections"].items()}))
