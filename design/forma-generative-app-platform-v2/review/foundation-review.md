# Foundation and input review before the factory run

Reviewed 2026-10-22 by the lead at the head's request, before either design starts. Scope: the two
product definitions in `input/context/`, both preparation packets (this task and
`../../forma-northstar-demo/`), the Forma Stack as delivered (`tasks/forma-stack/as-built-v3.md`,
deliverable v3) and as shipped (`getforma-dev/forma` tag `v3.0.0`, source
`2006423b21fe0f8dd5a510bd09bfe3b8748327a5`, published 2026-10-20T04:09:01Z), the work board
`~/dev/Forma/Tickets/`, and the head's decisions of 2026-10-22 recorded in
`../../forma-generative-app-platform/intake.md`. One small build experiment was run against the
published 3.0.0 packages; its files are under `spike-native-composition/`.

## 1. Verdict

- **The inputs are sound.** Both packets read the controlling app-platform definition correctly,
  preserve the inputs, resolve the two PDFs' conflicts the way the request asks, and are
  consistent with each other through the shared contract (digest unchanged). Nothing in the
  product definition is contradicted. Four small items are not yet carried into the packets
  (section 3); none blocks intake.
- **The stack is a sound foundation.** 3.0.0 is published, authenticated and soak-tested, with a
  0 FAIL gate on its v3 record. Its boundaries are stated precisely and the packets cite them
  correctly (section 4). It provides everything the platform needs from a framework and withholds,
  by design, what the platform must build itself: identity, authorization, storage, jobs and
  tenancy.
- **One decision determines the ticket board and must be made before phase 2:** the app
  representation. The packets recommend a typed app definition rendered by first-party compiled
  components; design A v4's D13 and the six P1 tickets filed this morning (FORMA-237 to 242)
  assume model-written markup rendered by the HTML runtime. The factory must choose once for both
  designs (section 5). The lead's recommendation is the typed definition for the first release,
  with the markup path kept as a later custom-widget tier.
- **Under that recommendation no new framework capability is required for the first release.**
  This was checked, not inferred: a generic data-driven canvas (summary, table, timeline and form
  cards from one typed props object, one island) compiles natively on 3.0.0 as shipped in strict
  mode with no diagnostic (section 4.3). Two existing tickets become dependencies the design may
  assume done: FORMA-232 (configurable route size budgets) and the new FORMA-246 (the native
  composition fixture kept as a framework conformance test). If the head keeps D13 instead,
  FORMA-237 to 242 are the required set and nothing further is missing from them.

## 2. What was read, at which pins

| Source | Pin or state | Used for |
|---|---|---|
| `input/context/forma-app-platform-product-definition.pdf` (25 pp.) | SHA-256 recorded in `source-manifest.json`; identical to `~/dev/Forma/Notes/` original | The controlling product definition, read in full |
| `input/context/forma-platform-product-definition.pdf` (23 pp.) | same | The earlier definition, read in full for conflicts |
| Both packets, all Markdown | working tree 2026-10-22 17:23 to 17:42 | Fidelity, consistency, completeness |
| `tasks/forma-stack/as-built-v3.md` and deliverable v3 | tag `task/forma-stack/v3` | What 3.0.0 is, its retained risks and deferrals |
| `getforma-dev/forma` at `v3.0.0` | `SECURITY.md`, `packages/core/docs/HTML-RUNTIME.md`, `crates/forma-server/src/frame.rs`, `packages/compiler/docs/subset.md`, `docs/DECLARED-PROPS.md`, `docs/ARCHITECTURE-3.0.md`, `packages/core/src/runtime.ts` | The shipped contracts the designs rely on |
| `getforma-dev/forma` `main` | `b4faca0a` (PRs 149 to 151 merged 2026-10-22 after the packets' audit at `753be2ae`) | Nothing after the tag changes a contract cited here |
| `~/dev/Forma/Tickets/` | TRACKER regenerated 2026-10-22 18:17; FORMA-245 the last number | Ticket dispositions |
| `../../forma-generative-app-platform/intake.md` "The head's decisions of 2026-10-22" and `deliverables/v4/md/design.md` 5.3 and 12.2 | tag `task/forma-platform/v4` | H0 to H11, D13's decision record |
| `~/dev/Forma/Notes/FORMA-FOLLOW-UPS.md` | 2026-10-22 00:34 | F1 fAuth track, F2 kmd, hygiene items |
| `~/dev/FormaStackDev/.agents/skills/forma-stack/` | 2026-10-21 | The maintained 3.0 contracts summary |

## 3. The inputs against the packets

The controlling definition's twenty-two sections were checked against `PRODUCT-BRIEF.md`,
`SHARED-CONTRACTS.md`, `VALIDATION.md` and the demo's `SCENARIOS.md`. Everything material is carried:
the four layers; spaces, click and slash navigation; the canvas with linked filtering and the
focused-view alternative; CSV and bounded XLSX with the drop zone and Upload control; the three
creation paths and the prepared core pack; writable internal records with read-only connected
sources; the three sharing operations; the lifecycle and version adoption; multi-organization
isolation; the model policy and outage behavior; the six acts; the two-app completeness test; the
open decisions of section 22. The precedence rule (the app-platform definition governs; the
earlier definition's security-first launch is superseded) matches the request verbatim.

Not yet carried, all minor; add at phase 1 as requirements:

| ID | Where in the definition | What the packets lack | Proposed home |
|---|---|---|---|
| I-1 | §5 "light or dark preference and other accessibility settings"; §7 keyboard navigation | No accessibility requirement beyond keyboard alternatives. The library is first-party, so a conformance target is cheap and testable; design A had WCAG 2.2 AA (N12). | An N requirement on the shell and the component library, checked by the gate |
| I-2 | §9 "It asks a focused question when an unresolved interpretation would materially affect the result" | The composer's clarification step is absent from C08 and the demo acts | C08 and act 3: the composer may ask one bounded question before a draft; a question is not a generation failure |
| I-3 | §10 "An app should define what completion means" | The definition does not require a declared completion state; C04 commands include status transitions but no notion of done | A declared completion state in the app definition, shown in saved work; Northstar's handoff and Spend Review's reviewed queue are its two examples |
| I-4 | §8 "AI can suggest field mappings and sensitive fields" | Sensitive-field suggestion at import is not in C05 or C12 | C05: a suggestion the user confirms; it never classifies on its own |

Two things the packets say that the definition does not: the typed `AppDefinition` and the
transactional record store are the packets' proposals, and they say so. That is correct; the
definition is deliberately implementation-neutral. Section 5 is where the factory settles them.

The demo packet's fixtures recompute (checked by `preparation-checks.json`): 1,140,000 bytes total,
900,000 unmatched, 114,000 bytes per minute; Jordan's 210,000 and 21,000; Spend Review 64,000 cents
and the 45,000-cent queue. The half-open window, exact-match precedence and the integer-cents rule
are the right calls and match the security packet's semantics.

## 4. The stack as foundation

### 4.1 What 3.0.0 establishes

The v3 record is an as-built kind with a 0 FAIL, 2 WARN, 20 PASS gate, independent reception
("advance"), 9 vendor claims (8 confirmed, 1 fixed), 9 cross-check findings (8 fixed, 1 accepted),
final publication authenticated (E88) and a post-final soak of 3,660 active seconds over 15 rounds
with zero faults (E99). Seven packages publish from one tag; the four npm packages' `latest` is
3.0.0 and the crates match. That is a real release, not a version label.

Its retained risks (as-built 13.1) that touch the platform: shared founder authority on publishing
(L-60 deferred; FORMA-116 still Backlog), declared browser props being public by design, one
expensive component defeating responsiveness (no windowing), the seven-package upgrade cost, and
one unexplained task outlier. None blocks building on it. The first is the only one the platform's
own release story inherits; say so in the platform's section 10.

### 4.2 The contracts the platform relies on, verified at the tag

| Fact the packets rely on | Where it is in the tag | Verified |
|---|---|---|
| FMIR is trusted input; every module must come from the application's own build; M9 (untrusted admission) is deferred | `SECURITY.md` forma-ir section | yes |
| The HTML runtime binds only through `mountMarkup(root, trustedHtml(markup), { tier, grants })`; tiers `compiled`, `reviewed`, `unreviewed`; grants `fetch`, `dom`, `navigate`, none by default; `trustedHtml` is not a sanitizer | `packages/core/docs/HTML-RUNTIME.md` | yes |
| `MountHandle` has `update`, `unmount`, `grantsUsed` and no way to push state; `update` keeps a scope's state when its shape is unchanged | `packages/core/src/runtime.ts` interface `MountHandle` | yes (so FORMA-237's premise is correct) |
| `data-fetch` is `[METHOD] url → prop` with a fixed URL and no request body from state | `runtime.ts` `RE_FETCH` and the directive comment | yes (so a markup widget cannot post a typed command by itself; it would `$dispatch` to its host) |
| The preview frame has an opaque origin, `connect-src 'none'`, `style-src 'nonce-N'` only, and answers only `ready`, `resize`, `error` | `crates/forma-server/src/frame.rs` | yes (so FORMA-238's premise is correct, and no data bridge exists) |
| `PageResponse` keeps HTML, fresh nonce, enforced CSP with Trusted Types and `no-store` together; Browser routes are explicit | `SECURITY.md` forma-server section; `docs/ARCHITECTURE-3.0.md` | yes |
| The native subset is finite: declared props by property path, boolean conditions, lists over declared arrays with stable keys, one Element root per island, at most three inlining levels, no `createSwitch`, computed declared values refused (BUILD-610) | `packages/compiler/docs/subset.md` | yes, and exercised (4.3) |
| Declared values: booleans, strings, finite numbers, dates, exact string enums, nested objects, arrays, optionals; head state capped at 1 MiB; Rust `PageProps` binds and finalizes before listening | `docs/DECLARED-PROPS.md`; as-built 8 | yes |
| Route size policy: Forma code at most 28,100 B gzip on any route with JavaScript; total JavaScript 75 KiB, or 365 KiB for a route declared `canvas: true`; not configurable in 3.0.0 | `subset.md` BUILD-819; FORMA-232 | yes, and measured (4.3) |
| No framework shared store across islands (R9 unmet, L-57); no application data cache; no progressive rendering; no JavaScript server renderer | as-built 3.2, Appendix C; FORMA-226, FORMA-229 | yes |
| Nothing merged after the tag (PRs 139 to 151: docs reconciliation, Dependabot, CI reliability, package metadata, packed-resolution tests, symlinked `outputDir` refusal) changes a contract above | `git log v3.0.0..origin/main` | yes |

### 4.3 The experiment: a typed-definition renderer on 3.0.0 as shipped

Question: can first-party renderers interpret a typed app definition natively, with data at first
paint, on the released framework, with no framework change? Method: `npx @getforma/create-app@3.0.0
--template minimal`, the published packages from the registry, the home page replaced by one
`Canvas` island whose only input is a declared `cards` array; each card carries boolean kind flags,
a title, a layout string, and the rows, bars, items and fields of a summary, a table (nested lists
for rows and cells), an SVG timeline (attributes bound to declared strings) and a form (label,
text input or select by declared flags, submit). The island seeds one local signal and binds a
reactive attribute from it. Files, build output and size report: `spike-native-composition/`.

| Result | Value |
|---|---|
| `npm run build:ssr` (strict) | admitted: `home.ir` 4,752 B, 1 island, no diagnostic |
| Route `/` Forma code | 27,674 of 28,100 B gzip; JavaScript 30,116 of 76,800 B |
| Variant: `createShow(() => card.kind === 'table', …)` on a declared enum | refused, BUILD-610 at `app.tsx:34:29` ("computed declared values remain outside the native subset") |
| Rust `#[derive(PageProps)]` for the same shape (nested `Vec`, unit enum with renames, booleans), bind, finalize, render | bound and finalized (2 SSR routes); `GET /` returned 6,752 B of HTML with every fixture value at first paint (summary values, 3 rows of 3 cells, 6 `<rect>`, the select's two choices, one inert state block) under `cache-control: no-store` and the enforced nonce CSP with Trusted Types; browser activation not exercised here (FORMA-246 adds it) |

Consequences for the design, all within the stack as shipped:

- Kind dispatch is declared boolean flags computed by the handler, never an enum comparison in
  the component. The query engine already resolves each view server-side, so this is free.
- Renderer trees are written inline or at most three local components deep (BUILD-403); the
  library is a set of admitted shapes, not a deep component hierarchy.
- Pages paginate: the inert head state is capped at 1 MiB and one island's adoption is one task.
- The canvas route's Forma-code budget has 426 B of headroom with four primitives imported. A
  library importing `createResource`, `createKeyedRows` or the HTML runtime will exceed 28,100 B.
  The HTML runtime alone is 28,337 B (FORMA-237's measurement), so it is lazy-loaded if used at
  all. **The platform therefore depends on FORMA-232** (application budgets with a justified
  override) or on fitting its canvas route under the policy by measurement; the design assumes
  FORMA-232 done and the spike measures the real route.

### 4.4 What the platform must build itself

Identity and session, authorization at every data and command request, organizations and
spaces, the app and work stores, source import and the query engine, the transactional record
store, jobs and quotas, retrieval, the composer and its provider adapter, audit, backup and
restore, and operations. This is exactly the as-built record's stated boundary ("application
database/authentication design are outside this record"), and it is what design A v4 and the
packets already assume. Donors with pins: Fieldwork (`forma-fullstack-demo`, 3.0.0) for the
typed SSR plus island plus JSON API skeleton and its verification script; genui-runtime for the
canvas geometry, keyboard and snapshot code (its `reconcile` host must move to compiled components
or `mountMarkup`); generative-ui for admission, canvas store and binding patterns; vector-studio
for retrieval and refinement. All three prototypes pin core 2.0.1; the legacy repositories are
archived, so their fixes require unarchiving. Port, do not depend.

## 5. The decision before phase 2: how an app is represented

Two candidates, both real, both already designed somewhere in this repository.

**A. Model-written markup (design A v4, D13 reworded).** The model writes HTML with Forma directives
and scoped CSS; admission sanitizes and pins it; the browser mounts it through the HTML runtime at
tier `reviewed` with per-widget grants; a translator derives FMIR for server rendering; the preview
frame shows unreviewed drafts. Requires FORMA-237 to 241 and the 3.1.0 release (FORMA-242); the
head lifted D6 for exactly this on 2026-10-22, with each addition proven in the demo's spike and a
3.0.0 fallback per addition.

**B. Typed app definition (the packets' C04; also the security packet's recommendation).** The model
proposes a validated JSON definition: views over a closed component vocabulary, a bounded query
grammar, record schemas and declared commands. First-party compiled components render it; the
server computes every value. No untrusted markup exists, so no sanitizer, no M9, no translator, no
preview frame; previews are the same renderers over a draft with synthetic data.

| Criterion | A: markup | B: typed definition |
|---|---|---|
| Matches the controlling definition's words | partly: §9 asks for "reusable building blocks" and "new working behavior, not … an unfamiliar visual primitive each time"; §15 notes the stack does not admit untrusted modules | directly: the prepared pack, generation as new comparisons, calculations and forms, honest limits outside the grammar |
| Precise edits (§9: "changing a measure requires its actual definition to change") | enforced by review of a diff of markup; a relabel can look like a change | structural: the measure is a query node; a semantic diff is exact |
| Server-side exact calculation | data comes from approved views, but widget expressions (`data-computed`) can compute; admission must forbid it | all arithmetic is in the query grammar on the server |
| Framework work on the critical path | five P1 tickets, two security-critical (M9 with hostile corpus and fuzzing; a Rust translator with a three-engine parity harness), then a minor release | none; FORMA-232 and the FORMA-246 fixture |
| Security surface | sanitizer gap, grants per widget, frame residuals (WebRTC, self-navigation); TH11/TH22 High/High in the as-built record | validated data and trusted code; the model's text is escaped like any value |
| Expressiveness | anything the directive language renders; the "wow" of novel widgets | the library; a new primitive is an engineering release, which the definition already states as the honest limit |
| Accessibility | per-widget checks at admission | once, in the library |
| Reuse of prior work | 56 seed modules and three prototypes produce markup; the v4 threat model and admission design | the canvas host, retrieval, bindings and admission patterns port; the seed modules do not |
| MCP Apps later | natural: a UI resource is markup | feasible: a bundled renderer plus a definition |
| What the stack's HTML runtime is for | used from day one | unused until the custom-widget tier |

**Recommendation.** B for the first release, with A kept as the second tier: a `custom` view kind
whose body is admitted markup mounted at tier `reviewed` with grants, added when a customer needs a
widget the library cannot express. That tier is exactly design A's mechanism and exactly what
FORMA-237 to 241 build, so nothing designed this morning is lost; it moves off the first-release
critical path. The reasons, in order: the controlling definition describes B; B needs no framework
change, which the experiment verified; B removes the two High/High threat rows from the first
release; and the YC proof (two different apps through one authoring path, section 18 of the
definition) is a composition proof, which B makes structural.

**Cost of B, stated.** The demo's generation moment shows a working app assembled from a library
rather than a bespoke widget; the seed module library is not reused; the "custom widget" story is
a later tier. **Cost of A, stated.** The build cannot start its scenes until two security-critical
framework tickets pass their proofs, and the founder carries a 3.1.0 release during the MVP build.

**What the decision does to the head's decisions of 2026-10-22** (H0 to H11; added to `ALIGNMENT.md`
as a table so the packet is complete either way):

| Head's decision | Under B (recommended) | Under A |
|---|---|---|
| H0 D6 lifted: 3.0.0 plus the 3.1 additions | 3.0.0 as shipped plus FORMA-232 and FORMA-246; the 3.1 additions deferred to the custom-widget tier | as decided |
| H1 server rendering and hydration for generated widgets, public pages first | public pages are outside this release (C14); typed views render natively (4.3); FORMA-239/240/241 deferred | as decided |
| H2 `setState` into a mounted widget | renderers are platform code; no framework API needed; FORMA-237 deferred | as decided |
| H3 linked filtering | kept (C03; definition §7); platform work | as decided |
| H4 agents drive approved widgets | kept as C12 "parameter changes": the assistant sets declared parameters, server-checked, audited, undoable; the MCP channel later | as decided |
| H5 preview renders the CSS that ships | moot: previews are the trusted renderers; FORMA-238 deferred | as decided |
| H6 fAuth as a separate track | kept; the design chooses an identity adapter and may pin fAuth as the first | kept |
| H7 to H9 deferred (tenant keys, second region, money movement) | kept | kept |
| H10 B's simplifications unchanged | superseded by the demo packet's own reductions table | kept |
| H11 kmd separate | kept | kept |

## 6. Ticket dispositions

| Ticket | State | Disposition for the new pair |
|---|---|---|
| FORMA-237 to 241 (3.1 additions) | Backlog, P1 | Under B: deferred to the custom-widget tier; a dated Log line added to each saying so and naming this review; status and priority untouched for the head. Under A: the required set; nothing missing from them (checked against the runtime source) |
| FORMA-242 (release 3.1.0) | Backlog, P1 | Under B: not a first-release dependency; its "pins move to 3.1.0" clause does not apply. Under A: required |
| FORMA-232 (application size budgets) | Backlog, P2 | **A dependency under either option**; the design assumes it done; Log line added with the 27,674 B measurement. Recommend P1 |
| FORMA-246 (new; native composition fixture) | Backlog, P2 | Filed by this review: the fixture of 4.3 kept as a framework conformance test with its documented flag pattern, so a future minor cannot silently withdraw the admission the platform depends on; the design assumes it done |
| FORMA-116 (agents on their own identity, release alerts) | Backlog, P1 | Not a platform blocker; the platform's section 10 inherits the residual and says so |
| FORMA-225, 226, 229, 231 (props generator, caching, browser query cache, cacheable pages) | Backlog | Optional ergonomics; FORMA-229's keyed query cache is useful to the canvas but not required; none assumed |
| FORMA-236, 244, 245 | Backlog | Framework hygiene; not platform dependencies |
| FORMA-189, 192, 143, 147, 149, 150, 152 | Backlog | Stack release and application-migration work outside this pair; ksx and fAuth migrations are not prerequisites (L-54 order unchanged) |

Not filed, with the reason: a `data-submit` or typed-command directive for markup widgets (only
under A; a widget can `$dispatch` to its host today, so the design, not the stack, decides);
server-side filtering through a parameterised `data-fetch` (only under A; under B the renderer
fetches); a host-to-island state channel or shared store (R9; the platform's renderers are its
own code in one bundle, so it builds its own; file a ticket only if the design wants a framework
store).

## 7. Corrections applied to the packets

- `ALIGNMENT.md`: a section mapping H0 to H11 to the new pair, under both options, and a pointer
  to this review and to the stack facts verified in 4.2.
- `STATUS.md` of both tasks: this review in "Read first"; the representation decision named as
  the first phase-2 ruling with the recommendation; FORMA-232 and FORMA-246 recorded as assumed
  dependencies; a dated log line.
- `SOURCES.md`: an addendum with the experiment as evidence S23 and the note that `main` moved to
  `b4faca0a` after the audit without affecting any cited contract.
- Nothing in `SHARED-CONTRACTS.md` changed, so `platform-contract-lock.json` still holds. The four
  I-items of section 3 are requirements for phase 1, not contract edits.
- Not changed: `input/` (never), `review/readiness.md` and `paired-review.md` (the earlier
  reviewers' records), `design.md` (the untouched skeleton).

## 8. Housekeeping found on the way

- The renames of `tasks/forma-platform` to `tasks/forma-generative-app-platform` and
  `tasks/forma-mvp` to `tasks/forma-insurance-demo`, the security packet and both new packets were
  uncommitted working-tree changes. They are committed with this review as a checkpoint; the
  delivery tags keep their old names and still resolve. The renamed folders' `STATUS.md` titles
  still say `forma-platform` and `forma-mvp`; left as delivered history.
- `~/dev/Forma/Tickets/README.md` and FORMA-237 to 242 cite
  `design-doc-factory/tasks/forma-platform/intake.md`, which is now
  `tasks/forma-generative-app-platform/intake.md`; the Log lines added to those tickets name the
  new path.
- Untracked and left alone: `archives/forma-data-canvas-input-gemini-2026-10-20/`, `tmp/`,
  `.claude/`, and one stray PDF under `tasks/dawn-security-home-assignment-v2/deliverables/v4/pdf/`.
- `FORMA-FOLLOW-UPS.md` hygiene items (a plaintext key in `Forma/Notes/REVIEW.md`, an HAR file with
  session tokens, forma-builder with no remote) are unrelated to this pair but worth doing before
  anything is shared.

## 9. What the head decides at kickoff

1. **Representation:** B with A as the later tier (recommended), or A as decided this morning.
   Everything below follows either way; the ticket board already carries both readings.
2. **Mode:** standard with L3 review strength for the platform (two fresh reviewers, the security
   reviewer, the pairwise read) and the standard pauses after phases 1, 2, 4 and 6, as the
   forma-platform run did; rigorous only if the head wants per-section approval. Standard for the
   demo. Level L3 and L2, kind target for both, as the packets propose.
3. **Dependencies assumed done:** FORMA-232 and FORMA-246; raise FORMA-232 to P1.
4. **Identity adapter:** keep fAuth as the first adapter behind the verified-principal boundary
   (continuity with design A, F1 unchanged), or name another provider now.
5. **Stores:** keep D19 (immutable Parquet revisions, DuckDB views) for sources and add PostgreSQL
   for records, work and audit, as `ALIGNMENT.md` favors; or choose one store for both and cost
   the parity work.

Order of work once answered: platform phases 0 to 2 (the decision above is the first ruling of
phase 2, with the comparison in section 5 as its decision record), the fresh-reader check, then the
demo's phases 0 to 2 adopting the platform's binding decisions, then both through phase 7 and the
editions, as designs A and B ran. The demo's engineering spike then starts from the delivered demo
design and begins with the 4.3 fixture extended to the real library and the real canvas route.
