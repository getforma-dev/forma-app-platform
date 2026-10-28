# Forma app platform

The company app platform built on Forma Stack: teams create, run, adapt and share their own apps. Northstar and Spend Review prove the same platform through two different workflows.

**Phase 1 is underway — October 28, 2026.** APP-001–005 are complete: published Forma **3.2.0**, locked data contracts, and native canvas/parser/query qualifications. The repository also contains reviewed local authentication and application/CDK preparation. The [live board](Tickets/TRACKER.md) records actual review/completion state. Hosted authentication, managed tenant storage and the integrated saved workflow remain gated; local preparation does not establish those capabilities.

## Start here

1. [Six-phase roadmap](docs/planning/ROADMAP.md), [phase board](Tickets/PHASES.md), and [quality gates](docs/planning/QUALITY-GATES.md)
2. [Ticket board](Tickets/TRACKER.md), [sprints](Tickets/SPRINTS.md), [categories](Tickets/CATEGORIES.md), and [FormaStack blockers](Tickets/BLOCKERS.md)
3. [First session](docs/planning/START-HERE.md)
4. [Design source guide](docs/planning/SOURCE-GUIDE.md) and [framework baseline](docs/architecture/FRAMEWORK-BASELINE.md)
5. [Existing related work](docs/planning/EXISTING-WORK.md) and [open decisions](docs/planning/DECISIONS-AND-RISKS.md)
6. [Phase 2 handoff checklist](docs/planning/PHASE-2-HANDOFF.md) — pending full Phase 1 acceptance

The ticket Markdown files in `Tickets/` are the source of truth. Each of the 72 tickets belongs to one of six phases. Tickets have dependencies, scope, acceptance checks and verification evidence; each phase also requires a reviewed completion gate. `APP-NNN` identifies this application's work; existing `FORMA-NNN` tickets retain their upstream owners. GitHub hosts the private repository; there is no duplicate GitHub Issues board.

## Workspace map

| Folder | Purpose |
|---|---|
| `design/` | Frozen platform and Northstar packets, PDFs, diagrams, source material and reviews; checksummed provenance |
| `Tickets/` | Familiar status folders, ordered tracker, phase/sprint/category views and requirement coverage |
| `docs/planning/` | Roadmap, pickup instructions, dependencies, decisions and verification gates |
| `docs/architecture/` | Current implementation baseline and design reconciliation |
| `contracts/` | Locked schema/encoding edition, synthetic fixtures and cross-language vectors |
| `spikes/` | Reproducible released-baseline, WASM parser and confined query qualifications |
| `app/` | Minimal native SSR Rust/browser scaffold, role configuration, migration and managed-check helpers |
| `infra/` | Pinned TypeScript AWS CDK for the isolated GenUI synthetic environment |
| `fixtures/` | Entry point for later integrated synthetic acceptance inputs |
| `evidence/` | Actual checks, failures, reviews, source identities and operational records |
| `.github/` | Ticket-specific qualification CI and pull request template |

No GenUI AWS resources or model integration have been deployed. The [environment plan](docs/planning/SYNTHETIC-ENVIRONMENT.md) and [cost estimate](docs/planning/SYNTHETIC-ENVIRONMENT-COST.md) identify the remaining deployment inputs. At Victor’s separate request, unused GateWasm application resources were [torn down](evidence/APP-007/gatewasm-teardown.md); the [FormaStack handoff](docs/planning/CROSS-TEAM-HANDOFF.md) records rebuild prerequisites and team boundaries.

## Check the preparation

Python 3.10+ is sufficient for board checks. Runtime/qualification toolchains and exact commands are documented in each ticket’s owning directory; board checks do not replace them.

```sh
python3 Tickets/_tools/board.py check
python3 Tickets/_tools/board.py next
```

After editing or moving a ticket, run `python3 Tickets/_tools/board.py render`, then `check`. See [contributing](CONTRIBUTING.md). Capacity and start dates will be set when implementation starts; sprint numbers are an order, not delivery promises.
