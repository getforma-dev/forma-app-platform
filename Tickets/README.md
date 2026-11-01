# Forma app platform work board

This is the implementation board for Forma's company app platform and its Northstar/Spend Review proof. Phase 1 implementation has started with APP-001’s isolated baseline harness. The current status folders and evidence records distinguish completed qualification from planned application behavior; upstream consumers remain explicitly blocked until accepted evidence arrives. Creating the original board did not run application acceptance or deploy a service.

## Start here

1. Read the repository [contribution guide](../CONTRIBUTING.md) and the [source guide](../docs/planning/SOURCE-GUIDE.md).
2. Read the [phase plan](PHASES.md) and [quality gates](../docs/planning/QUALITY-GATES.md), then the [sprint plan](SPRINTS.md) for smaller outcomes and the [ordered tracker](TRACKER.md) for individual tickets. [Categories](CATEGORIES.md) group responsibility; they do not create separate copies of a ticket.
3. For the November 1 clean-user pivot, use [DIRECTION.md](../docs/planning/DIRECTION.md) and the [review](../docs/planning/PIVOT-REVIEW-2026-11-01.md). `board.py next` follows the older dependency queue and does not select this work. Existing dependency and Done gates still apply; implementation on an open PR is recorded in the ticket log without declaring it Done.
4. Use [IMPLEMENT](_tools/IMPLEMENT.md) for execution and [REVIEW](_tools/REVIEW.md) for independent review.

The current product direction is the app as a page in the clean-user trial. Victor’s later sharing clarification prioritizes [APP-154](0-backlog/APP-154.md), private live app guests and connected canvas placement, before APP-139’s integrated trial. [APP-145](0-backlog/APP-145.md) enables the first compose; then follow APP-141, APP-142, APP-144, APP-143, APP-139 and APP-140. The record is [DIRECTION.md](../docs/planning/DIRECTION.md). `board.py next` still lists the older queue. It does not choose this work.

The subsequent screenshot-driven [app composition decision](../docs/architecture/ADR-015-COMPOSED-APP-PRESENTATION.md) adds APP-155–161 in PH09/S15: coherent app presentation, component actions, persisted themes, real record dashboards, integrated qualification and a future sleep/wake spike. These remain Backlog and add no November 2 gate. The genuine APP-139/140 trial remains first.

Sprints are ordered scope batches, not calendar commitments. Start dates and capacity are unset. Team ownership is assigned; individual implementers are selected when work starts. Later sprints are forecasts; refine or split a ticket before selection if it cannot fit a small, reviewable change. The GenUI and FormaStack teams work in parallel when actual dependencies permit it. An upstream integration gate blocks only the dependent behavior; continue unrelated app work and record precisely what evidence is needed to unblock integration.

## One authoritative ticket

Each `APP-NNN.md` lives in exactly one status folder. Its Markdown fields, acceptance checklist and log are authoritative. `plan.json` defines phase gates, category and sprint metadata; `external-dependencies.json` records upstream gate evidence. `TRACKER.md`, `PHASES.md`, `SPRINTS.md`, phase/category/sprint pages, `COVERAGE.md`, `BLOCKERS.md` and `index.json` are derived views. Edit the source ticket or register, then regenerate; do not hand-edit those views.

GitHub hosts source and pull requests. It does not hold a second issue board. The **GenUI team** owns this app and its `APP-NNN` implementation tickets. The **FormaStack team** owns the upstream blockers, including coordination of framework and fAuth work, under their existing `FORMA-NNN` identities. The GenUI team implements app integration and verifies the required upstream evidence; it does not duplicate those fixes. [Upstream snapshots](evidence/upstream/README.md) preserve evidence only; never move or close them as app tickets.

| Folder | Status | Meaning |
| --- | --- | --- |
| `0-backlog/` | Backlog | Planned work, including normal waiting for predecessors. |
| `1-selected/` | Selected | Ready, dependencies evidenced, scope understood and owner assigned. |
| `2-in-progress/` | In progress | Implementation underway with branch/work context recorded. |
| `3-in-review/` | In review | Concrete change and evidence available for independent review; record current CI state. |
| `4-done/` | Done | Acceptance and review complete; landed in the agreed source, with any ticket-specific release/deployment condition met. |
| `9-blocked/` | Blocked | Progress cannot continue because of a concrete missing decision, access, service or other operational condition. |

**Blocked does not automatically mean Victor must act.** Record the blocker, responsible team/person/service, attempted resolution and measurable unblock criterion. Ordinary dependency waiting stays Backlog. Only an actual unresolved product decision is labeled as needing Victor; an unavailable environment or upstream fix is an operational blocker.

Moving a ticket means moving the file, updating its `Status` field and dated `Log`, repairing incoming local links, and regenerating the views. Preserve earlier failures and decisions. Never mark acceptance checks passed solely because a related ticket or framework release completed.

## Board commands

Run from the repository root; Python 3's standard library is sufficient.

```sh
python3 Tickets/_tools/board.py render
python3 Tickets/_tools/board.py check
python3 Tickets/_tools/board.py next
```

`render` validates ticket structure, phase membership and completion-gate prerequisites, then regenerates indexes. `check` also refuses stale indexes and checks design digests and active local links. `next` lists Backlog tickets whose local APP predecessors are Done; **external dependencies still require inspection**. These are planning checks, not application tests.

[Requirement coverage](COVERAGE.md) is a map from design obligations to planned work. Coverage links are not evidence that the behavior exists or passes.

## Phase acceptance

Each ticket and sprint belongs to exactly one phase in `plan.json` (currently PH01–PH09). A phase is complete only when all its tickets are Done, predecessor gates have passed, and its own gate has reviewed evidence. Use the [gate record](_tools/PHASE-GATE.md) and the phase page. Pending upstream gates still block only their consumers; eligible independent tickets may proceed. The board validates evidence links and status relationships, while the reviewer judges whether the tests satisfy the design.
