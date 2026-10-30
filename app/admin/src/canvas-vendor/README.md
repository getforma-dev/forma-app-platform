# Vendored canvas runtime (ADR-011, APP-092)

Source: `getforma-dev/forma-genui-runtime` (Victor's own repository; its `package.json` is `private`, license `UNLICENSED`), commit `93c3871` ("fix: keep routing byte decoding lint-clean").

Copied unchanged from that commit:

- `src/canvas/*.ts` → this folder: the camera, world coordinates, pan/zoom, drag, focus/fit, navigator and keyboard navigation (`widget-canvas.ts`, `canvas-surface.ts`, `canvas-item.ts`, `keyboard-shortcuts.ts`, `widget-chrome*.ts`, `canvas-capacity.ts`, `runtime-adapter.ts`, `index.ts`).
- `styles/canvas.css` → `canvas.css` (built as `canvas-vendor.css`; themed by `../styles/canvas.css`).

Two changes: `widget-canvas.ts` gains one public method, `moveItemTo(item, x, y)` (the private `#moveItem` plus a navigator render), so the board can re-flow its default layout as results arrive; and `runtime-adapter.ts` imports `WidgetVirtualizationPolicy` from the local `contracts.ts` (a one-type excerpt of the runtime's `src/contracts.ts`) instead of `../contracts`.

APP-104 local changes in `widget-canvas.ts` (DS-10), each marked "Local change (APP-104)" in the code:

- `#moveItem` asks for a visibility pass through `#requestVisibilityUpdate`, so a batch of moves inside `deferDerivedUpdates` runs one pass, not one per item (the board's re-flow reads every size first, then writes).
- Figma's input mapping: a plain wheel or two-finger scroll pans (Shift: sideways), Ctrl/Cmd + wheel and a trackpad pinch zoom toward the cursor (never a plain wheel); a scrollable region inside a card that can still scroll keeps the wheel (`scrollsNatively`). Space pans also when nothing else holds focus.
- The hand tool: `setPanMode(on)` makes a primary drag anywhere pan (`is-hand-tool`), and suspends selection and dragging.
- A card's own header (`[data-canvas-drag-handle]`) moves it, like the chrome's drag handle (its interactive controls keep their clicks).
- Public `zoomAt(factor, x, y)`, `panBy(dx, dy)` and `cancelGestures()` (the pitch's pinch and zoom keys, `../canvas/controls.ts`).

Not vendored: the runtime's Forma 2.x widget host (`src/forma`), its contracts, persistence and vector routing. The pitch canvas mounts only server-rendered cards (`../canvas/board.ts`); it has no runtime host, so the canvas runs with `NO_RUNTIME_ADAPTER`.

Design D10 (one native canvas island, geometry and keyboard from genui-runtime) is the decision this follows. The look and interaction of the page (`../styles/canvas.css`, `../canvas/app.tsx`) adapt Forma Generative UI's canvas workspace (`forma-generative-ui` at `a108203`, `client/css/shell.css` and `client/html/shell.html`): top bar, bottom dock with the intent bar, materialize motion, empty state; not its planner, widget host or widgets. Re-vendoring is a copy of the same files from a newer commit, recorded here.
