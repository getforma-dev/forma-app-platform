# APP-004 independent review

Reviewed candidate: `05c95a4bfaad81cdd103b8c1194e75922e84f023`. **No blocking findings remain in the declared parser qualification scope.** The reviewer read the implementation and proposed corrections but did not implement the parser source. CI and landing remain separate completion conditions.

The review covered APP-004 and its design references, guest/host source, exact dependencies and packaging decision, fixture generator, resource/refusal/recovery tests, mutation corpus, provenance records, final reports and workflow. The parser returns unconfirmed literal inspection data; it does not claim that a C05 SourceRevision, Parquet file, authenticated import or persisted audit record already exists.

## Boundary and evidence

- All imports are rejected before instantiation, and the actual parser module is instantiated with an empty interface. WASI, file, environment, credential and network callbacks are absent. Uploaded bytes cannot select a module in this operator harness. The integrated worker must later pin the admitted built artifact.
- Input/configuration caps precede guest allocations. ZIP directory/local-header validation precedes dependency parsing; actual bounded inflation/CRC checks and XML admission precede calamine construction. This is important for declared shared-string counts, which can otherwise cause eager allocation. Exact coordinates, counts, dimensions, XML nesting and output size are constrained. Macros, external links, DTDs and every formula node are refused, including formulas outside the selected sheet.
- Numeric lexemes are retained from worksheet XML and decoded without passing through floating-point values. CSV remains inert text with explicit physical/logical coordinates. The narrow format subset and permissive CSV dialect are documented rather than presented as universal spreadsheet compatibility.
- Wasmtime enforces linear-memory, stack, fuel and epoch limits. The host serializes an engine's jobs, joins/cancels each timer and drops each store. The final elapsed-time fence also covers native decoding/digests. Concurrent integrated scheduling needs its own coordinated engines/deadlines; this spike does not establish host-wide RSS or the full deployed 2 vCPU / 8 GiB envelope.
- Seventeen local tests pass, including 256 bounded mutations and valid-parser recovery after failures. The actual demo bomb is below the compressed cap and above the expanded cap; pre-materialization refusal is intentional. These are finite qualification cases, not exhaustive fuzzing or a security certification.
- The final WASM is 854,630 bytes with SHA-256 `aed4238c91d89fdd8face76face5f2d1e4c414c272c53fdb8f22f4f3fc1be1ba`. Both clean exact-toolchain builds match. Every final source hash in `build-metadata.json` matches the reviewed source. The capture's earlier checkout HEAD is correctly distinguished from the source hashes and this committed candidate.
- ZIP 9.1.3 has one explicit local manifest feature deletion, not a library-source patch or an upstream release. The verifier reconstructs the published manifest and verifies all 66 original files. The published vendor lockfile is tracked despite its original ignore rule. Earlier incompatible and advisory-affected candidates remain rejected, with primary references and failure logs retained.

## Findings resolved during review

1. **Late native completion:** epoch interruption alone could allow native JSON decoding to finish past the deadline and return success. The clock now starts before setup/timer launch; success is fenced after decoding and digest work. A deterministic decoder delay exceeds the reduced budget and is refused, followed by a healthy parse. All five exposed host ceilings also reject increases.
2. **Noncanonical zero:** `0e3` and `-0e3` originally produced `0000`. Expansion now normalizes zero to the canonical integer string `0`. The old-artifact failures remain in `numeric-review-before.log`.
3. **Ambiguous worksheet value:** duplicate value elements could leave preflight lexeme selection ambiguous relative to the spreadsheet reader. They now fail before calamine selection. A real malformed XLSX regression retains the old failure and checks the stable refusal.

## Independent checks performed

- Ran `python3 verify_vendor.py`: all 66 published files and the exact manifest-only change verified.
- Recomputed the final WASM digest and every recorded source hash; all matched.
- Built three separate XLSX controls from the valid workbook and ran the actual compiled host/module: positive and negative exponent-zero values both returned integer `0`; duplicate numeric value elements returned `cell_value_duplicate`. These checks used the real WASM path, not a direct native guest function.
- Ran the focused `native_decode_finishing_after_deadline_cannot_return_success` and `host_limit_overrides_cannot_expand_the_admitted_envelope` Cargo tests under the exact pinned toolchain. Both passed.
- Checked authored Rust/Python/workflow whitespace. The broad Git whitespace check flags preserved CRLF fixture bytes, verbatim failure logs and unchanged published vendor text; those bytes are intentionally retained rather than normalized and invalidating provenance.

At review, [planning CI](https://github.com/getforma-dev/forma-app-platform/actions/runs/40759765025) passed. [Parser CI](https://github.com/getforma-dev/forma-app-platform/actions/runs/40759765118), baseline, contract and query checks for this revision were still running. This review does not claim they passed or close the ticket before their results and landing are recorded.

CI follow-up: the [parser workflow](https://github.com/getforma-dev/forma-app-platform/actions/runs/40759765118) passed on this exact revision, including clean Linux build, vendor/fixture checks, both target lints and the complete test suite. Baseline, contract and planning also passed. The separate APP-005 query workflow failed its admission control and remains under its own review; [CI record](ci.json) preserves the distinction. Parser source acceptance is satisfied; landing is still pending.
