# APP-007 migration repair — independent review

**Verdict: no blocking findings** for the exact source below. This is a review of the non-superuser role-migration repair and its regression/CI integration, not APP-007 managed acceptance.

Reviewed independently of the migration author on 2026-10-28, against working-tree changes over `7e105bf607a7a70982b57fbf66621a13b7e58b1a`. The [machine-readable review](review.json) retains exact results, probe SQL, commands, source hashes and limitations. No implementation file was edited by this reviewer.

## Source identity

| Reviewed file | SHA-256 |
|---|---|
| `app/migrations/0001_role_foundation.sql` | `f98f67b7b516483692e5873741080df8ebf023dd9af9872f958258e98e3bd963` |
| `app/runtime/qualify_migration.py` | `fd9192cad2d0999d2da1ff0f0517b79464a95ef275984958f76263c93cb34a8e` |
| `.github/workflows/foundation.yml` | `102c247c78d9188bac0ecc9dfb1c34c714ecaea9ca2245221d96d239e2884c85` |
| `evidence/APP-007/migration-repair/README.md` | `dcde319a7b03b08a259c56e9428ec4dc401fcf75e17d0995b4c385290c91459b` |
| `evidence/APP-007/migration-repair/local-postgresql16.json` | `e4dc71c95797af2d3b0b6ecda4073990d64e0ebbb809dba91517ac3f80d5c5bd` |
| `evidence/APP-007/migration-repair/image-qualification.json` | `fd4f588ff6fc1675e2e71ec29eaf632e031614fbff3e89dbbae8084d35054c03` |

The application migration caller, Docker packaging and RDS engine/user configuration were also inspected; their exact hashes are in the JSON. Hosted CI for this uncommitted repair has not been observed. The workflow now invokes the pinned migration regression and retains its result.

## Evidence and assessment

- Re-ran the authored regression: **43 checks passed** on **PostgreSQL 16.13 (Debian 16.13-1.pgdg13+1)**, linux/arm64, official image `postgres@sha256:5d143123fdf80462d1778cd4f24b9f7ca13c87174bca19141fb194c5a1ebca59` (ARM64 manifest `sha256:bf738fa93668ee69ca247db4d98830fcc9b7e486257096adf793fa7836b5f076`). Both original `ALTER ROLE` failures reproduce with SQLSTATE `42501`; fresh and repeated migrations succeed as the non-superuser administrator. The 24 app-role denial operations and 13 role-drift cases pass.
- Ran an independently written SQL matrix: **48 checks passed**. All seven unsafe attribute variants were refused on each of the five foundation roles. Every role refused a privileged membership even with both SET and INHERIT disabled; indirect ancestry through `rds_iam` and owner membership in `rds_iam` were also refused. Each failed migration preserved the deliberately perturbed catalog exactly, and the final restored migration succeeded.
- The clean foundation gives each app role no schema CREATE or metadata-table TRUNCATE/TRIGGER/REFERENCES privileges. These additional four checks verify initial effective grants; they do not establish detection of existing object-ACL drift.
- Reviewed SQL lines 7–35 create restricted roles and reject unexpected attributes/memberships before later grants and metadata creation. The application caller executes the migration in one transaction; failure rollback and repeat behavior were reproduced. App identities cannot select owner/migration/sibling roles. No new app privilege was introduced by the repair.
- `git diff --check` passed. Both owned database containers were removed and a separate listing confirmed their absence. They used no network, no published port and disposable database storage.
- Read the embedded migration from immutable local app image `sha256:770bcb00395b135325ebb0955edc87a84c3c86b7ec05d0711af9a7765f03eb22` in an owned no-network, read-only container. Its hash matches the reviewed SQL exactly. The retained author image-qualification report was inspected; its broader image checks were not rerun by this reviewer.

Restricted role creation and membership defaults agree with [PostgreSQL 16 CREATE ROLE documentation](https://www.postgresql.org/docs/16/sql-createrole.html). AWS documents a non-superuser master role with role/database creation rights and the grant of `rds_iam` to application users. This supports compatibility as an inference; only the real managed run can establish RDS behavior. See [RDS master-role capabilities](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/Appendix.PostgreSQL.CommonDBATasks.Roles.rds_superuser.html) and [RDS IAM account setup](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/UsingWithRDS.IAMDBAuth.DBAccounts.html).

## Boundaries

No AWS operation or secret read was performed. The local `rds_iam` group is simulated, and the tests use Unix sockets with local trust authentication. Actual IAM/TLS access, managed grants/denials, tenant isolation and cleanup remain required.

The repair checks role attributes and memberships. It does not validate or revoke arbitrary existing object ACLs. Concurrent operator migrations or administrative role edits were not exercised; the migration is operator-only. The separate managed SQL trial is outside this review. Registry/deployment verification, ticket acceptance and the Phase 1 gate remain open.
