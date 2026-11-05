# Design

## What this is

Frozen copies of the delivered platform and Northstar design packets. `snapshot-manifest.json` records the bytes. `board.py` checks those digests.

## Authoritative sources

- [Source guide](../docs/planning/SOURCE-GUIDE.md) for reading order.
- `snapshot-manifest.json` for the captured files.
- Later decisions in [docs/architecture](../docs/architecture) when they explicitly amend a design assumption.

## Verify a change here

Do not change these files. `just check board` refuses a digest mismatch.

## Never

Do not edit, reformat or rename anything under this folder. Do not treat a design sentence as proof the application does it. Do not search `deliverables/` for current guidance. Open a path from the source guide, a ticket or an ADR.

## Read next

- [Source guide](../docs/planning/SOURCE-GUIDE.md)
- [Implementation decisions](../docs/architecture/DECISIONS.md)
- [Docs index](../docs/README.md)
