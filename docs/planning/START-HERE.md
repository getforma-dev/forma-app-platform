# First implementation session

This preparation checkpoint intentionally stops before application code. Begin with **APP-001** when implementation is requested.

1. Read the [source guide](SOURCE-GUIDE.md), [3.2 baseline](../architecture/FRAMEWORK-BASELINE.md) and [cross-team handoff](CROSS-TEAM-HANDOFF.md).
2. Run `python3 Tickets/_tools/board.py check` and `python3 Tickets/_tools/board.py next` from the repository root.
3. Select the first ready ticket, confirm its scope and required evidence, assign an individual under Team GenUI, and move it through the [normal workflow](../../Tickets/README.md).
4. Work from the published version and its contracts. Record exact lockfiles and local commands when the scaffold ticket creates them. This preparation repository has no claimed `npm run dev` or application test command yet.
5. Capture the result in the ticket's log and `evidence/APP-NNN/`, regenerate the board, and obtain independent review before marking completion.

FormaStack can work its assigned upstream tickets immediately in parallel. GenUI can prepare contracts, synthetic fixtures and independent parser/query spikes while native/auth integration is blocked. Do not hide a blocked acceptance check behind a mock and call the integrated spike complete.

The first engineering milestone is small: one admitted app, one authorized query with a receipt, one user-submitted durable record, an identical retry, a stale-edit conflict, and a successful reopen. Follow with real bounded CSV/XLSX imports and the first ported native components before polishing the whole canvas or six acts.

Later work does not need another round of product rediscovery: the design already selected typed definitions, first-party renderers, PostgreSQL records, Parquet/DuckDB sources, fAuth and native rendering. Record any required amendment explicitly rather than silently replacing those decisions.
