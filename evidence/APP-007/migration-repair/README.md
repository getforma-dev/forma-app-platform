# APP-007 local migration repair — 2026-10-28

The original migration's unconditional `ALTER ROLE ... NOSUPERUSER ... NOBYPASSRLS` cannot run as a non-superuser PostgreSQL administrator. Both options fail with SQLSTATE `42501`, even when the target role already has the requested false flag. The [local PostgreSQL record](local-postgresql16.json) preserves the actual diagnostics.

The repaired migration creates missing roles with explicit restricted flags and validates all five foundation roles before granting access or creating metadata. It refuses unexpected login/inheritance flags, superuser/database/role-creation/replication/RLS-bypass privileges, memberships other than an app role's `rds_iam`, and membership admin/inherit options. It neither demotes an unsafe existing role nor silently accepts it. For the tested clean foundation, a successful repeat preserves the observed role catalog, expected grants and schema-version row.

## Original author checks — historical PostgreSQL 16.15 run

The retained author run used the pin below. Reproduce it from the repository root without overwriting the original evidence:

```sh
python3 app/runtime/qualify_migration.py --image postgres@sha256:ca0bd484cb98bf4b24eb1010e73fb3fcbd6714d240fbc1a10eea5b7dbecb641d --output .cache/migration-regression-16.15.json
app/runtime/.venv/bin/python -m unittest discover -s app/runtime -p test_runtime.py -v
git diff --check
```

- The migration qualification passed **43 checks** against real PostgreSQL **16.15**, image `postgres@sha256:ca0bd484cb98bf4b24eb1010e73fb3fcbd6714d240fbc1a10eea5b7dbecb641d`.
- Migration execution uses a real `genui_migration` login with `CREATEROLE`/`CREATEDB`, `NOSUPERUSER` and `NOBYPASSRLS`. The fixture superuser only prepares and deliberately perturbs the local database.
- Both original prohibited `ALTER` options failed. Failed transactions left no probe role behind.
- A fresh migration refused an already-existing superuser app role, preserved its unsafe flag, and rolled back all new foundation roles/schema.
- First migration and repeat succeeded; the repeat preserved role flags, memberships and the single schema-version row.
- For all four app logins, exact expected CONNECT/schema-USAGE/metadata-SELECT grants were observed. CREATE/TEMP/schema-CREATE/metadata INSERT/UPDATE/DELETE grants were absent.
- **24 real SQL operations** failed with `42501`: each app login was denied owner/migration/sibling role selection, schema creation, temporary table creation and metadata mutation. These were ordinary read/write transactions, not a read-only-session simulation.
- **13 drift cases** were refused without changing the perturbed catalog state: seven app attributes, two owner attributes, app/owner foreign memberships and IAM membership admin/inherit options. The safe migration succeeded again after fixture cleanup.
- Existing runtime unit checks: **10 passed**. `git diff --check` passed.

The `role_grants` entries in the JSON are ordered as: session user, current user, database CONNECT, database CREATE, database TEMP, metadata-schema USAGE, metadata-schema CREATE, public-schema CREATE, metadata SELECT, INSERT, UPDATE, DELETE, schema version.

## Source identity and boundaries

- Migration SHA-256: `f98f67b7b516483692e5873741080df8ebf023dd9af9872f958258e98e3bd963`.
- Historical author harness SHA-256: `da5519d544f165e70da43faf5c5b0c78d5a14f2fc70578b8a6845a1f0a716543`. The final harness defaults to exact PostgreSQL **16.13**, manifest-list digest `5d143123fdf80462d1778cd4f24b9f7ca13c87174bca19141fb194c5a1ebca59`; the foundation workflow now runs it. Final independent checks and hashes are recorded separately in the review evidence.
- Every regression container uses a unique `genui-app007-migration-` name, `--network=none`, no published port, one CPU, 512 MiB memory and a disposable database tmpfs. The harness removes its container/volumes in `finally`; the retained run reports successful removal. The separately named initial exploration container was also removed.

This is **local regression evidence**, not managed acceptance. `rds_iam` is a simulated local group; the test uses Unix sockets and local trust authentication, not TLS or IAM tokens. PostgreSQL 16.15 does not prove exact RDS 16.13 behavior. AWS migration, per-role IAM/TLS/grant checks, tenant isolation and cleanup remain required. The author regression changed no AWS resource or auth source. The original image in `../registry-image-receipt.json` contains the old SQL. The new ARM64 candidate has been rebuilt with the repaired migration and passed [local image qualification](image-qualification.json); its independent source review, registry receipt and managed deployment are separate gates.
