# App windows and file cards

Local design decision, 2026-11-02, following Victor's review of the uploaded-file and app shells. This extends [ADR-018](ADR-018-WHOLE-APP-VIEWS.md) without changing whole-app ownership. It is not a hosted release.

## Window structure

An app remains one canvas object. Its trusted shell has a compact title bar, a view/action toolbar, a scrolling component body and a status footer. File provenance is summarized in the footer; originals are available in the attached Sources view. The body keeps existing components, actions, query receipts and admitted presentation layout/theme. Fixed SVG icons have accessible names; model output never becomes shell markup.

The user-facing views are **Card**, **Browse** and **Open**. Browse replaces the misleading app label Stack; stored `stack` values and actual independent stacks are unchanged. Card shows app identity/description and an Open action. Browse shows one retained view at a time. Open exposes the full component layout and resize control.

The title bar's close control removes the placement. Card/Browse/Open selects the view without duplicate title-bar controls; every icon has a tooltip and accessible name. Removing a placement never deletes its app or data. Settings, Focus and the new-tab page remain in the title bar; the App assistant and authorized Share stay in the toolbar. Expanding preserves the saved position and centers the camera when needed to keep controls reachable.

The uploaded-file card uses the same border and header/body/footer hierarchy: a small document glyph, filename, type/size, Open spreadsheet and Make an app, and integrated exact-file Ask. It makes no data request merely to decorate its face. Double-click, keyboard opening, dragging, Library restoration and in-place upgrade remain intact.

## Attached Summary

Summary is an app-owned companion, not a standalone canvas card or modal. It opens beside the window and moves/removes with that same item. Narrow layouts put it below the window. Close/reopen do not change identity, drafts or saved Open size.

Opening/reopening reads the existing authorized explanation. Generate/Regenerate is explicit; placement and view switching do not generate summaries or start polling. The existing backend supports explanations for apps derived from files. Other apps receive an unavailable message. Cited values open their app component in Focus; their receipt metadata stays retained. Closing/removing aborts pending reads and guards late navigation so they cannot resurrect a removed app. No model, provenance or authorization contract changed.

## References and performance

The [VS Code interface](https://code.visualstudio.com/docs/editing/getting-started/userinterface) separates controls, working content, status information and supporting panels. Forma adapts that hierarchy to a canvas object. [Apple's toolbar guidance](https://developer.apple.com/design/human-interface-guidelines/toolbars) informs compact action grouping and the persistent title. These are structural references, not reproductions of either application's design.

CSS layout, existing theme tokens and retained DOM avoid additional dependencies, raster assets, background model calls or periodic work. Published application colors remain local to the app; outside chrome retains the canvas theme. [Local qualification](../../evidence/APP-168/window-shell-2026-11-02/README.md) records checks and limits.

## App assistant and saved answers (next local iteration)

The title bar now keeps only Close; Card/Browse/Open is the single view selector. Settings uses a symmetric gear. One **App assistant** icon toggles the attached panel and resumes its last selected tab. Ask, Summary, Sources and Saved are tabs inside that panel. The former footer source disclosure and the Library Ask tab are removed. Sources lists the exact originals with View original and Ask about file. View original and file citations open the existing full-size, paginated file viewer directly; there is no narrow inline preview.

An app conversation is scoped to one saved app/work, or to one explicitly selected original. Apps with record collections default to App data, including empty collections. Otherwise a sole original is selected by default; multiple originals require a choice. Record-component questions retain their collection scope. App questions from Ask controls, `/ask`, mentions and saved-app `/<address> <question>` route to this companion. Existing explicit filing/decision commands remain intact. Cross-app, folder and widget questions retain the workspace Ask capabilities in a canvas panel rather than in Library.

The app assistant reads and explains existing data in **Ask**, and offers a separate **Change data** mode. The member selects an existing create/update command and, for an update, a specific record, then describes the change. The server extracts typed fields only for that command and validates a proposal without saving. The review names the action, record and proposed fields. Only explicit confirmation submits the exact proposal to the existing audited, idempotent command route, which rechecks permissions, schema, work/publication and record versions. The review replaces the request form; Cancel restores the untouched draft. Editing the request requires a new proposal; uncertain saves retry the same identity. Supported spreadsheet-built apps receive independent editable collections and Add/Update actions; existing source-only apps offer an explicit Enable app data review. Other apps without supported commands remain read-only. See [editable spreadsheet app data](CSV-APP-DATA.md). No uploaded original is rewritten. The assistant never calls the builder, Shape, composition fallback or the generic action router. Building, theming and reshaping remain separate. The current Ask backends answer one question at a time; displaying a session transcript does not add conversational memory to the provider request. File Ask's bounded context remains distinct from viewing every row of an original.

**Save answer** files a historical snapshot in that app's Saved drawer. **Place on canvas** exposes an optional reference card linked back to its app. Its X and **Put away** remove only placement. **Put all away** files that app's placed answers without deleting any saved answer or touching another app. **Delete saved answer** is a separate, explicit server operation. App-owned answers no longer automatically scatter onto the canvas on reload. Existing saved answers with a single work association appear in that work's drawer; the Library Cards inventory remains available. Record answers retain their receipts and the existing Make it a card preview for live record subsets.

The existing member-card store now accepts explicit app/work ownership for no-record answers and file snapshots. The server validates original file identity, exact lineage, digest, canonical parsed sheet/row/label and bounded row-reference structure, and rechecks original-file access during save and listing. This is provenance validation, not a recomputation of generated prose. Pins remain private to their member. No schema migration or background model job is introduced.

Each view cancels/invalidates pending reads when hidden or removed, reauthorizes when reopened and rejects late callbacks. Only the selected view loads data. Chat retains a bounded session thread and draft while changing app views or closing the panel; revocation clears them. Saved snapshots are durable; unsaved conversation text is not persisted across reloads.

## Component receipts

App-owned components keep their query receipts, digests and source metadata for validation and AI context. Their technical Verified footer is hidden in app views and Focus; incomplete, partial or truncated result notices remain visible next to the data. Verified described a traceable execution receipt, not a guarantee that uploaded facts are correct. Original files are reached from the app assistant. Independent cards retain their existing receipt affordance.
