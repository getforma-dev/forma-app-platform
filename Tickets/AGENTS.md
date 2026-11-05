# Tickets

## What this is

The work board. One `APP-NNN.md` file lives in one status folder. That file is the ticket. `plan.json` defines phases, sprints and categories.

## Authoritative sources

- [README.md](README.md) for status folders and commands.
- The ticket file in `0-backlog/` through `4-done/` or `9-blocked/`.
- [Quality gates](../docs/planning/QUALITY-GATES.md) for what Done requires.

## Verify a change here

After a ticket edit, run `python3 Tickets/_tools/board.py render`, then `just check board`.

## Never

Do not hand-edit `TRACKER.md`, `PHASES.md`, `SPRINTS.md`, `CATEGORIES.md`, `COVERAGE.md`, `BLOCKERS.md`, `index.json`, or the pages under `phases/`, `sprints/` and `categories/`. Do not edit `evidence/upstream*` or mark a gate passed from a plan. `board.py next` is the old dependency queue, not the product order in [DIRECTION.md](../docs/planning/DIRECTION.md).

## Read next

- [README.md](README.md)
- [Tracker](TRACKER.md)
- [Direction](../docs/planning/DIRECTION.md)
- [Developer guide](../docs/DEVELOPMENT.md)
