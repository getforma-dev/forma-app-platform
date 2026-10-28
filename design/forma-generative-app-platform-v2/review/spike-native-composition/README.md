# Native composition experiment on Forma 3.0.0 as shipped

Run 2026-10-22 by the lead during the foundation review (`../foundation-review.md`, 4.3). It answers
one question: can first-party renderers interpret a typed app definition natively, with data at
first paint, on the released framework, with no framework change?

## Method

1. `npx -y @getforma/create-app@3.0.0 composition-spike --template minimal` in a scratch folder;
   `npm ci` in `admin/` installed the published `@getforma/core`, `@getforma/compiler` and
   `@getforma/build` 3.0.0; `cargo build --locked` resolved `forma-server` and `forma-ir` 3.0.0
   from crates.io (Node 22.21.0, rustc 1.93.1, dev profile).
2. `admin/src/home/app.tsx` replaced by `app.tsx` here: one `Canvas` island whose only input is a
   declared `cards` array. Each card has boolean kind flags, a title, a layout string and the data
   of a summary (`items`), a table (`rows` of `cells`, nested lists), an SVG timeline (`bars`,
   attributes from declared strings) and a form (`fields` with a text input or a select chosen by
   declared flags; label and control paired by `for`/`id`). The island seeds one literal-default
   signal and binds a reactive attribute from it. `build.ts` is the template's, unchanged.
3. `src/main.rs` patched as in `main.rs` here: `#[derive(PageProps)]` for the same shape (nested
   `Vec`, a unit enum with `#[props(rename)]`, booleans) and a `fixture()` with four cards; the
   handler renders it through the typed handle.
4. `npm run build:ssr` (strict), then the server with `FORMA_PORT=0`, then `curl` of `/`.
5. A variant (`variant.diff`) replaced the four flag conditions by `card.kind === '…'` on the
   declared enum and typed the SVG coordinates as numbers.

## Results

| Step | Outcome | File |
|---|---|---|
| Strict SSR build | admitted: `home.ir` 4,752 B, 1 island, no diagnostic; `about.ir` 415 B | `manifest.json`, `size-report.json` |
| Route `/` size policy | Forma code 27,674 of 28,100 B gzip; JavaScript 30,116 of 76,800 B | `size-report.json` |
| Rust bind, finalize, render | both SSR routes bound and finalized ("Phase 2 SSR enabled for 2 route(s)"); `GET /` returned 200 with 6,752 B of HTML carrying every fixture value: the summary's two values, three table rows of three cells, six `<rect>` with titles, the select with both choices, the text input, one inert `data-forma-state` block, the island marked `pending` before activation | `rendered-home.html`, `server.log` |
| Response policy | `cache-control: no-store`; enforced CSP with a per-response nonce, `'strict-dynamic'`, `style-src-attr 'unsafe-inline'` as the one allowance, `require-trusted-types-for 'script'`; `x-content-type-options: nosniff` | `response-headers.txt` |
| Variant | refused at `app.tsx:34:29` with BUILD-610 "computed declared values remain outside the native subset" on the first enum comparison; the build stops there, so the numeric-attribute change was not reached | `variant-result.txt` |

## What it shows and what it does not

- A data-driven canvas of four renderer kinds is inside the published native subset: boolean
  conditions from declared props, nested `createList` over declared arrays inside `createShow`
  branches inside list rows, SVG attributes from declared strings, forms within the BUILD-618
  nesting rules, and one island root with a reactive attribute. The server renders it with its
  data under the normal `PageResponse` policy. No framework change was needed.
- Kind dispatch must be declared booleans computed by the handler; an enum comparison in a native
  condition is refused by name.
- The route's Forma-code budget had 426 B of headroom with four core primitives imported, which is
  why FORMA-232 is treated as a platform dependency.
- Not shown: browser activation of the rendered island (adoption with zero writes, the button's
  update) was not run here; the framework's own suite covers these primitives, and FORMA-246 adds
  this exact fixture to it with a three-engine adoption check. This is a dev-profile scratch build,
  not a release qualification, a performance measurement or an application.
