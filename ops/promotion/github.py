"""Narrow GitHub boundary: read-only gates plus post-health annotated-tag sealing.

No workflow retry, merge or deployment approval API is called. Only the separate
seal job has repository write permission; AWS execution has contents:read.
"""
import argparse
import base64
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time
import urllib.parse
import urllib.request
import zipfile

from .artifacts import NAMES, REPORTS, sha, validate_bundle
from .candidate import BRANCH, OWNER_ID, REPOSITORY, REPOSITORY_ID, WORKFLOW, api, require

REVIEWER_ID = 15230422


def oidc_claims(claims, environment, candidate, run_id, run_attempt):
    require(environment in {"beta-plan", "beta"}, "unknown promotion environment")
    expected = f"repo:getforma-dev@{OWNER_ID}/forma-app-platform@{REPOSITORY_ID}:environment:{environment}"
    require(claims.get("iss") == "https://token.actions.githubusercontent.com"
        and claims.get("aud") == "sts.amazonaws.com" and claims.get("sub") == expected
        and claims.get("repository") == REPOSITORY and str(claims.get("repository_id")) == str(REPOSITORY_ID)
        and str(claims.get("repository_owner_id")) == str(OWNER_ID)
        and claims.get("ref") == "refs/heads/" + BRANCH and claims.get("event_name") == "push"
        and claims.get("workflow_ref") == REPOSITORY + "/" + WORKFLOW + "@refs/heads/" + BRANCH
        and claims.get("sha") == candidate and str(claims.get("run_id")) == str(run_id)
        and str(claims.get("run_attempt")) == str(run_attempt), "OIDC claims do not match the frozen trust boundary")


def verify_oidc(environment, candidate):
    # This is a diagnostics guard over the token received from GitHub's HTTPS
    # runner endpoint. STS still verifies its signature; no token is stored/logged.
    url = urllib.parse.urlsplit(os.environ["ACTIONS_ID_TOKEN_REQUEST_URL"])
    require(url.scheme == "https" and url.hostname and url.hostname.endswith(".actions.githubusercontent.com")
        and not url.username and not url.password, "untrusted OIDC token endpoint")
    query = [(k, v) for k, v in urllib.parse.parse_qsl(url.query) if k != "audience"]
    target = urllib.parse.urlunsplit(url._replace(query=urllib.parse.urlencode(query + [("audience", "sts.amazonaws.com")])))
    request = urllib.request.Request(target, headers={"Authorization": "Bearer " + os.environ["ACTIONS_ID_TOKEN_REQUEST_TOKEN"]})
    try:
        class NoRedirect(urllib.request.HTTPRedirectHandler):
            def redirect_request(self, req, fp, code, msg, headers, newurl):
                return None
        with urllib.request.build_opener(NoRedirect).open(request, timeout=15) as response:
            body = response.read(128 * 1024 + 1)
        require(len(body) <= 128 * 1024, "OIDC response too large")
        token = json.loads(body)["value"]
        parts = token.split(".")
        require(len(parts) == 3, "invalid OIDC response")
        claims = json.loads(base64.urlsafe_b64decode(parts[1] + "=" * (-len(parts[1]) % 4)))
        oidc_claims(claims, environment, candidate, os.environ["GITHUB_RUN_ID"], os.environ["GITHUB_RUN_ATTEMPT"])
        require(type(claims.get("exp")) is int and claims["exp"] > time.time(), "expired OIDC token")
    except Exception:
        # HTTP exceptions can contain the signed request URL. Never expose it.
        raise ValueError("OIDC claim preflight refused; no cloud session assumed") from None
    print(json.dumps({"trusted_claims_match": True}))


def run_identity(run, run_id, candidate):
    require(str(run.get("id")) == str(run_id) and run.get("head_sha") == candidate
        and run.get("repository", {}).get("id") == REPOSITORY_ID
        and run.get("repository", {}).get("owner", {}).get("id") == OWNER_ID
        and run.get("event") == "push" and run.get("head_branch") == BRANCH
        and run.get("path") == WORKFLOW and run.get("run_attempt") == 1,
        "foreign, rerun or untrusted promotion workflow")


def artifact_identity(record, run_id, candidate, artifact_id, digest):
    require(str(record.get("id")) == str(artifact_id) and record.get("expired") is False
        and record.get("name") == "beta-qualified-" + candidate
        and record.get("workflow_run", {}).get("id") == int(run_id)
        and record.get("workflow_run", {}).get("head_sha") == candidate
        and record.get("digest") == "sha256:" + digest
        and re.fullmatch(r"[0-9a-f]{64}", digest)
        and type(record.get("size_in_bytes")) is int and 0 < record["size_in_bytes"] <= 25 * 1024**3,
        "artifact identity mismatch")


def extract_bundle(archive, output):
    require(not output.exists(), "artifact output already exists")
    allowed = {"manifest.json", *REPORTS, *(n + ".oci.tar" for n in NAMES)}
    with zipfile.ZipFile(archive) as zipped:
        items = zipped.infolist()
        require(len(items) == len(allowed) and {i.filename for i in items} == allowed,
            "artifact ZIP file set differs")
        require(sum(i.file_size for i in items) <= 25 * 1024**3, "artifact ZIP too large")
        require(all(not i.is_dir() and (i.external_attr >> 16) & 0o170000 in {0, 0o100000}
            and not i.flag_bits & 1 for i in items), "nonregular or encrypted artifact member")
        output.mkdir(parents=True, mode=0o700)
        for item in items:
            with zipped.open(item) as source, (output / item.filename).open("xb") as target:
                for chunk in iter(lambda: source.read(1024 * 1024), b""):
                    target.write(chunk)


def download(candidate, run_id, artifact_id, digest, output, manifest_digest):
    run_identity(api(f"repos/{REPOSITORY}/actions/runs/{run_id}"), run_id, candidate)
    artifact_identity(api(f"repos/{REPOSITORY}/actions/artifacts/{artifact_id}"), run_id, candidate, artifact_id, digest)
    with tempfile.TemporaryDirectory() as temporary:
        archive = Path(temporary) / "artifact.zip"
        with archive.open("wb") as stream:
            subprocess.run(["gh", "api", f"repos/{REPOSITORY}/actions/artifacts/{artifact_id}/zip"], stdout=stream, check=True)
        require(sha(archive) == digest, "artifact ZIP digest mismatch")
        extract_bundle(archive, output)
    require(sha(output / "manifest.json") == manifest_digest, "qualified bundle manifest mismatch")
    validate_bundle(output, candidate)


def validate_approval(document, expected_run_id, expected_candidate):
    require(document.get("run_id") == str(expected_run_id) and document.get("repository") == REPOSITORY
        and document.get("environment") == "beta" and document.get("approved") is True
        and document.get("commit_sha") == expected_candidate, "approval identity mismatch")
    evidence = document["evidence"]
    require(document["raw_sha256"] == hashlib.sha256(json.dumps(evidence, sort_keys=True,
        separators=(",", ":")).encode()).hexdigest(), "approval evidence hash mismatch")
    run_identity(evidence["run"], expected_run_id, expected_candidate)
    environment = evidence["environment"]
    require(environment.get("name") == "beta" and environment.get("can_admins_bypass") is False,
        "beta environment bypass enabled")
    rules = [r for r in environment.get("protection_rules", []) if r.get("type") == "required_reviewers"]
    require(len(rules) == 1 and rules[0].get("prevent_self_review") is False
        and [(r.get("type"), r.get("reviewer", {}).get("id")) for r in rules[0].get("reviewers", [])]
        == [("User", REVIEWER_ID)], "beta required reviewer changed")
    require(environment.get("deployment_branch_policy") == {"protected_branches": False, "custom_branch_policies": True}
        and [(p.get("name"), p.get("type")) for p in evidence["branches"].get("branch_policies", [])]
        == [(BRANCH, "branch")], "beta branch policy changed")
    reviews = [r for r in evidence["reviews"] if any(e.get("name") == "beta"
        and e.get("id") == environment.get("id") for e in r.get("environments", []))]
    require(len(reviews) == 1 and reviews[0].get("state") == "approved"
        and reviews[0].get("user", {}).get("id") == REVIEWER_ID
        and reviews[0].get("user", {}).get("type") == "User", "one actual human beta approval required")
    reviewer = {k: reviews[0]["user"][k] for k in ("login", "id")}
    require(document.get("reviewer") == reviewer, "reviewer projection mismatch")
    return document


def approval(candidate, run_id, output):
    evidence = {"run": api(f"repos/{REPOSITORY}/actions/runs/{run_id}"),
        "reviews": api(f"repos/{REPOSITORY}/actions/runs/{run_id}/approvals"),
        "environment": api(f"repos/{REPOSITORY}/environments/beta"),
        "branches": api(f"repos/{REPOSITORY}/environments/beta/deployment-branch-policies")}
    reviews = [r for r in evidence["reviews"] if any(e.get("name") == "beta" for e in r.get("environments", []))]
    require(len(reviews) == 1, "beta review history missing or ambiguous")
    document = {"run_id": str(run_id), "repository": REPOSITORY, "environment": "beta", "approved": True,
        "reviewer": {k: reviews[0]["user"][k] for k in ("login", "id")}, "commit_sha": candidate,
        "observed_at_utc": datetime.now(timezone.utc).isoformat(),
        "raw_sha256": hashlib.sha256(json.dumps(evidence, sort_keys=True, separators=(",", ":")).encode()).hexdigest(),
        "evidence": evidence}
    validate_approval(document, run_id, candidate)
    output.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    output.write_text(json.dumps(document, indent=2) + "\n")
    output.chmod(0o600)


def request(candidate, manifest_digest, output, promotion_id=None, packet_sha256=None):
    require(os.environ["GITHUB_REPOSITORY"] == REPOSITORY
        and os.environ["GITHUB_WORKFLOW_REF"] == REPOSITORY + "/" + WORKFLOW + "@refs/heads/" + BRANCH
        and os.environ["GITHUB_RUN_ATTEMPT"] == "1", "untrusted workflow request")
    value = {"candidate_sha": candidate, "qualified_artifact_manifest_sha256": manifest_digest,
        "authorization": {"actor": os.environ["GITHUB_ACTOR"], "run_id": os.environ["GITHUB_RUN_ID"],
            "run_attempt": os.environ["GITHUB_RUN_ATTEMPT"], "repository": REPOSITORY,
            "workflow_ref": os.environ["GITHUB_WORKFLOW_REF"]}}
    if promotion_id is not None:
        value.update(promotion_id=promotion_id, packet_sha256=packet_sha256)
    output.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    output.write_text(json.dumps(value, indent=2) + "\n")
    output.chmod(0o600)


def validate_summary(value, candidate):
    require(value.get("status") in {"prepared", "verified"} and value.get("candidate_sha") == candidate,
        "promotion summary identity mismatch")
    common = {"status", "candidate_sha", "promotion_id", "packet_sha256"}
    fields = {"cost_genui_cents", "cost_total_cents", "expires_at_utc", "review"} if value["status"] == "prepared" else {
        "activated_at_utc", "tag", "service_revision", "broker_revision"}
    require(set(value) == common | fields, "private or missing summary field")
    require(re.fullmatch(r"[A-Za-z0-9_-]{1,96}", value["promotion_id"])
        and re.fullmatch(r"[0-9a-f]{64}", value["packet_sha256"]), "invalid packet identity")
    if value["status"] == "prepared":
        for key, cap in (("cost_genui_cents", 17000), ("cost_total_cents", 20800)):
            require(type(value[key]) is int and 0 <= value[key] <= cap, "summary cost out of bounds")
        require(re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", value["expires_at_utc"]), "invalid expiry")
        review = value["review"]
        require(set(review) == {"images", "predecessor_candidate_sha", "source_tree_sha", "deployment_contract_sha256",
            "schema_version", "preserved", "changes", "scan", "sequence", "outage_required"}, "invalid review projection")
        require(set(review["images"]) == set(NAMES) and all(re.fullmatch(r"sha256:[0-9a-f]{64}", d)
            for d in review["images"].values()), "invalid review image identities")
        require(all(re.fullmatch(r"[0-9a-f]{40}", review[k]) for k in ("predecessor_candidate_sha", "source_tree_sha"))
            and re.fullmatch(r"[0-9a-f]{64}", review["deployment_contract_sha256"])
            and type(review["schema_version"]) is int and 1 <= review["schema_version"] <= 10000,
            "invalid review source/schema")
        require(review["preserved"] == {"schema": True, "template": True, "trial": True, "aliases": True, "secrets": True},
            "preserved state not proven")
        require(isinstance(review["changes"], list) and 1 <= len(review["changes"]) <= 100, "invalid change count")
        ids = set()
        for change in review["changes"]:
            require(set(change) == {"logical_id", "resource_type", "action", "replacement"}
                and re.fullmatch(r"[A-Za-z][A-Za-z0-9]{0,254}", change["logical_id"])
                and re.fullmatch(r"AWS::[A-Za-z0-9]+::[A-Za-z0-9]+", change["resource_type"])
                and change["action"] in {"Add", "Modify", "Remove"}
                and change["replacement"] in {"True", "False", "Conditional", None}
                and change["logical_id"] not in ids, "invalid review change")
            ids.add(change["logical_id"])
        require(set(review["scan"]) == set(NAMES), "missing scan projection")
        for scan in review["scan"].values():
            require(set(scan) == {"findings", "introduced"} and type(scan["findings"]) is int
                and 0 <= scan["findings"] <= 10000 and type(scan["introduced"]) is int
                and scan["introduced"] == 0, "unreviewed scan change")
        require(review["sequence"] == ["stop both services", "execute reviewed change set", "verify stopped boundaries",
            "activate both services", "verify health and images"] and review["outage_required"] is True,
            "operation sequence changed")
    else:
        require(re.fullmatch(r"beta/\d{8}-" + re.escape(candidate[:7]), value["tag"]), "release tag differs")
        require(re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", value["activated_at_utc"]), "invalid activation time")
        require(all(type(value[k]) is int and 1 <= value[k] <= 100000 for k in ("service_revision", "broker_revision")),
            "invalid service revision")
    return value


def tag_annotation(candidate, tag, packet_sha256, run_id):
    require(re.fullmatch(r"[0-9a-f]{40}", candidate)
        and re.fullmatch(r"beta/\d{8}-" + re.escape(candidate[:7]), tag)
        and re.fullmatch(r"[0-9a-f]{64}", packet_sha256) and re.fullmatch(r"[1-9][0-9]*", str(run_id)),
        "invalid verified tag request")
    return (f"Verified beta release {candidate}\nPrivate release packet sha256:{packet_sha256}\n"
        f"https://github.com/{REPOSITORY}/actions/runs/{run_id}\n")


def existing_tag(ref, candidate, tag, annotation, read=api):
    require(ref.get("ref") == "refs/tags/" + tag and ref.get("object", {}).get("type") == "tag", "tag is not annotated")
    obj = read(f"repos/{REPOSITORY}/git/tags/{ref['object']['sha']}")
    require(obj.get("tag") == tag and obj.get("message") == annotation
        and obj.get("object") == {"type": "commit", "sha": candidate,
            "url": f"https://api.github.com/repos/{REPOSITORY}/git/commits/{candidate}"}, "existing tag differs")


def seal(candidate, tag, packet_sha256, run_id):
    annotation = tag_annotation(candidate, tag, packet_sha256, run_id)
    run = api(f"repos/{REPOSITORY}/actions/runs/{run_id}")
    # A tag-only retry may run at attempt 2; it must not rerun AWS execution.
    run_identity(dict(run, run_attempt=1), run_id, candidate)
    path = f"repos/{REPOSITORY}/git/ref/tags/{tag}"
    def read_ref():
        response = subprocess.run(["gh", "api", path], capture_output=True, text=True)
        value = json.loads(response.stdout)
        if response.returncode:
            require(str(value.get("status")) == "404", "tag lookup unavailable")
            return None
        return value
    found = read_ref()
    if found:
        existing_tag(found, candidate, tag, annotation)
        return
    def post(path, value):
        response = subprocess.run(["gh", "api", "--method", "POST", path, "--input", "-"],
            input=json.dumps(value), capture_output=True, text=True)
        require(response.returncode == 0, "tag write unavailable; reconcile exact tag before retry")
        return json.loads(response.stdout)
    obj = post(f"repos/{REPOSITORY}/git/tags", {"tag": tag, "message": annotation, "object": candidate, "type": "commit"})
    require(re.fullmatch(r"[0-9a-f]{40}", obj.get("sha", "")), "invalid tag object response")
    post(f"repos/{REPOSITORY}/git/refs", {"ref": "refs/tags/" + tag, "sha": obj["sha"]})
    existing_tag(read_ref(), candidate, tag, annotation)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("download", "approval", "request", "summary", "seal", "oidc"))
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--run-id", default=os.environ.get("GITHUB_RUN_ID"))
    parser.add_argument("--artifact-id")
    parser.add_argument("--artifact-digest")
    parser.add_argument("--manifest-digest")
    parser.add_argument("--promotion-id")
    parser.add_argument("--packet-sha256")
    parser.add_argument("--tag")
    parser.add_argument("--environment", choices=("beta-plan", "beta"))
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.command == "download":
        download(args.candidate, args.run_id, args.artifact_id, args.artifact_digest, args.output, args.manifest_digest)
    elif args.command == "approval":
        approval(args.candidate, args.run_id, args.output)
    elif args.command == "request":
        request(args.candidate, args.manifest_digest, args.output, args.promotion_id, args.packet_sha256)
    elif args.command == "seal":
        seal(args.candidate, args.tag, args.packet_sha256, args.run_id)
    elif args.command == "oidc":
        verify_oidc(args.environment, args.candidate)
    else:
        value = validate_summary(json.loads(args.output.read_text()), args.candidate)
        with open(os.environ["GITHUB_OUTPUT"], "a") as out:
            for key in ("promotion_id", "packet_sha256"):
                out.write(key + "=" + value[key] + "\n")
            if value["status"] == "verified":
                out.write("tag=" + value["tag"] + "\n")
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as out:
            out.write("### Promote to beta — " + value["status"] + "\n\n```json\n" + json.dumps(value, indent=2) + "\n```\n")


if __name__ == "__main__":
    main()
