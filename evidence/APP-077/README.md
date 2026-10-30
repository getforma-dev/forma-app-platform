# APP-077 local evidence: operator image, tools and the optional OperatorTask

**Author evidence, not acceptance.** Everything here ran on one Apple-silicon Mac with local Docker. Nothing touched AWS: no deployment, image push, registry, STS, S3, KMS, RDS or public DNS. S3 was a loopback stub ([`app/operator/local_s3.py`](../../app/operator/local_s3.py)) driven by the real boto3 client; PostgreSQL was a disposable 16.13 container with a throwaway TLS CA. The two-process race coordinator is APP-080's (ADR-008) and was not built.

**Re-recorded at `293528b`** (2026-10-29), the fixes for the [independent review](independent-review.md) of merge `34a42eb` (its dispositions are at the end of that file). The first author run, at `1c581f2`, is kept as history at the end of this file; its identity table described a tree that the merge no longer had (review m7).

## Identity

| Item | Value |
| --- | --- |
| Base | `e7c7421` (main with APP-077 merged at `34a42eb`, ADR-010 accepted) |
| Code commit | `293528b` fix(operator): APP-077 review M1-M3, m1-m4, m6. Every check below ran at this commit; the evidence commit adds only `evidence/APP-077/` |
| Host | Darwin arm64; Docker 29.8.2 linux/arm64; Python 3.11.0 (a fresh venv installed from `app/operator/requirements.lock` with `--require-hashes --only-binary=:all:`); Node v22.21.0 |
| Operator image | `sha256:16398054e2c08f46b032753eb09fb01a6e95b2e31cbd596959385fecebc97198`, linux/arm64, 438,225,947 bytes, user `65532:65532`, entrypoint `python3 -I -u /operator/app/operator/operator.py`, cmd `verify-catalog`, volume `/run/operator` only, no exposed port. Local only and **removed after the checks**. |
| Build context | 132 files, exactly the allowlist in [`Dockerfile.dockerignore`](../../app/operator/Dockerfile.dockerignore) (unchanged) |
| Parser in the image | guest `a1d3bb37…4774`, host `44672951…2934`, `parser-manifest.json` `9cd03be7…8a13`, sources digest `89711310…de61` (equal to a recomputation over this checkout's `parser/`), rustc 1.93.1 `01f6ddf7…` aarch64; unchanged from the first run |
| Packages | exactly the lock (unchanged) |
| Schema | database version 5 (`app/migrations/manifest.json` `7857f4bb…c6bf`) |

File SHA-256 at `293528b`:

```
eb133b55fa2614f768ed30b86ac8569943d1674be7483652e3f797d174c09051  app/operator/Dockerfile
05cfbf14778553496bb246b7e223c09c5c35165acd663e536b257dda526ee945  app/operator/Dockerfile.dockerignore
01c80c187faf607991127afee2b189b0223e31e517663eef6047cba5b59a613c  app/operator/operator.py
9b779da4beef4c595811ca87fb54651a796fe9c4b6e89c58023c156c0dd3d0d6  app/operator/overrides.json
72a22a15b6deaa312b2a6f4b98db686bae08731e01bc13fa32a0a0b7385c357a  app/operator/requirements.lock
48d5daaae5b18800b72aed4932f41daa8f690d6b7a3f4a44a26c5d36a3581bca  app/operator/test_operator.py
add40b07a9dab46338041a9ac90a7f2c001d7e12433b5d421874897c9c0b7351  app/operator/local_s3.py
580aded0a26c202ecd5e06213366c8fd1560fadffb89beca4098c555287b244c  app/operator/check_image.py
74b12c8c5d73f2d9525acb7eda77454412c09739825cd2d2f1a1e3f3f1509b3d  infra/src/stack.ts
f991a0ab4dc75d3ec711341feed2cb53f9a9523cc57956edcdbaf361ea89ae6d  infra/test/security.test.ts
8e4b0299bbb54afff6d69d84328f1549afd6691fcc99865a562aea3af6411cea  app/runtime/admission_operator.py   (packaged unchanged from e7c7421)
73bd6b2d16c636fc3e2a8f91b71507d42aa737823084c7f6a2fe3570ee65fc2e  app/runtime/tenant_seed.py
689462bf2abc4b81d0603677ec0a1940f75e81be85a6bfa9bec3115020602e04  app/runtime/runtime.py
```

The image's own `build-identity.json` lists the digest of every packaged file; [image/check.json](image/check.json) `image_files` records them as probed in the image, each equal to the repository file.

## What exists

- [`app/operator/operator.py`](../../app/operator/operator.py): closed subcommands `migrate`, `verify-catalog`, `seed`, `admission …` and `upload-sources [--plan]`. Every database command, the `admission` passthrough included (except `admission defaults`, which needs no database), runs the same checks before any connection: `PGHOST` is one DNS name or address (no `/` socket path, no `@` abstract socket, no comma-separated host list), `PGUSER=genui_migration`, `PGDATABASE=genui`, verify-full, `PGSSLROOTCERT` an absolute CA file (never libpq's `system`), and inside a managed task exactly `/etc/ssl/certs/rds-global-bundle.pem`. Inside a managed task (`ECS_CONTAINER_METADATA_URI_V4` or `ECS_CONTAINER_METADATA_URI` set, which Fargate always sets) `admission issue` and `admission reconcile` refuse with `refused_in_managed_task`; the subcommand is decided by `admission_operator`'s own parser.
- `upload-sources`: the pinned Northstar CSV (`6e0f0ed7…4647`) runs through the image's manifest-bound parser-host and WASM guest and the trusted adapter to `events_v1` Parquet, as before. The objects now go to a **new revision** of source `…006e`, never to the seeded placeholder revision `…006f`: the revision id is `uuid5(namespace = organization id, name = "sha256:<content digest hex>")` stamped version 4 (`uuid.UUID(bytes=…, version=4)`), because `contracts/schema.json` `source_revision.revision_id` and the operator channels accept only version 4 or 7 identifiers. For Northstar it is `9161c1e4-27c0-4cbb-addf-eb7ce504bf45`. Each PUT is conditional (`If-None-Match: *`), SSE-KMS under the tenant key with `x-amz-checksum-sha256`; on 412 the existing object is read back by a checksum-mode HeadObject and must match. After both objects are stored the command prints exactly one JSON record for APP-012's `register-source-revision`: `{organization_id, source_id, revision_id, content_digest, normalized_digest, object_digest, row_count, contract_digest}`, where `object_digest` is the stored `part.parquet` digest and equals `normalized_digest` (ADR-009 D9.3 option a). No database row is written. `--plan` prints the full plan plus the same record as planned.
- [`overrides.json`](../../app/operator/overrides.json): 20 documented RunTask overrides (command only, container `operator`), largest 282 bytes of 8,192; entry point `["python3","-I","-u","/operator/app/operator/operator.py"]`, matching the Dockerfile, the image and the CDK task. Every `admission_operator` subcommand is either documented (`backfill`, `create-app`, `defaults`, `extend`, `grant-permission`, `list`, `provision`, `revoke`, `revoke-creator-authority`, `revoke-entitlement`, `revoke-membership`, `revoke-permission`, `verify`) or listed under `not_ecs_overrides` and refused in code (`issue`, `reconcile`); a unit test binds the two sets to the parser's choices.
- `infra/src/stack.ts`: `OperatorRole` (conditional) per tenant holds `s3:GetObject` on `o/<tenant>/s/*`; `s3:PutObject` on the same prefix only with `s3:if-none-match = *`; and `kms:GenerateDataKey` plus `kms:Decrypt` on that tenant's key, only with `kms:ViaService = s3.<region>.amazonaws.com` and `kms:EncryptionContext:aws:s3:arn` StringLike that tenant's `o/<tenant>/s/*` object ARN (the bucket's `BucketKeyEnabled` is false, so S3's encryption context is the object ARN; the CDK test asserts both). Still no `sts:*`, `rds-db:*`, secrets, ListBucket or delete.

## Commands and results (all at `293528b`)

| Command | Result |
| --- | --- |
| `GENUI_OPERATOR_TEST_PARSER=<main checkout>/parser/target $VENV/bin/python -m unittest discover -s app/operator -p 'test_*.py' -v` | **36 passed**, 0 skipped, 21.4 s ([log](tests/operator-unittest.txt)). New: managed-task refusal of issue/reconcile, admission pre-connect checks (mocked connect never called), host and CA guards, the derived revision, the no-Decrypt rerun refusal, the unconditional-PUT denial, the one-line registration record (and an identical rerun), plus the overrides coverage binding. Run from the repository root with `discover -s app/operator`: `python -m unittest test_operator` from inside `app/operator/` would put `operator.py` ahead of the standard library (review m6). |
| `$VENV/bin/python -m unittest discover -s app/runtime` | **119 passed** (104.6 s) |
| `cd infra && npm run typecheck` | exit 0 |
| `cd infra && npm test` | **24 passed**, 0 failed ([log](tests/infra-npm-test.txt)) |
| `docker build -f app/operator/Dockerfile -t genui-app077-fix:293528b .` | built (cached parser stage) |
| `$VENV/bin/python -P app/operator/check_image.py --image genui-app077-fix:293528b --base-revision e7c7421 --output evidence/APP-077/image/check.json` | **79 of 79 expectations met; cleanup complete** ([report](image/check.json)); contexts 3.9 s, upload 2.8 s, database sequence 7.0 s. New: 7 refusal cases (abstract socket, host list, `system` trust store, admission identity, managed issue, managed reconcile, managed unpinned CA) and 3 upload expectations (derived revision and no placeholder key, the registration record, the no-Decrypt rerun refusal). |
| `docker run --rm --network=none --read-only --cap-drop=ALL … genui-app077-fix:293528b upload-sources --plan` | exit 0; [plan-in-image.json](upload-sources/plan-in-image.json) |
| `python3 evidence/APP-077/infra/template_diff.py E7C.json TIP.json evidence/APP-077/infra/template-diff.json e7c7421 293528b` | changed only `OperatorRoleDefaultPolicyA061B00C` (the M1/m3 statements) and `OperatorTask534EBA45` (only `EntryPoint` gains `-u`); nothing added or removed ([diff](infra/template-diff.json)) |
| the same from `495c555` (before APP-077) to `293528b` | only the conditional additions of the first run: Parameter `OperatorImageDigest`, Condition `OperatorTaskEnabled`, Resources `OperatorImagePull`, `OperatorRole…`, `OperatorRoleDefaultPolicy…`, `OperatorTask…`, Outputs `OperatorTaskDefinition`, `OperatorTaskRoleArn`; nothing removed or changed ([diff](infra/template-diff-from-495c555.json)) |

Templates were synthesized locally (`Template.fromStack(new GenuiFoundation(new App(), 'SecurityQualification'))`, aws-cdk-lib 2.275.0) from `git archive` of each revision's `infra/`; no cloud call. The only Docker resources created were the image (removed) and the containers and network `check_image.py` names with its run label (removed; `cleanup_complete: true`).

Failing first: before the code change, the new and changed operator tests failed (among them every S3 test because the stub did not yet model KMS grants, `derived_revision_id` missing, admission connecting with `PGUSER=genui_workspace`, `@abstract` accepted), and `npm test` failed only "operator task role holds no role assumption and reaches only B1 source objects under each tenant key".

Objects of the uploaded Northstar events revision (same bytes as the first run; only the revision in the key changed):

| Object | Key | SHA-256 | Bytes |
| --- | --- | --- | --- |
| original | `o/00000000-0000-4000-8000-000000000001/s/00000000-0000-4000-8000-00000000006e/r/9161c1e4-27c0-4cbb-addf-eb7ce504bf45/original` | `6e0f0ed7f182509d7cbadb70405c21d2e25f24e89d80c2137297e411fc034647` | 397 |
| part.parquet | `…/r/9161c1e4-27c0-4cbb-addf-eb7ce504bf45/part.parquet` | `0c2582d801005bd49ed4173b86bf8d03b9e681cbcfcca8a8387b5eb32237a082` | 642 |

Registration record (from the image's plan; the real upload prints the same after both objects are stored): `organization_id` `…0001`, `source_id` `…006e`, `revision_id` `9161c1e4-27c0-4cbb-addf-eb7ce504bf45`, `content_digest` `6e0f0ed7…4647`, `normalized_digest` = `object_digest` `0c2582d8…a082`, `row_count` 6, `contract_digest` `b92d4b96…8f64`. The seeded placeholder revision `…006f` (`content_digest` `f85bf298…7371`, `normalized_digest` `bb16c6e4…c119`, `row_count` 5) is reported as `seeded_placeholder_not_written` and is never a key.

## Acceptance, item by item

1. *Image from frozen source …; each documented override ≤ 8,192 bytes with a matching entryPoint.* **Supported locally (arm64)** by the image check (79/79) and the operator tests. Not yet done: an x86_64 image and a registry digest (APP-078).
2. *Optional `OperatorTask` … passes the CDK template tests; service and qualification task definitions unchanged.* **Supported**: `npm test` 24/24; both template diffs above; the service and qualification contexts are byte-equal to `e7c7421`.
3. *Independent review with no blocking finding; the preparation record's package items answered or recorded for APP-078.* The review found no blocker; its findings are fixed or dispositioned in [independent-review.md](independent-review.md). A re-check of these fixes has not run. The package items of [managed-harness-preparation](../APP-008/managed-harness-preparation.md) slice 3:
   - **Seed package paths.** Unchanged and verified: the image mirrors the repository layout under `/operator`; the probe proves each packaged file equals the repository.
   - **Partial-mutation recovery.** `migrate`: one transaction, rollback on error, a repeat is a no-op. `seed`: one transaction after the catalog proof; any refusal rolls back whole (unchanged `pg_dump`), a rerun is exact-match idempotent. `admission`: one transaction per command. `upload-sources`: objects only, original before Parquet, each a conditional create at the derived revision's keys. A rerun after a partial upload creates the missing object and compares each existing one by a checksum-mode HeadObject; **in AWS that comparison needs `kms:Decrypt` on the tenant key, which `OperatorRole` now holds (conditioned on S3 and that tenant's `o/<tenant>/s/*`)**. Without it (the grant at `34a42eb`) the first run succeeds and every rerun refuses `object_write_refused`, which the stub now reproduces. A differing object at a revision key refuses and is never overwritten. That the real S3/KMS pair behaves this way is an APP-078 observation, not proven here.
   - The coordinator items are APP-080's.

## Open questions and APP-078 observations

1. **IAM authority** (review open question 1, M1, m3): resolved in code as above. APP-078 observes, in the managed window: a first `upload-sources` and a rerun both succeed (the rerun reports both objects unchanged) under these conditions; S3 enforces `s3:if-none-match` on the identity policy (an unconditional PUT is denied); `kms:ViaService` and the `aws:s3:arn` context condition admit S3's own calls for both a single-part PUT and a checksum-mode HEAD.
2. **Placeholder digests** (M2): resolved by the coordinated decision. `upload-sources` writes a new revision and prints its registration record; APP-012's `register-source-revision` (built in that slice) records the `source_revisions` row, and binding work to it is that slice's. `upload-sources` must not run in the managed window before `register-source-revision` exists, or the objects exist without a row (harmless: a later run with the same original targets the same keys and reports them unchanged).
3. **`events_v1` versus APP-005's shape**: unchanged; APP-012's.
4. **`admission issue` through ECS** (M3): now refused in code inside a managed task. APP-078 still chooses the managed delivery channel; this guard is where to admit it.
5. **APP-010 subcommands**: resolved; packaged and documented (m4).
6. **Entry point `python3 -I -u`** (m6): `-u` makes standard output unbuffered under `-I`, which ignores `PYTHONUNBUFFERED`.
7. **`/run/operator` on Fargate** (m5): the local check used a tmpfs owned by UID 65532; whether the task's bind-mount volume keeps 65532 0700 is unproven. APP-078 observes it by running `upload-sources --plan` first.
8. **S3 semantics proven only against the stub**: conditional `If-None-Match` on the versioned bucket, the 409 `ConditionalRequestConflict` path (a refusal; a rerun handles it), the bucket-policy denials and the KMS conditions above remain APP-078 observations.
9. Not covered here: an x86_64 image and the guest digest compared with the service image's (APP-078 or CI), and any RDS/IAM/TLS/S3 behavior.

No credential, token or password is in this directory: the database used trust authentication with a synthetic `PGPASSWORD` that the check asserts was never printed, and the stub's request-signing values are fixed local test strings.

## History: the first author run at `1c581f2`

Base `d6986c2`; image `sha256:a3b07f93…d41b` (438,200,398 bytes, entrypoint `python3 -I …`); operator tests 29 passed, runtime 116, infra 24, image check 69/69, plan keys at the seeded revision `…006f`. The review rebuilt the merged tree at `34a42eb` (image `c1c4cbb5…`, 29/119/24 and 69/69) and found M1-M3 and m1-m7; the files of that run were replaced by this re-recording.
