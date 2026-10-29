# Forma app platform

**October 29 transfer:** start with [the GenUI Claude handoff](CLAUDE-HANDOFF.md). Execution is being wound down at Victor's request; Phase1 is incomplete and the proposed hosted-auth window was withdrawn. The handoff identifies the separate FormaStack/fAuth owner and the unaccepted managed-entry draft.

The company app platform built on Forma Stack: teams create, run, adapt and share their own apps. Northstar and Spend Review prove the same platform through two different workflows.

**Phase 1 is underway — October 28, 2026.** APP-001–005 and APP-007 are complete: published Forma **3.2.0**, locked data contracts, and native canvas/parser/query qualifications. APP-007 has retained [managed foundation evidence](evidence/APP-007/managed/README.md): real RDS migration and role denials, two-tenant S3/KMS isolation, TLS, credential refresh and negative-path refusals, snapshot restore, scheduled service stop and recovery of the qualified initial baseline. The final inventory records every observed task stopped. Independent acceptance and all eight [final source CI checks](evidence/APP-007/managed/ci.json) pass at `b925600`, landed on main. The [live board](Tickets/TRACKER.md) records actual review/completion state. Hosted authentication, application tenant/RLS authorization, invite admission and the integrated saved workflow remain open.

## Start here

1. [Six-phase roadmap](docs/planning/ROADMAP.md), [phase board](Tickets/PHASES.md), and [quality gates](docs/planning/QUALITY-GATES.md)
2. [Ticket board](Tickets/TRACKER.md), [sprints](Tickets/SPRINTS.md), [categories](Tickets/CATEGORIES.md), and [FormaStack blockers](Tickets/BLOCKERS.md)
3. [First session](docs/planning/START-HERE.md)
4. [Design source guide](docs/planning/SOURCE-GUIDE.md) and [framework baseline](docs/architecture/FRAMEWORK-BASELINE.md)
5. [Existing related work](docs/planning/EXISTING-WORK.md) and [open decisions](docs/planning/DECISIONS-AND-RISKS.md)
6. [Phase 2 handoff checklist](docs/planning/PHASE-2-HANDOFF.md) — pending full Phase 1 acceptance

The ticket Markdown files in `Tickets/` are the source of truth. Each of the 74 tickets belongs to one of six phases. Tickets have dependencies, scope, acceptance checks and verification evidence; each phase also requires a reviewed completion gate. `APP-NNN` identifies this application's work; existing `FORMA-NNN` tickets retain their upstream owners. GitHub hosts the private repository; there is no duplicate GitHub Issues board.

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

The bounded synthetic environment has been deployed for APP-007 trials at `beta.getforma.dev`. Its [observed configuration](evidence/APP-007/managed/configuration-inventory.json) records private encrypted PostgreSQL 16.13, separate role/key identities and the 2-vCPU/8-GiB ARM64 task configuration. [Public-surface checks](evidence/APP-007/managed/public-surface.json) observed inert pages/readiness and refused representative unavailable application/auth routes; they do not establish signup or application access. The [final operating receipt](evidence/APP-007/managed/final-operating-state.json) records zero service tasks and all ten observed tasks stopped at 01:04 UTC on October 29. Retained RDS/ALB/key/storage resources remain billable; the service stays stopped until another reviewed activation. The [environment plan](docs/planning/SYNTHETIC-ENVIRONMENT.md) and [cost record](docs/planning/SYNTHETIC-ENVIRONMENT-COST.md) capture the $100 monthly project limit and $97.49 reservation, not an invoice or hard billing cap. At Victor’s separate request, unused GateWasm application resources were [torn down](evidence/APP-007/gatewasm-teardown.md); the [FormaStack handoff](docs/planning/CROSS-TEAM-HANDOFF.md) records rebuild prerequisites and team boundaries.

## Check the preparation

Python 3.10+ is sufficient for board checks. Runtime/qualification toolchains and exact commands are documented in each ticket’s owning directory; board checks do not replace them.

```sh
python3 Tickets/_tools/board.py check
python3 Tickets/_tools/board.py next
```

After editing or moving a ticket, run `python3 Tickets/_tools/board.py render`, then `check`. See [contributing](CONTRIBUTING.md). Capacity and start dates will be set when implementation starts; sprint numbers are an order, not delivery promises.
