# Schema release 20 → 22

Reviewed scope: the first beta promotion whose candidate carries migrations 0021 (APP-084, PR #83) and 0022 (APP-169, PR #84), from the live release at `05db000` (PR #88's merge, schema 20). The release is [`ops/promotion/schema-release.json`](https://github.com/getforma-dev/forma-app-platform/blob/511d97e35733572e8571715fd881f0b25692e7e2/ops/promotion/schema-release.json); every value in it (this record, versions 20 and 22, the four digests, the infrastructure digest and the expiry **2026-11-20 00:00 UTC**) is also pinned in `ops/promotion/schema_release.py`, so editing the JSON alone can neither widen nor retarget it. The procedure is [Schema releases](../../docs/pitch/BETA-PROMOTION.md#schema-releases); the decision is the [ADR-017 amendment](../../docs/architecture/ADR-017-BETA-PROMOTION.md#amendment-2026-11-06-reviewed-schema-releases).

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

After every pre-execution recheck, while beta still serves: a manual snapshot `forma-schema-release-<promotion id prefix>-v22` of the instance named by the `DatabaseEndpoint` output (at most 12 minutes to `available`). Then every final recheck again, the outage reserve check, the stop and the reviewed change set. Inside the outage, after the stopped boundary: one run of the stack's `MigrationTaskDefinition` on the candidate service image (`python3 /app/runtime.py migrate`; at most 12 minutes; exit 0 required). The migration runner applies 21 and 22 in one transaction and proves each version's catalog. Then the stopped boundary is proved again and the services activate. Success sets the baseline digests to the `to` values and the policy schema version to 22. Preparation and both execution rechecks also require the checkout to be the candidate commit (`git rev-parse HEAD`) before reading its manifest or infrastructure.

A snapshot failure leaves the phase `prepared` with services untouched. `maintenance abandon` then accepts exactly one `schema-snapshot` entry, bound to the packet's snapshot name and the live `DatabaseEndpoint`. A confirmed entry needs its intact receipt. An intent (a refused, denied or unanswered call) is read back once with DescribeDBSnapshots on the deterministic name; the observation (absent, or present with its status) goes into the journal entry (status `read-back`) and an immutable `schema-snapshot-readback.json` before the release is abandoned. Any snapshot is kept; no other RDS call is made. A second entry, another instance or snapshot name, a packet without a release, a malformed intent, an unavailable or ambiguous readback, or any `schema-migrate` or `stop-` entry refuses.

A failed or unstarted migration leaves beta down in phase `executing`. `maintenance restore-after-failed-migration` (operator identity only; reviewed state hash; 65-minute fence; read-only plan, then `--apply`; immutable receipts; no replay) puts the previous release back. Its preconditions and steps are in [Schema releases](../../docs/pitch/BETA-PROMOTION.md#schema-releases). It accepts no migration, or one observed STOPPED with a known non-zero exit code. It refuses exit 0, an unknown exit code, a missing observation, a start with failures and an unresolved start.

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

The normal bounds alone sum to 4800 s (80 minutes) and a schema release adds 1440 s, both beyond the role session. `runner.EXECUTE_BUDGET_SECONDS` (3300 s) bounds the runner step from the start of `Runner.execute` through the health wait; `Runner.bounded` clips every wait to what remains and refuses once it is spent, with the lock held. The snapshot and the migration start only if their whole 720 s remain. The outage starts only with `OUTAGE_RESERVE_SECONDS` (1800 s) left; before it, the worst case is the snapshot bound, which leaves 3300 − 720 = 2580 s. The reserve covers the typical outage (about 4 minutes for the last normal execute) plus the migration bound, not the worst case: a slow successful stack update can leave too little time to observe the migration, which then never starts (tested), leaving beta down at schema 20 and recoverable with the restore command.

Outside the budget: before it, the job's checkout, artifact verification, venv, CI recheck and OIDC, so the execute job's `timeout-minutes` is now 75; after the health wait, one boundary read (about 17 AWS calls), six public probes and two S3 writes. Those take seconds in practice. Their bounds are 65 s per AWS call (5 s connect, 60 s read, one attempt) and 15 s per probe, roughly 22 minutes if every call hung. Such a hang would outlast the role session (3600 s from just before the step, 5 minutes beyond the budget) with the services already activated and the lock held. Each AWS wait can also overrun its bound by one 10 s poll and one call. Tests simulate every wait using its whole bound: a schema release refuses before starting a migration it could not observe, a normal release refuses during its health wait after activating, and neither passes 3300 s.

## Checks

- `python3 -m unittest discover -s ops/promotion -p 'test_*.py'`: 173 tests, OK (125 before this ticket).
- `test_schema_release.py` (32 tests) covers the file (missing; 30 edits that widen, reshape or retarget it, including each pinned digest; eight malformed texts), each scope mismatch including a checkout other than the candidate, the end-to-end lifecycle in fake AWS (snapshot → stop → change set → migration → activate, exact requests, receipts, admission), refusal before the lock, release changes after preparation, snapshot failures that leave beta serving and abandonable (a denied call read back as absent, a lost response read back as present), migration failures that leave beta down without replay, and the time budget.
- `test_schema_recovery.py` (7 tests): the restore after a migration that exited 1 and after one that never started (exact mutations, the restricted role, previous parameters, ledger abandonment, unchanged baseline and policy, receipts, no RDS call); refusal with no write of exit 0, an unknown exit code, a missing observation, a start with failures and an unresolved start; 19 precondition refusals, each with its own reason (including previous parameters that are not runtime-only and an observation naming another task or not stopped); a restore change set outside the runtime boundary that never executes; a previous release that never becomes healthy (lock kept, outcome recorded, no replay); a failed restore update and a lost response (services stay stopped); the CLI's plan default and never-overwritten output.
- `test_maintenance.py` adds 8 tests for the pre-outage snapshot in abandonment: confirmed and kept; intent read back once and kept; a snapshot intent beside another unknown outcome refuses; an unavailable or ambiguous readback keeps the lock; a recorded readback reused by a retried abandonment, refused when its receipt or recorded observation changes; malformed, duplicate, foreign, unbound, stopped or migrated journals refuse; a changed receipt refuses; `release-rolled-back` refuses once `schema-migrate` is journaled.
- `test_bootstrap.py` asserts the five added execute-role statements exactly.
- Mutation passes removed each new or moved guard in turn: 44 for the first two rounds and 26 for the review fixes (the source pin, the candidate checkout, the snapshot readback and its receipt, the restore's journal, migration outcome, fence, identity, boundary, template, parameters, expiry, phase, change-set, stack-status, health and ledger checks, and the restore registry). Every one made a test fail.

## Limits

- No pre-approval tenant check (accepted). If two organizations share an active `(issuer, provider_tenant_id)` binding, 0021's primary key fails the migration inside the outage: the transaction leaves schema 20, beta stays down, and `restore-after-failed-migration` brings the previous release back. No role in this design can query the database before the outage. Evidence for the upgrade itself is the schema campaign on these migrations: it upgrades a version-20 database seeded with organizations and active and inactive bindings to version 22, and refuses whole, leaving version 20 and every byte unchanged, for a shared binding ([report](../APP-169/schema-campaign-review-fixes-2026-11-05.json), checks "0021 upgrade of a populated version-20 database…" and "0021 refuses whole…"). It does not read beta's data.
- `ecs:RunTask` cannot be restricted against container overrides by IAM, so the execute role could start the migration task definition with another command; the trust rests on the reviewed trunk code that alone assumes that role.
- No AWS write was made. The canonical state was not read by this change: that the live baseline holds the `from` digests and `policy.schema_version` is 20 rests on the November 4 schema 20 admission and the APP-173 record. If either differs, preparation refuses before the lock; read them with `maintenance inspect` before enabling promotion.
- The AWS service reference (v1.5, read 2026-11-06) lists both the `db` and `snapshot` resource types for `rds:CreateDBSnapshot` and `rds:DescribeDBSnapshots` without saying which are required, so both statements name both. Whether copied tags need `rds:AddTagsToResource` was not established; the role allows it on the release snapshots. Confirm by policy simulation after applying the bootstrap update.
- Beta accepts writes after the snapshot until both services' tasks have drained after the stop call (the final rechecks plus the drain); the seven-day automated backups cover that interval, the snapshot does not.
- The restore command restores images and template, never data. A migration task that never started or whose start failed without an observed exit code still needs manual reconciliation.
- Snapshot storage is outside the budget ledger, and the snapshot is retained until someone reviews its deletion.
- `schema-release.json` and its source pin must be removed by a follow-up pull request after the release succeeds or expires; until then CI fails any change to the deployment contract.
- The Tier A review of `0aac563..52c288d` found no code blocker and requested these changes; they have not been re-reviewed.
