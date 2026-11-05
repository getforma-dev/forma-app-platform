# Spikes

## What this is

`APP-006` is the production auth verifier (`genui-auth-verifier`) and a member of the root Cargo workspace. The app's `auth` module links it. `APP-001`, `APP-003`, `APP-004` and `APP-005` are measurement spikes whose code is not changed casually, but CI still runs them: `baseline.yml` (APP-001), `canvas.yml` (APP-003), `parser.yml` and `fixtures.yml` (APP-004, also the parser release qualification suite) and `query.yml` (APP-005). `APP-005/requirements.lock` is the DuckDB pin the contracts check installs.

## Authoritative sources

- The root `Cargo.toml` workspace members and excludes.
- [spikes/APP-006/README.md](APP-006/README.md)
- [ADR-001](../docs/architecture/ADR-001-RELEASED-BASELINE.md) for the published Forma pin those spikes measured.

## Verify a change here

`just check clippy` and `just check test` build the app, which depends on `APP-006`. No `just check` step rebuilds the other spikes; the CI workflows above do, so a change there must keep them green.

## Never

Do not treat a frozen spike as the application, and do not move `APP-006` out of the root workspace without the workspace change that owns it. Do not copy a spike result forward as new acceptance evidence.

## Read next

- [Application](../app/AGENTS.md)
- [Contracts](../contracts/README.md)
- [Framework baseline](../docs/architecture/FRAMEWORK-BASELINE.md)
