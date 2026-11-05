# Canvas

## What this is

The TypeScript canvas. `build.ts` builds it with `--ssr --strict`. The Rust service serves the result. One registered island is `IntentBar`.

## Authoritative sources

- `src/` and `build.ts` in this folder.
- [Theme roles](../../docs/architecture/THEME-ROLES.md) and [brand](../../docs/architecture/FORMA-BRAND.md).
- [Canvas rules](../../docs/pitch/CANVAS-RULES.md), read with [ADR-018](../../docs/architecture/ADR-018-WHOLE-APP-VIEWS.md).

## Verify a change here

`just check admin`

For a canvas behavior change, also run `just campaign canvas`.

## Never

Do not hand-edit `dist/`, FMIR or the route manifest. Do not add a second island without the decision the module map already requires for a router. Compute flags and derived rows in Rust and pass them as props.

## Read next

- [Application](../AGENTS.md)
- [Module map](../../docs/architecture/MODULE-MAP.md)
- [Developer guide](../../docs/DEVELOPMENT.md)
