# APP-143 contextual Ask — local implementation evidence

2026-11-01. Source merged through [PR 31](https://github.com/getforma-dev/forma-app-platform/pull/31), reviewed head `711d87d985cd0484bd25d2f863f6428d370881ea`, merge `6c308f1a9c7db08d019332bb14b9b235db650362`. The observations below were collected during implementation from baseline `9ba2061`; their individual build identities remain explicit. This is local behavior and source-delivery evidence, not hosted-trial acceptance.

The canvas selects the clicked app/card/widget. United apps expose Ask; focus has a local panel and closing focus restores app/canvas context. The standalone `/w` adapter binds only its server-rendered app/work and part collection. Local panels preserve an unsent question while clearing/aborting the previous context's answer. Record answers retain the exact existing receipt/reference tuple; closed rendering introduces no HTML parser.

Weather, clock and calculator have a separate, closed widget scope containing widget identity and admitted state, with no work selector or records. This scope explains controls/settings only. The route does not attest current weather/time/calculation results: numerical model answers, references, extra response keys and unsupported widget kinds are refused. No rehearsed widget answer is added. Source-query/static part questions explicitly say that Ask cannot read those values; app/collection record questions remain available.

After model completion the route resolves the original session again and reads the exact record scopes again. Release requires the same app/work/publication/definition digest/work version and currently authorized reads. Snapshot record receipts remain the evidence for the answer; unrelated app records never enter a card's collection request. No question/record value is added to logs.

Focused validation: admin typecheck and 83 tests passed; five Rust Ask/Shape body/response tests passed. Coverage includes exact collection receipts, widget record-selector denial, numeric-result refusal, closed document scopes, late old-context response suppression and unsent draft preservation. The native pitch campaign adds real storage/model-barrier revocation and exact widget-input assertions; final campaign/browser results are recorded below when completed.

Related canvas integration in this candidate preserves all declared safe parts and admitted `default_layout`: note text, separately identified forms, supported source results, and honest unavailable placeholders. Supplemental record/app controls remain visible. Unite moves existing nodes so form drafts/receipts survive mode changes. Record choice links open the standalone app selector. Independent saved-work naming is APP-141 follow-through: exact work endpoint, optimistic version, no shell-wide fallback; sibling names remain separate.

Limits: local personas and a deterministic model fixture qualify mechanics, not paid-account onboarding, live hosted provider quality, or the Nov 2 recording. Independent review and final exact-source CI are complete; the managed trial remains separate.

## Completed local checks

- Native pitch campaign: 19/19 checks, 126.93 seconds. `native-pitch.json` includes the model-barrier work revocation case, all three widget scopes, numeric/unsupported denial, source/tenant/work isolation and unchanged existing Ask receipt semantics. It uses deterministic model fixtures with real storage and route execution.
- `admin-tests.txt`: 83/83 tests; `ask-unit-tests.txt`: 5/5 Rust body/response tests.
- `admin-build.txt`: strict build passes, manifest `b3a507a82188…`, /w JavaScript 38,878 bytes against 76,800; canvas JavaScript 172,318 against 373,760. No ceiling was changed. This build predates the final concatenated Ask CSS fix and the final guest part-rendering work.
- `browser.json` / `browser.mjs`: disposable Chrome profile on isolated port 7601; synthetic dev member, real HTTP/storage. Actual modal template creation automatically united guide then form; fan-out/Unite kept draft and command key; app/focus/standalone scopes stayed exact; rename and unchanged Save worked; unsupported static Ask refused without a request; clock request had no work selectors. No page errors. Record questions used existing demo rehearsed zero-record answers; browser widget step validates scope, while complete widget responses are covered by the native campaign.
- Visual inspection found the initial Ask stylesheet import was dangling (the published builder concatenates CSS inputs). `build.ts` now explicitly concatenates `app.css` and `ask.css`. The earlier mobile screenshot is retained as a before-fix artifact; final screenshot follows the integrated rebuild.

Independent review by root checked the Ask release reauthorization and scope/client/rendering contracts; a work-name no-op response/version validation issue was corrected and tested. Peer review of root naming found stale titles in command/openable-work discovery; root corrected the authorized read paths, with native assertions added for both renamed and unchanged sibling work.

## Final integrated browser follow-up

On manifest `66a004c8bae58393`, `final-ui.json` / `final-ui.mjs` pass with no browser errors. Fresh-browser Library and united app retain the independent saved-work name after reload/command discovery. The Ask CSS now loads (computed panel background verified), the scope switch retains an unsent question, and the 390px mobile page has no horizontal overflow; final screenshots are `document-mobile.png` and `canvas-final.png`. The clock question completed through the locally configured real model with HTTP 200 and settings-only prose, empty receipts/references, and no numeric current-value claim. This checks a live provider response through the local dev identity, not paid-account/provider signup. Earlier browser answers over the empty template were existing demo rehearsed record answers.

Delivery completed in PR 31: all 28 exact-head checks and the full local check passed. The [candidate receipt](../APP-140/mvp-candidate-711d87d/README.md) ties CI, merge and qualified image identities together. This does not change the recorded local browser build identities or claim hosted signup/recording. Owner ports were never used.

## Genuine hosted owner Ask

[Actual hosted evidence](../APP-139/hosted-owner-2026-11-01/README.md) records one exact app-context question over the first committed record on deployed `711d87d`. The response returned its item/date and one matching full server receipt in 1,520 ms; the rendered answer/expanded receipt matched, and authorized record listings were unchanged. Exact-task/time-correlated provider telemetry confirms a real model response. No source-query/static-result support or completed recording is inferred.
