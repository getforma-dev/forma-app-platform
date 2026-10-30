"""Unit tests for the APP-075 local proof tools (no Docker, AWS or RDS).

The entry (app/src/storage/managed_qualification.rs) is the single source of
the source list, the seed inputs and their declared pins; these tests keep
local_harness.py and check_image.py reading the same tables.
"""
import hashlib
from pathlib import Path
import re
import sys
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
                          "seed_program_sha256", "seed_validator_sha256"])
        paths = dict(inputs)
        self.assertEqual(paths["seed_northstar_sha256"], "contracts/fixtures/impl-2026-10-28.1/northstar.json")
        self.assertEqual(paths["seed_spend_review_sha256"], "contracts/fixtures/impl-2026-10-28.1/spend-review.json")
        self.assertEqual(paths["seed_program_sha256"], "app/runtime/tenant_seed.py")
        self.assertEqual(paths["seed_validator_sha256"], "contracts/validate.py")

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
        editions = {Path(path).parent.name for field, path in harness.seed_inputs() if field.startswith("seed_") and path.endswith(".json")}
        self.assertEqual(len(editions), 1)
        seed = (harness.APP / "runtime/tenant_seed.py").read_text()
        self.assertIn('CONTRACTS / "fixtures" / "%s" / (name + ".json")' % editions.pop(), seed)


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

def refusal(**changes):
    """A readiness refusal shaped like the entry's (APP-086)."""
    pools = [{"pool": name, "stage": "done", "step": None, "error": None, "sqlstate": None, "cause": None, "elapsed_ms": 40}
             for name in harness.ROLES]
    pools[1] = {"pool": "control", "stage": "acquire", "step": "connect", "error": "Unavailable", "sqlstate": "28P01",
                "cause": "db_error", "elapsed_ms": 12}
    value = {"case": "readiness", "reason": "stage_failed", **pools[1], "pools": pools, "checks_passed": 0}
    value.update(changes)
    return value


class RefusalTests(unittest.TestCase):
    def test_the_harness_reads_the_entry_refusal_members(self):
        block = ENTRY_SOURCE[ENTRY_SOURCE.index("const REFUSAL_KEYS:"):]
        block = block[:block.index("];")]
        self.assertEqual(set(re.findall(r'"(\w+)"', block)), harness.REFUSAL_KEYS)
        for key in harness.POOL_KEYS:
            self.assertIn('"%s":' % key, ENTRY_SOURCE[ENTRY_SOURCE.index("fn probe_value"):])

    def test_a_closed_refusal_names_pool_stage_and_cause(self):
        value = refusal()
        self.assertTrue(harness.closed_refusal(value, [harness.HOST, "genui_", "password"]))
        self.assertTrue(harness.refusal_names(value, case="readiness", pool="control", stage="acquire", step="connect",
                                              error="Unavailable", sqlstate="28P01", cause="db_error"))
        self.assertFalse(harness.refusal_names(None, case="readiness"))
        panic = {key: None for key in harness.REFUSAL_KEYS}
        panic.update(case="scoped_reads", reason="panic", checks_passed=1)
        self.assertTrue(harness.closed_refusal(panic))

    def test_free_text_extra_members_or_forbidden_text_are_refused(self):
        for value in (refusal(cause="password authentication failed for user genui_control"),
                      refusal(stage="acquire " + harness.HOST),
                      refusal(sqlstate="28p01x"),
                      refusal(elapsed_ms=1.5),
                      refusal(elapsed_ms=True),
                      refusal(detail="x"),
                      {k: v for k, v in refusal().items() if k != "pools"},
                      refusal(pools=[{"pool": "control"}]),
                      "readiness"):
            with self.subTest(value=value):
                self.assertFalse(harness.closed_refusal(value))
        self.assertFalse(harness.closed_refusal(refusal(cause="db_error"), ["db_error"]))


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
