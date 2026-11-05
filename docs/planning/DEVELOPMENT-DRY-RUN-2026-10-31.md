> **Historical (pitch era, kept as a record).** Current guidance: [DEVELOPMENT.md](../DEVELOPMENT.md).

# DEVELOPMENT.md dry-run

Checked the local setup instructions in a worktree, not in the main checkout. No deploy, image push, snapshot, migrate, seed, or reset was run for this check. The delivery table was read and not executed.

## Environment

| Item | Value |
| --- | --- |
| Commit of the instructions | `af63feba41aa825a972dde354e0a5d2195f7d608` |
| Checkout | `/Users/victorvillacis/dev/FormaBuiltApps/forma-pitch-rehearsal` |
| Port base | `7500`, written by `scripts/dev/env.sh` into that checkout's gitignored `.env.dev` |
| Left alone | Listeners on 127.0.0.1:4001–4003 and :4010 |

## What was run

| Command | Result |
| --- | --- |
| `just setup` in the worktree | Rust 1.93.1, Python 3.11.14, four lockfile venvs, `npm ci` for `app/admin` and `infra`. The first attempt had been typed in the main checkout, which reported its own `node_modules` up to date and did not install the worktree. |
| `just doctor` | ok. rustc 1.93.1, node 22.21.0, npm 11.6.2, python 3.11.14, uv 0.12.22, just 1.59.0, process-compose 1.124.0, Docker 29.8.2. All four venvs ok. |
| `just dev -d` in the main checkout while 4001 was already listening | Printed `already running` and the main checkout's URLs. Did not reset that instance. |
| `just dev -d` in the worktree before `just setup` | Admin log: `app/admin/node_modules is missing: run 'just setup'`. The supervisor restarted that process. Pitch was stopped, setup was run, then `just dev -d` reached ready on port base 7500 (Diego `:7501`, Sarah `:7502`, Priya `:7503`). |
| `just parser` | Wrote `.cache/parser-target/` (`parser-host` and `parser_guest.wasm`) before the server that later parsed a CSV. |
| `just status` / `just status clean` | Pitch stopped, its Docker volume kept. Clean running: Alex `http://127.0.0.1:7510/canvas`, database port 7521 and supervisor 7531 follow the `+21` / `+31` rule. |
| `cargo metadata --no-deps` | Workspace root is the worktree. Members: `forma-app-platform`, `genui-auth-verifier`. |
| `just check --list` | Prints the named steps in DEVELOPMENT.md, including `fmt`, `clippy`, `clippy-dev`, `admin`, `test`, `contracts`, `board`. The full `just check` was not run. |
| `npm --prefix app/admin run typecheck` | `tsc --noEmit`, exit 0. |
| `npm --prefix app/admin run build` | `tsx build.ts --ssr --strict`, exit 0. Canvas JavaScript 157,659 of 373,760 B. This rewrites `app/admin/dist`, which a running dev server watches. |
| `just dev stop` and `just dev clean stop` | Each printed that the profile stopped and the data volume was kept. Used on this worktree only. |

Logs are `.cache/dev/<pitch|clean>/logs/` (`server.log`, `admin.log`, `db.log`, `process-compose.log`). The clean server log recorded `genui-dev: model Anthropic (ANTHROPIC_API_KEY from the clone's .env)` and did not print the key.

## Corrections made from this run

- A worktree base is `4100` plus a multiple of 100, not the next integer above 4100. This checkout recorded `7500`.
- `just dev -d` on a profile that is already up does not reset it.
- `just setup` has to be run in the checkout being started. Another checkout's `node_modules` does not count.
- The dev server reads the sealed parser at startup. Build it first, or restart after `just parser`.

## Not executed

`just images`, every `just deploy` recipe, snapshot, migrate, seed, grants, activate, rollback, and `just dev reset`. Those sections were read. They still require the runbook and Victor's per-action approval. This dry-run does not show that they succeed.
