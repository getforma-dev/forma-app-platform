# Forma app platform

Teams describe, run, adapt and share apps on a canvas, with governed data, receipts and explicit audited commands. This repository owns the application built on Forma Stack: Rust/FMIR SSR, registered canvas islands, PostgreSQL and confined query/parser children.

**Start with [DEVELOPMENT.md](docs/DEVELOPMENT.md)** for setup, local instances, checks, the module map and the path from a change to beta. The current trunk is `pivot/pitch-mvp`; `main` stays at the Phase 1 go-live record until the November 3 revisit ([ADR-013](docs/architecture/ADR-013-REPOSITORY-AND-DELIVERY.md)).

**License:** All rights reserved; see [LICENSE](LICENSE). The repository was made temporarily public for GitHub Actions during the deployment work. That grants no permission to use, copy or modify it; returning it to private remains part of the delivery follow-up.

## Run locally

From the checkout root, with the tools in [tools.versions](tools.versions) installed:

```sh
just setup
just doctor
just dev -d
```

Pitch personas are [Diego on 4001](http://127.0.0.1:4001/canvas), [Sarah on 4002](http://127.0.0.1:4002/canvas) and [Priya on 4003](http://127.0.0.1:4003/canvas) in the main checkout. `just dev clean -d` starts [Alex on 4010](http://127.0.0.1:4010/canvas) as a new customer. A worktree receives its own ports; `just status` prints them. Both profiles keep their data across restarts. See the developer guide before resetting or cleaning an instance.

## Current work and delivery

The November 2 pitch follows [ADR-011](docs/architecture/ADR-011-NOV-2-EMERGENCY-PIVOT.md). Local code includes app creation, Ask/Shape, made cards and pins, imports, sharing and live updates. The [product guide](docs/pitch/GUIDE.md), [canvas rules](docs/pitch/CANVAS-RULES.md), [pitch script](docs/pitch/PITCH-SCRIPT.md) and [recording checklist](docs/pitch/RECORDING.md) describe their behavior and demo limits.

Beta and local source are separate states. The latest **recorded** beta deploy is `3901992`, TaskDefinition 12, on October 31; [RELEASES.md](docs/pitch/RELEASES.md) is the history, not a fresh AWS observation. Later local changes are not deployed just because they are merged. Every AWS mutation follows the [deploy runbook](docs/pitch/DEPLOY-RUNBOOK.md) and Victor's approval for that action. A production environment does not yet exist. The `getforma.dev` website on OVH is a separate repository and pipeline.

The [October 31 continuation record](docs/planning/CONTINUATION-2026-10-31.md) records remaining technical work. [APP-007 foundation evidence](evidence/APP-007/managed/README.md) preserves an earlier stage; its stopped-service and incomplete-runtime observations are historical, not the current operating instructions. The pitch does not close the unfinished [Phase 1 gate](evidence/phases/PH01/GATE.md) or later platform obligations.

## Find the owner

| Area | Start here |
| --- | --- |
| Setup, commands, state, agents and delivery | [DEVELOPMENT.md](docs/DEVELOPMENT.md) |
| Rust modules and dependency debt | [MODULE-MAP.md](docs/architecture/MODULE-MAP.md) |
| Work and acceptance | [Ticket board](Tickets/TRACKER.md), [board guide](Tickets/README.md), [phase gates](docs/planning/QUALITY-GATES.md) |
| Architecture and design precedence | [ADRs](docs/architecture), [source guide](docs/planning/SOURCE-GUIDE.md), [framework baseline](docs/architecture/FRAMEWORK-BASELINE.md) |
| Framework/fAuth integration | [Cross-team handoff](docs/planning/CROSS-TEAM-HANDOFF.md), [upstream blockers](Tickets/BLOCKERS.md) |
| Deferred platform work | [APP-097](Tickets/0-backlog/APP-097.md), [roadmap](docs/planning/ROADMAP.md), [phase board](Tickets/PHASES.md) |

`app/` owns the application, `parser/` the separately built parser, `infra/` and `ops/beta/` delivery, `contracts/` the locked schemas and vectors, and `scripts/dev/` the local recipes. `design/` and `evidence/` retain source history and measured results; they are not disposable scratch. The root Cargo workspace includes `app` and the auth verifier in `spikes/APP-006`; builds go to root `target/`.

## Check a change

```sh
just check
just campaigns
just campaign command
```

The developer guide explains focused checks, `just ci` and the remote workflows. Ticket Markdown under status folders is authoritative; there is no duplicate GitHub Issues board. After a ticket change, regenerate and validate its derived views:

```sh
python3 Tickets/_tools/board.py render
python3 Tickets/_tools/board.py check
```

Board checks validate planning, links and frozen design hashes; they do not prove application acceptance. See [CONTRIBUTING.md](CONTRIBUTING.md) for the review and evidence rules.
