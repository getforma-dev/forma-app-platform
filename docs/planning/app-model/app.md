# What an app is

Spike APP-146. Read from APP-120, APP-106, APP-141, APP-142, APP-144, APP-145, and `choose_authoring` in `app/src/storage/composer.rs`. A sentence marked **proposed** is Victor's direction and is not in the code yet.

## Identity

In the product, an app on the canvas is a saved work with its admitted definition and bound data. The database identities are distinct:

- `organization_id` scopes every identity below.
- `app_id` identifies the owned authorization/authoring container (`genui.apps`). It can contain several definition revisions, publications, and saved works.
- `publication_id` identifies the admitted published definition selected by a saved work.
- `work_id`, together with `app_id`, identifies the saved work and its record collections and source bindings. `genui.saved_work` binds that pair to its publication (`app/migrations/0002_tenant_foundation.sql`, tables `apps`, `publications`, `saved_work`, `record_collections`, and `work_bindings`).

`choose_authoring` can reuse the same owned non-library container for successive generated definitions (`app/src/storage/composer.rs`). `draft_publish_save` publishes each definition, and `save` creates a new work pinned to that publication (`app/src/composer/routes.rs`). Therefore two separately generated product apps can share one `app_id`; that ID alone does not identify the page or the record set. Page, Ask and share targets must keep the exact `(app_id, work_id)` under the trusted organization. They must not substitute the container's latest publication.

The library address, such as `expenses`, and pane number are UI locators, not authority. A component is addressed `app.card` or by pane number (APP-120). Current persistent names and aliases are keyed by `app_id`, with an optional card key (`app/src/storage/labels.rs`, `app_labels` and `member_labels`). Independent naming of two generated works in the same container is therefore an unresolved product-model gap; this spike does not assert it exists. A card, a widget, a stack, and a focus modal are not a product app.

**Proposed.** Creating that app happens in a modal before anything is placed (APP-141). Today, placing a library app is not creation, and describing something new publishes onto an app the member already owns.

## Who it belongs to

The owner is the principal in `genui.apps.owner_principal_id`. `choose_authoring` will publish onto an owned app only when that member has both `edit_definition` and `run`, and it never chooses a library app (APP-093 review M2, `app/src/storage/composer.rs`). An `edit_definition` grant by itself does not make its holder the owner. A share grants use of the exact work; it does not make the recipient the owner (APP-095, APP-120). The current route grants read access by default; write access requires its explicit `record_write` request and the sharer's existing authority (`app/src/sharing/mod.rs`, `app/src/storage/sharing.rs`).

ADR-007 D7.3, restated in `app/src/composer/mod.rs`: a member who owns no such app can retrieve but not generate. **Proposed.** APP-145 is the one exception. The member's own first compose creates one empty shell. As of the 2026-11-01 review, that implementation exists on open PR 25 (`pivot/app-145-shell`), separate from this spike; it is not merged or accepted by this note.

## Where it lives

Two views of the same saved work are **proposed**.

- The board shows the app as cards. Unite can gather those cards into one frame today (`canvas/unite.ts`). That frame is not yet the app page. APP-142 adopts it as the page.
- The HTML page is the existing `GET /w/{app}/{work}` (`app/src/query/routes.rs`). APP-144 extends that renderer so the page is the admitted app. The model does not write the HTML.

## What it is not

An app is not a widget, not a folder, and not a stack. Focus is a view of one component. A unite frame that has not been adopted as the page is still an arrangement (APP-106, APP-120).
