# ADR-001 — Adopt published Forma 3.2.0

Date: 2026-10-28. Owner: GenUI. Ticket: APP-001. Scope: first implementation baseline, not full platform or framework release acceptance.

## Decision and evidence

Use published core/compiler/build 3.2.0, exact manifest pins and committed lockfiles. Use forma-ir/forma-ir-derive/forma-server exactly 3.2.0. The generator is `@getforma/create-app@3.2.0`. Eight downloaded registry archives, including optional router, match registry integrity/checksums; `v3.2.0` resolves to `fc76ea8531f0e3a85d0224087384e41f52040e3c`. Registry archive hashes and tagged source identity are separate evidence, not a claim of reproducible package compilation. [Registry report](../../evidence/APP-001/registry.json) retains both. npm manifests do not publish gitHead.

Start with ordinary document navigation, typed Rust/FMIR pages and named islands; do not install router in the initial app. A future router decision needs native-group lifetime and whole eager-graph measurements. Use declared props, server-derived presentation flags, stable list keys and `activateIslands`. Model output remains typed data. Strict unsupported-source refusal never becomes an implicit Browser fallback.

Require SSR on build/dev paths and assert required route modes before binding, finalizing and listening. Return complete `PageResponse` through its normal response adapter. HTML, fresh response nonce, CSP and no-store must remain together. The minimal derivative demonstrates raw count 7, a no-JS page and HTTP policy; it does not establish full canvas browser adoption. [Harness](../../spikes/APP-001/README.md), [HTTP report](../../evidence/APP-001/http.json).

## Dispositions against the frozen 3.0 design

| Historical assumption | Current disposition |
|---|---|
| 3.0.0 source and package set | Superseded for new implementation by the exact 3.2.0 set above at Victor's request. Historical measurements remain 3.0 evidence. |
| N3 Forma contribution <=28,100 B gzip | Keep this application target pending measured APP-003/FORMA-232 disposition. The framework enforces 28,420 B in 3.2; this ADR does not automatically relax the app target. The minimal harness measures 26,514 B estimated Forma / 28,164 B total, below both. It is not the complete primitive library. |
| Ordinary page/canvas budgets | Unchanged 75 KiB / 365 KiB total gzip; canvas classification does not enlarge the Forma ceiling. Include emitted worker accounting as the build reports it. |
| Retained native navigation absent | Available but deferred: fixed group ceiling 40,708 B Forma / 64 KiB total, closed eager graph. `canvas: true` does not override it. No router adoption inferred from matching package availability. |
| Native component limits | Three inlining levels and closed declared expression subset remain. Preserve boolean presentation flags, bounded nested lists, canonical SVG attributes and form nesting rules. APP-003/246 must reproduce concrete admitted/refused combinations; inspecting the subset guide is not passing them. |
| Head-state capacity | Browser island state remains capped at 1,048,576 UTF-8 bytes, not characters. Keep pagination/500-row app bounds and tighter definition limits. APP-003 must measure complete state and boundary failures. Native navigation owners have a separate 1 MiB cap and are not adopted here. |
| Server security boundary | Typed bind/finalize before listening and complete PageResponse remain mandatory. Starter defaults are dual mode; the fixture explicitly rejects unexpected Browser/missing required routes. |
| FORMA-232/246 assumed complete | Open, FormaStack-owned, blocking APP-003. 232 may clear this consumer by measured no-override-needed disposition while its broader feature remains open. |
| Hosted fAuth assumed at historical revision | Current deployment unverified. 247 blocks APP-006; 248 gates real organization sign-in at APP-058. |
| HTML-runtime work 237–242 | Not an initial dependency: approved typed definitions/trusted renderers remain the representation. |

Source checks use tagged `packages/build/src/route-size.ts`, `packages/compiler/docs/subset.md`, `packages/core/src/dom/island-state.ts`, and `crates/forma-server/src/{lib.rs,response.rs,navigation/owners.rs}`. See [preparation baseline](FRAMEWORK-BASELINE.md) for linked upstream source. In the subset guide some generic diagnostic prose says BUILD-610 even under BUILD-403; runtime refusal names must be measured in APP-003/246, not inferred from that prose.

## Tooling and limits

Measured on macOS arm64, Node 22.21.0, npm 11.6.2, Rust/Cargo 1.93.1 and Python 3.11. The generator declares Node >=22 and server Rust >=1.85; qualification uses the explicit newer Rust toolchain and does not claim an MSRV run. Cargo concurrency is 3. CI reproduces locked build/tests on Linux; its actual result is recorded in the ticket.

Only the APP-001 startup guards/HTTP behavior are qualified. Native full-library adoption in three browsers, auth, databases, query/parser bounds, application latency and release trials retain their own gates. Revert this fixture/ADR to roll back; no stored data or deployment is changed.
