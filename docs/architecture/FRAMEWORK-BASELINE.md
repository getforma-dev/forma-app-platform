# Forma 3.2 baseline for the app-platform implementation

Assessed 2026-10-28. This is a preparation record, not an app build or framework release qualification. The delivered design remains an immutable historical baseline; implementation refinements belong in a new ADR rather than edits that make old measurements appear current.

## Verified release

- Canonical repository: `getforma-dev/forma`, confirmed from the current framework checkout remote.
- Published stable release: [Forma 3.2.0](https://github.com/getforma-dev/forma/releases/tag/v3.2.0), published 2026-10-27 at 18:22:07 UTC; not a draft or prerelease.
- Annotated tag object: `06f60b423ac0758ded40a44000301249ffcc097c`; peeled source commit: `fc76ea8531f0e3a85d0224087384e41f52040e3c`. Use the commit when describing source identity.
- Queried the npm registry for exact 3.2.0 records of `@getforma/core`, `@getforma/compiler`, `@getforma/build`, `@getforma/router` and `@getforma/create-app`; all exist and declare Node >=22. Queried crates.io for `forma-ir`, `forma-ir-derive` and `forma-server` 3.2.0; all exist and are not yanked. This verifies metadata/publication availability, not a new downloaded-archive qualification campaign.
- `forma-server` requires Rust 1.85; the two IR crates declare 1.71. Choose a supported explicit toolchain for the app and commit npm/Cargo lockfiles when scaffolding.
- The release explicitly waives active soak and a new N7 twenty-run campaign. Those are not passing measurements and do not carry app performance evidence.

Registry evidence endpoints: `https://registry.npmjs.org/@getforma%2F<name>/3.2.0` and `https://crates.io/api/v1/crates/<name>/3.2.0`.

| Crate | Registry SHA-256 |
|---|---|
| forma-ir | 5617c2febe4a47e030bb412505d42ed24a0d5410dfadcc6430d2fbeb9eda3c2c |
| forma-ir-derive | 2a5d5dd3e9cd8cf28765a9aef84c142bad5ed3aa353dd904419e5b864cd16bc1 |
| forma-server | ff29031e7a3c8a12ee97791755244e4730bcca3bf05a80abc0e24f7c128c30cd |

Frozen design sources: [platform design](../../design/forma-generative-app-platform-v2/design.md), [foundation review](../../design/forma-generative-app-platform-v2/review/foundation-review.md), [demo design](../../design/forma-northstar-demo/design.md). [Implementation decisions](DECISIONS.md) reconcile the source precedence and deployment choice.

## Rendering and version decisions

Keep the delivered design's D1/K1: a typed, versioned `AppDefinition`, admitted as data and interpreted by trusted compiled first-party renderers. Model output never becomes JSX, executable code, SQL, arbitrary HTML, FMIR or a permission grant. This decision is supported by the design's sections 5.3, 6.1, 6.2, 12.2 and `review/foundation-review.md`; the later delivery is controlling over the earlier preparation wording that still calls it proposed.

Use typed Rust/FMIR SSR plus islands as the initial rendering model. Pages, tables and forms must carry their data at first paint, and ordinary links/forms retain a no-JavaScript path. Compute presentation flags/derived values on the server, use declared props, stable keys, bounded lists and registered islands. Do not switch to an explicit Browser application because the canvas is interactive. Do not import the old 2.0.1 prototype runtime into this app.

At scaffold time pin core/compiler/build and all used Rust crates to exact 3.2.0; invoke the exact published `@getforma/create-app@3.2.0`. Router is optional. If retained navigation is adopted, pin `@getforma/router` to 3.2.0 and enable `forma-server`'s `navigation` feature; use `createNativeAdapter` and a declared native group. The `native-router` starter demonstrates that path; `minimal` is suitable for ordinary SSR and must have SSR explicitly retained in all build/watch commands. No scaffold/build was run in this assessment.

The app's proposed navigation ADR should start with ordinary SSR links and canvas islands, then admit native retained-layout routing only where a representative measured group meets its constraints. It may legitimately put small library/settings pages in a group and use explicit document transitions to the larger canvas. Preserve one coordinator per document, server-owned session context generation, revocation of retained owners on context changes, complete `PageResponse` policy, and document fallback on policy/build incompatibility. Enhanced actions do not provide auth, CSRF protection or idempotency.

## Historical design assumptions that need explicit reconciliation

| Design assumption | Released 3.2 evidence | Implementation disposition |
|---|---|---|
| Forma v3.0.0 = 2006423b | 3.2.0 commit above, eight coordinated packages | New baseline ADR and exact app lockfiles; do not rewrite delivered evidence |
| N3: estimated Forma code <=28,100 B gzip; old four-kind fixture measured 27,674 B | `route-size.ts` ordinary-route ceiling is now 28,420 B, inherited from the 3.1 decision | Keep the design's 28,100 B target until explicitly reconciled; distinguish framework enforcement from the app target. Measure actual complete renderer library before claiming fit |
| Canvas total JavaScript up to 365 KiB | Ordinary islands/canvas still allow 365 KiB; ordinary non-canvas 75 KiB | Genuine canvas classification is appropriate, but does not enlarge framework-contribution ceiling |
| Retained native routing absent from 3.0 design | A native navigation group has 40,708 B Forma ceiling and 64 KiB total; `canvas: true` does not override a group. Closed eager graph, no group code splitting/computed dynamic imports | Explicit route-boundary/size spike before including the full canvas in a group |
| FORMA-232 configurable application budgets assumed done under K3 | The legacy work-board ticket is Backlog. Released BuildConfig only has the older Brotli `budgetThreshold` warning option; no proposed per-route application override API | External dependency remains open. Do not invent configuration, patch emitted assets or raise limits. If the app passes existing policy it can progress; if it fails, measure/reconcile this dependency before the blocked slice |
| FORMA-246 native composition conformance fixture assumed done under K3 | The legacy work-board ticket is Backlog; no named `composition-canvas` fixture or ticket reference found in relevant v3.2 sources | Add an app-local packed-3.2 compatibility gate; track upstream conformance work separately rather than claiming it shipped |
| fAuth at b750ca53 accepted for the demo | FORMA-247 deployment/provisioning/client setup and FORMA-248 key custody are legacy work-board Backlog tickets, separate from Forma release | Treat fAuth as an external adapter dependency; current deployment not verified here. FORMA-248 remains a real-organization gate |
| FORMA-237–242 HTML-runtime/markup work | D1 selected typed definitions and deferred the custom-widget tier | Related historical work, not blockers for the chosen initial representation |

Current source references:

- [3.2 upgrade contracts](https://github.com/getforma-dev/forma/blob/v3.2.0/docs/UPGRADING-3.2.md)
- [Routing guide](https://github.com/getforma-dev/forma/blob/v3.2.0/docs/ROUTING.md)
- [Route budgets](https://github.com/getforma-dev/forma/blob/v3.2.0/packages/build/src/route-size.ts)
- [Build configuration](https://github.com/getforma-dev/forma/blob/v3.2.0/packages/build/src/types.ts)
- [Native router starter](https://github.com/getforma-dev/forma/tree/v3.2.0/packages/create-app/templates/native-router)
- [Server manifest/features](https://github.com/getforma-dev/forma/blob/v3.2.0/crates/forma-server/Cargo.toml)

## Required early backlog slices

1. **Adopt the 3.2 implementation baseline and route model.** Record exact package/toolchain choices, preserve typed-definition D1, identify ordinary vs native-group route boundaries, and reconcile old N3 bytes with the published ceilings explicitly. Acceptance: ADR distinguishes released capability, historical evidence and unproven app behavior.
2. **Scaffold the published 3.2 app and build discipline.** Exact registry scaffold, lockfiles, reproducible frontend-before-Rust build, required SSR route assertions at startup, bind/finalize before listening, complete response policy. Acceptance: clean clone install/build, raw HTML with real data, explicit startup failure for unintended Browser/missing SSR output.
3. **Four-kind renderer feasibility/conformance spike.** Port the preserved 3.0 fixture into the app's own test surface; summary/table/timeline/form from one admitted definition; escaped strings, numeric SVG, empty/long/non-ASCII states; default route budgets. Test computed-condition and over-deep composition refusal where still applicable to the selected 3.2 source. Acceptance: strict native build; typed bind/finalize; first paint; retained node identity on adoption; real form interaction; zero initial repair writes; current size report. No hand-edited generated output.
4. **Routing feasibility and lifetime gate.** If using router, measure whole eager graph; test direct URL/refresh/back-forward, no-JS navigation, retained layout state, disposed leaf resources, stale-request cancellation, changed-session revocation, full GET fallback and uncertain mutation recovery. Treat this as an app gate, not re-running the framework release campaign.
5. **Data-through-renderer vertical spike.** Run the design's admission → authorized query → receipt → command → idempotent outcome → reopen path on both synthetic app shapes. Record real full-library canvas bytes, server render rate/memory and cold one-million-row DuckDB query behavior before committing performance claims.
6. **fAuth integration readiness.** Link FORMA-247 instead of duplicating its external work; app adapter tests include PKCE/state/replay, missing tenant, forged/unknown kid, canary organization, cross-site POST and opaque cookie lifetime. Link FORMA-248 as a release gate for real organizations; historical viability is not a live deployment receipt.

## Limits and completion evidence

No app implementation, dependency installation, build, registry archive authentication, fAuth deployment check, test run, framework change, release or migration was performed. The work verified registry metadata, release metadata, tagged source contracts and local backlog records. Runtime acceptance remains work for the new sprint backlog. Existing release/delivery evidence must stay labeled with its original version and date.
