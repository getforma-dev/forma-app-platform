# Read-only schema-one auth preflight

The [operator helper](../../../../../infra/scripts/auth_schema_preflight.py) supplies the fresh SQL check required by the [schema-one sequence](../README.md). The [actual managed observation](managed-observation.json) passed on October 29 at 03:58 UTC: exact version1 catalog before metadata, verified TLS, fixed operator identity and a read-only transaction. The task exited zero and reached STOPPED after 53.012 seconds. Its temporary definition is INACTIVE; the original migration definition, protected source database and budget ledger remain unchanged, with service counts zero and no running/pending cluster tasks. This is point-in-time compatibility evidence; it does not start the hosted auth window or pass application admission.

[Independent review of the completed operation](managed-independent-review.md) accepts the SQL observation and bounded cleanup with no blocking finding. It binds the exact request, runtime receipt and all 23 private operation records; its 30 checks do not substitute for hosted authentication or APP-008 acceptance.

The helper embeds the exact reviewed `catalog-query.json` and `catalog-v1.json` bytes, pinned respectively to `e17ceb5e62dc6f03dfb9dc26f34018d3f2977e1b93ee5910ed16f03364a63963` and `4182e1425f2c7fc9d9540f2837cb92f07cd2521c86adc81f3add6e606e92aac9`. It does not import the newer application runtime, execute migrations, learn a live baseline or accept supplied SQL. The existing schema-one image already contains its Python/psycopg dependencies; no image change or download is required.

## Exact launch preparation

From the repository root, this command **only emits JSON**:

```sh
python3 infra/scripts/auth_schema_preflight.py --emit-override
```

The deterministic emitted `containerOverrides` document names `migration` and sets command `['-c', <fixed compressed Python payload>]`. Its complete compact UTF-8 representation is **7,867 bytes**; the emitter refuses more than8,192. The payload is self-contained, validates its decompressed length/SHA and both resource SHAs, and contains no credentials. Pass the JSON as structured arguments; do not interpolate the payload into a shell command. Added overrides must be included in the final size review.

**The existing migration task cannot run this command unchanged.** Both its image and task definition specify `entryPoint=['python3','/app/runtime.py']`. Root prepared the independently [reviewed temporary task request](operation-review.md) with **`entryPoint=['python3']`**, preserving the exact reviewed schema-one image, UID65532, execution-secret channel, environment, roles, network, limits and logging. Combining this entry point with the emitted command produces `python3 -c <payload>`. No new image, secret grant or object-store fetch was needed. Root registered and inspected the exact revision, ran it once, observed STOPPED and deregistered only that temporary revision. Existing reserved standalone usage totals 378.872 of 7,200 seconds; no new reservation was created. A future run requires its own fresh operating checks and is not authorized by this historical receipt.

Runtime accepts only the fixed retained RDS hostname, port5432, database `genui`, user `genui_migration`, `PGSSLMODE=verify-full` and the image's fixed RDS CA path. It rejects alternate libpq `PG*` channels, including host-address/service/options overrides. The existing ECS execution role injects `PGPASSWORD`; the helper consumes it privately, and the operator never reads it. The reported RDS resource ID is labeled as originating from reviewed control-plane evidence, not as an independently queried SQL fact.

Connection startup installs `search_path=pg_catalog,pg_temp`, default read-only mode and finite connection/statement/lock/idle limits before any SQL. A fresh repeatable-read, read-only transaction checks origin mode and fixed catalog-only identity, then compares the bounded complete canonical version1 catalog **before** reading `genui_meta.schema_version`. Every outcome rolls back; there is no migration, seed, state-write, repair or version2 path. The entry point sets a30-second process alarm. Output is bounded success metadata/hashes or one fixed refusal, without credentials, exception text or catalog differences. This is point-in-time evidence; the agreed operator/schema quiescence is still necessary throughout the auth window.

## Local evidence

```sh
PYTHONPATH=contracts/.venv/lib/python3.11/site-packages app/runtime/.venv/bin/python evidence/APP-006/hosted-callback/schema-one-sequence/sql-preflight/qualify.py --output evidence/APP-006/hosted-callback/schema-one-sequence/sql-preflight/local-qualification.json
```

[The final receipt](local-qualification.json) records **20 passing checks**, a disposable PostgreSQL16.13 container bound to a random loopback port, verified local TLS, and confirmed container removal. The exact emitted payload is imported for real database checks; its combined entry-point/command is also executed as a real subprocess for bounded fixed-output refusal before network access. Production offers no local endpoint or insecure-TLS option.

Successful version1 reads leave the database unchanged; an actual operator UPDATE fails with SQLSTATE25006. Role drift and an actual version2 installation are refused without repair. A temporary `pg_namespace` view fires in the unsafe-path positive control but is never evaluated by the exact safe path. A substituted version view fires in the writable control and reaches SQLSTATE25006 in the direct read-only control; the helper instead raises its catalog refusal before evaluating that expression. A committed all-role replica default is observed on a fresh connection and refused without repair. Resource tampering and alternate connection channels are also rejected.

[Author bindings](author-checks.json) retain source/resource and receipt hashes. [The initial18-check receipt](initial-local-qualification.json) predates the final entry-point descriptor and added process/version2 checks; it remains historical. These local results do not qualify actual RDS/IAM credentials, managed secret delivery, the temporary task definition, hosted auth or APP-008 acceptance.
