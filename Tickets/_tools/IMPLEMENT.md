# Working an application ticket

Work one small ticket through implementation, evidence and review. Read the current user request, repository instructions, [contribution guide](../../CONTRIBUTING.md), [board guide](../README.md), the ticket and its cited design sections. The repository's initial preparation leaves implementation in Backlog or explicitly Blocked; begin implementation when the user starts that work.

## Select and establish scope

- Run `python3 Tickets/_tools/board.py next`. Confirm local predecessors are Done and inspect external dependency evidence. A framework release or historical snapshot is not proof of app acceptance. FormaStack owns upstream blockers; GenUI owns this application and its integration acceptance. Link the exact upstream ticket and required result, and continue independent app work in parallel.
- Confirm the ticket's outcome, negative cases, verification and rollback. Split work that is too large before starting; retain traceability to its parent requirement.
- Move the ready ticket to Selected, update its field/log and assign the GenUI team and the person doing the work. Move to In progress once work starts; record the branch or work context.
- Inspect the checkout and its existing changes. Reuse a suitable checkout or worktree; isolate conflicting work when necessary. A fresh clone is not a routine requirement. Preserve changes belonging to others.

## Implement and prove the outcome

1. Inspect the smallest owning source surface. Keep the change within the ticket and use its declared rendering model and pinned dependencies.
2. For a defect, reproduce the failure and retain a focused regression check. For new behavior, verify the acceptance criteria at the level that can actually observe them. Avoid tests that only restate the implementation or unnecessary tests for low-impact documentation edits.
3. Exercise applicable failure and recovery paths: authorization, malformed input, cancellation, duplicate commands, stale versions and unavailable services, as specified by the ticket. Do not convert a failure into a pass by weakening the criterion.
4. Run the repository checks appropriate to the changed behavior and any required CI. Record commands, versions/source identity, results and material limits. No app check substitutes for framework release qualification.
5. Check each acceptance item only after recording its evidence. Store durable reports under an app-owned evidence path; keep credentials and sensitive payloads out. Retain relevant failed results with their later disposition.

Use a ticket-specific scratch directory for temporary files and a separately identified evidence directory for retained results. Track processes/ports you start and stop only those you own. Remove disposable build output only after preserving evidence and confirming it is unused.

## Review and close

Produce a concrete reviewable diff or pull request. Its description should explain the user-visible result, relevant design decision, verification and remaining limits. Record the source revision, PR link when present and actual CI status in the ticket, then move it to In review. Follow [REVIEW](REVIEW.md) for an independent assessment; preserve findings and fixes rather than replacing the earlier record.

Once the acceptance criteria and independent review are satisfied and the change has landed in the agreed source, move it to Done. If the ticket explicitly requires a published package or deployed service, that condition must also have evidence. Otherwise, do not add release/deployment work to a small implementation ticket. Follow the current task's authorization for external actions; the historical board does not confer standing deployment or release authority.

At every status move, update the file's metadata and dated log, repair incoming links, then run:

```sh
python3 Tickets/_tools/board.py render
python3 Tickets/_tools/board.py check
```

A normal predecessor wait remains Backlog. If work hits an actual blocker, record what failed, who or what can resolve it, and the unblock criterion before using Blocked. Distinguish a product decision from an operational problem; use `Tickets/9-blocked/` with the responsible team and unblock criterion. Do not label a cross-team implementation dependency as a product decision needing Victor.
