# APP-007 restore manifest validator — author notes

Prepared 2026-10-28. This is preparation for the bounded foundation snapshot/restore trial, not evidence that a snapshot, restore or managed comparison ran. The root operator owns those resources and the separately reserved $2 allowance. This author made no AWS calls, read no credentials, and opened no real database connection for this change.

## Implementation

`infra/scripts/foundation_restore_manifest.py` uses only Python standard-library modules and the existing image's pinned psycopg dependency. Its original source SHA-256 was `1523f47de2899801486663e976151e436ed01fa8ee916b95d2521aab551758d4` (10,725 bytes). The subsequently authorized column-ACL repair below produces SHA-256 `87d62d667e0a65235a9718bed6ec9f301db732aff974acc346e7a04c5c7fc70c` (10,744 bytes).

The program requires the explicit `GENUI_QUALIFY_SYNTHETIC=APP-007` marker, effective UID 65532, `PGUSER=genui_migration`, `PGDATABASE=genui`, port 5432, an us-east-1 RDS hostname and the image's existing RDS CA path. It reads `PGPASSWORD` only inside the task's connection call; this remains an ECS execution-secret injection, never an override or operator argument.

Connections explicitly use `sslmode=verify-full`, a five-second connection timeout and a single repeatable-read/read-only transaction. Statement timeout is five seconds, lock timeout two seconds, idle-in-transaction timeout ten seconds, and the process has a 60-second alarm. The connection is rolled back even on validation failure. Actual database identity, negotiated SSL, PostgreSQL 16.13, transaction read-only state and timeout settings are checked.

The sorted comparison manifest contains:

- The exact singleton version-1 row and PostgreSQL version number.
- All five foundation roles' non-secret flags, direct membership grants touching them, and their complete membership closure, with role names replacing OIDs.
- Database/schema/table ownership and normalized ACL entries, resolving default ACLs and PUBLIC grants explicitly.
- Column shape, table ownership, column ACLs and metadata constraints.
- Effective app grants for all four logins.

The validator rejects elevated/misconfigured roles, unexpected inherited memberships, a writable public database, altered metadata shape or unexpected effective application write grants. ACL differences otherwise remain visible in the comparison. Canonical JSON uses sorted object keys and sorted row lists; SHA-256 covers only `manifest`. Hostnames, credentials, raw OIDs, connection timestamps, TLS session details and the separately reported assertions are absent from that digest. Neither this program nor its queries access `pg_authid` or execute a migration.

Success prints one JSON object with `passed:true`, sanitized assertions, `manifest_sha256` and `manifest`. Failure prints one fixed JSON error with `passed:false`, exits 1 and never prints the underlying exception.

## Task handoff

Create a temporary task-definition copy from the reviewed migration definition, retaining its image/architecture, 2-vCPU/8-GiB allocation, execution/task roles, readonly root, UID, log configuration, CA and existing `PGPASSWORD` secret reference. Set its container entrypoint to `["python3","-c"]` and command to a one-element array containing the exact reviewed script source. Put the source in the registered definition, not the run-task override. Do not add data grants to the app roles.

For each run, use the approved migration network and only these container environment overrides:

```json
{"containerOverrides":[{"name":"migration","environment":[{"name":"GENUI_QUALIFY_SYNTHETIC","value":"APP-007"},{"name":"PGHOST","value":"<source-or-restored-RDS-endpoint>"}]}]}
```

Run once on the migrated source before snapshot creation, then once on the exact restored endpoint **before any migration or reseeding there**. Preserve the actual task-definition revision, source hash, task/image identities, exit codes and complete sanitized JSON reports. Both tasks must exit zero, stop and report `passed:true`; their `manifest_sha256` values and complete `manifest` objects must be equal. The operator binds each run to its actual endpoint/resource identity outside the comparison manifest. Any failure or mismatch stops acceptance and must be retained, not normalized away. Finish exact task/definition/restore/snapshot cleanup through the root-owned plan.

## Local verification and limits

An in-memory fixture harness exercised **18 checks**, with psycopg's connection replaced by a stub and environment values replaced by explicitly synthetic placeholders. Checks covered complete canonical output; reversed row-order equality; grant-drift digest changes; wrong identity/database/engine; missing TLS/read-only/timeouts; privileged role; unexpected membership; missing column; PUBLIC database grant; app write grant; UID/opt-in preconnection refusal; explicit TLS/timeout arguments and rollback; and exception redaction. No real password was read. `git diff --check` passed.

These fixtures establish Python behavior only. SQL execution, actual catalog shape, RDS16.13 role semantics, TLS, snapshot completeness, manifest equality and cleanup remain managed trial obligations. This foundation manifest does not prove later APP-053 application/audit/deletion-ledger recovery.

## Subsequent local SQL compatibility and repair

At root's request, a no-network/no-host-port PostgreSQL 16.13 fixture executed the actual catalog queries after the repaired migration. The first run found a SQL bug in the original column-grant expression: `aclexplode('{}'::aclitem[])` rejects the zero-dimensional array used as a fallback for ordinary NULL column ACLs. The failure was reported before source changes and is retained in [the SQL regression evidence](restore-local-sql/README.md).

Root authorized the minimal repair: use `aclexplode(a.attacl)` directly and explicitly require an empty column-grant list for this initial foundation baseline. All 14 real queries now execute, validate and canonicalize; repeated full manifests match. A deliberately introduced column UPDATE grant appears in the query, changes the digest and is refused by validation. After revocation, all queries pass and the original manifest returns exactly. Both named fixture containers and their volumes were removed; cleanup inventories are retained.

This follow-up establishes local PostgreSQL16.13 SQL/catalog compatibility only. It uses a simulated `rds_iam`, local socket/trust authentication and does not invoke `run()`/`collect()` or prove TLS, RDS, ECS secrets, snapshot/restore or managed cleanup. No AWS calls or credential reads occurred.
