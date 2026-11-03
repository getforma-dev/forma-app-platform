"""One native ARM64 build per image, two exporters from that same result.

Requires a docker-container Buildx builder. Runs without AWS/OIDC credentials.
Only explicitly selected source/synthetic qualification files enter the bundle.
"""
import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess

from .artifacts import NAMES, REPORTS, inspect_oci, sha, validate_bundle
from .candidate import SHA, git, require, source_binding


def run(command, log):
    with log.open("wb") as stream:
        result = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT)
    require(result.returncode == 0, "qualification failed; inspect local " + log.name)


def build(candidate, output, input_dir, logs):
    require(SHA.fullmatch(candidate) and git("rev-parse", "HEAD") == candidate, "candidate checkout mismatch")
    require(not git("status", "--porcelain", "--untracked-files=no"), "dirty candidate checkout")
    require(platform.machine().lower() in {"arm64", "aarch64"}, "native ARM64 runner required")
    require(not any(os.environ.get(k) for k in ("AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_SESSION_TOKEN",
        "ACTIONS_ID_TOKEN_REQUEST_URL", "ACTIONS_ID_TOKEN_REQUEST_TOKEN")), "build must not have AWS or OIDC credentials")
    require(not output.exists(), "bundle output already exists; never overwrite a qualified attempt")
    output.mkdir(parents=True)
    logs.mkdir(parents=True, exist_ok=True)
    for name in ("candidate.json", "ci.json"):
        shutil.copyfile(input_dir / name, output / name)
    for name, dockerfile in (("service", "app/Dockerfile"), ("qualification", "app/qualification/Dockerfile"),
            ("operator", "app/operator/Dockerfile")):
        tag = "genui-" + name + ":pitch-" + candidate[:7]
        # Multiple exporters consume ONE BuildKit build result. Provenance is in
        # the separately hashed qualification bundle, not an unqualified extra image.
        run(["docker", "buildx", "build", "--platform", "linux/arm64", "--provenance=false", "--sbom=false",
            "--file", dockerfile, "--tag", tag,
            "--output", "type=oci,dest=" + str(output / (name + ".oci.tar")),
            "--output", "type=docker", "."], logs / ("build-" + name + ".log"))
    runtime = "app/runtime/.venv/bin/python"
    operator = "app/operator/.venv/bin/python"
    commands = [
        [runtime, "app/runtime/qualify_image.py", "--image", "genui-service:pitch-" + candidate[:7],
            "--output", str(output / "service-qualify.json")],
        [runtime, "app/qualification/check_image.py", "--image", "genui-qualification:pitch-" + candidate[:7],
            "--base-revision", candidate, "--output", str(output / "qualification-image-check.json")],
        [operator, "-P", "app/operator/check_image.py", "--image", "genui-operator:pitch-" + candidate[:7],
            "--base-revision", candidate, "--output", str(output / "operator-image-check.json")],
        [runtime, "-I", "ops/beta/image_record.py", "--candidate", candidate, "--out", str(output), "--checks-passed", "yes"],
    ]
    for number, command in enumerate(commands):
        run(command, logs / ("qualify-" + str(number) + ".log"))
    record = json.loads((output / "build.json").read_bytes())
    images = {}
    for name in NAMES:
        image = inspect_oci(output / (name + ".oci.tar"))
        info = json.loads(subprocess.check_output(["docker", "image", "inspect", record["images"][name]["reference"]]))[0]
        image_id = record["images"][name]["id"]
        require(info["Id"] == image_id, "local qualified image changed")
        kinds = [k for k in ("config", "index", "manifest") if image_id == image[k + "_digest"]]
        require(kinds, "qualified image is not the exported OCI graph")
        kind = kinds[0]
        if kind != "config":
            require(info.get("Descriptor", {}).get("digest") == image_id, "image-store Descriptor mismatch")
        images[name] = dict(image, qualified_id=image_id, id_kind=kind)
    manifest = {"schema_version": 1, "candidate_sha": candidate, "source": source_binding(), "images": images,
        "reports": {name: sha(output / name) for name in REPORTS}}
    (output / "manifest.json").write_text(json.dumps(manifest, sort_keys=True, indent=2) + "\n")
    validate_bundle(output, candidate)
    with open(os.environ["GITHUB_OUTPUT"], "a") as stream:
        stream.write("manifest_sha256=" + sha(output / "manifest.json") + "\n")
    print("Qualified three exact ARM64 OCI graphs for " + candidate)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--input-dir", type=Path, required=True)
    parser.add_argument("--logs", type=Path, required=True)
    args = parser.parse_args()
    build(args.candidate, args.output.resolve(), args.input_dir, args.logs)


if __name__ == "__main__":
    main()
