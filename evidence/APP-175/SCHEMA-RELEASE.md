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

After every pre-execution recheck, while beta still serves: a manual snapshot `forma-schema-release-<promotion id prefix>-v22` of the instance named by the `DatabaseEndpoint` output (at most 12 minutes to `available`). Then every final recheck again, the outage reserve check, the stop and the reviewed change set. Inside the outage, after the stopped boundary: one run of the stack's `MigrationTaskDefinition` on the candidate service image (`python3 /app/runtime.py migrate`; at most 12 minutes; exit 0 required). The migration runner applies 21 and 22 in one transaction and proves each version's catalog. Then the stopped boundary is proved again and the services activate. Success sets the baseline digests to the `to` values and the policy schema version to 22.

A snapshot failure leaves the phase `prepared` with services untouched. `maintenance abandon` then accepts exactly one confirmed `schema-snapshot` entry (bound to the packet's snapshot name, the live `DatabaseEndpoint` and an intact receipt), keeps the snapshot and makes no RDS call. An intent, a second entry, another instance or snapshot name, a packet without a release, or any `schema-migrate` entry refuses.

## Live facts (read-only, 2026-11-06, from the coordinator)

| Fact | Effect |
| --- | --- |
| The instance has `CopyTagsToSnapshot = true` | The execute role may tag only `forma-schema-release-*` snapshots (`rds:AddTagsToResource`); the requests name no tags. |
| `PreferredBackupWindow` 05:10–05:40 UTC | Do not approve between about 05:00 and 05:45 UTC; a snapshot refused there leaves beta serving and the release abandonable. |
| The live `provider_management_url` is `https://auth-staging.getforma.dev` | APP-084's provider-host pin in `infra/scripts/preflight.py` accepts the live configuration. |
| The execute job has `timeout-minutes: 60`; the role session is 3600 s | Execute carries one 55-minute budget (below). |
| The last normal execute took about 4 minutes | The 30-minute outage reserve and the 12-minute bounds leave wide margins. |

## Time budget

| Wait | Bound | Where |
| --- | --- | --- |
| Snapshot available | 720 s | before the outage |
| Stop settle | 900 s | outage |
| Stack update | 1200 s | outage |
| Settle after the change set | 900 s | outage |
| Migration stopped | 720 s | outage |
| Activation settle, then health | 900 s + 900 s | outage |

The normal bounds alone sum to 4800 s (80 minutes) and a schema release adds 1440 s, both beyond the job and the session. `runner.EXECUTE_BUDGET_SECONDS` (3300 s) therefore runs from the start of `Runner.execute`, and `Runner.bounded` clips every wait to what remains and refuses once it is spent, with the lock held. The snapshot and the migration start only if their whole 720 s remain. The outage starts only with `OUTAGE_RESERVE_SECONDS` (1800 s) left; in the worst case before it, the snapshot bound, that leaves 3300 − 720 = 2580 s. Each AWS wait can overrun its bound by one 10 s poll and one call, which the 5 minutes between 55 and 60 absorb. Tests simulate every wait using its whole bound: a schema release refuses before starting the migration it could not observe, a normal release refuses during its health wait after activating, and neither passes 3300 s.

## Checks

- `python3 -m unittest discover -s ops/promotion -p 'test_*.py'`: 161 tests, OK (125 before this ticket).
- `test_schema_release.py` (31 tests) covers: a missing file; 25 JSON edits that widen or reshape it (later expiry, other evidence, versions 19/21/23, booleans, floats and strings for versions, extra and missing keys, malformed digests, a `from` digest equal to its `to` digest) and eight malformed texts including duplicated keys and invalid UTF-8; each scope mismatch (baseline schema or contract, candidate schema or contract, a contract-only change, infrastructure, expiry, policy version, candidate bytes, candidate manifest version); the end-to-end lifecycle in fake AWS (packet, approval summary, exact `CreateDBSnapshot` and `RunTask` arguments, call order snapshot → stop → change set → migration → activate, bounded waits, receipts, baseline and policy admission); refusal before the lock for each mismatch; a release file changed, removed, expired or with changed infrastructure after preparation, including one restored before the final check and one expiring after the snapshot; a failed, unfinished or denied snapshot, each leaving beta serving in phase `prepared`, then abandoned with the snapshot kept (the denied one stays an intent and refuses abandonment); a failed, unfinished, wrong-image or unstartable migration, each leaving both services stopped in phase `executing` with no activation and no replay; lost responses for both mutations; the time budget (worst case, too little time for the snapshot, for the outage and for the migration, late waits clipped); a normal promotion with the file valid, malformed or absent; and the exact approval-summary projection.
- `test_maintenance.py` adds 4 tests: a confirmed pre-outage snapshot is kept and abandonment succeeds with no RDS call; an intent, a duplicate, another instance or snapshot (with a consistent receipt), a packet without a release, a journaled migration or the `preparing` phase refuses; a changed receipt refuses; `release-rolled-back` refuses once `schema-migrate` is journaled and still releases without it.
- Two mutation passes removed each new or moved guard in turn. The first covered infrastructure, expiry, policy version, candidate version, `from` ≠ `to`, exit code, stop code, image digest, snapshot status, start failures, pinned expiry, both execute rechecks, the migration call, the policy update, the scan-finding refusal, the summary contract and `preserved` checks, the stopped boundary after the migration, the `ecs:cluster` condition, the promotion-identity type check and the migration task-definition binding. The second covered the pre-outage snapshot call, the recheck after it, the outage reserve, both observe-before-start checks, the confirmed-snapshot requirement, the budget clip, its exhaustion check and each clipped wait, the snapshot identity, both bounds, the abandonment binding, receipt, shape and use, the rolled-back refusal, the tagging scope and the summary sequence. All 44 made a test fail. A `schema-migrate` prefix added to abandonment's existing execution refusal is deliberately redundant with the snapshot-shape check.
- `test_bootstrap.py` asserts the five added execute-role statements exactly, that tagging reaches only the release snapshots, that no other role gains them, and that the role gains no other tagging, stop, delete or restore action.

## Limits

- The ticket's earlier read-only preflight is not part of this design. If two organizations share an active `(issuer, provider_tenant_id)` binding, 0021's primary key fails the migration inside the outage: the transaction leaves schema 20, but beta stays down until reconciled. The runner itself proves the recorded migration history 1–20 before applying. A read-only uniqueness check before merging avoids that outage.
- No AWS write was made. The canonical state was not read by this change: that the live baseline holds the `from` digests and `policy.schema_version` is 20 rests on the November 4 schema 20 admission and the APP-173 record. If either differs, preparation refuses before the lock; read them with `maintenance inspect` before enabling promotion.
- The AWS service reference (v1.5, read 2026-11-06) lists both the `db` and `snapshot` resource types for `rds:CreateDBSnapshot` and `rds:DescribeDBSnapshots` without saying which are required, so both statements name both. Whether copied tags need `rds:AddTagsToResource` was not established; the role now allows it on the release snapshots. Confirm by policy simulation after applying the bootstrap update.
- Writes between the snapshot and the stop (the final rechecks, about a minute) are not in the snapshot; the seven-day automated backups cover them.
- Snapshot storage is outside the budget ledger, and the snapshot is retained until someone reviews its deletion.
- `schema-release.json` must be removed by a follow-up pull request after the release succeeds or expires; until then CI fails any change to the deployment contract.
- Independent review of this change has not run yet.
