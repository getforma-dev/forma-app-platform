# APP-210: break-and-restore proofs (2026-11-14)

Branch `ops/app-210-contract-release`, from `main` `140a5a9f`. The tests were written first. On the old code, all 9 new tests of the first pass failed: 6 failures and 10 errors over their subtests. A tenth test, for a policy version moved during execution, came after the implementation and is proved by B12. Then each guard below was broken in the working tree, and the whole promotion suite was run (`python3 -m unittest discover -s ops/promotion -p 'test_*.py'`, 248 tests). The file was restored from its saved bytes, and its sha256 was compared with the value before the break. A scratch script (not committed) applied the breaks one at a time. Every restore was byte-identical. The suite after the last restore: 248 tests, OK.

## The guards

`SchemaLifecycleTests` and `SpecFileTests` are in `ops/promotion/test_schema_release.py`. The counts are those the runner printed, subtests included.

| # | Guard | Where | Break | Result with the break |
| --- | --- | --- | --- | --- |
| B1 | The contract always changes | `schema_release.load` | the `from.deployment_contract_sha256 != to.deployment_contract_sha256` line removed | 3 failures: `test_a_release_is_one_migration_or_one_contract_change` (identical ends, a contract kept by a contract-only release and by a migration) |
| B2 | The version never falls | `schema_release.load` | the `from.database_version <= to.database_version` line removed | 1 failure: the same test (a downgrade from 22 to 20) |
| B3 | The schema manifest changes exactly when the version does | `schema_release.load` | the equivalence removed | 2 failures: the same test (manifest kept while the version moves, manifest moved while it stays) |
| B4 | Only a migration names a snapshot | `Runner.prepare` | `if release and migrates(release):` → `if release:` | 3 failures: the promotion test, the moved-policy test and the restore test (the packet names a snapshot, then execute refuses "schema release changed after preparation") |
| B5 | `preserved.schema` follows `migrates` | `Runner.prepare` review | `schema=not migrating` → `schema=not release` | 8 errors: every contract-only lifecycle test (the summary refuses at preparation) |
| B6 | The sequence follows `migrates` | `Runner.prepare` review | `SCHEMA_SEQUENCE if migrating` → `if release` | 8 errors: the same tests |
| B7 | `same_release` adds the snapshot name only for a migration | `Runner.same_release` | the `migrates` condition removed | 2 failures and 1 error: the promotion, moved-policy and restore tests (execute refuses "schema release changed after preparation") |
| B8 | `same_release` still compares a contract-only release | `Runner.same_release` | `release == expected` → `release == expected or not migrates(release)` | 1 failure: `test_a_contract_only_release_changed_or_expired_after_preparation_cannot_stop` (the reformatted file executed) |
| B9 | The snapshot runs only for a migration | `Runner.execute` | `if migrating:` → `if reviewed:` before `snapshot_schema` | 3 errors: the promotion, moved-policy and restore tests (`KeyError: 'snapshot_identifier'`, before any service stops) |
| B10 | The migration runs only for a migration | `Runner.execute` | `if migrating:` → `if reviewed:` before `migrate_schema` | 1 failure and 1 error: the promotion and moved-policy tests ("no confirmed pre-migration snapshot", services stopped) |
| B11 | A contract-only release is admitted | `Runner.execute` `finish` | `record = migrated or dict(reviewed)` → `record = migrated` | 2 failures: the promotion test (the baseline keeps the `from` contract) and the moved-policy test |
| B12 | The policy must equal `from` before admission | `Runner.execute` `finish` | the `require(... == record['from_version'] ...)` line removed | 2 failures: `test_a_policy_version_moved_during_execution_admits_nothing`, both subtests (migration and contract-only) |
| B13 | `from_version == schema_version <= to_version` | `github.validate_summary` | `<=` → `<` | 1 failure and 9 errors: the summary tests and every contract-only lifecycle test |
| B14 | Equal versions need the spec's two schema digests equal | `github.validate_summary` | the condition removed | 1 failure: `SummaryTests.test_equal_versions_need_one_schema_manifest_in_the_spec` |
| B15 | `preserved.schema` follows the versions | `github.validate_summary` | `migrating = release is not None and from != to` → `release is not None` | 10 errors: the summary tests and every contract-only lifecycle test |
| B16 | The sequence follows the versions | `github.validate_summary` | `if migrating` → `if release is not None` | 10 errors: the same tests |
| B17 | The restore refuses a contract-only release | `schema_recovery.plan` | the `require(schema_release.migrates(release), ...)` line removed | 2 failures: `test_restore_after_failed_migration_refuses_a_contract_only_release` (plan and apply) |

Without B17 the restore still refused, but on its journal check ("restore requires the stopped execution and nothing after the migration"), because a contract-only release journals no `schema-snapshot`. The guard names the right recovery instead.

`maintenance abandon` has no new guard. `test_a_prepared_contract_only_release_abandons_like_a_normal_promotion` covers it as shipped: no RDS call, one change set deleted, `retained_schema_snapshot` null in both receipts, and the baseline and policy unchanged.

## A committed contract-only record (simulated)

`CommittedReleaseTests` and `test_the_committed_release_and_disposition_promote_together` run only against the committed file. Release C's record was simulated at this branch's `HEAD`, uncommitted. `schema-release.json` and the `from` side of the pin both became `{24, 176d68cb…, 0123456789abcdef…}`, and `to` stayed release B's `{24, 176d68cb…, 88d17cb5…}`, which `HEAD`'s tree has.

- Before the test fix: `test_committed_release_applies_only_to_its_reviewed_baseline` failed, `AssertionError: Refused not raised`. It expected `applicable` to refuse at `policy == to_version`, which for a contract-only release is the valid `from`. The other three tests passed, among them the end-to-end promotion of the committed record (the baseline took `88d17cb5…`, and the policy stayed 24).
- After the fix (the wrong policies are `{from+1, from-1, to} - {from}`): all 4 passed.
- The two files were restored from their saved bytes; `shasum -a 256 -c` printed `OK` for both. `schema-release.json` has no diff against `main`.

## Commands and results

- `python3 -m unittest discover -s ops/promotion -p 'test_*.py'`: 238 tests on `main` `140a5a9f`, OK; 248 on this branch, OK.
- `just check py-promotion py-ops board`: ok (248 promotion tests, 40 ops tests, board OK).
- `just check` (after `just setup` in the new worktree): `fmt`, `admin`, `clippy` and `clippy-dev` ok. `test` (cargo) failed to compile: `No space left on device (os error 28)` writing the incremental cache. The machine's disk had 642 MiB free, and this worktree's `target/` held 2.5 GiB. That `target/` was deleted (ignored build output this run created). The four steps that need a full Rust build, `test`, `http-smoke`, `msrv` and `catalog`, were not rerun: the disk cannot hold another one. No Rust file changed on this branch.
- `just check infra py-scripts py-runtime shared-broker py-qualification py-app-scripts py-ops py-promotion py-infra schema-pins contracts board bash-n`: all ok except `py-runtime`. Of its 159 tests, 157 passed, and 2 `setUpClass` errors (`test_admission_operator.DatabaseTests`, `test_provider_invitations.ProviderDatabaseTests`) came from `docker run`: "Docker Desktop is unable to start". This branch changes nothing under `app/runtime`.

## Limits

- Not run here: the Rust `test`, `http-smoke`, `msrv` and `catalog` steps, and the two Docker-backed `py-runtime` classes (see above). CI on the pushed branch runs them.
- Fake AWS only. No AWS call was made and no canonical state was read. That the live baseline will hold release B's `to` after release B rests on release B's own execution, which is in progress.
- The scratch script that applied the breaks is not committed. The table records each break's exact change.
- The simulated committed record used a synthetic `from` contract. Release C's real record needs its own evidence record and pin, and its own review.
