# Independent paired preparation readiness review

Reviewed 2026-10-22. Scope: all preparation Markdown in the platform v2 and Northstar-demo tasks, excluding the untouched design.md templates, against the controlling 25-page App Platform definition. This is a preparation review, not a full factory security/design gate, implementation audit, or software test result. Shared-contract lock/manifests were being finalized independently and their temporary absence was not treated as a finding.

## Findings at first review

1. **P2 — The mutation contract leaves its decision order relative to revocation and saved-work adoption unspecified.** `forma-generative-app-platform-v2/SHARED-CONTRACTS.md`, C07 (initial lines 59–61), authorizes and validates the active app/schema before describing a transaction that checks uniqueness and expected version. C10 separately permits compare-and-set adoption, and C11 promises current authority. The packet does not yet require the authorization epoch and pinned work/app/schema version to participate in the commit decision. A command could validate under an old permission or schema, then race revocation/adoption and commit under stale authority or meaning. Add a contract-level ordering rule: the commit must validate/serialize against authoritative permission and pinned-work version changes; if revocation/adoption wins, reject without mutation, while a commit that wins remains committed and must be reported honestly. Add a controlled race scenario. This need not choose a database or locking mechanism during preparation.

2. **P2 — The planned acceptance proof can pass without demonstrating the central spatial canvas or file-drop/upload entry experience.** `forma-generative-app-platform-v2/VALIDATION.md`, Product requirements P01–P15 and proof stages (initial lines 7–25 and 48–52), plus `forma-northstar-demo/SCENARIOS.md`, S01–S03. The source makes canvas/file drop central (pp.5,9–10,20), and the platform PRODUCT-BRIEF already promises pan/zoom, move/resize, groups, fit-to-content and saved layouts. Existing scenarios check views and saved context but do not exercise those interactions or the equivalent accessible Upload route. Add a small explicit acceptance row/scenario covering spatial edits, keyboard/focused alternative, reload of saved layout, and real import through both entry controls. This prevents a fixed grid and hidden fixture loader from satisfying all stated demonstration gates.

## Readiness judgment

The packets are otherwise suitable inputs to the factory's intake and targets work. Product identity, two-app authoring proof, generic source queries and writable internal records, retry replay ordering, preview isolation, explicit version adoption, derived-data revocation, source precedence, historical amendments, and proposed-versus-implemented status are coherent. The L3 rigorous platform and L2 standard demo remain clearly proposed rather than approved. Design choices and full gates are correctly deferred.

Conditional recommendation: ready for the next design-factory intake once the two findings are corrected and the final contract lock/manifests reflect those corrections. Neither finding calls for starting full design or application work in this preparation task.

## Fix recheck

Both findings are **closed** after a read-only recheck of the revised files:

- C07 now requires the authoritative membership/policy epoch and saved-work/app/schema revision to participate in the commit ordering against acknowledged local revocation and adoption. It explicitly rejects an HTTP-entry check or external cache as sufficient, and preserves a commit that wins the ordering with current-visibility checks at delivery. SCENARIOS S06 tests both interleavings.
- C03 now states the spatial, keyboard/focused, persistence and semantic-link requirements. C05 requires equivalent visible drop-zone and accessible Upload paths with inspection and confirmation. VALIDATION P16 and SCENARIOS S01 explicitly exercise those behaviors, saved personal/shared views, and preservation of unrelated work.

**Final judgment:** no unresolved preparation-readiness blocker found in the reviewed Markdown. Ready for the next design-factory intake, conditional only on the coordinating agent refreshing the final source/review records and contract lock after edits. This does not approve the proposed rigor/levels, settle the remaining architecture choices, pass full design gates, or validate a running implementation.
