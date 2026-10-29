> Historical author record before independent review found R1/R2. These passing checks did not establish drift rejection; current repair evidence is in the parent folder.

# APP-008 schema and migration slice

This is author evidence for the schema/runtime slice of [APP-008](../../../../Tickets/3-in-review/APP-008.md), implementing [ADR-004](../../../../docs/architecture/ADR-004-TENANT-TRANSACTIONS.md). It does not complete APP-008 or the Phase 1 gate. Independent review, the native pool/authorization slice and coordinated managed qualification remain separate.

## Implemented boundary

Migration0001 remains byte-for-byte unchanged (`f98f67b7b516483692e5873741080df8ebf023dd9af9872f958258e98e3bd963`). The migration manifest pins the original migration and new migration0002. The runner requires one caller-owned transaction, verifies the known version1 foundation before creating history, rejects altered checksums/partial state, applies version2 atomically, and accepts exact repeat execution. Runtime readiness requires database version2; it refuses version1 and unknown future versions. No destructive downgrade or automatic business retry exists.

Migration0002 creates 26 real tenant relations from the schema plan: organizations/provider bindings/principals/memberships; spaces/apps; definition and publication declarations; saved work and its source/record bindings; source revisions; current records and immutable revision shapes; source dependencies; and scoped permission relations. Every tenant table has tenant-leading keys and references, ENABLE/FORCE RLS, and both USING/WITH CHECK. Canonical contract bytes remain bytea; PostgreSQL JSON rewriting is not used to compute contract digests. Exact binding/dependency, definition digest and publication/schema references are enforced by composite foreign keys.

Application roles receive only the currently implemented reads. Workspace/control/data can call the member fence; only data can call the work fence. Worker has the bounded organization read. No application role can mutate business rows or permissions, truncate them, assume an owner/migration/lock role, or bypass RLS. Successful future publication, record, admission and session writes are not claimed here.

`genui_lock` is a separate NOLOGIN/NOINHERIT/NOBYPASSRLS principal with no IAM membership. It owns exactly two fixed lock/read functions, with safe search paths and no PUBLIC execution. Its only lock-enabling UPDATE privileges are one column each on organizations, memberships and saved work. It has no schema CREATE after ownership transfer. The migration operator can transfer ownership; application roles cannot assume this principal.

The member fence checks the bound organization, principal and epoch, then locks organization before membership and returns bounded current facts. The work fence checks the bound organization and required principal/epoch context, locks the exact work, and returns only the expected publication/version. NULL arguments, incomplete context, unsafe counters and stale pins fail closed. The functions do not replace native action/resource authorization; callers must use the same transaction and the staged order in ADR-004.

## Local execution evidence

[local-postgres.json](local-postgres.json) binds the exact source hashes, actual PostgreSQL16.13 image digest and individual outcomes. The tests use distinct, memory-bounded disposable containers and random loopback-only ports; cleanup of both fresh-install and upgrade containers is recorded. They use actual SQL connections under the four fixed roles and the non-superuser migration identity, not mocked row-policy outcomes.

The retained cases cover successful fresh install/repeat, verified version1 upgrade/repeat, whole-transaction rollback after an injected end-of-migration failure in both starting states, partial-state and recorded-checksum refusal, autocommit refusal, operator-seed repeat/refusal, all 26 tables' catalog/policy/key boundaries, every granted relation under both organizations, direct DML and role-assumption refusal, and real composite FK/check failures. Each lock-order test observes the waiting backend's PostgreSQL lock-wait state before releasing the holder: reader-first and writer-first revocation, organization suspension, and saved-work version advance all execute against real rows. Writer-first operations see the changed state/pin; reader-first operations retain their fence until commit.

[original-migration-regression.json](../original-migration-regression.json) retains a separate 43-check run of unchanged migration0001 against the same PostgreSQL16.13 image, including its original role/membership drift refusals. [author-checks.json](author-checks.json) records commands, hashes and the runtime unit results. The existing APP-006 secret-file handoff tests remain in the 24-test runtime suite.

Commands, run from the repository root using the existing locked environments:

```sh
app/runtime/.venv/bin/python -m unittest discover -s app/runtime -p test_runtime.py -v
PYTHONPATH=contracts/.venv/lib/python3.11/site-packages app/runtime/.venv/bin/python app/runtime/qualify_tenant_schema.py --output evidence/APP-008/schema/local-postgres.json
python3 app/runtime/qualify_migration.py --output evidence/APP-008/schema/original-migration-regression.json
```

No package was installed or reselected for these Python tests. The runtime environment supplies its pinned psycopg, and the contracts environment supplies its pinned JSON Schema validator. This local environment composition is not a production image contract; root owns CI/image packaging.

## Operator fixture interface and limits

`app/runtime/tenant_seed.py` exposes `make_bundles()` and `seed(connection, bundles)`. It creates new validated APP-002 envelopes for managed synthetic organizations `…0001` and `…0002`, preserving the original fixtures. Northstar IDs intentionally repeat across organizations; the Finance app exists only in the second organization. The operator fixture issuer is `app008-fixture-only`, tenant is `ten_<organization UUID>`, and subject is `usr_<principal UUID>`. These are isolated qualification identities, never actual fAuth provisioning identifiers.

Seed validates all bundles before writes, requires the migration identity and a transaction, uses FORCE-RLS-scoped owner writes, and verifies any existing row is an exact match. It refuses changed authority/epochs rather than resetting them. It is neither a signup route nor a general destructive reset API. APP-011 owns original CSV/XLSX byte provenance; these contract-shape source/admission digests do not claim imported bytes or live admission. Database reset integration remains explicitly pending its owning workflow.

Local trust authentication and simulated `rds_iam` membership do not qualify real IAM/TLS, managed RDS concurrency, native cancellation/reuse, hosted identity, session admission or application business writes. Root owns those separate trials and recovery coordination. The native author owns pool lifecycle, private principal mapping, permissions and transaction-bound permits.

During authoring, a catalog-constraint ordering assumption was corrected to compare sorted definitions, and the harness was corrected to separate SET ROLE from a parameterized SQL statement. Neither harness error counted as a denial pass. Author inspection also closed nullable record-binding policy and NULL fence-argument paths, added safe context counter checks, and explicitly refused autocommit. The final retained run includes regressions for these cases; historical APP-007 evidence was not overwritten.
