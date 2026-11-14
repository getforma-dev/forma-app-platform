# Release B integration: independent Tier A review (2026-11-14)

**Reviewer:** Claude (Opus 5.5), an independent subagent. It did not write the integration and did not review any of its input branches.

**Base:** `release/b-schema-24` at `8a78bbf05bb273fa631c242de621b45caa24b639` (PR #124), in the worktree `forma-relb`. `origin/main` is `8bdbabf6`. Nothing in the worktree was changed except this file.

**Verdict: no BLOCKER and no MAJOR. Two MINOR and seven NIT.** The integration's own additions are sound for merge:
- the release record;
- the scan disposition's retirement;
- the semantic conflict resolutions;
- the 0024 preflight answer and the deferred-constraint claim;
- the docs.

Every digest recomputes and every check this review ran is green, apart from one load-related flake that passed on rerun. MINOR-1 must be closed before any promotion after Nov 26 needs a scan disposition. MINOR-2 must be closed before go/no-go item 6 is signed on the refresh-herd evidence.

## Scope and method

The scope was what the integration itself adds. Each input branch had its own Tier A review, so their insides were not re-reviewed.

The integration's commits (first parent, `origin/main..HEAD`):
- `712cb218`: the qualifier's 0024 upgrade checks;
- `d0f2fcd3`: the semantic conflict resolutions;
- `075d4337`: the disposition retirement;
- `78131d23`: the release record;
- `f61b5999`: the docs;
- `a68dc280` and `8a78bbf0`: the ticket, the record's file sha and the fixture reset fix;
- `5b993fe7`: the recipe;
- the merges `3a0b56d1`, `be902201`, `cc19ab53`, `c1abf31d`, `6431780d`, `ab375e55`, `f0b6365c` and `21b1bd99`.

Method:
1. Recomputed every digest with the repository's own functions.
2. Read each changed line of the integration commits. In the merges, read every hunk that `git show --cc` reports. Only `cc19ab53` has hunks: the tenant-foundation steps, `live.ts` and `board.ts`.
3. Read the code each claim in the record rests on:
   - `0024`;
   - `runtime.py` `apply_migrations`, `migrate` and `commit_migration`;
   - `schema_recovery.py`;
   - `registry.py`, `github.py`, `scan_renewal.py` and the tests that changed;
   - `admission_operator.py` purge;
   - the admission grammar (`validate.rs`, `canonical.rs`, `definitions.rs`).
4. Diffed catalog-v22 against catalog-v24 for every constraint.
5. Ran an upgrade probe of my own, from a seeded schema-22 database to 24.
6. Dry-ran runbook §8 in an isolated copy, and ran break proofs there.
7. Looked for interactions between branches that no single review could see.
8. Ran `just check`, the schema qualifier, the operator package tests and the seven named campaigns. `refresh-herd` ran last, alone.

## Commands and results

All runs are local, on the shared 10-core Mac (Docker VM 10 CPUs, 8.3 GB). Load figures are 1-minute averages.

| Command | Result |
| --- | --- |
| `python3 -I -c` over `ops/promotion/candidate.py` `deployment_contract_digest` / `infrastructure_digest` at `511d97e3…`, `8bdbabf6…` and `HEAD`, plus `sha256(git show <rev>:app/migrations/manifest.json)` | Release A `511d97e3`: contract `14a161a0…d99b`, infra `e74b620e…9da1`, manifest `7ceb859d…8176` (version 22). `main`: the same three. `HEAD`: contract `88d17cb5…c356`, infra `e74b620e…9da1`, manifest `176d68cb…08f1` (version 24). Each equals `schema-release.json` and `REVIEWED_SCHEMA_RELEASE` exactly. |
| `sha256(ops/promotion/schema-release.json)` and a re-encode test (`json.dumps(indent=2, sort_keys=True)+"\n"`) | `c3d1c055…a5a4`, as the record says. The bytes equal the canonical form. |
| `git diff --stat 511d97e3 HEAD -- <the contract's paths>` | 9 files: the three dockerignores, 0023, 0024, catalog-v23, catalog-v24, `manifest.json` and `runtime.py`. Nothing under `infra/`. |
| `just check` | Exit 0. All 21 steps ok. `py-runtime`: 220 tests (296 s), including the database-backed operator tests at schema 24. `py-promotion`: 238 tests, OK, skipped=2 (the two `ReleaseBindingTests`, because no disposition is committed). `board`: OK. |
| `python3 -m unittest -v ops.promotion.test_schema_release` | 34 tests OK, including `test_committed_release_is_bound_to_this_tree`, `…applies_only_to_its_reviewed_baseline`, `…equals_the_source_pin` and the end-to-end `test_the_committed_release_and_disposition_promote_together`. |
| `app/runtime/.venv/bin/python app/runtime/qualify_tenant_schema.py --output <scratch>/schema.json` | `passed: true`, 413 checks, every container removed (1 min 47 s). It includes the three `release_b_upgrade_checks`: the upgrade by the real migrate process exits 0; invalid JSON refuses with `22P02` and exit 1; invalid UTF-8 refuses with `22021` and exit 1. |
| The reviewer's own upgrade probe, 22 → 24 (below) | Exit 0. Every pre-existing row is unchanged. The 0024 definers work over a version-22 share row. |
| `just campaign private-entries` | ok, 44 s |
| `just campaign share-routes` | ok, 31 s |
| `just campaign sharing` | ok, 17 s |
| `just campaign postgres` | ok, 62 s |
| `just campaign box`, first run (load about 9–10, run back to back with the others) | **FAILED.** The `kpis` view failed after dispatch with `deadline` (2212 ms), then `box_campaign.rs:1445` saw `Null` for the teammate's explanation `exact`. |
| `just campaign box`, rerun alone (load 9.3 → 4.8) | ok, 98 s. The deadline is the query worker's 2 s. No query-worker code changed since `main` (`git diff origin/main HEAD -- app/src/query/worker.rs app/src/query/mod.rs` is empty). I read the first failure as host load, not the integration. |
| `just campaign query` | ok, 80 s |
| `just campaign refresh-herd` (the 15-page gate), alone after everything else, load 4.72 → 4.99 | ok, 100 s. `gate_passed: true`, `gate_c_503: 111`. Run A, three herds: first-try 200 share 1.0 in each, 0 terminal failures, 0 view 429s, p95 current 4650 / 5759 / 4858 ms, slowest 5771 ms, commit 77 / 208 / 97 ms, 4 catalog checks, no admission saturation. Chrome and other desktop apps were busy, so the host was not fully quiet. |
| `app/operator/.venv/bin/python -m unittest discover -s app/operator -p test_operator.py` | 40 tests OK, including `test_dockerignore_is_an_exact_allowlist_of_tracked_files`. This suite is not in `just check`. |
| `local_harness.source_names()` against `check_image.allowlist(app/qualification/Dockerfile.dockerignore)` | All 227 compile-time sources are admitted, and nothing untracked is allowlisted. |
| Runbook §8 dry run and seven break proofs, in an isolated copy (below) | §8 works against the `to` contract. Every guard bites. |
| `gh pr checks 124`, read once | Every check passes except one of the two `tenant-foundation` runs: run 43597356148 failed after 26 min 45 s, and run 43597359475 passed. Logs not read, per the task's rule. The coordinator is watching CI. |

### The upgrade probe

The script was a throwaway in the reviewer's scratch space and is not committed. It used the qualifier's own `LocalPostgres` (PostgreSQL 16.15), `migrations_at` and `run_migrate`.

**Seeding at version 22.** HEAD's migrations 1–22 were applied, then:
- the APP-008 tenant seed (`tenant_seed.seed`) with Northstar, Spend Review and the library apps. That gives 23 apps, 23 real canonical record schemas, publications, saved work, sources, and internal records under the deferred keys.
- audit heads;
- a personal share made by the **version-22** `share_work` definer, called as `genui_workspace` (owner → member, record_write);
- a guest live share, accepted, with a `share_events` row and a linked app.

That made 726 rows in 37 tables.

**The upgrade.** The real `runtime.py migrate` process ran, as the qualifier runs it.

**Results:**
- Exit 0, `tenant_foundation_migration_complete`, version 24.
- 68 pre-existing tables compared. Every pre-existing column of every row is unchanged.
- The new columns are NULL on every row:
  - `access_codes.note` and `code_reservations.notice_version` (no rows were seeded in these two);
  - `memberships.display_name`;
  - `work_shares.role`, `revoked_at`, `revoked_by`, `revoke_event_id` and `removed`.
- `owner_only` is false for all 23 schemas.
- `member_consents` is the only new table.
- `share_events` keeps only `share_events_account_id_fkey`.

**The 0024 definers over the legacy row,** whose `role` is NULL:
- `share_work(…, 'viewer', 'set')` removed `record_write` and appended `permission.revoked`.
- `revoke_work_share` then removed `record_read`.
- All three of the grantee's `work_shares` records end revoked, and no work permission is left.
- The admission chain recomputes through sequence 4 (`qualify_tenant_schema.verify_chain`).

### The runbook §8 dry run and the break proofs

The copy was `git archive HEAD` of `ops/`, `infra/`, `app/migrations/`, the contract's other files, `evidence/APP-175` and `evidence/APP-194`, in a fresh `git init`.

**The synthetic inputs.** Release A's committed delta (`evidence/APP-175/scan-delta-2026-11-14.json`) supplied the per-role "current" ECR scans, each with its 16 findings, and empty baselines with release A's predecessor digests.

**The dry run:**
1. `scan_renewal delta --contract 88d17cb53bf7…c356` wrote `evidence/APP-194/scan-delta-2026-11-23.json`.
2. `generate --evidence evidence/APP-194/SCAN-DISPOSITION.md --expires-at-utc 2026-11-26T00:00:00Z` printed the pin, which replaced `REVIEWED_DISPOSITION = None`.
3. `scan_renewal check` printed `bound`, with disposition `4140ebf8…`.
4. `test_scan` ran 17 tests OK. Both `ReleaseBindingTests` now ran and passed.
5. `test_scan`, `test_schema_release`, `test_runner`, `test_github` and `test_scan_renewal` together: 76 tests OK.
6. `guard_scan_delta` accepts 16 of 16 per role on 2026-11-23 for the `to` contract. It refuses on 2026-11-26 and for the baseline contract `14a161a0…`.

The full `ops/promotion` discovery in the copy shows 24 errors and 2 failures in `test_admission` and `test_candidate`, identically with and without the disposition. They are artifacts of the partial copy, which lacks paths those tests read.

**Break proofs.** Each was made on the copy without a disposition, then restored, and the restored copy is green again.

| | Break | Result |
| --- | --- | --- |
| B1 | `guard_scan_delta`'s `disposition is not None` check removed | 6 errors (`TypeError` instead of the refusal) |
| B2 | the loader's expiry pin check removed | 4 failures (the tamper suite) |
| B3 | the loader's missing-pin check removed | 1 error |
| B4 | the severity comparison in `reviewed_finding` removed | 2 failures |
| B5 | a pin kept with no file | `CommittedDispositionTests` fails |
| B6 | `validate_summary`'s `None` check removed | 1 error |
| B7 | `scan_renewal check`'s `None` check removed | 1 error |

## Findings

### MINOR-1: after release B, a scan disposition for any later promotion is forced to expire on 2026-11-26

**Where:**
- `ops/promotion/test_scan.py:323–330`, `test_contract_and_expiry_are_the_schema_releases`. Line 326 asserts the disposition's expiry equals `schema_release.REVIEWED_SCHEMA_RELEASE['expires_at_utc']`, whether or not `schema-release.json` exists.
- `docs/operations/TRIAL-RUNBOOK.md:280–300`, §8, which covers only release B's preparation.
- `docs/pitch/BETA-PROMOTION.md:119`, which says the pin may stay as an inert record.

`075d4337` turned these tests from release A's specific binding into one that runs "whenever a disposition is committed".

**Failure scenario:**
1. Release B goes live on Nov 23, and the follow-up removes `schema-release.json` but keeps the inert pin, as the docs allow.
2. On Dec 1, during the trial, a fix needs promoting.
3. Overnight the scanner reports a new finding on the unchanged base image. It did so on Nov 5, Nov 12 and Nov 14.
4. Preparation refuses, so the operator follows §8 and generates a disposition expiring, say, Dec 8. `ReleaseBindingTests` fails CI, because the expiry is not 2026-11-26.
5. A Nov 26 expiry would pass CI, but the guard refuses it as expired.

The fix cannot ship without an unplanned edit to a test or to the inert pin, made in the middle of the trial. The contract binding is not a problem here, because a fix that touches no contract path keeps the contract `88d17cb5…`.

**Fix (either):**
- bind the expiry to the schema release only while `schema_release.SPEC.exists()`, and otherwise require only expiry > now and ≤ the deployment's stop;
- or add a §8 paragraph for a non-schema promotion after release B that says which test lines change and who reviews them.

### MINOR-2: the refresh-herd evidence describes and measures a pre-W4 workload on the branch that ships W4

**Where:**
- `app/src/storage/qualification.rs:2785–2786`. The report's `setup` says "no per-person list on this branch" and "no organization-wide share on this branch". Both are false at this HEAD.
- The campaign's simulated client, `qualification.rs:2136–2170`. It models the view's 429 retries and the one 503 retry, but not APP-198's `409 stale` re-read (`app/admin/src/canvas/queries.ts:17`, `:136–140`: up to three re-reads, each after a fixed 1 s with no jitter).
- `docs/planning/TRIAL-25-READINESS.md:196` and go/no-go item 6 rely on this evidence for "the 25-member herd".
- The same file's item 5, added by `f61b5999`, now requires every student filing to go in a private-entries (`own`) list.

**Failure scenario.** On class day, 25 students submit into an `own` list of an app shared with "everyone", while Victor edits the shared assignment list. Many writers move the lists under the views.
- The gate never produced a `409 stale`. Its one write per herd, 10 s apart, cannot.
- Each stale answer costs the client up to three more reads, in step 1 s apart.
- The 26-page release measurement that the recipe (`5b993fe7`) prescribes would run the same shared-list shape.

**What limits the risk:**
- `own_rows` and the reviewer clause are applied on every record statement as an InitPlan, so the measured reads already pay W4's authorization SQL.
- An `own` write fans out to fewer pages than a shared one.
- The campaign counts a 409 as terminal, so the gate is conservative, not weakened.

That is why this is MINOR, not MAJOR.

**Fix.** Before the 26-page measurement:
- give the campaign's setup an `own` entries list, written by several members within a herd, and an organization-wide share in place of the 25 personal shares;
- model the stale re-read in the simulated client;
- correct the two setup strings.

Alternatively, record in the plan, in writing, that the herd evidence covers the shared-list fan-out with one writer only.

### NIT-1: the record names two parse errors where any jsonb parse error applies

**Where:** `evidence/APP-194/SCHEMA-RELEASE.md:152`. The record says `canonical_jsonb` "raises `22021` on invalid UTF-8 and `22P02` on invalid JSON".

**Gap.** forma-json-v1 can encode U+0000 as `\u0000` (`app/src/admission/canonical.rs:139–143`), and PostgreSQL's jsonb refuses that with `22P05`. Any such error still fails the transaction and exits 1, so the conclusion stands.

The real reason no stored record schema can carry it is stronger than "the canonical encoder's UTF-8 JSON": the record-schema grammar has always admitted only ids (id, version, fields of id / value_type / nullable). I checked the grammar at `a8d93c31`, `90c7d3b8`, `cc23c557` and HEAD.

**Fix:** say "any jsonb parse error (`22021`, `22P02`, `22P05`, …)" and cite the closed grammar.

### NIT-2: the record's list of statements that touch existing rows is incomplete

**Where:** `SCHEMA-RELEASE.md:157–163`. The list of statements that "depend on existing rows" omits two CHECKs that are validated over existing rows:
- the re-added `work_permissions_permission_check` (`0024:114–116`);
- `work_shares_grantee_or_role`, which reads the old `grantee_id` as well as the new `role`, so "every value is NULL" does not cover it.

Both pass. The first is a superset of catalog-v22's set. The second holds because `grantee_id` was NOT NULL until `:121`. I diffed every constraint between catalog-v22 and catalog-v24, and none is deferrable.

**Fix:** add them.

### NIT-3: "No role can query beta's database before the outage" overstates

**Where:** `SCHEMA-RELEASE.md:146`. The operator task connects as `genui_migration` (`stats`, `list`). What is true is that no reviewed command runs the preflight's query.

**Fix:** "no reviewed command can run it", as the rollback section already says for the version read.

### NIT-4: the refresh-herd setup uses `ShareMode::Set`

**Where:** `app/src/storage/qualification.rs:2752–2759` uses `ShareMode::Set`. The call it replaces, the old `share_work(.., true)`, was additive: 0008's definer only inserts. The literal translation is `ShareMode::Ensure`.

For a member with no rows the two are identical, so the measured shape is unchanged. Contributor is right (`from_record_write(true)`), and the app owner is not among the members, so Set's owner refusal never applies.

**Fix:** none needed. Optionally use `Ensure` and say why.

### NIT-5: the stats table list omits 0023's `member_consents`

**Where:** `app/runtime/admission_operator.py:1025`. `STATS_TABLES` ("what a restore drill compares … and what the end-of-trial purge proves empty") was written at schema 22 and does not count 0023's `member_consents`. W3 and W1 were reviewed separately, so neither review could see this.

**Effect.** The purge still deletes those rows and proves them gone: `content_tables` comes from `pg_catalog`, and `remaining` is checked over all of them. Only the operator-visible before/after `stats` and the restore drill's comparison leave out the consent records.

**Fix:** add `member_consents` to `STATS_TABLES`, with a test.

### NIT-6: the PR body's "Operator steps after merge" heading is wrong

**Where:** PR #124 body. The heading says "after merge", but steps 1–4 come before step 5, "Merge", and the record (`SCHEMA-RELEASE.md:220`) says the renewal must land within 24 hours *before* the merge.

**Fix:** "Operator steps around the merge (runbook §2–3)".

### NIT-7: the ticket's "What changes" still promises a renewed disposition

**Where:** `Tickets/0-backlog/APP-194.md:30` says "A renewed scan disposition for the release window". After `075d4337` there is none unless preparation introduces findings. The log says so.

**Fix:** "a scan disposition only if preparation introduces findings (runbook §8)".

## What I verified

**1. The release record**
- `from` is release A's live baseline as read back in `evidence/APP-175/release-2026-11-14.md`: schema 22, manifest `7ceb859d…`, contract `14a161a0…`. `to` is this tree. The infrastructure digest is unchanged at `511d97e3`, at `main` and at HEAD. The JSON equals the source pin field for field.
- `schema_release.load()` and `applicable()` refuse:
  - a retyped file;
  - another baseline or policy version;
  - a checkout other than the candidate;
  - a clock at or after expiry (tests ran).
- `test_committed_release_is_bound_to_this_tree` passes and recomputes the `to` and infrastructure digests from HEAD. No test reads the wall clock against the expiry.
- **The expiry, 2026-11-26 00:00 UTC, is sound:**
  - it is after the planned Nov 23 release, with Nov 23–25 for a retry, which the runbook §2 repeat and revert path supports;
  - it is before the Nov 26 rehearsal and the Nov 29–30 class;
  - preparation and both execution rechecks enforce it;
  - a packet prepared before it and approved after it refuses before the snapshot.
  
  Two consequences are worth knowing. If release B never lands, no promotion of `main` at all is possible in the class week, which is the intent. A post-release disposition is forced to the same date (MINOR-1).

**2. The scan disposition's retirement**
- Without a file, `scan_disposition()` returns `(None, None)`.
- With introduced findings, `guard_scan_delta` refuses cleanly with `new scan findings require separately reviewed disposition: <role>`. That is the string runbook §8 keys on. With no introduced finding, it never reads a disposition.
- A file without its pin refuses.
- `validate_summary` and `scan_renewal check` refuse instead of crashing. No other caller of `scan_disposition` or `REVIEWED_DISPOSITION` exists (`git grep`): `reviewed_finding` is reached only after the pin check.
- The release A fixture regenerates from the committed delta to `f00adb9a…`, and the tamper suite runs on it. Break proofs B1–B7 all bite.
- `ReleaseBindingTests` are skipped while no file exists, so they bind only a disposition that exists. `CommittedDispositionTests` requires the file and the pin to land and leave together.
- Runbook §8 works end to end against the `to` contract `88d17cb5…` with the Nov 26 expiry (dry run above).

**3. The semantic conflict resolutions (`d0f2fcd3`, `cc19ab53`)**
- **`qualification.rs`:** Contributor is the old `true`, and the share mode is equivalent here (NIT-4). The campaign passes.
- **`test_runtime.py`:** `APPLIED = range(21, DATABASE_SCHEMA_VERSION + 1)` follows the constant. The test still sees what the fake runner applied, so a skipped migration would still fail it. It passes.
- **`test_admission_operator.py`:**
  - The note test now proves the column exists, the note is stored, `code.issued` is appended, and `list` shows the note.
  - The pre-0023 refusal is still covered by `test_issue_with_a_note_binds_it_to_the_insert_and_the_result` and `test_issue_note_is_bounded_and_chooses_the_0023_insert`, both mocked.
  - The purge test expects 0024's dropped `share_events` key and the remaining `linked_apps` key. The purge's `foreign_children ⊆ IDENTITY_TABLES` check still holds, and a guest's `share_events` rows still refuse by count (`guest_linked_shares_present`).
  - Both sides' intents are kept. The operator's comment matches.
- **`cc19ab53`:**
  - both workflow steps are kept;
  - `live.ts` keeps both W2b's notice and APP-195's schedule and retry;
  - the `board.ts` import is the union of both;
  - the pool keeps catalog-v24 and version 24, with no schema-22 literal left in code (`git grep`).

**4. Interactions examined**
- **APP-197's catalog cadence and W4's tables.** The cadence compares against catalog-v24, which holds W4's tables, policy and generated column. The statements under the search-path test are only the reuse checks (identity, baseline, version). No W1, W2, W2b or W4 statement runs before them.
- **APP-195's jitter and APP-198's `Stale`.** The client composes them as `runView` over `retryOnce`: up to 4 attempts, each with at most one 503 retry. The campaign does not model the stale re-read (MINOR-2).
- **APP-196's `SET CONSTRAINTS` and 0023/0024:**
  - the only deferrable constraints at v24 are 0002's four keys;
  - neither migration has a top-level DML statement (statements classified after stripping comments and bodies);
  - neither touches the four tables;
  - the probe migrated a database holding rows under the deferred keys, through `SET CONSTRAINTS ALL IMMEDIATE`, with exit 0.
- **W2b's routes and W2's limits:**
  - every share POST (`/share`, `/share/add`, `/share/organization`) passes `share_request` → `admit_member_write`, as on `main`;
  - no share route calls a model, so W2's daily cap does not apply;
  - the per-address windows concern sign-in and invitation only.
- **W3's purge and the 0023/0024 tables:** the purge's table set comes from the catalog, so `member_consents` and the new `work_shares` key are handled. Only `stats` misses the new table (NIT-5).
- **The image contexts:**
  - the service image builds in CI;
  - the qualification context admits all 227 compile-time sources;
  - the operator allowlist test passes.

**5. The 0024 preflight answer and the deferred-constraint claim**
- **The preflight answer is correct.** A stored schema that does not parse fails the STORED generated column's table rewrite. That happens inside `apply_migrations`' one transaction. `migrate()` then closes without sending COMMIT, and `__main__` exits 1. `schema_recovery.py:80` accepts exactly that. The qualifier proves it for 22P02 and 22021, and my probe proves the populated success path.
- **Every cited line number** in 0023, 0024, 0002, `runtime.py` and `schema_recovery.py` matches HEAD.
- **The deferred-constraint claim is correct.**

**6. The docs (`f61b5999`)**
- The approver-step text matches `app/admin/src/canvas/actions.ts:171` and `:180`.
- The record's values are consistent across `BETA-PROMOTION.md`, `TRIAL-RUNBOOK.md` §3 and §8, the plan and the record:
  - 22 → 24;
  - snapshot `-v24`;
  - expiry 2026-11-26;
  - `to` contract `88d17cb5…`.
- The renewal arithmetic holds: 2026-12-16 08:00 − 18 h = 2026-12-15 14:00 UTC.
- `8a78bbf0`'s fixture fix is consistent: the Synthetic fixture job (`fixtures`) passes on both PR runs.

## Limits

- No AWS call, no canonical-state read and no beta database read. The `from` side rests on release A's readback. Beta's stored schemas were not read, so the preflight answer rests on the code, the closed grammar and the local proofs.
- I did not re-review the input branches' insides. The interactions above are the ones I looked for, not an exhaustive cross-product.
- `refresh-herd` ran alone, after the other campaigns, at a 1-minute load of 4.7–5.0. That is above the recipe's quiet-host bar of 4, because desktop apps were busy. It is one run, not three. The 26-page measurement was not run.
- The box campaign's first failure is attributed to host load because the rerun passed and the query worker is unchanged. It was not reproduced deliberately.
- One `tenant-foundation` CI run is red (43597356148). I did not read its log, per the task's rule.
- The upgrade probe's seed is synthetic: the APP-008 shapes plus hand-made share rows, with no access codes, reservations or pending admissions. It is evidence about the migration, not about beta's data.
- The §8 dry run used release A's findings as stand-in scanner output. It proves the tool path and the bindings, not any real finding's applicability.
