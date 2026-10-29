# APP-008 native R3 supplement: independent review

Reviewer: `/root/app008_native_review`, 2026-10-29 UTC. Review-only ownership; no implementation, ticket, Git or cloud changes.

**R3 candidate requires repair; this receipt is retained unchanged as the finding record.** R3's replication-origin checks and its three actual local regression scenarios were source-reviewed. This supplement found the separate admission-ordering defect below before repeating the final native campaign. The earlier 29-check review remains an exact-source historical receipt, not approval of this changed candidate.

## Open finding NATIVE-R3-01 (P2)

The frozen R3 `RoleManager::connect` calls `verify_identity` before canonical catalog comparison. That query reads `genui_meta.schema_version` while its object kind and definition are still unchecked. It also resolves `pg_roles`, `pg_stat_ssl` and functions using the unverified database startup search path. Catalog refusal afterward cannot undo side effects from the earlier autocommit query.

I reproduced both paths against canonical migrations in an isolated local PostgreSQL 16.13 container. Replacing `schema_version` with a view that called a marker function produced one committed marker row during the exact native initial identity SQL; the canonical catalog then mismatched. Separately, a committed database search path of `reviewer_probe, pg_catalog` resolved the initial `pg_roles` reference to a marker view and produced one marker row. The owned container was removed. This reproduction tests query ordering, not TLS, and does not claim that application roles have DDL authority. It demonstrates failure of the promised refusal before using unexpected database objects under operator-visible drift.

Requested repair: establish a fixed `pg_catalog` path at connection startup; use only qualified built-in catalogs for initial identity validation; compare the catalog before reading the schema-version relation; retain current supported-version checks on new and reused sessions. Add actual native TLS regressions proving refusal without marker execution, recovery after operator restoration, and version-1 metadata rejection even when the structural v2 catalog is unchanged.

## R3 source assessment

The initial identity and baseline checks require `session_replication_role=origin`. Cleanup refuses non-origin state before `DISCARD ALL` can hide it. No application `SET session_replication_role` repair or privilege grant was added. The three qualification cases cover an inherited all-role database default, an operator-enabled leased-session change, and a poisoned idle connection; they observe the inherited state and backend replacement. The canonical projection now includes effective SET/ALTER SYSTEM privilege checks and database-wide role settings. Those controls remain distinct from the startup-ordering finding.

Full reproduction and input hashes are in [independent-review.json](independent-review.json). The repaired candidate and final native test results will have a separate [R4 review](../r4/independent-review.md); this R3 finding record will not be relabeled as passing. Managed RDS, image, CI, hosted identity, business writes and ticket completion remain outside this review's approval scope.
