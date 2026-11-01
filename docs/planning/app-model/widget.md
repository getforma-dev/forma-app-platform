# What a widget is

Spike APP-148. Read from APP-103, APP-108, APP-120, and the ask body in `app/src/composer/talk.rs`.

## Definition

A widget is one member's one-off presentation on the board. It has no component hierarchy or app-page entry in that presentation. The kinds APP-120 names are weather, calculator, clock, note, and folder. This product category does not imply that the object has no governed backing:

- Weather, calculator and clock are one-off utilities, not admitted app works.
- Note keeps records in the member's own saved work of the admitted Notes library app. `/v1/notes` returns `app_id` and `work_id`; reads and writes use the existing record and audited command routes. It is still presented as a widget (APP-120; `app/src/actions/routes.rs`, APP-109 module notes).
- Folder is an authorized box holding files and can carry an optional `app_id` association (`app/src/storage/boxes.rs`, `BoxRow` and box creation). Formation produces a separate saved work through admission. APP-149 describes that handoff.

Calling Notes or Folder a widget must not remove their authorization, record, or receipt checks.

## What it can do

It can be placed on the board. Where it already has fields, those fields stay. The widget presentation is not united into an app page or given components. That is not a claim that its backing work cannot be shared: the Notes implementation explicitly permits its author to share the work, and folders have their own share route (`app/src/actions/routes.rs`, `app/src/boxes/routes.rs`).

The general Ask route has no widget scope today. Folder already has its separate authorized Ask route (`POST /v1/boxes/{box}/ask`); the claim below is about the general route. `POST /v1/ask` accepts `scope.kind` of `card`, `app`, or `canvas` only (`app/src/composer/talk.rs`). **Proposed.** APP-143 adds a widget scope for weather, clock, and calculator. Any other widget kind is refused. Shape can already target a widget. Shape is not how the member asks.

## Boundary with an app

A product app presents an admitted definition and saved work as a unit of components. Widget is a presentation category, not a grant exemption or a statement about its storage identifiers. Formation on a folder produces an app through admission. That result is an app. The folder remains a widget (APP-108, APP-120). This note does not redesign that handoff.
