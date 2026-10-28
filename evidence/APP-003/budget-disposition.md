# FORMA-232 consumer disposition — no override required

Date: 2026-10-28. Consumer: APP-003. Accepted source: `74a72316c5f4bda03edc0c97db60f1e727bd22c3`, landed on `origin/main`. This clears only the budget dependency for the measured GenUI fixture.

The full nine-primitive native consumer, using exact registry-published Forma 3.2.0, measures **27,938 B estimated Forma gzip** against both the retained 28,100 B application target and shipped 28,420 B framework ceiling. The actual build report includes **31,176 B total JavaScript**, including worker accounting, under the declared canvas cap and the ordinary-page cap. No router or budget override is installed. The report is reproduced on macOS ARM64 locally and Linux/x86 CI: [local report](budget.json), [CI report](ci-74a7231/budget.json).

[Independent review](review.md) inspected all nine rendered primitives, verified source identities and independently recomputed the total with the pinned Node compressor. All seven [source CI checks](ci.json) pass. The published package pins and lockfiles are part of the reviewed candidate. The smaller upstream fixture alone was not used to clear this condition.

Upstream FORMA-232 [PR213](https://github.com/getforma-dev/forma/pull/213) is merged as `d7642c6a280f32e44cf48dff1a6190b2d79664b0`, with 29 successful checks in the [source receipt](forma-232-source-receipt.json). Its configurable-budget feature remains **unreleased**, and its local archive was not installed. This consumer proceeds because it needs no override, not because merge implies registry availability. ADR-001's 28,100 B app target remains unchanged.

This disposition applies to the measured fixture and source identity. Added renderer behavior, dependencies, routing or an integrated app build must be measured again. A future overage reopens the relevant budget decision; it cannot silently inherit this pass. FORMA-246 conformance receipt, hosted authentication, managed trials and PH01 acceptance remain separate requirements.
