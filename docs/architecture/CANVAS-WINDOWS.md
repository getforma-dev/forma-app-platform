# App windows and file cards

Local design decision, 2026-11-02, following Victor's review of the uploaded-file and app shells. This extends [ADR-018](ADR-018-WHOLE-APP-VIEWS.md) without changing whole-app ownership. It is not a hosted release.

## Window structure

An app remains one canvas object. Its trusted shell has a compact title bar, a view/action toolbar, a scrolling component body and a status footer. File provenance is a compact disclosure above the footer. The body keeps existing components, actions, query receipts and admitted presentation layout/theme. Fixed SVG icons have accessible names; model output never becomes shell markup.

The user-facing views are **Card**, **Browse** and **Open**. Browse replaces the misleading app label Stack; stored `stack` values and actual independent stacks are unchanged. Card shows app identity/description and an Open action. Browse shows one retained view at a time. Open exposes the full component layout and resize control.

The title bar's close, minimize and open controls remove the placement, switch to Card, and switch to Open. Color is supplementary: controls also have persistent symbols, tooltips and accessible names. Removing a placement never deletes its app or data. Settings, Focus and the new-tab page remain in the title bar; Summary, Ask and authorized Share stay in the toolbar. Expanding preserves the saved position and centers the camera when needed to keep controls reachable.

The uploaded-file card uses the same border and header/body/footer hierarchy: a small document glyph, filename, type/size, Open spreadsheet and Make an app, and integrated exact-file Ask. It makes no data request merely to decorate its face. Double-click, keyboard opening, dragging, Library restoration and in-place upgrade remain intact.

## Attached Summary

Summary is an app-owned companion, not a standalone canvas card or modal. It opens beside the window and moves/removes with that same item. Narrow layouts put it below the window. Close/reopen do not change identity, drafts or saved Open size.

Opening/reopening reads the existing authorized explanation. Generate/Regenerate is explicit; placement and view switching do not generate summaries or start polling. The existing backend supports explanations for apps derived from files. Other apps receive an unavailable message. Cited values retain receipt navigation. Closing/removing aborts pending reads and guards late navigation so they cannot resurrect a removed app. No model, provenance or authorization contract changed.

## References and performance

The [VS Code interface](https://code.visualstudio.com/docs/editing/getting-started/userinterface) separates controls, working content, status information and supporting panels. Forma adapts that hierarchy to a canvas object. [Apple's toolbar guidance](https://developer.apple.com/design/human-interface-guidelines/toolbars) informs compact action grouping and the persistent title. These are structural references, not reproductions of either application's design.

CSS layout, existing theme tokens and retained DOM avoid additional dependencies, raster assets, background model calls or periodic work. Published application colors remain local to the app; outside chrome retains the canvas theme. [Local qualification](../../evidence/APP-168/window-shell-2026-11-02/README.md) records checks and limits.
