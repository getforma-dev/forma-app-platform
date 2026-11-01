# What an app is

Spike APP-146. Read from APP-120, APP-106, APP-141, APP-142, APP-144, APP-145, and `choose_authoring` in `app/src/storage/composer.rs`. A sentence marked **proposed** is Victor's direction and is not in the code yet.

## Identity

An app is one admitted definition and the saved work a member holds. The ids the product already uses are `app_id` and `work_id` (`genui.apps`, `genui.saved_work`). The library address is the app's address, such as `expenses` or `1`, and a component is `app.card` or the pane number (APP-120). A card, a widget, a stack, and a focus modal are not an app.

**Proposed.** Creating that app happens in a modal before anything is placed (APP-141). Today, placing a library app is not creation, and describing something new publishes onto an app the member already owns.

## Who it belongs to

The owner is the principal in `genui.apps.owner_principal_id`. `choose_authoring` will publish onto an owned app only when that member has both `edit_definition` and `run`, and it never chooses a library app (APP-093 review M2, `app/src/storage/composer.rs`). A share grants use of the work. It does not make the recipient the owner (APP-095, APP-120).

ADR-007 D7.3, restated in `app/src/composer/mod.rs`: a member who owns no such app can retrieve but not generate. **Proposed.** APP-145 is the one exception. The member's own first compose creates one empty shell. That grant is not built.

## Where it lives

Two views of the same saved work are **proposed**.

- The board shows the app as cards. Unite can gather those cards into one frame today (`canvas/unite.ts`). That frame is not yet the app page. APP-142 adopts it as the page.
- The HTML page is the existing `GET /w/{app}/{work}` (`app/src/query/routes.rs`). APP-144 extends that renderer so the page is the admitted app. The model does not write the HTML.

## What it is not

An app is not a widget, not a folder, and not a stack. Focus is a view of one component. A unite frame that has not been adopted as the page is still an arrangement (APP-106, APP-120).
