# APP-007 restore-manifest SQL compatibility

This directory contains a focused local SQL/catalog check for the committed restore manifest validator. It creates one uniquely named disposable PostgreSQL 16.13 container with no Docker network and no published host port. The only administrator is a synthetic local `genui_migration` role, with a simulated `rds_iam` group. The repaired migration is applied through a local Unix socket before executing the validator's actual `QUERIES` statements.

Run from the repository root:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 evidence/APP-007/restore-local-sql/qualify.py --output-dir .cache/APP-007/restore-sql
```

The exact PostgreSQL image is `postgres@sha256:5d143123fdf80462d1778cd4f24b9f7ca13c87174bca19141fb194c5a1ebca59`. The fixture is limited to one CPU, 512 MiB memory and a 256-MiB disposable database tmpfs. Startup waits for TCP availability inside the no-network container so the image's temporary Unix-socket bootstrap server cannot be mistaken for the final server. It publishes no port and sends every SQL command through `docker exec` and the local socket.

`bootstrap.sql` records synthetic setup; `queries.sql` records the actual catalog SELECTs, with the validator's fixed role-name lists substituted for driver placeholders. No other query expression is changed. The test wraps each query's rows as JSON so the committed `validate()` and `canonical()` functions can inspect actual SQL results without invoking the runtime `run()`/TLS path. `results.json` records exact command arrays, input digests, outputs, source hashes, manifest comparisons and cleanup. The full-run comparison is intended to execute twice and require identical manifests.

## Preserved initial failure

The first run against validator SHA-256 `1523f47de2899801486663e976151e436ed01fa8ee916b95d2521aab551758d4` failed with exit code 1. PostgreSQL executed the first eleven catalog queries, then rejected `column_grants` with SQLSTATE `22023`: `ACL arrays must be one-dimensional`.

The cause is `aclexplode(COALESCE(a.attacl,'{}'::aclitem[]))`. Ordinary metadata columns have `attacl=NULL`; the fallback is a zero-dimensional empty ACL array, which `aclexplode` rejects. This is an actual SQL incompatibility, not a connection/TLS result. The failure was reported to the root owner before any source change.

The exact initial record is preserved as `initial-failure.json`, with `initial-queries.sql`. Cleanup of container `genui-app007-restore-sql-846325a8a22e` and its volumes was confirmed, followed by an empty exact-name container inventory. A later successful run must remain separate from this failed evidence.

## Authorized minimal repair and successful rerun

Root authorized a two-line helper repair: call `aclexplode(a.attacl)` directly so NULL returns no rows, and require `column_grants == []` because the version-1 migration grants SELECT only at table level. The repaired validator SHA-256 is `87d62d667e0a65235a9718bed6ec9f301db732aff974acc346e7a04c5c7fc70c`. No other validator behavior changed.

`results.json` records the successful repaired run on PostgreSQL **16.13**:

- All **14 actual QUERIES** execute and pass the committed validator's `validate()` and `canonical()` functions. Repeated full manifests match.
- Both real foundation columns have NULL ACLs and produce an empty column-grant list. A direct `aclexplode(NULL::aclitem[])` query returns zero rows.
- A real `GRANT UPDATE(version) ON genui_meta.schema_version TO genui_data` is reported by `column_grants`. Validation refuses it even though table-level write privilege checks do not capture this column-specific permission.
- The baseline manifest digest is `c167e503ea2485f2cae0871969ad36da2758cea4cf7c1805deee3ae2a26f0f53`. The altered manifest digest is `d0c8d7699d8f3df1052ccf82a80cb1f8647cb11c3932b99144f81a3023476f23`.
- After explicitly revoking that UPDATE grant, all 14 queries and validation pass again, and the entire original manifest is restored exactly.
- Container `genui-app007-restore-sql-e287a861cb15` and its volumes were removed; exact-name inventory was empty. `git diff --check` passed.

## Limits

This is local PostgreSQL 16.13 SQL regression only. It does not invoke `run()` or `collect()`, satisfy UID65532/verified-TLS checks, use the ECS secret channel, contact AWS, create a snapshot, restore a database or establish managed acceptance. No credentials are read. Root owns review and the subsequent managed trial.

The current harness writes to a disposable output directory by default; the retained author records above are not overwritten by reproduction. `--output-dir` selects a separate destination, and fixture cleanup failure fails the command. Foundation CI runs the same SQL regression and uploads its separate result.
