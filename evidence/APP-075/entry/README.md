# APP-075 B1: managed qualification entry

Source: [managed_qualification.rs](../../../app/src/storage/managed_qualification.rs) (sha256 `e8a17662…` at `d6986c2`) and [its tests](../../../app/src/storage/managed_qualification/tests.rs) (`f32073eb…`). The starting point was the frozen draft at `57478ae9…`. After the handoff the catalog include moved to v4 during APP-073, `f44ce72` moved the seed pins to edition `impl-2026-10-28.1`, `da3d203` applied the independent review's fixes (`516b972c…`), and the `3f918fb` merge moved the `setup.json` and `tenant_seed.py` pins and bound the APP-010 sources (first recorded at `5023eb9` as `4f17b072…`/`c1ad2030…`).

## Repairs to the draft

| Draft gap (ticket, handoff review) | Now |
| --- | --- |
| Cleanup was reported `true` for refusals before any store existed | `cleanup_applicable:false` (and `cleanup_confirmed:false`) before store construction. `cleanup_confirmed:true` requires a finished or abort-acknowledged case task, a successful `Stores::close` and `pool::slots().0 == 0`. `passed:true` requires confirmed cleanup. |
| `issued_at` could not lead the clock | Same +5 s tolerance as `config.rs` |
| Only 4 s had to remain | At least 60 s must remain. Cases stop at expiry −4 s, the abort must be acknowledged by −3 s, close must finish by −1 s, and the watchdog exits with 124 at expiry. |
| Non-case panics were reported as `case_refused` | `entry_panic`, with cleanup unconfirmed |
| The stdout contract was implicit | Documented in the module docs. The result line starts on its own line, because libtest has already written `test … ... `. |
| Fixture pin covered only `setup.json` | The `SEED_INPUTS` table pins `setup.json`, edition `impl-2026-10-28.1`'s `northstar.json` and `spend-review.json` (the files `tenant_seed.make_bundles` reads), `tenant_seed.py` itself and `contracts/validate.py` (its `digest`), plus `database_version`. A test checks each declared hash against the repository and the edition folder against `tenant_seed.py`; `local_harness.py` and `check_image.py` read the same table. |
| Source bundle covered 11 storage files | Covers all 42 compile-time inputs of the library test executable: 29 Rust sources, the Cargo files, the APP-006 crate, and every `include_str!`/`include_bytes!` target outside `app/migrations/` (the RDS CA and 9 contract files). A test fails if a new `.rs` file or embedded file is not bound. |
| A pass counted any checks | `passed:true` needs exactly the six named `CHECKS`, in order, each passed; otherwise `case_refused` |
| An entry panic dropped the watchdog | Once the manifest is validated, the panic fallback re-arms the watchdog at the original deadline until the result line is written |
| An oversized report always claimed cleanup applicable and unconfirmed | The `report_refused` fallback copies the report's boolean `cleanup_applicable`/`cleanup_confirmed` |
| `catalog_v2_sha256` naming | `catalog_sha256`, `database_version` |

The entry still calls only `Settings::new(CREDENTIALS, REGISTRY, CA, HOST)`. A source test forbids `Settings::fixture`, `::fixture(`, `NoTls`, `dangerous`, `SslMode::Disable/Prefer` and `set_var` in the entry. `Settings::new` refuses `localhost`, `127.0.0.1` and any host outside `.us-east-1.rds.amazonaws.com`.

## Tests

`cargo +1.93.1 test --locked --lib storage::managed_qualification` (at `d6986c2`): 20 pass and 3 are ignored (the entry itself, the child helper and the Docker campaign). See [storage-tests.txt](../checks/storage-tests.txt).

- **Closed manifest:** unknown or duplicate keys at the top level and in `pins`, missing fields, wrong types, trailing data, empty input, non-UTF-8, more than 8,192 bytes, three organizations.
- **Selector, arming, UID:** missing, extra, reordered and renamed arguments and `--test-threads=2` all refuse without reading files. Arming refuses absent, `app-008`, `APP-008 `, `APP-075` and empty. UID refuses 0 and any other value. Scope, `schema_version`, `armed:false` and v7 or uppercase run IDs refuse.
- **Fixed target and pins:** another RDS host, `localhost`, an uppercase host, port 5433, database `postgres`, another resource ID, swapped or duplicated organizations, the registry hash of another run, and an uppercase or short CA hash all refuse. Each of the eight pins mutated on its own refuses.
- **Files:** pretty-printed or newline-suffixed registry bytes; a CA with changed bytes, mode 0664 or 0646, or replaced by a symlink; a credential file naming `localhost`, empty, missing or mode 0644. For the manifest: another owner (via the expected UID), modes 0640/0604/0400/0700, parent directory 0750/0755/0500, a hard link, a symlink, a directory, an oversized file, a FIFO (refused without blocking), a relative path, `..`, and a symlinked ancestor.
- **Deadline:** span 601 s; remaining time under 60 s; `issued_at` more than 5 s ahead; expired or empty intervals. The monotonic deadline comes from the sample taken before files are read.
- **Report:** a closed key set, error codes only, `checks` empty unless passed, one result line, a bound of 8,192 bytes, and an oversized report replaced by `report_refused` (keeping only its boolean cleanup state), never truncated. Empty, five, seven, renamed, failed, extended or reordered checks never pass.
- **`supervise` without a database:** success, a panic and a cooperative timeout each give confirmed cleanup. A case that blocks its worker so the abort is never acknowledged gives `cleanup_unconfirmed` and `passed:false`. Settings refused or a missing registry gives not-applicable. A replaced input after the cases voids the pass.
- **Process boundary:** the test executable re-runs itself. A case panic carrying a sentinel string leaves no trace of the sentinel or of `panicked` on stdout or stderr and reports `case_refused` with confirmed cleanup. An entry panic reports `entry_panic` with cleanup unconfirmed. The watchdog exits with 124 at a 1 s deadline and prints no result line. An entry panic after the deadline was recorded, followed by a stalled write, still ends in 124 at that deadline without a result.
- **Cross-language registry vector:** the same bytes are asserted in the supervisor tests.

## Real transports (Docker)

`managed_cleanup_with_real_transports` uses the existing local fixture (PostgreSQL 16.13, TLS, migrations, seed) through `supervise`. Receipt: [cleanup-campaign.txt](cleanup-campaign.txt).

| Case | Error | Cleanup | Transport permits | Application-role backends (`pg_stat_activity`) |
| --- | --- | --- | --- | --- |
| The six production checks (exactly the named `CHECKS`) | none | confirmed | 0 | 0 |
| Panic while holding an open data transaction and a workspace lease | `case_refused` | confirmed | 0 | 0 |
| Timeout while a `TestHooks` `begin` barrier holds a transaction (8 s deadline, cut at 4,004 ms) | `original_deadline_reached` | confirmed | 0 | 0 |

Rerun at `d6986c2` on PostgreSQL 16.13 with TLS (migrations 0001–0005). Earlier runs: `da3d203` (cut at 4,001 ms) and `5023eb9`, in Git history.

This campaign uses `Settings::fixture` to reach the local fixture's port. It tests the cleanup machinery around the cases, not the entry's target binding, which the [local positive](../local-positive/README.md) covers with `Settings::new`. CI now runs this campaign in `tenant-foundation.yml`.
