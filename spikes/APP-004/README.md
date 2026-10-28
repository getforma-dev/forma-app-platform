# APP-004 parser boundary qualification

This synthetic spike runs the actual CSV and XLSX parsers inside a `wasm32-unknown-unknown` module. Wasmtime rejects every imported function or other interface, and instantiates it with an empty import list. Only bounded input/configuration buffers and a bounded result buffer cross the host boundary. No WASI linker, filesystem, network, environment, clock, entropy or credential callback is installed. The operator chooses the built module; uploaded bytes cannot supply executable WASM.

The owning ticket is [APP-004 on the ticket board](../../Tickets/TRACKER.md). This is a parser qualification, not a deployed import service. [Recorded evidence](../../evidence/APP-004/README.md) distinguishes successful checks, rejected candidates, review/CI, and later integration requirements.

## Run from this directory

Rust **1.93.1** and the `wasm32-unknown-unknown` target are pinned in `rust-toolchain.toml`. Python **3.11.14** is the CI fixture tool; Python 3.11's standard library is sufficient. Cargo's configuration is local to this directory, so change into it before invoking Cargo.

```sh
cd spikes/APP-004
python3 verify_vendor.py
python3 make_fixtures.py
cargo fmt --all --check
CARGO_BUILD_JOBS=2 cargo build --locked -p parser-guest --target wasm32-unknown-unknown --release
CARGO_BUILD_JOBS=2 cargo clippy --locked --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=2 cargo clippy --locked -p parser-guest --target wasm32-unknown-unknown -- -D warnings
CARGO_BUILD_JOBS=2 cargo test --locked -p parser-host -- --test-threads=1 --nocapture
CARGO_BUILD_JOBS=2 cargo run --locked -p parser-host -- target/wasm32-unknown-unknown/release/parser_guest.wasm fixtures/valid.xlsx fixtures/xlsx-config.json
CARGO_BUILD_JOBS=2 cargo run --locked -p parser-host -- target/wasm32-unknown-unknown/release/parser_guest.wasm fixtures/literal.csv fixtures/csv-config.json
```

`fixtures/` contains deterministic synthetic inputs, including a multi-sheet workbook. CI regenerates the fixtures and refuses byte changes, verifies all vendored source hashes, builds the real guest, lints both host and guest targets, and runs the same hostile-input/resource/mutation tests. No external service or local spreadsheet application is required. The 256-case mutation suite is a bounded fuzz smoke test, not coverage-guided fuzzing or a complete security assessment.

## Explicit profiles

Binary MiB/GiB units apply. The demo profile implements delivered demo A6/M19 first; the platform profile preserves the separate platform A4 values. A caller must name the profile and format. Test-only configuration can reduce a limit, never increase it.

| Limit | `demo` | `platform` |
| --- | ---: | ---: |
| CSV input | 10 MiB | 100 MiB |
| CSV logical records | 100,000 | 1,000,000 |
| XLSX compressed input | 5 MiB | 10 MiB |
| XLSX total expanded bytes | 50 MiB | 100 MiB |
| XLSX rows per sheet | 10,000 | 10,000 |
| XLSX columns per sheet | 32 | 32 |
| Guest linear memory | 2 GiB | 2 GiB |
| Guest execution wall deadline | 120 seconds | 120 seconds |
| Instruction fuel | 5,000,000,000 | 5,000,000,000 |

Fuel is an instruction budget, not a duration. Both fuel metering and an independent epoch-interruption timer are enabled. The host serializes jobs on an engine, cancels/joins each timer, destroys each store after a job, and remains usable after traps. An elapsed-time fence after native output decoding and digest calculation rejects late success even if timer scheduling or non-WASM work outlasts the deadline. Every exposed host limit is lower-only as well. The integration worker must preserve this lifecycle or give each concurrently running job its own engine/deadline coordination.

Additional conservative admission limits for this candidate: 512 archive entries; 16 sheets; 100,000 declared and actual shared strings; 320,000 aggregate cells in demo and 1,000,000 in platform; 32 columns in CSV; 8 KiB XML tokens/CSV fields and 2,000 Unicode characters per returned text value; XML nesting depth 32; 128 MiB serialized output; 512 KiB WASM stack; one instance, memory and table, with at most 10,000 table elements. These are explicit narrower admission choices, not additional capacity promises. They can be raised only with review and fresh measured qualification.

The 2 GiB limit applies to guest linear memory, not all host RSS. JIT compilation, input storage and JSON decoding also use host memory. The measured test process RSS is recorded separately. Demo §11.2's combined **2 vCPU / 8 GiB** host envelope, simultaneous query/cache/parser load, and full platform maximum-scale acceptance remain later integrated trials. This spike neither allocates 2 GiB to manufacture a test result nor claims that local execution proves that deployed envelope.

## Admission before materialization

1. The host caps configuration and input bytes before guest allocation. The CLI also bounds reads; it is an operator harness, not an upload endpoint.
2. Guest ZIP admission checks the actual footer, directory span/count, methods and encryption flags, local/central names, CRCs and sizes, paths, mode bits, payload ranges and nonoverlap before `ZipArchive::new`. Only stored/deflated classic ZIP is admitted. ZIP64, data descriptors, archive comments, extra fields, ambiguous footers, Unicode filename aliases, hidden gaps, duplicate case-folded paths, symlinks, privileged modes and devices are refused.
3. Each entry is inflated through a remaining-budget-plus-one reader; actual cumulative expanded bytes and CRC/size consistency are checked. A lying directory cannot disable the actual byte bound. No filesystem extraction API is called.
4. All XML parts are scanned before `calamine::Xlsx::new`: shared-string declarations **and actual entries**, sheet/entry/cell counts, declared dimensions **and actual coordinates**, token/depth limits, DTDs, macros, external relationships and formulas. This ordering prevents calamine's declared `uniqueCount` reserve from preceding admission. Expanded data is bounded even before the XML scan. Worksheets use calamine's cell iterator, not its dense-range allocation API.
5. Output is bounded while accumulating cells and again during serialization/copying. Any refusal creates no revision. Runtime traps retain a stable reason and release the instance.

The accepted OOXML subset is deliberately conservative: UTF-8 XML, canonical `xl/workbook.xml`/relationship paths, explicit cell coordinates and an explicit sheet selection. It is not universal Excel-format compatibility. Real workbook compatibility must be broadened through fixtures and review, without weakening the resource boundary.

## Literal interpretation and provenance

CSV uses pinned csv-core's **permissive** comma/quote dialect; it is not advertised as strict RFC validation. UTF-8 is required. CSV cells remain text, including large digits and strings beginning `=`. No header inference, formula evaluation, unit or time inference occurs. Logical-record coordinates and inclusive physical-line spans preserve quoted newlines, CRLF, bare CR, blank lines and a final line without a newline.

XLSX returns the selected sheet's literal text, boolean and numeric values with one-based sheet/row/column coordinates. Numeric XML lexemes are retained and converted directly to exact integer strings or coefficient/scale decimals (38 digits, scale up to 18); they never round-trip through calamine's `f64`. Positive-exponent zero stays canonical `0`, including negative zero; duplicate value elements in one cell are refused before either parser can select a value. Calendar/serial interpretation and units remain unconfirmed. Empty/error/unsupported or out-of-domain cells follow the explicit parser refusal/literal branches. Cells on other sheets never silently replace the selected sheet.

The formula policy is **reject**: any XML formula node rejects the workbook, including one with a cached value or on an unselected sheet. No permitted cached-formula execution policy exists. Macros, binary workbook parts, external links and DTDs are refused. Text resembling a formula stays inert text.

Outputs identify parser version 1 and contract edition `impl-2026-10-28.1`; every report binds input bytes, canonical host configuration bytes and the actual WASM artifact with SHA-256. These are immutable qualification/provenance records. The output is explicitly `inspection_required`, not a confirmed C05 `SourceRevision`. User-confirmed column types/units/time mapping, schema hashing, Parquet creation, immutable revision persistence, organization-bound storage, audit and job states remain the import integration work. This spike does not fabricate those records or modify the locked contracts.

## Dependency decision

| Component | Exact version | License | Role |
| --- | --- | --- | --- |
| calamine | 0.37.0 | MIT | selected-sheet XLSX cell parser |
| csv-core | 0.1.13 | MIT OR Unlicense | bounded incremental CSV parser |
| quick-xml | 0.42.0 | MIT | XML admission before calamine |
| zip | 9.1.3, local manifest-only packaging patch | MIT | in-memory ZIP reading/deflate |
| Wasmtime | 36.0.19 LTS | Apache-2.0 WITH LLVM-exception | isolated host, fuel/epoch/memory bounds |
| serde / serde_json | 1.0.228 / 1.0.145 | MIT OR Apache-2.0 | finite configuration/result data |
| sha2 / anyhow | 0.10.9 / 1.0.100 | MIT OR Apache-2.0 | host digests / error propagation |

Every transitive version and registry checksum is locked. The metadata evidence records licenses and resolved features; `guest-features.txt` records the actual guest-only feature graph. Wasmtime's default features are off; only `cranelift`, `runtime`, `std`, `wat` are enabled. Calamine and ZIP default features are off; ZIP enables `deflate`, not encryption, legacy compression, filesystem extraction calls or WASI.

**This is not an unmodified published ZIP build.** Unmodified 9.1.3 forces getrandom's JavaScript feature on wasm32 and introduces a `wasm-bindgen` import, violating the boundary. Combining that feature with the unsupported backend also failed to compile. The final candidate keeps all 66 files from the registry archive and deletes exactly one `features = ["wasm_js"]` line from the wasm32 getrandom dependency in `Cargo.toml`; no library source is changed. `.cargo/config.toml` selects getrandom's documented `unsupported` backend for wasm32. This read-only parser needs no entropy; any accidental entropy call fails. Host randomness is unaffected.

[Vendor provenance](vendor/provenance.json), the [one-line diff](vendor/zip-9.1.3-manifest.patch), and `verify_vendor.py` bind the published archive checksum and every original file. The verifier reconstructs the original manifest and checks its published hash. The patched package has not been published upstream. Maintainers must carry/remove the patch deliberately after an upstream feature fix and rerun the complete boundary suite. When staging, include the vendor's **published `Cargo.lock` with `git add -f`**, because its untouched original `.gitignore` excludes that file; CI verifies the complete published file set.

An earlier ZIP 8.6/calamine 0.36 candidate produced zero imports but was rejected as the final choice after the advisory review. No old-version security exception is being accepted. The [dependency findings](../../evidence/APP-004/DEPENDENCIES.md) record primary advisory URLs, affected paths, reachability reasoning and the current-version replacement. Archive guards remain defense in depth; they do not establish that a dependency is vulnerability-free.

## Reproducibility and limits

The evidence includes a clean build in a second target directory with the same toolchain/configuration, compared byte-for-byte to the normal WASM artifact. That qualifies reproducibility in the recorded environment, not identical binaries on every host. Fixture content and behavior are checked again on Linux CI.

Remaining integration obligations include a fixed built-artifact allowlist; bounded transport; job cancellation/audit/alert routing; confirmed mappings and Parquet; organization storage isolation; concurrent-job scheduling; full workload capacity; longer coverage-guided fuzzing and independent security review. Neither parser failure nor a missing integration control is permission to bypass the boundary. No customer data, cloud deployment or paid resource was used for this spike.
