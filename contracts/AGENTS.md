# Contracts

## What this is

The locked cross-language contract edition: schemas, vectors and the library check. The current edition is named in [README.md](README.md). These files are not application admission or storage.

## Authoritative sources

- [README.md](README.md) and `implementation-lock.json`
- [ADR-002](../docs/architecture/ADR-002-CONTRACT-EDITION.md)
- [Framework baseline](../docs/architecture/FRAMEWORK-BASELINE.md)

## Verify a change here

`just check contracts`

## Never

Do not change a published edition's fixtures or digests in place. Do not treat a schema change as permission to execute model output. The DuckDB pin used by the library check is `spikes/APP-005/requirements.lock`, installed into `contracts/.venv`.

## Read next

- [ADR-002](../docs/architecture/ADR-002-CONTRACT-EDITION.md)
- [Spikes](../spikes/AGENTS.md)
- [Developer guide](../docs/DEVELOPMENT.md)
