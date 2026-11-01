# Library pane: placement, help and contextual Ask

APP-153, source review on 2026-11-01. This note is the design spike; it does not move a tab. APP-143 implements context selection and direct Ask entries separately.

## What the four tabs do

**Library** places admitted apps, their cards and utility widgets on the canvas. `canvas/app.tsx` owns `#cv-library` and its tab panels; `canvas/library.ts` fills and filters the catalog and handles placement/removal, with drag behavior in `canvas/library-drag.ts`. The catalog is permission-filtered discovery, not the template-creation flow. The L shortcut opens Library.

**Commands** is the searchable reference for registered verbs. `canvas/command-sheet.ts` fills its panel from `canvas/command-registry.ts`; `canvas/slash.ts` routes `/commands` and `/help` into it. Selecting an example fills the intent bar and hands focus there; it does not execute the command. This is discoverability and syntax help, not an alternative permission path.

**Ask** is a read-only question and answer thread. `canvas/ask.ts` fills it and `canvas/talk.ts` routes `?` and `/ask` to it. It submits a captured context to `/v1/ask`; the response is text with storage-backed record receipts. Shape has a separate route and a different job. APP-143 adds direct entries on the united app, focused part and supported utility widgets, plus a lightweight panel for standalone work documents.

**Shortcuts** documents each action's click path and keyboard or command equivalent. `canvas/shortcut-tab.ts` fills the panel from its closed action table and links to Commands. `/shortcuts` and the menu entry open it. It replaced the earlier standalone shortcuts sheet, as the source comment explicitly records.

## Why one pane exists

`canvas/pane.ts` records APP-119's consolidation into one right pane, one close control, shared focus return and ARIA tab keyboard behavior; `command-sheet.ts` and `shortcut-tab.ts` explicitly say their former separate sheets were replaced. APP-122 added Ask as the fourth tab. This explains the shared container and reduced collection of sheets. A broader product argument that placement, conversation and every help reference must permanently share the same information architecture is **not written down** in these files. The earlier aria label omitted Ask; APP-143's integration corrects that label without moving the tabs.

## Recommendation

| Tab | Decision | Reason and what a future move must preserve |
| --- | --- | --- |
| Library | Stay | It owns placement. Preserve L, search, drag, Add/Remove and permission-filtered catalog state. |
| Commands | Stay | The compact command reference makes the intent bar discoverable without another modal. Preserve `/commands`, `/help`, filtering, examples and fill-without-execute behavior. |
| Ask | Stay as the canvas conversation home; add contextual entries | The selected object determines scope, so a member should not have to find or name it in a separate catalog. Direct app/focus/widget entries solve that while preserving the canvas thread, `?`, `/ask`, mentions and queued questions. Focus and standalone documents need a local panel because the canvas pane is inert or absent there. Moving the entire thread now would disrupt those existing routes and focus behavior without evidence of a better home. |
| Shortcuts | Stay | One existing help surface already has the click/keyboard parity table and Commands link. A menu may remain its entry, but moving it back to a separate sheet recreates the prior fragmentation and risks keyboard/focus regression. |

The pane should be described as a multi-purpose pane in accessibility copy, while its Library trigger remains a convenient placement entry. Do not rename the product's app objects or treat every tab as an app catalog.

## APP-143 contract

Clicking another canvas object changes the context of the next question and leaves an unsent question intact. A united app uses only its saved work. Focus uses the focused part's collection where available; leaving focus restores app context. Each pending/queued question keeps the scope captured at submission. App and part buttons on `/w/{app}/{work}` use that same exact saved work; they cannot grant broader reads.

Supported utility widget scope is limited to weather, clock and calculator and carries no work identifiers. It describes validated controls/configuration; it is not an attestation of a browser display or a shortcut to app records. Notes retain their governed backing and folders retain their existing dedicated table Ask route. Unsupported source-result explanations must say so rather than answering from unrelated records. A refused or unverifiable response shows no purported answer or number. Existing receipt and authorization checks remain required.

No tab relocation is proposed for this MVP. The note is ready for independent review; accepting it does not close APP-143's implementation or runtime checks.
