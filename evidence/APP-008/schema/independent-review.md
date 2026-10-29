# APP-008 schema/runtime independent review

**Latest schema/runtime verdict: no blocking findings at the final hashes below.** R1–R3 are resolved; earlier findings and failed reproductions remain historical evidence. This limited review does not approve managed deployment or complete APP-008.

**Initial review — changes requested.** Reviewed the uncommitted schema/runtime slice at repository parent `2c5760248d2fda8c71bb78f8a2a0fc1d64d1651e`, using the exact hashes in [independent-review.json](independent-review.json). The reviewer did not implement the slice. At that revision, two existing-state paths could report a successful migration without establishing the required schema/security baseline.

## Blocking findings

### R1 — Validate the existing version2 catalog, not only its history rows

**P1**, [runtime.py](../../../app/runtime/runtime.py) (line 367), lines367–374 at reviewed hash `8045ebb7…`.

When both expected history rows and `schema_version=2` exist, `apply_migrations()` executes no SQL migration and checks no version2 relations, policies, ownership, functions or grants. In a disposable PostgreSQL16.13 database, the reviewer installed version2, then observed that migration repeat still succeeded after each of these changes: dropping `genui.organization_permissions`, disabling RLS on `genui.organizations`, and granting publication UPDATE to `genui_workspace`. Those changes do not alter the history checksum. The latter two states violate tenant isolation and the explicit no-business-write boundary while the migration command reports success.

Require the reviewed version2 structural/security baseline before accepting an already-version2 database and after applying the migration. Retain negative tests for missing/altered relations, RLS/owner/policy/function/ACL drift and exact unchanged-state refusal. Do not silently repair drift or rewrite its history. The independently repeated106-check campaign does not cover this path.

### R2 — Do not let a preexisting history table bypass version1 verification

**P1**, [runtime.py](../../../app/runtime/runtime.py) (line 356), lines356–370 at the same reviewed hash.

`verify_initial_foundation()` runs only when `schema_migrations` is absent. A preexisting one-row version1 history is accepted directly and proceeds to migration2. The reviewer created the same history table and relaxed version constraint that the runner normally creates, recorded the genuine version1 checksum, then changed `genui_workspace` to INHERIT. `apply_migrations()` succeeded and left that role drift in the resulting version2 database. The normal no-history path would refuse the same unsafe baseline.

This runner creates history and migration2 in one transaction, so a committed version1-only history is not a normal completed state of this two-migration implementation. Refuse that partial state, or explicitly verify a separately supported version1-with-history baseline before continuing. In either case, a history row cannot bypass the initial role/schema proof. Add a regression showing refusal and preservation of the drifted state.

## Inspected boundary and successful checks

The new migration defines the26 real planned tenant relations, tenant-leading keys/references, FORCE RLS and explicit USING/WITH CHECK. The two definer functions are schema-qualified, have fixed search paths, revoke PUBLIC execution and are owned by the separate `genui_lock` role. Its only UPDATE privileges are the three columns needed for row locks; application roles receive no direct business DML. Member locking follows organization then membership; work locking remains a same-transaction primitive whose caller must first perform native authorization. These observations do not resolve R1/R2.

The reviewer independently ran:

- `PYTHONPATH=contracts/.venv/lib/python3.11/site-packages app/runtime/.venv/bin/python app/runtime/qualify_tenant_schema.py --output evidence/APP-008/schema/independent-postgres.json`:106checks passed on actual PostgreSQL16.13; both disposable containers removed. The deterministic receipt is byte-identical to the author's retained report.
- `app/runtime/.venv/bin/python -m unittest discover -s app/runtime -p test_runtime.py -v`:24tests passed in9.531seconds, including the existing APP-006 private-file lifecycle and exact-version readiness checks.
- Two additional bounded disposable PostgreSQL reproductions: R1 accepted all three corruptions; R2 accepted partial history plus role drift. Each container was removed. Exact observations and reproduction steps are retained in the JSON review record.

The original migration hash remains `f98f67b7b516483692e5873741080df8ebf023dd9af9872f958258e98e3bd963`. The reviewer inspected the author's separate43-check original-migration regression receipt without rerunning that unaffected campaign.

## Limits and reset follow-up

This is a schema/runtime review, not native pool or route acceptance. The PostgreSQL harness uses local trust authentication and simulated `rds_iam`; no AWS operation, managed IAM/TLS, production deployment, hosted identity, source import, admission, successful application business write or Phase1 acceptance was performed. CI, final source/image binding, managed trials and the native authorization review remain required.

The operator seed intentionally refuses changed existing authority/epochs and is not a reset API. For APP-011, a subsequent reset qualification should bind an explicitly owned disposable environment, exact schema/fixture manifest, operator identity and no-active-consumer precondition; preserve failed evidence and test refusal for a foreign/shared target. Recreating a reviewer-owned disposable database can establish local reset reproducibility, but it must not be described as managed restore or reset of a shared tenant. No reset implementation was added in this review.

Root and the schema author were notified of both blocking paths. Re-review must bind repaired source hashes and include the new negative cases; this verdict must not be relabeled as approval of later code.

## Follow-up: R1/R2 repaired; R3 remains blocking

Reviewed the repaired source bound in the JSON record's `followups` entry: runtime `580e1e3698262d5bdffa9f34d1ef9ab120fb94ea10213437c6c02e282daf0113`, canonical catalog query `028d7a84db3e7bbade26fd12c6fcc363fae4e93fe04777d011d07287085892cf`, and manifest `858429ac1d15344b8f7738d4d02a67fe29f3d45d5f886223dade637046e85629`. Original findings and results above remain preserved as history.

**R1 and R2 are resolved at these hashes.** A pinned, readable logical catalog now covers relevant schema/column/constraint/index/policy/function/ownership/ACL facts, role attributes and memberships. It is verified before legacy bootstrap, before accepting version2 and after migration2. Preexisting partial history1 is refused. The new generator only creates review candidates in a fresh local environment; it does not learn expected state from a managed target or overwrite authoritative files.

The reviewer independently repeated the full142-check PostgreSQL campaign and30 runtime tests. All passed; the six database containers were removed. [independent-postgres-repair.json](independent-postgres-repair.json) preserves this second run separately from the earlier106-check evidence. The30 runtime tests ran in11.550seconds. The six new empty-registry tests cover actual child-process handoff as well as refusal/cleanup paths. Inspection found no blocking issue in the default deny-all registry: absent paths receive a fresh owned0600 file under0700; explicit paths, including empty/invalid values, remain for native validation; cleanup refuses replacements and preserves unrelated files. APP-006 raw-auth handoff remains unchanged.

### R3 — Include database-wide defaults and verify the effective replication role

**P1**, [catalog-query.json](../../../app/migrations/catalog-query.json) (line 6), `role_database_settings`, plus the native/migration session baseline integration.

The catalog query joins `pg_db_role_setting.setrole` to `pg_roles` with an inner join. Database-wide settings have `setrole=0`, so they disappear from the comparison. The reviewer installed version2 in a fresh PostgreSQL16.13 container and committed `ALTER DATABASE genui SET session_replication_role=replica`. New workspace and migration connections both reported `replica`; nevertheless `verify_schema_catalog()` and `apply_migrations()` both succeeded. The temporary container was removed.

In this state PostgreSQL's normal origin FK triggers are disabled for new sessions, while the stored trigger-enabled flags still match the expected catalog. This defeats the effective constraint baseline without changing a migration checksum, table shape or trigger definition. The current native baseline also does not inspect this setting.

Include the current database's all-role defaults in the canonical projection, and require effective `session_replication_role=origin` in migration and native admission/reset. The effective check also handles an inherited server or operator-session setting that is not represented by a database-wide catalog row. Add committed unchanged-state refusal and actual effective-session regressions; do not silently reset or adopt unexpected catalog defaults as a successful repair.

**Verdict at that follow-up: changes requested for R3.** Root and the schema author received the exact reproduction. Managed catalog compatibility is still unproven: PostgreSQL/RDS membership representation and relevant effective privileges must be observed against the reviewed baseline before migration. Local acceptance must not cause the expected baseline to be regenerated from a differing live target. Native lifecycle, real RDS IAM/TLS, source CI/image and application/phase acceptance remain separate gates.

## Final follow-up: R1–R3 resolved

**No blocking schema/runtime findings** at runtime `a488f35b92a2d0413c236a6a3bf35097470bbd1a7780521120a872713dc93ea9`, query `3af18e8ec8b58b52e7956dc900deae8dffc9a585bc1db9e1ce1ab4dee02ed6bc`, version2 catalog `5772132f163b5490b8d39f6d1fdc26cef388100daf9fd8e86cc7aa9f8ebffe07`, and manifest `a3130b979cf84851eb05e930a52fc4a7439e73da820155bb87424436fbb1eff2`. The final JSON follow-up binds every reviewed schema/runtime/resource file and the amended ADR.

R3 is resolved by including current-database all-role defaults through a left role join and explicit `ALL_ROLES` sentinel. The canonical fixed-role facts also require the expected absence of SET and ALTER SYSTEM privilege on `session_replication_role`. Python migration entry checks actual origin mode before any foundation write; catalog verification checks it again. Neither changes the unexpected mode as a repair. The existing R1/R2 structural/ACL and partial-history refusals remain intact.

The reviewer independently ran the final146-check PostgreSQL campaign and30 runtime tests. Both passed, with all seven campaign containers removed; the runtime tests completed in11.629seconds. [independent-postgres-r3.json](independent-postgres-r3.json) preserves this third run. The actual database cases distinguish a benign all-role default from replica mode, refuse a parameter SET grant while the session is still origin, and preserve committed drift after refusal.

One additional independent real-database check enabled replica mode on a fresh migration connection before any migration had run. The entry point refused, preserved replica mode and created none of the foundation schemas or fixed roles. That extra disposable container was also removed. This confirms the early guard prevents fresh-install writes as well as refusing an already-version2 repeat.

The default empty-registry lifecycle and original auth-file lifecycle passed again on final source. The new tenant-foundation CI definition uses the locked runtime/contract validator dependencies and real schema/native campaigns; the existing foundation check runs the runtime unit suite. This is source inspection of CI wiring, not a successful remote CI run.

Native origin admission/reset/recycle checks were inspected only for integration direction; their final full review and actual lifecycle campaign remain separately owned. A separate native observation about reading schema metadata before catalog verification was sent to root for owning review, without being represented as a proved schema/runtime finding. Managed catalog compatibility, real IAM/TLS, deployment/restore trials and source/image CI remain required before the corresponding gates close. This final schema verdict does not pass the whole APP-008 ticket, a hosted auth journey or Phase1.
