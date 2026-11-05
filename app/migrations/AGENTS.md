# Migrations

## What this is

Versioned schema for the application database. Each `NNNN_*.sql` file is one version. The matching `catalog-vN.json` pins are checked with the manifest.

## Authoritative sources

- The SQL files and `catalog-v*.json` pins in this folder.
- `scripts/check-schema-pins.py` for the pins that must move together.
- [ADR-004](../../docs/architecture/ADR-004-TENANT-TRANSACTIONS.md) and [ADR-007](../../docs/architecture/ADR-007-PHASE-1-WRITE-AUTHORITY.md).

## Verify a change here

`just check schema-pins`

A behavior change also needs the tenant campaign that owns it (`just campaigns`).

## Never

Do not edit an applied migration in place. Do not bump a version without the catalog pin, the runtime schema constant and the other pins `check-schema-pins` names. Do not treat a local pin update as a database upgrade on beta.

## Read next

- [Application](../AGENTS.md)
- [Developer guide](../../docs/DEVELOPMENT.md)
- [Promotion](../../docs/pitch/BETA-PROMOTION.md)
