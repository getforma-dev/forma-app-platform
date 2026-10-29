# APP-008 native R5 supplement: independent review

Reviewer: `/root/app008_native_review`, 2026-10-29 UTC. Implementation owner: `/root/auth_readiness_answer`. This review changed only its evidence files; no implementation, ticket, Git or cloud changes.

**Disposition: no open actionable native source findings in the exact candidate bound by [independent-review.json](independent-review.json).** The independent local PostgreSQL/TLS campaign passed all **36 checks** with unchanged before/after inputs. This approves the reviewed source/local behavior only; it does not complete APP-008 or pass a managed/Phase 1 gate.

## Reviewed repair and regression

The [R4 review](../r4/independent-review.md) remains a historical exact-source receipt. R5 closes the subsequently reproduced temporary-schema lookup gap: a bare `pg_catalog` path still allows an existing temporary schema to precede built-in relation names. A read-only transaction does not prevent evaluation of a temporary sequence, so read-only mode alone does not establish the intended catalog lookup boundary.

The production startup options now establish the exact path `pg_catalog, pg_temp` before SQL. Identity and clean-session baseline checks require that value; successful reset reinstalls it. The catalog document validator requires the same value, and canonical catalog verification explicitly installs it transaction-locally. The built-in catalog names used by the query therefore resolve ahead of temporary names. Fully qualified initial identity and post-catalog schema-version ordering from R4 remain intact. No TEMP privilege or arbitrary SQL path was added to the application.

The independently executed regression first proves ordinary TEMP creation is denied. Within the disposable fixture only, the operator temporarily grants TEMP so a private test connection can create a temporary `pg_namespace` view whose predicate increments a temporary sequence. The operator revokes the grant before verification. Executing the canonical query under the old path inside READ ONLY sets the marker, including after rollback; this is a meaningful positive control rather than a permanent write that read-only mode would simply prohibit.

After clearing the marker, the corrected `Catalog::verify` leaves it untouched. The test then returns the clean object and acquires it through the real pool: its unchanged backend ID proves actual same-connection recycle, and the marker remains untouched. Successful finish/reset removes the temporary marker while preserving safe same-backend reuse; readiness succeeds afterward. The direct pool/client seam and operator grant exist only in the isolated test module. All prior authority, tenant isolation, catalog/version, replication-mode, cancellation, connection-capacity, credential and TLS cases also pass in the same campaign.

## Independent checks and exact inputs

From `app/`, with `CARGO_BUILD_JOBS=1`:

```sh
cargo +1.93.1 test --locked --lib
cargo +1.93.1 test --locked --doc
GENUI_NATIVE_REPORT=/temporary/private/local-postgres-tls.json cargo +1.93.1 test --locked --lib storage::qualification::native_postgres_campaign -- --ignored --exact --nocapture
```

18 library tests and the private-construction compile-fail doctest passed. The separately invoked actual PostgreSQL 16.13/TLS campaign passed 36 checks in 39.006 seconds including command overhead. The runtime receipt includes cleanup of its owned local database and private files. Exact commands, full output, runtime receipt and before/after hashes are embedded in the JSON. The schema owner confirmed fixture resources were frozen before execution; every author-pinned source plus ADR-004 stayed unchanged. Author production-library, strict lint and Rust 1.85 floor receipts were inspected but not independently rerun.

| Input | SHA-256 |
| --- | --- |
| `app/src/storage/pool.rs` | `4a8eae301412ed8ce48ef2b84e08919380747e17179ae314b61c15d1d429a0d5` |
| `app/src/storage/catalog.rs` | `8c0462983afb7e7d877f661d3c49b63eae71de0b0b95c5e61f7c291c1f2cf35a` |
| `app/src/storage/qualification.rs` | `14f8042623a529b8643106dc9e122b0d10b47b596651f2457eea0e4b4c9b9582` |
| `app/migrations/catalog-query.json` | `e17ceb5e62dc6f03dfb9dc26f34018d3f2977e1b93ee5910ed16f03364a63963` |
| `app/migrations/catalog-v2.json` | `51418a12cf45809b2e4a3ea7e9ee6deeed8d2189a4dbc936fe220698ef031954` |

## Remaining gates

The Python/catalog counterpart has a separate author and reviewer. Root retains final binary/HTTP smoke, image input/build checks, current-source CI and actual managed RDS isolation/reuse/race/recovery qualification. Local SCRAM, a throwaway TLS CA, fixture identities and the synthetic positive readiness helper do not establish RDS IAM, S3/KMS, hosted identity/session/admission or business writes. No protection against a hostile database administrator changing DDL after validation is claimed. The original private-authority and later-delivery limitations remain in force; this review changes no ticket status.
