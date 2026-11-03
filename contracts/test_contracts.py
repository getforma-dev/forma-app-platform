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
RETAINED_TWO = ROOT / "fixtures" / "impl-2026-10-29.2"
RETAINED_THREE = ROOT / "fixtures" / "impl-2026-10-29.3"
LIBRARY = ROOT / "fixtures" / "library"
# Stored-data vectors (edition impl-2026-10-31.4): documents admitted and stored before a later rule, which must keep
# their stored-document verdict. Beta's Expense claims bundle as seeded at 0c3261b (APP-105), byte for byte.
STORED_EXPENSE = ROOT / "fixtures" / "stored" / "expense-claims-d70710d3.json"
STORED_EXPENSE_SHA256 = "446e5485a97b1a261b4c5ca6ee6836aee99f43e14140e61f3b41a3dfc97bd059"
# The fixture identities edition impl-2026-10-29.3 locked; its copies are retained byte-identically.
EDITION_THREE_FIXTURES = {
    "canonical-vectors": "eb9a4501813c8c90d8fbb756179a64074dd2df0a31883f39e995601afa9354f9",
    "command-recovery": "b2539b15b32c7086576bd42aed824d823e5fa4c8ed2f40ed57637cdf79d14ecf",
    "negative-vectors": "315a9d32e91dbeaf21c0ad3ec6f8e882315e46ef23b7db4c3c47f4b161ae1cbc",
    "northstar": "d9acfd8f399d20ea6dbcabc72dca1437c802a1565ca7119c6dd53587b14e1480",
    "spend-review": "186162da87532f368ed3c74faafc46c04780cc7331c095dc92f729bead0f0851",
}
# The fixture identities edition impl-2026-10-29.2 locked; its copies are retained byte-identically.
EDITION_TWO_FIXTURES = {
    "canonical-vectors": "eb9a4501813c8c90d8fbb756179a64074dd2df0a31883f39e995601afa9354f9",
    "negative-vectors": "315a9d32e91dbeaf21c0ad3ec6f8e882315e46ef23b7db4c3c47f4b161ae1cbc",
    "northstar": "d9acfd8f399d20ea6dbcabc72dca1437c802a1565ca7119c6dd53587b14e1480",
    "spend-review": "186162da87532f368ed3c74faafc46c04780cc7331c095dc92f729bead0f0851"
}
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


class EditionThreeTests(unittest.TestCase):
    """Edition impl-2026-10-29.3 (ADR-007 D7.15, APP-013): the recovery view, the claim and the command problem codes."""

    def test_edition_three_retains_edition_two_byte_identically(self):
        for name, identity in EDITION_TWO_FIXTURES.items():
            with self.subTest(fixture=name):
                self.assertEqual(hashlib.sha256((RETAINED_TWO / (name + ".json")).read_bytes()).hexdigest(), identity)
                # The current edition's bundles and vectors are edition two's, unchanged.
                self.assertEqual((RETAINED_TWO / (name + ".json")).read_bytes(), (ROOT / "fixtures" / (name + ".json")).read_bytes())
        for name in ("northstar", "spend-review"):
            validate_bundle(loads((RETAINED_TWO / (name + ".json")).read_bytes()))

    def test_recovery_views_claim_and_problem_bodies_validate(self):
        specimen = fixture("command-recovery")
        self.assertEqual(specimen, make_fixtures.command_recovery())
        for view in specimen["recovery_views"]:
            validate(view)
            self.assertEqual(canonical(view), canonical(loads(canonical(view))))
        validate(specimen["claim"])
        self.assertEqual(specimen["claim"]["payload_digest"], payload_digest(fixture("spend-review")["command"]))
        for body in specimen["problems"]:
            with self.subTest(code=body["code"]):
                validate_problem(body)
        # Edition-two problem bodies are unchanged.
        validate_problem({"status": 429, "code": "limit_exceeded", "limit": "task_queue", "retry_after_seconds": 7})
        validate_problem({"status": 413, "code": "payload_too_large"})

    def test_recovery_view_and_problem_refusals(self):
        specimen = fixture("command-recovery")
        stale_edit, reused_key, absent = specimen["recovery_views"]
        cases = [
            ("a key conflict names the stored outcome", dict(stale_edit, reason="payload_mismatch"), "stored outcome"),
            ("a version conflict names none", dict(reused_key, reason="expected_version"), "stored outcome"),
            ("values iff the record exists", dict(absent, current_values=[]), "current values iff"),
            ("values iff the record exists (absent values on a live record)", dict(stale_edit, current_values=None), "current values iff"),
            ("stored outcome under another key", dict(reused_key, idempotency_key="00000000000000000000000000000001"), "scope mismatch"),
            ("duplicate recovery value", dict(stale_edit, current_values=[stale_edit["current_values"][0]] * 2), "duplicate recovery value"),
        ]
        for label, view, expected in cases:
            with self.subTest(case=label), self.assertRaisesRegex(ContractError, expected):
                validate(view)
        with self.assertRaisesRegex(ContractError, "schema"):
            validate(dict(stale_edit, reason="overwritten"))
        with self.assertRaisesRegex(ContractError, "schema"):
            validate(dict(specimen["claim"], idempotency_key="short"))
        problems = [
            ("conflict without its view", {"status": 409, "code": "conflict"}, "conflict is 409 with the recovery view"),
            ("conflict at 412", {"status": 412, "code": "conflict", "recovery_view": stale_edit}, "conflict is 409"),
            ("a view on another code", {"status": 400, "code": "malformed_command", "recovery_view": stale_edit}, "only a conflict carries one"),
            ("stale without the revision", {"status": 412, "code": "stale"}, "stale is 412 with the current revision"),
            ("stale at 409", {"status": 409, "code": "stale", "current_revision": specimen["problems"][2]["current_revision"]}, "stale is 412"),
            ("in_flight without Retry-After", {"status": 409, "code": "in_flight"}, "in_flight and busy are 409 with Retry-After"),
            ("busy at 503", {"status": 503, "code": "busy", "retry_after_seconds": 2}, "in_flight and busy are 409"),
            ("Retry-After on a 400", {"status": 400, "code": "malformed_command", "retry_after_seconds": 2}, "only a 429, in_flight or busy does"),
            ("a limit on in_flight", {"status": 409, "code": "in_flight", "retry_after_seconds": 2, "limit": "task_queue"}, "in_flight and busy are 409 with Retry-After seconds"),
            ("an unregistered code", {"status": 409, "code": "overwritten"}, "schema"),
        ]
        for label, body, expected in problems:
            with self.subTest(case=label), self.assertRaisesRegex(ContractError, expected):
                validate_problem(body)

    def test_create_on_an_existing_record_must_name_a_declared_transition(self):
        """Edition impl-2026-10-29.3 amendment (APP-013 review M1): a create command expects the absent record
        (version 0), or an existing version only when its declaration has transitions and the command carries a
        declared transition's target value; an update command expects an existing version."""
        def moved(bundle, status, version):
            bundle["command"]["expected_record_version"] = version
            bundle["command"]["values"][0]["value"]["value"] = status
            bundle["outcome"]["record_version"] = version + 1
            bundle["outcome"]["payload_digest"] = payload_digest(bundle["command"])
            return bundle
        # S05 ik-2: reviewed under a new key at version 1 is the declared needs_context -> reviewed transition.
        validate_bundle(moved(fixture("spend-review"), "reviewed", 1))
        validate_bundle(moved(fixture("northstar"), "reviewed", 3))
        # At version 1 without a transition target (a note-only or identical resubmission) it is refused.
        with self.assertRaisesRegex(ContractError, "create expects absent record"):
            validate_bundle(moved(fixture("spend-review"), "needs_context", 1))
        # A create whose declaration has no transitions still expects the absent record.
        bundle = moved(fixture("spend-review"), "reviewed", 1)
        bundle["definition"]["content"]["commands"][0]["transitions"] = []
        with self.assertRaisesRegex(ContractError, "create expects absent record"):
            check_command(bundle)
        # An update expects an existing version.
        bundle = fixture("spend-review")
        bundle["definition"]["content"]["commands"][0]["operation"] = "update"
        with self.assertRaisesRegex(ContractError, "update expects an existing version"):
            check_command(bundle)

    def test_declared_guards_and_initial_values(self):
        """APP-107 amendment (additive): a transition may carry a typed guard (`when`) over a field of the same record
        schema, and a filing create may declare the `initial` value of an enum field, chosen by guards; the seeded
        create must carry it. Ordering comparisons need an ordered field; values are typed literals, never null."""
        library = ROOT / "fixtures" / "library"
        deal = loads((library / "deal-desk.json").read_bytes())
        purchase = loads((library / "purchase-requests.json").read_bytes())
        validate_bundle(copy.deepcopy(deal))
        validate_bundle(copy.deepcopy(purchase))
        # The deal desk seed (8%) is auto-approved by the first initial rule; a seed claiming "submitted" is refused.
        bundle = copy.deepcopy(deal)
        status = next(item for item in bundle["command"]["values"] if item["field_id"] == "status")
        status["value"]["value"] = "submitted"
        bundle["outcome"]["payload_digest"] = payload_digest(bundle["command"])
        with self.assertRaisesRegex(ContractError, "declared initial value"):
            validate_bundle(bundle)
        # Hostile declarations are refused by the definition rules.
        def edited(source, change):
            bundle = copy.deepcopy(source)
            change(bundle["definition"]["content"]["commands"])
            return bundle
        cases = [
            (lambda c: c[1]["transitions"][0]["when"].update(field_id="nope"), "unknown guard field"),
            (lambda c: c[1]["transitions"][0]["when"].update(value={"type": "text", "value": "5000"}), "guard value type/unit mismatch"),
            (lambda c: c[1]["transitions"][0]["when"].update(value={"type": "null"}), "guard value type/unit mismatch"),
            (lambda c: c[1]["transitions"][0]["when"].update(field_id="vendor", value={"type": "text", "value": "x"}), "ordered field"),
            (lambda c: c[0]["initial"][0].update(value="paid"), "unknown initial value"),
            (lambda c: c[0]["initial"][0].update(field_id="item"), "initial requires enum field"),
            (lambda c: c[1].update(initial=[{"field_id": "status", "value": "submitted"}]), "initial values belong to a filing create"),
        ]
        for change, expected in cases:
            with self.subTest(expected=expected), self.assertRaisesRegex(ContractError, expected):
                check_command(edited(purchase, change))
        # A guard member outside the closed shape is a schema refusal.
        with self.assertRaises(Exception):
            check_command(edited(purchase, lambda c: c[1]["transitions"][0]["when"].update(sql="1=1")))


    def test_filing_creates_declare_the_initial_value_of_a_governed_field(self):
        """APP-113 review (storage 2, 14): a create that may write an enum field another command governs by declared
        transitions must declare its `initial` value (the expense `submit` could file a claim as `approved`); a
        command declares each (field, from, to) once; and a guarded transition only counts as fired when its guard
        holds on the values the command carries."""
        library = ROOT / "fixtures" / "library"
        expense = loads((library / "expense-claims.json").read_bytes())
        purchase = loads((library / "purchase-requests.json").read_bytes())
        validate_bundle(copy.deepcopy(expense))
        # The expense seed may not file a claim already approved.
        bundle = copy.deepcopy(expense)
        next(item for item in bundle["command"]["values"] if item["field_id"] == "status")["value"]["value"] = "approved"
        bundle["outcome"]["payload_digest"] = payload_digest(bundle["command"])
        with self.assertRaisesRegex(ContractError, "declared initial value"):
            validate_bundle(bundle)
        def edited(source, change):
            bundle = copy.deepcopy(source)
            change(bundle["definition"]["content"]["commands"])
            return bundle
        # Edition impl-2026-10-31.4: both rules are the authoring verdict (`new_content=True`); the stored-document
        # verdict is edition impl-2026-10-29.3's, so the same documents stored before the rules keep validating.
        for source, change in ((expense, lambda c: c[next(i for i, x in enumerate(c) if x["id"] == "submit")].pop("initial")),
                               (purchase, lambda c: c[0].pop("initial"))):
            with self.assertRaisesRegex(ContractError, "requires initial"):
                check_command(edited(source, change), new_content=True)
            check_command(edited(source, change))
        duplicated = lambda c: c[1]["transitions"].append(copy.deepcopy(c[1]["transitions"][-1]))
        with self.assertRaisesRegex(ContractError, "duplicate transition"):
            check_command(edited(purchase, duplicated), new_content=True)
        check_command(edited(purchase, duplicated))
        # Guard parity: submitted -> manager_approved holds only when amount > $5,000.
        def decided(amount):
            bundle = edited(purchase, lambda c: c[1]["writable_fields"].append("amount"))
            definition = bundle["definition"]
            definition["digest"] = digest("app-definition", definition["content"])
            bundle["publication"]["definition_digest"] = bundle["receipt"]["definition_digest"] = definition["digest"]
            rehash_receipt(bundle["receipt"])
            command = bundle["command"]
            command.update(command_id="decide", expected_record_version=1, values=[
                {"field_id": "status", "value": {"type": "enum", "value": "manager_approved"}},
                {"field_id": "amount", "value": {"type": "money", "minor_units": amount, "currency": "USD", "minor_unit_exponent": 2}}])
            bundle["outcome"].update(command_id="decide", record_version=2, payload_digest=payload_digest(command))
            return bundle
        validate_bundle(decided("600000"))
        with self.assertRaisesRegex(ContractError, "create expects absent record"):
            validate_bundle(decided("100"))


def check_command(bundle, new_content=False):
    """validate_bundle after a definition edit: re-identify the definition and its pins."""
    definition = bundle["definition"]
    definition["digest"] = digest("app-definition", definition["content"])
    bundle["publication"]["definition_digest"] = bundle["receipt"]["definition_digest"] = definition["digest"]
    rehash_receipt(bundle["receipt"])
    validate_bundle(bundle, new_content=new_content)


class EditionFourTests(unittest.TestCase):
    """Edition impl-2026-10-31.4 (APP-129): the APP-107/APP-113 command-declaration amendment and the prebuilt library,
    over the retained edition impl-2026-10-29.3."""

    def test_edition_four_retains_edition_three_byte_identically(self):
        for name, identity in EDITION_THREE_FIXTURES.items():
            with self.subTest(fixture=name):
                self.assertEqual(hashlib.sha256((RETAINED_THREE / (name + ".json")).read_bytes()).hexdigest(), identity)
                # The current edition's specimens and vectors are edition three's, unchanged.
                self.assertEqual((RETAINED_THREE / (name + ".json")).read_bytes(), (ROOT / "fixtures" / (name + ".json")).read_bytes())
        def kept(name):
            return loads((RETAINED_THREE / (name + ".json")).read_bytes())
        for name in ("northstar", "spend-review"):
            with self.subTest(bundle=name):
                bundle = kept(name)
                validate_bundle(bundle)
                self.assertEqual(bundle["definition"]["digest"], digest("app-definition", bundle["definition"]["content"]))
        run_negative_vectors(self, json.loads((RETAINED_THREE / "negative-vectors.json").read_text()), kept)
        specimen = kept("command-recovery")
        for view in specimen["recovery_views"]:
            validate(view)
        validate(specimen["claim"])
        for body in specimen["problems"]:
            validate_problem(body)

    def test_every_library_bundle_validates_and_matches_the_catalog(self):
        catalog = json.loads((LIBRARY / "catalog.json").read_text())
        names = sorted(p.name for p in LIBRARY.glob("*.json") if p.name != "catalog.json")
        self.assertEqual(names, sorted(app["fixture"] for app in catalog["apps"]))
        for app in catalog["apps"]:
            with self.subTest(app=app["library_id"]):
                raw = (LIBRARY / app["fixture"]).read_bytes()
                self.assertEqual(hashlib.sha256(raw).hexdigest(), app["fixture_sha256"])
                bundle = loads(raw)
                validate_bundle(bundle, new_content=True)
                self.assertEqual(bundle["definition"]["schema_version"], 1)
                self.assertEqual(bundle["definition"]["digest"], app["definition_digest"])

    def test_stored_definitions_keep_their_verdict(self):
        """Victor's option A (2026-10-31): the APP-113 definition rules judge new content only. Beta's stored Expense
        claims revision predates them: it validates under the stored-document verdict (as under impl-2026-10-29.3 and
        Rust admission) and is refused only as new content. Storage's command-time fence protects it instead."""
        raw = STORED_EXPENSE.read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), STORED_EXPENSE_SHA256)
        bundle = loads(raw)
        self.assertEqual(bundle["definition"]["digest"], "sha256:d70710d345d807be6893bb88cf73eacdbf3a8e1ebea94de59f2c21c52453d0de")
        validate_bundle(copy.deepcopy(bundle))
        validate(bundle["definition"])
        with self.assertRaisesRegex(ContractError, "create writing a governed enum field requires initial"):
            validate_bundle(copy.deepcopy(bundle), new_content=True)
        # The library's current Expense claims app is the same app with `initial` declared: a new revision.
        current = loads((LIBRARY / "expense-claims.json").read_bytes())
        self.assertEqual(current["definition"]["app_id"], bundle["definition"]["app_id"])
        self.assertNotEqual(current["definition"]["digest"], bundle["definition"]["digest"])
