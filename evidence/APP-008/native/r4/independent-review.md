# APP-008 native R4 supplement: independent review

Reviewer: `/root/app008_native_review`, 2026-10-29 UTC. Implementation owner: `/root/auth_readiness_answer`. This review made no implementation, ticket, Git or cloud changes.

**Disposition: no open actionable native source findings in the exact candidate bound by [independent-review.json](independent-review.json).** The independently executed final PostgreSQL/TLS campaign passed all **35 checks** with unchanged before/after source hashes. This is source/local qualification approval, not APP-008 completion, managed RDS acceptance or Phase 1 acceptance.

## Finding resolved

The [R3 review and reproduction](../r3/independent-review.md) are retained as the historical finding record. The frozen R3 initial identity query could execute an unexpected `schema_version` view or a shadowed `pg_roles` view before canonical catalog comparison. Local reproduction produced committed marker rows before rejection; it did not claim that application roles had DDL authority. That review appropriately withheld approval of the changed source.

R4 establishes `search_path=pg_catalog` through the PostgreSQL startup options, before driver type discovery or application SQL. Initial identity validation references qualified built-in catalogs/functions/types only and verifies the actual path and `origin` replication mode. The shared admission check verifies identity, then compares the embedded canonical catalog, then reads the supported schema version. Connection creation, ordinary pool recycle and readiness all use this sequence. Recycle checks the qualified clean-session baseline before admission. A failed or canceled check retains the existing dirty-connection destruction behavior and bounded acquisition deadline.

The real TLS regressions use operator-installed traps solely inside the disposable fixture. Each trap first proves that the worker role can execute it and create marker rows. After clearing the markers, every fixed role must refuse acquisition and readiness for both new and previously pooled sessions without creating a marker. Restoring the original metadata relation or database path restores readiness. The idle-path case also poisons a clean pooled session through a test-only internal seam. These tests directly distinguish safe pre-use refusal from a failure after unexpected SQL has run.

The additional version test sets the legal metadata value to 1 while keeping the complete structural v2 catalog unchanged. It explicitly proves that canonical catalog verification still succeeds, then verifies native rejection for fresh connections, pooled reuse and readiness until the operator restores version 2. This independently exercises the post-catalog version read rather than relying on a changed constraint to cause catalog refusal.

## R3 replication-mode repair

The initial identity and reusable baseline require `session_replication_role=origin`. Cleanup checks the actual mode before `DISCARD ALL`, so reset cannot conceal a changed session. No application replication-mode repair or SET privilege was added. The final campaign independently exercises the three R3 regressions: an inherited committed database default observed by all four roles, operator-enabled leased-session drift refused before reset, and poisoned idle-session rejection. Replacement backend IDs and restored-origin observations demonstrate disposal/recovery. The shared catalog also binds effective replication-parameter SET/ALTER SYSTEM privileges and database-wide defaults; its schema projection and operator-edge normalization have their own separate schema review.

## Independent verification

From `app/`, with `CARGO_BUILD_JOBS=1`:

```sh
cargo +1.93.1 test --locked --lib
cargo +1.93.1 test --locked --doc
GENUI_NATIVE_REPORT=/temporary/private/local-postgres-tls.json cargo +1.93.1 test --locked --lib storage::qualification::native_postgres_campaign -- --ignored --exact --nocapture
```

- 18 library tests passed; the Docker qualification was separately invoked rather than skipped as acceptance evidence.
- The compile-fail public-construction doctest passed.
- All 35 actual local PostgreSQL 16.13/TLS checks passed. The final campaign command took 38.278 seconds including process overhead. Its full output and runtime receipt are embedded in the JSON.
- The 35 checks retain prior authority, two-tenant isolation, pool capacity, cancellation/reset, credential rotation, catalog drift, role/TLS and composed-readiness coverage, and add the six R3/R4 cases described above. The campaign verifies removal of its owned local database and private fixture files.
- All inputs from the author's frozen receipt, plus ADR-004, were hashed before and after the independent run and remained unchanged. The author's Rust 1.85 floor, strict lint and production-library evidence were inspected; this supplement does not claim an independent repeat of those commands.

Final principal pins:

| Input | SHA-256 |
| --- | --- |
| `app/src/storage/pool.rs` | `56d44535b2ea0d1e4f1f78764b1a700fc8eb1e6c9a6ee9d4f83f56af100d0f99` |
| `app/src/storage/qualification.rs` | `7680e0e80c43531f855fe7dd1168d988c17ce779afdcbe8147108a9f14e2c3c9` |
| `app/migrations/catalog-query.json` | `dd86a4be87895fe4ddb8ccd24e31da50f6705754ffc2dc51709c1b7171f24422` |
| `app/migrations/catalog-v2.json` | `51418a12cf45809b2e4a3ea7e9ee6deeed8d2189a4dbc936fe220698ef031954` |

## Remaining delivery gates

The [original native review](../independent-review.md) describes the unchanged private authority, fixed operations and lifecycle boundaries, with its original source pins retained. This supplement approves the changed candidate only within that same narrow scope. The run uses local SCRAM authentication, a throwaway CA and synthetic identities; it is not RDS IAM, S3/KMS or hosted-session evidence. The positive readiness helper is synthetic. Application writes and ordinary invitation/session/admission remain owned by later delivery gates.

Root must retain final binary/HTTP smoke, exact image inputs/build, current-source CI, separate repaired schema/launcher review and actual managed RDS isolation/reuse/race/recovery checks. No ticket status changes follow from this review alone.
