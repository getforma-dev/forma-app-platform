# Contract-only release at schema 24 (release C)

Reviewed scope: the first beta promotion that changes the deployment contract without a migration. Release C ships the participation primitives (APP-202–APP-208 and APP-211) from release B, live since 2026-11-14 18:15 UTC (candidate `140a5a9f`, schema 24; [record](../APP-194/release-2026-11-14.md)). The schema manifest, its version 24 and the CDK source are release B's. The one contract path that changes is the qualification image context, `app/qualification/Dockerfile.dockerignore`. The release ships as the reviewed contract-only release that [APP-210](../../Tickets/0-backlog/APP-210.md) admits ([BETA-PROMOTION "Contract-only releases"](../../docs/pitch/BETA-PROMOTION.md#schema-releases); [trial runbook section 3a](../../docs/operations/TRIAL-RUNBOOK.md#3a-release-c-a-contract-only-release-schema-24--24)). The release is `ops/promotion/schema-release.json`. Every value in it is also pinned in `ops/promotion/schema_release.py` (`REVIEWED_SCHEMA_RELEASE`), so a JSON edit can neither widen nor retarget it: this record, version 24 at both ends, one schema manifest digest, the two contract digests, the infrastructure digest and the expiry **2026-11-27 00:00 UTC**.

## The record

| Field | Value | How |
| --- | --- | --- |
| `from.database_version` | 24 | the canonical `policy.schema_version` after release B ([record](../APP-194/release-2026-11-14.md), "After") |
| `from.schema_manifest_sha256` | `176d68cbc363a822a32b747b3a52f6a45809a946bed2649e93bd65f7d84208f1` | SHA-256 of `git show 140a5a9f:app/migrations/manifest.json`; its `database_version` is 24. It is release B's `to` and the live canonical baseline. |
| `from.deployment_contract_sha256` | `88d17cb53bf77c2f2bfa0b4345953f9f74c0aa39b81b677d6105807a54bcc356` | `candidate.deployment_contract_digest(repo, '140a5a9f…')`: release B's `to` and the live canonical baseline (`live.candidate_sha` is `140a5a9f`) |
| `to.database_version` | 24 | the candidate manifest's `database_version`, unchanged |
| `to.schema_manifest_sha256` | `176d68cbc363a822a32b747b3a52f6a45809a946bed2649e93bd65f7d84208f1` | SHA-256 of `git show HEAD:app/migrations/manifest.json`: the same bytes as `from` |
| `to.deployment_contract_sha256` | `3493b0a68b08b686a245be7ffa602817772895c0f5cc8983c06ad78b7a3e2dd9` | `candidate.deployment_contract_digest(repo, 'HEAD')` at `b7bfcef5` |
| `infrastructure_sha256` | `e74b620e4d9749b5eb47819a1546be1adc00765008f042abbb0b747d9e549da1` | `candidate.infrastructure_digest(repo, rev)` over `infra/src/*` and the four `infra/` project files. It is the same at `140a5a9f` and at `HEAD`, and is release A's and release B's value, so the stack is unchanged. |
| `expires_at_utc` | `2026-11-27T00:00:00Z` | below |
| `evidence` | `evidence/APP-209/CONTRACT-RELEASE.md` | this file |
| file bytes | `561148629cf2f218ba725d809d8bba8910b62a0775ee58c511c078f9bb45e539` | SHA-256 of the committed `schema-release.json` (`json.dumps(indent=2, sort_keys=True)` plus a newline, release A's form). The approval summary shows it as `spec_sha256`. |

Computed at `b7bfcef5`, the merge of `origin/main` `692ce826` (release B's close-out and APP-210) into `release/c-participation` `3bd4319b`, with this repository's own functions (`ops.promotion.candidate`). The tree digests use one `path NUL sha256(blob) LF` row per committed path, sorted, hashed with SHA-256. The commits after `b7bfcef5` on this branch (this record, the file, the pin and ticket bookkeeping) touch no deployment-contract path, so the `to` digest holds; `CommittedReleaseTests` recompute it at `HEAD` on every run. PR #130 (APP-212) touches no contract path either, so merging `main` again after it would not move the digest.

`schema_release.load` admits exactly this shape: the contract differs, the version does not fall, and the schema manifest digest is equal because the version is equal. `applicable` then requires the live baseline to equal `from`, the policy version to be 24, the candidate's committed manifest to hash to `176d68cb…` with `database_version` 24, the checkout to be the candidate commit, the infrastructure digest `e74b620e…`, and the clock before the expiry.

## The migrations diff is empty

`git diff --stat 140a5a9f b7bfcef5 -- app/migrations` prints nothing: no file under `app/migrations/` differs between the live candidate and release C, and the manifest's bytes are identical (one SHA-256 at both ends). The promotion pins the manifest's digest and the CDK source, not the files under `app/migrations/`; this diff is the check that the trial runbook (section 3a step 1) and BETA-PROMOTION require of the release's own review (APP-210 review, NIT-3).

## The one contract file that changed

`git diff --stat 140a5a9f b7bfcef5` over the deployment contract's paths (`infra/src/*`, `app/migrations/*`, the four `infra/` project files, `app/runtime/runtime.py`, the three Dockerfiles, `.dockerignore` and the two `Dockerfile.dockerignore` files) changes exactly one file: `app/qualification/Dockerfile.dockerignore`, 15 insertions and no deletion. It is the qualification image's explicit allowlist: BuildKit reads it instead of the root `.dockerignore`, so the service and operator image contexts are unchanged. The qualification image compiles the `cfg(test)` library, so every source and fixture its tests embed must be admitted, or the image build fails. Two Release C commits added the lines:

- `57747132` (B0, APP-202, the contract edition impl-2026-11-14.2), 14 lines: `contracts/fixtures/participation.json`, the new edition's participation vectors, which `app/src/admission/tests.rs` and `app/src/composer/prompt.rs` embed; and the frozen previous edition `contracts/fixtures/impl-2026-11-14.1/` with its eight vector files, admitted file by file with the directory's other contents excluded, as for every frozen edition, which `app/src/admission/validate.rs`, `app/src/admission/tests.rs` and `app/src/storage/managed_qualification.rs` embed.
- `0f557305` (B1, APP-203 and APP-205), 1 line: `app/src/storage/participation_campaign.rs`, the `participation` campaign's source, a module of `storage` compiled into the test library.

Nothing else in the contract moved: `app/runtime/runtime.py` still has `DATABASE_SCHEMA_VERSION = 24`; the three Dockerfiles, `.dockerignore` and `app/operator/Dockerfile.dockerignore` are release B's; nothing under `infra/` changed.

## What the approval must show

Trial runbook section 3a step 2. The approver compares the preparation summary with this record, not with the candidate's own `schema-release.json`, which `validate_summary` already binds:

- `schema_release: {from_version: 24, to_version: 24, spec_sha256: 561148629cf2f218…}` (the file-bytes row above);
- `schema_version: 24`;
- `preserved`: `schema`, `template`, `trial`, `aliases` and `secrets` all `true`;
- `sequence`: exactly `stop both services`, `execute reviewed change set`, `verify stopped boundaries`, `activate both services`, `verify health and images`; no `snapshot database` and no `run reviewed migration`;
- `deployment_contract_sha256: 3493b0a68b08b686…` (the `to` row above);
- the stop time and the scan as for any release. If preparation refuses on newly listed findings, runbook section 8 with `--contract 3493b0a68b08b686a245be7ffa602817772895c0f5cc8983c06ad78b7a3e2dd9` and `--expires-at-utc 2026-11-27T00:00:00Z`: `test_scan.ReleaseBindingTests` bind a committed disposition to this release's `to` contract and expiry.

## What runs

A normal promotion. Execution rechecks the release at its start and in the final checks before the outage. Then the 30-minute outage reserve, stop both services and prove them stopped, execute the reviewed change set (the task definitions on the candidate images), prove the stopped boundary, activate, and verify health and images. No RDS snapshot, no migration task and no `schema-` journal entry. The 05:00–05:45 UTC snapshot window does not apply. On success the baseline takes contract `3493b0a6…`; its schema manifest digest `176d68cb…` and `policy.schema_version` 24 stay. The completed record and `verified.json` name `{from_version: 24, to_version: 24, spec_sha256: 56114862…}` with no snapshot and no migration task.

Failures: a prepared release that is not approved is abandoned as any prepared release (`maintenance abandon`; `retained_schema_snapshot` null). A failure after the outage began is a normal promotion's failure and is reconciled as one ([BETA-PROMOTION "Deployment state and failures"](../../docs/pitch/BETA-PROMOTION.md#deployment-state-and-failures)). `restore-after-failed-migration` refuses it: `only an executing schema migration can be restored; reconcile a contract-only release as a normal promotion`.

## Expiry

**2026-11-27 00:00 UTC.** It falls after the planned release (Tuesday Nov 25 at the latest) and after the rehearsal (Wednesday Nov 26, which runs one of Release C's polls), which leaves Nov 26 for a second attempt; and before the codes (Friday Nov 28) and the class (Nov 29–30), so no contract change can promote in the class weekend even if the follow-up that removes the file is late. `applicable()` requires the clock to be before it at preparation and at both execution rechecks. A release-C scan disposition, if one is needed, must expire at the same instant. The deployment's stop time is separate: `DeploymentExpiresAtUtc` is 2026-12-15 14:00 UTC after release B's renewal.

## Everything else in this release

All of it is on `release/c-participation`, merged from the builders' branches and `main` ([integration record](integration-2026-11-14.md)).

| Ticket | What ships |
| --- | --- |
| APP-202 | Contract edition impl-2026-11-14.2: the participation grammar in both validators (one entry per creator, `requires` with `via` and `now`, `set_now`, the countdown renderer, a ranking by creator with `matches`, `multi_enum`). |
| APP-203 | One entry per person per key, judged under the organization's commit lock; "open while" guards with server time; 422 `unchanged` and `requirement_not_met`. |
| APP-204 | The countdown, and a ranking by person scored against a reviewer-only key list. |
| APP-205 | The reference picker in forms, and read-only stamped fields. |
| APP-206 | The approver step for a filer without share authority. |
| APP-207 | The composer generates the participation primitives from plain words. |
| APP-208 | Multi-select fields (stretch): checkboxes with per-option counts. |
| APP-211 | No misleading score for a viewer who cannot read the key; grading by name in the record chooser. |
| APP-209 | This record and the integration. |
| APP-210 | The mechanism, on `main` since PR #128. |

## Checks

Recorded in the pull request, with exact commands and results.

## Limits

- No AWS call was made and the canonical state was not read. That the live baseline holds `from` (contract `88d17cb5…`, schema manifest `176d68cb…`, policy schema version 24) rests on release B's readback of 2026-11-14 ([record](../APP-194/release-2026-11-14.md)). If either differs, preparation refuses before the lock. `maintenance inspect` shows both before promotion is enabled (runbook section 3a step 1).
- The `to` contract is the merged tree's at `b7bfcef5`. Any later change to a contract path on this branch fails `CommittedReleaseTests` and needs this record and the pin recomputed.
- Scanner findings at preparation are unknown until the images are built and scanned.
- `schema-release.json` and its pin are removed or replaced by a follow-up pull request after the release succeeds or expires. Until then, CI fails any change to the deployment contract.
