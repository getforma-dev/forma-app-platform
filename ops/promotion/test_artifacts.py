import hashlib
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

from ops.promotion import artifacts as a
from ops.promotion.test_candidate import SHA, runs
from ops.promotion.candidate import ci_status


def encoded(value):
    return json.dumps(value, sort_keys=True).encode()


def archive(path, *, architecture="arm64", extra=None, indexed=False, index_platform=True, manifest_padding=0):
    blobs = {}
    def blob(raw, media):
        digest = "sha256:" + hashlib.sha256(raw).hexdigest()
        blobs["blobs/sha256/" + digest[7:]] = raw
        return {"mediaType": media, "size": len(raw), "digest": digest}
    config = blob(encoded({"os": "linux", "architecture": architecture}), a.CONFIG)
    layer = blob(b"synthetic image-layer bytes", next(iter(a.LAYERS)))
    manifest = blob(encoded({"schemaVersion": 2, "mediaType": a.MANIFEST,
        "config": config, "layers": [layer], **({"annotations": {"test": "x" * manifest_padding}} if manifest_padding else {})}), a.MANIFEST)
    child = dict(manifest, platform={"os": "linux", "architecture": architecture}) if index_platform else manifest
    root = blob(encoded({"schemaVersion": 2, "mediaType": a.INDEX, "manifests": [child]}), a.INDEX) if indexed else manifest
    files = {"oci-layout": encoded({"imageLayoutVersion": "1.0.0"}),
        "index.json": encoded({"schemaVersion": 2, "manifests": [root]}), **blobs}
    if extra:
        files.update(extra)
    with tarfile.open(path, "w") as tar:
        for name, data in files.items():
            info = tarfile.TarInfo(name)
            info.size = len(data)
            tar.addfile(info, io.BytesIO(data))
    return config["digest"]


def bundle(directory):
    images = {}
    for name in a.NAMES:
        qualified_id = archive(directory / (name + ".oci.tar"))
        images[name] = dict(a.inspect_oci(directory / (name + ".oci.tar")), qualified_id=qualified_id, id_kind="config")
    parser = {"guest": {"sha256": "c" * 64}, "host": {"sha256": "d" * 64}, "source": {"digest": "e" * 64}}
    for name in ("service", "operator"):
        (directory / ("parser-manifest-" + name + ".json")).write_bytes(encoded(parser))
    identity = {"guest_sha256": parser["guest"]["sha256"], "host_sha256": parser["host"]["sha256"], "source": parser["source"]}
    build = {"candidate_sha": SHA, "passed": True, "checks_passed": True,
        "images": {n: {"id": i["qualified_id"], "architecture": "arm64"} for n, i in images.items()},
        "parser": {"equal": True, "identity": identity, **{n + "_manifest_sha256": a.sha(directory / ("parser-manifest-" + n + ".json")) for n in ("service", "operator")}}}
    (directory / "build.json").write_bytes(encoded(build))
    (directory / "service-qualify.json").write_bytes(encoded({"passed": True, "architecture": "arm64", "image_id": images["service"]["qualified_id"]}))
    for name in ("operator", "qualification"):
        (directory / (name + "-image-check.json")).write_bytes(encoded({"passed": True, "cleanup_complete": True,
            "image_identity": {"id": images[name]["qualified_id"], "architecture": "arm64"}, "service_context": {"base_revision": SHA}}))
    source = {"tree": "b" * 40, "runtime_sha256": "c" * 64, "schema_manifest_sha256": "d" * 64, "deployment_contract_sha256": "e" * 64}
    (directory / "candidate.json").write_bytes(encoded({"candidate_sha": SHA, "source": source, "runtime_changed": True}))
    (directory / "ci.json").write_bytes(encoded({"candidate_sha": SHA, "ready": True, "checks": ci_status(runs(), SHA)[1]}))
    manifest = {"schema_version": 1, "candidate_sha": SHA, "source": source, "images": images,
        "reports": {n: a.sha(directory / n) for n in a.REPORTS}}
    (directory / "manifest.json").write_bytes(encoded(manifest))
    return manifest


class ArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.manifest = bundle(self.root)

    def tearDown(self):
        self.temporary.cleanup()

    def rewrite_report(self, name, change):
        path = self.root / name
        document = json.loads(path.read_bytes())
        change(document)
        path.write_bytes(encoded(document))
        self.manifest["reports"][name] = a.sha(path)
        (self.root / "manifest.json").write_bytes(encoded(self.manifest))

    def test_valid_complete_graph_and_qualifiers(self):
        self.assertEqual(a.validate_bundle(self.root, SHA), self.manifest)

    def test_oversized_registry_manifest_refuses_before_publication(self):
        archive(self.root / "service.oci.tar", manifest_padding=4 * 1024**2)
        with self.assertRaisesRegex(ValueError, "oversized OCI document"):
            a.inspect_oci(self.root / "service.oci.tar")

    def test_stale_operator_or_qualification_report_refused_even_rehashed(self):
        for name in ("operator", "qualification"):
            with self.subTest(name=name):
                self.manifest = bundle(self.root)
                self.rewrite_report(name + "-image-check.json", lambda d: d["image_identity"].update(id="sha256:" + "0" * 64))
                with self.assertRaisesRegex(ValueError, "identity differs"):
                    a.validate_bundle(self.root, SHA)

    def test_stale_source_context_refused_even_rehashed(self):
        self.rewrite_report("operator-image-check.json", lambda d: d["service_context"].update(base_revision="f" * 40))
        with self.assertRaisesRegex(ValueError, "source context"):
            a.validate_bundle(self.root, SHA)

    def test_parser_equality_cannot_be_claimed_by_boolean(self):
        self.rewrite_report("parser-manifest-operator.json", lambda d: d["guest"].update(sha256="f" * 64))
        with self.assertRaisesRegex(ValueError, "parser identities"):
            a.validate_bundle(self.root, SHA)

    def test_private_extra_file_and_symlink_are_refused(self):
        extra = self.root / "private-state.json"
        extra.write_text("{}")
        with self.assertRaisesRegex(ValueError, "unexpected bundle"):
            a.validate_bundle(self.root, SHA)
        extra.unlink()
        p = self.root / "ci.json"
        p.unlink()
        p.symlink_to(self.root / "candidate.json")
        with self.assertRaises(ValueError):
            a.validate_bundle(self.root, SHA)

    def test_foreign_candidate_or_platform_is_refused(self):
        with self.assertRaises(ValueError):
            a.validate_bundle(self.root, "b" * 40)
        archive(self.root / "service.oci.tar", architecture="amd64")
        with self.assertRaisesRegex(ValueError, "ARM64"):
            a.inspect_oci(self.root / "service.oci.tar")

    def test_oci_does_not_extract_or_accept_traversal_or_unreferenced_bytes(self):
        for extra in ({"../outside": b"x"}, {"blobs/sha256/" + hashlib.sha256(b"x").hexdigest(): b"x"}):
            archive(self.root / "service.oci.tar", extra=extra)
            with self.assertRaises(ValueError):
                a.inspect_oci(self.root / "service.oci.tar")
        self.assertFalse((self.root.parent / "outside").exists())

    def test_manifest_root_and_index_root_both_preserve_exact_digest(self):
        for indexed in (False, True):
            archive(self.root / "service.oci.tar", indexed=indexed)
            result = a.inspect_oci(self.root / "service.oci.tar")
            self.assertEqual(result["index_digest"] == result["manifest_digest"], not indexed)

    def test_index_without_publishable_arm_child_platform_is_refused(self):
        archive(self.root / "service.oci.tar", indexed=True, index_platform=False)
        with self.assertRaisesRegex(ValueError, "platform missing"):
            a.inspect_oci(self.root / "service.oci.tar")


if __name__ == "__main__":
    unittest.main()
