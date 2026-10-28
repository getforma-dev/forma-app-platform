# FORMA-246: Native composition fixture: a data-driven canvas proven in the 3.0 subset, kept as a conformance test

| Field | Value |
|---|---|
| Epic | E3 |
| Status | Backlog |
| Priority | P2 |
| Release | 3.0.x maintenance or 3.1.0; no public API or wire-format change |
| Repos | getforma-dev/forma: @getforma/compiler (`tests/fixtures/conformance/`, `docs/subset.md`), forma-server (`tests/typed_pages.rs`), the parity harness, docs |
| Decision | Founder 2026-10-22: "if the design asks for changes to the stack ... create a ticket and assume it done for the design"; platform v2 foundation review (`design-doc-factory/tasks/forma-generative-app-platform-v2/review/foundation-review.md`, 4.3 and 6) |
| Depends on | none |
| Assignee | FormaStack team |
| Team | FormaStack |
| Blocks team | GenUI |
| Blocks | [APP-003](../../../FormaBuiltApps/forma-app-platform/Tickets/9-blocked/APP-003.md) — full-canvas native composition qualification |
| Branch / PR | |

## Why

The Forma platform v2 (the industry-agnostic company app platform) renders typed app definitions
through first-party compiled components. Its canvas is one island whose only input is a declared
`cards` array: each card carries boolean kind flags, a title, a layout string and the rows, bars,
items and fields of a summary, a table (nested lists for rows and cells), an SVG timeline
(attributes bound to declared strings) and a form (label and control by `for`/`id`, a text input or
a select chosen by declared flags).

On 2026-10-22 the review compiled exactly that shape against the published 3.0.0 packages
(`@getforma/create-app@3.0.0` minimal template; core, compiler, build 3.0.0; forma-server and
forma-ir 3.0.0 from crates.io):

- `npm run build:ssr` in strict mode admitted it: `home.ir` 4,752 B, one island, no diagnostic.
- Rust `#[derive(PageProps)]` for the same shape (nested `Vec`, a unit enum with renames, booleans)
  bound, finalized and rendered the page with its data at first paint under the normal
  `PageResponse` policy (6,752 B of HTML; nonce CSP; `no-store`).
- Two refusals the platform designs around: `createShow(() => card.kind === 'table', …)` on a
  declared enum is BUILD-610 (flags from the handler instead); composition deeper than three
  local components is BUILD-403.
- The route's Forma code measured 27,674 of 28,100 B gzip with four core primitives (FORMA-232).

Evidence: `design-doc-factory/tasks/forma-generative-app-platform-v2/review/spike-native-composition/`
(`app.tsx`, `main.rs`, `size-report.json`, `rendered-home.html`, `variant.diff`, `variant-result.txt`).

Nothing in the framework names this combination as supported. A future minor could narrow
admission (nested lists inside `createShow` branches inside list rows; SVG attribute bindings from
declared strings; label and control by `for`/`id` under BUILD-618; a reactive attribute from a
literal-default signal on the island root) without any test failing. The platform depends on it,
so it becomes a named fixture: the platform design assumes this ticket done.

## What changes

- **A conformance fixture `composition-canvas`**: the TypeScript page, the derived Rust
  `PageProps` and a fixture props value, placed with the compiler's conformance fixtures and
  consumed by `crates/forma-server/tests/typed_pages.rs`: strict SSR admission, bind, finalize,
  render, and the existing parity harness over the fixture's states (two or three cards per kind,
  empty arrays, long and non-ASCII text).
- **Refusal variants beside it**: the enum-comparison condition (BUILD-610) and a four-level
  composition (BUILD-403), each refused by name.
- **`docs/subset.md`** gains a short "data-driven composition" pattern: boolean kind flags from
  the handler, nested lists for cells, SVG from declared strings, forms inside the BUILD-618
  nesting rules, pagination against the 1 MiB head-state cap.
- **Size reference**: the fixture route's Forma gzip recorded beside the route policy as a reference
  point for FORMA-232.
- **No runtime, compiler or wire-format change.** If the fixture reveals a refusal on current
  `main`, the scope grows to restore admission; it then becomes P1 and a Forma release matter.

## Acceptance tests

- [ ] The fixture compiles in strict SSR mode on `main` with zero diagnostics; the two refusal
  variants fail with BUILD-610 and BUILD-403 by name.
- [ ] forma-server binds the derived `PageProps`, finalizes and renders the fixture; the HTML
  contains every fixture value and one inert state block; the response is a normal `PageResponse`.
- [ ] Browser adoption of the rendered page succeeds with zero initial DOM writes in Chromium,
  Firefox and WebKit; the island's button updates its reactive attribute; the parity harness is
  green over the fixture's states.
- [ ] `docs/subset.md` and the CHANGELOG are written; there is an independent review; CI is green.

## Migration and rollback

- **Migration:** none; additive test and documentation.
- **Rollback:** none needed.

## Log

- 2026-10-22: Filed from the platform v2 foundation review. The platform design treats the fixture's
  admission as given; under design A's markup representation (FORMA-237 to 241) the fixture is still
  useful but not a dependency.

- 2026-10-28 (GenUI cross-team handoff): Assigned to the FormaStack team; status remains Backlog and no implementation has started. This is the native-composition upstream blocker for GenUI APP-003 full-canvas qualification. Rebaseline the fixture against the selected Forma 3.2 source/package set and deliver the ticket's native, browser-adoption and refusal evidence, with the reviewed source identity and availability recorded. GenUI retains app-specific integration acceptance; historical 3.0 server rendering and the 3.2 release alone do not clear this gate. FORMA-232's separate budget disposition also applies. Independent GenUI work continues in parallel. See the [cross-team journey](../journeys/genui-platform-2026-10-28/README.md).
