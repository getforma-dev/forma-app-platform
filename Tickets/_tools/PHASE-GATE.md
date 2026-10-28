# Phase gate record template

Copy this template to `evidence/phases/PHxx/GATE.md` when reviewing an implemented phase. Do not create a passing record during planning.

- Phase:
- Review date:
- Candidate commit/build/image and dependency pins:
- Test environment and workload/input identities:
- GenUI implementation owner:
- Independent reviewer:
- Decision: Pending / Changes required / Accepted

## Design and acceptance evidence

| Requirement / phase exit criterion | Owning ticket | Test or trial | Expected result | Actual result | Evidence |
|---|---|---|---|---|---|
| Fill from the phase plan | | | | | |

Include relevant success, denial, error, race, restart, accessibility and resource-bound cases. Record actual commands or reproducible manual steps and durations. Preserve failures and rerun evidence after fixes.

## Integrated and release trials

Name the phase's demonstration, upgrade/rollback, restore or load trial and exact candidate identity. Mark a trial not applicable only with the design-based reason and its later owner; do not waive a required phase criterion.

## FormaStack handoffs

Record upstream ticket, consumable artifact/deployment identity, upstream test evidence and GenUI's integration result. An assignment or new version number is insufficient.

## Limits and unresolved findings

Record failed checks, unmeasured targets, known limitations and any explicitly accepted design amendment. Hard-gate failures require Changes required.

## Acceptance and next permitted step

Record the reviewer conclusion, evidence that all phase tickets and predecessor gates are complete, and the accepted next step. For PH06, preserve the separate pre-customer live-data decision and subsequent learning outcome. Only then mark the phase gate passed in `Tickets/plan.json`, link this record and regenerate the board.
