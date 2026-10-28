# FORMA-294: Learn from winning Grand Prix implementations with Rea and test transferable performance improvements

| Field | Value |
|---|---|
| Epic | E10 |
| Status | Done |
| Priority | P2 |
| Type | Completed competitor analysis and reviewed compiler/runtime implementation |
| Release | Published and authenticated in stable 3.1.0 at 2026-10-26T15:34:04Z; final source 9a570afece5eb5091824422f00134a875310e857; published-package smoke verified |
| Repos | getforma-dev/forma; exact pinned Grand Prix contestants and upstream sources; local morluto/rea used as an analysis tool |
| Decision | Victor, 2026-10-25: use installed Rea to discover why competing frameworks win, compare their code with Forma, adopt useful patterns, and create a tracking ticket |
| Depends on | FORMA-257–264 retained measurements; coordinate with FORMA-293 before changing competitor versions |
| Assignee | Codex |
| Branch / PR | Merged [forma#200](https://github.com/getforma-dev/forma/pull/200), `codex/forma-294-compiler` at `e06939dc5661fd00183db67659e4314e8a12be4d`; measured compiler/core commit1d204 and frozen research evidence retained |

## Governing decision — no existing-user compatibility constraint

Victor, 2026-10-25: **Forma has no current users. Optimize for advancing Forma; do not preserve old behavior merely to protect hypothetical users or old applications.** Intentional API, compiler and runtime changes are valid candidates. Judge them by the selected product contract, actual correctness, security, performance, simplicity, size and maintainability. When a behavior changes, document the new rule and test that rule; compatibility with the former behavior is not an acceptance gate by itself.

This supersedes the initial investigation's compatibility-based rejection of transparent event delegation. The original probe remains valid evidence that propagation/order differ. Its result now identifies a design choice to evaluate, not a veto on delegated events. The same applies to inherited props/ref observability restrictions on static templates: review whether the old contract is desirable, rather than automatically retaining it.

The initial research was kept separate from RC2. The selected default-off clientTemplates implementation subsequently merged in PR200 and is now published and verified in stable 3.1.0. Adopted lists keep their original construction path (follow-up FORMA-296). The startup, memory and image budgets remain product goals; no existing-user argument is needed to keep them.

## Purpose

Improve Forma's actual workloads: server-rendered content, interactive islands, fine-grained updates and predictable ownership/cleanup. Explain the remaining gaps from what the measured builds execute. Identify transferable algorithms, allocation strategies, generated code and runtime patterns, then test the best candidates without removing meaningful work or correctness obligations from the benchmark.

The reviewed RC2 implementation is frozen at `9a7146f23e1524d0a104e3a5c15087914cdd3ee4` in PR199; the subsequent audit-only integration is `3b5537a931c5448b349a5965c838317e583869da`. Research and experiments live outside that checkout. This ticket does not silently change its production code, accepted tradeoffs or qualification evidence. Original results remain historical and source-bound.

## Starting evidence

| Path | Retained observation | Question |
|---|---|---|
| Effects | Linux run40140319634: writes/fan-out/create-dispose improve70.74%/145.77%/251.34% versus published3.0. Fan-out remains71.645% of pinned alien-signals against80%; batching throughput is13.79% lower than published. | Which costs belong to Forma's ownership and adoption guarantees, and which are avoidable wrapper, queue, link or allocation work? |
| Rows | Same-session creation is1.7047× vanilla against1.10×. Update-every-tenth and clear regress14.33%/30.43% versus previous main. | How do vanilla, Solid and Svelte create, bind, insert, reconcile and dispose equivalent rows, including the code their compiler emits? |
| SSR | Paired run40116668819: static100-row HTML33193→17154B; walker improves about19%, but combined Sprint only3.74%. Earlier3300/5000req/s estimates were not achieved. | What is spent outside the walker, and do faster SSR competitors perform the same escaping, data preparation, routing and response work? |
| Size/startup | Combined client8498B Brotli; accepted startup target50ms with bounded≤55ms exceptions; server strength limits8MiB/50ms/150MB. | Can a change improve the missed workload without shifting cost into bundle size, startup, memory, tails or other operations? |

Sources: `docs/release/3.1-QUALIFICATION.md`, `3.1-PERFORMANCE-TRADEOFFS.md`, the original campaign artifacts and `work/claude-release-3.1/evidence/codex-grand-prix-regression-audit/`. Different hardware/profile results cannot be combined into a new gain claim.

## Investigation

1. Freeze the measured contestant versions, source commits, lockfiles, compiler flags and built artifact hashes. Rank actual per-workload leaders from the retained campaign, rather than assuming one framework wins every operation. Keep PR194's proposed major upgrades separate.
2. Use local Rea at `FormaStackDev/tools/rea` to inspect selected built JavaScript artifacts and requested runtime evidence; preserve its Evidence IDs, source selection, tool/runtime version, hashes and limitations. Use normal repository tools for complete open-source algorithms. A static call graph is not a runtime profile or proof of causation.
3. Trace one shared workload end to end on both sides. Account for actual calls, allocations, dependency edges, subscriptions, DOM construction/insertion, event handling, list identity, disposal and first adoption; for SSR include request parsing, data preparation, escaping, serialization, headers, body writes and compression policy.
4. Review earlier compiler/listener/bootstrap prototypes and their rejected regressions before proposing the same experiment. Record why each competitor pattern transfers, does not transfer, or needs a changed public contract. Preserve upstream license notices and attribution for any adopted code.
5. Select at most three initial hypotheses with concrete predictions and cheap falsifying checks. Run isolated diagnostic variants with identical work and correctness checks; retain every result. Advance only a supported candidate to a paired, frozen-harness Linux run. Estimates must remain labelled estimates.

## Acceptance

- [x] A comparison matrix maps each important measured gap to exact competitor and Forma source lines and built artifact identities. Observations, inferences and unresolved questions are separate.
- [x] At least one actual Rea analysis is retained for a relevant built artifact, or its concrete limitation is recorded with supported source/runtime analysis continuing. No claim that source inspection itself proves speed.
- [x] A ranked decision record covers the best transferable patterns and previously rejected experiments, including complexity, size, startup, memory, ownership and security costs. Any implementation preserves applicable contracts or explicitly proposes a reviewed API decision.
- [x] Bounded experiments have reproducible exact commands, source hashes and all original measurements. Candidate conclusions survive correctness, update/clear/disposal, batching, zero-write island adoption and relevant hostile-input controls; no benchmark-only shortcuts or best-run selection.
- [x] Before claiming a performance win, re-race the same pinned competitors and candidate through the existing protocol on one qualified profile. Record all nine DOM and nine reactive workloads and relevant SSR/tail/size metrics; preserve50ms startup target, bounded55ms exceptions and existing memory/image limits. Changing a limit is a separate decision, never an invisible test edit.
- [x] Publish a concise adopt/reject/defer result with independent exact-source review. Successful experiments become concrete implementation tickets/PRs; inconclusive or harmful variants remain documented without a false success claim.

## Reproduction and artifacts

Initial artifact-analysis command (use the supported local Node runtime and substitute the exact selected artifact directory and a new output path recorded in the finding ledger):

```sh
node /Users/victorvillacis/dev/FormaStackDev/tools/rea/scripts/rea.mjs analyze-javascript-application "$ARTIFACT_DIRECTORY" --detail summary --evidence_output "$NEW_EVIDENCE_BUNDLE" --format json
```

Use `--source-path` only for an explicitly recorded selection; summary delivery alone does not reduce analysis scope. The analysis invocation and resulting Evidence IDs belong in `work/FORMA-294/client/` or `server/`. Exact diagnostic and re-race commands will be recorded when each hypothesis and frozen variant is selected; none has run merely because this ticket exists.

## Estimate and stopping rule

Estimate, not a commitment: one focused investigation pass to rank the causes, followed by up to three bounded experiments. No unbounded attempt to reach a leaderboard number. Stop or defer a candidate when its prediction is falsified, the gain does not survive paired measurement, or its cost harms Forma's intended workload more than it helps. The founder's existing stopping point still applies: after RC2 publication, dispatch burn-in to Actions and stop agent polling until the next user command; retain research state here.

## Rollback

Research does not modify the frozen release. Discard an experiment by returning to its recorded baseline while retaining evidence. If an independently reviewed optimization later merges, revert that implementation and any inseparable generated/compiler changes together; rerun relevant correctness and race controls. Never rewrite old benchmark artifacts, move a release tag, or claim an old result belongs to a new dependency graph.

## Log

- 2026-10-25: Created at the founder's explicit request; client/effects and SSR investigations delegated separately. Local Rea6.8.0 found and CLI help verified. MCP tools are absent in this session; CLI analysis requires no registration change. Existing local Node24.22.0 is below Rea's declared24.22.1 floor; a supported isolated runtime is being resolved, without a global upgrade. No competitor analysis result, new race, optimization or gain claimed yet.

- 2026-10-25: Initial server study complete at `work/FORMA-294/server/ANALYSIS.md` (SHA256 `6e0fae1592e87dbe8c20033f33c6e71b65a4e50f341f03e2a5f661a03cf3ece6`). Rea6.8.0 actually analyzed retained Svelte/Solid built JavaScript on isolated Node24.22.1. Exact sources, partial graph limitations and MIT notices retained; later local bundles are explicitly not authenticated hosted bytes. Ranked next checks: profile checked props→JSON→slots allocation/copy costs; evaluate a prepared static emission plan; measure the final body copy before changing it. No new SSR timing or performance gain claimed.
- 2026-10-25: Client investigation has an untimed, output-checked four-car diagnostic. Per1000new rows, Forma made6000shallowclonecalls and2000nativeclick registrations/AbortControllers; Solid/Svelte/vanilla each made1000deepclonecalls and no new per-row nativeclick registrations/controllers. Deepclones include descendants: this is a call-shape finding, **not six times the allocations or a predicted speedup**. Existing failed template/ownership experiments are being checked before choosing a new transfer; transparent delegation must preserve propagation, disposal and adoption contracts. Raw evidence: `work/FORMA-294/client/four-car-operation-counts.json`.

- 2026-10-25: Client/effects ledger complete: [findings](../../work/FORMA-294/client/FINDINGS.md), [receipt](../../work/FORMA-294/client/RECEIPT.json), receipt SHA256 `33828b3fd6eaa61919a7d64bbd8661b250ed91495472288d2c0bad62294c941e`. Exact measured Forma sources/package and pinned Solid1.9.17/Svelte5.57.2/alien-signals3.1.2 controls are distinguished from the official alien3.2.1 entrant. The concrete delegation prototype rejects transparent replacement: native stopPropagation suppresses a delegated callback that currently runs; capture changes registration order. No latency claim or production change. Next candidate requires a proven compiler-owned static-template/binding-path contract while retaining native event semantics. Effects need callback/context profiling; replacing the shared scheduler is not supported by the evidence.

## Current research state

The first source comparison and actual Rea analyses are complete. Two cheap client diagnostics ran: native-call counts with identical row output, and an event-delegation probe that demonstrates differences from the old native-listener contract. The new governing decision permits intentionally changing that contract. Earlier rejected templates, ownership changes and effect variants remain in the ledger. Independent cross-review of both ledgers is complete; the original evidence and dated amendments are retained.

No optimization has been promoted. The local future-contract screen below has mixed measured results; the exact candidate is now dispatched for one Linux follow-up. Server phase profiling remains unexecuted: the old walker-only benchmark excludes projection and follows a different JSON serialization path, so its totals cannot answer the new question. Use the recorded phase recipe with its thread-isolation amendment. Product acceptance remains open.

## Initial cross-stack decision

Prioritize three next hypotheses, in order:

1. **Checked props-to-slot construction.** Profile the complete server path first. Removing redundant owned intermediate data could benefit every dynamic SSR request while keeping the safety checks. The measured walker improvement alone did not explain HTTP throughput.
2. **Compiler-owned static subtrees with direct binding paths.** Choose and document construction, props and observability rules that the compiler can implement correctly; old behavior is not a constraint. The completed compatibility probe shows that shared delegation needs an intentional new event contract; under the governing decision above, it remains eligible for measurement. Include helper and fallback bundle cost in any comparison.
3. **Public-effect context and object shape.** Count actual callback/dependency work before changing layout. Forma already uses the same dependency scheduler as the matched alien-signals control; copying another scheduler has no demonstrated justification.

Defer additional FMIR specialization and response-buffer changes until the server phase profile shows their share of total work. Keep the current reviewed release unchanged: it has real effect and payload gains, documented batching/DOM losses and accepted target misses. Any future candidate must improve Forma's server-rendered/island workload as a whole, retain the 50 ms startup target and rare bounded 55 ms exceptions, and pass the original correctness and ownership controls.

- 2026-10-25: Independent cross-reviews completed: [client ledger review](../../work/FORMA-294/server/client-review.md) verified 276 hashes and 99 recomputed statistics; [server ledger review](../../work/FORMA-294/client/server-review.md) matched 85 source files and all three measurement-input hashes. No material analysis blocker. [Client precision amendment](../../work/FORMA-294/client/REVIEW-AMENDMENT.md) distinguishes the actual Forma stopPropagation probe from the native registration-order model.
- 2026-10-25: The unexecuted server [phase recipe](../../work/FORMA-294/server/phase-experiment/NEXT-EXPERIMENT.md) requires the [thread-isolation amendment](../../work/FORMA-294/server/phase-experiment/AMENDMENT-2026-10-25-THREAD-ISOLATION.md), SHA256 `6ecd4e50ea519e0b721358ff0fdf81292f16c6adff927634e281780b29614fb7`. A single libtest worker is not a single-threaded process: use preinitialized thread-local counting or a proven standalone executable, with an off-owner allocation exclusion control. Original recipe/manifest remain unchanged. No allocation results, fresh performance race or implementation acceptance yet.

- 2026-10-25: Founder explicitly removed existing-user/backward-compatibility constraints. Reopened delegated-event/static-template design as an intentional future contract. One isolated bounded candidate is authorized during RC2 qualification, with honest helper/bundle accounting, real Forma effects/ownership/list work and the unchanged visible benchmark workload. No frozen release edit or claim of a product gain from a benchmark-only helper. Historical compatibility findings remain evidence, now superseded as automatic rejection criteria.

## Future-contract candidate — first local screen

After the founder removed compatibility constraints, one handwritten static-row/delegated-click candidate ran outside the release checkout. It retains the actual Forma createList/createRoot/internalEffect graph; it is a prototype of compiler output, not an implemented compiler feature. Nine-operation state parity and seven lifecycle controls passed before one unchanged local five-sample/two-warmup timing screen. No tuning or repeat selection.

| Operation | Baseline ms | Candidate ms | Absolute delta ms |
|---|---:|---:|---:|
| Create 1,000 | 22.22 | 17.80 | -4.42 |
| Replace 1,000 | 19.80 | 23.82 | +4.02 |
| Update every tenth | 3.02 | 4.14 | +1.12 |
| Select | 0.66 | 0.92 | +0.26 |
| Swap | 1.90 | 1.78 | -0.12 |
| Remove | 1.60 | 2.86 | +1.26 |
| Create 10,000 | 241.20 | 175.60 | -65.60 |
| Append 1,000 | 20.88 | 23.34 | +2.46 |
| Clear | 3.16 | 3.06 | -0.10 |

The candidate is 1.057× same-screen vanilla for creating 1,000 rows, but this is **not** the qualified Linux target passing. Full app raw/gzip/Brotli sizes change 25,347/9,316/8,498 to 25,903/9,541/8,691 bytes; all helper bytes included. The nine-operation geomean worsens about 10.4%. Five samples on a busy local host, high variance in several operations, fixed run order and one browser limit causal confidence.

Decision: promising creation mechanism worth an independently reviewed qualified follow-up, not an established overall win and not ready to promote. Evaluate the absolute interaction costs, relevant workload mix, memory, startup and genuine compiler/adoption integration. Do not discard the new design merely for changing the old event contract, and do not declare victory from creation alone. Raw evidence and original candidate freeze: `work/FORMA-294/client/future-contract/`.

- 2026-10-25: Candidate report frozen at `work/FORMA-294/client/future-contract/RESULTS.md`, receipt SHA256 `f20bc608380a0d1cda06f6c2bbc09e95fa27d7880ca10f850ebf1f8f6d3f7547`. Independent review (`independent-review.md`, SHA256 `d86b7b38eea96ae6deab2e70feaa4c23cbc5a189bb09b5e93438d0452ca4f4d7`) recommends one qualified follow-up, with all operation costs and source limits retained.
- 2026-10-25: Separate supplemental ownership probe passed 34 checks covering actual createList removal, clear and parent disposal with live rows; disposed label/class effects stop, stale reconnected nodes cannot trigger removed handlers, and parent listener abortion is separately checked. Report `OWNERSHIP-SUPPLEMENT.md`; receipt SHA256 `ef920f35eaf774b3db547aec43d16c6704f600fe116d780535dbec01529cb50d`. Original candidate, all prior evidence and timing inputs remain unchanged; no tuning or timing rerun. Existing-user compatibility was not a gate. Same-key rerender, garbage-collection behavior and SSR adoption are not established by this CSR supplement.

- 2026-10-25 17:37Z: One Linux research follow-up dispatched as [40217278179](https://github.com/getforma-dev/forma/actions/runs/40217278179), exact branch commit `d2b45d3f824242d626b14f1cba18a861eace790a`, attempt 1. Applied all 28 files byte-identical to independently accepted proposal manifest `ac0a99b34b0c3163d92292fba254210d257753d607d3090052e53c08ec0415dc`; both wrappers parse and diff checks pass. Existing workflow's explicit research mode preserves canonical collectors, exact frozen browser assets, all nine operations, five samples/two warmups, whole bundles and untimed ownership controls. Uses the existing unchanged Linux subreaper and bounded cleanup. No result/pass claimed; authenticate the finished artifact and exact commit before interpreting. Branch remains separate from main/RC2, no product promotion. Pause polling at the published-RC burn-in handoff even if this research is unfinished.

- 2026-10-25 17:45Z: Linux40217278179 completed successfully and its artifact authenticated. Exact source/profile/campaign/ownership and all27means/SD, three geomeans and whole bundles verified. Create1k36.86→30.86ms vs vanilla24.02 (1.285×; target still misses); nine-operation geomean improves13.8%; remove+0.46ms,clear+0.86ms; +193B Brotli. Seven+34 controls passed. Report `work/FORMA-294/client/future-contract/linux-run40217278179/ANALYSIS.md`; receipt SHA `bf34a3222fb3eccab6c3bc89e4d9d9a5af9e76e1797cec6cf7bd63f6cfc6565d`. Independent result review must be checked on resume; proposal review accepted. Research paused at the requested RC2 burn-in handoff. No compiler/island integration or product promotion.

- 2026-10-25: Independent bounded Linux result review completed before the pause, SHA `7076c0c3fbad710e704c2defe72e2944389c3fb68e72ba7f990b50a7583fb3f9` at `codex-d14-3.1/forma294-research/LINUX-RESULT-REVIEW.md`. Replayed original archive/provider metadata, all50members,135samples, source/controls/cleanup/compression. Valid one-campaign research evidence; no product/release acceptance. Reviewer did not inspect author's later final prose/receipt. No more work running.

- 2026-10-25: Victor explicitly resumed this ticket and authorized real compiler integration, island/cleanup/bundle verification, removal/clear regression work and preparation to ship. Implementation branch `codex/forma-294-compiler` starts from main226f0d8d in the existing github-followup checkout; frozen research branchd2b45d3f and all evidence remain. Compiler pass, core helper/cleanup, and source-driven island tests delegated separately. Release finalization/burn-in polling are outside this resumed ticket. No compiled product gain or shipping readiness claimed yet.

- 2026-10-25 19:34Z: Actual compiler candidate committed/pushed as1d204ba4, draftPR200. General first iteration rejected for eager sibling capture, late disposal listener retention and raw-entry inventory gap; repaired staged/lazy-trust/list-only implementation independently reviewed. Also fixed the existing native+compiled root setup exception leak. Full compiler1771/build400 pass; core4270 plus two intentional export inventories fixed/24affected tests pass; browser60/60; actual packed ESM+CJS ownership2/2 pass. Source/evidence review `codex-d14-3.1/forma294-compiler/STAGED-REVIEW.md` SHA2afaf92b234fc0f400c763f04e69e2a6e009659d5710f5debae8579166ad45f7.
- 2026-10-25: Promotion still blocked: main ESM49796 exceeds49414 by382B; actual eligible native list28875 exceedsN2 28420 by455B. Small Counter and original single-node list remain byte-identical to the disabled transform. Dashboard+1180gzip; Grand Prix+936Brotli vsRC2. No limits/version/tag changed. Original failed size/browser attempts retained. Ordinary3-argument list retained h handlers are an existing element-owned lifetime gap, recorded separately; strict compiled stale callbacks remain0/0.
- 2026-10-25: One new local four-car screen retains all180samples: create10k286.36→205.84ms; create1k22.02→20.66; geomean8.427→8.755; remove1.98→4.32 with wide variance; clear3.66→4.02. Busy fixed-order local diagnostic, not causal/qualified success. Receipt `compiler-integration/staged-build/RECEIPT.json` SHA62c9804e55841410e816bc8caa6982a0cb1d4b9c93eac307c1a007afd39a7c17. No outliers excluded.
- 2026-10-25: One qualified GHA follow-up dispatched [40226501380](https://github.com/getforma-dev/forma/actions/runs/40226501380), gha-4-v1, diagnostic=false, compiler_templates=true, exact baseline226f0d8d and candidate1d204ba4 plus unchanged published contestants. Actual product compiler/source hashes and emitted assets retained; allnineDOM/nine reactive workloads. This result is pending. Separate PR diagnostic40226466888 is not qualification. RC2 burn-in remains unpolled and final release remains separately paused.

- 2026-10-25: Qualified Linux40226501380 independently authenticated: all1,305 samples/261 means+SDs/29 geomeans verified. RC2→compiled creation1k31.92→28.22 ms,10k347.24→287.34; nine-operation geomean improves5.592%. Removal2.44→2.42 is effectively unchanged; clear4.24→5.18 (+0.94ms/+22.17%), select+0.06 and swap+0.10 remain accepted bounded costs for an opt-in client-list workload. Creation still1.271×/1.256× matchedvanilla: original1.10× target remains missed. Reactive results are mixed/noisy; no scheduler/root causal speedup claim. Review SHA c7a6813ad4839427cb285b71fc37f02301b89f25e00303ae9c49f4d5ac15a07d.
- 2026-10-25: Product decision: make `clientTemplates: true` explicit/defaultoff. Adopted lists use original JSX descriptions even for added rows, so automatic eligibility charges helper bytes without the clone benefit. Keep current delegated mechanism; native-event alternative passed133units/66browsercontrols and saved319Brotli but failed main by361B and realfauthN2 by373B; rejected without timing. Ordinary listener-ownership gap filed as FORMA-295; adopted-list measurement/optimization filed as FORMA-296. No further runtime variants in this experiment.
- 2026-10-25: Explicit package-only tradeoff under the founder's delegated decision: main graph49,212→49,796gzip (+584), largely an extra compression stream; accept maincap49,414→49,926 (+512,130Bheadroom). Helper itself absent from main. Other4packagecaps, routeN228,420, startup50ms with existing bounded55ms exceptions, memory8MiB/image150MB unchanged. OriginalfailedCI/head1d204 remainsfailed. Fullfauth3route+CSS default28216passes; explicitoptin29167stillfailsN2by747, as required. Fullbuild410/410, size/docs29/29, revisedfivepackagegatespass. Finalreview/CIawaitcommit.
- 2026-10-25: Cleanup removed only4unusednative-event-screen node_modules directories,507MBallocated; source,94distfiles,failedrunsandall evidence rehashed/preserved. Cleanup record compiler-integration/native-event-screen-evidence/cleanup.json SHA4bd097f9c00058486a375dfe9c5dd34c7cab587d9e1a9a0ba30852bf8feb6f5b.

- 2026-10-25: Final opt-in commit `e06939dc5661fd00183db67659e4314e8a12be4d` pushed to PR200. Independent exact-commit approval: FINAL-REVIEW.md SHA7af0eb32b15ed4dc6c96a6b9655fff6bffdeb4dee9649079828653c93ba786e9; FINAL-COMMIT-BINDING.json SHA019c9c68eb15a3aa914b90be7c20cb6a908dbe60ec7d3900df680bbf95964b27. All397 core/compiler sources unchanged from measured1d204; final race JS/CSS/HTML and compiler/source/transformed JSX hashes match qualified Linux exactly. Real opted-in CSR build passes N2 at9250B; wholeJS+worker10896gzip/9828Br. Artifactreceipt2d837ee887fdee1bba8e4f5a6a9fd6919365ccc07f1cfce7a2ea87b17d9f7188. Additional mixed-entry/watch test1/1 closes shared native/Browser composition gap; earlier410fullbuildtests remain valid. Fresh CI40229046588/client40229046604/islands40229046584/budgets40229046578 running; no merge/tag/publication.

- 2026-10-25: Additional authorized cleanup removed only3completed main-ab dependency directories (381MBallocated), after fresh process/open-file checks. All4,549trackedfiles/80distfiles/lockfile rehashed unchanged; gitclean. Combined current cleanup approximately888MBallocated. Record `work/_maintenance/logs/forma294-main-ab-cleanup-2026-10-25.json`, SHA819a91fe287f8fe56cd1efd203934ebbd23c23367f314ab15feb9873fd9fad1c. Reused/unmerged257,259/260experiments/sharedsymlinks,live262, andactiveRC2 merge-probe were retained.

- 2026-10-25 20:22Z: PR200 merged as `ad26d9f8cdf78f6cfd2f22afefa2883cd903187a` after all26exact-head checks passed. Reviewedhead, tested syntheticmerge andactualmain have identicaltree88293fb5a0b31ce8b470d36a565ca77f49446903. ExactmainCI40230458249 running; releaseplane40230458234 passed. MERGE-RECEIPT.json retains binding. No release/tag change; oldRC2qualification cannot certify new main. Keep ticketInreview until postmergevalidation closes.

- 2026-10-25: Closed after exact-main ad26d9f8 CI40230458249 passed all21 jobs and release-plane40230458234 passed. Feature merged in PR200 after independent exact-source review. Final294 runtime/compiled artifacts matched its retained Linux producer; all accepted costs and rejected variants remain recorded. FORMA290 is now separately changing list ownership, so its new combined artifact needs new regression qualification and freshRC3; final3.1 publication is not claimed by this ticket closure.

- At 2026-10-26T15:43:54.446974+00:00, closed against verified [stable 3.1.0](https://github.com/getforma-dev/forma/releases/tag/v3.1.0), source `9a570afece5eb5091824422f00134a875310e857`. [Release 40344765522](https://github.com/getforma-dev/forma/actions/runs/40344765522) completed through record; all seven registry archives and all four npm `latest` tags are authenticated. Final tagged P17 passes 62/62 checks; independent raw review: [independent final review](/Users/victorvillacis/dev/Forma/work/claude-release-3.1/evidence/codex-d14-3.1/rc3/fresh-review/published-final-3.1.0/REVIEW.md) (SHA-256 b903f481051abcfe04ee81fb673528332b0713b879a0ad072771d136060ed5b9). The published minimal/dashboard Mac smoke passes with all three engines: [published smoke report](/Users/victorvillacis/dev/Forma/work/claude-release-3.1/evidence/codex-d14-3.1/published-3.1.0-macos.abrQK6/report.json) (SHA-256 1758f9e0c2f4008e195ef7fdccfb881032608ce8c7cba79da9dcd981b45d3bd2); this is not Linux N8 evidence. RC3 burn-in 40322508923 is authenticated and independently accepted: 15 rounds, 135 tasks, 3,660 credited active seconds, zero faults and complete cleanup. The original failures and source-specific campaigns remain unchanged. Implementation completion remains Done. This log updates publication status only; original measurements, accepted scope and prior evidence are unchanged.
