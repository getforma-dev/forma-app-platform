# APP-077 local evidence: operator image, tools and the optional OperatorTask

**Author evidence, not acceptance.** Everything here ran on one Apple-silicon Mac with local Docker. Nothing touched AWS: no deployment, image push, registry, STS, S3, KMS, RDS or public DNS. S3 was a loopback stub ([`app/operator/local_s3.py`](../../app/operator/local_s3.py)) driven by the real boto3 client; PostgreSQL was a disposable 16.13 container with a throwaway TLS CA. No independent review has run yet, so no acceptance box is checked. The two-process race coordinator is APP-080's (ADR-008) and was not built.

## Identity

| Item | Value |
| --- | --- |
| Base | `d6986c2` (APP-073 in review; APP-010, APP-075 in progress) |
| Code commit | `1c581f2` feat(operator) — every check below ran at this commit; the evidence commit adds only `evidence/APP-077/` |
| Host | Darwin arm64; Docker 29.8.2 linux/arm64; Python 3.11.0 (a scratch venv installed from `app/operator/requirements.lock` with `--require-hashes --only-binary=:all:`); Node v22.21.0; rustc 1.93.1 (Rust not touched, so no `cargo test`) |
| Operator image | `sha256:a3b07f930744fdd470e2c2012fa6bee75c6f594038f7d01c59d175aa3990d41b`, linux/arm64, 438,200,398 bytes, user `65532:65532`, entrypoint `python3 -I /operator/app/operator/operator.py`, cmd `verify-catalog`, volume `/run/operator` only, no exposed port. Local only and **removed after the checks**. |
| Build context | 132 files, exactly the allowlist in [`Dockerfile.dockerignore`](../../app/operator/Dockerfile.dockerignore): the 28 packaged inputs, the Dockerfile and its ignore file, and the 102 tracked `parser/` files (the manifest binds the parser bytes to exactly these) |
| Parser in the image | built by the service image's parser stage (same base, lockfile, toolchain, `/source/parser` path): guest `a1d3bb37…4774`, host `44672951…2934`, `parser-manifest.json` `9cd03be7…8a13`, sources digest `89711310…de61` (equal to a recomputation over this checkout's `parser/`), rustc 1.93.1 `01f6ddf7…` aarch64 |
| Packages | exactly the lock: attrs 26.1.0, boto3/botocore 1.43.121, duckdb 2.0.0, jmespath 1.1.0, jsonschema 4.26.0, jsonschema-specifications 2025.9.1, psycopg/psycopg-binary 3.3.6, python-dateutil 2.9.0.post0, referencing 0.37.0, rpds-py 2026.9.1, s3transfer 0.19.3, six 1.17.0, typing-extensions 4.16.0, urllib3 2.8.0 (plus the base image's pip, setuptools, wheel). The lock is the verbatim union of `app/runtime`, `contracts` and `spikes/APP-005` locks; no new selection. |
| Reviewed RDS CA | `90a6f1a7…2f21b` at `/etc/ssl/certs/rds-global-bundle.pem`, mode 0444 |
| Schema | database version 5 (`app/migrations/manifest.json` `b3cc8070…1013`) |

File SHA-256 at `1c581f2`:

```
e0dda83715a51edf335df7e6972901c7f0461864d855216b310bb12b4a32cbd2  app/operator/Dockerfile
05cfbf14778553496bb246b7e223c09c5c35165acd663e536b257dda526ee945  app/operator/Dockerfile.dockerignore
97de5f3be1654b9f91b48a2aebeab3dc3ffc19113aa503d39cd0c78fcffebc78  app/operator/operator.py
b71ccda7ea97fbc660f812169e593e74a20fcca8a03ea39e6ec2542454b2ac49  app/operator/overrides.json
72a22a15b6deaa312b2a6f4b98db686bae08731e01bc13fa32a0a0b7385c357a  app/operator/requirements.lock
7a648f62c33caac9bb362361c94cdabbbcf3f0b2cf37364c51cc6619ef8b3f6c  app/operator/test_operator.py
f65fac3d32f3f6098c43b91e30ce1e6dfe875c858afa561fc1d40caed51cb9ba  app/operator/local_s3.py
62e17e02f94b62adf26a114574893faee535631922f4b648f917e7d4f6d01dd7  app/operator/check_image.py
c5c82d53e965af2fa9cc45d5c81143ab2d5131e1d937882f9a44c6d88ed126ca  infra/src/stack.ts
427210afba5bfca5f6a196060509948effc4d563c2918c831476369cd129fc3c  infra/test/security.test.ts
aa76c5de301bd6ce1a179c73008e54bcba91845601397b24d4d9a0c5e84bbaf8  app/runtime/admission_operator.py   (packaged unchanged, as at d6986c2)
c9073aee151f25006219acb15d0e7ac35c01a76751ee3392c02f8f26aa50d4ad  app/runtime/tenant_seed.py
689462bf2abc4b81d0603677ec0a1940f75e81be85a6bfa9bec3115020602e04  app/runtime/runtime.py
```

The image's own `build-identity.json` lists the digest of every packaged file; [image/check.json](image/check.json) `image_files` records them as probed in the image, each equal to the repository file.

## What exists

- [`app/operator/operator.py`](../../app/operator/operator.py): closed subcommands `migrate` (`runtime.apply_migrations`), `verify-catalog` (read-only catalog proof, then the version row), `seed` (`tenant_seed.make_bundles` validates every envelope before connecting; catalog proof; `tenant_seed.seed`; one commit), `admission …` (argv passed unchanged to `admission_operator.main`, so whatever closed subcommands that file has at merge time are packaged) and `upload-sources [--plan]`. Database commands require `PGUSER=genui_migration`, `PGDATABASE=genui`, a TCP host, `PGSSLROOTCERT` and verify-full before any connection, then check `session_user/current_user/current_database()`. Refusals are fixed codes on standard error.
- `upload-sources`: the closed source table has one entry, the Northstar organization's seeded `events` revision (source `…006e`, revision `…006f`). The pinned CSV (`6e0f0ed7…4647`, also checked against the fixture manifest) runs through the image's manifest-bound parser-host and WASM guest; `fixtures/qualify.py`'s `parser_rows` must equal the independent `fixtures/verify.py` reader; the trusted adapter selects asset `build-worker-17`, types rows with the oracle's strict `timestamp`/`integer`, orders by time then original record, and writes `events_v1` `[time, destination, bytes]` Parquet (ZSTD, one thread, DuckDB 2.0.0), round-trip checked. Objects go to ADR-009 B1's `o/{org}/s/{source}/r/{rev}/original` then `…/part.parquet` with `x-amz-server-side-encryption: aws:kms`, the tenant key id and `x-amz-checksum-sha256`, as a conditional `If-None-Match: *` PUT; on 412 the existing object is HEAD-compared (length and SHA-256) and left `unchanged`, any difference refuses. The receipt prints the real `content_digest`, `normalized_digest`, `contract_digest`, row count, adapter identity, the seeded placeholders it supersedes, and each object's key, digest, version id and key ARN. No database row is written.
- [`overrides.json`](../../app/operator/overrides.json): 12 documented RunTask overrides (command only, container `operator`), largest 206 bytes of 8,192; entry point `["python3","-I","/operator/app/operator/operator.py"]`, matching the Dockerfile, the image and the CDK task. `admission issue` is deliberately **not** an ECS override (see open questions).
- `infra/src/stack.ts`: optional `OperatorImageDigest` (empty default) → condition `OperatorTaskEnabled` → `OperatorTask` (1 vCPU/2 GiB, count zero, nothing references it), execution role = `MigrationExecutionRole` with `PGPASSWORD` from the existing secret, a conditional `OperatorImagePull` policy for the operator repository on that role, and its own `OperatorRole` task role: **no `sts:AssumeRole`**, only `s3:GetObject`/`s3:PutObject` on `o/<tenant>/s/*` and `kms:GenerateDataKey` on each tenant key. Environment: `PG*`, `GENUI_EVIDENCE_BUCKET`, `GENUI_OPERATOR_TENANTS_JSON` (organization, key ARN, prefix; no role ARN), `AWS_REGION`.

## Commands and results (all at `1c581f2`)

| Command | Result |
| --- | --- |
| `GENUI_OPERATOR_TEST_PARSER=<main checkout>/parser/target $VENV/bin/python -m unittest discover -s app/operator -v` | 29 passed, 0 skipped, 17.6 s ([log](tests/operator-unittest.txt)): closed parsing; value-free refusals; env checks before connect; override sizes/entry point/default; lock union; allowlist vs tracked files and Dockerfile sources; B1 keys; tenant config; fixture pin; S3 stub (SSE-KMS + key + checksum + SigV4 + conditional, rerun unchanged with no second version, differing object refused and kept, partial upload completed on rerun, another tenant's key denied by the stub's bucket-policy rule, missing tenant refused before any request); real parser adapter (digests, 6 rows, the S02 window total 1,140,000 = `expected.json`, ZSTD, determinism, parser/reader disagreement and wrong guest refused); PG 16.13/TLS sequence (catalog refusal before migration, migrate + identical repeat, drift refusal, a seed that meets a changed row refuses whole with an unchanged dump, idempotent seed, admission backfill/verify/list passthrough, wrong identity refused) |
| `$VENV/bin/python -m unittest discover -s app/runtime` | 116 passed, 0 skipped (101.7 s) |
| `$VENV/bin/python -m unittest discover -s app/qualification` | 6 passed |
| `cd infra && npm run typecheck` | exit 0 |
| `cd infra && npm test` | 24 passed, 0 failed ([summary](tests/infra-npm-test.txt)); five new operator tests plus the updated task-definition and pull tests |
| `docker build -f app/operator/Dockerfile -t genui-app077-operator:1c581f2 .` | built; the cold build took about 4 min (parser stage 224 s), the rebuild at the commit was cached |
| `$VENV/bin/python -P app/operator/check_image.py --image genui-app077-operator:1c581f2 --base-revision d6986c2 --output evidence/APP-077/image/check.json` | **69 of 69 expectations met; cleanup complete** ([report](image/check.json)); contexts 4.4 s, upload 2.1 s, database sequence 7.4 s |
| `docker run --rm --network=none --read-only --cap-drop=ALL … genui-app077-operator:1c581f2 upload-sources --plan` | exit 0; [plan-in-image.json](upload-sources/plan-in-image.json) |
| `python3 evidence/APP-077/infra/template_diff.py BASE.json NEW.json evidence/APP-077/infra/template-diff.json` | the template synthesized at `d6986c2` versus `1c581f2`: only added Parameters (`OperatorImageDigest`), Conditions (`OperatorTaskEnabled`), Resources (`OperatorImagePull`, `OperatorRole…`, `OperatorRoleDefaultPolicy…`, `OperatorTask…`, each under the condition) and Outputs; **no existing resource, parameter, condition or output changed** ([diff](infra/template-diff.json)) |

The image check binds: the exported context equals the allowlist byte-for-byte and carries every tracked `parser/` file; the service and qualification contexts are byte-identical to `d6986c2`; image user, entry point, command, volume, ports and environment (no `PG*`, `GENUI_*`, role, key or endpoint); every packaged file equals the repository and the build identity; modes (0444, parser binaries and directories 0555), no set-id file, `/run/operator` 0700 65532; the CA; parser bytes against the manifest and its source digest against `parser/`; installed packages against the lock; `operator.py` reaches no `refresh`/`serve`/`probe`/`scoped_session`/`assume_role`; every documented override's size and its command parsed inside the image; seven fixed refusals; `upload-sources` through the image's own parser, DuckDB and boto3 against the in-container stub with `--network=none`; and, on an internal Docker network, `verify-catalog` (refused), `migrate`, `migrate` (identical), `verify-catalog`, `seed`, `seed` (identical), `admission backfill` for both organizations, `admission verify`, `admission list` and the default command, with the migration history equal to the manifest and the synthetic `PGPASSWORD` never printed.

Digests of the uploaded Northstar events revision (same bytes in the macOS-arm64 checkout run and the linux-arm64 image):

| Object | Key | SHA-256 | Bytes |
| --- | --- | --- | --- |
| original | `o/00000000-0000-4000-8000-000000000001/s/00000000-0000-4000-8000-00000000006e/r/00000000-0000-4000-8000-00000000006f/original` | `6e0f0ed7f182509d7cbadb70405c21d2e25f24e89d80c2137297e411fc034647` | 397 |
| part.parquet | `…/r/00000000-0000-4000-8000-00000000006f/part.parquet` | `0c2582d801005bd49ed4173b86bf8d03b9e681cbcfcca8a8387b5eb32237a082` | 642 |

`contract_digest` `b92d4b96…8f64` equals the seeded work binding. The seeded placeholders are `content_digest` `f85bf298…7371`, `normalized_digest` `bb16c6e4…c119`, `row_count` 5; the real revision has 6 rows (E6 lies outside the S02 half-open window, which still totals 1,140,000 bytes).

## Acceptance, item by item

1. *Image from frozen source with exact context, source and image bindings; contains the migration runner, catalog verifier, seed, admission operator, source upload and hash-locked dependencies; no application role-assumption grant; each documented override ≤ 8,192 bytes with a matching entryPoint.* **Supported locally** by the image check (69/69) and the operator tests. "No application role-assumption grant" holds at both layers: `OperatorRole` has no `sts:*` (CDK test) and the operator code reaches no refresh/assume path (image check). Not yet done: an image from CI's x86_64 build and a registry digest (APP-078).
2. *Optional `OperatorTask` (migration execution role, `PGPASSWORD`, count zero) passes the CDK template tests; service and qualification task definitions unchanged.* **Supported**: `npm test` 24/24 and the template diff (no existing resource changed).
3. *Independent review with no blocking finding; the preparation record's package items answered or recorded for APP-078.* **Open** (no review has run). The two package items of [managed-harness-preparation](../APP-008/managed-harness-preparation.md) slice 3 are answered here for the reviewer:
   - **Seed package paths.** The image mirrors the repository layout under `/operator` so the reviewed modules resolve unchanged: `tenant_seed.py` reads `contracts/fixtures/impl-2026-10-28.1/{northstar,spend-review}.json` and `contracts/validate.py` (+ `schema.json`); `runtime.py` reads `app/migrations/`; `qualify.py`/`verify.py` read `fixtures/phase-one/v1/manifest.json` and the one CSV. The context is exactly these files; the image probe proves each equals the repository.
   - **Partial-mutation recovery.** `migrate`: one transaction under the runner's advisory lock, rollback on any error, a repeat is a proven no-op. `seed`: one caller-owned transaction after the catalog proof; any refusal (including "existing seed differs") rolls the whole seed back — tested by a changed canary row with a byte-identical `pg_dump` after the refusal — and a rerun is exact-match idempotent; it never overwrites or resets an epoch. `admission`: one transaction per command (APP-073's). `upload-sources`: objects only, original before Parquet, conditional writes; a rerun completes a partial upload and leaves equal objects unchanged; a differing object at a revision key refuses (tested). Recovery from a *committed* wrong seed is not automated (by design: no overwrite); it is a reviewed forward fix or APP-078's snapshot path.
   - The coordinator items (cross-process timing, lock visibility under RDS roles, drift-trial target) are APP-080's.

## Decisions for the reviewer and open questions

1. **New IAM authority.** `upload-sources` needs S3 and KMS, so `OperatorRole` (conditional) holds `s3:GetObject`/`s3:PutObject` on `o/<tenant>/s/*` and `kms:GenerateDataKey` on each tenant key, with no ListBucket (the conditional PUT answers 412 instead of a 403/404 ambiguity) and no delete. The bucket policy's two SSE-KMS denials still bind each prefix to its key. This is narrower than the tenant roles (`o/<tenant>/*`) but is a new direct grant to review.
2. **Database digests are not written.** Plan Task 9 says the real `normalized_digest`/`contract_digest` are recorded "in `source_revisions` and `work_bindings` … replacing the placeholder digests". `upload-sources` only prints them: the seeded digests are FK-chained through `work_bindings`, `work_source_dependencies` and `record_source_dependencies`, the seed is exact-match idempotent (a rewrite makes every later `seed` refuse), and ADR-009 says a revision is never rewritten. Until a decision (a new revision row, or a seed edition carrying the real digests), D7.10's fetch would refuse the uploaded Parquet against the placeholder `normalized_digest`. Suggested owner: APP-012 with APP-078.
3. **`events_v1` versus APP-005's shape.** The Parquet holds the contract's three fields; APP-005's executor registers `event_id, time, destination, bytes`. APP-012 copies that executor and must use the contract shape (or the contract must change).
4. **`admission issue` through ECS** would print the raw code to the task's stdout, which is the awslogs log group, retaining the secret in CloudWatch. It is excluded from the documented overrides; provider invitations and `reconcile` also need the management key and a private directory. APP-078 must choose a delivery channel (for example an operator-held public key) before issuing in the managed window.
5. **APP-010's operator subcommands** (`grant-permission`, `revoke-permission`, `revoke-creator-authority`) are not in `admission_operator.py` at `d6986c2`; the image packages that file unchanged (`aa76c5de…`). After the APP-010 fix merges, rebuild and rerun the check; the passthrough and the override parser need no change.
6. **Entry point `python3 -I`.** The plan names the file `operator.py`, which shadows the standard library's `operator` module when its directory is on `sys.path`; isolated mode removes the script directory (and ignores `PYTHON*` variables, so `PYTHONDONTWRITEBYTECODE`/`PYTHONUNBUFFERED` are inert; the root filesystem is read-only and output is flushed at exit). `check_image.py` refuses to run without `-P` for the same reason.
7. **S3 semantics proven only against the stub**: conditional `If-None-Match` on the versioned bucket, `HeadObject` of an SSE-KMS object without `kms:Decrypt`, and the bucket-policy denials are APP-078 observations.
8. Not covered here: an x86_64 image, the service image's guest digest compared with this image's (same sources and stage, not built in this run), and any RDS/IAM/TLS/S3 behavior.

No credential, token or password is in this directory: the database used trust authentication with a synthetic `PGPASSWORD` that the check asserts was never printed, and the stub's request-signing values are fixed local test strings.
