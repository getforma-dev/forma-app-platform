"""Unit tests for the APP-075 local proof tools (no Docker, AWS or RDS).

The entry (app/src/storage/managed_qualification.rs) is the single source of
the source list, the seed inputs and their declared pins; these tests keep
local_harness.py and check_image.py reading the same tables.
"""
import hashlib
import json
from pathlib import Path
import re
import sys
import tomllib
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_image  # noqa: E402
import local_harness as harness  # noqa: E402

ENTRY_SOURCE = (harness.APP / "src/storage/managed_qualification.rs").read_text()


def sha256(path):
    return hashlib.sha256((harness.REPOSITORY / path).read_bytes()).hexdigest()


def pin_fields():
    block = ENTRY_SOURCE[ENTRY_SOURCE.index("struct Pins {"):]
    block = block[:block.index("}")]
    return re.findall(r"^\s*(\w+):", block, re.MULTILINE)


class SeedInputTests(unittest.TestCase):
    def test_seed_inputs_are_the_entry_table(self):
        inputs = harness.seed_inputs()
        self.assertEqual([field for field, _ in inputs],
                         ["fixture_setup_sha256", "seed_northstar_sha256", "seed_spend_review_sha256",
                          "seed_program_sha256", "seed_validator_sha256", "seed_library_sha256"])
        paths = dict(inputs)
        self.assertEqual(paths["seed_northstar_sha256"], "contracts/fixtures/impl-2026-10-28.1/northstar.json")
        self.assertEqual(paths["seed_spend_review_sha256"], "contracts/fixtures/impl-2026-10-28.1/spend-review.json")
        self.assertEqual(paths["seed_program_sha256"], "app/runtime/tenant_seed.py")
        self.assertEqual(paths["seed_validator_sha256"], "contracts/validate.py")
        self.assertEqual(paths["seed_library_sha256"], "contracts/fixtures/library/catalog.json")

    def test_expected_pins_recompute_every_entry_pin_from_the_seeded_files(self):
        pins = harness.expected_pins()
        self.assertEqual(sorted(pins), sorted(pin_fields()))
        for field, path in harness.seed_inputs():
            with self.subTest(field=field):
                self.assertEqual(pins[field], sha256(path))
                declared = re.search(r'"%s",\s*"%s",\s*"([0-9a-f]{64})"' % (field, re.escape(path)), ENTRY_SOURCE)
                self.assertIsNotNone(declared)
                self.assertEqual(pins[field], declared.group(1), "declared pin differs from the repository file")

    def test_the_seed_reads_the_pinned_edition(self):
        editions = {Path(path).parent.name for field, path in harness.seed_inputs()
                    if field.startswith("seed_") and path.endswith(".json") and "/fixtures/impl-" in path}
        self.assertEqual(len(editions), 1)
        edition = editions.pop()
        seed = (harness.APP / "runtime/tenant_seed.py").read_text()
        self.assertIn('CONTRACTS / "fixtures" / "%s" / (name + ".json")' % edition, seed)
        # ADR-011 library: the catalog pin names every library fixture (by digest) and the same edition.
        catalog = json.loads((harness.APP.parent / dict(harness.seed_inputs())["seed_library_sha256"]).read_text())
        self.assertEqual((catalog["edition"], catalog["synthetic"]), (edition, True))
        for entry in catalog["apps"]:
            self.assertEqual(sha256("contracts/fixtures/library/" + entry["fixture"]), entry["fixture_sha256"])


class SourceTests(unittest.TestCase):
    def test_source_names_include_compile_time_inputs(self):
        names = harness.source_names()
        self.assertEqual(names, sorted(names))
        for name in ("app/certs/rds-global-bundle.pem", "contracts/implementation-lock.json",
                     "contracts/fixtures/impl-2026-10-28.1/northstar.json", "spikes/APP-006/src/lib.rs"):
            self.assertIn(name, names)
        for name in names:
            self.assertTrue((harness.REPOSITORY / name).is_file(), name)


    def test_the_native_stage_copies_every_compile_time_input(self):
        """APP-078 step 2 rehearsal (2026-10-30): the image built without
        contracts/schema.json although the context allowed it. Every source the
        entry binds must sit under some COPY source of the native stage."""
        dockerfile = (harness.APP / "qualification/Dockerfile").read_text()
        native = dockerfile[dockerfile.index(" AS native"):]
        copied = [src for line in native.splitlines() if line.startswith("COPY ") and "--from=" not in line
                  for src in line.split()[1:-1]]
        for name in harness.source_names():
            with self.subTest(source=name):
                self.assertTrue(any(name == src or name.startswith(src.rstrip("/") + "/") for src in copied),
                                "%s is a compile-time input the native stage never copies" % name)

def service_allowlist():
    """The files the service image's context (the repository-root .dockerignore) admits."""
    return check_image.allowlist((harness.REPOSITORY / ".dockerignore").read_text())


def service_native_copies():
    """The context-relative COPY sources of app/Dockerfile's native stage."""
    dockerfile = (harness.APP / "Dockerfile").read_text()
    native = dockerfile[dockerfile.index(" AS native"):]
    native = native[:native.index("\nFROM ")]
    return [src for line in native.splitlines() if line.startswith("COPY ") and "--from=" not in line
            for src in line.split()[1:-1]]


def compiled_rust_sources():
    """Every .rs file under app/src that a release build compiles: the module tree from lib.rs
    and main.rs minus every `#[cfg(test)]` or `#[cfg(feature = "…")]` module declaration (and its
    `#[path]`), and minus every `[[bin]]` with `required-features`. The Dockerfile enables no
    feature (ADR-007 D7.13, ADR-012 D12.1: `dev-stand` builds genui-dev only locally; APP-131)."""
    root = harness.APP / "src"
    declaration = re.compile(r'(#\[cfg\((?:test|feature = "[\w-]+")\)\]\s*)?(?:#\[path = "([^"]+)"\]\s*)?(?:pub(?:\(crate\))? )?mod (\w+);')

    def declared(path, text, tests_only):
        module_dir = path.parent if path.name in ("mod.rs", "lib.rs", "main.rs") else path.parent / path.stem
        for match in declaration.finditer(text):
            gated, custom, name = match.group(1), match.group(2), match.group(3)
            if tests_only or gated:
                if custom:
                    yield (path.parent / custom).resolve()
                else:
                    yield (module_dir / (name + ".rs")).resolve()
                    yield (module_dir / name).resolve()

    test_only = set()
    for path in sorted(root.rglob("*.rs")):
        test_only.update(declared(path, path.read_text(), False))
    # A module declared inside a test-only file (any gate) is test-only too, transitively.
    pending = [p for p in test_only if p.is_file()]
    while pending:
        path = pending.pop()
        for child in declared(path, path.read_text(), True):
            if child not in test_only:
                test_only.add(child)
                if child.is_file():
                    pending.append(child)
    manifest = tomllib.loads((harness.APP / "Cargo.toml").read_text())
    test_only.update((harness.APP / binary["path"]).resolve() for binary in manifest.get("bin", [])
                     if binary.get("required-features"))
    compiled = []
    for path in sorted(root.rglob("*.rs")):
        resolved = path.resolve()
        if resolved in test_only or any(parent in test_only for parent in resolved.parents):
            continue
        compiled.append(path.relative_to(harness.REPOSITORY).as_posix())
    return compiled


def release_includes(source):
    """The repository-relative targets of include_str!/include_bytes! outside the file's inline
    `#[cfg(test)] mod` block (conventionally the file's tail)."""
    text = (harness.REPOSITORY / source).read_text()
    cut = re.search(r"#\[cfg\(test\)\]\s*(?:pub(?:\(crate\))? )?mod \w+ \{", text)
    if cut:
        text = text[:cut.start()]
    targets = []
    for match in re.finditer(r'include_(?:str|bytes)!\(\s*"([^"]+)"', text):
        target = ((harness.REPOSITORY / source).parent / match.group(1)).resolve()
        targets.append(target.relative_to(harness.REPOSITORY).as_posix())
    return targets


class ServiceImageTests(unittest.TestCase):
    """ADR-011 pitch integration (APP-096): the service image (app/Dockerfile, built from the
    repository root under its .dockerignore) must carry every file the release build compiles
    and every file that build embeds at compile time. The library agent found the catalog
    sidecar (storage/queries.rs) and the composer fixtures (composer/prompt.rs) missing."""

    def test_compiled_sources_are_derived_from_the_module_tree(self):
        compiled = compiled_rust_sources()
        for name in ("app/src/lib.rs", "app/src/main.rs", "app/src/query/canvas.rs", "app/src/composer/prompt.rs",
                     "app/src/storage/queries.rs", "app/src/events/routes.rs"):
            self.assertIn(name, compiled)
        for name in ("app/src/query/canvas_tests.rs", "app/src/query/tests.rs", "app/src/composer/tests.rs",
                     "app/src/storage/qualification.rs", "app/src/storage/managed_qualification/tests.rs",
                     "app/src/auth_qualification/tests.rs", "app/src/admission/tests.rs"):
            self.assertNotIn(name, compiled)
        # Feature-gated code (dev-stand: genui-dev and its module) is not in a release build.
        for name in ("app/src/bin/genui-dev.rs", "app/src/dev/mod.rs", "app/src/dev/seed.rs"):
            self.assertNotIn(name, compiled)

    def test_service_context_admits_every_compiled_source_and_only_tracked_files(self):
        import subprocess
        names = set(service_allowlist())
        tracked = set(subprocess.run(["git", "ls-files", "--cached", "--others", "--exclude-standard"], cwd=harness.REPOSITORY,
                                     capture_output=True, text=True, check=True).stdout.splitlines())
        self.assertEqual(sorted(names - tracked), [], "allowlisted but not tracked")
        self.assertEqual(sorted(set(compiled_rust_sources()) - names), [], "compiled by the release build, absent from the context")
        # The browser stage bundles app/admin/src whole: every tracked source there is in the context.
        admin = {name for name in tracked if name.startswith("app/admin/src/") and not name.endswith(".md")}
        self.assertEqual(sorted(admin - names), [], "admin sources the browser stage cannot see")
        for required in ("app/runtime/runtime.py", "app/runtime/query_engine.py", "app/runtime/requirements.lock",
                         "app/migrations/manifest.json", "app/certs/rds-global-bundle.pem",
                         "contracts/fixtures/library/catalog.json", "contracts/fixtures/impl-2026-10-28.1/northstar.json"):
            self.assertIn(required, names)
        for excluded in ("app/runtime/admission_operator.py", "app/runtime/test_runtime.py", "app/operator/operator.py",
                         "contracts/schema.json", "contracts/fixtures/library/expense-review.json"):
            self.assertNotIn(excluded, names)
        migrations = sorted("app/migrations/" + p.name for p in (harness.APP / "migrations").iterdir() if p.is_file())
        self.assertEqual(sorted(name for name in names if name.startswith("app/migrations/")), migrations)

    def test_the_service_native_stage_copies_every_compile_time_input(self):
        """Every include_str!/include_bytes! of a compiled source must sit under some COPY source
        of app/Dockerfile's native stage and be admitted by the context."""
        copied = service_native_copies()
        names = set(service_allowlist())
        includes = {source: release_includes(source) for source in compiled_rust_sources()}
        flat = sorted({target for targets in includes.values() for target in targets})
        for required in ("contracts/fixtures/library/catalog.json", "contracts/fixtures/impl-2026-10-28.1/northstar.json",
                         "contracts/fixtures/impl-2026-10-28.1/spend-review.json", "app/migrations/catalog-query.json"):
            self.assertIn(required, flat)
        # The inline test module's include (storage/definitions.rs, edition 2 fixture) is not a release input.
        self.assertNotIn("contracts/fixtures/spend-review.json", flat)
        for source, targets in includes.items():
            for target in targets:
                with self.subTest(source=source, target=target):
                    self.assertTrue((harness.REPOSITORY / target).is_file(), target)
                    self.assertIn(target, names, "%s is embedded by %s but the context excludes it" % (target, source))
                    self.assertTrue(any(target == src or target.startswith(src.rstrip("/") + "/") for src in copied),
                                    "%s is a compile-time input the native stage never copies" % target)
        # Every COPY source exists in the context, so the build cannot fail on a missing path.
        for src in copied:
            with self.subTest(copy=src):
                self.assertTrue(src in names or any(name.startswith(src.rstrip("/") + "/") for name in names), src)


class AllowlistTests(unittest.TestCase):
    def test_directory_reincludes_are_not_files(self):
        names = check_image.allowlist()
        self.assertFalse([name for name in names if name.endswith("/")])
        self.assertIn("app/src/imports/mod.rs", names)
        self.assertIn("contracts/fixtures/impl-2026-10-28.1/northstar.json", names)
        self.assertTrue(set(harness.source_names()) <= set(names))

    def test_a_directory_reinclude_must_be_closed_by_its_exclusion(self):
        self.assertEqual(check_image.allowlist("**\n!a/\na/**\n!a/b.rs\n!c.rs\n"), ["a/b.rs", "c.rs"])
        for text in ("**\n!a/\n!a/b.rs\n", "**\n!a/\nb/**\n!a/b.rs\n", "**\n!a/\n"):
            with self.subTest(text=text):
                with self.assertRaises(ValueError):
                    check_image.allowlist(text)


if __name__ == "__main__":
    unittest.main()
