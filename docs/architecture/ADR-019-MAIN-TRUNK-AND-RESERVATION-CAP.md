# ADR-019: Land the pitch on main and raise the reservation ceiling

Status: accepted 2026-11-03. Victor chose one hosted environment, a landing of `pivot/pitch-mvp` onto `main`, and an internal reservation ceiling of $180 GenUI / $218 project. This supersedes ADR-013 D13.2's freeze of `main` and ADR-017's sentence that the deploy branch remains `pivot/pitch-mvp`. It does not create production, a second stack, or a new hostname.

## Trunk

`main` is the integration and deploy branch. Promote to beta runs only on a push to `main`, and only for a merge commit of one pull request into `main`. Topic work still uses pull requests. `pivot/pitch-mvp` remains in history as the pitch trunk through PR #49 (`bbb926d`). The demo simulations stay on the local branch `showcase/spend-and-jacket` and are not part of this landing.

## Reservation ceiling

The November ledger no longer fits the $170 GenUI reservation cap. The live window plus the clean-trial retained allowance is $173.54, and the next image release at the existing 18-hour shortening needs $171.33. Victor set the ceiling to **$180 GenUI and $218 including the unchanged $38 fAuth reserve**.

`MonthlyBudgetUsd` stays `170`. That stack parameter is a tag, not a billing stop, and changing it would make the automatic promotion refuse the change set. The preflight caps and the canonical policy fields `budget_genui_cents` / `budget_total_cents` are the reservation check. Raising them does not start or stop the running service. The November 1–2 account estimate was about $12.95; the ceiling is not that invoice.

The idle command `python -m ops.promotion.reservation_cap` performs only that pair of integer updates, from 17000/20800 to 18000/21800, while no deployment holds the lock. A billing-basis refresh remains the separate reconcile command and is required before the next preparation, because a basis older than 24 hours fails after the lease is taken.
