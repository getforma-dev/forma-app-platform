# Proposed ledger basis — always-on beta (APP-078 preparation)

The existing private ledger `.cache/operations/beta/budget-ledger.json` (schema 1, SHA-256 `bf5ff561052c334e17e936ffce444a90df44660cc30af8fc9689bc6a3d65f616`) was read and **not changed**. The rewritten basis is a new private file, `.cache/operations/beta/app-078-prep/budget-ledger-always-on.proposed.json` (mode 0600, SHA-256 `d6fe6fb92ff2f3b4822b65f2a51327d3c415042f6fecd25197f4f6311428addd`), with its unified diff beside it (`budget-ledger-always-on.proposed.diff`, SHA-256 `4850a05e73510571cf9f820afb5df229606cf4de8e70b0e2ff468ec74f07800a`). Both are gitignored. Adopting it is approval A2.

## The diff, in words

- `schema_version` 1 → 2 and a new `operating_plan: "always-on-beta"`. `preflight.py` accepts a schema 2 ledger only for an always-on input and a schema 1 ledger only for the bounded plan, so neither basis can be used with the other's caps.
- `months.2026-10` and both retained `windows` (`6362e1a2-…` 109¢, `f84517f4-…` 110¢) are carried forward byte-for-byte as history. Nothing is released or reset.
- New `months.2026-11`: `prior_extra_spend_cents 0`, `variable_reserve_cents 1500`, `auth_spent_or_reserved_cents 3800`, `unallocated_cents 0`, and a `basis` that cites ADR-008's approval and the read-only [snapshot](snapshot.json). Its `reconciled_at_utc` is the preparation time, so preflight will refuse it on Nov 1 (older than 24 hours) until the operator re-reconciles it in runbook step 4.1. That refusal is intended.

## The caps are code, not ledger input

| Plan | Ledger | GenUI cap | Separate auth | Unallocated | Project cap | Stop backstop | `MonthlyBudgetUsd` |
| --- | --- | ---: | --- | --- | ---: | --- | --- |
| `bounded-window` (default, ADR-003) | schema 1 | $70 | ≥ $5 | exactly $25 | $100 | 10 min – 48 h | 100 |
| `always-on-beta` (ADR-008, 2026-10-29) | schema 2 | $170 | $33–38 (FormaStack lean fAuth) | any, within the project cap | $208 | 10 min – 32 days | 170 |

$170 is the upper end of the approved "about $150–170/month"; $38 is FormaStack's quoted maximum for lean fAuth ("about $33, up to $38"); $208 is their sum, with no money beyond the two approved figures.

## Dry run (no file written)

`preflight.reserve` with the proposed ledger re-stamped as reconciled at 16:30, placeholder digests and the planned input, at 2026-11-01T17:00:00Z:

| November 2026 | Cents |
| --- | ---: |
| Retained core, full 30-day month (RDS, ALB, two ALB IPv4, 20 GiB storage, two keys) | 5,074 |
| Window compute: 17:00 Nov 1 → 23:00 Nov 30 + 1 h margin = 703 h, + 3 extra task-hours, at the x86 2 vCPU/8 GiB + IPv4 rate | 8,580 |
| Variable reserve | 1,500 |
| **GenUI total** (cap 17,000) | **15,154** |
| FormaStack lean fAuth reservation | 3,800 |
| **Project total** (cap 20,800) | **18,954** |

The ARM64 task is cheaper than the x86 rate used, so this is a conservative reservation, not a quote. The parameters it emits include `MonthlyBudgetUsd "170"`, `QualificationImageDigest` and `OperatorImageDigest`. Reconciliation after the window adds the measured standalone task-seconds and any observed charges to the November basis; December needs its own reconciled reservation before the backstop renewal.

## What the read-only snapshot showed

Cost Explorer month-to-date for October (estimated, all services in the account, FormaStack's App Runner and secrets included): $26.37. No AWS Budget exists. GenUI's service was 0/0/0 with no task running. One Cost Explorer request was made ($0.01).
