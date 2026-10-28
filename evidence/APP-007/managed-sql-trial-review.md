# APP-007 managed SQL trial — author notes

Prepared 2026-10-28. This is the implementer's review handoff, **not an independent review or a managed pass**. The script was not executed against AWS or a database while preparing these notes. It extends APP-007 foundation evidence only; APP-008 tenant tables, FORCE RLS, scoped transactions/pool reset and cross-organization authorization remain unqualified.

## Invocation

The source is [`infra/scripts/managed_sql_trial.py`](../../infra/scripts/managed_sql_trial.py). Use one temporary task-definition copy of the independently reviewed application image and configuration, with the same application task role, execution role, network/subnets/security group, UID 65532, read-only root and private writable `/run/genui` volume. Do not use the migration task or inject migration credentials.

Change only the qualification container's process to `entryPoint: ["python3", "-c"]`, `command: [<the exact reviewed script source as one JSON string>]`, and add `GENUI_QUALIFY_SYNTHETIC=APP-007` to its environment. Preserve the application environment documented in [`infra/README.md`](../../infra/README.md), including `GENUI_RDS_CA_FILE`. Supply the source through the task definition, not the size-limited RunTask overrides. Record the source SHA-256, image digest, original and temporary task-definition revisions and actual task result separately. Run once; inspect its exit status and structured report, then deregister the temporary definition. No service update is needed for this one-off trial.

The equivalent entry point when the reviewed source is already available inside that container is:

```sh
GENUI_QUALIFY_SYNTHETIC=APP-007 python3 /path/to/managed_sql_trial.py
```

No script arguments, connection strings, tokens or secrets belong in the task definition/command. The source imports `/app/runtime.py` explicitly and calls its `refresh` and `load` functions. The existing helper assumes all six constrained application roles while refreshing; this trial only uses the four SQL tokens and performs no S3/KMS data operations. A separate temporary mode-0700 directory under `/run/genui` contains the mode-0600 credential generation and is removed on ordinary exit, including exceptions. It neither reads nor replaces the service credential file. ECS task teardown must remove the task-local volume after forced termination.

## What a successful report establishes

Each of the four actual logins must satisfy all of these checks:

- `sslmode=verify-full`, the packaged RDS trust bundle, minimum TLS 1.2, and both libpq/server confirmation that TLS is active.
- Distinct expected `session_user` and `current_user`; LOGIN and NOINHERIT; no superuser, database/role creation, replication or RLS-bypass attributes. Both default and active transaction read-only settings must be off.
- Effective database CONNECT, metadata schema USAGE and version-table SELECT; the actual table read must return exactly `(true, 1)`. Database CREATE/TEMP and CREATE in `public` and `genui_meta` must be absent.
- No effective metadata table INSERT/UPDATE/DELETE/TRUNCATE/REFERENCES/TRIGGER or column INSERT/UPDATE/REFERENCES privileges. Membership is limited to self and `rds_iam`, and direct grants may have neither admin nor inherit options.
- The same libpq client, CA and actual positive connection's IP must reject the deliberately mismatched hostname `app007.invalid`. Only the specific certificate-hostname-mismatch error is accepted; DNS, timeout, authentication, generic TLS failure or a successful connection fails the trial. This test runs separately with each login's parameters.
- The owner, migration, `rds_superuser` and all three sibling roles must exist, then actual `SET ROLE` attempts to each must fail with SQLSTATE `42501`.
- Actual CREATE SCHEMA, CREATE TEMP TABLE, CREATE TABLE in `public` and `genui_meta`, and metadata INSERT/UPDATE/DELETE attempts must fail with SQLSTATE `42501`. The INSERT uses a zero-row source so an unexpectedly allowed operation cannot violate the singleton constraint before the privilege assertion. UPDATE/DELETE target the existing synthetic version row and are rolled back.

Every mutation runs in its own explicit `BEGIN READ WRITE` transaction, checks read-only is off, and unconditionally calls rollback before evaluating the denial. This also rolls back an unexpectedly successful SET ROLE, DDL or DML operation. The script requires idle transaction state and restoration of the original identity after rollback. A connection is always closed, so transport/rollback failure cannot lead to commit. Generated object identifiers are 39 ASCII characters (`app007_` plus 32 UUID hex digits). There are no CREATE DATABASE or other nontransactional mutation probes. Metadata is read again after each role's probes.

The final credential generation must still pass the runtime's expiry/private-file validation and match the original file digest. Success is exit code 0 with top-level `passed: true`; any failed assertion, interruption or unexpected operation is exit code 1 with `passed: false`. The report contains at most 88 fixed-label checks, a synthetic run UUID, fixed role names, booleans and validated five-character SQLSTATE values. No database host/address, connection string, credential generation, raw exception, SQL result payload or AWS response is printed. The hostname diagnostic is inspected only in memory and never emitted.

## Bounds and local verification

- No application retry loop. Each fresh connection has a three-second timeout; SQL statements have a three-second timeout, lock waits one second, idle transactions five seconds, and the process has a 120-second alarm. The helper's AWS clients already use one total SDK attempt and two-second connect/read timeouts. A forced task kill or kernel-level hang can prevent the final report; absent/incomplete output is never a pass.
- At most eight SQL connections: four positive sessions and four deliberate TLS refusals, sequentially. Every expected denial is checked before proceeding to the next probe/role. Unexpected grants or successful operations stop qualification.
- Python AST parsing passed (11,798-byte initial source). An inline, in-memory connection harness passed seven mutation cases: expected privilege denial, unexpected allow, read-only SQLSTATE `25006`, unrelated SQLSTATE `23505`, read-only session before mutation, identity not restored, and rollback failure. It confirmed a rollback attempt in every started transaction and no mutation when the read-only check failed. Separate checks passed for sanitized unarmed invocation and rejection of an invalid SQLSTATE. These tests use doubles and establish helper control flow only.
- The pinned local psycopg `3.3.6` exposes the used `ConnectionInfo.hostaddr` and `PGconn.ssl_in_use` APIs. No dependency, runtime, migration or image change was made for this script.

The negative TLS mechanism follows PostgreSQL's documented separation of the network `hostaddr` from the name checked by `verify-full`; see [libpq connection parameters](https://www.postgresql.org/docs/16/libpq-connect.html#LIBPQ-PARAMKEYWORDS), [certificate verification](https://www.postgresql.org/docs/16/libpq-ssl.html#LIBPQ-SSL-CERTIFICATE-VERIFICATION), and [psycopg connection information](https://www.psycopg.org/psycopg3/docs/api/objects.html#psycopg.ConnectionInfo.hostaddr). It establishes hostname rejection using the actual client stack. It does not independently test every invalid-CA/expired-certificate case or perform a certificate-rotation exercise.

Independent source review and the actual managed run are pending. Retain failed outcomes; do not retry blindly or convert these author notes into acceptance evidence.
