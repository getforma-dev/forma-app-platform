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

## Amendment 2, 2026-11-15: the class continuation, the renewal to Jan 15 and the 45-day backstop

Victor's decisions of 2026-11-15 ([CLASS-CONTINUATION-PLAN.md](../planning/CLASS-CONTINUATION-PLAN.md) §1; [APP-228](../../Tickets/0-backlog/APP-228.md)): the class members **keep their accounts, organization membership, apps and data by default, on a free plan**, after the class, instead of the Dec 14 purge, and **the beta keeps running to Jan 15, 2027** ("Renew to Jan 15"). This amendment records what that needs from the first amendment's mechanisms: a new expiry pin, a new ledger month, a wider backstop and the ceilings. The caps do not move. The notice, the charter and the free plan are APP-227 and APP-231, not this ADR.

**Expiry.** `RENEWED_EXPIRY` in `ops/promotion/expiry_renewal.py` moves from `2026-12-16T08:00:00Z` (consumed by release B on 2026-11-14) to **`2027-01-16T08:00:00Z`**. The promotion still subtracts the 18-hour default, so the release that consumes the pin (release H, Dec 2–9) reserves a stop of **2027-01-15 14:00 UTC**; approve its packet only if it reads that. As before, the renewal changes no schedule or stack parameter: the live window keeps its recorded stop (2026-12-11 20:00 UTC after release F; 02:00 UTC if release G executes first) until release H supersedes it, and `--revert` returns the config to the window's recorded value if H does not merge in time.

**The January month.** `MONTH` in `ops/promotion/month_renewal.py` is re-pinned from `2026-12` to **`2027-01`**: a pin, not a `--month` argument, so the command keeps the one-reviewed-value pattern (one month per renewal, refused for any other month, never lowered on a re-assessment, refused once the month has begun). December needs no re-assessment: it has begun, `reconcile` owns it, and release H's window reserves its December compute at reservation time.

**The backstop: 45 days.** `PLANS['always-on-beta']['max_expiry']` in `infra/scripts/preflight.py` moves from 32 to **45 days**; the reservation check and the live activation check read the same value, and the refusal says "45 days". Under 32 days a window reserved from Dec 2–9 reached Jan 11 at most, and Jan 15 would have cost a second renewal release (each release costs 18 hours of stop and a review). From **Dec 1 14:00 UTC** a 45-day window reaches Jan 15 14:00 UTC; a day earlier is 46 days and refused. The wider bound is safe for the reasons the 32-day one was: it limits how far a *single* reservation may reach, while every window still reserves every month it touches, within the unchanged per-month caps, against a ledger reconciled within 24 hours (the ledger already spans two months); the stop schedule, the stack parameter and the ledger window still carry the same one value; and nothing runs past the stop without a new reviewed release. A longer bound reserves no more money: the per-month caps do.

**Ceilings Victor confirms.** From the reservation model (`preflight.py`: retained core 0.0645 $/h plus $4.30 for storage and keys plus $3.60 while a window names the trial; compute 0.1215248 + 0.0173428 $/h for the service and the broker; each figure rounded up to the cent):

| Month | GenUI | Project (with the $38 fAuth reserve) | Model, in cents |
| --- | --- | --- | --- |
| December 2026 (the whole month, to the Jan 15 stop) | **about $175** | **about $213** | 5589 retained + 10360 compute (744 h + 2) + 1500 reserve = 17449; + 3800 = 21249 |
| January 1–15, 2027 (to the 14:00 UTC stop) | **about $120** | **about $158** | 5589 retained + 4903 compute (351 h + 2) + 1500 reserve = 11992; + 3800 = 15792 |

Both fit the first amendment's **21000 / 24800** caps, so **the caps stay**; December's GenUI headroom is 3551 cents, about $35. The check is the December `reconcile observe` before release H: if the recorded extra spend (the measured account runs about $180 a month on the GenUI side against the model's $174) pushes December above 21000, raise by this ADR's pattern (`reservation_cap`, idle, after a code constant merge, no service release). `TRIAL_CEILING_CENTS` in `month_renewal.py` becomes January's 12000 / 15800, printed beside the plan's projection. The first amendment's "December 1–16" row and its sentence "the December ceiling ($80 / $118 to the stop) ... needs no code change" are superseded by the rows above. The AWS Budget (`ops/monitoring/alarms.json`, `MonthlyBudgetUsd`) **stays 248 through December** and becomes about 160 on Jan 1: change only the amount (runbook A7), never the email.

**Order of operations** ([runbook §2.1](../operations/TRIAL-RUNBOOK.md#21-renewal-to-jan-15-before-release-h-dec-29)), all within 24 hours, at least a day before the live stop, the approval outside 05:00–05:45 UTC: (1) `reconcile observe/apply` for December; (2) `month_renewal` with a `2027-01` assessment bound to the post-apply `ledger_sha256`, plan then apply; (3) `expiry_renewal` plan then apply against the hash the month apply printed, expecting `after_expiry 2027-01-16T08:00:00Z`, `next_release_expiry 2027-01-15T14:00:00Z`, caps 21000 / 24800 and both months under them; (4) `PROMOTION_ENABLED` true, merge release H, approve only if its packet's stop reads 2027-01-15T14:00:00Z; `--revert` if it does not merge in time. The pins and the bound must be on `main` before step 2 (the commands read the checkout); merging them while promotion is paused costs no release (`beta-promote.yml`, `runtime_changed`).

**After Jan 15.** The stop is a backstop, not a renewal: past Jan 15 14:00 UTC the beta stops unless a later reviewed renewal exists. APP-232 decides by Jan 1 whether to park, tear down or renew once more.
