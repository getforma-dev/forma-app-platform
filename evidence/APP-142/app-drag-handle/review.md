# Independent source review: app-page move handle

Reviewed on 2026-11-02 UTC by an independent Codex reviewer against base `7d81dbe5c80b4df162c648f81978613ac7ae6eea` on `pivot/app-drag-handle`. **No blocking findings remain in the reviewed source.** This is a source review, not a hosted deployment or browser qualification receipt.

## Exact reviewed source

| File | SHA-256 |
| --- | --- |
| `app/admin/src/canvas/unite.ts` | `71ee7eb717ccba694783a389e1c11916b186e57cf5955500cd2291b56533055a` |
| `app/admin/src/styles/canvas.css` | `69316f40fa7cbb49b648e4568f3d87a73758acf227a0f2ee5da70c223be1a80e` |

The change enables the existing canvas-item chrome on a united app, removes outer clipping from both sizing code and CSS, and synchronizes its accessible name after a successful work rename. No renderer, authorization, API, dependency, schema or saved-layout format changes are included.

## Findings and repairs

1. **Repaired blocker: inline clipping still hid the rail.** The first patch removed the stylesheet's outer `overflow: hidden`, but `applySize()` also set the same property inline. Every first open, restore and resize uses this function. The implementer removed that assignment; the final code at `unite.ts:309` no longer clips the item. Inner `.cv-unite.is-sized` clipping and grid scrolling remain at `styles/canvas.css:1863`–`1869`.
2. **Repaired accessibility inconsistency: stale Move label after rename.** The rename handler previously updated only the page heading and inner label. With chrome enabled, its `Move <name>` label could keep the old name. The final handler at `unite.ts:579`–`581` also updates the item name, item label and Move label using text attributes. It retains the existing exact app/work guard.

Both repairs were re-read in the final source identified above.

## Contract and lifecycle assessment

- `createCanvasItem()` adds chrome as an absolutely positioned sibling of the existing page content. The unchanged vendor CSS keeps the outer item overflow visible and the chrome out of layout flow; it does not add to the page's measured height. Only the Move button receives pointer events in that overlay. The app's inner page still clips and its grid still scrolls; its resize corner remains within the page.
- The same adaptive placement code selects a visible edge and accounts for camera zoom. `WidgetCanvas.#bindItemDrag()` binds the new handle through the existing pointer and keyboard path. Existing interactive-control exclusions still protect the page header's controls. Movement uses the whole mounted item, not individual cards.
- Pointer completion and keyboard movement reach the existing `onCommit`/`onChange` callbacks, then `board.saveSoon()`, `onLayoutSaved(save)` and member-scoped `saveUnited()`. Frame coordinates and size retain the existing browser-local format. This change introduces no server-side layout persistence.
- Removing/fanning out a frame still calls `removeItem()`, which ends an active gesture and aborts that item's drag listeners. Mounting, restoring and reuniting therefore use the existing listener cleanup. Focus still moves the actual card node and returns it through its placeholder; fan-out still drops those same cards at their saved positions. This change does not reconstruct forms or reset receipts, values or idempotency state.
- APP-142's preserved card identity, focus/fan-out distinction, browser-local sizing and exact-work sharing contracts are unchanged. No new authorization surface or network operation was added.

## Verification and limits

The reviewer read the final diff, APP-142, `unite.ts`, `focus.ts`, the canvas item/chrome/placement and drag implementation, board persistence and layout helpers, and the applicable CSS. `git diff --check` passed, and `shasum -a 256` produced the source bindings above.

The implementation owner is running the required full check and a final affected frontend rebuild; another reviewer owns actual browser qualification on the isolated stand. Those results must be recorded separately. This review does not claim a completed browser drag, draft-retention interaction, CI run, merge or AWS rollout, and does not close APP-142's older predecessor/phase gates.
