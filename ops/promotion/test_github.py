import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

from ops.promotion import github as g
from ops.promotion.test_candidate import SHA


def run():
    return {"id": 42, "head_sha": SHA, "repository": {"id": g.REPOSITORY_ID, "owner": {"id": g.OWNER_ID}},
        "event": "push", "head_branch": g.BRANCH, "path": g.WORKFLOW, "run_attempt": 1}


def approval():
    evidence = {"run": run(), "environment": {"id": 17, "name": "beta", "can_admins_bypass": False,
        "protection_rules": [{"type": "required_reviewers", "prevent_self_review": False,
            "reviewers": [{"type": "User", "reviewer": {"id": g.REVIEWER_ID}}]}],
        "deployment_branch_policy": {"protected_branches": False, "custom_branch_policies": True}},
        "branches": {"branch_policies": [{"name": g.BRANCH, "type": "branch"}]},
        "reviews": [{"state": "approved", "environments": [{"name": "beta", "id": 17}],
            "user": {"id": g.REVIEWER_ID, "login": "owner", "type": "User"}}]}
    return {"run_id": "42", "repository": g.REPOSITORY, "environment": "beta", "approved": True,
        "commit_sha": SHA, "reviewer": {"id": g.REVIEWER_ID, "login": "owner"},
        "observed_at_utc": "2026-11-02T00:00:00+00:00", "evidence": evidence, "raw_sha256": evidence_hash(evidence)}


def evidence_hash(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


class GithubTests(unittest.TestCase):
    def test_oidc_immutable_subject_binds_repo_environment_ref_and_attempt(self):
        claims = {"iss": "https://token.actions.githubusercontent.com", "aud": "sts.amazonaws.com",
            "sub": f"repo:getforma-dev@{g.OWNER_ID}/forma-app-platform@{g.REPOSITORY_ID}:environment:beta",
            "repository": g.REPOSITORY, "repository_id": str(g.REPOSITORY_ID), "repository_owner_id": str(g.OWNER_ID),
            "ref": "refs/heads/" + g.BRANCH, "event_name": "push",
            "workflow_ref": g.REPOSITORY + "/" + g.WORKFLOW + "@refs/heads/" + g.BRANCH,
            "sha": SHA, "run_id": "42", "run_attempt": "1"}
        g.oidc_claims(claims, "beta", SHA, "42", "1")
        for key, wrong in (("sub", "repo:" + g.REPOSITORY + ":environment:beta"), ("ref", "refs/heads/pivot/pitch-mvp"),
                ("run_attempt", "2"), ("sha", "b" * 40), ("aud", "other"), ("event_name", "pull_request")):
            with self.subTest(key=key), self.assertRaises(ValueError):
                g.oidc_claims(dict(claims, **{key: wrong}), "beta", SHA, "42", "1")

    def test_existing_annotated_tag_must_match_commit_and_packet(self):
        tag = "beta/20261102-" + SHA[:7]
        message = g.tag_annotation(SHA, tag, "b" * 64, "42")
        ref = {"ref": "refs/tags/" + tag, "object": {"type": "tag", "sha": "c" * 40}}
        obj = {"tag": tag, "message": message, "object": {"type": "commit", "sha": SHA,
            "url": f"https://api.github.com/repos/{g.REPOSITORY}/git/commits/{SHA}"}}
        g.existing_tag(ref, SHA, tag, message, read=lambda _: obj)
        for bad in (dict(obj, message="different release packet"), dict(obj, object={"type": "commit", "sha": "d" * 40})):
            with self.assertRaises(ValueError):
                g.existing_tag(ref, SHA, tag, message, read=lambda _: bad)
        with self.assertRaises(ValueError):
            g.existing_tag(dict(ref, object={"type": "commit", "sha": SHA}), SHA, tag, message, read=lambda _: obj)

    def test_summary_requires_concrete_bounded_private_free_review(self):
        value = {"status": "prepared", "candidate_sha": SHA, "promotion_id": "run-42", "packet_sha256": "b" * 64,
            "cost_genui_cents": 16984, "cost_total_cents": 20784, "expires_at_utc": "2026-11-30T12:00:00Z",
            "review": {"images": {n: "sha256:" + "c" * 64 for n in g.NAMES}, "predecessor_candidate_sha": "d" * 40,
                "source_tree_sha": "e" * 40, "deployment_contract_sha256": "f" * 64, "schema_version": 19,
                "preserved": {"schema": True, "template": True, "trial": True, "aliases": True, "secrets": True},
                "changes": [{"logical_id": "ServiceTask", "resource_type": "AWS::ECS::TaskDefinition", "action": "Modify", "replacement": "True"}],
                "scan": {n: {"findings": 24, "introduced": 0, "reviewed": [], "reviewed_count": 0,
                    "unreviewed": 0, "disposition_sha256": None, "review_expires_at_utc": None} for n in g.NAMES},
                "sequence": ["stop both services", "execute reviewed change set", "verify stopped boundaries", "activate both services", "verify health and images"],
                "outage_required": True}}
        self.assertEqual(g.validate_summary(value, SHA), value)
        for mutate in (lambda v: v.update(private_arn="secret"), lambda v: v.pop("review"),
                lambda v: v["review"]["changes"][0].update(logical_id="arn:aws:private"),
                lambda v: v["review"]["scan"]["service"].update(introduced=1),
                lambda v: v["review"].update(outage_required=False), lambda v: v.update(cost_genui_cents=18001)):
            changed = copy.deepcopy(value)
            mutate(changed)
            with self.assertRaises(ValueError):
                g.validate_summary(changed, SHA)
        from ops.promotion.registry import scan_disposition
        disposition, sha = scan_disposition()
        value["review"]["deployment_contract_sha256"] = disposition["deployment_contract_sha256"]
        for role in g.NAMES:
            names = sorted(f["name"] for f in disposition["roles"][role]["findings"])
            value["review"]["scan"][role].update(introduced=len(names), reviewed_count=len(names), reviewed=names,
                disposition_sha256=sha, review_expires_at_utc=disposition["expires_at_utc"])
        self.assertEqual(g.validate_summary(value, SHA), value)
        for role in g.NAMES:
            for mutate in (lambda s: s.update(introduced=0), lambda s: s.update(reviewed_count=0),
                    lambda s: s.update(unreviewed=1), lambda s: s.update(disposition_sha256="0" * 64),
                    lambda s: s.update(review_expires_at_utc="2099-01-01T00:00:00Z"),
                    lambda s: s["reviewed"].__setitem__(0, "CVE-2099-12345")):
                changed = copy.deepcopy(value); mutate(changed["review"]["scan"][role])
                with self.assertRaises(ValueError): g.validate_summary(changed, SHA)
        changed = copy.deepcopy(value); changed["review"]["deployment_contract_sha256"] = "0" * 64
        with self.assertRaises(ValueError): g.validate_summary(changed, SHA)

    def test_artifact_requires_same_run_head_id_digest_and_name(self):
        value = {"id": 8, "expired": False, "name": "beta-qualified-" + SHA,
            "workflow_run": {"id": 42, "head_sha": SHA}, "digest": "sha256:" + "b" * 64, "size_in_bytes": 1024}
        g.artifact_identity(value, "42", SHA, "8", "b" * 64)
        for key, wrong in (("id", 9), ("expired", True), ("name", "beta-qualified-latest"),
                ("digest", "sha256:" + "c" * 64), ("workflow_run", {"id": 41, "head_sha": SHA})):
            with self.subTest(key=key), self.assertRaises(ValueError):
                g.artifact_identity(dict(value, **{key: wrong}), "42", SHA, "8", "b" * 64)

    def test_run_from_fork_pr_other_workflow_or_rerun_is_refused(self):
        g.run_identity(run(), "42", SHA)
        for key, wrong in (("event", "pull_request"), ("head_branch", "pivot/pitch-mvp"), ("run_attempt", 2),
                ("path", ".github/workflows/foundation.yml"), ("repository", {"id": 123})):
            with self.subTest(key=key), self.assertRaises(ValueError):
                g.run_identity(dict(run(), **{key: wrong}), "42", SHA)

    def test_actual_configured_human_approval_supports_sole_owner(self):
        value = approval()
        self.assertEqual(g.validate_approval(value, "42", SHA), value)
        self.assertNotIn("reviewed_at", value)

    def test_initiator_or_boolean_cannot_substitute_for_actual_review(self):
        for mutate in (lambda e: e.update(reviews=[]),
                lambda e: e["reviews"][0].update(state="rejected"),
                lambda e: e["reviews"][0]["user"].update(id=999),
                lambda e: e["reviews"][0]["environments"][0].update(id=18),
                lambda e: e["environment"].update(can_admins_bypass=True),
                lambda e: e["branches"]["branch_policies"].append({"name": "*", "type": "branch"})):
            value = approval()
            mutate(value["evidence"])
            value["raw_sha256"] = evidence_hash(value["evidence"])
            with self.assertRaises(ValueError):
                g.validate_approval(value, "42", SHA)

    def test_approval_bound_to_exact_run_and_candidate(self):
        for run_id, candidate in (("43", SHA), ("42", "b" * 40)):
            with self.assertRaises(ValueError):
                g.validate_approval(approval(), run_id, candidate)

    def test_zip_traversal_duplicate_or_symlink_rejected_before_extraction(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            archive = base / "artifact.zip"
            for name in ("../secret", "manifest.json", "/tmp/secret"):
                with zipfile.ZipFile(archive, "w") as zipped:
                    zipped.writestr(name, "fake")
                with self.assertRaises(ValueError):
                    g.extract_bundle(archive, base / "out")
                self.assertFalse((base / "out").exists())


if __name__ == "__main__":
    unittest.main()
