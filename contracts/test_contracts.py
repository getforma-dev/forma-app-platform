import copy
import hashlib
import json
import unittest
from pathlib import Path

from schema import SCHEMA as SOURCE_SCHEMA
from validate import ContractError, SCHEMA, canonical, digest, loads, payload_digest, validate, validate_bundle

ROOT = Path(__file__).resolve().parent


def fixture(name="northstar"):
    return loads((ROOT / "fixtures" / (name + ".json")).read_bytes())


class ContractTests(unittest.TestCase):
    def test_schema_source_matches_published_artifact(self):
        self.assertEqual(SOURCE_SCHEMA, SCHEMA)

    def test_two_distinct_apps_share_contract(self):
        for name in ("northstar", "spend-review"):
            with self.subTest(name=name):
                validate_bundle(fixture(name))
        self.assertNotEqual(fixture()["definition"]["digest"], fixture("spend-review")["definition"]["digest"])

    def test_negative_fixture_vectors(self):
        vectors = json.loads((ROOT / "fixtures" / "negative-vectors.json").read_text())
        for vector in vectors:
            with self.subTest(case=vector["name"]):
                bundle = fixture(vector.get("fixture", "northstar"))
                target = bundle
                path = vector["path"]
                for key in path[:-1]:
                    target = target[key]
                if vector.get("operation") == "remove":
                    del target[path[-1]]
                else:
                    target[path[-1]] = vector["value"]
                # Preserve cryptographic validity when isolating a semantic refusal.
                if vector.get("rehash_definition"):
                    bundle["definition"]["digest"] = digest("app-definition", bundle["definition"]["content"])
                if vector.get("rehash_receipt"):
                    receipt = bundle["receipt"]
                    receipt["digest"] = digest("query-receipt", {k: v for k, v in receipt.items() if k != "digest"})
                with self.assertRaisesRegex(ContractError, vector["expected"]):
                    validate_bundle(bundle)

    def test_valid_owner_publication_policy(self):
        bundle = fixture()
        bundle["publication"]["policy"]["kind"] = "owner_publish"
        bundle["publication"]["approver_id"] = bundle["publication"]["author_id"]
        validate_bundle(bundle)

    def test_all_registered_renderer_tags_have_shape(self):
        definition = fixture()["definition"]
        for kind in ("summary", "table", "detail", "timeline", "chart", "filter", "status"):
            with self.subTest(kind=kind):
                value = copy.deepcopy(definition)
                value["content"]["views"][0]["kind"] = kind
                value["digest"] = digest("app-definition", value["content"])
                validate(value)

    def test_parameter_values_are_not_executable(self):
        bundle = fixture()
        text = "<script>alert(1)</script>; SELECT * FROM secrets; $(touch /tmp/pwned)"
        bundle["command"]["values"][1]["value"]["value"] = text
        bundle["outcome"]["payload_digest"] = payload_digest(bundle["command"])
        validate_bundle(bundle)
        self.assertEqual(bundle["command"]["values"][1]["value"]["value"], text)

    def test_command_payload_hash_binds_versions_and_values_not_retry_key(self):
        original = fixture()["command"]
        new_key = copy.deepcopy(original)
        new_key["idempotency_key"] = "fedcba9876543210fedcba9876543210"
        self.assertEqual(payload_digest(original), payload_digest(new_key))
        for key, value in (("expected_record_version", 1), ("expected_work_version", 2)):
            changed = copy.deepcopy(original)
            changed[key] = value
            self.assertNotEqual(payload_digest(original), payload_digest(changed))

    def test_complete_job_requires_result_failure_requires_reason(self):
        example = {"kind": "job_state", "schema_version": 1, "organization_id": fixture()["context"]["organization_id"], "job_id": "00000000-0000-4000-8000-000000000777", "state": "queued", "claim_sequence": 0}
        validate(example)
        example["state"] = "done"
        with self.assertRaisesRegex(ContractError, "done iff"):
            validate(example)
        example["result_digest"] = digest("result", {"example": True})
        validate(example)
        example["state"] = "failed"
        del example["result_digest"]
        with self.assertRaisesRegex(ContractError, "failed iff"):
            validate(example)
        example["failure_code"] = "resource_limit"
        validate(example)

    def test_preview_receipt_has_no_publication_claim(self):
        receipt = fixture()["receipt"]
        receipt["mode"] = "synthetic_preview"
        del receipt["publication_id"]
        receipt["digest"] = digest("query-receipt", {k: v for k, v in receipt.items() if k != "digest"})
        validate(receipt)

    def test_definition_byte_cap(self):
        value = fixture()["definition"]
        content = value["content"]
        # Twenty-four independent note views make large valid metadata without executable fields.
        content["views"] = [{"id": f"note_{i}", "kind": "note", "renderer_version": 1, "title": "x" * 2000, "text": "y" * 2000} for i in range(24)]
        content["default_layout"] = [{"view_id": f"note_{i}", "row": i, "column": 0, "width": 12} for i in range(24)]
        # Four-byte Unicode proves byte accounting differs from character counts.
        for view in content["views"]:
            view["text"] = "😀" * 2000
            view["title"] = "😀" * 2000
        value["digest"] = digest("app-definition", content)
        with self.assertRaisesRegex(ContractError, "256 KiB"):
            validate(value)

    def test_half_open_window_requires_increasing_valid_instants(self):
        value = fixture()["definition"]
        window = {"start": "2026-10-28T10:00:00Z", "end": "2026-10-28T10:10:00Z", "bounds": "half_open"}
        value["content"]["views"][0]["query"]["window"] = window
        value["digest"] = digest("app-definition", value["content"])
        validate(value)
        for end in ("2026-10-28T10:00:00Z", "2026-10-28T09:59:59Z"):
            window["end"] = end
            value["digest"] = digest("app-definition", value["content"])
            with self.assertRaisesRegex(ContractError, "time window"):
                validate(value)


class CanonicalTests(unittest.TestCase):
    def test_golden_vectors(self):
        for vector in json.loads((ROOT / "fixtures" / "canonical-vectors.json").read_text()):
            with self.subTest(name=vector["name"]):
                self.assertEqual(canonical(vector["value"]).hex(), vector["utf8_hex"])
                self.assertEqual(digest("result", vector["value"]), vector["digest"])

    def test_key_order_does_not_change_digest(self):
        self.assertEqual(digest("result", {"b": 2, "a": 1}), digest("result", {"a": 1, "b": 2}))

    def test_array_order_domains_and_unicode_meaning_preserved(self):
        self.assertNotEqual(digest("result", [1, 2]), digest("result", [2, 1]))
        self.assertNotEqual(digest("result", {"a": 1}), digest("query", {"a": 1}))
        self.assertNotEqual(canonical("é"), canonical("e\u0301"))

    def test_ambiguous_json_refusals(self):
        for raw in ('{"a":1,"a":2}', '{"a":1.0}', '{"a":1e2}', '{"a":NaN}', '{"a":Infinity}', '{"a":9007199254740992}', '{"é":1}', '{"a":"\\ud800"}'):
            with self.subTest(raw=raw), self.assertRaises((ContractError, UnicodeError)):
                loads(raw)

    def test_depth_limit(self):
        value = None
        for _ in range(34):
            value = [value]
        with self.assertRaisesRegex(ContractError, "nesting"):
            canonical(value)
        with self.assertRaisesRegex(ContractError, "nesting"):
            loads("[" * 2000 + "null" + "]" * 2000)

    def test_floats_and_unregistered_domains_refused(self):
        with self.assertRaises(ContractError):
            canonical({"a": 0.1})
        with self.assertRaisesRegex(ContractError, "unregistered"):
            digest("arbitrary", {})


if __name__ == "__main__":
    unittest.main(verbosity=2)
