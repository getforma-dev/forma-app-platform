"""Put the library source objects of one organization (DEPLOY-RUNBOOK 5.4).

From the scratchpad upload_library.py of 2026-10-30, parameterized; run by
`just deploy library <window> --approved "<words>"`, which first writes the two
inputs below. [MUTATION]: S3 PutObject, If-None-Match "*" (a 412 on an existing
key is expected and reported as "exists"), SSE-KMS under the tenant's key, a
SHA-256 checksum on every object.

Inputs, all private:
  <window>/task-env.json      the live task definition's environment
                              (name/value list: GENUI_EVIDENCE_BUCKET, GENUI_TENANTS_JSON)
  <window>/library-plan.json  app/runtime/library_sources.py plan --organization <org>
  <cache-root>/o/<org>/<source>.<revision>.parquet
                              app/runtime/library_sources.py build --cache-root <cache-root>
Prints a JSON report: the tenant prefix and, per object, key, bytes, sha256, state.
"""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path

NORTHSTAR = "00000000-0000-4000-8000-000000000001"


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--repo", type=Path, required=True, help="the checkout holding contracts/fixtures/library")
    parser.add_argument("--window-dir", type=Path, required=True)
    parser.add_argument("--cache-root", type=Path, help="default: <priv>/library")
    parser.add_argument("--priv", type=Path, required=True)
    parser.add_argument("--organization", default=NORTHSTAR)
    parser.add_argument("--region", default=os.environ.get("AWS_REGION"))
    args = parser.parse_args()
    if not args.region:
        raise SystemExit("--region or AWS_REGION is required")
    cache_root = args.cache_root or args.priv / "library"

    env = {e["name"]: e["value"] for e in json.loads((args.window_dir / "task-env.json").read_text())}
    bucket = env["GENUI_EVIDENCE_BUCKET"]
    tenant = next(t for t in json.loads(env["GENUI_TENANTS_JSON"]) if t["organization_id"] == args.organization)
    plan = json.loads((args.window_dir / "library-plan.json").read_text())

    import boto3  # the operator venv (app/operator/.venv), imported only to upload
    import botocore

    s3 = boto3.client("s3", region_name=args.region)
    out = []
    for src in plan["sources"]:
        original = (args.repo / "contracts/fixtures/library" / src["csv"]).read_bytes()
        if "sha256:" + hashlib.sha256(original).hexdigest() != src["content_digest"]:
            raise SystemExit(f"content digest differs from the plan: {src['csv']}")
        parquet = (cache_root / "o" / args.organization / f'{src["source_id"]}.{src["revision_id"]}.parquet').read_bytes()
        for name, data, ctype in (("original", original, "text/csv"),
                                  ("part.parquet", parquet, "application/vnd.apache.parquet")):
            key = src["objects"][name]
            if not key.startswith(tenant["prefix"]):
                raise SystemExit(f"object key outside the tenant prefix: {key}")
            digest = hashlib.sha256(data).digest()
            try:
                s3.put_object(Bucket=bucket, Key=key, Body=data, ContentType=ctype, ServerSideEncryption="aws:kms",
                              SSEKMSKeyId=tenant["kms_key_arn"], ChecksumSHA256=base64.b64encode(digest).decode(),
                              IfNoneMatch="*")
                state = "uploaded"
            except botocore.exceptions.ClientError as error:
                if error.response["Error"]["Code"] in ("PreconditionFailed", "412"):
                    state = "exists"
                else:
                    raise
            out.append({"key": key, "bytes": len(data), "sha256": digest.hex(), "state": state})
    print(json.dumps({"bucket_prefix": tenant["prefix"], "objects": out}, indent=1))


if __name__ == "__main__":
    main()
