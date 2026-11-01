# What an app is

Spike APP-146. Read from APP-120, APP-106, APP-141, APP-142, APP-144, APP-145, and `choose_authoring` in `app/src/storage/composer.rs`. A sentence marked **proposed** is Victor's direction and is not in the code yet.

November 1 follow-through: [ADR-015](../../architecture/ADR-015-COMPOSED-APP-PRESENTATION.md) defines one composed app with reusable components and coherent canvas/standalone/shared presentation. [The composition brief](composed-app.md) separates existing behavior from APP-155–161's planned extensions. It preserves the identities below.

## Identity

In the product, an app on the canvas is a saved work with its admitted definition and bound data. The database identities are distinct:

- `organization_id` scopes every identity below.
- `app_id` identifies the owned authorization/authoring container (`genui.apps`). It can contain several definition revisions, publications, and saved works.
- `publication_id` identifies the admitted published definition selected by a saved work.
- `work_id`, together with `app_id`, identifies the saved work and its record collections and source bindings. `genui.saved_work` binds that pair to its publication (`app/migrations/0002_tenant_foundation.sql`, tables `apps`, `publications`, `saved_work`, `record_collections`, and `work_bindings`).

`choose_authoring` can reuse the same owned non-library container for successive generated definitions (`app/src/storage/composer.rs`). `draft_publish_save` publishes each definition, and `save` creates a new work pinned to that publication (`app/src/composer/routes.rs`). Therefore two separately generated product apps can share one `app_id`; that ID alone does not identify the page or the record set. Page, Ask and share targets must keep the exact `(app_id, work_id)` under the trusted organization. They must not substitute the container's latest publication.

The library address, such as `expenses`, and pane number are UI locators, not authority. A component is addressed `app.card` or by pane number (APP-120). Library names and aliases remain keyed by `app_id`, with an optional card key (`app/src/storage/labels.rs`). The MVP follow-up adds a separate display name/version on each `saved_work` (migration 0018, `storage/work_labels.rs`). Generated works use that exact-work name; an optimistic, audited rename does not change sibling names, publication, work version, records or share pins. A card, a widget, a stack, and a focus modal are not a product app.

Creating that app begins in a modal before placement (APP-141). The MVP follow-up supplies four real admitted empty templates. An unchanged description instantiates the selected definition; an edited description customizes it through model admission. Successful confirmation places and opens one united page automatically. Placing a library app is not creation. An explicit description publishes onto an eligible owned app, creating a first shell through APP-145 when necessary.

## Who it belongs to

The owner is the principal in `genui.apps.owner_principal_id`. `choose_authoring` will publish onto an owned app only when that member has both `edit_definition` and `run`, and it never chooses a library app (APP-093 review M2, `app/src/storage/composer.rs`). An `edit_definition` grant by itself does not make its holder the owner. A share grants use of the exact work; it does not make the recipient the owner (APP-095, APP-120). The legacy member-sharing route grants read access by default; write access requires its explicit `record_write` request and the sharer's existing authority (`app/src/sharing/mod.rs`, `app/src/storage/sharing.rs`). The newer named app-guest Share UI is view-only under [ADR-014](../../architecture/ADR-014-LIVE-APP-SHARING.md); it does not create organization membership or command authority.

APP-145 is the reviewed exception to ADR-007 D7.3: an eligible member's intentional first compose creates one empty owned shell. Read-only commands, catalog and context access create nothing. The existing publication policy still governs publication. [PR 25](https://github.com/getforma-dev/forma-app-platform/pull/25) records delivery and CI; the model definition does not substitute for first-user hosted acceptance.

## Where it lives

The MVP follow-up implements these presentations of the same saved work; final source and hosted evidence is tracked in [MVP gap completion](../MVP-GAPS-2026-11-01.md).

- The board shows the app as cards. Unite can gather those cards into one frame today (`canvas/unite.ts`). APP-142 adopts that frame as the app page; its parts follow the admitted default layout.
- The HTML page is the existing `GET /w/{app}/{work}` (`app/src/query/routes.rs`). APP-144 renders the admitted parts/layout, exact command forms, current records and qualified source views. Unqualified queries show an explicit unavailable state. The model does not write the HTML.
- A named guest uses `/s/{share}` or an explicitly added connected canvas reference under APP-154. Both keep the same pinned saved work and read-only guest authority. A stable URL does not imply anonymous access or a dedicated server.

## What it is not

An app is not a widget, folder or stack. Focus is a view of one component. The united page is a presentation of the saved app, not an additional app identity (APP-106, APP-120, APP-142).
