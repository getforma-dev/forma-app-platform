# What a widget is

Spike APP-148. Read from APP-103, APP-108, APP-120, and the ask body in `app/src/composer/talk.rs`.

## Definition

A widget is one member's one-off object on the board. It has no `app_id`, no cards, and no `/w/{app}/{work}` page. The kinds APP-120 names are weather, calculator, clock, note, and folder. Note keeps its private records and is still a widget to the member (APP-120). Folder is a widget too. APP-149 says how a folder produces an app.

## What it can do

It can be placed on the board. Where it already has fields, those fields stay. It cannot be united into an app page, shared as an app, or given components.

Ask does not target a widget today. `POST /v1/ask` accepts `scope.kind` of `card`, `app`, or `canvas` only (`app/src/composer/talk.rs`). **Proposed.** APP-143 adds a widget scope for weather, clock, and calculator. Any other widget kind is refused. Shape can already target a widget. Shape is not how the member asks.

## Boundary with an app

An app is an admitted definition and a saved work. A widget is not. Formation on a folder produces an app through admission. That result is an app. The folder remains a widget (APP-108, APP-120). This note does not redesign that handoff.
