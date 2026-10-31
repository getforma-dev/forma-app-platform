#!/usr/bin/env python3
"""The local image record of one candidate (ADR-013 D13.5, `just images`).

After the three images are built and their checks ran, this writes
`<out>/build.json`: each image's local id, size and creation time, and the
parser identity both parser-carrying images hold. The service and operator
images build the sealed parser in the same stage from the same sources
(ADR-009 D9.1); their `parser-manifest.json` files differ only in the
install paths, so the guest and host bytes and the source binding must be
equal (DEPLOY-RUNBOOK 0.3). A difference fails the record.

Local Docker only: `docker image inspect`, `docker create` (never started),
`docker cp`, `docker rm`. No AWS, no registry, no network.

  image_record.py --candidate <sha40> --out <dir> [--checks-passed yes|no]
"""
import argparse
import datetime
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

NAMES = ("service", "qualification", "operator")
# Where each parser-carrying image keeps its manifest (app/Dockerfile, app/operator/Dockerfile).
MANIFESTS = {"service": "/app/parser-manifest.json", "operator": "/operator/bin/parser-manifest.json"}


def docker(*args):
    return subprocess.run(["docker", *args], check=True, capture_output=True, text=True).stdout


def utc(created):
    """Docker's RFC 3339 nanosecond time as whole-second UTC with Z."""
    stamp = datetime.datetime.fromisoformat(created[:19] + "+00:00") if created.endswith("Z") else \
        datetime.datetime.fromisoformat(created).astimezone(datetime.timezone.utc)
    return stamp.strftime("%Y-%m-%dT%H:%M:%SZ")


def manifest_of(image, path, scratch):
    container = docker("create", image).strip()
    try:
        destination = Path(scratch) / (image.replace(":", "_").replace("/", "_") + ".json")
        docker("cp", "%s:%s" % (container, path), str(destination))
        return destination.read_bytes()
    finally:
        docker("rm", container)


def parser_identity(document):
    """What must be equal across images: the bytes and the source binding, not the install paths."""
    return {"guest_sha256": document["guest"]["sha256"], "host_sha256": document["host"]["sha256"],
            "source": document["source"]}


def record(candidate, out, checks_passed):
    images, created = {}, []
    for name in NAMES:
        reference = "genui-%s:pitch-%s" % (name, candidate[:7])
        info = json.loads(docker("image", "inspect", reference))[0]
        images[name] = {"reference": reference, "id": info["Id"], "size_bytes": info["Size"],
                        "architecture": info.get("Architecture"), "created_at_utc": utc(info["Created"])}
        created.append(images[name]["created_at_utc"])
    with tempfile.TemporaryDirectory() as scratch:
        raw = {name: manifest_of(images[name]["reference"], path, scratch) for name, path in MANIFESTS.items()}
    documents = {name: json.loads(data) for name, data in raw.items()}
    identities = {name: parser_identity(document) for name, document in documents.items()}
    equal = identities["service"] == identities["operator"]
    build = {
        "schema_version": 1,
        "candidate_sha": candidate,
        "built_at_utc": max(created),
        "images": images,
        "parser": {"service_manifest_sha256": hashlib.sha256(raw["service"]).hexdigest(),
                   "operator_manifest_sha256": hashlib.sha256(raw["operator"]).hexdigest(),
                   "equal": equal, "identity": identities["service"]},
        "checks_passed": checks_passed,
        "passed": checks_passed and equal,
    }
    out.mkdir(parents=True, exist_ok=True)
    (out / "build.json").write_text(json.dumps(build, indent=2) + "\n")
    for name, data in raw.items():
        (out / ("parser-manifest-%s.json" % name)).write_bytes(data)
    return build


def main(argv=None):
    parser = argparse.ArgumentParser(description=(__doc__ or "").split("\n\n")[0])
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--checks-passed", choices=("yes", "no"), default="yes")
    args = parser.parse_args(argv)
    if len(args.candidate) != 40:
        parser.error("--candidate must be the full commit sha")
    build = record(args.candidate, args.out, args.checks_passed == "yes")
    print("parser manifests: %s (guest %s…, host %s…)" % (
        "equal" if build["parser"]["equal"] else "DIFFER", build["parser"]["identity"]["guest_sha256"][:12],
        build["parser"]["identity"]["host_sha256"][:12]))
    for name in NAMES:
        print("%-13s %s %s bytes" % (name, build["images"][name]["id"][:19], build["images"][name]["size_bytes"]))
    print("build record: %s (%s)" % (args.out / "build.json", "passed" if build["passed"] else "NOT passed"))
    return 0 if build["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
