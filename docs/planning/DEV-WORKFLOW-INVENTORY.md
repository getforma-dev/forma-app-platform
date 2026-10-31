# Developer workflow: inventory, root causes, options

Research only, 2026-10-31, at `pivot/pitch-mvp` `e7dda41`. Nothing was changed, started or deployed while it was written. Every claim cites a file and line or a command that was run. Secrets and env values were not read into this report: the env files are described by their variable names only.

Victor's request, verbatim: "do you have a developer workflow environment setup? why did I have to ask you to set up localhost? … heavy cleaning and separation of concerns and best practices … a workspace or a concurrently-like library for rust? … rust libraries to help development, like python venv … figure out the deployment process … document decisions and procedurals".

Short answer: no, until 2026-10-30 22:54 there was no way to run the product locally. The dev stand that exists now is a test harness used as a server, held together by shell. Building, Python environments, toolchains and the deploy scripts are each set up per checkout or per session. Some of them live outside Git, in another clone's `.cache` or in a Claude scratchpad.

---

## A. Local run (the "dev stand")

### A.1 What it is

| Part | Where | What it does |
| --- | --- | --- |
| Launcher | `app/scripts/dev_stand.sh` (281 lines) | Runs `npm run build` (l.62-73) and `cargo test --lib --no-run`, then picks the test executable out of cargo's JSON (l.76-82) and copies it to `$state/dev-stand-bin` (l.88-91). It runs that binary as `storage::dev_stand::native_dev_stand --exact --ignored` (l.42, l.127), waits up to 600 s for `stand.json` (l.133-146), then starts the proxies (l.152-173). |
| Second profile | `app/scripts/fresh_stand.sh` (15 lines) | `GENUI_DEV_STAND_PROFILE=fresh`, then `exec dev_stand.sh` (l.14-15). |
| The "server" | `app/src/storage/dev_stand.rs` (711 lines) | An `#[ignore]` **test** (l.145-147) that builds a tokio runtime on a thread with a 16 MB stack (l.148-161). It composes `product::routes` with the pages (l.6-13, l.245-267), seeds data and serves until `GENUI_DEV_STAND_SECONDS` (8 h) passes or a `.stop` file appears (l.274-279, l.681). |
| Database | `storage/qualification.rs` `Fixture::new` (l.146 ff.) | `docker run --detach --rm` of a digest-pinned `postgres` image (l.24-25, l.232-247) on tmpfs. The data is gone when the container goes. Migrations and seed run through Python (l.330-349). |
| Credentials | `qualification.rs:353, 378-383`; `config.rs:117-118` | A credentials file that may live at most 600 s. The stand rewrites it every 60 s (`dev_stand.rs:687-689`, `KEEPALIVE` l.99). |
| Sign-in | `app/scripts/dev_stand_proxy.py` (229 lines), one Python process per persona | A loopback proxy adds the persona's `__Host-` session cookie, which a browser will not keep over http (l.4-8). It hands each browser one of 8 sessions (l.35-38, l.100-112; `BROWSER_SLOTS` `dev_stand.rs:97`). |
| Sealed parser | `dev_stand.rs:93`, `box_campaign.rs:66` | `DEFAULT_PARSER` is hard-coded to `/private/tmp/claude-501/…/scratchpad/parser-target` (507 MB, a Claude session scratchpad). |
| Model key | `dev_stand.sh:16-18, 103-118`, `app/scripts/read_env.py` | `ANTHROPIC_API_KEY` from the clone's `.env`, or from the main clone's `.env` in a worktree (l.94-101). |
| Python | `qualification.rs:75-81`; `dev_stand.sh:121-125`; `qualification.rs:337-343` | Needs `app/runtime/.venv` (found through `CARGO_MANIFEST_DIR`). The `contracts/.venv` site-packages directory is added through `PYTHONPATH`: two environments joined by an environment variable. |
| State | `dev_stand.sh:31, 35` | `$TMPDIR/forma-dev-stand` and `$TMPDIR/forma-fresh-stand`. These are per machine, **not per checkout**. |
| Ports | `dev_stand.sh:32, 36`; `.claude/launch.json` | Hard-coded: 4001-4003 and 4010. `launch.json` holds three attach-only entries (`url` and no command), so opening them starts nothing. |
| Docs | `docs/pitch/GUIDE.md:133-171` | The start/stop/restart-server/status/reset table. |

**Moving parts while it runs** (`ps`, 10:21 today): 2 stand processes, 4 proxy processes and 2 `genui-native-*` Postgres containers, plus their `$TMPDIR` state, logs, pid files and a 0600 stand file. That makes 8 processes and containers for 2 stands. Nothing supervises them: bash pid files and `kill -0` checks (`dev_stand.sh:47`) are the only process management.

### A.2 Test harness used as the dev environment

- The stand imports the qualification harness: `Fixture`, `operator`, `send`, `vault_token`, the fixed `qualification_cases` principals, `query::tests::{real_engine, repository, Directory}` (`dev_stand.rs:37-63`).
- It is a test binary, so `cargo test --lib --no-run` compiles all 376 tests and 34 ignored campaigns just to get a server (`grep` counts in `app/src`).
- The production binary cannot run locally. `main.rs` requires `GENUI_RUNTIME_CONFIG_FILE`, `GENUI_RDS_CA_FILE`, `GENUI_IDENTITY_REGISTRY_FILE`, `GENUI_HELPER_PATH` and others (`main.rs:75, 81, 86, 285`), and refuses anything else (`main.rs:32, 77`). `app/.env.example` says it is "intentionally incomplete and refused". There is no development configuration path in the binary, so the only way to get a configured database, roles and credentials is the harness.

### A.3 What breaks (observed today)

- **The pitch proxies run old code.** PIDs 35360-35362 have been up for 9 h 45 m, which puts their start before `e7dda41` (10:18, "each browser gets its own session"). `start` and `restart-server` skip a proxy that is alive (`dev_stand.sh:156`), so a proxy code change takes effect only after `stop`.
- **The two stands share one machine-wide state directory and fixed ports.** A second checkout cannot run a stand. APP-119 says so in its log: "`dev_stand.sh` hard-codes the proxy ports 4001–4003, which Victor's running stand uses, so a second stand could not run beside it" (`Tickets/0-backlog/APP-119.md:100`).
- **Every server change re-seeds the database** (`GUIDE.md:141`), because the database lives in tmpfs inside a container the stand owns.

---

## B. Build and test

### B.1 Toolchains

| Tool | Pin | Evidence |
| --- | --- | --- |
| Rust | **No `rust-toolchain.toml`** in the repo (`ls` finds none). CI installs 1.93.1 in each workflow (`canvas.yml:21-22`, `tenant-foundation.yml:24, 44-52`, `parser*.yml`). The runbook uses `cargo +1.93.1` (`DEPLOY-RUNBOOK.md:58`). `dev_stand.sh:76` runs plain `cargo`. | Locally `stable` currently resolves to 1.93.1 (`rustc --version`), and 1.97.1, 1.85 and 1.71 are also installed (`rustup toolchain list`). The next `rustup update` moves the stand off CI's compiler without any sign. The MSRV floor is `rust-version = "1.85"` (`app/Cargo.toml:8`), and CI also checks `cargo +1.85.0 check` (`tenant-foundation.yml:47`). |
| Cargo layout | `app/` is a single package, not a workspace. `parser/` is its own workspace (`parser/Cargo.toml:1`). Five spike crates have their own `target/` directories. | `find -name Cargo.toml` |
| Cargo profile | No `[profile]` and no `.cargo/config.toml` in `app/`. | `app/Cargo.toml` |
| Node | No `.node-version` in this repo. `engines: node >=22` (`app/admin/package.json`); Docker uses `node:22.21.0` (`app/Dockerfile:1`); local is v22.21.0. The Forma framework and getforma-site pin Node with `.node-version` (24.22.0). | |
| Python | No `.python-version` file. Local is pyenv 3.11.0, the image is `python:3.11.14` (`app/Dockerfile:52`). | |

### B.2 Python environments (4 hand-made venvs, 3 hand-maintained hashed locks)

| Venv | Lock | How it is created | Where |
| --- | --- | --- | --- |
| `app/runtime/.venv` | `app/runtime/requirements.lock` (11 pins) | Only in CI YAML: `python3 -m venv` plus `pip install --require-hashes --only-binary=:all:` (`tenant-foundation.yml:26-27`) | per checkout, gitignored (`.gitignore` `.venv/`) |
| `contracts/.venv` | `contracts/requirements.lock` (6) | `contracts/README.md:21-22`, `contracts.yml:20-21` | per checkout |
| `spikes/APP-005/.venv` | union of 3 locks | `fixtures.yml:21-22`, `query.yml:18-19` | per checkout |
| `opvenv` (operator) | `app/operator/requirements.lock` (16; "exactly the union" of the other three, l.2-4) | in the APP-078 runbook only (`evidence/APP-078/prep/RUNBOOK.md:22`) | **`forma-app-platform/.cache/operations/beta/app-078-window/opvenv`, inside the other clone** |

No script creates the venvs, and `uv` (installed at `/opt/homebrew/bin/uv`) is not used. The worktree `forma-pitch-mvp-talk` has `contracts/.venv` but **no `app/runtime/.venv`** (checked today). An agent wrote `scratchpad/op-python`, which runs the *forma-app-platform* runtime venv with *forma-app-platform*'s contracts site-packages on `PYTHONPATH`.

### B.3 Front end

`app/admin`: `npm run build` (`tsx build.ts --ssr --strict`). A watch script exists, `npm run dev` (`--watch`, `build.ts:72`), and nothing uses it. The server embeds `admin/dist` through `rust_embed` (`dev_stand.rs:80-82`) and loads `manifest.json` and the FMIR modules **once at startup** (`dev_stand.rs:354-359`; `forma-server-3.2.0/src/assets.rs:57-69` "Call it at start"). The Forma skill says: "Build frontend assets before compiling a server that embeds them. Rebuild … when those embedded assets change" (`FormaStackDev/.agents/skills/forma-stack/references/building-and-verification.md:36-37`). `GUIDE.md:144` says a front-end change needs only `npm run build` and a page reload.

### B.4 Campaigns and qualifiers

- There are 34 `#[ignore]` tests: campaigns, real-model smokes and the stand. CI runs 10 of them (`grep` of `.github/workflows`).
- **Not in CI:** `native_{actions,box,canvas,composer,mcp,pitch,sharing,slack,workflows}_campaign`, which covers every campaign of the pitch era.
- Each campaign runs as `cargo test --locked --lib <module::path::name> -- --ignored --exact --nocapture`, often with an `GENUI_*_REPORT` path (`tenant-foundation.yml:52-67`).
- Nothing lists them except the CI YAML and ticket logs.

### B.5 How many commands a full check takes

The pre-deploy check list (`DEPLOY-RUNBOOK.md:57-65`) has **8 command lines** across 4 toolchains (cargo fmt/clippy/test; infra npm; infra unittest; scripts unittest; admin typecheck/build; schema pins; `qualify_tenant_schema.py`; board check). On top of that come the campaigns not in CI (about 9, each its own command, each starting Docker) and the image qualification (B.6). No single entry point runs them. The Forma framework does have one: `npm run check` → `scripts/check.sh` (`Forma/work/FORMA-246/forma/CONTRIBUTING.md:18-24`).

### B.6 CI

- 11 GitHub Actions workflows run on push and pull request (`.github/workflows/*.yml`).
- The repo is "temporarily public only so that GitHub Actions can run" (`README.md:3`).
- Branch health, from `gh run list --branch pivot/pitch-mvp --limit 60`:
  - **Contract qualification: 6 of 6 failed.**
  - **Foundation preparation: 4 of 4 failed.**
  - **Tenant foundation: 4 of 4 failed.**
  - Everything else passes.
- CI is red on the pitch branch, and nothing blocks on it.

### B.7 Disk

- `app/target` in the main clone is **33 GB** now, after `clean-space.sh` removed **43.1 GB** from it at 08:37 today (`Forma/work/_maintenance/logs/space-20261031-083714.log`, aggregated). It grew back in about 2 hours.
- 32 GB of it is `debug/deps`. That holds 48,650 `genui-*.rcgu.o` object files from **41 different build hashes** (apparent size 48.1 GB).
- Cause: on macOS, Cargo's dev profile defaults to `split-debuginfo = "unpacked"`, which keeps debug info in the per-unit `.o` files, with `codegen-units = 256` ([Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)). Every new hash of the test binary (stand, campaign, `--features`, a branch switch) leaves about 256 more objects that nothing collects.
- The other checkouts: `forma-pitch-mvp-talk/app/target` is 8.7 GB; this morning's log also cleaned `forma-pitch-mvp-model` and `-devstand`.
- `~/dev/Forma/work` is 52 GB (366 directories).
- The disk is 79 % used, 89 GB free (`df`).

---

## C. Deploy

### C.1 AWS beta (`beta.getforma.dev`, the pitch)

The procedure is `docs/pitch/DEPLOY-RUNBOOK.md`: **28 checklist items, 13 marked [MUTATION]**. It inherits names and helpers from `evidence/APP-078/prep/RUNBOOK.md` (64 items). The sequence (l.7-13):

1. preflight at count 0;
2. change set;
3. execute (the outage starts here, l.38);
4. snapshot;
5. migrate;
6. seed and grants;
7. `preflight.py --activate`.

The outage runs 10-15 minutes with a migration (l.45). Every [MUTATION] needs Victor's word in `$PRIV/approvals.md` (l.15), and Victor runs the secret steps himself (l.17, §4).

| Piece | In Git? | Location |
| --- | --- | --- |
| CDK stack, preflight (622 lines), `window_requests.py` (449), `readonly_snapshot.py` | yes | `infra/src/stack.ts`, `infra/scripts/` |
| Image build and qualify (3 ARM64 images) | **no** | `/private/tmp/claude-501/…/scratchpad/pitch-images/build.sh`, a Claude session scratchpad. Hard-coded candidate `C=3901992` (l.3), `set -u` without `-e` (l.1). It uses the operator venv from the other clone (l.10). |
| Push, preflight, synth, change set, review | **no** | `forma-app-platform/.cache/operations/beta/app-078-window/redeploy-pitch.sh` (30 lines, zsh). It hard-codes both clones' absolute paths (l.4, l.6) and uses `set -u` without `-e` (l.3). `aws cloudformation wait` is unchecked (l.26). |
| Operator task runner | **no** | `…/run-step-pitch.sh`. **Hard-codes `--window-id $WIN_gov`** (l.6), while `env-pitch.sh` also records the later windows `WIN_perf` and `WIN_alias`. Each run overwrites `$PRIV/run-$step.*` (l.6-13). |
| Environment | **no** | `env.sh` (22 variables: region, repo, candidate, priv, window, stack, registry, cluster, service, db, logs, assets, opvenv, snapshot, target group, input secret, …) and `env-pitch.sh` (9 window and snapshot ids). |
| Private state | **no** (by design) | `$PRIV` = `forma-app-platform/.cache/…/app-078-window` (`APP-078/prep/RUNBOOK.md:11-12`, `REPO=` **the forma-app-platform clone, not the pitch clone**). 564 files in one flat directory (298 `run-*`, 45 `change-*`, 18 `activation-*`). The runbook prescribes one `0700` directory per window (`DEPLOY-RUNBOOK.md:70`), but the script writes flat, label-suffixed files instead. |
| Library upload helper | **no** | `scratchpad/upload_library.py` |

**Volume:** 8 pitch activations on 2026-10-30 (`activation-{pitch,pane,wf,fold,rev,gov,perf,alias}.json`). The runbook still says "the five redeploys of 2026-10-30" (`DEPLOY-RUNBOOK.md:5`). Before the dev stand (`326d56c`, 10-30 22:54), these beta redeploys were how changes got seen in a browser.

### C.2 OVH (getforma.dev, the website), in `~/dev/FormaDeploy`

- **Layout:** two sibling repositories with their own remotes. `FormaDeploy.code-workspace` is a multi-root workspace named "Website" and "Infrastructure" (`FormaDeploy/README.md:5-9, 39-41`).
- **Task runner:** a `Makefile` with `help doctor dev-setup build preview preview-down bootstrap deploy verify status logs rollback tf-init tf-validate tf-plan test check` (`getforma-infra/Makefile:1-37`). Every target calls a script in `scripts/`, all of them in Git.
- **Build:**
  - A clean website checkout is required (`scripts/build.sh:5`).
  - The release ID is `<sha12>-<UTC>-<arch>` (l.8).
  - The image is saved as a tarball together with `compose.yaml`, `Caddyfile` and the activation script, checksummed with `SHA256SUMS` (l.14-29).
  - `.local/latest` is a symlink (l.30).
- **Deploy:** the payload is tarred over SSH, `activate.sh` runs remotely, and the previous release is kept for rollback (`scripts/deploy.sh:9-31`). No build happens on the VPS (`docs/DEVELOPER-WORKFLOW.md:204-206`).
- **Runtime:** Docker Compose with a read-only site container and Caddy, pinned by digest (`compose.yaml:1-57`).
- **Verify:** health and WASM compression checks, then the Playwright suite, local or public (`DEVELOPER-WORKFLOW.md:160-200`).
- **Rollback:** `make rollback` (`DEVELOPER-WORKFLOW.md:219-248`).
- **Terraform:** a read-only inventory of an already-purchased VPS (`FormaDeploy/AGENTS.md:17-19`).
- **Receipts:** each deployment has a committed receipt (`docs/deployment-receipt-2026-10-2*.md`).
- **Secrets:** only in `.env.local`, gitignored (`AGENTS.md:15-16`).

### C.3 Comparison

| | AWS pitch | OVH site |
| --- | --- | --- |
| Entry point | 28-step Markdown checklist plus out-of-git zsh | `make build / deploy / verify / rollback` |
| Scripts in Git | CDK and Python checks only | all of them |
| Release identity | tag `pitch-<sha7>-arm64`, 3 ECR digests, window UUID | `<sha12>-<ts>-<arch>`, `SHA256SUMS` |
| Private state | in the *other* clone's `.cache`, flat | `getforma-infra/.local/releases/<id>/` |
| Rollback | fix forward; a stack-level rollback is about 5 manual steps (`DEPLOY-RUNBOOK.md:240-257`) | `make rollback` |
| Approval gate | Victor's word per [MUTATION], budget ledger, preflight, change-set review | none beyond PR CI |
| Downtime | 10-15 min per deploy (`ServiceDesiredCount` pinned to 0, l.36) | brief restart |

The AWS path has much more safety machinery (preflight, ledger, change-set review), which the OVH path does not need. What the OVH path has and the AWS path lacks is **packaging**: one command per step, scripts in Git, one directory per release.

---

## D. Workspaces and context switching

- `forma-app-platform.code-workspace` is a **single folder** (`"path": "."`). Identical copies exist in `forma-app-platform`, `forma-pitch-mvp` and `forma-pitch-mvp-talk`. It has no tasks, no extension recommendations and no `rust-analyzer` linked projects.
- `FormaBuiltApps/gatewasm/gatewasm.code-workspace` (Victor's fAuth app) is the in-house good example:
  - 4 roots;
  - `rust-analyzer.check.command: clippy`;
  - excludes for target, node_modules and dist;
  - extension recommendations;
  - **tasks** for db up, SSR build, dev (server + frontend + mail), unit, integration and e2e (l.1-37).
- `FormaDeploy.code-workspace` has 2 roots (site, infra).
- **Repos and checkouts:**
  - `forma-pitch-mvp` is a separate clone of `getforma-dev/forma-app-platform` (`git remote -v`) on `pivot/pitch-mvp`, with 63 local `pivot/*` branches.
  - `forma-app-platform` is another clone, on `phase-one-spikes`. It holds the deploy state and the operator venv.
  - `forma-pitch-mvp-talk` is a worktree of the pitch clone (`pivot/from-data`).
  - Other worktrees (`-model`, `-devstand`) were removed by `clean-space.sh` this morning.
  - `~/dev/Forma/work` holds 366 ticket checkouts and is the shared Codex/Claude board; `clean-space.sh:14-16` never touches it.
- `.claude/launch.json` has 3 entries that only attach to 4001-4003, so they cannot start anything.
- **Cleanup tools** (outside every repo, `~/dev/Forma/work/_maintenance/`):
  - `clean-build-artifacts.sh` (112 lines) removes idle `target/` and `node_modules/`.
  - `clean-space.sh` (195 lines) handles incremental caches, merged worktrees, **stopped** `genui-*` containers (l.137), images, build cache, scratch except `parser-target`, `pitch-images` and `upload_library.py` (l.160), and stand scratch.
  - Both are dry-run by default and run only on demand.

---

## Root causes (incident → cause)

| Incident | Direct cause (evidence) | Underlying cause |
| --- | --- | --- |
| Victor had to ask for localhost | The binary refuses to start without AWS-shaped configuration (`main.rs:32, 75-94`; `.env.example` "intentionally incomplete and refused"). The stand first appears in `326d56c` (10-30 22:54), after the day's 8 beta redeploys. | The project started from deploy qualification, not from an inner development loop. Nobody owned a dev environment. |
| Stale `admin/dist` served an old canvas | `dist` is gitignored output, embedded and manifest-loaded at startup (`dev_stand.rs:80-82, 354-359`). Patched by forcing `npm run build` on each start (`f474ffb`, `dev_stand.sh:62-73`). | No dependency between "front-end source changed" and "server restarted". The framework's own watch mode (`npm run dev`) is unused. |
| Front-end rebuild not picked up | The manifest and FMIR modules load once at startup (`forma-server assets.rs:63-69`). `GUIDE.md:144` says a reload is enough. | The docs contradict the framework guidance (`building-and-verification.md:36-37`). There is no watcher that restarts the server on a `dist` change. |
| Stand crashed after 600 s | The harness writes a 600 s credentials file once (`qualification.rs:353`), and config refuses anything longer (`config.rs:118`). Patched with a refresh every 60 s (`6f08db9`, `dev_stand.rs:687-689`). | A short-lived campaign fixture is being run as an 8-hour server. Each assumption of the harness fails one at a time. |
| Two browsers on one persona flapped the live stream | One stream per session, and the proxy gave every browser the same session. Patched with 8 sessions per persona and a client cookie (`e7dda41`). | The cookie-injecting proxy stands in for real sign-in. **The pitch proxies started before `e7dda41` are still running the old code** (A.3, `dev_stand.sh:156`). |
| Docker VM disk filled (56 GB), engine hung | Docker Desktop's single `Docker.raw` (7.9 GB now, after cleanup). `docker build` uses BuildKit cache mounts for a full cargo `target` per arch (`app/Dockerfile:26, 40`). 3 images × 8 redeploys on 10-30, 13 retained `getforma-site` images and `rust:1.93.1` (2.2 GB) also sit there (`docker image ls`). The 56 GB figure itself comes from the incident report; it could not be re-observed. | Nothing prunes on a budget; `clean-space.sh` is manual. Release builds compile Rust inside the VM. |
| Worktrees each with a 6-60 GB `target/` | The default target is per checkout (`dev_stand.sh:41`). macOS `split-debuginfo = unpacked` keeps every unit's `.o`: 41 build hashes, 48,650 objects (B.7). | No profile tuning, no shared or `build-dir` layout, no sweep. |
| Python venvs missing in worktrees; `PYTHONPATH` pointed at another checkout | Venvs are gitignored per checkout and only CI YAML creates them (B.2). `fixture_python()` resolves via `CARGO_MANIFEST_DIR` (`qualification.rs:75-81`). Contracts are added via `PYTHONPATH` (`dev_stand.sh:121-125`). The `op-python` wrapper points at forma-app-platform. | No bootstrap command. Three overlapping locks where one dev environment would do (the operator lock is already their union). |
| Campaign test names hard to find | 34 ignored tests across 25 files, run by full module path with `--ignored --exact` (B.4). | No catalog or alias. Only 10 are in CI, so the rest are known only from ticket logs. |
| Leaked `genui-native` containers | `docker run --rm --detach` plus removal in `Drop` (`qualification.rs:122-144, 232-247`). A Ctrl-C (SIGINT) or SIGTERM to a campaign skips `Drop`, so Postgres keeps running; `--rm` only acts once it stops. `clean-space.sh:137` removes only **stopped** containers. 2 are running now (both owned by live stands). | Lifecycle is tied to a Rust destructor rather than to a supervisor or Compose project. There is no label-based sweep (`--label` is not set). |
| A second stand or worktree cannot run | Fixed ports (`dev_stand.sh:32, 36`) and a machine-wide `$TMPDIR/forma-dev-stand` (l.31, 35); APP-119:100. | Instance identity is not parameterized per checkout. |
| Deploy steps outside Git | `build.sh` lives in a session scratchpad, the redeploy and run-step scripts in the other clone's `.cache` (C.1). The scratchpad path is compiled into `dev_stand.rs:93` and `box_campaign.rs:66`. | Procedures were first written as shell during the deploy window and never moved into the repository. A temp directory and a sibling clone became infrastructure. |
| CI red on the pitch branch | 3 workflows failing on every recent run (B.6). | Nobody runs a local "same as CI" command, and nothing gates on CI. |

---

## Options survey

Each option below is judged on what it fixes here, its cost, its maturity, and whether it conflicts with Forma conventions.

**Constraints found in our own guidance:**
- AGENTS.md:13: "New dependency selections and commands belong in the owning implementation ticket. Pin actual released versions and lockfiles."
- The forma-stack skill:
  - "Prefer the target project's existing scripts and `AGENTS.md`" (`building-and-verification.md:5`).
  - Resource caps `CARGO_BUILD_JOBS=3`, Playwright ≤2 workers (l.60-61). `dev_stand.sh` sets no `CARGO_BUILD_JOBS`.
  - "Do not run duplicate servers/campaigns; check who owns the listener" (l.62-63).
- The Forma framework keeps npm lifecycle scripts off (`.npmrc` `ignore-scripts=true`; CONTRIBUTING.md:141-147, FORMA-117).

None of the guidance forbids just, make, process-compose, Compose, mise, uv or devcontainers. The precedents in our own projects:
- Makefile + Compose + Caddy (getforma-infra);
- npm scripts + `concurrently` + Compose Postgres + VS Code tasks (gatewasm `package.json` `dev`, `docker-compose.yml`);
- npm scripts + `scripts/check.sh` (the framework).

### Task runners: one front door for "start / check / campaign X / deploy step N"

| Tool | Solves for us | Cost | Maturity | Conflicts |
| --- | --- | --- | --- | --- |
| **just** ([just.systems](https://just.systems), [repo](https://github.com/casey/just)) | Named recipes with arguments: `just stand`, `just campaign mcp`, `just check`, `just deploy-build`. Loads `.env`. A good fit for listing the campaigns (B.4) and moving the out-of-git deploy shell into Git. | `brew install just`, a single binary, one `justfile`. | ~36.5k stars, CC0. | None. It is a new tool, so it needs a ticket and a pin (AGENTS.md:13). |
| **make** | Same, already used by getforma-infra (`Makefile`). | Zero install. | Universal. | None, and it has in-house precedent. Weaker with arguments and quoting. |
| **cargo xtask** (pattern, [matklad/cargo-xtask](https://github.com/matklad/cargo-xtask)) | Rust code instead of bash for orchestration (stand lifecycle, campaign catalog). Typed, testable, cross-platform. | A Cargo workspace is needed (`app/` is a single package today), plus compile time. | A convention, not a tool; widely used. | It adds another crate to build in a repo where build size is already the problem. |
| **cargo-make** | TOML tasks with dependencies. | A heavy install, its own DSL. | Mature. | Overlaps with just/make and adds little for us. |
| npm scripts | Already the framework's and gatewasm's front door. | None. | Mature. | Odd for a Rust- and Python-heavy repo; `ignore-scripts` policy applies. |

### Process orchestrators: the "concurrently" for Rust (stand, proxies, watcher, DB)

| Tool | Solves for us | Cost | Maturity | Conflicts |
| --- | --- | --- | --- | --- |
| **process-compose** ([docs](https://f1bonacc1.github.io/process-compose/), [repo](https://github.com/F1bonacc1/process-compose)) | Replaces the pid files and `kill -0` in `dev_stand.sh` with dependencies, readiness probes, restart policies, a TUI, per-process logs and a REST API. One YAML can start Postgres (or the Compose DB), the server, proxies or sign-in, and the admin watcher, with restarts done right (fixes A.3). | A single Go binary. | ~2.9k stars, Apache-2.0. | None found. |
| **overmind** / hivemind ([repo](https://github.com/DarthSim/overmind)) | Procfile; restart one process; auto-restart (`-r`). | Overmind **requires tmux**; hivemind does not. | ~3.8k stars, MIT. | Procfile has no readiness or dependencies, which matter here (DB → migrate → server). |
| **mprocs** ([repo](https://github.com/pvolok/mprocs)) | A TUI that runs several commands side by side. The repo page now presents itself as **dekit** (`dekit.yaml` with `deps` and `ready`); verify the current name and state before choosing it. | Rust binary, MIT, ~2.8k stars. | In flux. | None. |
| **concurrently** (npm, used by gatewasm) | Runs N commands with colored prefixes. | npm dev dependency. | Mature. | No readiness or restart. It is npm in a Rust and Python repo. |
| **docker compose** for everything | One `compose.yaml` with `depends_on: condition: service_healthy`, used in getforma-infra and gatewasm. | Builds the Rust server inside the Docker VM, which is slow and is the disk problem (root cause row 6). | Mature. | Fine for the **database**; poor for the inner loop of a server built natively. |

### A Postgres dev database (replacing the per-run tmpfs fixture for development)

- **Compose service with a named volume and a healthcheck**, as in gatewasm `docker-compose.yml` (`db` and `db-test` services). Data survives server restarts, so there is no re-seed on every change (A.3). Its lifecycle is a Compose project, so nothing leaks through `Drop`.
- **Cost:** the stand needs a real development configuration path: a TOML or env-file config for the binary, or a harness entry point that takes an existing database. That is the actual separation-of-concerns work (A.2). The campaigns keep their throwaway fixtures, and should add `--label` so a sweep can find stray containers.

### File watchers

| Tool | Status | Notes |
| --- | --- | --- |
| **cargo-watch** (installed in `~/.cargo/bin`) | **No longer maintained.** Its README says it is "on life support" and recommends Bacon or Watchexec ([repo](https://github.com/watchexec/cargo-watch)). | Do not build on it. |
| **bacon** ([repo](https://github.com/Canop/bacon)) | Active. Jobs in `bacon.toml`, `bacon --init`. | **AGPL-3.0**: fine as a local dev tool that is not distributed, but say so in the decision. Best for check/clippy/test feedback. |
| **watchexec** | Active, general-purpose (the cargo-watch engine). | Good for "rebuild admin/dist → restart server"; or use process-compose's restart together with the framework's `npm run dev --watch`. |

### Build caching and artifact cleanup

- **Profile tuning, free, biggest effect** ([Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)):
  - `[profile.dev] debug = "line-tables-only"`, or `split-debuginfo = "packed"` on macOS, removes most of the retained `.o` files (B.7).
  - `[profile.dev.package."*"] debug = false` keeps dependencies without debug info.
  - These belong in `app/Cargo.toml` or `.cargo/config.toml`. Release images are unaffected.
- **`build.build-dir`** ([Cargo config](https://doc.rust-lang.org/cargo/reference/config.html); present in the 1.93.1 config reference): separates intermediate artifacts from final outputs and supports `{workspace-path-hash}` and `{cargo-cache-home}` templating. That allows one cache root per checkout outside the tree, which tooling can sweep.
- **Shared `CARGO_TARGET_DIR`:** dependencies are compiled once across worktrees. `dev_stand.sh:88-91` already copies the executable privately in anticipation of this. Cost: cargo's build lock serializes concurrent builds across worktrees, and branches churn each other's artifacts.
- **sccache** ([repo](https://github.com/mozilla/sccache)): "Incrementally compiled crates cannot be cached", and bin and proc-macro crates are not cached either. With incremental dev builds it helps dependencies across worktrees, not `genui` itself.
- **cargo-sweep** ([repo](https://github.com/holmgr/cargo-sweep)): `--time`, `--installed`, `--maxsize`, `--recursive`. Its README says it is **currently unmaintained** and points to `cargo-clean-all`. Our own `clean-space.sh` already does a size-aware sweep. Moving that sweep into the repo's task runner, with a label-based Docker sweep, is the low-risk option.

### Toolchain and environment pinning ("venv for Rust")

| Tool | Solves for us | Notes |
| --- | --- | --- |
| **`rust-toolchain.toml`** (rustup) | Pins 1.93.1 plus components and the wasm target for every cargo call in the directory, CI and local alike (B.1). | No install. The Forma framework uses one only in `fuzz/` (`forma/fuzz/rust-toolchain.toml`), and CI already pins by command line. Free, and the closest Rust equivalent of "venv". |
| **mise** ([site](https://mise.jdx.dev/)) | One `mise.toml` for Rust, Node and Python versions, env vars and tasks. `[env] _.python.venv = { path = ".venv", create = true }` creates and activates a venv per checkout and **uses uv when installed** ([python docs](https://mise.jdx.dev/lang/python.html)). It directly fixes "worktree without venv". | New tool and shell hook. It overlaps with pyenv (in use) and rustup. |
| **direnv** | Per-directory env (`PYTHONPATH`, `CARGO_TARGET_DIR`, ports). | Small. mise covers it. |
| **devbox / nix** | Fully reproducible toolchains. | High cost and learning curve; nothing in Forma uses them. |
| **uv** (installed) | `uv venv` plus `uv pip sync --require-hashes -r …lock` works with our existing hashed locks ([uv docs](https://docs.astral.sh/uv/pip/compile/)), so a bootstrap recipe can create every venv per checkout in seconds. One dev venv from the operator union lock would remove the `PYTHONPATH` splice. | Keeps `--require-hashes` and `--only-binary`. No change to the locks is needed to start. |

### Editor and containers

- **A VS Code multi-root workspace with tasks**, modeled on `gatewasm.code-workspace`: roots for app, admin, infra, contracts and Tickets; `rust-analyzer.check.command`; excludes; tasks bound to the task-runner recipes. Cost is close to zero.
- **`.claude/launch.json`** can name a start command for the stand instead of attach-only URLs, if the stand gets a single foreground command (process-compose or `just stand`).
- **Devcontainers:** a reproducible environment, but the Rust build would run inside Docker Desktop's VM on this Mac, which is the resource that already filled and hung (root cause row 6). Not recommended as the primary inner loop here. Only `N64FlashcartMenu` uses one on this machine.

### What a decider might combine (inputs, not a decision)

1. **A dev configuration path in the binary**, or a stand entry point that takes an existing database, separate from the qualification harness. Use a Compose Postgres with a named volume for development; campaigns keep throwaway fixtures with labels.
2. **One process supervisor** (process-compose, or make/just plus Compose) owning DB, server, sign-in or proxies, and the admin watcher, with per-checkout ports and state.
3. **One front door** (just or make; make has in-house precedent) for bootstrap (uv venvs, `npm ci`), stand, check (same as CI), the campaign catalog, deploy build/push/run-step and clean. Every out-of-git deploy script and the scratchpad parser path move into the repo or `$PRIV` by name.
4. **Pins and build hygiene:**
   - `rust-toolchain.toml` 1.93.1;
   - `.node-version`;
   - `.python-version`;
   - dev-profile debuginfo tuning;
   - `CARGO_BUILD_JOBS=3`;
   - a size-bounded sweep.
5. **A multi-root code-workspace with tasks.**
6. **ADR and procedure:** an ADR recording these choices, plus `docs/pitch/GUIDE.md` and `DEPLOY-RUNBOOK.md` corrected (front-end reload, "five redeploys", `$PRIV` location).
