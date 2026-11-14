# Schema release 22 → 24 (release B)

Reviewed scope: the first beta promotion whose candidate carries migrations 0023 (W1: APP-182, APP-183, APP-190) and 0024 (W4: APP-191, APP-192), from release A, live since 2026-11-14 07:17 UTC (candidate `511d97e3`, schema 22; [record](../APP-175/release-2026-11-14.md)). The release is `ops/promotion/schema-release.json`. Every value in it is also pinned in `ops/promotion/schema_release.py` (`REVIEWED_SCHEMA_RELEASE`), so a JSON edit can neither widen nor retarget it: this record, versions 22 and 24, the four digests, the infrastructure digest and the expiry **2026-11-26 00:00 UTC**. The procedure is [Schema releases](../../docs/pitch/BETA-PROMOTION.md#schema-releases), as release A used it ([APP-175](../APP-175/SCHEMA-RELEASE.md)), with APP-196's exit-code contract.

Decided in the [trial plan](../../docs/planning/TRIAL-25-READINESS.md) (D14, amended 2026-11-14): release B ships 0023 and 0024 together, on Nov 23 at the latest, before the Nov 29–30 class.

## Scope

### The migrations

**0023** (`app/migrations/0023_consent_names_org_admin.sql`, 519 lines). It is additive.
- The version CHECKs widen to 1–23 (`:46–49`).
- Consent (D5, APP-182):
  - `code_reservations.notice_version` is a nullable column (`:54`);
  - the new table `genui.member_consents` (`:64–76`) has RLS forced and a key to `memberships`;
  - `admit_redemption` is replaced in place (`:227`) and writes one consent row and the `admission.consented` event.
- Names (D6, APP-190):
  - `memberships.display_name` is a nullable column, bounded and printable (`:82`);
  - the definer `set_display_name` (`:364`).
- Organization admin (D4, APP-183):
  - the definers `org_admin_overview` (`:408`) and `admin_remove_member` (`:451`), owned by the NOLOGIN `genui_admit`;
  - the operator's `access_codes.note` (`:87`).
- The admission chain's kinds are widened in place (`:101`).
- `reserve_code` is replaced to carry the notice version (`:115–116`), and `record_pending_admission` likewise (`:151`).

**0024** (`app/migrations/0024_private_entries_and_roles.sql`, 584 lines). It is additive, with one recorded exception.
- The version CHECKs widen to 1–24 (`:84–87`).
- **The exception** (W3's purge review R1). The key from `genui_identity.share_events` to `live_work_shares` is dropped (`:90`). This lets the end-of-test purge delete an organization's shares while a guest's hash chain stays verifiable. The operator's purge still refuses while a guest's rows exist (runbook §10; the follow-up is APP-188).
- Own collections (D7, APP-191):
  - `genui.canonical_jsonb` (`:104–109`) is a real UTF-8 JSON parse of the canonical bytes;
  - `definition_record_schemas.owner_only` (`:110–111`) is a STORED generated column derived from it. Adding it rewrites the table to compute the column, but no stored value changes and no writer changes.
- Reviewers (D8): `record_review` joins the work-permission CHECK (`:114–116`).
- Shares with roles and with everyone (D9, APP-192):
  - `work_shares` gains a nullable grantee (meaning everyone), a role, revocation columns, `removed`, and a widened `granted` CHECK (`:121–140`);
  - a partial unique index allows one live organization-wide share per work (`:142`);
  - a RESTRICTIVE read policy limits the application roles to the organization-wide rows (`:155`);
  - `share_work` is replaced with a role and a mode (`:175–181`);
  - two new definers: `set_work_everyone` (`:375`) and the owner's `revoke_work_share` (`:515`);
  - the three sharing definers take one advisory lock per work.

`app/migrations/catalog-v23.json` and `catalog-v24.json` pin each version's catalog. `manifest.json` reads `database_version` 24. `runtime.py` has `DATABASE_SCHEMA_VERSION = 24`, and the native pool requires 24 (`storage/pool.rs`) and catalog-v24 (`storage/catalog.rs`).

### Everything else in this release

All of it is on `main` after release A, or on the branches merged here. The pull request lists each branch head and its review files.

| Ticket | What ships |
| --- | --- |
| APP-182 | The invite page shows notice v1 and requires the consent checkbox. `/invite/validate` accepts exactly `code` and `consent=1`, and consent is recorded at admission. |
| APP-183 | The `/admin` page for organization admins: members, consent, activity and Remove (CSRF, confirm). Anyone else gets the uniform 404. |
| APP-184 | An app-wide daily cap on model calls: 1,000 a day, with the `GENUI_MODEL_DAILY_CALLS` override for tests. Over the cap: 429 `daily_limit` with a plain message. The feedback link. The per-address invitation and sign-in windows are sized for one classroom network. |
| APP-185–188 | (W3, PR #121.) The operator back office: `ops/promotion/admission.py`, the operator's note, `admin_policy`, `stats`, `list-box-files` and the purge. Also the expiry and month renewals, `ops/monitoring/alarms.json` and the trial runbook. The operator changes ride this operator image. |
| APP-190 | Member names: the first-sign-in name prompt (a banner) and names in share lists, admin and records, for any organization. |
| APP-191 | `visibility: "own"` and `reviewer_only` in the grammar (contract edition impl-2026-11-14.1). One storage helper is applied on every read and write path. Live hints for an own list reach only the pages that can see the row. The composer can generate both properties. |
| APP-192 | Share with viewer, contributor or reviewer, and with everyone in the organization. The share dialog. The owner's "Remove access". A busy share answers 429 with `Retry-After`. The canvas approver step uses the additive share. |
| APP-195 | Live refreshes start after a random 0–3 s and retry a 503 once. A hidden page defers until it is shown. The pool admits 32 callers per role (was 8). |
| APP-196 | `runtime.py migrate` exits 0 (committed), 1 (nothing committed) or 4 (commit outcome unknown), and checks deferred constraints before its COMMIT. `restore-after-failed-migration` accepts only exit 1. The runbook's fix-forward path. |
| APP-197 | A reused connection repeats the catalog comparison at most every 30 s (ADR-004 A5). Identity, baseline and version are still checked on every reuse. The `refresh-herd` 15-page gate is back in the tenant-foundation workflow. |
| APP-198 | A view whose list changed while it ran answers `409 stale`, and the card shows "Updating…" and reads again. It is never shown as "no access". |
| APP-200 | Record snapshot capacity follows the query worker's admission: 24 slots (was 4). |
| APP-194 | This record. The schema qualifier's 0024 upgrade checks (below). The retirement of release A's spent scan disposition (below). |

APP-199 (one transaction per read path) and APP-201 (a read-only schema-version command) are not in this release.

## Digests

Computed at the integration branch's head with this repository's own functions (`ops.promotion.candidate`). The tree digests use one `path NUL sha256(blob) LF` row per committed path, sorted, hashed with SHA-256.

| Field | Value | How |
| --- | --- | --- |
| `from.schema_manifest_sha256` | `7ceb859d1794e79224ed609bfb557c21e79c23020894540b7b5114044ec68176` | SHA-256 of `git show 511d97e3:app/migrations/manifest.json`; its `database_version` is 22. It equals release A's `to` side and the live canonical baseline ([record](../APP-175/release-2026-11-14.md)). |
| `from.deployment_contract_sha256` | `14a161a09cf93044cbf6d1281b02b56f954fa7f73ae3a17a8f54b07b7958d99b` | `candidate.deployment_contract_digest(repo, '511d97e3…')`, the live baseline |
| `to.schema_manifest_sha256` | `176d68cbc363a822a32b747b3a52f6a45809a946bed2649e93bd65f7d84208f1` | SHA-256 of `git show HEAD:app/migrations/manifest.json`; its `database_version` is 24 |
| `to.deployment_contract_sha256` | `88d17cb53bf77c2f2bfa0b4345953f9f74c0aa39b81b677d6105807a54bcc356` | `candidate.deployment_contract_digest(repo, 'HEAD')` |
| `infrastructure_sha256` | `e74b620e4d9749b5eb47819a1546be1adc00765008f042abbb0b747d9e549da1` | `candidate.infrastructure_digest(repo, rev)` over `infra/src/*` and the four `infra/` project files. It is the same at `511d97e3`, at `main` (`8bdbabf6`) and at `HEAD`, and is release A's value, so the stack is unchanged. |
| file bytes | `c3d1c055a02867da30ea7b43474bec6fafae9792ebae998e37aecf5877a2d5a4` | SHA-256 of the committed `schema-release.json` (`json.dumps(indent=2, sort_keys=True)` plus a newline, release A's form). The approval summary shows it as `spec_sha256`. |

`git diff --stat 511d97e3 HEAD` over the deployment contract's paths changes only these:
- `.dockerignore`, `app/operator/Dockerfile.dockerignore` and `app/qualification/Dockerfile.dockerignore`, which admit the new compiled sources;
- `app/migrations/0023_consent_names_org_admin.sql`, `0024_private_entries_and_roles.sql`, `catalog-v23.json`, `catalog-v24.json` and `manifest.json`;
- `app/runtime/runtime.py`: `DATABASE_SCHEMA_VERSION` 22 → 24, plus APP-196's explicit commit and exit codes.

Nothing under `infra/src` or the four `infra/` project files changed.

`ops/promotion/test_schema_release.py` recomputes the `to` digests and the infrastructure digest at `HEAD` on every run (`test_committed_release_is_bound_to_this_tree`). The planning and promotion CI therefore fail if the tree moves away from this release. The `from` side cannot be checked against Git in a shallow CI checkout. At runtime it must equal the canonical baseline.

## Expiry

The expiry is **2026-11-26 00:00 UTC**.
- It falls after the planned release (Sunday Nov 23), which leaves Nov 23–25 for a second attempt: a snapshot refused in the backup window, a scan-disposition renewal, or a re-merge.
- It falls before the rehearsal (Nov 26) and the class (Nov 29–30), so no schema change can promote in the class week even if the follow-up that removes the file is late.

No code or test bounds the value itself. `applicable()` requires the clock to be before it at preparation and at both execution rechecks (`schema_release.py:132`).

One test ties it to something else. While a scan disposition is committed, `test_scan.ReleaseBindingTests` binds that disposition's contract and expiry to this release. So a release-B disposition, if preparation needs one, must also expire 2026-11-26. It is spent once release B is live, because its findings become baseline findings. The runbook's §8 command now says 2026-11-26.

Separate from this: the deployment's stop time is `DeploymentExpiresAtUtc`. After `expiry_renewal --apply` it is 2026-12-15 14:00 UTC (canonical 2026-12-16 08:00 minus 18 hours).

## What runs

Approve outside 05:00–05:45 UTC. Then, after every pre-execution recheck, while beta still serves:

1. **Snapshot.** A manual snapshot `forma-schema-release-<promotion id prefix>-v24` of the `DatabaseEndpoint` instance, waited on for at most 12 minutes.
2. **Rechecks.** Every final recheck again, then the outage reserve check (30 minutes of the 55-minute budget left).
3. **Stop.** Stop both services and prove them stopped.
4. **Change set.** Execute the reviewed change set, which registers the task definitions on the candidate images, and record the stopped boundary.
5. **Migrate.** One run of `MigrationTaskDefinition` (`python3 /app/runtime.py migrate`; at most 12 minutes; exit 0 required). The runner:
   - applies 23 and 24 in one transaction and proves catalog-v23 after 23;
   - proves catalog-v24 last;
   - runs `SET CONSTRAINTS ALL IMMEDIATE`, then sends the COMMIT.
6. **Activate.** Prove the stopped boundary again, activate, and verify health and images.

On success the baseline takes the `to` digests and the policy schema version becomes 24. The sequence, the time budget and the snapshot, migration and abandonment behaviour are release A's, unchanged ([APP-175](../APP-175/SCHEMA-RELEASE.md#what-runs)).

The 0024 table rewrite (`owner_only` over every stored record schema) runs under the migration's 30 s statement timeout and 5 s lock timeout. The services are stopped by then, so nothing holds a lock. Beta's `definition_record_schemas` holds one row per record schema of each admitted definition revision, a small table.

## Rollback

The database is never rolled back by the pipeline. The snapshot protects the data.

**Exit codes** (APP-196; [BETA-PROMOTION](../../docs/pitch/BETA-PROMOTION.md#schema-releases)):

| Exit | Meaning | What follows |
| --- | --- | --- |
| 0 | Committed: schema 24. | Activation. |
| 1 | Refused or failed before the COMMIT was sent: schema 22, unchanged. This includes a failing deferred constraint, which the runner checks with `SET CONSTRAINTS ALL IMMEDIATE` before the COMMIT. | `maintenance restore-after-failed-migration` (below). |
| 4 | Commit outcome unknown: the COMMIT may have reached the server, so the database may be at 22 or 24. | Fix forward (below). |
| other | Not the program's own (a signal's 128+N, Python's 120); proves nothing. | Fix forward. |

**Restore after exit 1, or a migration that never started.** `maintenance restore-after-failed-migration`:
- runs under the operator identity, with the reviewed state hash and after the 65-minute fence;
- shows a read-only plan, then applies it with `--apply`;
- writes immutable receipts and never replays.

It puts release A's images and template back, never data. It accepts either no `schema-migrate` entry, or one observed STOPPED with `EssentialContainerExited` and exit 1. Beta is down for at least the fence.

**Fix forward after exit 4 or any other code.**
1. Prove that the migration task has stopped.
2. Read the schema version. That read is a separately reviewed operator action: no command runs it in this state, and APP-201 tracks one.
3. Act on the version: at 24, a reviewed delivery activates; at 22, restoring is a separately reviewed delivery.

The restore refuses exit 4 whatever the read shows, because its proof is the exit code.

## The 0024 preflight question

W4 wrote a read-only preflight ([`evidence/APP-191/break-and-restore-2026-11-14.md`, "Release B preflight"](../APP-191/break-and-restore-2026-11-14.md#release-b-preflight-on-beta-read-only-review-m5)). It derives `owner_only` for every stored schema, as 0024 will, so that a row that would fail the migration shows up first. No role can query beta's database before the outage, so it cannot run.

**Answer: the migration is its own check, and the preflight is not needed for safety.**

**In the code.** The path from a bad stored schema to exit 1:
- `0024:110–111` adds `owner_only` as a STORED generated column. PostgreSQL computes it for every existing row while it rewrites the table.
- The expression is `genui.canonical_jsonb` (`0024:104–107`): `convert_from(bytes,'UTF8')::jsonb`. It raises `22021` on invalid UTF-8 and `22P02` on invalid JSON.
- `runtime.apply_migrations` runs every migration in the caller's one transaction (`runtime.py:905–916`).
- `migrate()` catches any exception, closes the connection without a COMMIT and re-raises (`runtime.py:934–943`). The `__main__` handler then exits 1 (`runtime.py:1312–1314`).
- `restore-after-failed-migration` accepts exactly that outcome (`schema_recovery.py:80`).

No other statement in 0023 or 0024 depends on existing rows:
- new columns are nullable, with no default;
- the CHECKs on them pass, because every value is NULL;
- 0024's new key (`:129`) is on the new, all-NULL `revoked_by`;
- the re-added `granted` CHECK (`:140`) is a superset of 0008's;
- the partial unique index (`:142`) covers rows with no grantee, and there are none before 0024, because `grantee_id` was NOT NULL until `:121`;
- the other statements are functions, grants and policies.

**Against the disposable PostgreSQL.** `app/runtime/qualify_tenant_schema.py`, `release_b_upgrade_checks`, on the pinned PostgreSQL 16.15 image, adds three checks:

| Check | Result |
| --- | --- |
| A populated version-22 database (organization, owner, app, revision, three stored schemas: ASCII, non-ASCII, a backslash escape) upgraded by the **real `runtime.py migrate` process** | exit 0, `tenant_foundation_migration_complete`, version 24, `owner_only` false for all three, every stored byte unchanged |
| The same plus one stored schema that is invalid JSON | `apply_migrations` raises `22P02`. Version 22 and the whole `pg_dump` are unchanged. The real migrate process exits **1**, never 4, with nothing written. After that one row is made parseable, the same database migrates (exit 0, version 24). |
| The same with invalid UTF-8 | `22021`; otherwise the same as the row above |

The migrate process is the shipped module, run with `runpy`. Only `psycopg.connect`'s TLS endpoint is swapped for the fixture's loopback port.

Break proofs:
- a `migrate()` that swallows the failure exits 4 and fails the check;
- a "broken" row that in fact parses fails it with "0024 accepted a stored schema".

**So what is missing is only a warning.** Skipping the preflight risks one outage at most, not data or an unknown state. If a stored schema were unparsable, the release would stop inside the outage with exit 1, at schema 22, and the restore would bring release A back after the 65-minute fence. A preflight would only have warned before the outage, and no role can give that warning. Such a row is also implausible: the only writer stores the canonical encoder's UTF-8 JSON. Statement 3 of the preflight (how many schemas are `own`) is informational. Before W4's grammar, admission refuses a `visibility` key, so beta has none. The release record therefore runs without the preflight.

## Deferred constraints

APP-196's final check found no deferred-constrained writes; this restates it at HEAD.
- **The deferrable keys.** The schema's only deferrable constraints are four keys in `0002_tenant_foundation.sql`:
  - `:135` (`record_collections` → `saved_work`) and `:145` (`work_bindings` → `saved_work`), both `DEFERRABLE INITIALLY IMMEDIATE`;
  - `:174` (`internal_record_revisions` → `internal_records`) and `:179` (`internal_records` → `internal_record_revisions`), both `DEFERRABLE INITIALLY DEFERRED`.
- **No other deferral.** No migration contains `SET CONSTRAINTS`, a constraint trigger or another `DEFERRABLE` clause (`git grep -n DEFERRABLE app/migrations`).
- **No top-level row writes.** 0023 and 0024 have no top-level `INSERT`, `UPDATE`, `DELETE`, `DO` or `SELECT`. Every row write is inside a function body:
  - 0023: `:116`, `:151`, `:227`, `:364`, `:408`, `:451`;
  - 0024: `:181`, `:375`, `:515`.
  
  Those bodies run at application time in their own transactions. This was checked by stripping comments and dollar-quoted, `BEGIN ATOMIC` and `RETURN` bodies, then classifying every remaining statement.
- **None of the four tables.** Neither migration writes, alters or adds a key to `record_collections`, `work_bindings`, `internal_records` or `internal_record_revisions`.

So the migration's COMMIT has no pending deferred check from 0023 or 0024. `SET CONSTRAINTS ALL IMMEDIATE` (APP-196) would turn one into exit 1 anyway.

## Scan disposition

The committed `ops/promotion/scan-disposition.json` was release A's: 16 findings per role, `f00adb9a…`, bound to contract `14a161a0…` and to release A's predecessors, expiring 2026-11-20. It was spent when release A went live, because its predecessors are no longer live. `test_scan` also bound it to `REVIEWED_SCHEMA_RELEASE['to']`, which this release replaces. Release B's images are not built yet, so its disposition, if one is needed, cannot exist now.

- **Retired.** The file is removed and `registry.REVIEWED_DISPOSITION` is `None`.
  - With no newly introduced finding, the guard never reads a disposition (tested with the loader set to fail).
  - With introduced findings and no file, `guard_scan_delta` refuses `new scan findings require separately reviewed disposition: <role>`. That is the trigger in runbook §8; it never crashes.
  - A file without its pin refuses `scan disposition has no reviewed source pin`.
  - `github.validate_summary` refuses a summary that names reviewed findings when no disposition exists. It raised `TypeError` before.
  - `scan_renewal check` refuses `no scan disposition is committed`.
- **Fixture.** Release A's disposition is now the test fixture (`test_scan.release_a_disposition`). It is regenerated by `scan_renewal generate` from the committed `evidence/APP-175/scan-delta-2026-11-14.json`, and its bytes must hash to `f00adb9a…` with the reviewed pin. The tamper suite, the runner's and summary's disposition tests and the renewal round-trip run on it.
- **Binding.** `ReleaseBindingTests` run only while a disposition is committed. They bind it to the pending release's `to` contract and expiry, and read the one `scan-delta-*.json` beside its evidence record. A consistency test requires the file and its pin to be committed together.
- **If release B's preparation introduces findings,** follow runbook §8. `scan_renewal delta --contract 88d17cb5…` (this release's `to` contract), then `generate --expires-at-utc 2026-11-26T00:00:00Z`. Paste the printed pin, write the record, and get it reviewed.

## Checks

Recorded in the pull request, with exact commands and results.

## Limits

- No AWS call was made and the canonical state was not read. That the live baseline holds the `from` digests and `policy.schema_version` 22 rests on release A's readback of 2026-11-14 ([record](../APP-175/release-2026-11-14.md)). If either differs, preparation refuses before the lock. `maintenance inspect` shows both before promotion is enabled.
- Beta's stored schemas were not read. The preflight answer above rests on the code path and the local proof, not on beta's rows.
- **The execute role.** The bootstrap readback of 2026-11-14 simulated release A's snapshot name (`…-v22`). The policy (`ops/promotion/bootstrap.py:77–90`) names every `forma-schema-release-*` snapshot, the migration task definition's whole family (`…:*`) on the stack's cluster, and that task's two roles. The infrastructure is unchanged, so release B needs no bootstrap change. Confirm with the same `iam simulate-principal-policy` on `…-v24` before approving.
- **Operator steps.** The expiry and month renewal (runbook §2) must be applied within 24 hours before the merge. Otherwise release B subtracts 18 hours from the 2026-11-24 14:00 UTC stop, and the stop refuses within 24 hours.
- **Removal.** `schema-release.json` and its pin are removed by a follow-up pull request after the release succeeds or expires. Until then, CI fails any change to the deployment contract.
- **The scan.** Scanner findings at preparation are unknown until the images are built and scanned.
- **The 65-minute fence.** A failed migration keeps beta down at least that long before the restore.
- **Invitations in flight.** 0023 refuses a pending admission recorded before it, which has no notice version, as malformed (its header). An invitation started on release A and finished on release B must start again. Before release B only Victor's account exists, and codes go out on Nov 28.
