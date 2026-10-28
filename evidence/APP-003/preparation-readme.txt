# APP-003 consumer preparation evidence

**Prepared locally; APP-003 remains Blocked pending the reviewed upstream handback and consumer acceptance.** No framework package was replaced or released, and no production app or cloud resource was changed.

The reproducible command is `python3 spikes/APP-003/verify.py`; [the fixture runbook](../../spikes/APP-003/README.md) defines scope, pins, controls and limitations. [qualification.json](qualification.json) contains exact commands, environment, exits and outputs; [identity.json](identity.json) binds source hashes to the recorded base revision. The configured canvas CI has no passing claim until this candidate is committed and run remotely.

## Measured route and state

The full nine-primitive native canvas route is `islands`, `canvas: true`, with no router. Its current build measures **27,938 B estimated Forma gzip**, below both the retained **28,100 B application target** and the **28,420 B published ceiling**. Total JavaScript including the emitted worker is **31,176 B**, below the canvas cap of 373,760 B and also below the ordinary 76,800 B page ceiling. No expanded-cap feature or override is required by this measured fixture. This is a consumer disposition, not evidence that unpublished FORMA-232 configuration support is released. See [budget.json](budget.json), the actual [size report](size-report.json) and [manifest](manifest.json). The worker is conservatively included by the build report even though the server config disables registration.

The normal native response is 17,596 bytes; its actual island state envelope is 6,524 UTF-8 bytes. The 500-row state's envelope is 92,297 bytes. All five states fit 1,048,576 bytes; a 1,206,304-byte typed UTF-8 input (including 400,000 three-byte characters in one note) is refused with `RENDER-502`. The exact current measurements are in [browser.json](browser.json) and [render.json](render.json); serialized input bytes and encoded state-envelope bytes are different quantities.

## Behavior observed

Chromium, Firefox and WebKit each pass normal, empty, empty-nested, long/non-ASCII/hostile-label and 500-row states. Normal retains its root and all 464 nodes; the largest state retains 5,931 nodes. Every activation has zero initial writes and an observer control that detects a seeded write. Real form/keyboard/focus/filter events work, the delayed epoch1 response does not replace epoch2, and all six linked views keep the current scope. Numeric SVG preserves 12 rectangles and title namespaces/values, hostile-looking strings stay literal, and page/CSP errors remain empty. A separate no-JavaScript form POST succeeds in each engine. Accessible labels and state are captured, not presented as a human screen-reader pass. The [representative Chromium screenshot](canvas-chromium.png) shows all nine rendered primitives after the form/filter/keyboard interaction path.

The release-renderer report contains one first call plus 200 sequential calls per state, throughput, local sample distributions and process-lifetime peak RSS. These are local macOS ARM64 results, not a managed-task/client p99 SLO or per-render allocation claim. Full rendering is timed; `native_walk_html_bytes` is only the framework walk count. Source identity and exact output should be used when quoting measurements rather than the historical first run.

[refusals.json](refusals.json) retains source and diagnostic evidence for enum-condition dispatch (`BUILD-610`), four-level composition (`BUILD-403`/`BUILD-610`), a reassigned declared local (`BUILD-610`) and negative unary tab index (`BUILD-207`). Rust tests also refuse incorrect native binding/unbound finalization, missing/Browser route modes, invalid geometry, 501 rows and duplicate card keys. Successful responses keep full PageResponse nonce/CSP/no-store policy.

## Retained failures and type boundary

The first source typecheck exposed the published declaration mismatch between JSX values and list callbacks, the missing SVG title tag and lowercase `tabindex`. The accepted source uses the correct `tabIndex` prop and a small consumer-only declaration bridge described in the runbook. This observation is not a reproduced raw initial log; the committed bridge, strict build and real namespace/identity evidence are reviewable. It changes neither production package code nor generated IR.

Initial builds also refused a reassigned prop-derived local and negative literal/module-constant tab index. Supported source patterns replaced those constructs, and their refusals are reproduced by the retained controls. The first refusal-harness run omitted the required empty CSS list and failed configuration validation before its intended case; [initial-harness-failure.json](initial-harness-failure.json) preserves that failure. The corrected control declares `css: []`; an infrastructure error is never counted as the intended compiler refusal.

## Upstream and remaining gates

Reference: `Forma/work/FORMA-246/evidence/published-3.2/HANDOFF.md`, independently replaying original and amended named fixtures on published 3.2.0. PR214 remains a separate upstream qualification/merge decision. Its four-kind results do not substitute for this nine-kind consumer campaign. FORMA-232 PR213's merged configurable-budget code is unpublished and absent from these dependencies. Root owns the authoritative handback/register and ticket status; this evidence does not clear either gate itself.

Production canvas layout, durable forms/idempotency, actual query/receipt/tenant security, full catalog integration, human accessibility testing and managed performance remain with their planned tickets. Cold-client/native structural parity is supplied only by the separate upstream reference corpus; this consumer record specifically proves native adoption and interactions, not a separately built cold-client twin.
