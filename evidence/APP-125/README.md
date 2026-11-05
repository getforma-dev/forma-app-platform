# APP-125 acceptance

Measured 2026-11-05 UTC on `chore/app-125-acceptance` at `a9a7305`, in a worktree of `origin/main`. The pins, the dev profile, `scripts/dev/setup.sh` and the removal of the committed `contracts/.venv` symlink are already on `main`. ADR-013 moved the dev profile and `jobs = 3` to the root `Cargo.toml` and `.cargo/config.toml`. This record does not change product code.

The dev listener on `127.0.0.1:4001` was left running. `CARGO_TARGET_DIR` pointed the clean build at this worktree's `app/target`, so the other checkout's `target/` was not deleted. The default workspace target is the checkout root `target/`.

## Toolchain

```text
(cd app && rustup show active-toolchain)
1.93.1-aarch64-apple-darwin (overridden by '<worktree>/rust-toolchain.toml')

rustc --version
rustc 1.93.1 (01f6ddf75 2026-02-11)
```

`CARGO_TARGET_DIR=$PWD/target cargo build --locked --tests`, run from `app/` with no `+` on the command, finished the dev profile in 1m 46s (`real 106.39`).

## Target size

```text
du -sh app/target app/target/debug app/target/debug/deps
1.6G  app/target
1.6G  app/target/debug
667M  app/target/debug/deps
```

Object files: 752. The inventory figure is 33 GB. One third of that is 11 GB. 1.6G is under it.

`npm --prefix app/admin run build` ran first because the test build embeds `app/admin/dist`. It finished in 4.31s. It warned that `/canvas` is 243.8KB against a 195.3125KB threshold. That warning is outside this ticket.

## Contract qualification

`git ls-files -s contracts/.venv` printed nothing. The venv is the one `scripts/dev/setup.sh` created in this worktree.

`just check contracts` (5s, exit 0):

```text
contracts/.venv/bin/python -m unittest discover -s contracts -p 'test_*.py'
Ran 40 tests in 2.750s
OK

node contracts/verify_vectors.mjs
6 edition-one and 13 edition-two canonical byte/digest vectors match independently in JavaScript

contracts/.venv/bin/python contracts/lock.py check
Contract edition impl-2026-11-01.2 lock matches 61 artifacts, 13 artifacts of 3 retained editions and both frozen source identities

contracts/.venv/bin/python contracts/make_library.py --check
{"files": 40, "apps": 20}
```

The 2026-10-31 log's `lock.py check` failure was the stale edition. The current edition matches.

## setup.sh

`scripts/dev/setup.sh` twice.

First run, `real 4.97`: four venvs at Python 3.11.14 from the hashed locks (`app/runtime/.venv`, `contracts/.venv`, `app/operator/.venv`, `spikes/APP-005/.venv`). `npm ci` installed `app/admin` (28 packages) and `infra` (15 packages). Docker was reachable.

Second run, `real 1.13`: the same four venvs, and both Node locks reported up to date.

`just doctor` printed `doctor: ok`. rustc 1.93.1, node 22.21.0, npm 11.6.2, python 3.11.14, uv 0.12.22, just 1.59.0, process-compose 1.124.0, docker 29.8.2. The toolchain row says the `app/` compiler comes from `rust-toolchain.toml`.

The four `.venv` paths are directories, not symlinks. The only links that leave the checkout are `bin/python`, `bin/python3` and `bin/python3.11`, and they point at the uv-managed CPython 3.11.14 under `~/.local/share/uv/python/`. They do not point at another checkout.
