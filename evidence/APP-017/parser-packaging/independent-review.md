# APP-017 preparation: independent review of parser-host packaging (Phase 2 Task A4)

| | |
| --- | --- |
| Reviewed | `583c112` (promote `spikes/APP-004` to `parser/`), `b32c6ad` (job mode, supervisor, sandbox wrapper, Dockerfile stage, record check, release workflow), `460ec70` (local evidence), merged to `phase-one-spikes` as `d79016a` |
| Tree state | Every file this slice touches is byte-identical at `d79016a` and at `65369a4` (later commits touch only APP-073 files). Local runs used the working tree at `7d566b2` |
| Controlling text | ADR-009 D9.1, D9.2, B20, the confinement paragraph (M4) and ADR-007 D7.10's child controls; plan Task A4; `spikes/APP-004` and `evidence/APP-004`; design §SR5 (and SR8 for the release gate); the implementer's [README](README.md) |
| Method | Read every changed line. Ran the suites below. Probed the boundary three ways: with the actual `parser-host`; with a scratch crate that holds verbatim copies of `app/src/supervise.rs` and `app/src/imports/sandbox.rs`; and against `scripts/check-parser-record.py` with crafted inputs. Ran a differential of the old and new guest, emulated the `.dockerignore` patterns over the working tree, and measured fuel and memory at the profile caps. No image was built and nothing was committed. The only file written in the repository is this one, apart from build output under the git-ignored `target/` directories. |
| Reviewer | Claude (independent of the implementer), 2026-10-29 |

**Verdict: changes requested.** The sandbox boundary held against every attempt: no blocker. The release gate, which is the D9.1 allowlist, has three major defects: it fails open on a crashed test run, its CI origin is self-asserted, and it binds the guest but not the host that enforces the boundary. A fourth major finding is a profile-conformance gap. It is pre-existing in the APP-004 bytes, does not come from this slice, and blocks Task A5's bound. Fix M1–M3 before any CI record counts as a release pass.

## Local runs (this review)

| Check | Result |
| --- | --- |
| APP-004 suite as `parser.yml` runs it in `spikes/APP-004`: `verify_vendor.py`, `make_fixtures.py` + `git diff --exit-code -- fixtures`, `cargo fmt --check`, guest build, both clippy `-D warnings`, `cargo test --locked -p parser-host -- --test-threads=1` | pass: 1 + 0 + 16 + 0 |
| The same sequence in `parser/` | pass: 1 + 0 + 7 (job) + 17 (qualification) + 0; `QUALIFIED_WASM sha256:7d114e60…` (local paths, not release bytes) |
| `cd app && CARGO_BUILD_JOBS=2 cargo +1.93.1 test --locked` (default parallel threads) | pass: lib 49 passed / 4 ignored, bin 1, integration 2; all 8 `supervise::` and 6 non-ignored `imports::sandbox::` tests pass |
| `python3 -m unittest discover -s scripts -p 'test_*.py'` | 11 OK |
| Old guest (`spikes/APP-004`) against new guest (`parser/`) through one host: 16 XLSX fixtures × 9 configurations (no sheet, each sheet, `platform`, lowered `sheets`/`output`/`text`/`entries`/`expanded`), CSV × 3, and 80 seeded mutations (bit flips, truncation, splices) × 3 | 387 cases. 371 are identical. In the other 16 the only difference is the added `available_sheets` on an old `sheet_selection_required` refusal |
| `.dockerignore` emulated with moby's pattern rules over the whole working tree | 151 files, all tracked. The `parser/` set equals `git ls-files parser` (102). The three tracked `app/src` files left out are `#[cfg(test)]` modules. Compared with the 148-file [context.json](context.json), the merged tree adds exactly APP-073's `0004_entitlement_reads.sql`, `catalog-v4.json` and `storage/admission.rs`, all listed. The allowlist is still exact |

## Findings

### Blocker

None.

### Major

**M1. A crashed hostile-suite run composes a passing release record (fail-open).**

- Where: `.github/workflows/parser-release.yml:68-72`, `scripts/check-parser-record.py:39-45`.
- Cause, workflow side: the step runs `cargo test … 2>&1 | tee "$EXTRACTED/hostile-suite.log"` under GitHub's default shell for an unspecified `shell:`, which is `bash -e {0}` with no `pipefail`. The step's status is therefore `tee`'s, and the job continues after `cargo test` fails.
- Cause, script side: `suite()` passes a log when every `test result:` line present says `ok`, at least one test passed, and the `QUALIFIED_WASM` line appears. It does not require a result line for each test binary.
- Reproduction: I took [hostile-suite-image-bytes.log](hostile-suite-image-bytes.log) up to the `qualified_guest_bytes_are_named_in_the_log … ok` line, then appended what cargo prints when `tests/qualification.rs` aborts in a later test: a stack overflow, `error: test failed, to rerun pass --test qualification`, and `process didn't exit successfully … (signal: 6, SIGABRT)`.
- Result: run with `GITHUB_ACTIONS=true`, `compose` printed `"passed": true` (exit 0) with `tests_passed: 8, result_lines: 3`, and `check` printed `"admitted": true`. Five hostile tests never ran.
- Exposure: the five tests that sort after the binding test run unrecorded (`resource_profiles_…`, `review_duplicate_…`, `review_zero_exponents_…`, `runtime_refuses_system_imports`, `selected_sheet_literals_…`). A native crash or an OOM kill on hostile input is exactly what the hostile suite exists to catch.
- Fix:
  1. Add `shell: bash` to the step, or as `defaults.run.shell: bash` on the `image-bytes` job. GitHub then runs `bash --noprofile --norc -eo pipefail`.
  2. In `suite()`, split the log on the `Running unittests src/lib.rs`, `Running unittests src/main.rs`, `Running tests/job.rs`, `Running tests/qualification.rs` and `Doc-tests parser_host` headers. Require exactly one `test result: ok.` line in each section, with at least 7 passed in `tests/job.rs` and at least 17 in `tests/qualification.rs`.
  3. Require the `QUALIFIED_WASM` line inside the `tests/qualification.rs` section.
  4. Refuse any log containing `error: test failed` or `process didn't exit successfully`.
  5. Add the crash-truncated log as a case in `test_compose_binds_suite_and_fixture_runs_to_the_bytes`.

**M2. The record's CI origin is self-asserted, so a local record can be a release pass.**

- Where: `scripts/check-parser-record.py:105-106, 35, 66-68`; `parser-release.yml:87`.
- Cause: `compose` sets `origin.kind = "github-actions"` from the `GITHUB_ACTIONS` environment variable, which anyone can set; `act` sets it too. `RUN_URL` accepts any `owner/repo`. `check` validates only the strings' shape.
- Reproduction: on this Mac, `GITHUB_ACTIONS=true … compose … --run-url https://github.com/someone-else/fork/actions/runs/42 --commit bbbb…` followed by `check` returned `"admitted": true`, exit 0.
- This refutes the README and docstring claim that "a record composed outside GitHub Actions … is never a release pass". The guarantee also falls short of SR8's "signed … an unsigned digest refused".
- Also: on `pull_request` events `$GITHUB_SHA` is the synthetic merge commit, so a record from a PR run names a commit that exists on no branch.
- Fix:
  1. Pin `RUN_URL` to `https://github\.com/getforma-dev/forma-app-platform/actions/runs/\d+/attempts/\d+`.
  2. Compose records only when `github.event_name == 'push'` (an `if:` on the compose step).
  3. In `check`, the release preflight step, verify the run through `gh api repos/getforma-dev/forma-app-platform/actions/runs/<id>`: `path == ".github/workflows/parser-release.yml"`, `event == "push"`, `conclusion == "success"`, `head_sha == origin.commit`, and the commit is reachable from `origin/main`. Then download that run's `app-017-parser-record` artifact and require byte equality with the committed record.
  4. Equivalent alternative that also gives SR8's signature: add `actions/attest-build-provenance` (SHA-pinned) on the record in the workflow, and in `check` run `gh attestation verify <record> --repo getforma-dev/forma-app-platform --signer-workflow getforma-dev/forma-app-platform/.github/workflows/parser-release.yml`.
  5. Until one of these lands, change the README/docstring wording to "self-declared origin".

**M3. The record binds only the guest, not the job-mode host that enforces the boundary.**

- Where: `scripts/check-parser-record.py:48-63, 71-96`.
- Cause: `check --wasm-sha256` admits any image whose guest digest has a record. The boundary that matters is in the native host: demo profile only, no `limits` channel, input cap before the guest, re-hash before compile (`parser/host/src/job.rs:36-43, 98-149`). A later edit to `job.rs` leaves the guest digest unchanged, so an image with a new, unqualified host passes on the old record.
- What exists: the CI job tests do run against the extracted host (`PARSER_HOST_BIN`), but nothing in the record names that host. The record copies the manifest, but `failures()` compares only `manifest.guest.sha256`.
- Architecture: CI builds amd64. An arm64 release image may produce the same guest digest while its arm64 host never ran the job tests.
- Fix:
  1. `tests/job.rs` prints `QUALIFIED_HOST sha256:<hex> path=…` for the binary it runs.
  2. `compose` requires that line to equal `manifest.host.sha256`, and records the host digest and the image platform.
  3. `check` takes `--manifest <parser-manifest.json extracted from the exact release image digest>` and requires the record's `manifest.guest.sha256`, `manifest.host.sha256` and `manifest.source.sources_sha256` to equal it. Alternatively, key the record file by the manifest's SHA-256.
  4. Amend plan Task A4's interface line, which names only `--wasm-sha256`.

**M4. Profile conformance (pre-existing in the APP-004 bytes, found here; owner D9.2 / Task A5): the fuel cap makes D9.2's cell and record bounds unreachable.**

- What: the 5,000,000,000 fuel cap binds long before the 320,000-cell, 10,000 × 32 and "64,000 records for the five-column events" bounds of D9.2/B5.
- Measured with the local release host and guest (fuel is a WASM instruction count, so host build and OS do not change it). Throughput is about 58,600 fuel per CSV cell and about 87,700 per XLSX cell:

| Input | Outcome | Fuel | Guest memory / wall time |
| --- | --- | --- | --- |
| CSV 1,000 × 32 one-character cells | parsed | 1.88e9 | — |
| CSV 2,000 × 32 | parsed | 3.75e9 | — |
| CSV 4,000 × 32 (128k cells) | `fuel_exhausted` | — | — |
| CSV 16,000 Northstar-shaped five-column records | parsed | 4.88e9 | — |
| CSV 32,000 and 64,000 Northstar-shaped records | `fuel_exhausted` | — | about 1.5 s |
| XLSX 10,000 × 5 | parsed | 4.38e9 | 155 MiB |
| XLSX 10,000 × 32 | `fuel_exhausted` | — | — |

- The APP-004 spike guest also refuses CSV 4,000 × 32 with `fuel_exhausted`, so the gap is not new to `parser/`.
- Practical ceiling: about 85,000 CSV cells or about 57,000 XLSX cells. Wall time stays near 1.5 s, so fuel binds, not the 120 s deadline.
- Effect: no sandbox defect. But Task A5's `csv_record_bound_is_cell_derived` (64,000 records for five columns) cannot pass on these bytes, and what invited members can import is about a quarter of what D9.2 advertises.
- Fix: before A5, record this measurement and decide one of:
  1. Amend D9.2/B5 to the measured bound, for example at most 50,000 aggregate cells, which is 10,000 five-column records.
  2. Cut the guest's per-cell cost and requalify. `push_cell` serializes every cell with `serde_json::to_vec` to meter output, then `parse_bytes` serializes the result twice; a counting writer and one final serialization would remove most of that.
  3. Raise fuel through an ADR amendment, since the plan's rule is that limits only ever reduce, and the design's wording is "120 s of fuel".

### Minor

**m1. The persistent parser target cache can ship a stale guest under a new sources digest.**

- Where: `app/Dockerfile:32`.
- Cause: `--mount=type=cache,id=genui-parser-target-${TARGETARCH},target=/source/parser/target` persists across builds and across worktrees on one builder. Cargo judges freshness of path packages by mtime. A source that changed but carries an older mtime than the cached output, as in a second, older worktree, reuses the old `parser_guest.wasm`. `write_manifest.py` would then bind those bytes to the current `sources_sha256`.
- Status: not observed. CI starts with an empty cache, so the risk is to local images and the manifest's claims about them.
- Fix: build the parser stage without a target cache (`CARGO_TARGET_DIR=/tmp/parser-target`), or run `cargo clean --release -p parser-guest -p parser-host` before building.

**m2. Two concurrent stages share the Cargo registry cache without a lock.**

- Where: `app/Dockerfile:20, 32`.
- Cause: the `native` and `parser` stages mount the same `genui-cargo-registry` cache with the default `sharing=shared` and run in parallel. Cargo's package-cache lock lives at `$CARGO_HOME/.package-cache`, outside the `registry/` mount. The two lockfiles share 29 crate versions.
- Effect: concurrent extraction can race and break a build.
- Fix: add `sharing=locked` to both mounts, or give the parser stage its own id (`genui-parser-cargo-registry`).

**m3. "Only its pipes" relies on CLOEXEC; nothing closes inherited descriptors.**

- Where: `app/src/supervise.rs:127-148`.
- Probe: the verbatim `run_confined` copy, with a parent fd opened through `libc::open` without `O_CLOEXEC`. The child listed `/dev/fd/0 1 2 10` (plus the shell's own directory handle): fd 10 leaked. Rust's own fds are CLOEXEC, so no leak is known today. D7.10 and D9.1 state the property, and `RLIMIT_NOFILE` 64 does not close descriptors numbered above 64.
- Fix: in `pre_exec`, before `setrlimit`, read the previous soft limit and run `fcntl(fd, F_SETFD, FD_CLOEXEC)` for `fd in 3..limit`, ignoring `EBADF`. This is async-signal-safe, and unlike `close_range` it leaves std's exec-error pipe working.
- Test: add `child_inherits_only_its_three_pipes`, listing `/proc/self/fd` on Linux and `/dev/fd` on macOS with a deliberately leaked fd.

**m4. A source that ends early without an error still yields `parsed`.**

- Where: `app/src/imports/sandbox.rs:252-293`.
- Cause: `Killed::InputFailed` covers only streams that return an error. A stream that returns `Ok(0)` early is complete as far as the child knows.
- Probe: a reader stopping at 8 of 12 bytes returned `Ok(parsed, rows=2)` with the truncated input's `input_sha256`.
- Fix: give `parse`/`parse_with` the expected `sha256` and byte length; Task A5 has `content_digest` and `byte_length` in the payload. Return `Failure("input_digest_mismatch")` unless `report.input_sha256 == "sha256:"+expected`. The `input_limit` refusal, whose digest is null, stays a refusal. Correct the README sentence "a truncated input can never complete".

**m5. The release check is not wired anywhere yet.**

- `infra/scripts/preflight.py` does not call `check-parser-record.py`. Every image built from this tree (`foundation.yml`, any PH01 deployment) carries parser bytes with no record.
- Acceptable while no route reaches `parse` (until A5 and APP-078). State it in the PH01 deploy notes, or add the call to the activation preflight now.

**m6. `foundation.yml` still has a 20-minute timeout (line 10) but now builds the parser stage** (Wasmtime and Cranelift release at one Cargo job) in parallel with the app. This is unobserved: no CI run exists. Watch the first run and raise the timeout if needed.

**m7. D9.2's 1.5 GiB binary line ("measured by APP-017 at the output cap") is not yet measured.**

- Probe: at 64,000 cells (13.1 MB JSON), the child peaked at 347 MiB. The probe process's max RSS went from 17 to 169 MiB while holding the stdout buffer and the decoded `serde_json::Value`, about 12 times the JSON for small cells.
- Carry to A5: measure at the output cap. Consider a typed decode instead of `Value` if the line is tight.

### Nits

- n1 `app/src/imports/sandbox.rs:369`: `std::env::set_var` runs in a test binary that runs tests in parallel, which is racy with C `getenv` elsewhere. Use a variable cargo already sets for test processes, such as `CARGO_PKG_NAME`, as the leak probe.
- n2 `sandbox.rs:253-255`: exit 1 collapses to `Failure("parser_failure")` and drops the child's closed reason. `wasm_digest_mismatch` would signal tampering. Map the closed set into the refusal or the job log.
- n3 `supervise.rs:75-85`: the bound is on length. For a 129 MiB stdout the `Vec` capacity reached 224 MiB in the probe. Acceptable; note it in the memory budget.
- n4 `app/src/main.rs:38-43`: verification runs only when `GENUI_PARSER_MANIFEST_FILE` is set. Once A5 routes imports, make its absence fatal in `serve`.
- n5 The plan's Task A4 names `evidence/APP-017/packaging/`; the evidence lives in `parser-packaging/`. Align one of them.
- n6 The shipped `parser-host` keeps the operator CLI form, which accepts `platform` and lower-only `limits`. It is reachable only by exec inside the container; record it as intended.

## Claims I tried and failed to refute

- **No job reaches the `limits` channel or a non-demo profile.** `JobConfig` is `deny_unknown_fields` and the guest config is rebuilt from scratch (`job.rs:36-43, 98-121`). Duplicate keys, a `l`-escaped `limits` key, `"limits":null`, `Profile` casing, trailing bytes and a zero-length frame all end as `config_invalid`. `"demo "` and `platform` end as `profile_not_demo`. A 4,101-byte config ends as `config_limit`. The parent frames only `{profile:"demo", format, sheet?}` (`sandbox.rs:199-209`).
- **Input beyond the cap never reaches the guest.** CSV of exactly 10 MiB reaches the guest (`row_limit`); 10 MiB + 1 is refused as `input_limit` before `Sandbox::new` (`job.rs:135-149`). Through `run_confined`, a 12 MiB CSV returns `Refused{input_limit}`, not `InputFailed`. The parent's write to the closed pipe is classed `ChildClosed`.
- **No time-of-check/time-of-use gap between hash and compile.** The guest is read once (≤16 MiB), hashed and compiled from the same buffer (`job.rs:84-96, 133, 150`). The report's digest is checked again by the host (`job.rs:154`) and by the parent against the startup identity (`sandbox.rs:264-266`).
- **The startup manifest binds what runs.** The guest is the startup-verified digest on every job. The host is exec'd by the startup-verified absolute path, is root-owned and is unwritable by uid 65532. Re-hashing the host per job is not required by "integrity, not allowlist".
- **The path remaps remove build-host paths.** The locally built guest holds absolute paths of exactly three kinds: `CARGO_HOME`, the checkout, and `/rustc/<commit>` (no rustup paths). The image's two remaps cover the first two. Caveat: a `RUSTFLAGS` environment variable would replace the config `rustflags`; none is set in the stage.
- **The output bounds hold.** stdout of exactly 129 MiB returns `Ok`; 129 MiB + 1 returns `ResourceLimit`. stderr over 1 MiB returns `ResourceLimit` (unit test). The child's memory is bounded by the guest's 2 GiB `StoreLimits` plus the input and output caps, with the 100 ms RSS watch as backstop. A 1 GiB burst under a 64 MiB line was killed.
- **The environment is cleared and limits are only NOFILE 64 and CORE 0** (unit tests).
- **A killed or truncated child never yields a parsed result.** Every kill path returns `Killed` before output is used. A signal yields `Crashed`, exit 1 yields `Failure`, partial or trailing stdout fails JSON decoding, and an exit/status mismatch yields `Failure`. The one gap is on the input side (m4).
- **The guest change alters no cap or refusal.** The differential above holds. In code, the no-sheet path runs the same `preflight` before listing (`guest/src/lib.rs:693-700`). The list is at most 16 names of at most 8 KiB each, and the old path ran no output check on this refusal either.
- **`.dockerignore` is still an exact allowlist** (above).
- **The actions are SHA-pinned** (`checkout@11bd719…` v4.2.2, `setup-python@a26af69…` v5.6.0, `upload-artifact@ea165f8…` v4.6.2, the same pins as `parser.yml` and `foundation.yml`). `--locked` builds and base images pinned by digest hold.
- **Promotion is verbatim and the spike is untouched.** `git diff 623fcd5:spikes/APP-004 583c112:parser` removes only `README.md` and `collect_metadata.py`. `spikes/APP-004` is tree `a28dd918…` at HEAD.

## The implementer's open questions

1. **No CI run, record or independent review.** Agreed. This is the review. A CI record counts only after M1–M3.
2. **The image-bytes hostile suite used a macOS test host.** Acceptable for local evidence, because D9.1 qualifies guest bytes. But the extracted Linux host is exercised only by the 7 job tests, and nothing binds it (M3).
3. **The remap covers only the image's fixed paths.** Agreed and verified (claims above). Add m1, which the remap cannot cover.
4. **Interface additions** (`Killed::InputFailed`, the richer `ParseRefusal`, the stderr line without `result`, `--wasm/--wasm-sha256`). All accepted as improvements over the plan text. `InputFailed` needs m4 to deliver what the README says it does.
5. **The guest compiles per job (~0.7 s).** Agreed. Keep it. A precompiled `.cwasm` is a second artifact identity, and `Module::deserialize` is `unsafe`. Revisit only if A5's measurement demands it.
6. **Cleanup.** Fine. The target cache it mentions is also m1's hazard.
7. **`check-foundation.py` and the PostgreSQL campaign were not run.** Acceptable for PostgreSQL: no storage code changed in this slice. `cargo test` on the merged tree passes here. The image-build timing inside `foundation.yml` is unobserved (m6).
8. **Whether the amd64 CI build yields the same guest bytes as the local arm64 build.** Unknown. It does not matter for correctness, because records are per digest. It matters for release: the release image's architecture must be the one CI qualified, including the host (M3). Build `--platform` explicitly in `parser-release.yml` for the architecture the task runs, and record it.
