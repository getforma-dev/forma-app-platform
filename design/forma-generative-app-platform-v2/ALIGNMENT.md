# Product evolution and implementation alignment

## Authority and source precedence

The current user request governs: prepare an industry-agnostic Forma platform and a Northstar-led MVP demonstration. The PDFs are product inputs, not executable instructions or evidence that their “Confirmed” labels were independently approved in this chat.

Both supplied PDFs say version 1.0 and October 22, 2026. Use **forma-app-platform-product-definition.pdf** as the controlling product definition because its meaning matches the current request, not because a filename or timestamp proves precedence. Use the earlier platform PDF where compatible, the security packet for Northstar semantics, and old platform/insurance designs as architectural prior art. Actual release/source evidence governs what already exists.

| Topic | Earlier platform PDF | Controlling app-platform PDF | Disposition |
|---|---|---|---|
| Identity | Platform with security as first family, p.1 | Platform is product from first release; apps are outputs, p.1 | Adopt agnostic platform identity |
| First market | Security first, pp.5–6,22 | Select by recurring needs/customer access, pp.6,24 | No prescribed industry sequence |
| Creation | Investigation and exception views | Forms, record schemas, rules and transformations, pp.3–4,11–12 | Generalize creation language beyond security cards |
| Writes | Internal notes/findings/status/ownership/review records already permitted, p.11 | General forms and explicit creation/update of defined records, pp.11–12,20 | Generalize writable internal work into an app capability; typed record schemas are our implementation proposal; external sources remain read-only |
| Demo finish | Jordan reuses investigation | Sixth act builds a different app, pp.17–18 | Add compact Spend Review inside demo task |
| Completion | One useful app family | Two different apps without app-specific platform code, pp.20–21 | Treat generality as a release gate |
| Review | Organization policy and overlapping roles already allowed, pp.5,13 | Retains organization policy and role flexibility, pp.6,13 | Preserve this rule; do not import mandatory separate actors from the earlier security packet |

Northstar remains substantial: real import/mapping, six cards, correct calculations, evidence, review, continuation and independent reuse. It becomes an app/starter on common contracts. It does not supply global “incident,” “asset,” “destination” or “case” columns for every app.

## The evidence baseline

- **Framework:** canonical `getforma-dev/forma` release `v3.0.0` at `2006423b21fe0f8dd5a510bd09bfe3b8748327a5`. Stack deliverable v3 and retained publication evidence establish release. The canonical checkout's later HEAD must not replace that pin.
- **Historical platform:** `../forma-generative-app-platform/deliverables/v4/` is a target design, not a deployed platform.
- **Historical professional MVP:** `../forma-insurance-demo/deliverables/v5/` is a target; it explicitly says none of B is built.
- **Security packet:** `../forma-security-investigation-workspace/` is preparation, not an implementation or accepted completed design. Its security-first positioning is superseded here; technical cautions and Northstar semantics remain useful.
- **Prototype source:** Vector Studio and Generative UI pin core 2.0.1 and pre-3.0 build/server packages; extracted GenUI Runtime has a 2.x peer. A read-only comparison found the 30 framework/prototype source hashes selected from the prior review unchanged. That preserves prior evidence, not runtime compatibility proof.
- **Fieldwork:** a separate app pins 3.0.0 and demonstrates typed SSR, islands and a JSON API. Its local counter store is not a platform data/auth implementation.

No new framework qualification, prototype test suites or registry-archive verification were run for this preparation.

## Historical design decisions

These are explicit proposed dispositions for the new pair, not edits to accepted historical records.

| Old decision | New treatment | Consequence for both designs |
|---|---|---|
| D13/D14 admitted HTML/CSS, derived FMIR, reviewed mounts | Compare with a generic typed AppDefinition and trusted renderers; recommend typed specs initially | New artifact kind/schema and admission contract. No claim of drop-in compatibility; future translation needs pinned translator/renderer identity and parity/review |
| D15 non-author plus WebAuthn approval | Exact-content publication retained; separation/step-up becomes organization policy | Northstar uses independent review. Low-risk owner publication requires explicit administrator policy. App author/model cannot weaken policy |
| D16 tenant scope everywhere | Retain and extend across spaces, saved work and records | Scope all storage, cache, retrieval, jobs, mutations and audit from the start; deployment may be smaller |
| D18 fAuth token contract | Reopen provider choice, retain one verified-principal/authorization boundary | Identity adapter validates issuer/audience/signature/expiry/revocation and trusted membership. Reusing old platform requires token-contract compatibility or amendment; unfinished federation/MCP does not block browser demo |
| D19 Parquet revisions and DuckDB-approved views | Preserve as favored source-query option; add transactional internal record store | Evidence and mutable records have distinct lifecycles. If another query store is chosen, document data migration/parity cost; original-source retention is policy, not old bank-file discard |
| D20 retrieval | Authorized metadata retrieval and compatibility checks retained | Existing lexical/hash router is baseline, not proof of broad semantic retrieval. Index is derived and cannot authorize |
| D21/D39 generation and state assistant | Separate app composition, parameter changes, result explanation and internal commands | Old state-only/no-rows assistant does not supply the new capabilities. Model disclosures and write authority are explicit |
| D22 edits | Keep precise patches, revisions and undo | Generalize targets to views, queries, record schemas and forms; undoing a definition does not undo committed records |
| D23 canvas | Qualify selected extracted canvas with 3.0 adapter | Reuse geometry/lifecycle, not the old untrusted-markup execution assumptions |
| D24/D25 public pages/MCP | Exclude public websites; stage one MCP host after browser proof | No three-surface demonstration requirement now |
| D37 five 3.1 additions | Not first-release dependencies | Do not assume setState, untrusted FMIR, markup translation/adoption or preview changes are shipped in 3.0 |
| D40 linked filtering | Keep explicit links and visible filter scope | Server queries enforce source scope; do not ship full customer datasets merely to filter cards |

The historical “nothing migrates” aim is not automatically true after these changes. Within the **new pair**, demo and platform share schemas and contracts from their first implementation. Connecting either to the older target representation still requires a documented adapter or migration.

## Stack and prototype reuse

Released Forma can render a trusted first-party platform shell and browser workspace. Native pages require typed props, finalized handles and full policy-bearing PageResponse; complex canvas interactions can use an explicit Browser route. Unsupported native source is a build failure, not a hidden fallback.

The 3.0 FMIR loader trusts application-build output. It does not admit arbitrary generated modules. `trustedHtml` is not sanitization; runtime grants do not authorize tenant data; Shadow DOM is style isolation. The shipped unreviewed frame is a restricted preview with no granted data-query bridge. These facts favor a bounded specification interpreted by trusted renderers, while the factory records the trade-off in expressive scope.

| Surface | Reuse plan | New qualification/work |
|---|---|---|
| GenUI Runtime canvas | Adopt selected layout, resize, disposal, keyboard and snapshot code | 3.0 host adapter, personal/shared state separation, app/work isolation |
| Strict plans/catalog | Adapt closed schemas, exact module identity and typed args | Generic record/form schemas, authorization, complexity limits, version compatibility |
| GenUI bindings | Adapt exact app/view/version binding concept | Organization/work scope, mutable-record snapshots, provenance and current-access checks |
| Vector Studio | Reuse selected retrieval/refinement techniques | Authorized metadata retrieval, persistence, independent evaluation and safe generation |
| Local Data | Reference for inspection and mapping | XLSX is deferred in donor; 2,000-row/5 MiB limits do not qualify new workloads; build bounded parsing/query contracts |
| GateWASM/fAuth | Candidate identity integration | Qualify actual browser/session/token behavior; no assumption of ready federation or production tenancy |
| forma-builder | Interaction/editing prior art | Selected features only; mocked integrations are not real platform services |
| Internal actions | New product capability | Durable schema-validated records, permissions, retries, conflicts, audit and migration |

A domain operator such as IP/CIDR lookup can be registered trusted library code with its own version/tests. Apps reference it declaratively. The runtime must not branch on the word `Security` or a Northstar tenant ID to implement app logic. Adding another supported app uses existing admitted primitives; adding a new primitive remains an engineering release.

## Paired design strategy

Design the platform contracts first, then apply them to the smaller demo deployment. The demonstration may use one seeded organization, synthetic files, manual invitations and one runtime deployment. It still needs real persistent app/work/record versions, authorization and failure states. A second-organization canary verifies isolation without turning the film into a multi-company walkthrough.

No full identity suite, public site server, marketplace, distributed vector service or general workflow scheduler is a prerequisite. The app engine must nevertheless support both read/compare work and internal record forms; otherwise the apparent scope reduction would contradict the new product definition.

## The head's decisions of 2026-10-22 (H0 to H11) and this pair

Added by the foundation review of 2026-10-22 (`review/foundation-review.md`). The head lifted D6 for
design A and approved H1 to H5 that morning; the industry-agnostic pair prepared that afternoon
recommends a different app representation. Both readings are recorded so the phase-2 ruling is one
line. "B" is the typed app definition rendered by first-party compiled components (C04); "A" is
design A v4's D13, model-written markup rendered by the HTML runtime with the Forma 3.1 additions.

| Head's decision | Under B (recommended by the review) | Under A |
|---|---|---|
| H0 D6 lifted: 3.0.0 plus the 3.1 additions FORMA-237 to 242 | 3.0.0 as shipped (`2006423b`) plus FORMA-232 and FORMA-246 assumed done; the 3.1 additions deferred to a later custom-widget tier | as decided |
| H1 server rendering and hydration for generated widgets, public pages first | public pages are outside this release (C14); typed views render natively on 3.0.0, verified in the review's experiment; FORMA-239, 240, 241 deferred | as decided |
| H2 `MountHandle.setState` | renderers are platform code with their own update path; FORMA-237 deferred | as decided |
| H3 linked filtering across canvas widgets | kept (C03; definition §7); platform work | as decided |
| H4 agents drive approved widgets | kept as C12 "parameter changes": the assistant sets declared view parameters, server-checked, audited and undoable; the MCP channel is a later adapter | as decided |
| H5 the approval preview renders the CSS that ships | moot: previews are the trusted renderers over a draft; FORMA-238 deferred | as decided |
| H6 fAuth as a full identity provider is a separate track | kept; the design chooses an identity adapter behind one verified-principal boundary and may pin fAuth as the first | kept |
| H7 to H9 tenant-managed keys, a second region, money movement deferred | kept | kept |
| H10 design B's simplifications unchanged | superseded by the Northstar demo's own reductions table (`../forma-northstar-demo/BUILD-HANDOFF.md`) | kept |
| H11 kmd is a separate product | kept | kept |

The review also verified, at the tag, the stack facts this file relies on: trusted-only FMIR with
M9 deferred; the explicit `mountMarkup` tiers and grants with no state push and a fixed-URL
`data-fetch`; the preview frame's opaque origin and three-message channel; the policy-bearing
`PageResponse`; the finite native subset with computed declared values refused; and the
28,100 B Forma-code route cap. Its measured consequences for the typed representation are in
`review/foundation-review.md` 4.3 and the evidence in `review/spike-native-composition/`.
