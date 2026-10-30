# Vendored canvas runtime (ADR-011, APP-092)

Source: `getforma-dev/forma-genui-runtime` (Victor's own repository; its `package.json` is `private`, license `UNLICENSED`), commit `93c3871` ("fix: keep routing byte decoding lint-clean").

Copied unchanged from that commit:

- `src/canvas/*.ts` → this folder: the camera, world coordinates, pan/zoom, drag, focus/fit, navigator and keyboard navigation (`widget-canvas.ts`, `canvas-surface.ts`, `canvas-item.ts`, `keyboard-shortcuts.ts`, `widget-chrome*.ts`, `canvas-capacity.ts`, `runtime-adapter.ts`, `index.ts`).
- `styles/canvas.css` → `canvas.css` (built as `canvas-vendor.css`; themed by `../styles/canvas.css`).

Two changes: `widget-canvas.ts` gains one public method, `moveItemTo(item, x, y)` (the private `#moveItem` plus a navigator render), so the board can re-flow its default layout as results arrive; and `runtime-adapter.ts` imports `WidgetVirtualizationPolicy` from the local `contracts.ts` (a one-type excerpt of the runtime's `src/contracts.ts`) instead of `../contracts`.

Not vendored: the runtime's Forma 2.x widget host (`src/forma`), its contracts, persistence and vector routing. The pitch canvas mounts only server-rendered cards (`../canvas/board.ts`); it has no runtime host, so the canvas runs with `NO_RUNTIME_ADAPTER`.

Design D10 (one native canvas island, geometry and keyboard from genui-runtime) is the decision this follows. The look and interaction of the page (`../styles/canvas.css`, `../canvas/app.tsx`) adapt Forma Generative UI's canvas workspace (`forma-generative-ui` at `a108203`, `client/css/shell.css` and `client/html/shell.html`): top bar, bottom dock with the intent bar, materialize motion, empty state; not its planner, widget host or widgets. Re-vendoring is a copy of the same files from a newer commit, recorded here.
