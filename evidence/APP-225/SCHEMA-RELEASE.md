# Schema release 24 → 25 (release F: organization roles)

Reviewed scope: the organization roles of [ADR-021](../../docs/architecture/ADR-021-ORGANIZATION-ROLES.md), tickets [APP-225](../../Tickets/0-backlog/APP-225.md) and [APP-226](../../Tickets/0-backlog/APP-226.md), PR #153. Migration `0025_organization_roles.sql` adds `memberships.role` (owner, admin, manager, member or viewer), maps the existing `admin_policy` holders (the earliest-admitted becomes owner, the rest admin), drops `organization_permissions`, adds `access_codes.role`, and adds the definers `set_member_role`, `admin_remove_member` (replaced), `org_admin_overview` (recreated) and `steward_card` (replaced). It also adds RESTRICTIVE viewer policies on 37 write tables and the audit kind `membership.role_changed`. It starts from release E, live since 2026-11-15 ~18:20 UTC (candidate `4360a404`, tag `beta/20261115-4360a40`; [record](../APP-223/release-2026-11-15.md)).

**Reviews:**
- An independent Fable Tier A review: GO-after-fixes (one MAJOR, a last-owner race, fixed with a per-organization advisory lock and a concurrency check);
- a Fable recheck: GO;
- the CI-only suites (pitch, fixtures, tenant foundation) fixed and green.

## The record

| Field | Value |
| --- | --- |
| `from.database_version` | 24 |
| `from.schema_manifest_sha256` | `176d68cbc363a822a32b747b3a52f6a45809a946bed2649e93bd65f7d84208f1` (release E's, unchanged since release B) |
| `from.deployment_contract_sha256` | `79263fbbdbf4c59ced39902f76b2a352f443c3a35dfad6a926b7e23912f081b1` (release E's `to`, the live baseline) |
| `to.database_version` | 25 |
| `to.schema_manifest_sha256` | `8e1e43b57e1bb41e4f3e5e4dfe54ef49bda51e7b7dcedb8172b74c7676d0eb9e` |
| `to.deployment_contract_sha256` | `bd3c6437682bcc295260b476e069fd6c49f21956ae383ea5acbe539df2c38d44` |
| `infrastructure_sha256` | `e74b620e4d9749b5eb47819a1546be1adc00765008f042abbb0b747d9e549da1` (unchanged) |
| `expires_at_utc` | `2026-11-27T00:00:00Z` (before the codes and the class) |
| file bytes | `02a3353bd1d178927752a96f2b2061b13c28c648802aa884a6455a133d66fe7b`, the approval summary's `spec_sha256` |

Computed at `4203bb0f` (main after PR #153) with `ops.promotion.candidate`. The contract paths that changed since `4360a404`:

- `.dockerignore`
- `app/migrations/0025_organization_roles.sql`
- `app/migrations/catalog-v25.json`
- `app/migrations/manifest.json`
- `app/operator/Dockerfile.dockerignore`
- `app/qualification/Dockerfile.dockerignore`
- `app/runtime/runtime.py`

`infra/src` is unchanged.

## Before approving

- **The execute role** was read back on 2026-11-15 by `iam simulate-principal-policy` as `forma-beta-promotion-execute`, with the condition keys the policy uses. Allowed:
  - `rds:CreateDBSnapshot`, `rds:DescribeDBSnapshots` and `rds:AddTagsToResource` on `forma-schema-release-*-v25`;
  - `rds:CreateDBSnapshot` on the instance;
  - `ecs:RunTask` on the migration task (with `ecs:cluster`);
  - `iam:PassRole` for its two roles (with `iam:PassedToService=ecs-tasks.amazonaws.com`).

  The infrastructure is unchanged, so the policy is the same as for release B.
- **Approve outside 05:00–05:45 UTC**, because the pre-release snapshot refuses inside the RDS backup window.
- **The data mapping on beta.** The class organization's only `admin_policy` holder is Victor's Nov 1 account (granted 2026-11-15), so it becomes **owner**. The demo organizations have no `admin_policy` holder and get no owner, which is allowed for operator-managed organizations.

## What the approval must show

- `schema_release: {from_version: 24, to_version: 25, spec_sha256: 02a3353bd1d17892…}` and `schema_version: 24` (the live policy version at preparation, as release B showed 22; corrected after the packet review);
- the sequence: snapshot database, stop both services, execute reviewed change set, verify stopped boundaries, run reviewed migration, activate both services, verify health and images (the change set comes first, so the migration task runs on its replaced definition; corrected after the packet review);
- `deployment_contract_sha256: bd3c6437682bcc29…`;
- the stop: 18 hours before the live 2026-12-12T14:00:00Z, so **2026-12-11T20:00:00Z** (no expiry renewal applied; see Limits).

## What runs

As release B: the snapshot `forma-schema-release-<promotion id prefix>-v25`, the outage, the migration task (`runtime.py migrate`: exit 0 committed, 1 nothing committed, 4 unknown), the change set, activation, and health with the images. On success the baseline takes the `to` digests and `policy.schema_version` 25.

## Rollback

The database is never rolled back by the pipeline; the snapshot protects the data.
- **Exit 1** (nothing committed) or a migration that never started: `maintenance restore-after-failed-migration`, after the 65-minute fence.
- **Exit 4**: refused by the restore; investigate.
- **After success**: fix forward.

## After the release

- `/health/ready` 200; it requires schema 25 on every connection.
- `admission list` shows Victor's account as `owner`.
- `/admin` shows the Role column.
- The canonical `policy.schema_version` is 25.
- Victor's new build account (when issued) is promoted with `admission set-role --role owner|admin`.

## Limits

- **The operator's `revoke-membership` can remove the last owner** (review recheck MINOR-6), and it does so deliberately. The end-of-class purge (runbook §10 step 2) removes the operator's own, last membership through it. It is a trusted, audited, operator-only channel; in-product paths keep the last-owner rule.
- **The stop moves to 2026-12-11T20:00:00Z** with this release. Victor decided on 2026-11-15 to keep the beta running to **Jan 15**, and to keep students' accounts by default, with a notice v2 before the Nov 28 codes. The renewal (a reviewed change to `RENEWED_EXPIRY`, a January ledger month and the caps, then the operator apply) rides the next release, which sets the stop from the renewed date.
- `schema-release.json` and its pin are removed by the close-out after the release.
