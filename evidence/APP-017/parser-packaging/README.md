# APP-017 preparation — `parser-host` packaging and image-bytes qualification (local, 2026-10-29)

Phase 2 execution plan Task A4, under [ADR-009](../../../docs/architecture/ADR-009-PHASE-2-DATA-PLANE.md) D9.1 and B20. Preparation evidence only: APP-017 is not selected, nothing is deployed, no CI run exists for this candidate, and no independent review has happened yet (plan Step 5). Local results are evidence, never a CI pass.

## What changed

| Area | Change |
| --- | --- |
| `parser/` | Promoted from `spikes/APP-004` at `623fcd5` (tree `a28dd91865d2710351a75b4a2b29d63928eacc7a`): every tracked file except the spike README and `collect_metadata.py`, including the vendor's published `Cargo.lock` (force-added, as the spike README requires). `spikes/APP-004/` is unchanged. `Cargo.lock` is byte-identical (`sha256:d26fc437…5f2f`, the APP-004 qualified lock); regenerated fixtures are byte-identical to APP-004's. |
| Guest (the only guest change) | An xlsx config without `sheet` still runs the whole-workbook preflight first; an admitted workbook is then refused `sheet_selection_required` with `available_sheets` in workbook order. Hostile workbooks are refused for their content and never list sheets. Reason codes, `no_revision_created`, caps and every other path are unchanged; the 17 APP-004 tests pass unmodified apart from the guest-path override. |
| Host | `parser-host --job --wasm <path> --wasm-sha256 <hex>` (`parser/host/src/job.rs`): stdin is a 4-byte big-endian config length, the config JSON (`{"profile":"demo","format","sheet"?}`, ≤4,096 B, unknown keys refused, so the guest's lower-only `limits` channel is unreachable) and the input to EOF, read only up to the demo cap + 1 (CSV 10 MiB, XLSX 5 MiB; over is refused `input_limit` before the guest exists). The guest file must hash to `--wasm-sha256` before it is compiled. stdout is the guest's one JSON document; stderr is one line: the `RunReport` without `result`, plus `status`, `reason` and `process_ms`. Exit 0 parsed, 2 refused, 1 failure (no stdout). One `Sandbox`/`Engine` per process. The operator CLI form is kept for `fixtures/qualify.py`. |
| Build identity | `parser/.cargo/config.toml` adds `--remap-path-prefix` for `/usr/local/cargo` and `/source/parser`, the fixed paths of the image build. The extracted image guest contains no build-host path (0 hits; only `/cargo/registry/src`, `/parser/vendor` and rustc's own `/rustc/<commit>` prefixes remain). `[profile.dev.package."*"] opt-level = 2` makes the job-mode tests take ~17 s instead of ~245 s (debug Cranelift compiles the guest per process); it does not touch release artifacts. `parser/write_manifest.py` writes the manifest. |
| Image | `app/Dockerfile` gains a `parser` stage on the same pinned `rust:1.93.1-bookworm` digest: verifies the vendored ZIP, builds the guest and the native `parser-host` with `--locked --release` from `parser/` and its own lockfile (never in `app/Cargo.lock`), and writes `parser-manifest.json` (guest and host digests and sizes; sources digest over all 102 parser files, `Cargo.lock`, vendor provenance, toolchain and `rustc -vV`). The runtime stage ships `/app/parser/parser_guest.wasm`, `/app/parser/parser-host` (root-owned, not writable by uid 65532) and `/app/parser-manifest.json`, and sets `GENUI_PARSER_MANIFEST_FILE`. The stage uses one Cargo job so the image build stays at three aggregate jobs. `.dockerignore` lists every parser file and the three new app sources exactly ([context.json](context.json): 148 files, parser set equal to the checkout's). |
| App | `app/src/supervise.rs`: `run_confined(Command, stdin, &Confinement)` with `env_clear()`, three pipes, `RLIMIT_NOFILE`/`RLIMIT_CORE` only, RSS sampled every 100 ms, stdout ≤129 MiB / stderr ≤1 MiB, kill on deadline, RSS line or output bound. `app/src/imports/sandbox.rs`: `verify_manifest()` (startup integrity; `main.rs` refuses to start on mismatch when the image sets `GENUI_PARSER_MANIFEST_FILE`), `parse()`/`parse_with()` spawning one `parser-host --job` per parse in the 2.5 GiB slot, deadline `min(d, 120 s) + 5 s`, refusing any report that names other guest bytes. |
| Release gate | `scripts/check-parser-record.py --wasm-sha256 <hex>` exits 0 only for a passing record `evidence/APP-017/parser-records/<hex>.json` made by GitHub Actions (run URL and commit); `compose` writes a record from the extracted bytes, the hostile-suite log (which must name those bytes in its `QUALIFIED_WASM` line), the fixture summary and the manifest. `.github/workflows/parser-release.yml` runs the parser source suite, builds the image, extracts the guest, host and manifest, runs the hostile suite and job tests against the extracted bytes (and the extracted host), runs `fixtures/qualify.py` in image-bytes mode and `qualify_image.py`, composes the record and runs the check, uploading the record as an artifact. |
| Qualification inputs | `fixtures/qualify.py --parser-host/--parser-wasm/--parser-manifest` qualifies image bytes without rebuilding: the guest digest must equal the manifest's, and the manifest's sources digest, `Cargo.lock`, vendor provenance and rustc release/commit/LLVM must equal what this checkout and APP-004's recorded rustc give (D9.1 binds sources, lockfile, vendor and toolchain, not cross-host bytes). The default APP-004 mode is unchanged. `app/runtime/qualify_image.py` adds the parser artifact, startup and job checks. |

## Identity chain (local image `genui-app017-parser-local`, arm64)

| Link | Value |
| --- | --- |
| Sources (102 files under `parser/`) | `sources_sha256` `351a64b6aa8ed41c66ba770a73f8065f166a368215076c246b0cddb59c60c7f6`, computed identically in the image build and over the checkout |
| `Cargo.lock` / vendor provenance | `d26fc437…5f2f` / `5259d9a0…20e1a` |
| rustc in the build stage | 1.93.1, commit `01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf`, LLVM 21.1.8 (APP-004's recorded commit), host `aarch64-unknown-linux-gnu` |
| Image guest | `a1d3bb3716b1b64c675c1b679369d4b325f1c727e5207c51581b454f81c64774`, 855,790 bytes ([manifest](image-parser/parser-manifest.json), [digests of the extracted files](image-parser/sha256sums.txt)) |
| Image host (linux/arm64) | `44672951f0b266309f507b477eadf094193e44f1609b7303166426d01ba82934`, 10,795,440 bytes |
| Startup | the image's genui accepts its manifest (then stops at the absent credential file, as before); a copy naming another guest digest is refused: `parser manifest refused: parser artifact digest mismatch` |

The guest digest differs from APP-004's (`aed4238c…` recorded, `b6f46ff2…` rebuilt in this worktree) because the guest changed, the rustflags changed and the paths are remapped; it is qualified here as its own bytes. Whether the CI (amd64) image build yields the same guest bytes is not yet observed.

## Results

All commands ran on macOS 26.1 arm64, Docker 29.8.2, `CARGO_BUILD_JOBS=2` (the image's parser stage at 1), tests one thread at a time.

| Check | Result | Log |
| --- | --- | --- |
| Tests first: parser job tests before `job.rs` | 7 failed | [red-parser.log](red-parser.log) |
| Tests first: `supervise::` and `imports::` with `todo!()` bodies | 15 failed, 1 ignored | [red-app.log](red-app.log) |
| Tests first: `test_check_parser_record.py` without the script | 6 tests, 10 failures | [red-script.log](red-script.log) |
| APP-004 suite as `parser.yml` runs it (`spikes/APP-004`, unchanged): vendor, fixtures unchanged, fmt, guest build, both clippy, tests | 1 + 16 passed | [app004-baseline.log](app004-baseline.log) |
| `parser/`: vendor verified, fixtures regenerate byte-identically, fmt, guest build, `clippy --workspace --all-targets` and wasm32 clippy with `-D warnings`, `cargo test -p parser-host` | 1 + 7 job + 17 qualification passed | [parser-source-suite.log](parser-source-suite.log) |
| Hostile suite and job tests with `PARSER_GUEST_WASM` = the guest extracted from the image (native macOS test host; the linux host cannot run here) | 1 + 7 + 17 passed; `QUALIFIED_WASM sha256:a1d3bb37…` | [hostile-suite-image-bytes.log](hostile-suite-image-bytes.log) |
| `fixtures/qualify.py` image-bytes mode, extracted guest + manifest | passed, 28 parser cases, binding recorded | [summary](fixture-qualification-image-bytes/summary.json) |
| Same with a manifest whose `sources_sha256` was altered | refused: `not bound to this checkout's parser/: sources_sha256` | [log](fixture-qualification-tampered-manifest.log) |
| `fixtures/qualify.py` default APP-004 mode (regression of the edit) | passed, 28 cases | [summary](fixture-qualification-default/summary.json) |
| `docker build --tag genui-app017-parser-local --file app/Dockerfile .` | built | [image-build.log](image-build.log) |
| `qualify_image.py` (existing ownership, HTTP and supervisor checks plus parser artifacts, startup accept/refuse, three `--job` runs under `--network=none --read-only --cap-drop=ALL` with `env -i`: CSV parsed, XLSX `sheet_selection_required` listing `Inventory`, `Other`, formula refused; ~0.7 s per job including module compilation) | passed | [image-qualification.json](image-qualification.json) |
| Local record composed from the above | passed as a record, `origin.kind = local` | [local-record/](local-record/) |
| `check-parser-record.py --wasm-sha256 a1d3bb37… --records local-record` | refused, `origin_not_ci` (exit 1) | [record-checks.log](record-checks.log) |
| `check-parser-record.py --wasm-sha256 a1d3bb37…` (default `evidence/APP-017/parser-records/`) | refused, `no_record` (exit 1) | [record-checks.log](record-checks.log) |
| `scripts` unit tests (`discover -s scripts`), `check-schema-pins.py`, `app/runtime` unit tests, `board.py check` | 11 OK; pins OK; 39 OK; OK | — |
| `app/`: `npm --prefix app/admin` ci, typecheck, build (needed by the bin tests); `cargo +1.93.1 fmt --check`; `clippy --locked --lib --tests -- -D warnings`; `test --locked --lib -- --test-threads=1`; `cargo +1.85.0 check --locked --lib --tests` | all pass; 43 passed, 3 ignored | [app-checks.log](app-checks.log) |
| `imports::sandbox::tests::actual_parser_host_parses_refuses_and_lists_sheets` (`--ignored`) with `GENUI_TEST_PARSER_MANIFEST` naming the locally built host and guest: CSV parsed with matching input digest, XLSX without a sheet lists `Inventory`, `Other`, `Other` selected, formula refused; peak child RSS ~46 MB | passed | [app-actual-parser.log](app-actual-parser.log) |

## Limits and open questions

- No CI run, no parser record in `evidence/APP-017/parser-records/` and no independent review exist yet; the release check correctly refuses this candidate today.
- The image-bytes hostile suite here ran the extracted guest under a macOS-built test host; the extracted linux host is exercised only inside the image (`qualify_image.py`) and, in CI, by the job tests with `PARSER_HOST_BIN`.
- `--remap-path-prefix` names the image build's fixed paths; builds elsewhere keep their own paths and are not release bytes.
- Interface additions beyond the plan's text: `Killed::InputFailed` (a failing input stream kills the child so a truncated input can never complete), `ParseRefusal` carries `Deadline`/`ResourceLimit`/`Crashed`/`Failure`, the stderr line omits `result` (the document travels once, on stdout), and `--wasm/--wasm-sha256` arguments re-bind each job to the verified digest.
- The guest is compiled per job (~0.7 s on the local arm64 image); precompiling would need a separate identity decision.
- Cleanup: the local image and the parser stage's `/source/parser/target` cache mount (541 MB) were removed after qualification; ordinary reclaimable layer cache remains in the Docker builder, as for earlier image builds.
- Not run here: `python3 scripts/check-foundation.py` (it reinstalls and rebuilds the whole scaffold and CDK; its app pieces are covered by the rows above) and the Docker PostgreSQL campaign (no storage code changed).
