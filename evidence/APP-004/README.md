# APP-004 local qualification — 2026-10-28

The actual CSV/XLSX WASM candidate passes **17 tests (16 integration checks and one native-decoding deadline regression)**, including **256 bounded mutation cases**, with zero module imports and successful valid parses after resource failures. The normal build and an independent fresh-target rebuild under the exact Rust toolchain produced identical WASM bytes. Independent review and Linux CI on the committed candidate remain required; this record does not move the ticket to Done.

The implementation and repeatable commands are in [the spike README](../../spikes/APP-004/README.md). APP-002's contract edition remains `impl-2026-10-28.1`. These results qualify a synthetic parsing boundary, not persisted SourceRevisions, Parquet, an import API, a deployed host, or phase completion.

## Candidate identity and environment

- WASM: `sha256:aed4238c91d89fdd8face76face5f2d1e4c414c272c53fdb8f22f4f3fc1be1ba`, **854,630 bytes**.
- Cargo lock: `sha256:d26fc437c35691f64c1e1d47e398148f30a886782812bb4cd2bc75066c3c5f2f`.
- Rust/Cargo **1.93.1**, rustc commit `01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf`, LLVM 21.1.8, target `wasm32-unknown-unknown`; macOS 26.1 arm64 host.
- Local fixture tool: Python **3.11.0**, zlib **1.2.12**. CI explicitly selects Python **3.11.14** on Ubuntu 24.04 and verifies regenerated fixture bytes.
- Wasmtime **36.0.19**; calamine **0.37.0**; csv-core **0.1.13**; quick-xml **0.42.0**; ZIP **9.1.3 with the documented one-line manifest patch**. All dependency source/feature/license details and the rejected older candidate are in [DEPENDENCIES.md](DEPENDENCIES.md).
- [build-metadata.json](build-metadata.json) binds the exact source files, fixtures, dependency graph and toolchain. Its checkout HEAD is the parent repository state at capture; APP-004 was uncommitted. The eventual review must bind the actual committed candidate, not equate that earlier HEAD with these changes.

The published ZIP source archive checksum and all 66 source-file hashes are retained under `spikes/APP-004/vendor/`. [vendor-verification.txt](vendor-verification.txt) records successful verification. No parser library source was edited; only the wasm32 JavaScript entropy feature was removed from the package manifest. This locally patched packaging candidate is explicitly not an upstream published fix.

## Acceptance evidence

| Obligation | Actual check and result |
| --- | --- |
| CSV and selected-sheet multi-sheet XLSX literal values/source coordinates | `selected_sheet_literals_preserve_exact_numbers_and_coordinates` and `csv_keeps_literal_text_logical_records_and_physical_spans` pass. The two sheets produce different selected results; integer `9007199254740993` is exact, decimal `123.45` retains coefficient `12345`/scale 2, booleans/text remain literal. CRLF, bare CR, quoted multiline spans, blank lines and EOF without newline pass. Actual XLSX `0e3` and `-0e3` become canonical integer `0`, and duplicate numeric value elements are refused before calamine selection. [XLSX report](xlsx-report.json), [CSV report](csv-report.json). |
| No file/network/environment/credential interface | The actual compiled module's imports are checked before every instance can be created; both reports record `interface_imports: []`. Four deliberately imported interface controls are rejected by `runtime_refuses_system_imports`. Guest instantiation uses an empty import list and no WASI linker. |
| Compressed/expanded/entry/string/cell/dimension caps | `resource_profiles_have_explicit_lower_only_controls`, `archive_paths_counts_and_actual_expansion_are_bounded`, `metadata_counts_and_selection_need_actual_validated_content` pass. Declared billions of strings, more actual strings than the declaration, duplicate cells, impossible declared/actual dimensions, deep XML, excessive entries/cells/sheets, oversized text and output are refused. Actual CSV 10 MiB admission and 10 MiB + 1 byte host refusal are exercised; the 10 MiB single field is then refused by its text cap. |
| Actual demo hostile workbook | A multi-sheet XLSX plus bomb payload is **4,248,454 compressed bytes** (below 5 MiB), with **57,673,592 declared expanded bytes** (above 50 MiB). It is refused as `expanded_limit`, without a revision, in **203 ms**, using **5,570,560 bytes** of guest linear memory; a subsequent valid workbook parses. The report and fixture-generator path are in [qualification-final.log](qualification-final.log). Metadata-only rejection is intentional before expensive inflation; a separate lying-size case exercises actual-read/CRC/expansion refusal. |
| Time, fuel, memory and recovery | The actual parser is interrupted by one unit of fuel and by a reduced 1 ms wall deadline while parsing CSV; both refuse and the next valid parse succeeds. Actual guest instantiation under a reduced 1 MiB memory budget is refused. Controlled zero-import WASM loop/growth modules independently prove epoch interruption and 2 MiB growth denial. Host output-copy limit refusal also recovers. The native-decoding regression deliberately completes after its deadline, then refuses late success and proves subsequent health; all five host ceilings have explicit increase-refusal tests. Caps use the same runtime paths as the configured 2 GiB / 120 s defaults. |
| Macro/external/general formula refusal | Macro bytes and macro content types, external relationships, DTDs, formulas with cached values, and formulas on an unselected sheet are refused. Policy is `reject`; no cached-formula exception or execution engine is enabled. Strings beginning `=` stay inert text. |
| Archive identity/advisory regressions | Mismatched local/directory names and CRCs, unsupported legacy/encrypted methods, symlink/device/privileged modes, extra fields, earlier/missing/shifted footers and path traversal are refused before dependency materialization. The final dependency uses the current patched upstream ZIP reader; no ZIP 8.6 exception is accepted. |
| Bounded hostile corpus / M20 | **256 deterministic mutations**, seed `0x7a614c32`, over actual CSV/XLSX inputs: bit flips, truncation, invalid insertion and null runs. Each instance has 64 MiB memory, 20,000,000 fuel and a 2 s wall deadline. Every case completes as parsed or refused, and post-corpus health succeeds. CI runs the same test. This is a bounded mutation corpus, not exhaustive or coverage-guided fuzzing. |
| Reproducible parser build / license | Original published-source/license hashes verified; exact direct/transitive lock retained. [reproducibility.json](reproducibility.json) shows matching bytes from the normal build and a fresh target rebuild using the exact 1.93.1 toolchain. Both native and wasm-target lint and formatting checks pass. |

The platform and demo caps are separate named profiles, documented in the spike README. A valid small workbook also passes the platform profile. Full platform-capacity workloads and the complete demo host envelope are not qualified here.

## Commands actually run

Working directory: `spikes/APP-004`. Build concurrency was capped at two jobs; tests ran one at a time.

```sh
python3 make_fixtures.py
python3 verify_vendor.py
cargo fmt --all --check
CARGO_BUILD_JOBS=2 cargo build --locked -p parser-guest --target wasm32-unknown-unknown --release
CARGO_BUILD_JOBS=2 cargo clippy --locked --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=2 cargo clippy --locked -p parser-guest --target wasm32-unknown-unknown -- -D warnings
CARGO_BUILD_JOBS=2 cargo test --locked -p parser-host --no-run --message-format=json
/usr/bin/time -l <qualification-test-executable> --test-threads=1 --nocapture
CARGO_BUILD_JOBS=2 cargo run --locked -p parser-host -- target/wasm32-unknown-unknown/release/parser_guest.wasm fixtures/valid.xlsx fixtures/xlsx-config.json
/usr/bin/time -l target/debug/parser-host target/wasm32-unknown-unknown/release/parser_guest.wasm fixtures/literal.csv fixtures/csv-config.json
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=target/reproducibility cargo build --locked -p parser-guest --target wasm32-unknown-unknown --release
cargo tree --locked -p parser-guest --target wasm32-unknown-unknown -e features
python3 collect_metadata.py
```

The full `cargo test --locked -p parser-host -- --test-threads=1 --nocapture` run also passed; [complete-suite-final.log](complete-suite-final.log) records the one native-decoding regression and all 16 integration checks, totaling 17 tests. [deadline-review-regression.log](deadline-review-regression.log) retains the focused review fix. All final commands exited 0. [qualification-command.txt](qualification-command.txt) preserves the resolved test executable path; [qualification-final.log](qualification-final.log) records **16 integration checks passed / 0 failed** in **12.49 s**. [qualification-resources.txt](qualification-resources.txt) records **97,091,584 bytes maximum process RSS** on macOS. This includes the test process and JIT; it is distinct from each instance's linear-memory measurement. [csv-cli-resources.txt](csv-cli-resources.txt) records a separate CLI run, including startup/JIT. Per-report `elapsed_ms` starts before store/instance setup and includes native decoding/digest work, but excludes module compilation, so the 1–2 ms small-input report times are not cold application latency claims.

Build/lint outputs are `guest-build-final.log`, `host-test-build-final.log`, `clippy-final.log`, `guest-clippy-final.log`, `format-final.log` and `clean-rebuild.log`. Empty formatting output is the successful formatter's normal behavior. [local-validation.json](local-validation.json) records the final commands/results and material boundaries. `.github/workflows/parser.yml` implements the committed-candidate CI path; the parent will record the actual CI URL/status after publishing the candidate.

## Preserved failures and corrections

- **Unmodified current ZIP:** the initial actual module imported wasm-bindgen, so 9 of 10 initial tests failed at the interface gate. Kept in `initial-test.log`; the gate was not relaxed. The later stale-artifact retest is `import-fix-test.log`.
- **Backend attempt:** selecting getrandom's unsupported backend while retaining the forced JavaScript feature failed to compile with missing `Error::WEB_CRYPTO`. The final manifest patch removes that feature; the failure is not reported as successful qualification.
- **Older dependency trial:** ZIP 8.6/calamine 0.36.1 compiled with no imports; `qualification-attempt-2.log` records 9 passed / 1 failed. The failure exposed CSV physical-line starts after CRLF. Own line tracking and focused regression cases fixed it. The older dependency candidate was then discarded after advisory review.
- **API adaptation:** preliminary logs retain source/API compile failures while changing calamine/ZIP versions (`compatible-dependencies-build.log`, `zip9-manifest-probe.log`, `current-candidate-build.log`). Final source builds at the selected exact versions; no failed build is treated as a pass.
- **Packaging probe:** the first scratch probe removed the JavaScript feature from both dependency and dev-dependency declarations. The final vendored patch removes only the required normal dependency line; its exact reconstructed source hashes and zero-import tests are the accepted evidence.
- **Lint:** `clippy-attempt.log` retains initial borrow, iterator and missing unsafe-ABI safety-documentation findings. Final lint passes after correction; the unused deallocation export was removed.
- **Independent deadline review:** the initial host relied on epoch scheduling and could accept successful native JSON decoding after the deadline. The corrected host starts its clock before setup/timer launch, uses the remaining timer budget, and fences success after native decoding and digest work. The deterministic delayed-decoder regression passes. All exposed host limits now refuse increases; trap/refusal reasons are preserved.
- **Independent numeric review:** `numeric-review-before.log` reproduces two failures against the preceding WASM candidate: `0e3` returned noncanonical `0000`, and duplicate numeric value elements silently chose a value. Canonical-zero handling and one-value-per-cell admission fix both. The final suite includes actual XLSX regression fixtures, and its new artifact digest supersedes that earlier candidate.
- **Toolchain cache:** [reproducibility-initial-failure.json](reproducibility-initial-failure.json) records nonidentical artifacts from the pre-pin stable-alias cache and the exact 1.93.1 installation. Their standard-library source paths differed. The owned WASM target cache was set aside, rebuilt under the exact pin, and then matched the second clean build byte for byte. Final reports all name the corrected artifact digest.

## Scope remaining after this qualification

The parser emits inspection-required values, original text and coordinates; it does not confirm units/time, persist Parquet, publish a `SourceRevision`, bind organization storage, or write audit/job records. Input/configuration/module digests are real qualification provenance, not fabricated persisted revisions. These integrations remain the owning import/job tickets.

The default 2 GiB guest limit is verified through the runtime configuration and reduced-budget refusal paths, not by allocating 2 GiB locally. The 120 s deadline is exercised at shorter test deadlines on the same path. Actual demo expanded-cap refusal is measured; full platform maximum capacity and demo §11.2's combined 2 vCPU / 8 GiB task require later environment trials. Library-format compatibility is intentionally conservative and documented. JIT/native host RSS, concurrent job scheduling, a fixed artifact allowlist, HTTP transport limits, audit/alert routing and sustained fuzzing remain integration/security-review obligations.

No framework/auth change, upstream release, cloud resource, production deployment or customer data was used. FormaStack-blocked tickets remain independently blocked; this qualification does not unblock them.

## Committed candidate and independent acceptance

Candidate `05c95a4bfaad81cdd103b8c1194e75922e84f023` passed [Linux parser CI](https://github.com/getforma-dev/forma-app-platform/actions/runs/40759765118), baseline, contract and planning checks. [Independent review](review.md) found no remaining parser blockers after the timing and numeric fixes. The separate APP-005 query check failed and is not reported as passing; [all CI states](ci.json) are retained. Ticket closure still requires landing.
