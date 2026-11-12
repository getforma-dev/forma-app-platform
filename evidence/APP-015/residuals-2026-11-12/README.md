# APP-088 residuals, 2026-11-12 (APP-015 preparation)

This is the dated residuals record that [APP-088](../../../Tickets/2-in-progress/APP-088.md) requires. The decisions are in [ADR-009-AMENDMENTS.md](../../../docs/architecture/ADR-009-AMENDMENTS.md); the re-review's dispositions are appended to [adr-009-review.md](../adr-009-review.md). This directory holds the run that closes the [contract-edition review](../contract-edition/independent-review.md)'s last limitation: "The Docker-backed native campaign that uses the seed was not run." It is a local synthetic qualification, not managed acceptance and not a CI pass.

## Native campaign through the edition-one seed

- **Tree:** `main` at `5695869` in the worktree `forma-app-088` (branch `app088/adr009-residuals`), before any APP-088 commit. The current contract edition is `impl-2026-11-01.2`. No file the campaign reads changed afterwards; APP-088 changes only Markdown in `docs/`, `Tickets/` and `evidence/`.
- **Seed:** the campaign's fixture applies every migration (manifest `database_version` 22) and then runs `tenant_seed.seed(c, make_bundles())` (`app/src/storage/qualification.rs:329`). `make_bundles` reads the retained edition-one fixtures:
  - `contracts/fixtures/impl-2026-10-28.1/northstar.json`, SHA-256 `45639ea3…dd592`;
  - `contracts/fixtures/impl-2026-10-28.1/spend-review.json`, SHA-256 `4a4581f5…a5650`.

  Both equal the identities `contracts/lock.py` retains for that edition. `app/runtime/tenant_seed.py` is SHA-256 `b5c5770d…0c90c`.
- **Environment:** macOS (Darwin 25.1.0), Docker 29.8.2, cargo 1.93.1, Python 3.11.14 venvs created in this worktree by `just setup`. The campaign's PostgreSQL container was `postgres@sha256:5db6dac2…ef83a`, server 16.15.

### Commands and results

1. `just setup`. It created `app/runtime/.venv`, `contracts/.venv`, `app/operator/.venv` and `spikes/APP-005/.venv` from the hashed locks, ran `npm ci` in `app/admin` and `infra`, and reported Docker reachable.
2. `just campaign postgres` (the catalog's `storage::qualification::native_postgres_campaign`). **Failed to compile:** 30 errors E0277, ``the trait bound `…::Assets: Embed` is not satisfied``. In a fresh worktree `app/admin/dist`, the RustEmbed folder that the test campaigns embed, does not exist yet. Nothing ran. Disposition: build the admin assets, then rerun.
3. `just check admin` (`npm --prefix app/admin run typecheck && npm --prefix app/admin test && npm --prefix app/admin run build`): `ok admin (22s)`.
4. `just campaign postgres`, run again, which executes `env GENUI_NATIVE_REPORT=<checkout>/.cache/reports/postgres.json cargo test --locked -p forma-app-platform --lib storage::qualification::native_postgres_campaign -- --ignored --exact --nocapture`. Result:

   ```
   test storage::qualification::native_postgres_campaign ... ok
   test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 438 filtered out; finished in 58.69s
   ```

   Exit 0, 2 min 41 s wall time including the build.

The report is retained as [native-postgres-campaign.json](native-postgres-campaign.json), SHA-256 `db25d5a02a08235a2f3bcc5734e1dbc0dc069afca12c1797601b2d4c37f04036`. It has 36 checks, all `passed: true`, with `managed_acceptance: false`. Its own limits are: synthetic local password authentication is not RDS IAM; fixture identities do not establish APP-006/009/073/074 readiness; and initial application access is read-only.

## Contract lock

`contracts/.venv/bin/python contracts/lock.py check` passed before and after the APP-088 edits: "Contract edition impl-2026-11-01.2 lock matches 61 artifacts, 13 artifacts of 3 retained editions and both frozen source identities". APP-088 changes nothing under `contracts/`, `app/migrations/`, `parser/` or `infra/src/`, and no Dockerfile or dockerignore.

## Repository checks

`just check` ran on the edited tree in this worktree and passed all 21 steps in 15 min 9 s: fmt, admin, clippy, clippy-dev, test, http-smoke, msrv, catalog, infra, py-scripts, py-runtime, shared-broker, py-qualification, py-app-scripts, py-ops, py-promotion, py-infra, schema-pins, contracts, board and bash-n. The board was rendered again afterwards for one Log wording fix, and `python3 Tickets/_tools/board.py check` passed.

## Limits

- One local run on one machine. CI runs this campaign in `tenant-foundation`. This run is not a CI result.
- The campaign proves that the edition-one seed still loads and qualifies on the current tree. It does not exercise jobs, imports or any edition-two document at runtime; none of them exists yet.
