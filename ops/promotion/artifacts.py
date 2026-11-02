"""Verify a source-only qualified OCI bundle before any AWS credential use.

Archives are inspected without extraction. Registry promotion must copy these
verified bytes, not rebuild, docker-save, reserialize manifests or change tags.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import tarfile

from .candidate import SHA, require

NAMES = ("service", "qualification", "operator")
DIGEST = re.compile(r"sha256:[0-9a-f]{64}\Z")
INDEX = "application/vnd.oci.image.index.v1+json"
MANIFEST = "application/vnd.oci.image.manifest.v1+json"
CONFIG = "application/vnd.oci.image.config.v1+json"
LAYERS = {"application/vnd.oci.image.layer.v1.tar", "application/vnd.oci.image.layer.v1.tar+gzip",
    "application/vnd.oci.image.layer.v1.tar+zstd"}
REPORTS = ("build.json", "service-qualify.json", "qualification-image-check.json",
    "operator-image-check.json", "parser-manifest-service.json", "parser-manifest-operator.json",
    "candidate.json", "ci.json")


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def json_object(raw):
    def closed(pairs):
        value = {}
        for k, v in pairs:
            require(k not in value, "duplicate JSON key")
            value[k] = v
        return value
    value = json.loads(raw, object_pairs_hook=closed)
    require(isinstance(value, dict), "JSON object required")
    return value


def inspect_oci(path):
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= 8 * 1024**3,
        "invalid OCI archive")
    with tarfile.open(path, mode="r:") as archive:
        members, blobs = {}, {}
        for member in archive:
            require(len(members) < 10000, "too many OCI members")
            name = member.name.rstrip("/") if member.isdir() else member.name
            require(name not in members, "duplicate archive member")
            require(member.isfile() or member.isdir(), "nonregular OCI member")
            if member.isdir():
                require(name in {"blobs", "blobs/sha256"}, "unexpected OCI directory")
            else:
                require(name in {"index.json", "oci-layout"} or re.fullmatch(r"blobs/sha256/[0-9a-f]{64}", name),
                    "unexpected OCI path")
                require(0 <= member.size <= 4 * 1024**3, "OCI member too large")
                if name.startswith("blobs/"):
                    stream = archive.extractfile(member)
                    digest = hashlib.sha256()
                    for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                        digest.update(chunk)
                    require(digest.hexdigest() == name.rsplit("/", 1)[1], "OCI blob hash mismatch")
                    blobs["sha256:" + digest.hexdigest()] = member.size
            members[name] = member

        def document(name):
            require(name in members and members[name].isfile() and members[name].size <= 8 * 1024**2,
                "missing or oversized OCI document")
            return json_object(archive.extractfile(members[name]).read())

        require(document("oci-layout") == {"imageLayoutVersion": "1.0.0"}, "invalid OCI layout")
        layout = document("index.json")
        require(layout.get("schemaVersion") == 2 and len(layout.get("manifests", [])) == 1,
            "one immutable image required")
        seen = set()

        def descriptor(desc, media):
            require(isinstance(desc, dict) and desc.get("mediaType") in media
                and DIGEST.fullmatch(desc.get("digest", "")) and type(desc.get("size")) is int,
                "invalid OCI descriptor")
            digest = desc["digest"]
            require(blobs.get(digest) == desc["size"] and not desc.get("urls"), "unbound OCI descriptor")
            seen.add(digest)
            return "blobs/sha256/" + digest[7:]

        root = layout["manifests"][0]
        next_name = descriptor(root, {INDEX, MANIFEST})
        node = document(next_name)
        if root["mediaType"] == INDEX:
            require(node.get("schemaVersion") == 2 and node.get("mediaType") == INDEX
                and len(node.get("manifests", [])) == 1,
                "one ARM64 child required; attestations are separately bound")
            child = node["manifests"][0]
            require(isinstance(child.get("platform"), dict)
                and child["platform"].get("os") == "linux"
                and child["platform"].get("architecture") == "arm64", "index child platform missing")
            next_name = descriptor(child, {MANIFEST})
            node = document(next_name)
        else:
            child = root
        platform = child.get("platform")
        require(platform is None or (platform.get("os") == "linux" and platform.get("architecture") == "arm64"
            and platform.get("variant", "v8") == "v8"), "foreign OCI platform")
        require(node.get("schemaVersion") == 2 and node.get("mediaType") == MANIFEST, "invalid image manifest")
        config = document(descriptor(node.get("config"), {CONFIG}))
        require(config.get("os") == "linux" and config.get("architecture") == "arm64", "not a Linux ARM64 image")
        layers = node.get("layers")
        require(isinstance(layers, list) and layers, "image layers missing")
        for layer in layers:
            descriptor(layer, LAYERS)
        require(seen == set(blobs), "unreferenced OCI blob")
        return {"archive": path.name, "archive_sha256": sha(path), "size_bytes": path.stat().st_size,
            "index_digest": root["digest"], "manifest_digest": child["digest"],
            "config_digest": node["config"]["digest"], "platform": "linux/arm64", "blobs": blobs}


def validate_bundle(artifact_dir, expected_candidate):
    directory = Path(artifact_dir)
    require(SHA.fullmatch(expected_candidate), "full candidate required")
    manifest = json_object((directory / "manifest.json").read_bytes())
    require(set(manifest) == {"schema_version", "candidate_sha", "source", "images", "reports"}
        and manifest["schema_version"] == 1 and manifest["candidate_sha"] == expected_candidate,
        "foreign or unsupported bundle")
    require(set(manifest["images"]) == set(NAMES) and set(manifest["reports"]) == set(REPORTS), "incomplete bundle")
    require(set(manifest["source"]) == {"tree", "runtime_sha256", "schema_manifest_sha256", "deployment_contract_sha256"}
        and SHA.fullmatch(manifest["source"]["tree"])
        and all(re.fullmatch(r"[0-9a-f]{64}", v) for k, v in manifest["source"].items() if k != "tree"), "invalid source binding")
    allowed = {"manifest.json", *REPORTS, *(n + ".oci.tar" for n in NAMES)}
    require({p.name for p in directory.iterdir()} == allowed, "unexpected bundle file (private data must not be packaged)")
    for name, digest in manifest["reports"].items():
        path = directory / name
        require(path.is_file() and not path.is_symlink() and path.stat().st_size <= 32 * 1024**2
            and sha(path) == digest, "report hash mismatch")
    build = json_object((directory / "build.json").read_bytes())
    require(build.get("candidate_sha") == expected_candidate and build.get("passed") is True
        and build.get("checks_passed") is True and build.get("parser", {}).get("equal") is True,
        "qualification did not pass")
    candidate = json_object((directory / "candidate.json").read_bytes())
    ci = json_object((directory / "ci.json").read_bytes())
    require(candidate.get("candidate_sha") == expected_candidate and candidate.get("source") == manifest["source"]
        and candidate.get("runtime_changed") is True, "candidate binding differs")
    from .candidate import REQUIRED
    checks = ci.get("checks", [])
    require(ci.get("candidate_sha") == expected_candidate and ci.get("ready") is True
        and len(checks) == len(REQUIRED)
        and {c.get("path") for c in checks} == {".github/workflows/" + n for n in REQUIRED}
        and all(c.get("head_sha") == expected_candidate and c.get("event") == "push"
            and c.get("status") == "completed" and c.get("conclusion") == "success" for c in checks), "CI binding differs")
    for name in NAMES:
        image = manifest["images"][name]
        actual = inspect_oci(directory / (name + ".oci.tar"))
        require({k: v for k, v in image.items() if k not in {"qualified_id", "id_kind"}} == actual,
            "OCI identity differs from qualified manifest")
        qualified = build["images"][name]
        require(qualified.get("architecture") == "arm64" and image["qualified_id"] == qualified["id"], "qualified image differs")
        require(image.get("id_kind") in {"config", "index", "manifest"}
            and image["qualified_id"] == image[image["id_kind"] + "_digest"], "image-store identity not bound to OCI graph")
    for name in ("service-qualify.json", "qualification-image-check.json", "operator-image-check.json"):
        require(json_object((directory / name).read_bytes()).get("passed") is True, "image checks failed")
    service = json_object((directory / "service-qualify.json").read_bytes())
    require(service.get("image_id") == build["images"]["service"]["id"] and service.get("architecture") == "arm64",
        "service qualification identity differs")
    for name in ("qualification", "operator"):
        report = json_object((directory / (name + "-image-check.json")).read_bytes())
        require(report.get("image_identity", {}).get("id") == build["images"][name]["id"]
            and report.get("image_identity", {}).get("architecture") == "arm64"
            and report.get("cleanup_complete") is True, "qualification report identity differs")
        contexts = [v for k, v in report.items() if k.endswith("_context") and isinstance(v, dict) and "base_revision" in v]
        require(contexts and all(c["base_revision"] == expected_candidate for c in contexts), "qualification source context differs")
    from ops.beta.image_record import parser_identity
    parsers = {name: json_object((directory / ("parser-manifest-" + name + ".json")).read_bytes()) for name in ("service", "operator")}
    identities = {name: parser_identity(value) for name, value in parsers.items()}
    require(identities["service"] == identities["operator"] == build["parser"]["identity"], "parser identities differ")
    require(all(build["parser"][name + "_manifest_sha256"] == sha(directory / ("parser-manifest-" + name + ".json"))
        for name in parsers), "parser manifest binding differs")
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact-dir", type=Path, required=True)
    parser.add_argument("--candidate", required=True)
    args = parser.parse_args()
    value = validate_bundle(args.artifact_dir, args.candidate)
    print(json.dumps({"candidate_sha": value["candidate_sha"], "manifest_sha256": sha(args.artifact_dir / "manifest.json"), "qualified": True}))


if __name__ == "__main__":
    main()
