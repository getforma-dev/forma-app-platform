# Window shell local qualification

2026-11-02, `pivot/local-ui-iteration`, preview 4001 only. [Design decision](../../../docs/architecture/CANVAS-WINDOWS.md). No push, PR, migration or deployment.

## Actual browser checks

- The redesigned uploaded-file card shows its filename/type/size, explicit Open/Make an app and exact-file Ask field. Double-click opened the original modal; the local sample's five records/five columns remained readable.
- Card → Browse → Open retained one item and the exact DOM node/value of an unsent filter input. Existing details stayed open, and source provenance remained accessible.
- Settings opened the existing presentation editor. Window close/minimize/open use distinct labeled symbols; Focus, new-tab access, Ask and Share retain their existing action paths and permission controls.
- Summary opened beside the app within the same item. A local backend Generate returned HTTP 200, one finding and three receipt-linked values. Close/reopen issued GET only and restored the finding without regenerating. A cited value opened its matching component and receipt in Focus. No independent Summary item was created.
- Double-clicking Summary text while minimized left the app in Card view. Browse advanced to the next retained view. Removing the app removed both its window and companion; Library Add restored one app, and the original file card remained.
- At 390 × 844, the companion appeared below the app; both regions had zero horizontal overflow and remained within the canvas viewport. Native scroll offsets caused by focusing offscreen controls are cleared when revealing an expanded window, preserving camera ownership.
- Light and dark app palettes were inspected without publishing theme changes. Window/companion headings and backgrounds remained paired: light `rgb(22,32,51)` / `rgb(248,250,252)`; dark `rgb(241,245,249)` / `rgb(25,35,44)`.

## Checks and review

- `npm --prefix app/admin run typecheck`: passed.
- `npm --prefix app/admin test`: 203 passed, including companion lifecycle, late receipt navigation, retained drafts/nodes, saved Open size and clamped companion resizing.
- Focused file-card tests cover exact-file Ask scope, ownership, retry/cancellation and in-place upgrade.
- `just check admin test http-smoke`: all three groups passed, including the production frontend build, native Rust library/SSR/doctests and HTTP security smoke.
- Production route budgets passed: canvas JavaScript 197,369 / 373,760 bytes; canvas Forma runtime 25,913 / 28,420 bytes (reported compressed sizes). No dependency or lockfile change.
- Independent review found and repaired grid specificity, duplicate IDs, a clamped-resize dead zone, nested Shape header lookup and late receipt-navigation races. Follow-up review found no remaining blockers.

Uploaded contents and screenshots containing their values are not committed. Local screenshots under `/tmp/forma-window-*.png` and `/tmp/forma-file-card-design.png` were visually inspected. Full check log: `/tmp/forma-window-checks.log`. Parser/import/model contracts are unchanged. Summary availability follows the existing file-derived-app backend; unsupported apps get an honest message.
