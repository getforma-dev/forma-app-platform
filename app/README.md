# APP-007 application foundation — managed acceptance pending

The application is one Rust business binary with strict native Forma 3.2.0 pages. Native route admission and typed page finalization complete before listening; page responses retain their nonce/CSP/no-store policy. The only routes are a labeled synthetic foundation page, its about/assets routes and health checks. Authentication, tenant tables, queries, commands and generation remain with their owning tickets.

A small Python support process delivers temporary credentials and supervises the Rust binary. It is not a second business service. This explicit deployment detail preserves the demo's one business process, while avoiding a large unqualified Rust AWS SDK addition. Both processes share the task's trust boundary: a process compromise can reach the task's six permitted assumed roles. Logical role separation does not replace application authorization or future process isolation.

## Reproduce local preparation

Python 3.11, Node 22.21.0/npm 11.6.2, Rust 1.93.1 and Docker are the qualified tools. From the repository root:

```sh
python3 scripts/check-foundation.py
docker build --tag genui-app007-local:qualification --file app/Dockerfile app
docker run --rm --network=none --read-only --cap-drop=ALL --security-opt=no-new-privileges --entrypoint python3 genui-app007-local:qualification -c "import os,stat; p=os.stat('/run/genui'); assert os.getuid()==65532 and p.st_uid==65532 and stat.S_IMODE(p.st_mode)==0o700"
docker run --rm -i --network=none --read-only --cap-drop=ALL --security-opt=no-new-privileges --entrypoint python3 genui-app007-local:qualification - --binary /app/genui --directory /run/genui < app/runtime/smoke_http.py
```

These checks install hash-locked packages, typecheck/build native browser assets, run Rust/configuration and Python failure-path tests, exercise the actual HTTP nonce/no-store response, verify readiness stays 503 without a successful dependency probe, and check CDK templates. The image trial uses fake unit credentials and a deliberately absent probe, with no network. **It cannot pass managed acceptance.** Linux ARM64 local and AMD64 CI builds are separate image artifacts; the deployment parameter must match the selected digest's architecture.

The Dockerfile pins all three base image digests and builds source/lockfiles in a curated `app/` context. No home, AWS credential directory, environment file, build cache or private runtime file enters that context. The image is UID65532 with a read-only root and a declared `/run/genui` volume initialized mode0700/owner65532. Only that volume is writable during normal startup. Dependencies are in `runtime/requirements.lock`; public RDS roots and dependency metadata are recorded in `evidence/APP-007/runtime-dependencies.json`. The RDS bundle is public trust material, not a credential; update it deliberately with a reviewed digest.

## Credentials, reload and health

CDK supplies non-secret endpoint/role/key/prefix identities, shown by `.env.example`; its placeholders are intentionally unusable. `runtime.py serve` uses ECS task identity to assume four database roles and two organization storage roles. Each database role can authenticate only its fixed SQL user. It creates four different IAM tokens, with 900-second STS sessions and a published deadline no later than 600 seconds from issuance or 60 seconds before any session expiry. The helper refreshes approximately every 240 seconds. AWS documents a 15-minute IAM database token lifetime; the shorter published deadline is intentional. No token or password is printed or sent as a command-line argument.

The helper atomically replaces `/run/genui/credentials.json` (0700 parent, 0600 owned regular file). Rust adapters reload that file on credential acquisition; neither a request nor a model can choose an SQL role. Both readers refuse symlinks/FIFOs, inappropriate ownership/permissions, missing or expired credentials, role interchange, shared role tokens and oversized input. They use nonblocking open and a 65,537-byte read bound. Secret Debug output is redacted. Runtime files belong outside Git.

`/health/live` says only that the process is serving. `/health/ready` returns only 200 or 503, initially 503. A bounded background probe opens four actual PostgreSQL connections with `verify-full` TLS and verifies session/current user, database, no elevated role flags, a complete catalog membership set limited to the login itself and `rds_iam` (including refusal of RDS administrator and PostgreSQL predefined privilege roles), negotiated SSL and migration version. It also checks each scoped S3 prefix and an own-key KMS encrypt/decrypt round trip. It emits only a generation/expiry/fingerprint receipt, never credentials or source/object content. This performs no persistent object writes.

Readiness requires a recent successful probe for the exact current credential-file SHA-256 and generation, with its original expiry still valid. Refresh during a probe, stale results, credential expiry, wrong identity, unavailable dependencies and probe timeout fail closed. A late result cannot extend expiry; success ages out after 45 seconds. The helper process is limited to 25 seconds per probe, and the monitor waits 15 seconds between probes. Local success with test doubles is never reported as a managed probe.

## Initial migration

Use the separate migration task and execution role described in `infra/README.md`. The application task never receives the generated RDS migration password. The migration helper requires `PGUSER=genui_migration`, database `genui`, a verified TLS connection and the exact actual session identity; ECS injects `PGPASSWORD` only into the one-off migration task. No password belongs in overrides, shell arguments, context, logs or evidence.

The versioned SQL creates the non-login owner, four non-superuser/non-BYPASSRLS login roles with IAM membership, revokes public database creation/temp/connect authority, grants explicit application CONNECT and read-only migration metadata, and records version1 transactionally. It deliberately creates no tenant/business table; APP-008 owns those policies. The migration owner may assume the non-login schema owner; application roles may not. The script is intended to rerun safely, but real RDS execution, grants and rollback must still be observed.

For an operator already connected to the private database through an approved execution environment, the equivalent command is `app/scripts/migrate.sh` after supplying the documented PG variables via the authorized secret channel. Its Python dependencies are the same lockfile; it does not need a separately installed `psql`.

## Managed tenant qualification and cleanup

After the budget/operator/certificate/image decision and approved provisioning, run migration first with service desired count0. Then run the **application task definition** once with command `qualify` and the following container override (the operator fills the stack's actual cluster/task/subnet/security-group IDs in the separate network invocation):

```json
{"containerOverrides":[{"name":"workspace","command":["qualify"],"environment":[{"name":"GENUI_QUALIFY_SYNTHETIC","value":"APP-007"}]}]}
```

Use `aws ecs run-task --region us-east-1 --cluster "$CLUSTER" --task-definition "$APP_TASK" --launch-type FARGATE --platform-version 1.4.0 --network-configuration file://approved-network.json --overrides file://qualify-overrides.json`. The network file must use the stack's public task subnets, task security group and `assignPublicIp=ENABLED`; no database port is exposed publicly. This is an explicit mutating qualification command, separate from startup/readiness. It has not been run.

The qualifier refreshes real scoped credentials, verifies the four database identities/migration, writes a unique synthetic object under each organization prefix with its explicit KMS key, reads back the exact bytes/version/encryption metadata, then requires denial of the other existing object, other prefix listing, another organization's KMS key and a wrong-key upload to its own prefix. It records successes, denials and the exact created object versions as one sanitized JSON report. Any unexpected permission aborts. Even on failure, the report retains created versions for cleanup. No real source rows, token, password or ciphertext enters that report.

A failed or lost write response can mean S3 committed a version that the report could not record. Treat that as an **unknown outcome**: retain the original run UUID and reconcile both exact keys `o/<tenant UUID>/_qualification/<run UUID>/probe.bin` with the operator's `s3api list-object-versions` before cleanup or retry. Filter returned version/delete-marker records by exact key equality, retain the original report and record the reconciled versions separately. Do not start a new qualification run/UUID to hide an unresolved prior write. This applies to the wrong-key negative-control write too: an unexpected grant plus a lost response may have created another version. The operator can then build the cleanup report from verified exact versions; do not infer that a missing VersionId means nothing was written.

Save that JSON report from the task's sanitized log as `qualification.json`, then use the separate operator identity on the workstation to remove only those exact synthetic versions:

```sh
GENUI_QUALIFY_SYNTHETIC=APP-007 app/runtime/.venv/bin/python app/runtime/runtime.py cleanup --report qualification.json
```

Set `GENUI_EVIDENCE_BUCKET` and `GENUI_TENANTS_JSON` from the reviewed stack outputs before cleanup. The helper validates the report's scope, run UUID, bucket and exact per-tenant qualification keys before deleting anything. Application roles intentionally have no DeleteObjectVersion permission; cleanup must use the authorized operator identity, never broaden application grants to make the test pass. Verify those exact versions are absent afterward and retain the cleanup result. This does not delete the bucket, other revisions, keys, database or snapshots.

Then enable one service task, use the actual certificate hostname, and perform the remaining managed readiness, ingress-denial, expiry/refresh, rollback and restore checks. Do not equate the positive readiness probe with cross-tenant qualification or a recovery drill. Account/region are known (`877573132739`, `us-east-1`); monthly cap/operator/deployment inputs and all managed evidence remain open. No new GenUI resource has been provisioned.

Sources: [IAM database authentication and token lifetime](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/UsingWithRDS.IAMDBAuth.html), [RDS certificate verification](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/UsingWithRDS.SSL.html), and the frozen platform/demo design cited by APP-007.
