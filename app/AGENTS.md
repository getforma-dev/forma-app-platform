# Application

## What this is

The running Forma application: the Rust service in `src/`, the TypeScript canvas in `admin/`, the versioned schema in `migrations/`, and the confined Python children in `runtime/`.

## Authoritative sources

- [Module map](../docs/architecture/MODULE-MAP.md) for Rust modules and dependency rules.
- [Developer guide](../docs/DEVELOPMENT.md) for commands, instances and delivery.
- `Cargo.toml` in this folder. The root workspace also includes `spikes/APP-006`.

## Verify a change here

`just check fmt clippy clippy-dev test http-smoke msrv schema-pins py-runtime py-app-scripts py-qualification admin`

Name a campaign from `just campaigns` when the change has one.

## Never

Do not hand-edit FMIR, admin bundles, or catalog hashes. Model output is admitted data, not code, SQL, markup or a grant. Do not restart or reset a dev instance someone is using.

## Read next

- [Canvas](admin/AGENTS.md)
- [Migrations](migrations/AGENTS.md)
- [Module map](../docs/architecture/MODULE-MAP.md)
- [Docs index](../docs/README.md)
