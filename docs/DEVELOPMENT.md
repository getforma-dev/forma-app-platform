# Developing the Forma app platform

This is the working guide for the pitch application: setup, local development, checks, delivery and agent rules. The decisions are [ADR-012](architecture/ADR-012-DEVELOPER-WORKFLOW.md) and [ADR-013](architecture/ADR-013-REPOSITORY-AND-DELIVERY.md); the executable entry points are in the root [justfile](../justfile). Run commands from the repository root. `just` lists the recipes.

## Project and branch model

The product is a modular monolith: a Rust service renders typed FMIR pages with registered Forma canvas islands; PostgreSQL holds identity, grants, work, records and audit; confined Python children run the DuckDB queries; a separate sealed Rust/WASM parser handles imports. Model output is typed data admitted by the service, never executable code, SQL or a permission grant. The [module map](architecture/MODULE-MAP.md) records dependencies, known leaks and their owners. Dependency enforcement and a Python-to-Rust engine decision remain APP-097 work; neither is part of setup.

`pivot/pitch-mvp` is the trunk and beta candidate branch until the November 3 revisit. Work on `pivot/<topic>` branches from it, review the change and its CI, then merge back. `main` stays frozen at the Phase 1 go-live record; the landing PR and future production policy are decisions for APP-097. Both branches are protected against force-push and deletion. There is one clone plus worktrees; do not revive the archived second clone or borrow its environment.

| Path | Responsibility |
| --- | --- |
| `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml` | Root workspace: `app` and `spikes/APP-006` (auth verifier), shared dev profiles and three build jobs; one `target/` per checkout |
| `app/src/`, `app/admin/` | Rust application and TypeScript canvas; `product` composes the routes |
| `app/src/dev/`, `scripts/dev/`, `compose.dev.yaml`, `process-compose.yaml` | Feature-gated dev binary, recipes, PostgreSQL and process supervision |
| `app/runtime/`, `app/migrations/`, `contracts/` | Confined query/runtime children, versioned schema and locked cross-language contracts |
| `parser/` | Separate workspace, lockfile, toolchain, vendored patch and release profile |
| `infra/`, `ops/beta/` | AWS infrastructure and the scripts for each beta delivery step |
| `Tickets/`, `docs/`, `evidence/`, `design/` | Authoritative work board, maintained procedures, retained checks and frozen design evidence |

Open [forma-pitch-mvp.code-workspace](../forma-pitch-mvp.code-workspace) in VS Code. Its tasks call these same recipes. The framework and the `getforma.dev` website have their own repositories and instructions; the website's OVH pipeline does not deploy this AWS application.

## First hour

Use the existing `forma-pitch-mvp` checkout, or clone `getforma-dev/forma-app-platform` and select `pivot/pitch-mvp`. Install rustup, Node/npm, uv, just, process-compose and Docker with Compose. The exact versions are in [tools.versions](../tools.versions), [rust-toolchain.toml](../rust-toolchain.toml), [.node-version](../.node-version) and [.python-version](../.python-version). `just setup` installs the pinned Rust/Python toolchains and project dependencies; it does not install Node or the task runner/supervisor for you.

```sh
just setup
just doctor
just dev -d
just status
```

`setup` creates local hashed-lock venvs at `app/runtime/.venv`, `contracts/.venv`, `app/operator/.venv` and `spikes/APP-005/.venv`, and runs `npm ci` for `app/admin` and `infra`. Repeat it after lockfile changes or in every new worktree; `just setup --force` reinstalls Node dependencies. `doctor` compares installed tools and the venvs with the recorded pins. Resolve differences before treating local checks as equivalent to CI.

The main checkout's defaults are:

| Profile | Persona | Canvas |
| --- | --- | --- |
| Pitch | Diego Ramos, filer | [127.0.0.1:4001/canvas](http://127.0.0.1:4001/canvas) |
| Pitch | Sarah Chen, reviewer | [127.0.0.1:4002/canvas](http://127.0.0.1:4002/canvas) |
| Pitch | Priya Natarajan | [127.0.0.1:4003/canvas](http://127.0.0.1:4003/canvas) |
| Clean (`just dev clean -d`) | Alex Rivera, new customer | [127.0.0.1:4010/canvas](http://127.0.0.1:4010/canvas) |

`just dev` without `-d` runs in the foreground (a TUI in a terminal). The first build can take minutes. Pitch starts with the synthetic Northstar scenario and 20 library apps. A newly reset clean instance has no apps or records and demo mode is off; its data persists after you start using it. Each browser gets its own persona session. These loopback listeners mint sign-in locally; they do not test the hosted fAuth flow.

Known local limitation (APP-135): the header's Workspace link currently returns 404 because that legacy route requires a configured hosted provider. Use the canvas for the dev workflow; do not bypass the route's authentication check. The legacy full-page work view also retains source-query controls for records-only apps; the canvas is their form/records surface.

### Configuration and state

- Keep model secrets in the main checkout's gitignored `.env`. `just` does **not** load or source that file. The dev server reads only the needed `ANTHROPIC_API_KEY` through `app/scripts/read_env.py` into its process environment. A worktree can resolve the main checkout's file through Git's common directory. Do not copy keys into a worktree, commands, logs or chat.
- Without a model key, only the available deterministic/rehearsed paths work; clean-slate generation and real Ask/Shape calls need the model. Slack integration additionally needs its two secrets in the launching environment; the [Slack setup](pitch/SLACK-SETUP.md) owns that procedure.
- `.env.dev` holds the non-secret `GENUI_DEV_PORT_BASE`. The main checkout defaults to `4000`; a worktree selects and records an available base from `4100` upward. From that base, pitch uses `+1/+2/+3`, clean uses `+10`, databases `+20/+21` and supervisor APIs `+30/+31`. Use `just status` and `just status clean` for that checkout's URLs. Set a different base before starting if needed; the launcher refuses occupied ports.
- `.cache/dev/<pitch|clean>/` holds local credentials, objects, revision caches and logs; PostgreSQL data lives in the profile's named Docker volume. Each checkout has its own Compose project and dev state. Local objects replace S3 in this loop.
- `.cache/reports/`, `.cache/parser-target/`, `.cache/images/` and `target/` also stay per checkout. The exception is operator state: `$PRIV` resolves to the **main checkout's** `.cache/ops/beta/` through `git-common-dir`, so worktrees share one approval log and budget/window ledger. `GENUI_OPS_PRIV` is an explicit override, used by isolated operator tests; do not use it to start a second live ledger. `just deploy help` prints the selected path.

## The editing loop

| Change or task | Command and effect |
| --- | --- |
| Front-end source | The supervisor's watcher rebuilds `app/admin/dist`; the server notices the changed manifest and restarts. Reload the page after the build finishes. Database and browser session slot are kept. |
| Rust source | `just restart` or `just restart clean` rebuilds/restarts that profile's server and keeps its data. A failed build stops that server; fix it and repeat the restart. |
| Local schema/seed reset | `just dev reset -d` or `just dev clean reset -d` drops **that profile's database and private dev state**, then starts it again. Use only when its current data is disposable. |
| New-customer workflow | `just dev clean -d`; use `just dev clean reset -d` when you need it empty again. |
| Templates on clean | `just dev clean seed-library` installs the admitted template apps and grants for Alex. A clean reset removes them. |
| Status/logs | `just status`, `just status clean`, `just dev logs`, `just dev clean logs`; per-process files are under `.cache/dev/<profile>/logs/`. |
| Stop/start | `just dev stop` / `just dev -d`; `just dev clean stop` / `just dev clean -d`. Stop keeps the database volume. |

Tell the owner before restarting, resetting or stopping an instance they are using. Check which checkout owns the listener; do not kill another checkout's process to free a port. Use separate worktree ports for independent testing. The [pitch script](pitch/PITCH-SCRIPT.md) and [recording checklist](pitch/RECORDING.md) name exact demo words, expected outcomes and limitations.

## Before pushing

```sh
just check --list
just check
```

`check` runs formatting, clippy (including the dev feature), Rust library tests, binary SSR tests and doctests, the available MSRV check, campaign-catalog consistency, admin typecheck/build, infrastructure typecheck/tests, Python suites, schema pins, contracts, board checks and shell syntax checks. It runs the selected steps to completion and summarizes failures. Run a focused subset with names from `--list`, for example `just check fmt py-runtime py-ops board`. For rendering changes, use `just check admin test` so the SSR tests read freshly built assets. The MSRV step reports a skip if Rust 1.85.0 is not installed; a skipped local check is not a pass. The admin build also changes the manifest watched by running dev servers.

`just ci` adds the tenant schema qualifier and all campaigns marked as CI in the catalog. It is the local app-check set, not a substitute for the independent parser, published-baseline, fixture and image jobs in GitHub Actions. Inspect the exact pushed commit's workflow results before merging.

### CI failures and quarantine

The workflow source is [`.github/workflows`](../.github/workflows). All twelve workflows are enabled; none is quarantined. A green older branch run does not establish a green merged commit. Ten workflows cancel superseded runs per workflow/ref. APP-131 records two exceptions: Contract qualification's workflow bytes are pinned by the implementation lock, and Parser release qualification retains the push qualification record for each exact commit.

| Workflow family | Investigate first |
| --- | --- |
| Planning / Contract | Board rendering and links; contract schemas, vectors, locks and local venv installation |
| Foundation preparation | Foundation source/schema checks, confined runtime unit tests and the service image compile check; this job's AMD64 image is not the ARM64 deploy artifact |
| Tenant foundation | Schema/role denials, identity, commands, races and queries; reproduce the failing catalog campaign |
| Pitch campaigns | `actions`, `canvas`, `composer`, `mcp`, `pitch`, `sharing`, `slack`, `workflows`; these use synthetic/local providers without production keys |
| Released baseline / Published canvas / Auth consumer | The owning frozen spike and released consumer contract |
| Parser / Parser release / Bounded query / Synthetic fixture | The owning parser, query or fixture qualification and its source-bound report |

APP-131 repaired the missing `jsonschema` dependency, updated command/race fixtures to use a separate reviewer while retaining self-approval denials, and corrected the library-count expectation. Do not weaken authorization to make a campaign pass. Under ADR-012, a broken workflow is fixed or explicitly quarantined to `workflow_dispatch` with an owning ticket and a row here naming the limitation; red-and-ignored is not an accepted state. No quarantine is currently recorded.

## Campaigns

```sh
just campaigns
just campaigns --ci
just campaigns --all
just campaign command
```

The [catalog](../scripts/dev/campaigns.tsv) maps short names to exact ignored tests, prerequisites, report variables and CI workflow. `--all` includes external smokes and managed helpers; helpers cannot run alone. Reports, when the campaign defines one, go to `.cache/reports/<name>.json`. Fixtures use disposable labeled Docker databases, separate from the persistent dev profiles.

There are ten tenant-foundation and eight pitch campaigns in the current CI catalog. The box campaign and parser-sandbox probe require a sealed parser; real-model and Open-Meteo smokes use external services and are not run by `just ci`. Follow the named prerequisite/wrapper for a smoke rather than putting a key on its command line. A campaign's synthetic provider test is not evidence of a real-provider run.

## Parser and release images

`just parser` builds the separate parser workspace into `.cache/parser-target/`; `just parser --print-dir` reports the path. Build it before local CSV/XLSX import testing. These are local development bytes; the image stages build and qualify their own sealed parser.

`just images <candidate-sha>` builds the service, qualification and operator images for **Linux ARM64**, checks all three and compares the parser manifests. Use a clean checkout at the candidate commit, already pushed and checked. The result is `.cache/images/<sha7>/build.json` plus logs/check reports, and local `genui-{service,qualification,operator}:pitch-<sha7>` tags. No image is pushed by this recipe. The later push step verifies that the local image IDs still match the qualified build record. Do not use the dirty-tree escape hatch for a deploy candidate.

The three images are separate trust boundaries: the service has no qualification harness, the qualification image is never behind the public load balancer, and the operator image owns migrations. Root-workspace changes must update all three Dockerfiles/allowlists together; the parser keeps its separate lockfile.

## Delivery: local to beta, production later

```text
edit → just check / focused campaigns → push topic → exact-commit GitHub CI
  → reviewed merge into pivot/pitch-mvp → pushed, clean candidate
  → just images <sha> (local ARM64 images + build.json)
  → just deploy new → observe → push → preflight → synth → diff
  → change-set → review → execute → snapshot/migrate when needed
  → seed/library/grants as required → activate → verify → rehearsal
  → release.json + RELEASES.md row + beta/<yyyymmdd>-<sha7> tag
```

Use the full [beta deploy runbook](pitch/DEPLOY-RUNBOOK.md); the diagram is an index, not a replacement for its conditions. `just deploy help` lists actual arguments. Window arguments are positional IDs, for example `just deploy observe <window-id>`, not the directory path and not a `--window` option.

| Runbook stage | Recipe |
| --- | --- |
| 0, prepare | `just images <sha>`; `just deploy new --candidate <sha> --supersedes <live-window-id>` (with `--input-from <file>` for the imported legacy input when needed) |
| 1, observe | `just deploy observe <window-id>` |
| 2, ECR | `just deploy push <window-id> --approved "<Victor's words for this action>"` |
| 3, reviewed stack change | `just deploy preflight <window-id>`, `synth`, `diff`, `change-set`, `review`, `execute` with the same ID; `change-set` and `execute` each require their own `--approved` words |
| 4, secrets | Victor's commands in the runbook; intentionally no recipe |
| 5, database/library | `just deploy snapshot <window-id> --name <snapshot-id>`, `migrate`, `seed`, `library`, `grants` as required; each mutating call also requires `--approved` |
| 6, activate/check | `just deploy activate <window-id> --approved "<Victor's words for this action>"`; `just deploy verify <window-id>`; then the runbook's signed-in, agent and Slack checks |
| Abandon before activation | `just deploy abandon <window-id>` only under the runbook's rollback conditions |
| Immediate deny-all rollback | `just deploy rollback-deny <window-id> --approved "<Victor's words for this action>"`; `--restore` is a separate approved action |

AWS mutations require Victor's verbatim approval **per action**, appended to `$PRIV/approvals.md` before execution. Never invent or reuse words from another step. `--dry-run` prints without calling AWS or recording approval; the mutation gate can still refuse it when approval words are absent. Finishing local work or read-only preparation does not authorize activation.

One `$PRIV/windows/<window-id>/` holds the input, reservation receipt, templates, reviewed change set, stack snapshots, operator run reports, activation record and `release.json`. The shared `budget-ledger.json` and `approvals.md` stay outside individual windows. Preserve `windows/_pre-adr-012/` as imported history. The operator venv stays in the invoking checkout at `app/operator/.venv`; only the ledger/private records are shared.

After a healthy activation, the recipe records the release, appends [RELEASES.md](pitch/RELEASES.md), and creates/pushes the annotated `beta/<yyyymmdd>-<sha7>` tag. Commit the public release row as directed by the recipe. Verification records the four HTTPS surface results in `release.json`; these checks do not replace the signed-in rehearsal. ADR-013 step H remains the first real window run entirely through these recipes; fake-AWS tests alone do not complete it.

Executing the change set normally starts an outage until activation, with about 10–15 minutes budgeted when migrations are involved. Fix forward by default. A previous image needs a reviewed rollback window with its recorded digests, not an arbitrary old ECS revision. After a schema migration, older builds refuse the new catalog; snapshot restore is a separate reviewed recovery operation. The deny-all revision is the immediate containment path described in the runbook.

**Production does not exist.** ADR-013 defers it until the November 3 revisit: a separately budgeted stack with its own database, secrets and keys, promoting the exact three verified beta digests without rebuilding. Account separation before the first paying customer, budget approval and the landing on `main` remain decisions to complete. `getforma.dev` on OVH is a separate website, not this future production environment.

## Housekeeping

Start with `just clean docker --dry-run` or `just clean target --dry-run` and inspect the result. `just clean` defaults to `all`.

- `just clean docker` removes stopped containers and unused non-dev volumes carrying `genui.scope`, prunes BuildKit cache older than 72 hours, and keeps the newest three local pitch image sets. It keeps running containers, stopped dev database volumes and unlabeled legacy containers. The Docker engine/cache is shared across checkouts, so coordinate with an active image build.
- `just clean target` removes this checkout's root `target/`, a legacy `app/target`, `app/admin/dist` and **`.cache/dev/`**. It has no running-instance guard. Stop both profiles with their owner before using it; it deletes local objects/credentials as well as build output. Use `just dev [clean] reset` when you intend a consistent profile reset including its database.
- If Docker's VM is full, pause new builds, inspect the dry-run sweep and retain any needed evidence before cleaning. Do not delete Docker's VM disk or a live dev volume as a shortcut.
- The machine-wide sweeps remain under `~/dev/Forma/work/_maintenance/` (`clean-space.sh`, `clean-build-artifacts.sh`). They span repositories and are dry-run first; coordinate with other work. They are not another app deployment path.

## Rules for contributors and agents

1. Read [AGENTS.md](../AGENTS.md), the assigned ticket, its decisions and the actual source. Pre-assign unique APP IDs before parallel work. Ticket Markdown is authoritative; regenerate the board after ticket edits and check it. Keep unfinished acceptance and phase gates explicit.
2. Preserve typed admission, tenant isolation, identity checks, receipt provenance and transactional/idempotent audit. Keep the complete FMIR `PageResponse` contract. Demo shortcuts stay scoped and listed in APP-098; no synthetic check proves a live deployment.
3. Give new authority an independent review before merging. Use the risk tiers in [QUALITY-GATES.md](planning/QUALITY-GATES.md), repair findings and run the checks that exercise the changed behavior. Documentation alone is Tier C self-review plus the board/link check.
4. Keep product code, dev tooling, qualification harnesses and operations in their owning directories above. A script used twice belongs in Git; a scratchpad is not infrastructure. New tools/commands need the owning ticket, an actual version and a lock/pin where applicable.
5. Never borrow another checkout's venv, build directory, parser or scratch files. Run `just setup` there. Only the same repository's main `.env` and shared operator ledger/private records cross the worktree boundary through their explicit resolvers.
6. Preserve other agents' changes and the owner's running instances. Tell the owner before disruptive local actions. Keep credentials, private state and real data out of Git and artifacts; retain sanitized evidence. Show the founder-notes inventory before untracking anything.
7. Branch CI, local acceptance and AWS activation are separate outcomes. Follow the current authorization, not a historical approval excerpt. Do not create production, deploy beta, prune branches or mark a gate passed merely because a handoff lists them as future work.
