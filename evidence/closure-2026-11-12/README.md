# Ticket closure pass, 2026-11-12

Victor asked on 2026-11-12 for the next logical tickets to be completed and moved to Done. The pass uses only code, tests and command-line checks. It runs no browser or MCP tooling and no AWS mutation. Claude (Opus 5.5) is the lead for this pass. Each ticket's closure file records Claude's own mapping of every acceptance box to current evidence. The independent review is a separate file in the same folder, written by a reviewer who did not do the mapping.

## Why these tickets

Most PH08 pitch tickets were built on `pivot/*` branches between October 30 and November 2. That work merged into `main` with PR 22–48 and runs in the sealed beta. The tickets stayed in Backlog only because their base tickets (APP-092 to APP-095) were never closed. Many of them already have every box ticked from their own qualification. This pass closes the chain in dependency order:

- **Canvas base:** APP-092 to APP-095.
- **Delivered features:** APP-101 to APP-104, 106, 108, 109, 112, 118 to 120, 141 to 145, 154 and 166.
- **Spikes:** APP-146 to APP-149.
- **Developer front door:** APP-126 and APP-131.

## What "verified on main" means here

- Every check runs on `main` at `5695869` (PR #84's merge), in a clean worktree set up with `just setup`.
- **`just ci`:** the fast set, the schema qualifier and the 18 CI campaigns. [ci-run.md](ci-run.md) has the exact command, the step summary and every campaign's check list.
- **The box campaign** runs separately. It needs the sealed parser (`just parser`) and is not in CI.
- **Earlier acceptance evidence is cited, not re-run, where re-running it would need:**
  - a real model key (the `smoke-*` entries in `scripts/dev/campaigns.tsv`);
  - Victor's Slack workspace;
  - a browser.
  Those results stay as the ticket logs record them. Each closure file names them as limits.

## What this pass does not claim

- No new browser check. The canvas and widget behaviors that only a browser can show were checked by the earlier walks recorded in each ticket's log; Victor does the browser QA.
- No hosted or AWS acceptance beyond what each ticket already records.
- APP-104's unbuilt fences are moved to [APP-177](../../Tickets/0-backlog/APP-177.md) instead of being ticked.
