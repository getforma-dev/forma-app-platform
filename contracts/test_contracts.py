import copy
import hashlib
import json
import unittest
from pathlib import Path

import make_fixtures
from schema import SCHEMA as SOURCE_SCHEMA
from validate import (ContractError, SCHEMA, canonical, digest, domain_name, exact_rational, loads, payload_digest, validate,
                      validate_bundle, validate_exact_rational, validate_problem)

ROOT = Path(__file__).resolve().parent
RETAINED = ROOT / "fixtures" / "impl-2026-10-28.1"
# The fixture identities edition impl-2026-10-28.1 locked; its copies are retained byte-identically.
EDITION_ONE_FIXTURES = {
    "canonical-vectors": "33b46ff5970bdcd5259f23bdb5e9a7fcda4c5eeffebe93e0fedfebf5f661203d",
    "negative-vectors": "089e9a3017adda324a2a150fb68d5aa497889cd104d2f7096b9128d520764e75",
    "northstar": "45639ea369193d60f45fbafbe6c3aad648881017df57615bdf89bd501dfdd592",
    "spend-review": "4a4581f5a7fcfec867070584a00b0fbebdf33558fffe538eec35f97f72aa5650",
}


def fixture(name="northstar"):
    return loads((ROOT / "fixtures" / (name + ".json")).read_bytes())


def retained(name):
    return loads((RETAINED / (name + ".json")).read_bytes())


def rehash_receipt(receipt):
    receipt["digest"] = digest("query-receipt", {k: v for k, v in receipt.items() if k != "digest"})


def run_negative_vectors(case, vectors, load):
    for vector in vectors:
        with case.subTest(case=vector["name"]):
            bundle = load(vector.get("fixture", "northstar"))
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
                rehash_receipt(bundle["receipt"])
            with case.assertRaisesRegex(ContractError, vector["expected"]):
                validate_bundle(bundle)


class ContractTests(unittest.TestCase):
    def test_schema_source_matches_published_artifact(self):
        self.assertEqual(SOURCE_SCHEMA, SCHEMA)

    def test_two_distinct_apps_share_contract(self):
        for name in ("northstar", "spend-review"):
            with self.subTest(name=name):
                validate_bundle(fixture(name))
        self.assertNotEqual(fixture()["definition"]["digest"], fixture("spend-review")["definition"]["digest"])

    def test_negative_fixture_vectors(self):
        run_negative_vectors(self, json.loads((ROOT / "fixtures" / "negative-vectors.json").read_text()), fixture)

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


class EditionTwoTests(unittest.TestCase):
    """Edition impl-2026-10-29.2 (ADR-009 D9.6) over the retained edition impl-2026-10-28.1."""

    def test_edition_two_retains_every_edition_one_vector(self):
        for name, identity in EDITION_ONE_FIXTURES.items():
            with self.subTest(fixture=name):
                self.assertEqual(hashlib.sha256((RETAINED / (name + ".json")).read_bytes()).hexdigest(), identity)
        current = json.loads((ROOT / "fixtures" / "canonical-vectors.json").read_text())
        for vector in json.loads((RETAINED / "canonical-vectors.json").read_text()):
            with self.subTest(vector=vector["name"]):
                self.assertIn(vector, current)
                self.assertEqual(canonical(vector["value"]).hex(), vector["utf8_hex"])
                self.assertEqual(digest("result", vector["value"]), vector["digest"])
        for name in ("northstar", "spend-review"):
            with self.subTest(bundle=name):
                bundle = retained(name)
                validate_bundle(bundle)
                self.assertEqual(bundle["definition"]["schema_version"], 1)
                self.assertEqual(bundle["definition"]["digest"], digest("app-definition", bundle["definition"]["content"]))
        run_negative_vectors(self, json.loads((RETAINED / "negative-vectors.json").read_text()), retained)
        # An edition-one job state still validates; edition two never reinterprets it.
        validate({"kind": "job_state", "schema_version": 1, "organization_id": retained("northstar")["context"]["organization_id"],
                  "job_id": "00000000-0000-4000-8000-000000000777", "state": "queued", "claim_sequence": 0})

    def test_exact_rational_lowest_terms_and_38_digit_bound(self):
        self.assertEqual(exact_rational(1140000, 10), {"type": "exact_rational", "numerator": "114000", "denominator": "1"})
        self.assertEqual(exact_rational(6, -4), {"type": "exact_rational", "numerator": "-3", "denominator": "2"})
        self.assertEqual(exact_rational(0, -7), {"type": "exact_rational", "numerator": "0", "denominator": "1"})
        largest = 10 ** 38 - 1
        for value in (exact_rational(largest, 1), exact_rational(-largest, largest - 1), exact_rational(1, largest)):
            validate_exact_rational(value)
        with self.assertRaisesRegex(ContractError, "zero denominator"):
            exact_rational(1, 0)
        for numerator, denominator in ((10 ** 38, 1), (1, 10 ** 38), (-(10 ** 38), 3)):
            with self.subTest(numerator=numerator, denominator=denominator), self.assertRaisesRegex(ContractError, "38 digits"):
                exact_rational(numerator, denominator)
        for value, expected in (({"numerator": "2", "denominator": "4"}, "lowest terms"), ({"numerator": "0", "denominator": "2"}, "lowest terms"),
                                ({"numerator": "-6", "denominator": "9"}, "lowest terms"), ({"numerator": "1", "denominator": "0"}, "schema:"),
                                ({"numerator": "1", "denominator": "-2"}, "schema:"), ({"numerator": "-0", "denominator": "1"}, "schema:"),
                                ({"numerator": "1" * 39, "denominator": "1"}, "schema:"), ({"numerator": 1, "denominator": "1"}, "schema:"),
                                ({"numerator": "1", "denominator": "1", "unit": "byte"}, "schema:")):
            with self.subTest(value=value), self.assertRaisesRegex(ContractError, expected):
                validate_exact_rational({"type": "exact_rational", **value})
        vectors = [v for v in json.loads((ROOT / "fixtures" / "canonical-vectors.json").read_text()) if "rational" in v]
        self.assertGreaterEqual(len(vectors), 3)
        for vector in vectors:
            with self.subTest(vector=vector["name"]):
                self.assertEqual(exact_rational(int(vector["rational"]["numerator"]), int(vector["rational"]["denominator"])), vector["value"])
                validate_exact_rational(vector["value"])

    def test_job_state_cancelled_and_queue_fields(self):
        org = fixture()["context"]["organization_id"]
        queued = {"kind": "job_state", "schema_version": 2, "organization_id": org, "job_id": "00000000-0000-4000-8000-000000000777",
                  "state": "queued", "claim_sequence": 0, "cancel_requested": False, "queue_position": 1, "expected_wait_seconds": 30}
        validate(queued)

        def variant(**changes):
            value = {k: v for k, v in queued.items() if k not in ("queue_position", "expected_wait_seconds")}
            value.update(changes)
            return value
        validate(variant(state="running", claim_sequence=1))
        validate(variant(state="running", claim_sequence=1, cancel_requested=True))
        validate(variant(state="cancelled", cancel_requested=True))
        validate(variant(state="cancelled", claim_sequence=1, cancel_requested=True))
        validate(variant(state="done", claim_sequence=1, cancel_requested=True, result_digest=digest("result", {"example": True})))
        for code in ("formula_refused", "macro_refused", "time_limit", "worker_lost", "authority_revoked", "resource_limit"):
            validate(variant(state="failed", claim_sequence=1, failure_code=code))
        # Claimed but not yet started (D9.4 steps 2-3): still queued, no position, a cancel request may be pending.
        validate(variant(claim_sequence=1))
        validate(variant(claim_sequence=1, cancel_requested=True))
        for value, expected in ((variant(queue_position=1), "travel together"),
                                (variant(state="running", claim_sequence=1, queue_position=2, expected_wait_seconds=60), "only a queued job"),
                                (variant(state="cancelled"), "cancelled requires a cancel request"),
                                (variant(state="cancelled", cancel_requested=True, result_digest=digest("result", {})), "done iff"),
                                (variant(state="cancelled", cancel_requested=True, failure_code="cancelled"), "failed iff"),
                                ({**queued, "queue_position": 0}, "schema:"), ({**queued, "expected_wait_seconds": -1}, "schema:"),
                                ({k: v for k, v in queued.items() if k != "cancel_requested"}, "schema:"),
                                ({**variant(state="cancelled", cancel_requested=True), "schema_version": 1}, "schema:"),
                                (variant(state="failed", claim_sequence=1, failure_code="Formula Refused"), "schema:")):
            with self.subTest(value=value), self.assertRaisesRegex(ContractError, expected):
                validate(value)
        payloads = [v for v in json.loads((ROOT / "fixtures" / "canonical-vectors.json").read_text()) if v.get("domain") == "forma:job-payload:v1"]
        self.assertEqual(len(payloads), 1)
        self.assertEqual(digest("job-payload", payloads[0]["value"]), payloads[0]["digest"])
        self.assertNotEqual(digest("job-payload", payloads[0]["value"]), digest("result", payloads[0]["value"]))

    def test_receipt_operator_and_renderer_identity_lists(self):
        bundle = fixture()
        content, publication, receipt = bundle["definition"]["content"], bundle["publication"], bundle["receipt"]
        self.assertEqual((bundle["definition"]["schema_version"], publication["schema_version"], receipt["schema_version"]), (2, 2, 2))
        used = sorted({op["type"] for view in content["views"] if "query" in view for op in view["query"]["operations"]})
        self.assertEqual(publication["operators"], [{"id": name, "version": 1} for name in used])
        self.assertEqual(publication["renderers"], [{"kind": kind, "version": 1} for kind in sorted({view["kind"] for view in content["views"]})])
        self.assertEqual((publication["operator_version"], publication["renderer_version"], receipt["operator_version"]), (1, 1, 1))
        view = next(view for view in content["views"] if view["id"] == receipt["view_id"])
        self.assertEqual(receipt["operators"], [{"id": name, "version": 1} for name in sorted({op["type"] for op in view["query"]["operations"]})])
        self.assertEqual(receipt["renderers"], [{"kind": view["kind"], "version": 1}])
        sources = {source["revision_id"]: source for source in bundle["sources"]}
        for entry in receipt["source_revisions"]:
            self.assertEqual(entry["mapping_version"], sources[entry["source_revision_id"]]["mapping_version"])
        self.assertFalse(receipt["partial"])
        validate_bundle(bundle)
        # A registry edition above one is a valid publication shape; a zero version is not.
        validate({**publication, "operator_version": 2, "renderer_version": 3})
        for key in ("operator_version", "renderer_version"):
            with self.subTest(key=key), self.assertRaisesRegex(ContractError, "schema:"):
                validate({**publication, key: 0})
        # An edition-two definition never travels with an edition-one publication or receipt (no lists, no partial).
        older = fixture()
        older['publication'] = {k: v for k, v in older['publication'].items() if k not in ('operators', 'renderers')} | {'schema_version': 1}
        with self.assertRaisesRegex(ContractError, 'edition-two definition requires edition-two publication and receipt'):
            validate_bundle(older)
        older = make_fixtures.northstar(coverage='partial')
        receipt = {k: v for k, v in older['receipt'].items() if k not in ('operators', 'renderers')} | {'schema_version': 1, 'partial': False}
        receipt['source_revisions'] = [{k: v for k, v in item.items() if k != 'mapping_version'} for item in receipt['source_revisions']]
        rehash_receipt(receipt)
        older['receipt'] = receipt
        with self.assertRaisesRegex(ContractError, 'edition-two definition requires edition-two publication and receipt'):
            validate_bundle(older)
        # Partial coverage of a read input is the only meaning of a partial receipt.
        partial = make_fixtures.northstar(coverage="partial")
        self.assertTrue(partial["receipt"]["partial"])
        validate_bundle(partial)
        partial["receipt"]["partial"] = False
        rehash_receipt(partial["receipt"])
        with self.assertRaisesRegex(ContractError, "receipt partial"):
            validate_bundle(partial)

    def test_problem_codes_and_limit_bodies(self):
        validate_problem({"status": 413, "code": "payload_too_large"})
        for limit in ("organization_queue", "task_queue", "imports_per_day", "stored_original_bytes", "queries_per_hour"):
            validate_problem({"status": 429, "code": "limit_exceeded", "limit": limit, "retry_after_seconds": 30})
        for code in ("source_deleted", "source_quarantined", "unknown_value_in_aggregate", "lookup_ambiguous", "resource_limit"):
            validate_problem({"status": 422, "code": code})
        for body, expected in (({"status": 422, "code": "payload_too_large"}, "payload_too_large is 413"),
                               ({"status": 429, "code": "limit_exceeded", "retry_after_seconds": 30}, "429 names its limit"),
                               ({"status": 422, "code": "source_deleted", "limit": "task_queue"}, "429 names its limit"),
                               ({"status": 429, "code": "limit_exceeded", "limit": "task_queue"}, "429 names its limit"),
                               ({"status": 422, "code": "limit_exceeded", "limit": "task_queue", "retry_after_seconds": 1}, "limit_exceeded is 429"),
                               ({"status": 429, "code": "limit_exceeded", "limit": "jobs", "retry_after_seconds": 1}, "schema:"),
                               ({"status": 422, "code": "narrow_the_sql"}, "schema:"),
                               ({"status": 413.0, "code": "payload_too_large"}, "float"),
                               ({"status": 429, "code": "limit_exceeded", "limit": "task_queue", "retry_after_seconds": True}, "text or integers"),
                               ({"status": 200, "code": "source_deleted"}, "schema:")):
            with self.subTest(body=body), self.assertRaisesRegex(ContractError, expected):
                validate_problem(body)

    def test_normalized_rows_digest_domain(self):
        vectors = [v for v in json.loads((ROOT / "fixtures" / "canonical-vectors.json").read_text()) if v.get("domain") == "forma:normalized-rows:v1"]
        self.assertEqual(len(vectors), 2)
        for vector in vectors:
            with self.subTest(vector=vector["name"]):
                self.assertIsInstance(vector["value"], list)
                self.assertEqual(digest("normalized-rows", vector["value"]), vector["digest"])
        self.assertIn(vectors[0]["digest"], {source["rows_digest"] for source in fixture()["sources"]})


class CanonicalTests(unittest.TestCase):
    def test_golden_vectors(self):
        for vector in json.loads((ROOT / "fixtures" / "canonical-vectors.json").read_text()):
            with self.subTest(name=vector["name"]):
                self.assertEqual(canonical(vector["value"]).hex(), vector["utf8_hex"])
                self.assertEqual(digest(domain_name(vector.get("domain", "forma:result:v1")), vector["value"]), vector["digest"])

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
