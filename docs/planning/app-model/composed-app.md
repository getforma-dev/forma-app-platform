# From cards to a composed app

Accepted direction, 2026-11-01; implementation follows [ADR-015](../../architecture/ADR-015-COMPOSED-APP-PRESENTATION.md). The eight user-supplied screenshots guide hierarchy and interaction. Their private preview address and original image files are not copied into this repository. The [implementation record](../COMPOSED-APP-IMPLEMENTATION-2026-11-01.md) records the actual delivered subset and qualification; this brief does not claim competitive feature parity.

## What someone should experience

“I made an expense tracker” should yield a recognisable product: its name, a short explanation, a clear **Log meal** action, useful summaries and the records being tracked. On the canvas it is one contained app. Opened at its own URL, it fills the document. A permitted recipient sees that same app through their authorized projection, without builder tools. Going back to editing reveals its components and composition controls.

The useful mental model is **app → components → presentations**. A component can be a small metric, a chart, narrative text, a list or an interactive form. Its card is how the builder exposes it on the canvas; a visible card border is not its identity. The form does not need to occupy the dashboard until the user invokes it.

Buttons and actions are **app-agnostic**. The expense screenshots supply examples, not a special action library: the builder can label a control “View customer,” target a details component and present it in a modal, or label it “Add equipment” and open the appropriate form. A section link uses the same app's declared navigation action. Behavior and target are configured separately from the label and domain; each still obeys the app's current permissions. The first supported action kinds are deliberate, reusable building blocks rather than a promise that every possible interaction is already implemented.

| Reference element | Forma composition decision | Needed behavior |
| --- | --- | --- |
| Name and persistent “Log meal” button | App header, one primary action | Button targets the admitted meal form; view-only guests receive no write action |
| “Every meal out” introduction | Optional heading and text region | Escaped authored text, semantic heading order, no generated markup |
| Small spend/count/tip/status panels | Summary region with compact components | Real declared aggregates, scope and receipt access; empty differs from unavailable |
| Budget amount, progress and Set control | Progress component with declared denominator provenance | A preset/input budget is labelled honestly; exposing a persisted Set control requires an admitted audited budget-record command and record/version provenance in the result receipt |
| Monthly chart and dining ranking | Analysis section with chart/list components | A defined time window and deterministic sort; accessible table alternative |
| Search, status tabs and transactions | Working section with admitted records component | Filters state their targets and query scope; partial/paginated results are labelled |
| Expense modal | One existing form component shown on demand | Preserve draft and command identity; explicit submit, validation, pending/outcome recovery |
| Builder preview and standalone URL | Two containers for the same app document | Same identity, order, theme and authority; container-aware responsive layout |
| Static/waking banner | Later lifecycle state, not a data value | Clearly distinguish preview/waking/offline/error/live; never show invented zero totals |

The receipt-scan copy, CSV button and verification statuses in the screenshots are references only. They do not establish that Forma supports receipt-photo verification, authorized export or those workflow semantics. Add or select their owning capabilities before displaying such controls or claims.

## Layout and authoring

Start with one document: header → introduction → summaries → analysis → working area. Related summaries can share a row; a large chart and a ranked list can sit beside one another when space permits. At narrow widths they stack in the same semantic order. Dense record tables may scroll within their own labelled region; the app header and primary action must stay reachable without page-wide horizontal overflow. Long titles, labels and validation errors must fit at enlarged text sizes.

Use restrained shared spacing, typography, surfaces and chart colors so components belong to the app. The terminal theme is one optional preset, not Forma's required brand. All presets preserve contrast, focus visibility, state labels and reduced-motion behavior. Do not copy the screenshot's blurred or clipped content as a design requirement.

The builder selects a component, places it in a region, changes its bounded size/order, and chooses whether it is visible inline or opened by an action. A primary-action control has a label, a known target and a finite presentation choice. Preview uses the admitted draft and the existing isolated preview rules; publishing validates the whole definition. Missing targets, duplicate placements that create multiple active command forms, invalid layout and unsupported theme tokens receive specific errors. Keep one canonical form state per component per presentation session.

An authored theme/layout/action change follows publication and share-pin rules; dragging a personal canvas viewport does not republish the app. The first implementation should preserve existing definitions and their default layout, avoiding a mandatory rewrite of saved work. The primary-action slot stays absent until a valid action is admitted; no decorative dead buttons.

## Current foundation and actual gaps

The following observations describe the source inspected on November 1 after PR 31, not planned completion:

| Source surface | Already present | Extension required |
| --- | --- | --- |
| `contracts/schema.py`, `app/src/admission/validate.rs` | Nine view kinds: summary, table, detail, timeline, chart, filter, form, note, status; numeric default layout | Versioned semantic presentation and actions; a kind's admission alone does not prove its runtime behavior |
| `app/admin/src/canvas/unite.ts`, `focus.ts` | Unite/Focus preserve existing component nodes; declared layout is consumed | Coherent app-use chrome and shared presentation lifecycle without depending on the full board |
| `app/admin/src/work/app.tsx`, `render.ts`, `document.ts` | Member app document, admitted parts, inline forms, current records and qualified source views | Internal actions, modal form placement and reusable app shell |
| `app/src/sharing/live.rs`, `app/admin/src/shared/parts.tsx` | Named live guest projection, connected placement, current records, authority lease | Same presentation while retaining view-only capability filtering and revocation clearing |
| `app/src/composer/shape.rs`, `app/admin/src/canvas/shape.ts` | Closed `AppTheme` mode/accent/density, applied in session | Publication-owned persistence and matching member/shared rendering |
| `contracts/validate.py`, `app/src/admission/validate.rs` | Known-ID validation of existing view/input `links` | Separately typed actions; no inspected runtime consumer makes those links a button graph |
| `app/src/storage/queries.rs`, `app/src/query/canvas_ledger.rs` | Qualified source queries; specific live expense ledger bounded to newest 200 records | General admitted record snapshots/filters/aggregates with honest scope and receipts |

Published work is still the exact `(organization_id, app_id, work_id)` with its pinned publication and bindings. New component IDs cannot widen that scope. A shared page containing a table shares the permitted table data; hiding columns or filtering in the browser is not a metrics-only security boundary.

## Build and prove it

1. Implement and deploy the composed-app capabilities now, as Victor subsequently directed. Continue the genuine APP-139/140 recipient trial independently; synthetic dashboard qualification does not substitute for that trial.
2. APP-155 establishes a coherent shell using current supported components and deterministic layout fallback. APP-156 adds real actions and the target picker; APP-157 adds persisted safe themes.
3. APP-158/159 implement record components and qualified calculations. Reuse the existing bounded query, record snapshot and receipt contracts; never derive an all-record total from the currently visible page.
4. APP-160 creates a synthetic meal tracker through normal admission and real record submission. Its metric, trend, ranking and budget values must be recomputable from actual saved inputs and receipts. It is a test scenario, not a special-case renderer or fake customer dataset.
5. APP-161 measures future execution lifecycle options after this interaction model is understood. It produces a decision and follow-up tickets, not an immediate cloud switch.

Qualification compares embedded use, standalone member use and shared recipient use at wide and narrow container widths. It covers keyboard-only operation, focus and screen-reader names, draft recovery, failed and unknown submissions, live record updates, source/definition changes, guest restrictions, expiry/revocation, offline masking and reconnect. Record supported browser/device coverage and actual limits; a screenshot proves appearance, not persistence, authorization or concurrency.

## Later hosting choice

Forma currently uses a shared application service, not one server per saved app. Its authentication vault and event hub have process-local state; reducing that service to zero would affect other users. A future design can keep the stable app URL and authorized shell available while managing costly execution separately. It must compare measured costs, including storage and shared services that do not disappear when execution sleeps.

A private cached preview cannot outlive authority or be labelled Live. The current five-second sharing lease remains a requirement; a sleep screen must not preserve revoked content. Wake requests need fresh authorization, deduplication, bounded retries and cost controls. Existing background workflows, imports and webhooks need explicit lifecycle rules. No automatic resubmission of a meal or budget write is allowed when a service resumes.
