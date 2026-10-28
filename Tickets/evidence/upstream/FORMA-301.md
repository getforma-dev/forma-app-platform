# FORMA-301: Forma Router API for smooth navigation, persistent layouts and native SSR

| Field | Value |
|---|---|
| Epic | E2 |
| Status | Done |
| Priority | P2 |
| Type | Framework feature: developer experience and navigation |
| Release | Stable [Forma 3.2.0](https://github.com/getforma-dev/forma/releases/tag/v3.2.0) published 2026-10-27 at 18:22:07Z from fc76ea8531f0e3a85d0224087384e41f52040e3c. Tagged workflow40553136904 passed all 11 jobs; all five npm and three Rust packages published. Stable website deployed and accepted on OVH from 9f99455a4956083a75b1401d805fce69aae8183d |
| Repos | getforma-dev/forma (framework/packages/starters), getforma-dev/getforma-site (authorized ecosystem rollout), getforma-dev/getforma-infra (proxy and OVH release) |
| Decision | Founder request and clarification 2026-10-26: smooth navigation between native Rust SSR pages and their islands is an explicit goal; full-document loads are documented fallbacks. Revisit v2 section 3.2's default-routing non-goal without replacing the Rust SSR/islands default; preserve v3 R5/R7/R8 and D11/D14/D18 |
| Depends on | Complete for this release: published stable package set and actual stable site acceptance. Maintainer explicitly waived extra N7, active soak, extra RC, staged rollback exercise and duplicate full website CI for the docs/test-only successor; zero pass/soak credit. Security, correctness, integrity and trusted publication retained. FORMA-224 and FORMA-355 remain separate |
| Related | FORMA-226 (separate caching/streaming assessment); website PR8 navigation implementation |
| Assignee | Codex (implementation and qualification) |
| Branch / PR | Framework PR207 and [PR208](https://github.com/getforma-dev/forma/pull/208) merged; exact-main CI40549601525 passed 19 jobs/one expected skip; tagged P17 passed 75/75 in 1324.509s. Website [PR16](https://github.com/getforma-dev/getforma-site/pull/16) and [PR17](https://github.com/getforma-dev/getforma-site/pull/17) merged; deployed source9f99455a. Final local93/production93/live portal-docs27 passed. Original website CI40560701657 failed three stale test assertions; corrected case3/3 passed; duplicate PR17/main workflows cancelled with no pass credit |

## Final release disposition — completed

[Framework documentation PR209](https://github.com/getforma-dev/forma/pull/209)
merged at `4c4696c5d1ad6a89e1faf0fd0a8ac510c7194694`. Public entry points now use
3.2 as the launch baseline; historical releases and all existing changelogs are
preserved. Current guidance explains setup, routing, islands and deployment.

The [stable publication handoff](../../work/FORMA-301/qualification/ecosystem/stable-release-v3.2.0/handoff.json)
authenticates the published eight-package set, final-source and tagged
qualification, and trusted publication. The [actual production receipt](../../work/FORMA-301/qualification/ecosystem/stable-site-rollout/production-closeout.json)
accepts website source `9f99455a4956083a75b1401d805fce69aae8183d`, release
`9f99455a4956-20261027T185515Z-amd64` and image
`sha256:c589745f7a8b5289eefce1e4952b203a0675b39030f41ad8bfa1313a11f44dbc`.
OVH activated deployment `9f99455a4956-20261027T185515Z-amd64-20261027T185923Z`
at **2026-10-27 18:59:49 UTC**. Normal production verification used no identity
bypass and confirmed the actual running image/source; both services are healthy.

- Final immutable local image: **93/93** checks passed; graceful exit 0.
- Actual production: **93/93** checks passed.
- Live portal/docs: **27/27** checks passed across Chromium, Firefox and WebKit,
  without skips, retries or failures.
- Three public ZIPs match **272 source entries**; six sampled public pages show
  stable guidance without the old preview banner or version chooser.
- Published 3.2.0 minimal starter: install, frontend/SSR build, Cargo and **10/10
  HTTP** checks passed; the owned server was stopped.
- Previous 8f deployment is retained for rollback. No staged rollback exercise is
  claimed.

The original c6be [Website CI40560701657](https://github.com/getforma-dev/getforma-site/actions/runs/40560701657)
retains **342 passed, six intentional skips and three failed** browser assertions:
all three expected the first code block to be the scaffold command after
prerequisites had become the first block. The focused repair selects the scaffold
command and preserves exact clipboard-content equality; it passed **3/3**.
The build passed; content checks passed **42/42**. Duplicate PR17/main workflows were
cancelled under the explicit streamlining decision, with no pass credit. The
[CI dispositions](../../work/FORMA-301/qualification/ecosystem/stable-site-rollout/final-ci-dispositions.json)
are retained; **no full website CI green result is claimed**.

**3.2 is the public launch baseline.** Current onboarding covers setup, routing,
packages and deployment. Existing public changelogs and historical migration and
release records remain intact. Original failures preserve their source identities.
N7, active soak, extra RC publication and the staged rollback exercise remain
explicitly waived, with no pass or future-soak claim. FORMA-224 historical
inventory recovery and FORMA-355 optional native group code splitting remain
separate work. Original physical-device and BFCache evidence retains its actual
scope; no new manual session or universal BFCache claim is made. The final D14
retains runtime/security mutation applicability with the changed-document-read
exception; it does not claim a fresh full mutation campaign.

This ticket is **Done**. Historical checkpoint text and unchecked umbrella items
below preserve their earlier state; the actual final outcomes and explicit
waivers above determine closure. The frozen design, research and evidence are
not rewritten by this closeout.

## Historical release checkpoint — 2026-10-27 05:13 UTC

Historical checkpoint **2026-10-27 05:13 UTC** (later outcomes appear in the current fields and log). This remains an **in-progress** ticket: accepted
implementation and actual RC.1 registry publication do not establish a complete
release record, stable release or the authorized live website rollout. The failed
RC.1 run and its immutable tag remain part of the record. RC.2's first tagged
attempt also remains a valid failure: 70/71 recorded P17 checks passed, N1's
nominal 300-node throughput was 3,290,708.717 nodes/s against a strict
3,383,518.405 floor, and downstream publication/record were skipped. The
[prospectively bounded retry](../../work/FORMA-301/qualification/ecosystem/hosted-3b6c7766/tag-40450689494/bounded-full-retry-decision.json)
passed the complete unchanged P17 once more (75/75, 1,236.041 s); no third
performance retry was needed. Both publishers and the normal record then
completed successfully. All eight archive bytes and router OIDC are authenticated. Different hardware is evidence, not proof of the failure's cause.
The original tag clock and failed samples are retained.

A separate [registry finding](../../work/FORMA-301/qualification/ecosystem/hosted-3b6c7766/router-placeholder-independent-assessment.md)
authenticates npm's 364-byte `@getforma/router@0.0.0-stage` placeholder. SR2
currently refuses its missing record. The successor implementation must attach
an exact, inventory-only receipt to prospective RC.3's ordinary DS12, with a
verified pointer and actual recording identity. It must preserve router baseline
`[]`, ordinary package qualification and all immutable prior records. No explicit
staging action or exact npm backend mechanism has been established.

## Why

Forma users need a supported navigation API with the practical responsibilities
served by React Router: URLs and route matching, links and programmatic navigation,
nested/persistent layouts, history, loading/error handling and predictable page
lifetimes. React component compatibility and a drop-in React Router API clone are
not requested. Select the first public API deliberately rather than promising
complete feature parity with a mature framework.

For applications opting into the router, ordinary navigation between supported
native Rust SSR routes must keep the browser document and shared layout alive,
receive the destination's Rust-rendered HTML and typed state, and activate its
islands correctly. Smooth native navigation is a required product outcome, not
only a design investigation. Full-document navigation is a documented fallback
for exceptional or unsupported cases, not the normal native routing path.
Direct entry, explicit browser refresh, opting out and no-JavaScript use still
load complete server-rendered documents.

The website previously used complete-document navigation between every guide,
API page and other portal route. Website PR8 now preserves the shared shell,
fetches generated HTML, restores history/scroll/focus and disposes page features.
That implementation is site-owned code, not a reusable published Forma router.
Developers should not have to copy it and rediscover its navigation races and
cleanup rules to obtain a professional multi-page experience.

Observed baseline: website main `c69fa96cc4c71fb846d8982e5702aac8a48839e5`,
using published Forma 3.0.0. Its 108 hosted navigation/motion/mascot checks passed
on the deployed artifact. This validates the website's scope, not arbitrary native
SSR transitions or a future framework router. Inspect the then-current framework
release before implementation; do not assume this 3.0 observation describes every
later prerelease.

## Design evidence and boundaries

- [Current formal delivery index](../../../design-doc-factory/tasks/forma-stack/deliverables/README.md)
  identifies **v3, Forma 3.0 as built (2026-10-20)** as current. The current
  [51-page PDF](../../../design-doc-factory/tasks/forma-stack/deliverables/v3/pdf/design.pdf)
  discusses typed server binding in section 6 (page 15) and the public API in
  section 9 (page 24). `output/design.pdf` still matches delivered v2; it is not
  the latest formal record.
- [Frozen v2 design, section 3.2](../../../design-doc-factory/tasks/forma-stack/deliverables/v2/md/design.md)
  (PDF page 4) excludes “Whole-page client rendering and client-side routing as the default.”
  It explicitly says a console needing state across navigations would reopen the
  choice. This excludes the default, not every optional browser router.
- [V3 record](../../../design-doc-factory/tasks/forma-stack/deliverables/v3/md/design.md),
  sections 4.1, 5.3, 9 and 11.1, documents native links/forms, typed server pages,
  declared island activation, complete response policy and deployment skew. It
  does not specify an integrated public browser router. Server route matching and
  build route configuration do not provide that browser navigation API.
- V3 D11 records no supported public fragment-rendering consumer API. D14 keeps
  HTML and response policy together in `PageResponse`. Do not treat the private
  fragment helper or arbitrary HTML swapping as an already-supported native
  navigation protocol. The optional asset worker also does not cache page navigation.
- [Website PR8](https://github.com/getforma-dev/getforma-site/pull/8), especially
  `src/portal/navigation.js`, `src/portal/page-behaviors.js` and
  `tests/portal-navigation.spec.mjs`, supplies concrete implementation experience.
  Its native `/lab/` routes deliberately retain document navigation.

Keep the accepted v3 as-built PDF/source historical. Record the proposed router
in a reviewed design addendum or successor design before framework integration;
mark proposed and shipped behavior separately.

## What changes

1. **Design the public API and route lifecycle.** Select package/subpath ownership
   (for example `@getforma/router`, name not yet committed), route definitions,
   typed parameters/search/hash, declarative links, programmatic navigation and
   nested layouts/outlets. Define layout/state preservation, route-local disposal,
   push/replace/Back/Forward, redirects, not-found and error/loading states.
   Any names shown in the design are provisional until reviewed and implemented.
2. **Implement optional browser navigation.** Support explicit browser-rendered
   routes and generated-HTML routes with deterministic cancellation and latest-
   navigation ownership. Preserve native anchor semantics, modified/new-tab clicks,
   downloads and form behavior. Bound intent prefetching and caching; respect
   data-saving preferences. Do not introduce permanent caching of private HTML.
3. **Design, implement and prove smooth native SSR navigation.** Establish a
   supported partial-navigation protocol so ordinary navigation between supported
   native routes preserves the document and shared layout while receiving the new
   route's Rust-rendered HTML, typed state and matching assets. Dispose of departing
   route islands and activate the destination's islands by adopting their rendered
   DOM. Preserve CSP/nonce policy, complete `PageResponse` semantics, build identity,
   integrity and island ownership. Direct entry and refresh must retain complete
   Rust-generated HTML and typed props. Do not replay scripts from fetched HTML or
   remount server-marked roots.

   Document full-page fallback conditions, their technical reasons and user-visible
   effects, including any loss of in-memory state. Examples may include incompatible
   builds or response policies, unsupported destinations and unrecoverable transition
   failures; the reviewed design must define the actual conditions and tests. A
   fallback must not bypass response policy or retry indefinitely. Document loads
   alone do not satisfy native integration. A browser-only first stage must be
   labelled as such; this umbrella ticket stays open until smooth native transitions
   are implemented and verified against the supported contract.
4. **Define data and mutation boundaries.** Specify cancellation, status/redirect
   propagation, pending/error states, form submissions and revalidation against
   explicit application-owned Rust endpoints. Authentication and authorization stay
   on the server. No implicit JavaScript server runtime, arbitrary remote code
   execution, automatic database persistence or fabricated client authority.
   Progressive streaming and general data caching remain FORMA-226's separate scope.
5. **Package and teach the supported path.** Provide a documented API, migration
   from plain anchors where appropriate, and working TSX plus Rust examples.
   Include a starter and a bounded website pilot only when this ticket is selected.
   Review reuse of the site's code; its fixed route allowlist and singleton page
   shell are not automatically a general router architecture.

## Acceptance tests

### Current implementation evidence

The supported stable package implementation is published and accepted under the
[final D14](https://github.com/getforma-dev/forma/releases/download/v3.2.0/d14-report.json)
and actual tagged qualification. Native routing/islands, recovery, ownership and
refusal evidence retain their finite supported scopes. Original Mac/iPhone and
public Chromium BFCache observations do not become new stable-tag device tests
or universal eligibility claims. Changed core documentation reads were reviewed
separately; no all-read-bytes-identical or fresh full-mutation result is claimed.
Only the remaining website CI and production outcome are pending in the current
disposition above.

### Historical implementation evidence and umbrella checklist

The evidence and checkbox states in this subsection are preserved from their
original release checkpoints. Later publication and scoped acceptance do not
rewrite those observations or promise universal behavior.

The implementation merged through PR #204; release recovery uses the branch above:
optional `@getforma/router`, optional `@getforma/core/navigation` ownership,
compiler/build navigation groups, typed Rust owner/action responses, and the
`native-router` starter. It supports browser-rendered, registered generated-HTML
and typed native SSR routes. Core's default entry and existing route size caps
remain unchanged. The coordinated 3.2 release inventory contains eight
packages and three starters; historical 3.1.0 evidence remains unchanged.

The [implementation record](../../work/FORMA-301/forma/docs/design/router/README.md)
preserves all 49 research snapshot files and maps all 105 questions into decisions.
The [current 57-requirement qualification ledger](../../work/FORMA-301/forma/docs/design/router/qualification-3.2.md)
and [49-file evidence index](../../work/FORMA-301/forma/docs/design/router/evidence/acceptance-3.2.json)
distinguish source, browser, packed and remaining manual/release evidence. The
[historical ledger](../../work/FORMA-301/forma/docs/design/router/qualification.md)
retains earlier native/site pilots and failed attempts. The later
[historical pretag D14 report](../../work/FORMA-301/qualification/ecosystem/independent-review/final-d14/d14-report.json)
records the review of its exact input tree. It missed the subsequent changelog
failure, now acknowledged as D14-3.2-010; it does not clear corrected RC.2 source.
Umbrella checkboxes below remain open where any required subcondition is still
unverified; implementation progress must not silently become a release claim.

Observed at this checkpoint:

- Earlier product/harness source `4c59db07` passes all five PR CI planes and
  [hosted P17 40423336544](https://github.com/getforma-dev/forma/actions/runs/40423336544):
  **75/75**, all eight archives and three starters, 1,240.829 seconds within the
  unchanged 1,800-second limit. Those same archives pass **90/90** native cases and
  **12,287 / 12,288 B** incremental, **40,715 / 65,536 B** whole-group gzip limits.
- The accepted 862d supplementary campaigns retain their actual identity: 117
  history cases, 1,500 resource cycles plus 18 causal negative controls, 44 API/model
  plus two scroll-regression and 45 browser limit cases, 90 strict action/security
  cases, 54 scroll/prefetch cases and all four fixed timing cells. Their adapter,
  model, browser and native scopes are explicitly distinct; no universal leak or
  physical-device performance claim follows.
- The [original full Nightly campaign 40412642649](https://github.com/getforma-dev/forma/actions/runs/40412642649)
  is independently accepted: **326/328 caught, two actual survivors, zero equivalent
  or unqualified**, all 169 security and 98 expression mutants caught, and both clean
  baselines execute 4,320 tests with no skips. Source `98889302` remains the measured
  source. Current RC.2 applicability preserves 614 of 617 declared inputs and all
  20 additional read inputs; only two canonical internal version manifests and
  two literal core README RC.2 pins differ, with no runtime/test/corpus/workflow
  or external dependency change. No fresh mutation execution is claimed.
- The user confirms the six requested Mac Safari keyboard/VoiceOver/Reduce Motion
  tasks with **“ok done all good.”** This supplies no individual speech transcript.
  One separate genuine public no-cache/no-cookie Chromium BFCache restore proves
  trusted persistence, synchronous router-root revocation and one recovery GET.
  Private/native no-store and all-engine eligibility are not established.
- Historical D14 accepted exact input
  `3c10b6d056b1b160db47c7f603693502e78e0c71`; audit-only commit
  `fc22634376965ab048ffb395bf37aa2aa5aa3dc5` recorded that decision. It missed the
  missing exact changelog entry. The actual RC.1 tag at merged `6e5e2e49` passed
  **75/75 P17 checks in 1,247.311 seconds** in
  [run 40433276229](https://github.com/getforma-dev/forma/actions/runs/40433276229).
  All five npm and three crates.io packages were published and independently
  byte-verified, then record job **121967590802** failed deterministically with
  `missing CHANGELOG entry for 3.2.0-rc.1`. No qualified DS12 or RC.1 soak exists.
  The [D14-3.2-010 failure receipt](../../work/FORMA-301/qualification/ecosystem/hosted-6e5e2e49/tag-40433276229/record-changelog-blocker-independent.json)
  preserves that missed preparation check and the failed immutable tag.
- Guard commit **3da2bfd2** reuses the existing record parser to reject missing or
  empty exact-version changelog entries from committed source before publication.
  The retained negative controls reproduce the original missing refusal; the
  [corrected focused suite](../../work/FORMA-301/qualification/ecosystem/changelog-preflight-summary.json)
  passes **36/36**. Actual RC.2 branch/main checks subsequently passed; tagged
  P17 qualification and publication remain open at the checkpoint above.
  The earlier low-severity repeated live-region wording remains; one-send POST
  uncertainty/retry refusal remains correct.
- RC.2 recovery is implemented through commits **3da2bfd2, 44d98218, 36004d89,
  9a4c24fb, cf72586f and bdc691e1** in PR #205. Fresh-context independent
  review found **D14-3.2-011**, where authenticated proof bytes were treated as
  opaque. Nine negative controls demonstrate the semantic-validation correction
  at bdc. The broader [local release suite](../../work/FORMA-301/qualification/ecosystem/rc2-release-integration.log)
  passes **529/529**; this is local release-tool validation, not publication evidence.
- Original candidate artifact **12705944946**, run **40441486845**, source
  **bdc691e1346dc76d0954e5aaac66242bf7479ac2**, is authenticated as a development
  dry run. Fresh native cases pass **90/90** and paired size passes at
  **12,287 / 12,288 B** incremental and **40,715 / 65,536 B** group gzip. The
  [retained summary](../../work/FORMA-301/qualification/ecosystem/hosted-bdc691e1/native-and-size-summary.md)
  binds those actual archives and unchanged caps; no published RC.2 or physical
  device claim follows.
- Original [bdc CI 40441474914](https://github.com/getforma-dev/forma/actions/runs/40441474914)
  retains three real README-pin failures in check, Node 22 and Node 24, followed
  by an overall cancelled conclusion after the successor. Provider/log evidence
  remains intact. The five-Markdown-file repair **5b44312e** passes **20/20** docs
  tests and all four actual PR CI planes:
  [CI 40442597606](https://github.com/getforma-dev/forma/actions/runs/40442597606),
  [security 40442597660](https://github.com/getforma-dev/forma/actions/runs/40442597660),
  [scaffold 40442597559](https://github.com/getforma-dev/forma/actions/runs/40442597559)
  and [budgets 40442597599](https://github.com/getforma-dev/forma/actions/runs/40442597599).
  The CI workflow has 19 successful jobs and one intentional published-scaffold skip;
  both candidate scaffold jobs pass. Original [P17 rehearsal 40441486845](https://github.com/getforma-dev/forma/actions/runs/40441486845)
  passes **75/75 in 1,266.223 seconds** against 1,800 seconds, measured on bdc
  without repinning. The packed core README changes archive identities; final
  tagged archives still require their own qualification.
- Fresh [independent prepared-RC D14](../../work/FORMA-301/qualification/ecosystem/independent-review/rc2/final-d14/accepted-review.md)
  accepted exact **12476b5c14e00414f3c6e4c52215b65a40efadf2** at
  **2026-10-27 03:47:10.778 UTC**, with no blockers; its
  [validation](../../work/FORMA-301/qualification/ecosystem/independent-review/rc2/final-d14/validation.json)
  binds that input. All non-audit candidate rows were evidenced and done there.
  The [independently verified application](../../work/FORMA-301/qualification/ecosystem/independent-review/rc2/audit-application-independent-verification.json)
  at pushed **c94c78dd** contains only the three audit records and audit-row
  completion. PR #205 merged after exact final branch qualification passed in
  [CI 40446522943](https://github.com/getforma-dev/forma/actions/runs/40446522943),
  with [security 40446522939](https://github.com/getforma-dev/forma/actions/runs/40446522939),
  [scaffold 40446523073](https://github.com/getforma-dev/forma/actions/runs/40446523073)
  and [budgets 40446523044](https://github.com/getforma-dev/forma/actions/runs/40446523044)
  as the corresponding passing planes. Exact main CI **40448662104** also passed
  at **3b6c7766**; its R17 command took 625 s, not a 20-run distribution.
  Earlier 5b results do not substitute for these exact-source checks. The review accepts
  documentation/closure applicability, not a later-source execution or archive
  hash projection. This is no full-conformance, stable, physical-device or
  deployed-site approval.
- Medium nonblocking [D14-3.2-012](../../work/FORMA-301/qualification/ecosystem/independent-review/rc2/r17-timing-finding.json)
  records the successful **743-second R17 functional build/test/parity command
  with restored caches**, and no current 20-run N7 distribution. Frozen N7/G6
  **under 600 seconds at nearest-rank p95 over 20 complete runs** is unchanged
  and unqualified for 3.2. One observation proves neither a p95 failure nor a
  comparable product regression. This maintainability debt is nonblocking for
  prepared-RC review only: no timing waiver or stable/full-conformance approval.
  An actual current 20-run campaign or explicit applicable founder disposition
  is required before claiming conformance; historical 3.0 L-67 evidence does not transfer.

The completed observations retain their recorded scopes. The broader umbrella
items below remain subject to their physical-device, consuming-application and
final release/live acceptance conditions; the exact 57-row ledger governs scope.

- [x] Reviewed design states the supported MVP, later features, compatibility and
  packaging/version policy. Existing SSR/islands and ordinary navigation remain
  usable without opting into the router; static no-JavaScript routes gain no
  mandatory router bundle. The supported native route contract and documented
  fallback matrix are explicit; a design that sends every native transition through
  a full-page load does not meet this ticket. No change to existing performance
  gates by implication.
- [x] Packed consumer examples cover direct URL entry, refresh, nested layouts,
  route/search/hash changes, redirects/404s, Back/Forward and retained layout state.
  At least one example navigates in both directions between two real typed Rust SSR
  routes with distinct declared islands. Ordinary links and programmatic navigation
  preserve the document and shared layout nodes/state without a full-page load;
  each destination receives fresh server props and its islands adopt the incoming
  DOM and respond correctly to real input. Direct entry and refresh also work.
- [ ] Chromium, Firefox and WebKit tests verify actual document/node identity,
  focus, scroll restoration, accessible pending announcements and reduced motion,
  plus mobile navigation, real input and no-JavaScript links/forms.
- [ ] Rapid/duplicate clicks, aborted/late/failed requests, Back/Forward races and
  deployment/build skew cannot commit stale pages or silently run mismatched assets.
  Deliberate failures demonstrate each refusal/fallback and bounded loading state.
- [ ] Full-page loads during enhanced navigation occur only under the documented
  fallback conditions. Each condition has a test proving the fallback and its
  policy/state behavior; normal supported native transitions have explicit
  no-document-reload assertions. A successful fallback cannot count as a passing
  smooth-transition test.
- [ ] Leaving routes disposes owned effects/listeners/observers/timers/workers and
  islands; repeated route cycles do not multiply handlers or leak active resources.
  Persistent layout state and unsaved-state policy behave as documented.
- [ ] Real native tests preserve response/CSP/no-store policy, pre-activation user
  input and ownership. User/session changes cannot expose another user's cached
  content; protected data never becomes client-authorized. Unsafe/external URLs and
  invalid response/build/state inputs take the documented refusal/fallback path.
- [x] Measure incremental router gzip, cold entry, warm navigation, responsiveness
  and resource use on fixed routes and a mobile profile; report overhead and
  document-navigation baseline. Choose prospective budgets before the campaign,
  retain raw failures, and do not claim superiority from unrelated benchmarks.
- [ ] Examples/tutorial commands run in CI against the packed candidate. Independent
  review, supported-package release checks and updated design/docs precede any
  stable public capability claim. The website pilot is not a substitute for this.

The checked packed-consumer item is supported by 90 actual installed-archive
cases across Chromium, Firefox and WebKit. The measurement item is supported by
the paired archive size check and the four-profile timing campaign, including
document-navigation controls, descriptive cold samples and a bounded heap
observation. These do not establish zero resource leaks, physical-device performance or
universal speed superiority. The separate six-task Mac confirmation supplies
bounded human acceptance, with the known repeated warning text still disclosed.
Remaining device and release obligations stay open in the ledger.

## Historical remaining release and ecosystem acceptance

This section preserves the earlier RC2 preparation state. Its unpushed website,
physical-device, publication and soak requirements describe that checkpoint;
the current stable disposition above supersedes the earlier plan.

RC.1 publication succeeded for all eight packages, while its record job failed.
Corrected RC.2 recovery in merged PR #205 has accepted independent prepared-RC
D14 at 12476b5c and verified audit application at c94c78dd. Exact branch/main CI
passed. Actual tagged P17 and its own trusted publication/normal DS12 also
completed and are independently authenticated. The archival RC.1 receipt mechanism is explicitly inventory-only: RC.2's
normal record job is implemented to attach it for
reconciliation. It will not be an ordinary RC.1 DS12, a P17 baseline exception or
support/soak authorization. The hosted RC.2 attachment is authenticated; no RC.1 tag
rewrite or successful-workflow claim follows.

The bootstrap GitHub secret was verified absent at **2026-10-27 02:08:21 UTC**;
the temporary token was verified absent from the actual owner list at **02:30:55 UTC**.
No exact revocation time is inferred. Trusted publisher
**444b6303-11f2-4327-8837-8f009fe445fa** is saved for `getforma-dev/forma`,
`release.yml`, environment `npm-publish`, with publish permission. Actual npm
Settings verifies `mfa=publish`, requiring 2FA and disallowing bypass tokens.
Actual RC.2 normal OIDC publication is independently authenticated, and all
three installed site npm graphs pass signature verification. Enrollment alone
was not treated as publication proof. The
[enrollment](../../work/FORMA-301/qualification/ecosystem/published-3.2.0-rc.1/router-trusted-publisher-enrollment.json),
[secret-removal](../../work/FORMA-301/qualification/ecosystem/published-3.2.0-rc.1/bootstrap-secret-removal.json)
and [token-absence](../../work/FORMA-301/qualification/ecosystem/published-3.2.0-rc.1/bootstrap-token-absence.json)
receipts retain those separate observations. The live five-package observer and
eight-package registry reconciliation remain operational gates.

The website branch `codex/router-ecosystem` remains unchanged at
**1a088ca5f8df975ac18a40ee2cf0bb78ae6be87a**, not pushed. The canonical getforma.dev
rollout still requires qualified registry pins and final
site/lab/docs/download checks, followed by immutable OVH deployment with verified
proxy policy, build identity, live routes and rollback. The merged infrastructure
policy change has not yet reached production. Physical iPhone HTTPS/gesture
acceptance and the same-line hosted RC soak with **at least 3,600 fault-free
credited active seconds** remain open. Do not move this ticket to complete or
rewrite the frozen factory v1 inputs, delivery or PDF to imply these outcomes.

## Migration and rollback

The original ticket creation authorized planning only. The founder subsequently
accepted the plan, instructed its full implementation and confirmed that it should
follow Forma's release patterns on 2026-10-26. PR #204 is merged; corrected RC.2
preparation is isolated on the recovery branch above.
The founder subsequently authorized package publication and getforma.dev production
deployment, documentation, starters and example downloads. Unrelated application
migrations remain outside scope. Make the router opt-in; opting out restores ordinary document
navigation without losing server handlers or data. Define atomic rollback of the
matching server/build/browser artifacts before a pilot. Preserve the website's
working navigation and all original evidence.

## Log

- 2026-10-26: Created at the founder's request after the website navigation work.
  Checked the ticket board for an existing router feature (none found) and the
  current formal v3 record against v2's explicit non-goal. Filed as P2/E2 Backlog;
  no implementation, design-PDF rewrite, deployment or app migration started.
- 2026-10-26: Founder clarified that smooth navigation between native Rust SSR
  pages and their islands is an explicit goal, with full-page loads only as
  documented fallbacks. Replaced the protocol-or-document-boundary alternative
  with a required native transition outcome and tests for retained document/layout,
  destination state/island adoption, and separately verified fallbacks. Backlog
  status and planning-only authorization remain unchanged; no implementation began.

- 2026-10-26: Executed the accepted design/native-feasibility plan on published
  Forma 3.1.0 at 9a570afe. Reviewed target delivery:
  `~/dev/design-doc-factory/tasks/forma-router/deliverables/v1/`; all 105 research
  questions mapped to dispositions and 57 acceptance scenarios. Optional separate
  `@getforma/router` selected. The isolated worktree contains real compiler/Rust/core
  integration and a three-engine native/forms proof. See the delivery evidence
  index for exact receipts, retained failures and limits. This closes only the
  reviewed-design checkbox: general nested owners, public adapters, packed
  consumers, full resource/performance qualification and site pilot remain open.
  Moved the active ticket to 2-in-progress; historical planning log above retained.

- 2026-10-26: Implemented the optional package, native ownership/compiler/server
  contracts, three adapters, starter and coordinated release tooling in draft
  PR #204. Independent regression review added callback authority, durable
  pre-transmission POST markers and browser owner lifetimes. The frozen candidate
  at `9d4aafee0c99cde4faff3f39a8fca2705e70a398` passes 190 router tests,
  type/export checks and the unchanged paired size gate: 12,281 / 12,288 B
  incremental gzip, with only 7 B remaining. The full successor packed P17,
  candidate scaffold and final integrated measurement campaigns are in progress;
  their outcomes are recorded in the canonical qualification ledger. Published
  3.1.0, immutable design v1 and original REA snapshots remain unchanged.

- 2026-10-26: Completed local qualification of the sealed runtime: 190 router
  source tests, 75/75 successor P17 checks, 218/218 candidate scaffold checks
  including all 24 heavy checks, 90/90 installed-archive native browser cases,
  and 18/18 bounded real-site pilot cases. The installed archive paired size
  measurement passes at 12,281 / 12,288 B incremental gzip; the complete group
  is 40,713 B. The four timing profiles pass their original synchronous commit
  budgets (worst p95 2.0 ms desktop and 3.3 ms emulated mobile). Cold samples,
  end-to-end readiness and the +232,660 B heap observation after 100 transitions
  are retained separately, without a zero-leak claim. Closed the packed examples
  and measurement checkboxes only. Initial PR CI exposed release-test assumptions
  about the historical two-template inventory; those were corrected and the
  release-helper and published 3.1 checks pass locally. Final PR CI and remaining
  manual/release acceptance are tracked in PR #204 and the qualification ledger.

- 2026-10-26: Final implementation head `77192ed61ec9c0090ead5fc5094ef6d445d8a4dc`
  passed all 32 jobs in five PR workflows. [CI run 40399934454](https://github.com/getforma-dev/forma/actions/runs/40399934454)
  includes the complete stack check (733 seconds), Node 22/24, Windows Node/Rust,
  minimum Rust versions, optimized/32-bit tests and Linux/Windows candidate plus
  published-package scaffold lanes. Three-engine integration, security mutations,
  performance budgets and scaffold cache workflows also passed. The earlier
  Windows rename failure did not recur on this head; no assertion or budget was
  weakened. Final run metadata, job lists and logs are retained under
  `work/FORMA-301/qualification/ci-77192ed6/`. Draft PR #204 is ready for review;
  manual/partial scenario acceptance and the future coordinated release remain
  separate. No publication, version bump, merge or deployment occurred.

- 2026-10-26: Extended the authorized work to the full Forma ecosystem. Prepared
  all eight packages and starter seeds as 3.2.0-rc.1, migrated the website portal
  to the shared HTML adapter, and added native routing lab/docs/download work.
  Infrastructure PR #6 is merged with actual proxy-policy HTTP controls; the
  existing OVH deployment remains unchanged. Fresh `8ee6ae24` hosted archives
  pass native 90/90 across three engines. Current P17/platform/full mutation,
  paired size and independent release acceptance are separate ongoing gates;
  historical failures are retained. The user selected Mac VoiceOver and iPhone
  for manual acceptance once the preview is ready. Publication, deployment and
  stable acceptance are pending, not implied by source version preparation.

- 2026-10-27 UTC: Current hosted P17 and all scoped automated campaigns accepted;
  original full Nightly accounting independently accepted without relabeling its
  source or two survivors. User passed the six requested Mac tasks; a genuine
  public-profile BFCache witness passed separately. Independent D14 accepted exact
  pre-audit source 3c10b6d0 without blockers; audit-only fc226343 is pushed and the
  source is frozen. PR #204 ready flow and final CI are underway. Publication,
  registry pins, publisher cleanup/OIDC, physical iPhone, live OVH/site acceptance
  and the hosted 3,600-second RC soak remain pending. Ticket stays in progress;
  frozen design delivery and historical records are unchanged.

- 2026-10-27 UTC, after RC.1 publication: PR #204 merged at 6e5e2e49. Tag run
  40433276229 passed P17 75/75 in 1,247.311 seconds and published all eight matching
  archives. Record job 121967590802 then failed for the missing exact changelog
  entry, missed by pretag review and recorded as D14-3.2-010. Guard 3da2bfd2 and
  36/36 focused tests now reject missing/empty entries before publication. No RC.1
  DS12 or soak. Corrected RC.2 and a separate inventory-only RC.1 recovery receipt
  are being prepared. Bootstrap secret/token absence and npm trust/policy are
  verified; first OIDC publication, physical iPhone, live OVH and hosted soak
  remain pending. Site 1a088ca5 remains unpushed. Ticket stays In progress;
  frozen design/research and original failure receipts remain unchanged.

- 2026-10-27 UTC, RC.2 review checkpoint: recovery implemented in draft PR #205;
  fresh-context review found D14-011 proof-content admission gap, corrected at
  bdc691e1 with nine before/fixed controls. Local release validation passes 529/529.
  Authenticated bdc archives pass native 90 and unchanged 12,287 B incremental /
  40,715 B group limits. Original CI 40441474914 failed a stale README pin; five
  Markdown-only edits at 5b44312e pass 20 docs tests. New CI and original bdc P17
  remain running/pending, with candidate closure and fresh D14 still open. No
  RC.2 publication, site upgrade/deployment, soak or iPhone acceptance is claimed.
  Ticket remains In progress; frozen deliverables and original failures are retained.

- 2026-10-27 03:47 UTC, prepared RC.2 D14 accepted: exact input 12476b5c has no
  blockers. All four 5b PR CI planes and original bdc P17 75/75 in 1,266.223 s
  pass; native 90, paired size and local release 529/529 retain actual identities.
  Original bdc's three README-pin failures and overall cancellation are retained.
  Medium nonblocking finding 012 keeps successful functional R17 at 743 s with
  restored caches and current N7's unqualified 20-run distribution explicit;
  no waiver, threshold change or stable/full-conformance approval. Audit
  application/push, exact branch/main/tag qualification and publication/record,
  first OIDC, registry reconciliation, site/OVH, iPhone and 3,600 fault-free
  credited active soak seconds remain pending. Ticket stays In progress;
  frozen delivery and historical failures are unchanged.

- 2026-10-27 UTC, audit application: independent exact byte/Git verification
  confirms c94c78dd contains only three accepted D14 records and audit-row
  completion, bound to reviewed input 12476b5c. The commit is pushed and PR #205
  is ready; exact final branch CI 40446522943 and corresponding planes are the
  current qualification checkpoint. Main/tag checks, actual RC.2 publication and
  normal release record, registry/device/site/soak and stable acceptance remain open.

- 2026-10-27 UTC, RC.3/site qualification: source 34277d43 in draft framework PR #206
  has exact-source CI 40460309193 and packedP17rehearsal 40460337225 running. Finite
  placeholder/harness source changes and 22currentdocs are independently reviewed;
  RC3 remains unpublished with candidate/audit gates pending. Site 9d3aa2a3 is in
  draft website PR #12 / CI 40460736877. Its exact 14-file RC2 pins, original archives and
  preserved graphs are independently accepted; 441 local browser cases pass with
  six intentional skips,41 content checks and39 fresh consumer commands pass.
  Public ZIP Docker amd64 build succeeds after a retained worker-list transport EOF
  and one bounded retry. Fresh consumer/container verification continues; no OVH,
  iPhone, current N7, hosted soak or stable acceptance is claimed. Ticket stays in
  progress; frozen design/research and historical failures remain unchanged.

- 2026-10-27 08:12 UTC: RC3 prepared-source qualification passes packed 75/75,
  installed native 90 and unchanged paired size; fresh D14 accepts exact input 9c5c0916.
  Audit-only 86c80981 passes all four source planes (25 executed jobs, 13 original
  artifacts). PR #206 merged as 0cf47cdd; exact-main CI 40469879738 remains pending.
  The original 20-run N7 campaign passes functionally but fails p95=748.73s
  against <600s; no founder exception is approved. Site PR #12 merged as 1044;
  exact-main CI passes and image 5dc040db deploys to OVH at 07:43:19Z. Production 86,
  routing-guide bytes and all 241 downloaded members pass. Live portal failures
  61/63 and 65/66 retain their original traces; independently reviewed test-startup
  and CSS-load precondition fixes produce a fresh 66/66 with unchanged runtime
  and assertions. Exact two-file correction 8afa365c is in site PR #13. Old 919
  rollback image/archive/origin remain intact; no live rollback was exercised.
  Physical iPhone checklist is sent and pending. Actual RC3 publication/finite
  placeholder receipt, same-line hosted soak and stable acceptance remain open.
  Ticket stays In progress; frozen design/research/PDF remain unchanged.

- 2026-10-27 08:15 UTC: Exact main 0cf47cdd authentication completes: CI 40469879738
  passes 19 jobs plus one expected unpublished-scaffold skip, all 20 executed
  checkout logs and eight artifacts across CI/observation authenticate. R17
  passes functionally in 723s, without N7 credit. Immutable annotated tag
  v3.2.0-rc.3 object 5953703e points to this main source; normal release 40472265300
  starts at 08:15:35Z. Actual tagged qualification/publication/record and
  same-line soak remain separate gates. Factory working checkpoint 54d37fdf
  passes 21 mechanical checks with 0 failures/warnings; frozen delivery unchanged.

- 2026-10-27 09:00 UTC: Tagged RC3 attempt 1 fails 74/75 P17 in 1,356.260s: one
  islands-100 observation takes 501 ms after first paint against the unchanged 500 ms
  bound. All 40 observations complete; no measurement defect or cause established.
  Publishers/record skip, so RC3 remains unpublished without DS12/soak. All 703
  original evidence files and 41,141,812-byte failed P17 ZIP are retained/rehashed.
  Following independent policy review, root prospectively records exactly one
  unchanged full-P17 repeat and dispatches attempt 2 at 09:00:15Z; no further
  automatic attempt, selective sample reuse or waiver. Original outer start
  08:15:35Z remains and its 1,800s target is exceeded. N7 p95=748.73s still fails
  strict 600s; physical iPhone remains pending. Site PR #13 first CI at 8afa passes but
  retains only last-run metadata: reporter JSON resolved under examples/.local
  while CI uploaded root .local. Exact a758 changes only reporter path; fresh 6/6
  local proof passes and new CI 40476474204 is pending. Live 1044 remains accepted
  for production 86, corrected portal 66 and 241 downloads. Ticket remains In progress.

- 2026-10-27 09:39 UTC: RC3 attempt 2 completes normal publication after whole
  P17 passes 75/75 in 1,266.953s. All eight original DS11 archives match registry
  and Release bytes. DS12 7f7d283b binds actual P17 producer attempt 2 and evidence
  d926c49d. Finite router placeholder receipt e1ab6916 authenticates preparation
  in failed attempt 1 and recording in attempt 2, with all four eligibility flags
  false. Trusted-publisher/provenance metadata and empty bootstrap environment
  are verified; a separate signature-chain verification is not claimed. All 767
  new and 703 original evidence files authenticate. Original-clock acknowledged
  write elapsed is 5,023.767s, missing 1,800s. No N7 exception is approved; the
  exact original 748.73s p95 failure remains. Root is preparing an isolated N7
  scheduler prototype from 0cf, preserving every test/threshold and the macOS
  serial path. A harness successor requires its own qualified candidate/soak,
  so no RC3 soak was dispatched. Site PR #13 is merged as b0b83246 after exact
  CI passes and retains the six-case JSON; actual-main CI 40479873940 is pending.
  Physical iPhone and stable acceptance remain open. Ticket remains In progress.

- 2026-10-27 10:00 UTC: Site b0b83246 passes actual-main CI40479873940 with
  complete fresh reports, including router6. Normal immutable OVH build creates
  image43801e24; local86 passes and owned preview stops before deployment.
  Activation completes at09:52:18Z; default production86 and portal66 pass with
  no retries/skips/errors. All241 live ZIP members match exact b0 Git bytes;
  Field Notes/router archives and application/runtime/dependency bytes remain
  unchanged1044. Independent acceptance967f52d6 authenticates source, tree,
  checksums, image and original producer reports. Previous1044 remains available
  after read-only rollback checks; no actual rollback or physical phone result
  is claimed. N7 p95=748.73s remains failed without waiver; isolated scheduler
  development continues before any successor qualification/soak. Stable3.1 and
  frozen design/research/PDF are unchanged. Ticket remains In progress.

- 2026-10-27 10:24 UTC: Isolated scheduler prototype429b7f31 adds only eight
  reviewed harness files over0cf. Portable controls9/9 and Linux fake-child4/4
  (including11 inherited Python checks) pass; no workload/timing credit. One
  prospectively reviewed never-merge donor39a dispatches functional pilot40487907733.
  Attempt1 fails before fork after12 prerequisites because the source guard
  rejects ordinary setup-generated prefer-ubuntu-archive.cpython-312.pyc. All72
  original evidence files authenticate;56.32s is an incomplete command, not N7.
  A narrow guard correction is being reviewed; no same-source automatic rerun.
  Live b0/RC2 site remains verified. Physical iPhone, N7, settled-candidate soak
  and stable3.2 acceptance remain open; ticket remains In progress.

- 2026-10-27: Founder requested a follow-up after the islands/navigation guide
  clarification. [FORMA-355](../0-backlog/FORMA-355.md) now owns the evaluation
  and design of optional route-level code splitting within native groups. RD09's
  eager, integrity-checked group remains the 3.2 contract. This backlog ticket
  adds no 3.2 gate, implementation or promised release date.

- 2026-10-27 UTC, closeout: corrected isolated scheduler pilot40490320944
  authenticates all32 stages/280 core files, original logs/join/cleanup and
  exit0 in493.39s atb9eb. ZeroN7 credit; prior20-run p95748.73 remainsfailed.
  Maintainable integration into the real check entrypoint is in progress.
  Independent focused review dffe3d5c accepts20 framework/12 site files;
  framework Link policy, duplicate warning, inert field and docs fixes committed
  cad3d616 locally. Site theme return-path/search label/dead-code/doc fixes
  committed4ccb83be inPR14, CI40494157548 running. Local framework252+docs21;
  site33 browser/10 Rust/43content pass with before failures retained. New packed
  size proof is required (old incremental headroom1B); prior size does not transfer.
  Live b0/RC2, phone pending, actual rollback pending, new candidate/soak/stable
  qualification remain distinct. Ticket remains In progress.

- 2026-10-27 UTC, RC4 qualification: independently accepted maintained scheduler
  and coordinated source bd2b5221 are in draft PR207. Original CI40499608611
  failed the stale navigation export inventory on Node22/24 and R17; lane failure
  and complete owned cleanup are retained. Four-file correction 2a04eef0 is
  independently reviewed and passes 38 Node24, 15 Node22 and 18 scheduler controls.
  Original scaffold-cache40499608560 failed exact warm-cache admission because
  unrelated installed Rust versions changed its cache key; the pinned-action
  replay reproduces both hashes. Bounded repair is being reviewed without a
  threshold or cache-hit exemption. Both platform candidate scaffolds passed
  221 cases. Actual bd2b dry-pack archives authenticate; packed native 90 and
  paired size 12,276/12,288 incremental, 40,704/65,536 group pass with unchanged
  limits, awaiting independent consumer review. Whole packed P17 is still running.
  The 20-run N7 donor has never been dispatched and remains held at its original
  source binding. Site PR14 merged at feb0ef1a; exact-main CI40498003069 and all
  244 download members authenticate. Normal OVH rollout is underway; a green CI
  is not a deployment or rollback receipt. Stable, device, production rollback
  and credited active soak acceptance remain open.

- 2026-10-27 UTC: Corrected713b747d pushed to PR207. Fresh scaffold-cache and security workflows independently pass; ordinary R17 passes functionally in667s without N7 credit. Full source CI, new713b packed P17 and consumers remain pending. Original bd2b P17 passes75/75 in1263.829s, native90 and paired12,276/12,288 plus40,704/65,536-byte size accepted. New20-run donor reviewed but undispatched. Site feb0 deployment held on original local65/66 portal result; a test-only lifecycle oracle correction and independent causal controls are in progress. Liveb0/RC2 unchanged; all remaining release gates remain open.

- 2026-10-27 UTC: Founder chose “Continue optimizing before stable 3.2 (Recommended)”; no timing exception is authorized. Corrected-source full CI accepted; one independently reviewed N7 campaign 40506401105 dispatched with all 20 original samples required. Fresh native 90 and paired size pass; independent consumer review and P17 pending. User replies “looks good” to the physical iPhone checklist; exact feedback and unknown model/iOS/time retained in manual-iphone-feedback-20261027/feedback.json, without a fresh RC4/stable execution claim. Website oracle repair independently accepted and submitted as PR15.

- 2026-10-27 UTC, authenticated 713b qualification: the
  [original N7 campaign](../../work/FORMA-301/qualification/ecosystem/rc4-preparation/n7-campaign-713b747d/independent-run-review/authentication.json)
  retains all 20 functional successes, nearest-rank p95 **665.29s**, maximum
  **666.26s**, and 15 samples at/above 600s. Strict <600s fails without waiver,
  reruns or replacement samples; the previous 748.73s campaign stays unchanged.
  [P17 rehearsal 40504280506](../../work/FORMA-301/qualification/ecosystem/rc4-preparation/hosted-713b747d/packed-40504280506/authentication.json)
  passes 75/75 in 1,351.117s on exact 713b; original eight archives, native 90 and
  unchanged 12,276/12,288 incremental plus 40,704/65,536-byte paired size are
  independently accepted. [Declaration batching](../../work/FORMA-301/qualification/ecosystem/rc4-preparation/r17-declaration-batching/README.md)
  (34→10 compiler calls with separate CommonJS scope) and
  [native target reuse](../../work/FORMA-301/qualification/ecosystem/rc4-preparation/r17-native-target-reuse/independent-review/review-receipt.json)
  pass local proofs and independent review. Both complete 1,872-input admission
  profiles are refreshed; these successor changes have no hosted/N7 qualification
  yet. [Site main 8f CI 40509732361](../../work/FORMA-301/qualification/ecosystem/closeout-review-20261027/site/main-ci-8f272729-40509732361/authentication.json)
  is accepted; immutable 8f image/local92 pass, portal 66 remains pending at 13:40 UTC,
  and last accepted production is b0/RC2. The user reports **iPhone 17 Pro Max /
  iOS 26.7.1** for the earlier “looks good” six-item checklist response; the
  [device record](../../work/FORMA-301/qualification/ecosystem/manual-iphone-feedback-20261027/device-metadata.json)
  preserves its live RC2 scope and unspecified test time/per-step details.
  Final-candidate applicability, actual rollback, settled-candidate active soak,
  stable 3.2 and final pins remain open. Frozen design and old failures are unchanged.

- 2026-10-27 14:36 UTC: Exact79153778 full CI, packed P17 (75/75,
  1,345.655s), native90 and unchanged paired size are independently accepted.
  N7 campaign40518419051 finished19 successful commands/one watch recovery
  failure; all20 original durations retained, raw rank19=624.07s/max688.08s.
  Failed246.81s is incomplete workload, not an accepted performance sample.
  N7 fails completion and timing; no reruns, replacements or exception.
  The source-snapshot/watch-registration race is a hypothesis under causal
  investigation. Full original mutation campaign remains running; expression
  installation timed out, with conditional exact-source P17 component coverage.
  Site8f is now deployed and independently accepted (production92/portal66,
  download parity244), still on publishedRC2 packages. Bounded phone/Mac
  applicability is reviewed. Final D14, stable publication, final pins,
  actual rollback and credited active soak remain open. Ticket remains In progress.

- 2026-10-27 15:45 UTC: Reviewed successor0857163c/treec811980c is committed and
  pushed to draftPR207. The watch source-capture/registration race is causally
  reproduced and corrected in both split modes, retaining original publication
  code and the failed regression. Bounded compilation overlap, core correctness
  threads and isolated hostile-profile concurrency preserve complete workloads;
  both reviewed1877-input profiles are applied. Local controls and independent
  reviews are bound in the
  [integration record](../../work/FORMA-301/qualification/ecosystem/rc4-preparation/r18-integration/precommit.json).
  Fresh CI40533795585 and P17 40533967783 are running; no successor hosted/N7
  acceptance is claimed. Original7915 full mutation remains pending with exact
  successor read-scope applicability conditionally reviewed. Earlier failures,
  no-exception decision and live8f/RC2 deployment remain unchanged. Final D14,
  publication, active soak, actual rollback and final pins remain open.

- 2026-10-27 16:12 UTC: Source0e116ebf is pushed after original0857 CI ended
  18success/1release-helper failure/1expectedskip. Its617s ordinary R17 passed
  functionally with zero N7 credit. Four synthetic fixture metadata lines and
  their reviewed profile hashes correct the helper failure; full586 local tests
  pass without changing validators/assertions. Original0857 P17 remains failed
  at decoder p95 30.7ms versus30ms; all40 samples are retained, unchanged runtime
  bytes do not waive it, and no cause is inferred. Fresh0e11 CI40537713511 and
  P17 40537706950 run independently; its eight archives authenticate and fresh
  native/size consumers are underway. Actual7915 full mutation passes326/328,
  with two genuine survivors and all security/expression mutants caught;
  exact0e11 source applicability is reviewed. Whole Nightly stays cancelled
  from its separate setup timeout. N7 preflight remains under review and the
  next campaign is undispatched. Ticket and stable release remain incomplete.

- 2026-10-27 streamlined release decision: the maintainer waived additional N7,
  active soak, extra RC publication solely for soak inputs and the staged
  production rollback exercise. Preserve security, core correctness, exact-source
  independent review, ordinary final CI, tagged package qualification,
  provenance/integrity and trusted publication. Original0e11 CI and full75/75
  P17 (1303.893s) are independently accepted; native90 and paired size pass.
  Stable3.2.0 source preparation is in progress, not published. Next public site
  update is stable-only, including all current docs/examples/downloads. The
  [maintainer decision](../../work/FORMA-301/qualification/ecosystem/rc4-preparation/maintainer-decision-20261027/streamlined-release.json)
  supersedes earlier no-exception/soak-required checkpoint instructions without
  changing their historical observations. Ticket remains open through deployment.

- 2026-10-27 19:05 UTC: Closed after stable 3.2 publication and actual OVH acceptance at website9f99455a. Exact image/source identity, local93, production93, live portal/docs27, three ZIPs/272 entries, six stable pages and published minimal CLI10 HTTP checks are recorded in the production receipt. Original stale website CI assertions and cancelled duplicate runs retain their actual conclusions; all waivers carry zero pass credit. Moved to `4-done`; historical sections and frozen design remain unchanged.
