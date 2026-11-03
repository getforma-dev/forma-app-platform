# What a component is

Spike APP-147. Read from APP-120, APP-106, and `page::record_links` in `app/src/query/page.rs`. A sentence marked **proposed** is not in the code yet.

November 1 update: a **card is a component's canvas presentation**. A component can become a compact summary, a wide working area or an on-demand form inside one app. [ADR-015](../../architecture/ADR-015-COMPOSED-APP-PRESENTATION.md) and the [composition brief](composed-app.md) define the planned extension without changing component/work identity.

## Kinds

A component is one part of an admitted app. The kinds the definition already has are:

- An admitted view, which may present numbers, rows, text or a form. The canvas shows it as a card; the current work page renders the supported admitted parts and declared layout (APP-144).
- A records collection has a current-records presentation and authorized creation links. It is currently appended separately from admitted view parts; APP-158 will make a record list a first-class admitted component with explicit filter scope.
- A form that posts a command through the audited command route.

The address is `app.card` or the pane number (APP-120). A component is not a separate app. It has no `app_id` of its own.

## On the board and in the page

Today a component on the board is a card. Unite moves those cards into one frame and does not copy their data (`canvas/unite.ts`). Focus opens one card. Escape returns to the board with the card unmoved (`canvas/focus.ts`).

APP-142 implements the same component as part of the united page. Closing Focus returns to that page; Fan out returns the parts to the board. The records are the saved work in both places. The current source/local evidence and remaining hosted acceptance are recorded in [MVP gap completion](../MVP-GAPS-2026-11-01.md).

**Planned extension.** APP-155 defines semantic regions and presentation; APP-156 lets an admitted button open an existing component in a dialog or navigate to it. Opening a form never runs its command. APP-157 persists safe appearance, and APP-158/159 add record lists and real calculations. These are not implemented by the existing view/input `links`, and the presence of a view kind does not prove a supported query or action.

## What is not a component

A widget. A stack. The page frame. The window size. A file in a folder.

## Open

Not decided, and not built:

- A component leaving the app and becoming a widget while its records stay.
- A component removed from the app whose records are kept.
