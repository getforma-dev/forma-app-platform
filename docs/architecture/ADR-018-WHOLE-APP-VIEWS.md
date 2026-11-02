# ADR-018: Whole apps, independent cards and presentation views

Status: Accepted by Victor's November 2 redesign request; frontend implementation underway.

An app is one saved work and one canvas object. Its App Card is the entry point; Stack and Open are alternate presentations of that same object. Focus temporarily presents its components. The standalone member page is `/w/{app}/{work}` and opens in a new tab. Sharing retains its authorized `/s` URL and existing invitation/access rules.

The Library has Apps, Widgets and Cards. Apps expose Add (App Card), Stack and Open, with whole-object Remove. Components inside an app remain part of that app and cannot be separately placed from the Library or removed from its expanded view. Legacy Fan Out/Spread commands cannot extract them. Only Open exposes resize. A component remains a trusted renderer with its existing query, record, form and receipt logic; no replacement backend is introduced.

One controller owns all component nodes of each exact app/work, serializes asynchronous placements and reuses one canvas frame. Switching views replaces the presentation instead of creating a second owner. Old united frames migrate to Open, same-work stacks to Stack and older loose component subsets to App Card. Authorized metadata is required before restoration. Transient failure retains references without rendering cached private content. Removal cancels pending placement and clears view placement, not saved data.

The existing `/v1/cards` inventory already distinguishes saved Shape cards and pinned Ask answers by their own card IDs. The Library Cards section also lists these artifacts, reusing their existing governed rendering and historical-answer labels. They are not inferred from, or reclassified as, their source app/work.

For saved works produced by Make a card/Keep data versus Make an app, the backend has no durable artifact-kind distinction. Do not infer one from title, slug or component count. Successful explicit Make a card and Keep data creation records exact app/work intent in member-scoped local storage. A reversible personal Library control permits classification of an existing whole work. Ambiguous works remain Apps. APP-169 owns durable classification and a later compatibility-reviewed cleanup of legacy placement. Cards may contain more than one internal renderer but remain atomic.

Published component hiding is not admitted by the existing schema. Optional personal hiding must say “Hide in my views on this browser”, store bounded IDs for an exact app/work and stay outside publication requests. It applies to local member views, not guest projection or grants. It is appearance, not authorization. APP-170 owns durable/published visibility, including action/dialog reachability. The current backend components and data model are retained. APP-171 evaluates an optional desktop icon view later.

The common app toolbar retains Focus, Edit and Share where authorized, Open page and Remove from canvas. Inner component remove/focus controls and redundant successful Ready badges are suppressed; loading, errors, stale state and receipts remain. An app's live behavior, generic actions and record forms continue using the existing controller and server routes.

APP-168 carries implementation and qualification. This decision supersedes older fan-out/per-app-subcard UX instructions without rewriting their historical evidence. Deployment is explicitly held for the redesign PR. Local qualification does not claim AWS release.
