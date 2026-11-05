# Canvas

## What this is

The TypeScript canvas. `build.ts` builds it with `--ssr --strict`. The Rust service serves the result. Four islands are registered, each with `activateIslands`: `IntentBar` (`src/canvas/app.tsx`), `SharedControls` (`src/shared/app.tsx`), `ConnectedTools` (`src/shared/canvas.tsx`) and `DocumentStatus` (`src/work/app.tsx`).

## Authoritative sources

- `src/` and `build.ts` in this folder.
- [Theme roles](../../docs/architecture/THEME-ROLES.md) and [brand](../../docs/architecture/FORMA-BRAND.md).
- [Canvas rules](../../docs/pitch/CANVAS-RULES.md), read with [ADR-018](../../docs/architecture/ADR-018-WHOLE-APP-VIEWS.md).

## Verify a change here

`just check admin`

For a canvas behavior change, also run `just campaign canvas`.

## Never

Do not hand-edit `dist/`, FMIR or the route manifest. Register every new island explicitly. Adopting the optional router needs its own measured decision ([ADR-001](../../docs/architecture/ADR-001-RELEASED-BASELINE.md), root AGENTS.md). Compute flags and derived rows in Rust and pass them as props.

## Read next

- [Application](../AGENTS.md)
- [Module map](../../docs/architecture/MODULE-MAP.md)
- [Developer guide](../../docs/DEVELOPMENT.md)
