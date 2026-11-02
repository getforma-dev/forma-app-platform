# PR 44 independent review and fixture repair

The final local source passes the native command campaign (21 checks, eight commits), the native race campaign (26 checks, 17 commits), formatting and Clippy. The merged base had already passed all 21 `just check` steps. This incremental change leaves APP-014 In progress: review item m2 still requires observed values in every command report check. The ticket's acceptance boxes remain unchecked.

## Review and repair

The original PR adds a zero-lock-waiter precondition and retained waiter PID, wait event, granted tuple relations, blocking PIDs and blocker state to the isolated race campaign. Its exact result and database assertions retain authorization, tenant, principal, stale-work and idempotency boundaries. The observation comment now correctly calls the tuple lock **granted**. Review item n1 is satisfied by the README's link to the historical 21-check command report; the final 21-check report is also retained here.

The supporting command campaign exposed a fixture timing problem: two sequential one-second lock waits plus connection and verification work could outlive its paused holder's three-second idle-transaction limit. The second contender then legitimately acquired the released SQL locks and committed, contradicting the intended fixture interleaving. Two attempted concurrent-contender repairs also failed: the pool and process-wide capacity permit only two Data connections, including the holder. Independent Stores do not increase that process-wide bound.

The final fixture uses two holder cycles. The same-key contender first reaches a real PostgreSQL lock wait and returns `InFlight`; no committed footprint changes. The fixture cancels that uncommitted holder, observes that exact backend PID disappear and rechecks the footprint. It then starts the identical holder and observes the distinct-key contender wait and return `InFlight`. Releasing the second holder commits exactly once; the original key replays and the distinct key conflicts on the new version. No production code, role, timeout, capacity, authority rule or expected write count changes.

## Qualification

| Command | Result |
| --- | --- |
| `just check` | 21/21 steps pass on the merged base before the final test-only repair; parent run retained |
| `just campaign command` | 21 checks, eight commits; native test 30.38 s; [report](command-campaign.json) |
| `just campaign race` | 26 checks, 17 commits; native test 37.33 s; [report](race-campaign.json) |
| `just check fmt clippy` | 2/2 steps pass on the final Rust source |

[Verification metadata](verification.json) binds both final Rust files, reports and retained private logs, including all three failed command attempts and the earlier passing race run. The fixture databases are disposable local PostgreSQL 16.13 with verified TLS and synthetic identities. This is not a hosted, RDS, image or deployment qualification. No browser is needed for this SQL test-only change.

## Remaining review scope

The race report now retains observed values beside its group checks. The command report still uses `name` and `passed` for each of its 21 checks, with separate top-level outcome evidence. That does **not** close the ticket's complete m2 requirement. This repair does not claim APP-014 Done or the phase gate passed. Exact pushed-head CI and reviewed landing remain separate delivery gates.
