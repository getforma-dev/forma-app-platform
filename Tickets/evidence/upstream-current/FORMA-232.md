# FORMA-232: Configurable application size budgets with strict framework CI regression gates

| Field | Value |
|---|---|
| Epic | E10 |
| Status | Backlog |
| Priority | P1 |
| Type | Build-tool enhancement and bounded performance evaluation |
| Release | Post-3.0 proposal; no target version committed |
| Repos | getforma-dev/forma: build configuration, diagnostics, reports, tests and docs; isolated copies of existing FormaPlayground apps |
| Decision | Founder 2026-10-21: retain strict framework CI budgets; allow deliberate application budgets with clear warning/error policy; optimize and measure before considering an increase |
| Depends on | FORMA-190 implemented size reporting; FORMA-228/230 retained apps and evidence available; no migration dependency |
| Assignee | FormaStack team |
| Team | FormaStack |
| Blocks team | GenUI |
| Blocks | [APP-003](../../../FormaBuiltApps/forma-app-platform/Tickets/9-blocked/APP-003.md) — full-canvas qualification: budget disposition |
| Branch / PR | None; ticket creation only |

## Why

Forma 3.0.0 applies the same nonconfigurable size policy to framework release
fixtures and downstream production builds. `BUILD-819` refuses a route over
28,100 B of estimated Forma gzip contribution or 75 KiB of total cold-route
JavaScript (365 KiB for a genuine canvas route). This is a performance policy,
not a runtime capacity limit or proof that a page is responsive or cannot crash.

The framework contribution includes bundled core, alien-signals and identity
banners. It is estimated from each bundle's compressed size and raw module
contributions, so application code/compression and chunk layout can move the
figure without growth in framework source. The total-JavaScript budget also
includes the registered worker. Static dependencies count; separately emitted
dynamic chunks do not count in this cold-route metric, but their eventual load
and interaction cost still matters.

The retained experiments establish no need to raise Forma's own release ceiling.
They do establish useful friction: a small app can hit an approximate byte
boundary, and code changes made during an attempt still require behavioral
verification. FORMA-230's pending-checkbox bug is separately confirmed; its cause
has not been attributed to size optimization. Neither experiment proves that a
larger application budget is safe or necessary for a particular future app.

## Framework accountability stays strict

- Keep existing framework package/fixture/release regression budgets, including
  the 28,100 B N2 ceiling, enforced in Forma's own CI. No threshold increase or
  release-test relaxation is part of this ticket.
- Framework qualification must use its own explicit policy and fail if breached,
  regardless of a consuming application's configuration. Test that separation.
- If future evidence shows a framework budget must change, handle it as a
  separate reviewed framework decision with exact before/after bytes, behavior
  and browser results. Do not automatically ratchet to each new measurement or
  treat an application exception as precedent for raising release limits.
- Preserve unsupported-source refusal, static no-JavaScript contracts, typed
  props, adoption, CSP/Trusted Types, integrity, skew checks and runtime resource
  bounds. None is a size-budget option.

## Proposed application budget behavior

Design an explicit, versioned build configuration for application-wide defaults
and narrowly scoped route overrides. This is a future API, not shipped syntax.
Inventory the existing `budgetThreshold` Brotli warning setting: it does not
override BUILD-819 today. Reconcile/document its relationship to the new policy
instead of introducing two contradictory or easily confused controls.

1. Retain the current 3.0 size defaults and fail-on-excess behavior when no new
   configuration is supplied. Add a documented early-warning threshold with
   measured justification; do not silently make existing failures warnings.
2. Separate the total cold-route JavaScript budget from the estimated Forma
   contribution. A consumer may deliberately set its own warning/error policy
   for either metric without modifying the framework's CI baseline. Report the
   estimate as an estimate. A larger consumer Forma allowance does not certify
   that the framework meets its own release ceiling.
3. Permit explicit finite thresholds and warning/error behavior. Define units,
   equality/boundary behavior, override precedence and exit status. Reject
   invalid numbers, unknown fields/routes and inverted warning/error thresholds;
   never silently ignore a mistyped override. Do not require an unrelated heavy
   page to pretend to be a canvas to choose an appropriate app budget.
4. Print and persist actual bytes, default and effective budgets, scope/source of
   overrides, severity and exceeded values in the existing report. Preserve raw
   measurements when a build warns or fails. Do not label an oversized route
   as within the default budget just because an override permits it.
5. Keep the existing public report/diagnostic consumers compatible, or explicitly
   version and document the change. Define behavior for production versus watch
   output without measuring unminified development bytes as shipped bytes.

Passing a size budget is not a stability guarantee. An override changes build
policy, not runtime checks, application correctness, browser responsiveness or
security. Documentation must explain that distinction and the developer's
explicit choice without promising that Forma can prevent every app failure.

## Agent decision rule: raising a budget is not the first fix

When a route exceeds its allowance:

1. Inspect the report and actual import/chunk graph. Identify which metric failed,
   genuine runtime growth, duplicate imports, heavy dependencies, and attribution
   effects before selecting a remedy.
2. Try a bounded, low-risk improvement where justified: remove unused imports,
   avoid duplication, defer a genuinely optional feature, or use server-rendered
   content where appropriate. Do not distort a coherent user flow just to add
   routes, or shift an immediately needed module into a dynamic chunk to hide it
   from the cold-byte report.
3. Preserve behavior and safety. Never remove validation, error handling,
   accessibility, adoption or security checks to save bytes. Do not rewrite a
   correct implementation indefinitely to shave negligible estimated bytes;
   stop when benefit no longer justifies complexity or regression risk.
4. Consider an application override only when the requirement or measured
   tradeoff warrants it. Record the old/new budget, route, reason, alternatives
   considered, actual bytes, browser measurements and correctness results in the
   reviewed change. Scope it to the affected route where possible. An agent or
   build helper must not automatically raise budgets or change severity simply
   because a build failed, nor repeatedly raise them until tests turn green.

Update relevant application-development guidance when the API lands so future
agents follow this order. No new mandatory founder approval ritual is introduced
for every app build; use the project's normal change review and authorization.

## Bounded playground prototype, when selected

Reuse existing source and tests under
`~/dev/FormaPlayground/experiments/forma-228-fieldwork-feature/` and
`~/dev/FormaPlayground/experiments/forma-230-focus-desk/`. Do not rebuild apps from
scratch or overwrite their original apps, timed runs, metrics or raw evidence.
Create a separate derivative such as `FormaPlayground/experiments/forma-232-route-budgets/`
with its own data, ports, reports and source revision. Local framework overrides
must be explicit and labeled as unreleased; registry 3.0 remains the baseline.

Use Focus Desk's ordinary-guidance variant as the initial passing behavior
baseline. Retain the held-PATCH probe as a regression check. The original workflow
copy's known pending-checkbox failure stays preserved; if that copy is used,
repair it in the derivative and record/test that repair separately before drawing
budget conclusions. Do not claim that a budget adjustment fixed its behavior.

Predeclare a small comparison: unchanged baseline, a bounded safe optimization,
and an intentional app override if warranted. Include one realistic heavier
interaction using the existing app, such as opening a lazily loaded analysis
panel, only if the small baseline cannot exercise the relevant tradeoff. State
what the fixture does and does not represent; no synthetic padding claimed as a
real-world performance success.

Use one matched browser configuration and a bounded sample count chosen before
running (initial target: five cold navigations plus five feature interactions
per selected variant, at 4x CPU slowdown, serial execution). Record actual cold
and deferred bytes, navigation/paint and useful-input readiness, raw long tasks
and independent observer/input-response measurements. Retain rejected/slow runs
and distinguish browser-only from application work without inventing allowlists.
Tiny samples are diagnostic evidence, not a release p95 or universal guarantee.
Any app-specific performance acceptance must be declared before evaluating the
variant; do not relax it afterward to justify an override. Framework N3 is unchanged.

Stop after the bounded comparison. Expand only for a concrete unresolved finding;
do not run full release campaigns or repeat until favorable. Prefer CI for genuinely
heavy follow-up checks, retain the Mac concurrency caps, and never reset the live
Fieldwork preview or touch consumer application repositories.

## Acceptance tests

All items below are future acceptance criteria, not tests already run.

- [ ] Framework CI still passes exact-budget fixtures and rejects one-byte-over
  fixtures under its unchanged policy, even with permissive app overrides present.
- [ ] Unconfigured application builds preserve existing production defaults;
  test below/equal/above warning and error thresholds, explicit warning-only
  behavior, exit codes, route precedence and malformed/unknown configuration.
- [ ] Reports show actual/default/effective values, estimate labels, explicit
  overrides and severity consistently in console, API and emitted JSON. A warning
  does not erase the measured excess or claim default-budget conformance.
- [ ] Canvas, Browser, island and no-JS SSR cases remain distinct. Static no-JS
  and security/correctness failures cannot be bypassed through budget settings.
- [ ] Verify deduplicated shared/static chunks, separately emitted deferred
  chunks, worker accounting and unchanged gzip attribution. Do not change the
  counting method merely to admit an app; a method change needs separate review.
- [ ] Existing type/native-build, Rust and browser behavior checks pass on the
  derivative, including DOM adoption, persistence, keyboard/focus, failed writes,
  held-request state and recovery. Preserve any known original failure separately.
- [ ] Matched browser evidence shows the actual tradeoff, including delayed
  feature interaction. At least one over-budget warning/error control and one
  intentionally slow interaction control demonstrate that the relevant checks
  can detect failures; a smaller bundle or permitted build alone is insufficient.
- [ ] Document configuration, defaults, override reasoning, limitations and
  rollback; update agent guidance to discourage budget increases as the first
  response. Independent review and applicable CI pass before a future merge.

## Migration and rollback

No library changes, budget increases, prototype runs or app migrations are
authorized by creating this backlog record. When selected, develop the proposed
API in isolation and retain old behavior for consumers without configuration.
Removing an app override restores defaults; if that exposes an excess, report it
truthfully rather than silently changing source or build output. Reverting the
prototype must leave published 3.0, framework release evidence and the original
playground comparisons intact. Existing governance deferrals remain unchanged.

## Sources and related work

- [Published size policy and attribution](https://github.com/getforma-dev/forma/blob/v3.0.0/packages/build/src/route-size.ts)
- [Build configuration and report types](https://github.com/getforma-dev/forma/blob/v3.0.0/packages/build/src/types.ts)
- [Boundary/refusal tests](https://github.com/getforma-dev/forma/blob/v3.0.0/packages/build/tests/route-size.test.ts)
- L-64 and DR-35 in `~/dev/design-doc-factory/tasks/forma-stack/decision-brief.md`.
- [FORMA-190](../4-done/FORMA-190.md): implemented size reporting and enforcement.
- [FORMA-228](../4-done/FORMA-228.md), [FORMA-230](../4-done/FORMA-230.md): retained app/test evidence, including compiler and UI findings.
- [Playground index](../../../FormaPlayground/README.md): current paths and reports.

## Log

- 2026-10-21 — Created P2/E10 Backlog at the founder's request. Retain framework
  regression accountability; propose explicit app budgets with optimize-first,
  evidence-backed exceptions and bounded reuse of playground apps. No framework
  ceiling increase is established as necessary by the current experiments. Ticket
  only; no library implementation, performance run, migration or governance change.

- 2026-10-21 — Independent read-only ticket review found no material omissions; local source links verified. Board regenerated: 125 Done / 37 Backlog / zero active. No acceptance tests or prototypes run during ticket creation.
- 2026-10-22 (platform v2 foundation review): the industry-agnostic platform's canvas route
  measured 27,674 of 28,100 B Forma gzip with four core primitives on 3.0.0 as shipped (fixture in
  `design-doc-factory/tasks/forma-generative-app-platform-v2/review/spike-native-composition/`);
  a component library importing more of core, or the lazily loaded HTML runtime at 28,337 B, exceeds
  the cap. The platform design assumes this ticket done under either representation and the review
  recommends P1. Review sections 4.3 and 6.
- 2026-10-22 (evening): the founder accepted the platform v2 foundation review's recommendations: this ticket is a platform dependency assumed done for the design, and its priority is raised to P1.

- 2026-10-28 (GenUI cross-team handoff): Assigned to the FormaStack team; status remains Backlog and no implementation has started. GenUI APP-003's full-canvas qualification is blocked on an explicit budget disposition. Measure the actual application on the strict shipped Forma 3.2 caps first, and try bounded safe optimization where justified. The prerequisite clears either with completed FORMA-232 work available in the application's verified dependency baseline, or with a documented no-override-needed disposition accepted by GenUI showing the app fits the shipped caps. The latter clears this application dependency only; it does not complete or change FORMA-232's configurable-budget feature scope. No threshold bypass, automatic override or assumed publication is authorized or claimed. The teams can continue independent work in parallel. See the [cross-team journey](../journeys/genui-platform-2026-10-28/README.md).
