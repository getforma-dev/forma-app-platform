# What a component is

Spike APP-147. Read from APP-120, APP-106, and `page::record_links` in `app/src/query/page.rs`. A sentence marked **proposed** is not in the code yet.

## Kinds

A component is one part of an admitted app. The kinds the definition already has are:

- A view that returns numbers or rows. The canvas shows it as a card. The work page lists it in the outline.
- A records collection. `record_links` offers "New {schema} record" when the member can write (`app/src/query/page.rs`).
- A form that posts a command through the audited command route.

The address is `app.card` or the pane number (APP-120). A component is not a separate app. It has no `app_id` of its own.

## On the board and in the page

Today a component on the board is a card. Unite moves those cards into one frame and does not copy their data (`canvas/unite.ts`). Focus opens one card. Escape returns to the board with the card unmoved (`canvas/focus.ts`).

**Proposed.** In the APP-142 page the same component is a part of the page. Closing focus returns to the page. Fan-out is the only action that puts the parts back on the board. The records are the saved work in both places.

## What is not a component

A widget. A stack. The page frame. The window size. A file in a folder.

## Open

Not decided, and not built:

- A component leaving the app and becoming a widget while its records stay.
- A component removed from the app whose records are kept.
