# APP-003 independent consumer review

Reviewed revision: `74a72316c5f4bda03edc0c97db60f1e727bd22c3`. This reviewer did not author or edit the consumer harness, renderer, declarations, build, runtime or producer tests.

**Verdict: no blocking source or consumer-qualification finding.** The measured nine-primitive fixture fits published Forma 3.2.0 without a budget override. Final APP-003 closure still requires the accepted FORMA-246 reviewed merge/CI receipt and this app revision’s own CI result. This review does not infer either from local tests or a framework ticket’s status. Root owns those gate receipts.

## Source and artifact boundary

Recomputed all **15 source hashes** and **13 listed evidence hashes** in `identity.json` against Git objects at this exact revision; all match. The identity file’s SHA-256 is `d2cfe3900b8085d7e7779f4fbf844db2d77a4e6181ff71ab45ac304b154cf0a1`. Verified all three manifest asset integrity values against the actual emitted assets. Inspected the raw producer reports, all fourteen successful preparation steps, four compiler refusal cases, browser reports, render measurements and screenshot.

Core/compiler/build and the three Forma Rust crates are pinned to registry 3.2.0 with package integrity/checksums. There is no unpublished FORMA-232 archive, local framework runtime, router, budget override, hand-edited IR or Browser fallback. The one native route actually renders summary, table, detail, timeline, chart, filter, form, note and status; these are not unused imports counted as library coverage.

The consumer-only declaration bridge is explicit and emits no runtime code. Its broad JSX callback declaration does not establish arbitrary fragment-root support. The concrete list roots here are Elements, and strict native compilation plus actual SVG namespace/adoption tests qualify those uses. The `title` declaration admits the concrete SVG child used here. This is a disclosed typing accommodation, not a new published package API.

## Native, state and budget checks

The startup path requires the manifest’s SSR mode, binds the derived nested `CanvasProps`, finalizes before opening its loopback listener, and returns the complete `PageResponse`. Page nonce/CSP/no-store policy remains intact. Fixture values are trusted server-resolved presentation data; no arbitrary definition markup/code executes and no business aggregate is calculated in the browser. Numeric geometry is finite and bounded before rendering; card identity and row-count limits are explicit.

The report records **27,938 B estimated Forma gzip** and **31,176 B total JavaScript including the worker**. Those fit both the retained 28,100-B app target and the 28,420-B published Forma cap; the total also fits both ordinary-page and canvas ceilings. The route is explicitly declared as a canvas. I independently recompressed its actual JavaScript with pinned Node22.21.0’s gzip level9: **29,638 B module + 1,538 B worker = 31,176 B**. The Forma figure remains the build’s attribution estimate, not a separately compressed framework bundle. A first cross-check with Python’s different zlib implementation did not produce identical bytes; the reviewer-tool mismatch and exact matched-tool result are preserved in [review-evidence.json](review-evidence.json).

Producer evidence distinguishes serialized input from the actual encoded island envelope: normal envelope **6,524 UTF-8 bytes**, 500-row envelope **92,297 bytes**, each below 1MiB. The oversized 1,206,304-byte UTF-8 input produces **RENDER-502** instead of truncation. The four source controls produce the reported diagnostics: enum dispatch BUILD-610; four-level composition BUILD-403 and BUILD-610; reassigned declared local BUILD-610; negative unary tab index BUILD-207. Independent `cargo test --locked` passed all **three** controls covering wrong typed binding/unbound finalization, missing/Browser modes, geometry, 501 rows and duplicate card keys.

The release benchmark times complete `PageHandle.render` calls and response construction, with fresh contexts/nonces. One first call and 200 sequential subsequent calls per state produce local distributions and throughput; native-walk HTML bytes and process-lifetime high-water RSS are correctly labeled. These measurements neither establish managed capacity nor N3 client/network/database p99. They were inspected as producer evidence rather than silently relabeled as independently rerun benchmarks.

## Browser and independent interaction checks

The producer’s three-engine campaign covers normal, empty, nested-empty, long/non-ASCII/hostile-text and 500-row states. Its script blocks activation until the original root and every descendant are captured, awaits the actual module activation promise, verifies zero initial writes and exact node identity, and proves its mutation observer detects a seeded write. The normal state retains 464 nodes; the largest retains 5,931. Browser reports also record successful native no-JavaScript form posts, accessible labels/live output, numeric SVG, focus and stale-epoch handling without page or CSP errors. The screenshot visibly contains all nine primitives and literal hostile-looking text.

I additionally ran a separate focused Chromium replay against the actual native binary. [Runnable probe](review-probe.mjs), [captured result](review-probe.json).

- Reached Observation with **eight real Tab presses**, typed the value, selected the decision through keyboard input, then submitted with Enter and observed the actual200 acknowledgement. This goes beyond programmatically focusing a submit button.
- Changed the filter to unmatched and observed **900,000 bytes in all six linked views**, then cleared it back to1,140,000. This proves a changed result rather than only checking an unchanged initial label.
- Held the completed old server response until epoch2 was already visible, delivered the old response fully, then checked the resulting state after browser frames. All six views remained on epoch2 and the current result.
- Verified all twelve numeric SVG rectangles’ exact coordinates, sizes and both rectangle/title namespaces; hostile text stayed literal and executed no script. Existing Elements remained connected through interactions.
- Unknown filter and malformed form requests returned400 and422. Page errors and CSP violations remained empty. The probe terminated/reaped only its own loopback server.

## Limits retained

This is a representative full-library footprint and rendering qualification. It does not establish a production application’s eventual import graph or authorize future cap increases. The fixed synthetic form key and acknowledgement are not durable idempotency, authorization or record persistence. Fixed server filter results are not the production query/receipt path. No cold-client twin, human screen-reader pass, full spatial-layout persistence, managed performance or cloud acceptance is claimed. Those obligations remain with APP-012/025–029 and the relevant auth/data/command tickets. The independent published FORMA-246 replay supports the native pattern; its final handback and this app’s CI must still be recorded before clearing the consumer gate.
