# Schema release 20 → 22

Reviewed scope: the first beta promotion whose candidate carries migrations 0021 (APP-084, PR #83) and 0022 (APP-169, PR #84), from the live release at `05db000` (PR #88's merge, schema 20). The release is [`ops/promotion/schema-release.json`](../../ops/promotion/schema-release.json); this record, versions 20 and 22 and the expiry **2026-11-20 00:00 UTC** are also pinned in `ops/promotion/schema_release.py`, so editing the JSON alone cannot widen it. The procedure is [Schema releases](../../docs/pitch/BETA-PROMOTION.md#schema-releases); the decision is the [ADR-017 amendment](../../docs/architecture/ADR-017-BETA-PROMOTION.md#amendment-2026-11-06-reviewed-schema-releases).

Selected by Victor, 2026-11-05: finish PRs #83 and #84, then ship both migrations in one schema release; teach the pipeline a reviewed schema-release mode so the migrations ship with the one approval on the CI-qualified images.

## Digests

Computed on branch `app175/schema-release` at `1bf6dd9` (PR #83 and #84 stacked), 2026-11-06, with this repository's own functions. Row format for the tree digests: one `path NUL sha256(blob) LF` row per committed path, sorted, hashed with SHA-256.

| Field | Value | How |
| --- | --- | --- |
| `from.schema_manifest_sha256` | `08737320c891830a2a8c6deaec9111ca18afb9ae7927c172b44379506ac4e2cb` | SHA-256 of `git show 05db000:app/migrations/manifest.json`; its `database_version` is 20 |
| `from.deployment_contract_sha256` | `4a2cea855e962fda7f79a55f2b2754df9a056e4b3bab3fc4e5b4ee1a98fdd2a0` | `candidate.deployment_contract_digest(repo, '05db000…')`; the same value the [APP-173 disposition](../APP-173/SCAN-DISPOSITION.md) bound as the live baseline |
| `to.schema_manifest_sha256` | `7ceb859d1794e79224ed609bfb557c21e79c23020894540b7b5114044ec68176` | SHA-256 of `git show HEAD:app/migrations/manifest.json`; its `database_version` is 22 |
| `to.deployment_contract_sha256` | `14a161a09cf93044cbf6d1281b02b56f954fa7f73ae3a17a8f54b07b7958d99b` | `candidate.deployment_contract_digest(repo, 'HEAD')` |
| `infrastructure_sha256` | `e74b620e4d9749b5eb47819a1546be1adc00765008f042abbb0b747d9e549da1` | `candidate.infrastructure_digest(repo, rev)` over `infra/src/*`, `infra/package.json`, `infra/package-lock.json`, `infra/cdk.json`, `infra/tsconfig.json`: the same value at `05db000` and at `HEAD` |
| file bytes | `17ef4f098b521078f95e7e25e26628d40ae9293ee48e551edf679338f57cb59e` | SHA-256 of the committed `schema-release.json`; the approval shows this as `spec_sha256` |

`git diff --stat 05db000 HEAD` over the deployment contract's paths changes only: `.dockerignore`, `app/operator/Dockerfile.dockerignore` and `app/qualification/Dockerfile.dockerignore` (admitting the new migration, catalog and `artifact_kind.rs` files), `app/migrations/0021_operator_audit_kinds.sql`, `0022_work_artifact_kinds.sql`, `catalog-v21.json`, `catalog-v22.json`, `manifest.json`, and `app/runtime/runtime.py` (`DATABASE_SCHEMA_VERSION` 20 → 22 and the `PGHOST` check in `migrate`). Nothing under `infra/src` or the four `infra/` project files changed. Outside the contract, `infra/scripts/preflight.py` gained APP-084's provider-host check, which the runner's reservation uses.

`ops/promotion/test_schema_release.py` recomputes the `to` digests and the infrastructure digest at `HEAD` on every run, so the planning and promotion CI fail if the tree moves away from this release. The `from` side cannot be checked against Git in a shallow CI checkout; at runtime it must equal the canonical baseline.

## What runs

After the reviewed change set and the stopped boundary: a manual snapshot `forma-schema-release-<promotion id prefix>-v22` of the instance named by the `DatabaseEndpoint` output (at most 20 minutes to `available`), then one run of the stack's `MigrationTaskDefinition` on the candidate service image (`python3 /app/runtime.py migrate`; at most 30 minutes; exit 0 required). The migration runner applies 21 and 22 in one transaction and proves each version's catalog. Then the stopped boundary is proved again and the services activate. Success sets the baseline digests to the `to` values and the policy schema version to 22.

## Checks

- `python3 -m unittest discover -s ops/promotion -p 'test_*.py'`: 150 tests, OK (125 before this change).
- The 24 new tests in `test_schema_release.py` cover: a missing file; 25 JSON edits that widen or reshape it (later expiry, other evidence, versions 19/21/23, booleans, floats and strings for versions, extra and missing keys, malformed digests, a `from` digest equal to its `to` digest) and eight malformed texts including duplicated keys and invalid UTF-8; each scope mismatch (baseline schema or contract, candidate schema or contract, a contract-only change, infrastructure, expiry, policy version, candidate bytes, candidate manifest version); the end-to-end lifecycle in fake AWS (packet, approval summary, exact `CreateDBSnapshot` and `RunTask` arguments, call order stop → change set → snapshot → migration → activate, bounded waits, receipts, baseline and policy admission); refusal before the lock for each mismatch; a release file changed, removed, expired or with changed infrastructure after preparation, including one restored before the final check; a failed, unfinished, wrong-image or unstartable migration and a failed or unfinished snapshot, each leaving both services stopped in phase `executing` with no activation and no replay; lost responses for both mutations; a normal promotion with the file valid, malformed or absent; and the exact approval-summary projection.
- A mutation pass removed each new guard in turn (infrastructure, expiry, policy version, candidate version, `from` ≠ `to`, exit code, stop code, image digest, snapshot status, start failures, pinned expiry, both execute rechecks, the migration call, the policy update, the scan-finding refusal, the summary contract and `preserved` checks, the stopped boundary after the migration, the `ecs:cluster` condition, the promotion-identity type check and the migration task-definition binding): all 22 made a test fail.
- `test_bootstrap.py` asserts the four added execute-role statements exactly, that no other role gains them, and that the role gains no tagging, stop, delete or restore action.

## Limits

- The ticket's earlier read-only preflight is not part of this design. If two organizations share an active `(issuer, provider_tenant_id)` binding, 0021's primary key fails the migration inside the outage: the transaction leaves schema 20, but beta stays down until reconciled. The runner itself proves the recorded migration history 1–20 before applying. Checking the binding uniqueness read-only before merging avoids that outage.
- No AWS call was made. The canonical state was not read: that the live baseline holds the `from` digests and `policy.schema_version` is 20 rests on the November 4 schema 20 admission and the APP-173 record. If either differs, preparation refuses before the lock; read them with `maintenance inspect` before enabling promotion.
- The AWS service reference (v1.5, read 2026-11-06) lists both the `db` and `snapshot` resource types for `rds:CreateDBSnapshot` and `rds:DescribeDBSnapshots` without saying which are required, so both statements name both. It does not list dependent actions. The instance copies its tags to snapshots (CDK default); whether that needs `rds:AddTagsToResource` for the caller was not established. Confirm both by policy simulation after applying the bootstrap update.
- `CreateDBSnapshot` needs the instance `available`; during the automated backup window it may refuse, after the services have stopped. Merge outside that window.
- The bounded waits (stack 20 minutes, snapshot 20, migration 30, activation up to 15 per settle) can together exceed the execute job's 60 minutes and its one-hour role session. A timeout or expired session leaves the lock held for reconciliation, as for any execution.
- Snapshot storage is outside the budget ledger, and the snapshot is retained until someone reviews its deletion.
- Independent review of this change has not run yet.
