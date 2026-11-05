# Parser

## What this is

A separate sealed Rust and WASM workspace. The host is `host/` and the guest is `guest/`. The application reaches it through `imports` and `supervise`. It is not a member of the root Cargo workspace.

## Authoritative sources

- `Cargo.toml` and `Cargo.lock` in this folder.
- [ADR-009](../docs/architecture/ADR-009-PHASE-2-DATA-PLANE.md) for the process boundary.
- [Developer guide](../docs/DEVELOPMENT.md), section "Parser and release images".

## Verify a change here

There is no `just check` step for this workspace. Build with `just parser`. The probe is `just campaign parser-sandbox`.

## Never

Do not fold this workspace into the root workspace, lockfile or toolchain. Do not treat `just parser` output as the image that promotion ships. The image builds qualify their own parser.

## Read next

- [Application](../app/AGENTS.md)
- [Module map](../docs/architecture/MODULE-MAP.md)
- [Developer guide](../docs/DEVELOPMENT.md)
