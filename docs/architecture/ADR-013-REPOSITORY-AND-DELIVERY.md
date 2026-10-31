# ADR-013 — Repository and delivery: one clone, a root workspace, digests up the ladder

Date: 2026-10-31. Owner: GenUI (Claude, pitch lane). Status: **Accepted** (Victor, 2026-10-31: "Accept, approve all": the record and every item marked [Victor's word]: branch protection, archiving the second clone, removing the -talk worktree once its work is merged, one landing PR at APP-097, and the production approach, whose budget and account are settled after Nov 3). This record decides the repository model, the code structure and the AWS delivery pipeline that [ADR-012](ADR-012-DEVELOPER-WORKFLOW.md) leaves open. It changes no product behavior contract (ADR-003, 004, 006, 007, 009, 011 stand) and no AWS procedure gate: every [MUTATION] still needs Victor's word per action. Evidence was read at `pivot/pitch-mvp` `39a1c6e`, `pivot/devflow` `026f9ff` plus its uncommitted `ops/` (ADR-012 step 3 in progress), and the sibling clone `forma-app-platform` at `21159f5`.

Victor's words (2026-10-31): "was the development env hardened? Are we in a coherent dev model? a modular monolith? a workspace? like all files and folders in a clean dev working station with a clear progressive pipeline all the way to production and publishing?" and, later the same day: "not ovh — the aws pipeline and build and the docker yaml files etc. I need all of that built and finalized as soon as possible."

## Context

**Repositories.** `getforma-dev/forma-app-platform` is cloned twice: `FormaBuiltApps/forma-pitch-mvp` on `pivot/pitch-mvp` (the pitch, ADR-011 §2) and `FormaBuiltApps/forma-app-platform` on `phase-one-spikes` at `21159f5` (the paused ADR-010 track). The second clone has **no commit that is not on `origin`** (`git rev-list --count <branch> --not --remotes` is 0 for all 44 local branches, 15 of them `worktree-agent-*`), no modified file and no stash. What it still holds is `.cache/operations/beta/` (138 MB): the pre-ADR-012 deploy state, which ADR-012 step 3 imports (`ops/beta/import-legacy-priv.sh` on `pivot/devflow`). The pitch clone has three worktrees: `-devflow` (`pivot/devflow`, ADR-012 steps 1–3), `-devserver` (`pivot/devserver`, step 4, at the pitch tip), `-talk` (`pivot/from-data`, 9 modified files, 1 unmerged commit, 3 behind).

**Branches.** `main` is `f229f01`, PR #4, the Phase 1 go-live record of 2026-10-30. `pivot/pitch-mvp` is **191 commits ahead of `main`** with 45 merge commits: topic branches `pivot/<topic>` are merged with `git merge origin/pivot/<topic>` directly, no pull request (PRs #1–4 were all landings into `main`). Beta runs `422d332`, a `pivot/pitch-mvp` commit (DEPLOY-RUNBOOK.md "State at the last deploy"), so **`main` is not what is live**. No branch is protected (`gh api …/branches/main/protection` → 404, same for `pivot/pitch-mvp`); the repository is temporarily public (README.md:3); there are **0 tags**.

**Code.** One Rust package, `forma-app-platform` (`app/Cargo.toml`), library `genui`, 25 modules (`app/src/lib.rs`), in one process with four logical roles (DECISIONS.md APP-D05). It has one **path dependency on a spike**: `genui-auth-verifier = { path = "../spikes/APP-006" }` (`app/Cargo.toml:25`), a production dependency. `parser/` is its own Cargo workspace (`parser/Cargo.toml:1`) with its own toolchain file, a vendored `zip` patch and dependency `opt-level = 2`; ADR-009 D9.1 keeps it out of the app's Cargo graph. Spikes APP-001/003/004 are frozen Rust evidence with their own lockfiles; APP-005 is Python. There is no root `Cargo.toml`. The TypeScript canvas lives in `app/admin` (`@getforma/core 3.2.0`), the confined DuckDB engine in `app/runtime/*.py`.

The `use crate::` matrix (non-test and test code together) says where the boundaries are and where they leak:

| Module | Depends on |
| --- | --- |
| `admission`, `supervise`, `config`, `roles`, `readiness`, `origins`, `directory` | nothing in the crate |
| `auth` | `http`, `storage` |
| `events` | `auth` |
| `query` | `actions admission auth boxes composer demo directory events storage supervise` |
| `composer` | `actions admission demo directory events http query sharing storage widgets` |
| `actions`, `boxes`, `commands`, `sharing`, `widgets`, `imports` | the domain neighbours plus `admission`/`auth`/`storage` |
| `mcp` | `actions admission auth boxes commands composer events query sharing storage` |
| `slack` | `auth boxes directory events mcp sharing storage` |
| `pages`, `product`, `main` | `auth`, `commands`, `query` (composition only) |
| `storage` (37.6k lines) | **everything**, including `pages`, `product`, `readiness` |

`storage` depends on everything because it hosts the qualification campaigns (`*_qualification.rs`, `*_campaign.rs`, `tests.rs`) and `dev_stand.rs`; `http` depends on `boxes` and `composer` (route helpers reaching into domain types). Those are the two leaks.

**Delivery today.** Three Dockerfiles, each a multi-stage build from the repository root with a per-Dockerfile ignore file and digest-pinned bases (`node:22.21.0`, `rust:1.93.1`, `python:3.11.14`): the **service** (`app/Dockerfile`: browser → native → parser → python runtime; the service never contains tests), the **qualification** image (`app/qualification/Dockerfile`: the `cfg(test)` library test executable, never behind the ALB) and the **operator** image (`app/operator/Dockerfile`: migrations, seed, admission channel, the separate identity of ADR-007 D7.19). The parser stage is written twice (service and operator), and the two outputs are checked equal by `parser-manifest.json` digests (APP-078 runbook 2.5). Images are built **locally on the Mac, ARM64 native** (`docker build --platform linux/arm64`, APP-078 §2), qualified by `qualify_image.py`, `qualification/check_image.py`, `operator/check_image.py`, pushed to ECR `genui-synthetic`, `genui-qualification`, `genui-operator` (account `877573132739`, `us-east-1`) under the mutable tag `pitch-<sha7>-arm64`, and the stack takes the three **immutable digests** as parameters (`infra/README.md`, `ImageDigest`). CI builds `app/Dockerfile` on an AMD64 runner as a compile check only (`foundation.yml:31`, `parser-release.yml:66`); nothing in CI pushes to ECR. A deploy is a CloudFormation change set (preflight, ledger, change-set review, execute, snapshot, migrate, seed, grants, activate), 10–15 minutes of downtime, fix-forward by default, the deny-all task revision as the fast rollback (DEPLOY-RUNBOOK.md "Rollback"). ADR-012 step 3 puts all of that behind `just deploy <step> <window>` with one private directory per window and a `release.json` (`ops/beta/deploy.sh`, uncommitted on `pivot/devflow`). There is no production environment; the budget is ADR-003's $100/month, raised to $170 GenUI / $208 project for the always-on beta (ADR-008; runbook 3.1).

**Out of scope.** The getforma.dev marketing site on OVH (`~/dev/FormaDeploy`, `getforma-infra` Makefile) is a separate repository and pipeline; the product never deploys there and this record does not touch it (Victor, 2026-10-31: "not ovh").

Constraints: ADR-011 §2 and §6 (what the pitch proves lands back into the platform after Nov 2, revisit APP-097); the concept-car stance (MVP first, no verification campaigns); AGENTS.md:13 (new tools through an owning ticket, pinned); ADR-012 D12.4 (pins, dev profile, per-checkout `target/`) and D12.6 (deploy in Git, `--approved` gate, `$PRIV` layout); ADR-009 D9.1 (parser outside the app's Cargo graph); ADR-007 D7.13 (harness never in the release binary).

## Decision

### D13.1 Repository model: one clone, worktrees per lane, the second clone archived

- **`FormaBuiltApps/forma-pitch-mvp` is the one clone** of `getforma-dev/forma-app-platform`. Parallel lanes are **worktrees** of it, named `forma-pitch-mvp-<lane>` beside it (today `-devflow`, `-devserver`, `-talk`), one branch each, removed with `git worktree remove` when the branch has merged (the merged-worktree rule `clean-space.sh` already applies). A worktree is the same repository: it shares `.git`, `.env` (ADR-012 D12.4) and, by D13.6, the deploy ledger; it has its own `target/`, venvs and `.cache/dev/`.
- **The `forma-app-platform` clone is retired, not deleted.** Order: (1) ADR-012 step 3 runs `import-legacy-priv.sh` once, copying `.cache/operations/beta/` into `$PRIV/windows/_pre-adr-012/`; (2) `git -C forma-app-platform fetch && git for-each-ref` reconfirms zero unique commits and a clean tree; (3) the folder is **moved** to `~/dev/archive/forma-app-platform-clone-2026-10-31/` [Victor's word]. No branch is deleted anywhere; every branch it has exists on `origin`. If the ADR-010 track resumes on Nov 3, it resumes as a worktree of the one clone on `phase-one-spikes`.
- **Renaming the clone to `forma-app-platform`** (its repository name) is deferred to after Nov 3: `git worktree repair` makes it cheap, but it moves three worktrees' paths during the pivot for no demo value.
- **`forma-pitch-mvp-talk`** stays until `pivot/from-data` merges or Victor abandons it; its 9 modified files are not touched.

Rejected: two clones (the inventory's cross-clone borrowing of venvs, `$PRIV` and scripts came from having two; one repository on disk makes D12.4's "no cross-checkout borrowing" structural); a bare repository plus worktrees only (no gain over the current main checkout, and every script and `.code-workspace` assumes a checkout exists).

### D13.2 Branch model: `pivot/pitch-mvp` is the trunk until Nov 3; `main` is frozen; one landing PR

- **Until the Nov 3 revisit**, `pivot/pitch-mvp` is the **integration and deploy branch**: a beta candidate is a pushed commit on it with a clean tree (`build-images.sh` already refuses otherwise). Topic branches are `pivot/<topic>`, merged with a merge commit (`--no-ff`, as today) after `just check` is green locally and the branch's CI is green or quarantined (D12.6). No pull request is required for `pivot/*` → `pivot/pitch-mvp` before Nov 3: one developer and agents, risk-tiered review as ADR-010 §4 (Tier A for auth, isolation and admission seams) recorded in the ticket, not in a PR.
- **`main` is frozen at `f229f01`** (the Phase 1 go-live record). Nothing deploys from it; nothing lands on it before Nov 3. **What `main` means after Nov 3: the beta-deployable trunk.** The proposal for APP-097 is **one landing PR**, `pivot/pitch-mvp` → `main`, reviewed as a whole with the APP-098 shortcut list attached, after which `pivot/pitch-mvp` is retired and topic branches become `<ticket>/<topic>` off `main`. This narrows ADR-011 §2's "ticket by ticket" landing to one PR, because 191 commits with 45 merges cannot be replayed ticket by ticket in the time the pivot leaves; **Victor decides this at the revisit**, and it is recorded here only as the recommendation.
- **Protection, now:** `main` and `pivot/pitch-mvp` get GitHub branch protection against **force-push and deletion only** (no required reviews, no required checks: CI is red on three workflows until ADR-012 step 5 lands and a required check would block every deploy). Adding required checks is an APP-097 item once `just ci` and CI agree. This is a GitHub settings change [Victor's word].
- **Tags:** after each successful activation (`just deploy activate` returns healthy, 6.1), the candidate gets an annotated tag `beta/<yyyymmdd>-<sha7>` pushed to `origin` (a Git mutation, not an AWS one; no approval gate). The tag is the human handle; the digests in `release.json` are the identity (D13.5). No semantic versions: the crate stays `0.1.0` until a production environment exists.

### D13.3 Modular monolith, confirmed; one root Cargo workspace; parser stays its own

**The product is a modular monolith and stays one**: one binary, one process, four logical roles with distinct database grants (APP-D05), one deploy unit. Nothing in the pitch or in ADR-010's remaining phases needs a second service; sharing (ADR-011 M3) and real time (APP-095) were added inside the process. Rejected: a service split (cost with no owner), a crate-per-module split now (it would freeze today's leaky boundaries into public APIs).

**A root Cargo workspace**, `Cargo.toml` at the repository root:

```toml
[workspace]
resolver = "3"
members = ["app", "spikes/APP-006"]
exclude = ["parser", "spikes/APP-001/minimal", "spikes/APP-003", "spikes/APP-004"]

[profile.dev]            # moved here from app/Cargo.toml (ADR-012 D12.4)
debug = "line-tables-only"
split-debuginfo = "packed"
[profile.dev.package."*"]
debug = false
```

- **Members:** `app` and `spikes/APP-006` (`genui-auth-verifier`), because `app` depends on it in release builds; the path dependency stays `../spikes/APP-006` so the three Dockerfiles' `COPY spikes/APP-006/…` lines keep working. Moving it to `crates/auth-verifier` is deferred (a rename, no behavior).
- **Excluded:** `parser` (ADR-009 D9.1: its own lockfile, toolchain file, vendored `zip` patch and release profile; the Dockerfiles build it as a separate stage with its own cache mount; a `[patch.crates-io]` cannot live in a non-root member anyway) and the frozen Rust spikes (evidence, built only by their own CI workflows; their `target/` directories are swept by `just clean target`). APP-005 is Python and unaffected.
- **Effect on ADR-012 D12.4:** Cargo honors `[profile.*]` **only in the workspace root manifest**, so the dev profile that step 1 wrote into `app/Cargo.toml` moves to the root `Cargo.toml`, and `app/.cargo/config.toml` (`[build] jobs = 3`) moves to `.cargo/config.toml` at the root (Cargo discovers it walking up from the working directory, so `cd app && cargo …` still reads it). `Cargo.lock` moves to the root (its package set is unchanged: APP-006 is already resolved in `app/Cargo.lock`). `target/` is the root `target/`, still per checkout. `rust-toolchain.toml` is already at the root by D12.4. `rust-analyzer.linkedProjects` becomes `Cargo.toml` and `parser/Cargo.toml`.
- **Effect on the Dockerfiles and CI:** `COPY app/Cargo.toml app/Cargo.lock ./` becomes `COPY Cargo.toml Cargo.lock ./` plus `app/Cargo.toml` and `spikes/APP-006/Cargo.toml` at their paths; the cache mount target is `/source/target`; the build is `cargo build --release --locked -p forma-app-platform` and the qualification image's `cargo test --no-run --locked --lib -p forma-app-platform`; `app/.dockerignore` and the two `Dockerfile.dockerignore` files admit the root manifests. CI `working-directory: app` steps drop the `working-directory` or pass `-p`. `qualify_image.py`, `check_image.py` and the parser-manifest equality check are the acceptance (D13.9 step C).

Rejected: a workspace that includes `parser` (D9.1, the patch section, its separate toolchain pin); no workspace (then `app/` and `spikes/APP-006` resolve twice, cargo-xtask stays impossible, and the profile fix has to be repeated per package).

### D13.4 Module map and dependency rules

Four layers, by what a module may `use` outside `cfg(test)`:

| Layer | Modules | May depend on |
| --- | --- | --- |
| 0 Foundation | `config`, `roles`, `readiness`, `supervise`, `origins`, `directory`, `demo`, `http` (request/response helpers only) | nothing in the crate |
| 1 Trust | `storage` (the only module that holds a Postgres connection), `auth`, `admission` | layer 0 |
| 2 Domain | `query`, `composer`, `actions`, `commands`, `boxes`, `sharing`, `widgets`, `imports`, `events` | layers 0–1 and each other (a cycle inside layer 2 is allowed for the MVP and listed) |
| 3 Surfaces | `pages`, `mcp`, `slack`, the canvas (`app/admin`) | layers 0–2, never each other (`slack` → `mcp` today is the one exception, kept: Slack reuses the agent-token path) |
| 4 Composition | `product`, `main`, `bin/genui-dev` (D12.1) | anything |

Rules: (1) nothing below layer 4 names `pages`, `product`, `main` or a `bin/`; (2) only `storage` speaks SQL (`tokio-postgres`, `deadpool`); only `auth` verifies a session or token; only `admission` admits a definition; (3) `cfg(test)` and `feature = "dev-stand"` code is exempt (campaigns reach everywhere by design); (4) the two known leaks, `storage` (non-test parts: `dev_stand.rs`, `pool.rs` depending on `pages`/`product`/`readiness`) and `http` → `boxes`/`composer`, are recorded as debt, not fixed now.

**How it is checked: deferred to APP-097.** The MVP ships the map and the rules; enforcement is a `just check deps` recipe (a 40-line Python walk over `use crate::` outside `#[cfg(test)]` blocks against an allowlist derived from this table; cargo-modules would be a new tool under AGENTS.md:13 for the same answer). It is scheduled after the dev binary lands (D12.1 step 4 moves `dev_stand.rs` out of `storage`, which removes most of the `storage` leak for free).

### D13.5 The AWS delivery ladder: local → CI → image build → ECR → beta; production later

```
 developer edit ─► just check ─► git push pivot/<topic> ─► GitHub Actions (11 workflows; amd64
                                                          docker build = compile check only)
        │                                                        │ green or quarantined (D12.6)
        │   git merge --no-ff ─► pivot/pitch-mvp  ◄──────────────┘
        │                              │ CANDIDATE = pushed sha, clean tree
        ▼                              ▼
   just dev (D12.1)            just images <sha>            [local, Mac, ARM64 native, no AWS]
   genui-dev + compose.dev      3 Dockerfiles → genui-service / -qualification / -operator
   Postgres (D12.2)             qualify_image.py, check_image.py ×2, parser manifests equal
                                       │ tag pitch-<sha7>-arm64
                                       ▼
                        just deploy new → observe → push [MUTATION] ──► ECR (3 repos, 3 digests)
                                       │ release.json: candidate, digests, window, supersedes
                                       ▼
                        preflight → synth → diff → change-set [M] → review → execute [M]
                        → snapshot [M] → migrate [M] → seed/library/grants [M] → activate [M]
                                       │                                ▲
                                       ▼                                │ rollback-deny [M]
                        beta.getforma.dev (Fargate ARM64, GenuiSyntheticFoundation)
                                       │ verify (4 curls) → tag beta/<date>-<sha7> → RELEASES.md
                                       ▼
                        production: does not exist (D13.7). Later: a second stack, same three
                        digests promoted from release.json, never rebuilt.

   [M] = needs Victor's verbatim word via --approved, appended to $PRIV/approvals.md first.
   getforma.dev (OVH, FormaDeploy): separate repository and pipeline, out of scope here.
```

- **Local → CI.** CI stays `on: push` for every branch and `pull_request` (a topic branch's CI is what makes the merge green). CI's `docker build` of `app/Dockerfile` on AMD64 stays a **compile check**: that artifact is never pushed and is a distinct image from the ARM64 one (`infra/README.md`, `ImageArchitecture`). Workflow runs on a superseded push are cancelled with a `concurrency` group per workflow and ref (one line each; cuts Actions minutes on 40 live branches).
- **Image build stays local and ARM64-only, for now.** Fargate runs ARM64 (2 vCPU/8 GiB, `infra/README.md`); the Mac builds ARM64 natively without QEMU; the repository is temporarily public, there is no OIDC role for Actions, and every push to ECR is a [MUTATION]. CI building and pushing images needs a role, a private repository and arm64 runners or QEMU, which is APP-097 work (D13.8). **No multi-arch OCI index**: the stack takes one digest per image, and ECR basic scanning skips indexes (runbook 2.3).
- **Three Dockerfiles stay three.** They are three trust boundaries (service without tests; qualification never behind the ALB; operator as the migration identity), each with its own `.dockerignore`. The duplicated parser stage is not deduplicated by `docker bake` or an `include` now; the parser-manifest digest equality check is the guard. Base image digests and the Debian security pins are bumped together, by a commit, never in place.
- **ECR → beta: digests only.** The stack never receives a tag. `release.json` (step 3's schema, fixed here) is the release record: `{candidate_sha, branch, built_at_utc, images: {service, qualification, operator: {tag, digest, size_bytes}}, parser_manifest_sha256, window_id, supersedes_window_id, database_version, activated_at_utc, verified: {live, canvas, mcp, slack}}`. It lives in `$PRIV/windows/<window-id>/` (private, 0700); `just deploy activate` also appends one non-secret line (date, sha7, window id, the three digests, schema version) to **`docs/pitch/RELEASES.md`**, committed on `pivot/pitch-mvp` with the tag of D13.2. That is the deploy-history the inventory said did not exist.
- **Rollback is unchanged** (runbook "Rollback"): `rollback-deny` is the fast one; a rollback **window** (previous digests from the previous `release.json`, new window id, `supersedes` the failing one) is the real one; after a committed migration, fix forward. `release.json` is what makes the rollback window composable without the other clone.
- **Approvals stay manual and per action**: `push`, `change-set`, `execute`, `snapshot`, `migrate`, `seed`, `library`, `grants`, `activate`, `rollback-deny` each refuse without `--approved "<Victor's words>"` (D12.6). Secrets (runbook §4) stay Victor's own commands. Nothing in this record automates an AWS mutation.

### D13.6 Private state and the single ledger

`$PRIV` is `<main checkout>/.cache/ops/beta/` (D12.6). A worktree **resolves `$PRIV` through `git rev-parse --git-common-dir`** to the main checkout's directory, so there is exactly one `budget-ledger.json`, one `approvals.md` and one `windows/` tree per repository on this machine: the preflight's exclusive lock and `supersedes` chain assume a single ledger (`infra/README.md`, "Bounded window and cumulative budget preflight"), and two would double-count or lose a window. `ops/beta/lib.sh` implements the resolution; `just deploy` prints which `$PRIV` it uses before every step. Dev state (`.cache/dev/`, `.cache/reports/`, `.cache/images/`, `.cache/parser-target/`) stays per checkout as D12.7 says.

### D13.7 Production: a second stack, later, promoted by digest

Production does not exist and is **not created before the Nov 3 revisit**. When it is: a second CloudFormation stack (`GenuiProduction`) from the same `infra/` code with its own RDS, secrets, KMS keys and hostname; it receives the **three digests of a beta `release.json` that passed `verify`**, never a rebuild; its own `$PRIV/production/` ledger and window chain; the same `--approved` gate. First in the same AWS account (`877573132739`) as a separate stack, so the one account's budget ledger sees both; a **separate account** before the first paying customer (the honesty obligation of the concept-car amendment applies at onboarding). The `MonthlyBudgetUsd` decision for two environments is Victor's, at APP-097, against ADR-003/ADR-008.

### D13.8 The workstation

Target state under `~/dev`:

```
~/dev/FormaBuiltApps/forma-pitch-mvp/            the one clone (pivot/pitch-mvp); $PRIV in .cache/ops/beta/
~/dev/FormaBuiltApps/forma-pitch-mvp-<lane>/     worktrees, one branch each, removed when merged
~/dev/FormaBuiltApps/{gatewasm,forma-vector-studio,forma-genui-runtime,…}/   donor and sibling apps, untouched
~/dev/Forma/                                     the framework (FormaStack; separate owner; route changes there)
~/dev/FormaDeploy/                               getforma.dev site + infra (out of scope)
~/dev/archive/forma-app-platform-clone-2026-10-31/   the retired clone (D13.1), read-only history
~/dev/tools/, ~/dev/Forma/work/_maintenance/     machine-wide scripts (clean-space.sh; D12.7 rule 4)
```

- **VS Code:** `forma-pitch-mvp.code-workspace` (D12.5) and `FormaDeploy.code-workspace`; no third. The optional sibling root `../forma-app-platform` in D12.5 is dropped once the clone is archived.
- **Archive / remove list** (nothing deleted; each item needs Victor's word):
  1. move `forma-app-platform` to `~/dev/archive/…` after D13.9 step B;
  2. `git worktree remove forma-pitch-mvp-talk` only after `pivot/from-data` merges or Victor abandons its 9 modified files;
  3. the 15 `worktree-agent-*` branches travel with the archived clone; they are not deleted from it;
  4. the `pivot/*` topic branches already merged into `pivot/pitch-mvp` stay on `origin` until the landing PR (D13.2); pruning them is an APP-097 item.
- **Secrets** stay in `<main checkout>/.env` (gitignored; read only by `read_env.py`), never in a worktree's copy, a scratchpad or a `code-workspace`.

### D13.9 Migration plan, interleaved with ADR-012

ADR-012 steps 1–2 are committed on `pivot/devflow` (`3679264`, `026f9ff`); step 3 is uncommitted there; step 4 starts on `pivot/devserver`; steps 5–6 are pending. ADR-013's steps slot in as follows; each is one commit or one approved action, independently useful.

| # | When | What | Done when |
| --- | --- | --- | --- |
| A | now, no code | Branch protection on `main` and `pivot/pitch-mvp`: no force-push, no deletion [Victor's word, GitHub settings]. | `gh api …/protection` returns the two rules. |
| B | inside ADR-012 step 3 (on `pivot/devflow`) | `$PRIV` via `git-common-dir` (D13.6); `release.json` fields fixed (D13.5); `just deploy activate` appends `docs/pitch/RELEASES.md` and prints the `git tag` command; `import-legacy-priv.sh` run once from the other clone. | Step 3's own acceptance, plus `just deploy help` from a worktree names the main checkout's `$PRIV`; `windows/_pre-adr-012/` holds the 564 legacy files. |
| C | after step 1 lands on `pivot/pitch-mvp`, before the next `just images` | **Root Cargo workspace** (D13.3): root `Cargo.toml` with members/exclude and the dev profile, `.cargo/config.toml` moved up, `Cargo.lock` at the root, the three Dockerfiles and ignore files updated, CI `working-directory` lines, `rust-analyzer.linkedProjects`, `scripts/dev/*.sh` cargo calls with `-p forma-app-platform`. One commit, no behavior change. | `cargo build --locked` and `cargo test --locked --lib -p forma-app-platform` at the root; `just images <sha>` passes the three checks and the parser manifests are equal; `du target` after a clean build is within D12.4 step 1's bound. |
| D | with ADR-012 step 4 (`pivot/devserver`) | `compose.dev.yaml` is Postgres only (no app container; the Docker VM disk is the incident). `just clean docker` adds `docker builder prune --filter until=72h` and keeps the last three local `pitch-*` image sets. Dev binary is `cargo run -p forma-app-platform --features dev-stand --bin genui-dev` (after C). | Step 4's acceptance; `docker system df` stays flat across three image builds. |
| E | ADR-012 step 5 | `concurrency` groups in the 11 workflows; the CI policy of D12.6 applied. | Superseded runs show cancelled; branch CI green or quarantined. |
| F | ADR-012 step 6 | `docs/DEVELOPMENT.md` gains the ladder diagram and the branch model; `DEPLOY-RUNBOOK.md` gains `RELEASES.md` and the tag step after 6.2; `AGENTS.md` gains one line: "`pivot/pitch-mvp` is the trunk until the Nov 3 landing; see ADR-013". | The docs name no command that is not a recipe. |
| G | after B, with Victor's word | Archive the second clone (D13.1); drop the `../forma-app-platform` root from the workspace file. | `~/dev/FormaBuiltApps` holds the clone, its worktrees and the donor apps only. |
| H | the next beta window | First deploy entirely through `just deploy`, producing `windows/<id>/release.json`, the `RELEASES.md` line and the `beta/<date>-<sha7>` tag. | The window directory and the tag exist; the runbook was followed without a step outside Git. |

Order of landing on `pivot/pitch-mvp`: ADR-012 step 1 → step 2 → step 3 (with B) → **C** → step 4 (with D) → step 5 (with E) → step 6 (with F); G and H when their preconditions hold. C sits between 3 and 4 because step 4's new binary target and `process-compose.yaml` should be written against the root workspace once, not rewritten.

## Deferred (to the Nov 3 revisit, APP-097, unless noted)

- The landing PR `pivot/pitch-mvp` → `main` and `main` becoming the trunk (D13.2); required status checks on `main`; pruning merged `pivot/*` branches.
- CI builds and pushes images: OIDC role, private repository again (README.md:3), arm64 runners or QEMU; CI-built digests as beta candidates.
- The production stack, its account and budget (D13.7).
- `just check deps` enforcement of D13.4; fixing the `storage` and `http` leaks; `crates/auth-verifier` rename; a crate split if a second deploy unit ever appears.
- Renaming the clone to `forma-app-platform`; `docker bake` or a shared parser stage; semantic versions.
- `build.build-dir`, sccache, mise (ADR-012's own deferrals, unchanged).

## Consequences

- One repository on disk, so the "borrow from the other clone" class of incident cannot recur; the second clone's only value (the legacy `$PRIV`) is imported once and the clone is archived with its history intact.
- `pivot/pitch-mvp` is named for what it already is (the trunk and the deploy branch) and protected against the two irreversible mistakes; `main`'s meaning after Nov 3 is proposed, not decided, and the landing stays Victor's call per ADR-011 §6.
- The product is confirmed a modular monolith with a written module map; the root workspace makes the ADR-012 profile fix apply once and lets the dev binary, the spike dependency and future tooling (xtask, a deps check) share one build graph, while the parser keeps the isolation ADR-009 requires. Cost: one mechanical commit (C) touching the three Dockerfiles, which the existing image checks verify.
- The delivery ladder is explicit end to end: every artifact that reaches AWS is an immutable digest named in a `release.json`, every deploy leaves a tag and a line in `RELEASES.md`, and rollback is composable from the previous record. Nothing is automated past the approval gate; CI still cannot touch AWS.
- Production is deliberately absent until the revisit, and promoted by digest when it comes, so the beta build is the production build.
- Debt acknowledged: images are built on one laptop; module boundaries are documented but not enforced; `main` is stale for three more days; the Slack → MCP surface dependency is kept.
