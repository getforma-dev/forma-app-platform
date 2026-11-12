# APP-125 independent review

Reviewer: Claude subagent, independent of the implementer. Date: 2026-11-12 UTC.

## Source reviewed

- Implementation `3679264` (2026-10-31, ADR-012 step 1).
- The ADR-013 D13.3 move `76e5289` (APP-130, 2026-10-31). It moved the dev profile from `app/Cargo.toml` to the root `Cargo.toml`, moved `jobs = 3` from `app/.cargo/config.toml` to `.cargo/config.toml`, and moved `Cargo.lock` to the root.
- The later `scripts/dev/setup.sh` change in `1b10ab1`, which adds the DuckDB lock to `contracts/.venv`.
- All of the above as they stand on `main` `5695869`. I checked that `3679264` is an ancestor of `5695869`.
- Worktree: `/Users/victorvillacis/dev/FormaBuiltApps/forma-board`, branch `tickets/done-reviews-2026-11-12`.
- The acceptance record is PR 80. Its head was `d8084e4`. It merged on 2026-11-05 as `8c4343b`.

There was no earlier independent review.

## Method

I read these documents: AGENTS.md, the Tickets/README.md status rules, Tickets/_tools/REVIEW.md, docs/DEVELOPMENT.md, `Tickets/4-done/APP-125.md`, `evidence/APP-125/README.md`, ADR-012 D12.4 and step 1, and ADR-013 D13.3 and D13.9 step C.

I read these source files on `5695869`: `rust-toolchain.toml`, `Cargo.toml`, `.cargo/config.toml`, the pin files, `tools.versions`, `scripts/dev/setup.sh`, `scripts/dev/doctor.sh`, `scripts/dev/check.sh`, `.github/workflows/*.yml` and the ignore files.

I ran every check below myself. A dev instance from this worktree was running throughout: process-compose on port 5630 and `target/debug/genui-dev serve`, pid 8000. I did not touch it or its `target/`.

In this environment the shell's `grep` is a wrapper function, so I recorded grep results with `/usr/bin/grep`.

## Acceptance mapping

| Box | Implementer evidence | Reviewer's own result |
| --- | --- | --- |
| 1. `cargo build` in `app/` uses 1.93.1 without `+` | `rust-toolchain.toml:1-5`. Ticket log, line 55. Evidence README, lines 9-17. | `(cd app && rustup show active-toolchain)` printed `1.93.1-aarch64-apple-darwin (overridden by '/Users/victorvillacis/dev/FormaBuiltApps/forma-board/rust-toolchain.toml')`.<br><br>The rustup default is `stable`. On this machine `stable` is also rustc 1.93.1, so `rustc --version` alone does not show which toolchain was chosen. Two other observations do. First, `parser/` resolves its own `parser/rust-toolchain.toml`. Second, in the verbose build log (box 2), cargo invoked `/Users/victorvillacis/.rustup/toolchains/1.93.1-aarch64-apple-darwin/bin/rustc` with no `+` on the command.<br><br>CI: `cargo +1.93.1` appears nowhere in `.github/workflows/`. Only `cargo +1.85.0 check` remains, at `tenant-foundation.yml:51`, as the ticket intends. |
| 2. Target size after a clean `cargo build --tests` with the dev profile is under one third of 33 GB (11 GB) | Evidence README, lines 17-28: `CARGO_TARGET_DIR=$PWD/target` set from `app/` gave 1.6G, `debug/deps` 667M, 752 `.o`. | **The current root `target/`** is used by the running instance and was measured only: 998M, `debug/deps` 562M, 0 `.o` in `deps`.<br><br>**Clean build with the current profile.** From `app/`, `CARGO_TARGET_DIR=<scratch>/target-new cargo build --locked --tests -v` finished the `dev` profile in 2m 58s. The workspace members (`forma_app_platform`, `genui`, `genui_auth_verifier`) were compiled with `-C debuginfo=line-tables-only -C split-debuginfo=packed`. Dependencies (for example `tokio`) got no `-C debuginfo` flag. `du -sh` gave 1.7G (1,730,960 KiB), `deps` 670M and `incremental` 998M. There were 752 `.o` files: 0 in `deps`, 728 in `incremental`, 24 in `build`.<br><br>**Control build with the pre-APP-125 profile.** Same command into a separate scratch directory, with `--config 'profile.dev.debug=true' --config 'profile.dev.split-debuginfo="unpacked"' --config 'profile.dev.package."*".debug=true'`. Every crate was compiled with `-C debuginfo=2 -C split-debuginfo=unpacked`. It took 2m 42s. `du -sh` gave 2.3G (2,460,404 KiB) and `deps` 1.6G. There were 2,144 `.o` files, 1,357 of them in `deps`.<br><br>`jobs`: `cargo +nightly-2026-10-01 -Zunstable-options config get build.jobs --show-origin` from `app/` printed `build.jobs = 3 # …/forma-board/.cargo/config.toml`. Nightly was used only for this read-only query.<br><br>Both scratch targets were deleted afterwards. |
| 3. `contracts/.venv` is not tracked, and Contract qualification passes against the venv `setup.sh` made | Evidence README, lines 32-53. | `git ls-files -s contracts/.venv` printed nothing. The index holds no symlink at all: `git ls-files -s \| awk '$1=="120000"'` was empty. `contracts/.venv` is a directory, ignored by `contracts/.gitignore:1`.<br><br>`export PWD=$(pwd); just check contracts` exited 0 in 5.9s with these results:<br>- 40 tests OK.<br>- "6 edition-one and 13 edition-two canonical byte/digest vectors match".<br>- "Contract edition impl-2026-11-01.2 lock matches 61 artifacts, 13 artifacts of 3 retained editions and both frozen source identities".<br>- `{"files": 40, "apps": 20}`.<br><br>The venv was built from the same two hashed locks that `contracts.yml` installs (`setup.sh:65`). `uv pip freeze` lists 7 packages: attrs 26.1.0, duckdb 2.0.0, jsonschema 4.26.0, jsonschema-specifications 2025.9.1, referencing 0.37.0, rpds-py 2026.9.1, typing-extensions 4.16.0.<br><br>CI: Contract qualification push run `43144270041` on `5695869` succeeded. All other workflows on that commit succeeded too. Promote to beta was skipped. |
| 4. `setup.sh` is idempotent and leaves no path into another checkout | Evidence README, lines 55-65. | **Worktree.** Both Node stamps matched their `package-lock.json`, so `npm ci` could not run under the live admin watcher. `export PWD=$(pwd); just setup` was run twice. Both runs exited 0 (1.60s, then 1.63s), with both Node directories "up to date" and the same four venvs. An mtime snapshot of 21,910 entries (four venvs and two `node_modules`, excluding `.pyc` and `__pycache__`) was identical before run 1, after run 1 and after run 2. `uv pip freeze` was unchanged for all four venvs. `just doctor` printed `doctor: ok`.<br><br>**Fresh copy.** This is a negative control. I extracted `git archive 5695869` into a scratch directory and planted `contracts/.venv` as a symlink to a decoy directory holding a `SENTINEL` file. Run 1 exited 0 in 5.04s. It warned and replaced the link, and the decoy and its `SENTINEL` were left intact. It created four 3.11.14 venvs and installed 28 packages in `app/admin` and 15 in `infra`. Run 2 took 1.45s and reported everything "up to date". Its only filesystem change was that Python created `site-packages/__pycache__/_virtualenv.cpython-311.pyc` the first time the interpreter started (`setup.sh:51`). Package contents did not change.<br><br>**No path into another checkout.** All four `pyvenv.cfg` files have `home = ~/.local/share/uv/python/cpython-3.11-macos-aarch64-none/bin`, which is a link to `cpython-3.11.14-…`. Searching for `/forma-app-platform/` or `forma-pitch-mvp` with `/usr/bin/grep -rIl` across the four venvs (`pyvenv.cfg` and all other files) found nothing (exit 1). All 27 absolute `FormaBuiltApps/…` references in the venvs name `forma-board`, and `node_modules` has none. The briefed `find . -maxdepth 3 -type l … \| grep -v "^\.\|^[^/]"` printed nothing. A deeper symlink scan (excluding `target`, `.git`, `node_modules` and `.cache`) found only the 12 venv `bin/python*` links, all resolving to uv's CPython 3.11.14. The fresh copy gave the same results. |

**Is the evidence's 1.6G claim sound?** Yes. The method matches the box. It used a fresh target directory, a clean `--tests` build, and ran from `app/` without `+`. Pointing `CARGO_TARGET_DIR` at `app/target` does not change the profile, because Cargo reads `[profile.*]` only from the root manifest (`Cargo.toml:19-24`; ADR-013 line 77), and my log shows those flags applied. My 1.7G at `5695869` matches the evidence's 1.6G at `a9a7305`, given the code added since.

**Ticket structure.** The ticket has all the sections `board.py` requires: Why (line 24), What changes (line 28), Acceptance tests (line 36), Verification (line 43), Migration and rollback (line 47) and Log (line 51). `python3 Tickets/_tools/board.py check` printed OK: 175 tickets, 9 phases, 15 sprints, 80 requirements.

**Does the log overclaim?** No. The 2026-10-31 entry (ticket line 54) discloses that `lock.py check` failed and why, and calls the size bound trivially met. The 2026-11-05 entry (line 55) matches what I measured.

## Findings

### Blocking

None.

### Non-blocking

1. **The size bound cannot tell the two profiles apart** (ticket line 39; ADR-012 line 138; evidence README line 28). A clean build with the old profile is 2.3G, which also passes the 11 GB bound. The 33 GB came from object files piling up in `debug/deps` across 41 build hashes (ADR-012 line 17), and a single clean build does not reproduce that. The meaningful evidence is the like-for-like comparison: about 30% smaller in total, `debug/deps` from 1.6G to 670M, and `.o` files retained in `deps` per build hash from 1,357 to 0. The 2026-10-31 log says the bound "holds trivially", but the 2026-11-05 evidence README does not. The Done line should cite the comparison.
2. **Stale path in the ticket text** (What changes, line 31; box 2, line 39). These still name `app/Cargo.toml`, `app/.cargo/config.toml` and `app/target`. On `main` the profile is at `Cargo.toml:19-24`, `jobs` is at `.cargo/config.toml:5-6`, and the default target is the root `target/` (DEVELOPMENT.md lines 13 and 171). Evidence README line 5 explains this, and the measurement is equivalent. This is a wording issue, not a defect.
3. **A `.venv` symlink would not be ignored by Git** (`.gitignore:15` and `contracts/.gitignore:1` use the directory-only pattern `.venv/`). I confirmed in a scratch repository that a symlinked `contracts/.venv` shows as untracked, so `git add -A` could commit one again. Nothing in `just check` refuses a tracked symlink. Two things would still catch it: `doctor.sh:52` locally, and the `python3 -m venv contracts/.venv` step in `contracts.yml` in CI, which would fail the way it originally did. A possible fix is the pattern `.venv` without the slash, but that is outside this ticket's scope.
4. **The venvs reach Python through uv's minor-version link.** Each venv's `bin/python` and `pyvenv.cfg` point at `cpython-3.11-macos-aarch64-none`, which currently resolves to 3.11.14. If uv later installs another 3.11 patch release, that link moves. `setup.sh:51-54` and `doctor.sh:54` both detect a version mismatch, so this is safe today. Evidence README line 65 says the links point at CPython 3.11.14: true after the link is resolved, but imprecise.
5. **The object-file counts understate the fix.** Evidence README line 28 ("752") and the ticket log's "61% fewer object files left per hash" count objects in `incremental/` and `build/`. With the new profile, `debug/deps` keeps no objects at all.
6. **The log has no landing line yet.** PR 80 merged on 2026-11-05 as `8c4343b`, and `5695869` CI is green. Whoever records Done should add both facts to the log.

## Limits

- I ran `just setup` in the worktree only on a checkout that was already set up. I tested a first install only in a `git archive` copy, which is not a Git worktree.
- CI never runs `setup.sh`. `contracts.yml` builds its own venv with `python -m venv` and `pip`, so only the local check exercises the venv `setup.sh` makes.
- The size builds ran beside the live dev instance, so their timings are not comparable with the evidence.
- During this review another agent left uncommitted edits in this worktree (APP-169 ticket and evidence files, and an APP-082 review file). I did not touch them, and they do not affect APP-125's files.
- I did not run `just check` in full or any campaign. This ticket changes no product behavior.

Verdict: no blocking finding — Done may be recorded.
