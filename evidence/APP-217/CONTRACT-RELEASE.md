# Contract-only release at schema 24 (release D: themed error pages)

Reviewed scope: a beta promotion that changes the deployment contract without a migration, the second of its kind after release C ([record](../APP-209/CONTRACT-RELEASE.md)). Release D ships APP-217, the themed error pages for browser navigations (PR #143). It starts from the security release, live since 2026-11-15 (candidate `0f8546bf`, schema 24, tag `beta/20261115-0f8546b`; [record](../APP-215/release-2026-11-15.md)). The schema manifest, its version 24 and the CDK source are unchanged. The contract paths that change are the two image allowlists, which must admit the new page's sources. The release ships as the reviewed contract-only release that [APP-210](../../Tickets/0-backlog/APP-210.md) admits ([trial runbook section 3a](../../docs/operations/TRIAL-RUNBOOK.md#3a-release-c-a-contract-only-release-schema-24--24)). The release is `ops/promotion/schema-release.json`. Every value in it is also pinned in `ops/promotion/schema_release.py` (`REVIEWED_SCHEMA_RELEASE`), so a JSON edit can neither widen nor retarget it: this record, version 24 at both ends, one schema manifest digest, the two contract digests, the infrastructure digest and the expiry **2026-11-27 00:00 UTC**.

## The record

| Field | Value | How |
| --- | --- | --- |
| `from.database_version` | 24 | the canonical `policy.schema_version`, unchanged since release B |
| `from.schema_manifest_sha256` | `176d68cbc363a822a32b747b3a52f6a45809a946bed2649e93bd65f7d84208f1` | SHA-256 of `app/migrations/manifest.json`, the same bytes since release B |
| `from.deployment_contract_sha256` | `3493b0a68b08b686a245be7ffa602817772895c0f5cc8983c06ad78b7a3e2dd9` | `candidate.deployment_contract_digest(repo, '0f8546bf…')`. The canonical state's `live.candidate_sha` read `0f8546bffca2368eae827addf548a23273543afb` on 2026-11-15. It is release C's `to`; the security release did not change the contract. |
| `to.database_version` | 24 | the candidate manifest's `database_version`, unchanged |
| `to.schema_manifest_sha256` | `176d68cbc363a822a32b747b3a52f6a45809a946bed2649e93bd65f7d84208f1` | the same bytes as `from` |
| `to.deployment_contract_sha256` | `8578d6ac6fc95d40cda7055a3724b194a0a3314c7344bd295b8aeb2c324b6c48` | `candidate.deployment_contract_digest(repo, 'HEAD')` on `feat/app-217-error-pages`; the review-fix commits did not move it |
| `infrastructure_sha256` | `e74b620e4d9749b5eb47819a1546be1adc00765008f042abbb0b747d9e549da1` | `candidate.infrastructure_digest(repo, rev)`, the same at `0f8546bf` and at `HEAD`; the stack is unchanged |
| `expires_at_utc` | `2026-11-27T00:00:00Z` | below |
| `evidence` | `evidence/APP-217/CONTRACT-RELEASE.md` | this file |
| file bytes | `86a62316840d6c3778ce053ff4e0faa593293a493a037eb240b7d06eed70f141` | SHA-256 of the committed `schema-release.json` (`json.dumps(indent=2, sort_keys=True)` plus a newline). The approval summary shows it as `spec_sha256`. |

The commits after the record on this branch touch no deployment-contract path. `CommittedReleaseTests` recompute the `to` digest at `HEAD` on every run.

## The migrations, CDK and runtime diff is empty

`git diff --stat 0f8546bf HEAD -- app/migrations infra/src app/runtime/runtime.py` prints nothing, and no Dockerfile changed.

## The two contract files that changed

- Root `.dockerignore`, the service and operator image context: it admits exactly the new page's sources, which are `app/src/http/errors.rs`, `app/admin/src/error/app.tsx`, `app/admin/src/styles/error.css` and `app/admin/src/brand/forma-isotope.svg` (the brand directory pattern mirrors the others).
- `app/qualification/Dockerfile.dockerignore`: it admits `app/src/http/errors.rs`, which the test library compiles. The source bundle in `app/src/storage/managed_qualification.rs` grows from 236 to 237 by that file alone.

The independent review checked both allowlists (PR #143, review of 2026-11-15).

## What the approval must show

Trial runbook section 3a step 2. The approver compares the preparation summary with this record:

- `schema_release: {from_version: 24, to_version: 24, spec_sha256: 86a62316840d6c37…}`;
- `schema_version: 24`;
- `preserved`: `schema`, `template`, `trial`, `aliases` and `secrets` all `true`;
- `sequence`: exactly `stop both services`, `execute reviewed change set`, `verify stopped boundaries`, `activate both services`, `verify health and images`, with no `snapshot database` and no `run reviewed migration`;
- `deployment_contract_sha256: 8578d6ac6fc95d40…`;
- the stop time: 18 hours before the live 2026-12-14T02:00:00Z, so **2026-12-13T08:00:00Z**, and the scan as for any release. If preparation refuses on newly listed findings, follow runbook section 8 with `--contract 8578d6ac6fc95d40cda7055a3724b194a0a3314c7344bd295b8aeb2c324b6c48` and `--expires-at-utc 2026-11-27T00:00:00Z`.

## What runs

A normal promotion, as release C: the outage reserve, stop both services, execute the reviewed change set, prove the stopped boundary, activate, and verify health and images. No RDS snapshot, no migration task. The 05:00–05:45 UTC snapshot window does not apply, but the approval still avoids it by habit. On success the baseline takes contract `8578d6ac…`; its schema manifest digest and `policy.schema_version` 24 stay.

## Expiry

**2026-11-27 00:00 UTC**: after this release (planned 2026-11-15), and before the codes (Nov 28) and the class (Nov 29–30), so no contract change can promote in the class weekend even if the follow-up that removes the file is late. The deployment's stop time is separate. It moves from 2026-12-14T02:00Z to **2026-12-13T08:00Z** with this release, and the purge must finish before it.

## Limits

- The `to` contract is the PR head's. Any later change to a contract path fails `CommittedReleaseTests` and needs this record and the pin recomputed.
- Scanner findings at preparation are unknown until the images are built and scanned.
- `schema-release.json` and its pin are removed by the close-out after the release succeeds or expires.
