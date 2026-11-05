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

## Migrations

`migrations/` holds the versioned schema: each `NNNN_*.sql` file is one version, with its `catalog-vN.json` pin and the manifest. [ADR-004](../docs/architecture/ADR-004-TENANT-TRANSACTIONS.md) and [ADR-007](../docs/architecture/ADR-007-PHASE-1-WRITE-AUTHORITY.md) govern it. Verify with `just check schema-pins` and the tenant campaign that owns the behavior. Do not edit an applied migration in place. Do not bump a version without the catalog pin, the runtime schema constant and the other pins `scripts/check-schema-pins.py` names. A schema change needs a separately reviewed operator release ([promotion](../docs/pitch/BETA-PROMOTION.md)); a local pin update is not a beta upgrade.

## Never

Do not add any file to `migrations/`, `../parser/` or `../contracts/` that is not a release input (see the root AGENTS.md). Do not hand-edit FMIR, admin bundles, or catalog hashes. Model output is admitted data, not code, SQL, markup or a grant. Do not restart or reset a dev instance someone is using.

## Read next

- [Canvas](admin/AGENTS.md)
- [Root guide: hashed folders](../AGENTS.md#hashed-folders)
- [Module map](../docs/architecture/MODULE-MAP.md)
- [Docs index](../docs/README.md)
