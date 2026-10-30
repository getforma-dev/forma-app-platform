#!/usr/bin/env python3
"""Retain actual parser, independent oracle and bounded query-subset checks.

Run with the existing APP-005 locked environment. No network, cloud or app DB.
Every run needs a new output directory so earlier failures cannot be erased.
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import tomllib

from verify import DEFAULT, ROOT, load_package, sha, table, timestamp, verify


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def parser_rows(report):
    cells = report["result"]["result"]["cells"]
    rows = {}
    for cell in cells:
        coordinate = cell["coordinate"]
        row = coordinate.get("row", coordinate.get("logical_record"))
        column = coordinate.get("column", cell.get("column"))
        if (row, column) in rows:
            raise AssertionError("duplicate parser cell")
        rows[row, column] = cell["original"]
    last_row = max(row for row, column in rows)
    last_column = max(column for row, column in rows)
    grid = [[rows[row, column] for column in range(1, last_column + 1)] for row in range(1, last_row + 1)]
    return [dict(zip(grid[0], values)) for values in grid[1:]]


# Ambient Cargo/rustc knobs that would change the compiled parser without touching any pinned file.
BUILD_OVERRIDES = ("RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_TARGET_DIR")


def parser_binding(output):
    """Bind the parser under test to APP-004's recorded qualification inputs.

    The recorded WASM digest is host-specific: the wasm32 build embeds absolute
    Cargo registry and checkout paths, and evidence/APP-004/reproducibility.json
    claims no cross-host byte identity. Every recorded source file, Cargo.lock,
    the vendored ZIP provenance and the rustc/cargo release must match instead,
    and the locked build is rerun so the artifacts cannot be stale.
    """
    spike = ROOT / "spikes/APP-004"
    qualified = json.loads((ROOT / "evidence/APP-004/build-metadata.json").read_text())
    drift = sorted(name for name, digest in qualified["source_files_sha256"].items() if not (spike / name).is_file() or sha(spike / name) != digest)
    if drift or sha(spike / "Cargo.lock") != qualified["cargo_lock_sha256"] or sha(spike / "vendor/provenance.json") != qualified["vendor_provenance_sha256"]:
        raise AssertionError("APP-004 parser source differs from the qualified capture: " + " ".join(drift or ["Cargo.lock/vendor provenance"]))
    overrides = sorted(name for name in os.environ if name in BUILD_OVERRIDES or name.startswith(("CARGO_PROFILE_", "CARGO_TARGET_")))
    if overrides:
        raise AssertionError("ambient Cargo/rustc build overrides refused: " + " ".join(overrides))
    channel = "+" + tomllib.loads((spike / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    outputs = {}
    with (output / "parser-build.log").open("w") as log:
        for name, command in (("vendor", [sys.executable, "verify_vendor.py"]), ("rustc", ["rustc", channel, "-vV"]), ("cargo", ["cargo", channel, "-V"]),
                              ("guest", ["cargo", channel, "build", "--locked", "-p", "parser-guest", "--target", "wasm32-unknown-unknown", "--release"]),
                              ("host", ["cargo", channel, "build", "--locked", "-p", "parser-host"])):
            result = subprocess.run(command, cwd=spike, capture_output=True, text=True, timeout=900)
            log.write("$ " + " ".join(command) + "\n" + result.stdout + result.stderr + "\n")
            if result.returncode:
                raise AssertionError("APP-004 parser step failed: " + name)
            outputs[name] = result.stdout.strip()
    rustc = dict(line.split(": ", 1) for line in outputs["rustc"].splitlines() if ": " in line)
    recorded = dict(line.split(": ", 1) for line in qualified["rustc"].splitlines() if ": " in line)
    if outputs["cargo"] != qualified["cargo"] or any(rustc.get(key) != recorded[key] for key in ("release", "commit-hash", "commit-date", "LLVM version")):
        raise AssertionError("parser toolchain differs from the APP-004 qualified rustc/cargo")
    return {"source_files_matched": len(qualified["source_files_sha256"]), "cargo_lock_sha256": qualified["cargo_lock_sha256"], "vendor_provenance_sha256": qualified["vendor_provenance_sha256"],
            "vendor_verification": outputs["vendor"], "cargo": outputs["cargo"], "rustc": {key: rustc[key] for key in ("release", "commit-hash", "LLVM version", "host")},
            "app004_recorded_wasm_sha256": qualified["wasm_sha256"], "app004_recorded_host": recorded["host"], "app004_recorded_platform": qualified["platform"]}


def image_parser_binding(output, host, wasm, manifest_path):
    """Bind parser bytes extracted from a built image to this checkout's parser/ (ADR-009 D9.1).

    The guest under test is the image's own bytes; nothing is rebuilt. The image
    build's parser-manifest.json names the guest digest and the sources digest,
    Cargo.lock, vendored ZIP provenance and rustc that produced it. Each source
    input is recomputed from this checkout and must match, and rustc must be the
    release and commit APP-004 qualified. The host only runs the guest; its
    digest is reported, and matches the manifest when the image's own host runs.
    """
    parser_root = ROOT / "parser"
    image = json.loads(manifest_path.read_text())
    spec = importlib.util.spec_from_file_location("parser_write_manifest", parser_root / "write_manifest.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    tree, count = module.sources_sha256(parser_root)
    source = image["source"]
    with (output / "parser-build.log").open("w") as log:
        result = subprocess.run([sys.executable, "verify_vendor.py"], cwd=parser_root, capture_output=True, text=True, timeout=120)
        log.write("$ python verify_vendor.py\n" + result.stdout + result.stderr + "\n")
    if result.returncode:
        raise AssertionError("parser vendored ZIP verification failed")
    qualified = json.loads((ROOT / "evidence/APP-004/build-metadata.json").read_text())
    recorded = dict(line.split(": ", 1) for line in qualified["rustc"].splitlines() if ": " in line)
    channel = tomllib.loads((parser_root / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    wasm_digest = sha(wasm)
    mismatches = [name for name, ok in (
        ("schema", image.get("schema") == module.SCHEMA),
        ("guest_sha256", "sha256:" + image["guest"]["sha256"] == wasm_digest),
        ("sources_sha256", source["sources_sha256"] == tree and source["source_files"] == count),
        ("cargo_lock_sha256", source["cargo_lock_sha256"] == sha(parser_root / "Cargo.lock")[7:]),
        ("vendor_provenance_sha256", source["vendor_provenance_sha256"] == sha(parser_root / "vendor/provenance.json")[7:]),
        ("rust_toolchain", source["rust_toolchain"] == channel == source["rustc"]["release"] == recorded["release"]),
        ("rustc_commit", source["rustc"]["commit-hash"] == recorded["commit-hash"] and source["rustc"]["LLVM version"] == recorded["LLVM version"]),
    ) if not ok]
    if mismatches:
        raise AssertionError("image parser bytes are not bound to this checkout's parser/: " + " ".join(mismatches))
    return {"mode": "image-bytes", "parser_manifest_sha256": sha(manifest_path), "sources_sha256": tree, "source_files": count,
            "cargo_lock_sha256": source["cargo_lock_sha256"], "vendor_provenance_sha256": source["vendor_provenance_sha256"], "vendor_verification": result.stdout.strip(),
            "rustc": source["rustc"], "app004_recorded_rustc_commit": recorded["commit-hash"],
            "host_sha256": sha(host), "host_matches_manifest": "sha256:" + image["host"]["sha256"] == sha(host),
            "basis": "the guest bytes are the image's own; the manifest written by the image build binds them to parser/ sources, Cargo.lock, vendored ZIP provenance and rustc, each recomputed from this checkout; no rebuild and no cross-host byte identity"}


def parser_checks(output, manifest, image=None):
    if image:
        host, wasm, manifest_path = image
        binding = image_parser_binding(output, host, wasm, manifest_path)
        artifact_pins = {"parser_host": sha(host), "parser_guest": sha(wasm)}
        binding["wasm_sha256"] = artifact_pins["parser_guest"]
    else:
        host = ROOT / "spikes/APP-004/target/debug/parser-host"
        wasm = ROOT / "spikes/APP-004/target/wasm32-unknown-unknown/release/parser_guest.wasm"
        binding = parser_binding(output)
        artifact_pins = {str(path.relative_to(ROOT)): sha(path) for path in (host, wasm)}
        binding["wasm_sha256"] = artifact_pins[str(wasm.relative_to(ROOT))]
        binding["wasm_matches_app004_recorded_bytes"] = binding["wasm_sha256"] == binding["app004_recorded_wasm_sha256"]
        binding["basis"] = "recorded APP-004 source files, Cargo.lock, vendored ZIP provenance and rustc/cargo identity are byte-identical and the locked build was rerun; byte identity with the recorded arm64 macOS artifact is reported, not required, because wasm32 bytes embed absolute registry/checkout paths"
    reports = {}

    def one(entry):
        config = {"profile": "demo", "format": entry["format"]}
        if entry["sheet"]:
            config["sheet"] = entry["sheet"]
        config_path = output / "parser" / (entry["id"] + ".config.json")
        save(config_path, config)
        result = subprocess.run([str(host), str(wasm), str(DEFAULT / entry["path"]), str(config_path)], capture_output=True, text=True, timeout=180)
        if result.returncode:
            save(output / "parser" / (entry["id"] + ".failure.json"), {"exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr})
            raise AssertionError("parser host failed: " + entry["id"])
        report = json.loads(result.stdout)
        save(output / "parser" / (entry["id"] + ".json"), report)
        if report["input_sha256"] != entry["original_digest"] or report["wasm_sha256"] != binding["wasm_sha256"] or report["interface_imports"] != []:
            raise AssertionError("parser input/module binding mismatch")
        result = report["result"]
        if entry["parser_expect"] == "accepted":
            if result["status"] != "parsed" or result["result"]["mapping_status"] != "inspection_required":
                raise AssertionError("parser acceptance mismatch: " + entry["id"] + " " + str(result))
            if parser_rows(report) != table(DEFAULT, entry):
                raise AssertionError("parser original values/coordinates disagree: " + entry["id"])
        elif result["status"] != "refused" or result["reason"] != entry["parser_expect"] or result["no_revision_created"] is not True:
            raise AssertionError("parser refusal mismatch: " + entry["id"] + " " + str(result))
        return entry["id"], report

    # Two isolated WASM processes, bounded above by host budgets and outer timeout.
    with ThreadPoolExecutor(max_workers=2) as pool:
        for name, report in pool.map(one, manifest["sources"]):
            reports[name] = report
            print("parser checked " + name, flush=True)
    return reports, artifact_pins, binding


def query_checks(output, reports, oracle):
    sys.path.insert(0, str(ROOT / "spikes/APP-005"))
    import duckdb
    from query import execute_query, field_ref, measure, typed_query

    all_results = {}
    for dataset, oracle_name, asset in (("northstar-events", "northstar", "build-worker-17"), ("ops-runner-events", "ops-runner", "ops-runner-04"), ("canary-events", "canary", "canary-worker"), ("window-boundaries", "boundaries", "build-worker-17")):
        # Explicit trusted synthetic adapter; APP-005 has only the registered
        # event_id/time/destination/bytes shape and cannot itself filter asset.
        original = parser_rows(reports[dataset + "-csv"])
        selected = [row for row in original if row["asset"] == asset]
        with tempfile.TemporaryDirectory(prefix="app011-query-") as directory:
            root = Path(directory)
            connection = duckdb.connect(str(root / "empty.duckdb")); connection.close()
            connection = duckdb.connect()
            try:
                connection.execute("SET threads=2")
                connection.execute("CREATE TABLE events(event_id BIGINT, time TIMESTAMP, destination VARCHAR, bytes BIGINT)")
                connection.executemany("INSERT INTO events VALUES (?, ?, ?, ?)", [(i, timestamp(row["time"]).replace(tzinfo=None), row["destination"], int(row["bytes"])) for i, row in enumerate(selected, 1)])
                connection.execute("COPY events TO ? (FORMAT PARQUET, COMPRESSION ZSTD)", [str(root / "s02.parquet")])
            finally:
                connection.close()
            manifest = {"files": {"s02.parquet": {"digest": sha(root / "s02.parquet")}}}
            queries = {"total": typed_query([measure("sum")]), "destinations": typed_query([measure("sum", group=["destination"])]),
                       "timeline_observed": typed_query([{"type": "time_bucket", "field": field_ref("time"), "seconds": 60, "timezone": "UTC", "output_field": "time"}, measure("sum", group=["time"])])}
            if dataset == "northstar-events":
                queries["destination_filter"] = typed_query([{"type": "filter", "field": field_ref("destination"), "comparison": "eq", "value": {"type": "text", "value": "198.51.100.7"}}, measure("sum")])
                queries["one_row_page"] = typed_query([{"type": "sort", "fields": [{"field": field_ref("event_id"), "direction": "asc"}], "stable_tiebreak": field_ref("event_id")}, {"type": "limit", "rows": 1}])
            results = {name: execute_query(root, "s02.parquet", query, manifest) for name, query in queries.items()}
            expected = oracle[oracle_name]
            if results["total"]["rows"] != [[expected["total_bytes"]]]:
                raise AssertionError("query total disagreement: " + dataset)
            if dict(results["destinations"]["rows"]) != expected["destinations"]:
                raise AssertionError("query destination disagreement: " + dataset)
            expected_buckets = {f"2026-10-22T10:{i:02}:00Z": amount for i, amount in enumerate(expected["minute_bytes"]) if amount != "0"}
            if dict(results["timeline_observed"]["rows"]) != expected_buckets:
                raise AssertionError("query bucket disagreement: " + dataset)
            if dataset == "northstar-events":
                if results["destination_filter"]["rows"] != [["900000"]] or results["one_row_page"]["receipt"]["truncated_page"] is not True or len(results["one_row_page"]["rows"]) != 1:
                    raise AssertionError("filter/page check failed")
            all_results[dataset] = {"source_original_digest": reports[dataset + "-csv"]["input_sha256"], "temporary_parquet_digest": manifest["files"]["s02.parquet"]["digest"],
                                    "trusted_fixture_adapter": {"selected_asset": asset, "numeric_event_id_to_original": {str(i): row["event_id"] for i, row in enumerate(selected, 1)}, "not_application_import": True}, "queries": queries, "results": results}
    result = {"duckdb_version": duckdb.__version__, "scope": "local APP-005 typed compiler/engine subset, fed from actual APP-004 parsed originals through explicit trusted fixture adapter",
              "not_qualified_here": ["OS process confinement/performance (APP-005 owns existing qualification)", "tenant authorization (APP-008)", "CIDR/inventory join operator", "rate/coverage query operators", "Spend Review input/money query operators", "application mapping/import pipeline", "full application receipts (these are synthetic_preview spike receipts)"], "datasets": all_results}
    save(output / "query-subset.json", result)
    return {"datasets": len(all_results), "queries": sum(len(v["queries"]) for v in all_results.values()), "duckdb_version": duckdb.__version__}


def seed_manifest_checks(output):
    spec = importlib.util.spec_from_file_location("app011_seed_contract", ROOT / "app/runtime/tenant_seed.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    bundles = module.make_bundles()  # revalidates; no database connection/write
    setup = json.loads((DEFAULT / "setup.json").read_text())
    actual = set()
    for bundle in bundles:
        org = bundle["context"]["organization_id"]
        actual.add((org, bundle["definition"]["app_id"]))
        configured = next(item for item in setup["organizations"] if item["organization_id"] == org)
        actors = {item["principal_id"] for item in configured["principals"]}
        if not {bundle["publication"]["author_id"], bundle["publication"]["approver_id"]} <= actors:
            raise AssertionError("seed actors absent from setup manifest")
        if configured["provider_binding"] != {"issuer": module.ISSUER, "provider_tenant_id": "ten_" + org}:
            raise AssertionError("provider fixture binding mismatch")
    expected = {(org["organization_id"], seed["app_id"]) for org in setup["organizations"] for seed in org["contract_shape_seeds"]}
    if actual != expected:
        raise AssertionError("seed app scope mismatch")
    result = {"bundles_revalidated": len(bundles), "setup_digest": sha(DEFAULT / "setup.json"), "seed_source_digest": sha(ROOT / "app/runtime/tenant_seed.py"), "database_actions": "none", "guarded_reset": "not exercised by this parser/query campaign; see separate APP-011 disposable reset trial", "scope": "read-only contract/setup alignment, not provider provisioning or live tenant qualification"}
    save(output / "seed-alignment.json", result)
    return result


def run(output, image=None):
    output = Path(output)
    if output.exists() and any(output.iterdir()):
        raise SystemExit("choose a new output directory; retain earlier evidence")
    output.mkdir(parents=True, exist_ok=True)
    summary = {"passed": False, "started_at": datetime.now(timezone.utc).isoformat(), "python": platform.python_version(), "platform": platform.platform(),
               "command": "spikes/APP-005/.venv/bin/python fixtures/qualify.py --output " + str(output.relative_to(ROOT) if output.is_absolute() and output.is_relative_to(ROOT) else output)
               + ("".join(f" --parser-{name} {path}" for name, path in zip(("host", "wasm", "manifest"), image)) if image else "")}
    try:
        manifest = load_package()
        summary["source_pins"] = {str(p.relative_to(ROOT)): sha(p) for p in [*sorted((ROOT / "fixtures").glob("*.py")), ROOT / "spikes/APP-004/Cargo.lock", ROOT / "spikes/APP-004/guest/src/lib.rs", ROOT / "spikes/APP-004/host/src/lib.rs", ROOT / "spikes/APP-005/query.py", ROOT / "spikes/APP-005/requirements.lock", ROOT / "contracts/implementation-lock.json", DEFAULT / "manifest.json"]}
        for name, command in (("regeneration", [sys.executable, "fixtures/generate.py", "--check"]), ("unit-tests", [sys.executable, "-m", "unittest", "discover", "-s", "fixtures", "-v"])):
            result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=30)
            (output / (name + ".log")).write_text(result.stdout + result.stderr)
            if result.returncode:
                raise AssertionError(name + " failed")
        oracle = verify()
        save(output / "oracle.json", oracle)
        reports, pins, binding = parser_checks(output, manifest, image)
        summary["parser_artifacts"] = pins
        summary["parser_binding"] = binding
        summary["parser_cases"] = len(reports)
        summary["query_subset"] = query_checks(output, reports, oracle)
        summary["seed_alignment"] = seed_manifest_checks(output)
        summary["passed"] = True
    except Exception as exc:
        summary["failure"] = {"type": type(exc).__name__, "message": str(exc)}
        raise
    finally:
        summary["finished_at"] = datetime.now(timezone.utc).isoformat()
        summary["limits"] = ["No real app journeys, account provisioning, database reset, publication, model generation, record mutation or restore acceptance", "CIDR/rate/currency are independent oracle checks; unsupported APP-005 operators remain unsupported", "Actual APP-004 accepted files require mapping; acceptance is not source admission", "APP-008 shape seeds intentionally retain placeholder source/admission/receipt data"]
        save(output / "summary.json", summary)
        print(json.dumps({"passed": summary["passed"], "output": str(output)}), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--parser-host", type=Path, help="with --parser-wasm and --parser-manifest: qualify parser bytes extracted from a built image")
    parser.add_argument("--parser-wasm", type=Path)
    parser.add_argument("--parser-manifest", type=Path)
    arguments = parser.parse_args()
    image = (arguments.parser_host, arguments.parser_wasm, arguments.parser_manifest)
    if any(image) and not all(image):
        parser.error("--parser-host, --parser-wasm and --parser-manifest go together")
    run(arguments.output, tuple(path.resolve() for path in image) if all(image) else None)
