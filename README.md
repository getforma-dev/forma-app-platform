# Forma app platform

The company app platform built on Forma Stack: teams create, run, adapt and share their own apps. Northstar and Spend Review prove the same platform through two different workflows.

**Preparation checkpoint — October 28, 2026.** This repository contains the delivered design, its companion demo, and the ordered implementation board. Application implementation has not started. The intended baseline is published Forma **3.2.0** with typed Rust/FMIR rendering and a native canvas island.

## Start here

1. [Roadmap and stage gates](docs/planning/ROADMAP.md)
2. [Ticket board](Tickets/TRACKER.md), [sprints](Tickets/SPRINTS.md), [categories](Tickets/CATEGORIES.md), and [FormaStack blockers](Tickets/BLOCKERS.md)
3. [First session](docs/planning/START-HERE.md)
4. [Design source guide](docs/planning/SOURCE-GUIDE.md) and [framework baseline](docs/architecture/FRAMEWORK-BASELINE.md)
5. [Existing related work](docs/planning/EXISTING-WORK.md) and [open decisions](docs/planning/DECISIONS-AND-RISKS.md)

The ticket Markdown files in `Tickets/` are the source of truth. Each ticket has dependencies, scope, acceptance checks and verification evidence. `APP-NNN` identifies this application's work; existing `FORMA-NNN` tickets retain their upstream owners. GitHub hosts the private repository; there is no duplicate GitHub Issues board.

## Workspace map

| Folder | Purpose |
|---|---|
| `design/` | Frozen platform and Northstar packets, PDFs, diagrams, source material and reviews; checksummed provenance |
| `Tickets/` | Familiar status folders, ordered tracker, sprint/category views and requirement coverage |
| `docs/planning/` | Roadmap, pickup instructions, dependencies, decisions and gates |
| `docs/architecture/` | Current implementation baseline and design reconciliation |
| `fixtures/` | Entry point for synthetic acceptance inputs to be materialized in the foundation sprint |
| `evidence/` | Preparation and future ticket verification records |
| `.github/` | Planning checks and pull request template |

No paid infrastructure, live organization data, application scaffold or model integration is provisioned by this checkpoint. Those actions have explicit tickets and gates.

## Check the preparation

Python 3.10+ is sufficient; no application packages are needed yet.

```sh
python3 Tickets/_tools/board.py check
python3 Tickets/_tools/board.py next
```

After editing or moving a ticket, run `python3 Tickets/_tools/board.py render`, then `check`. See [contributing](CONTRIBUTING.md). Capacity and start dates will be set when implementation starts; sprint numbers are an order, not delivery promises.
