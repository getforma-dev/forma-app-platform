# Forma app platform

Teams describe, run, adapt and share apps on a canvas, with governed data, receipts and explicit audited commands. This repository owns the application built on Forma Stack: Rust/FMIR SSR, registered canvas islands, PostgreSQL and confined query/parser children.

**Start with [DEVELOPMENT.md](docs/DEVELOPMENT.md)** for setup, local instances, checks, the module map and the path from a change to beta. The trunk is `main`. A reviewed merge prepares the beta deploy and waits for one approval ([ADR-019](docs/architecture/ADR-019-MAIN-TRUNK-AND-RESERVATION-CAP.md)).

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

`main` is the trunk. A reviewed merge prepares one beta packet and deploys only after one approval ([ADR-019](docs/architecture/ADR-019-MAIN-TRUNK-AND-RESERVATION-CAP.md)). The sealed release is `beta/20261104-1e28488`. The app database is schema 20 on PostgreSQL 16.15, with automatic minor upgrades off. There is no production environment. The `getforma.dev` website on OVH is separate.

The next product slices are [APP-163](Tickets/0-backlog/APP-163.md) verified-email discovery, [APP-164](Tickets/0-backlog/APP-164.md) authenticated workspace switching, and [APP-165](Tickets/0-backlog/APP-165.md) workspace-aware invitation delivery. They follow [ADR-016](docs/architecture/ADR-016-WORKSPACE-SIGNIN.md). Each stays in its current status folder until its predecessors are Done.

The pitch trunk through PR #49 remains `pivot/pitch-mvp`. The seeded demo stays on `showcase/spend-and-jacket` and is not deployed. Older release notes, including the 2 November workspace-entry record, stay in [docs/planning](docs/planning) and [docs/pitch/RELEASES.md](docs/pitch/RELEASES.md).

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
