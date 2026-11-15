# Contract-only release at schema 24 (release E: footer, top bar, JSON logs, AI usage)

Reviewed scope: a beta promotion that changes the deployment contract without a migration, the third after release C ([record](../APP-209/CONTRACT-RELEASE.md)) and release D ([record](../APP-217/CONTRACT-RELEASE.md)). It ships:
- PR #149: APP-223, the site footer, `/notice`, `/.well-known/security.txt` and the canvas Help menu, and APP-224, the canvas top-bar refit;
- PR #150: APP-219 and APP-222 part A, the JSON logs, the typed model caller and source, the usage events, the 80 % and 100 % warnings, and the MCP limits as settings.

It starts from release D, live since 2026-11-15 14:31 UTC (candidate `b1c7fcd6`, tag `beta/20261115-b1c7fcd`; [record](../APP-217/release-2026-11-15.md)). Both PRs passed independent Fable reviews (GO), and their batched findings are applied or recorded in the PR bodies.

## The record

| Field | Value | How |
| --- | --- | --- |
| `from.database_version` | 24 | the canonical `policy.schema_version` |
| `from.schema_manifest_sha256` | `176d68cbc363a822a32b747b3a52f6a45809a946bed2649e93bd65f7d84208f1` | SHA-256 of `app/migrations/manifest.json`, unchanged since release B |
| `from.deployment_contract_sha256` | `8578d6ac6fc95d40cda7055a3724b194a0a3314c7344bd295b8aeb2c324b6c48` | release D's `to`, the live baseline (`candidate.deployment_contract_digest(repo, 'b1c7fcd6…')`) |
| `to.database_version` | 24 | unchanged |
| `to.schema_manifest_sha256` | `176d68cbc363a822a32b747b3a52f6a45809a946bed2649e93bd65f7d84208f1` | the same bytes |
| `to.deployment_contract_sha256` | `79263fbbdbf4c59ced39902f76b2a352f443c3a35dfad6a926b7e23912f081b1` | `candidate.deployment_contract_digest(repo, 'HEAD')` at `72fc0f36` (main after PRs #149 and #150) |
| `infrastructure_sha256` | `e74b620e4d9749b5eb47819a1546be1adc00765008f042abbb0b747d9e549da1` | unchanged |
| `expires_at_utc` | `2026-11-27T00:00:00Z` | before the codes and the class |
| `evidence` | `evidence/APP-223/CONTRACT-RELEASE.md` | this file |
| file bytes | `5bb250f9a6e521de21a3120ec6d85d8ad924cdc66fd22c1c94c55554d7ba259a` | SHA-256 of the committed `schema-release.json`; the approval summary's `spec_sha256` |

## What changed in the contract

`git diff --stat b1c7fcd6 72fc0f36 -- app/migrations infra/src app/runtime/runtime.py` prints nothing, and no Dockerfile changed. The one contract file is the root `.dockerignore`, with +7 lines admitting `app/admin/src/notice/app.tsx` and `app/admin/src/site/footer.tsx` (APP-223's review checked this). PR #150 changed `Cargo.toml` and `Cargo.lock` (the `json` feature of the pinned `tracing-subscriber`, adding `tracing-serde 0.2.0`), which are not contract paths.

## What the approval must show

- `schema_release: {from_version: 24, to_version: 24, spec_sha256: 5bb250f9a6e521de…}`, `schema_version: 24`, and every `preserved` flag true;
- the sequence stop, execute, verify stopped, activate, verify health and images, with no snapshot and no migration;
- `deployment_contract_sha256: 79263fbbdbf4c59c…`;
- the stop at **2026-12-12T14:00:00Z** (release D's 2026-12-13T08:00Z minus 18 hours), and the scan as for any release.

## After the release, beyond the readback

- **JSON logs:** the service's log lines are JSON. Read one with `aws logs filter-log-events` and confirm there is no `tokio_postgres` INFO noise.
- **Pages:** the footer is on `/`; `/notice` answers 200; `/.well-known/security.txt` answers its four lines.
- **The operator note:** a malformed `RUST_LOG` now refuses start (review of PR #150); nothing sets it.
- **Follow-up:** metric filters and alarms on `model.usage_threshold`, `model.call_failed` and `http.server_error` (APP-219's deployed acceptance), written against real lines.
