# ADR-019: Land the pitch on main and raise the reservation ceiling

Status: accepted 2026-11-03. Victor chose one hosted environment, a landing of `pivot/pitch-mvp` onto `main`, and an internal reservation ceiling of $180 GenUI / $218 project. This supersedes ADR-013 D13.2's freeze of `main` and ADR-017's sentence that the deploy branch remains `pivot/pitch-mvp`. It does not create production, a second stack, or a new hostname.

## Trunk

`main` is the integration and deploy branch. Promote to beta runs only on a push to `main`, and only for a merge commit of one pull request into `main`. Topic work still uses pull requests. `pivot/pitch-mvp` remains in history as the pitch trunk through PR #49 (`bbb926d`). The demo simulations stay on the local branch `showcase/spend-and-jacket` and are not part of this landing.

## Reservation ceiling

The November ledger no longer fits the $170 GenUI reservation cap. The live window plus the clean-trial retained allowance is $173.54, and the next image release at the existing 18-hour shortening needs $171.33. Victor set the ceiling to **$180 GenUI and $218 including the unchanged $38 fAuth reserve**.

`MonthlyBudgetUsd` stays `170`. That stack parameter is a tag, not a billing stop, and changing it would make the automatic promotion refuse the change set. The preflight caps and the canonical policy fields `budget_genui_cents` / `budget_total_cents` are the reservation check. Raising them does not start or stop the running service. The November 1–2 account estimate was about $12.95; the ceiling is not that invoice.

The idle command `python -m ops.promotion.reservation_cap` performs only that pair of integer updates, from 17000/20800 to 18000/21800, while no deployment holds the lock. A billing-basis refresh remains the separate reconcile command and is required before the next preparation, because a basis older than 24 hours fails after the lease is taken.

## Amendment 2026-11-14: the real-user trial, its expiry and its ceilings

Victor's class of 25 adults joins his organization on the beta on November 29–30 and uses it until the purge by December 14 (the 2026-11-14 decisions; the ADR-008 amendment is the charter). That needs the beta to run past its November 25 backstop, a December ledger month, and a spending ceiling Victor has agreed to. This amendment records the three.

**Expiry.** The canonical input `live.config.deployment_expires_at_utc` is renewed once, to **2026-12-16 08:00 UTC**, by `python -m ops.promotion.expiry_renewal` (pinned in source, plan/apply, operator identity, immutable intent). The promotion still subtracts the 18-hour default, so release B's packet shows a stop of **2026-12-15 14:00 UTC**; approve only if it does. The renewal changes no schedule or stack parameter: the live window keeps its recorded Nov 25 stop until release B supersedes it. Because the window's input hash binds the expiry, `reconcile apply` refuses between the renewal and release B; the runbook orders reconcile, then `month_renewal`, then `expiry_renewal`, then the merge, all within 24 hours, and `expiry_renewal --revert` returns the config to the window's recorded value if the release does not happen in time. The December stop is not a renewal of the trial: past Dec 16 the beta stops unless a later reviewed renewal exists.

**December month.** `reconcile` cannot create a ledger month. `python -m ops.promotion.month_renewal` adds `2026-12` as an explicit operator forward reservation (rationale and four amounts, bound to the reviewed ledger hash; never lowered on a re-assessment; refused once December begins, when `reconcile` owns it). Release B's reservation then spans both months, each reconciled within 24 hours of the merge.

**Ceilings Victor confirms.** The trial's expected spending, which Victor confirms at each apply against `reconcile observe`'s account total:

| Month | GenUI | Project (with the $38 fAuth reserve) |
| --- | --- | --- |
| November 2026 | **$210** | **$248** |
| December 1–16, 2026 (to the stop) | **$80** | **$118** |

They are what the account may reach, not what is reserved: the November figure allows the three releases of the month (the Nov 5 release, release A on Nov 15 and release B on Nov 23) each counted to its supersession, plus the live window; the December figure is the retained core for sixteen days, release B's compute to the stop and the reserves. The AWS Budget in `ops/monitoring/alarms.json` takes the project figure as its monthly limit (248, then 118 at the month boundary) and mails Victor at 80% and 100% actual and 100% forecast.

**The reservation ceiling, 2026-11-14.** The November ceiling is above the $180 GenUI cap, and the independent review of W3 estimated release B's November reservation at about 19000 cents (the retained core, every window counted to its supersession, release B to month end, the recorded extra spend and the reserves), so `expiry_renewal` would refuse `budget exceeded for 2026-11`. The code reservation caps therefore become **21000 / 24800 cents** ($210 GenUI, $248 with the $38 fAuth reserve) in `infra/scripts/preflight.py`, with the mirrors in `reservation_cap.py`, `expiry_policy.py` and `reconcile.py`, exactly the pattern of this ADR's first raise. The canonical policy pair moves from 18000/21800 to 21000/24800 by the idle `python -m ops.promotion.reservation_cap --apply` (plan without `--apply`), after the code has merged and before `expiry_renewal`, which requires the policy pair to equal the code caps. `MonthlyBudgetUsd` stays `170`: the tag is not the check. The December ceiling ($80 / $118 to the stop) is below the caps and needs no code change; `month_renewal`'s plan prints the December projection, release B's compute included, beside it.
