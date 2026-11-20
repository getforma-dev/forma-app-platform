# Renewal to Jan 15: checks and break-and-restore (APP-228; 2026-11-15)

Branch `ops/app-228-renewal-jan15`, worktree `forma-renewal`, from `main` `d00b9395`. Ops code and
docs only: no app, migration, workflow or AWS change; promotion stays paused and nothing here is a
release. No operator command was run against the live state. Logs are local only (the session
scratchpad); the lines quoted here are copied from them.

## The pins and the arithmetic

- `RENEWED_EXPIRY = '2027-01-16T08:00:00Z'` (`ops/promotion/expiry_renewal.py`). Minus the 18-hour
  shortening default: **2027-01-15T14:00:00Z**, the stop release H reserves.
- `MONTH = '2027-01'` (`ops/promotion/month_renewal.py`; a pin, not an argument),
  `TRIAL_CEILING_CENTS = {'genui': 12000, 'project': 15800}`; `december_projection` is now
  `month_projection` and the plan's public key is `month_projection`.
- `max_expiry` 32 → 45 days (`infra/scripts/preflight.py`, `PLANS['always-on-beta']`, read by
  `reserve` and by the live activation check; the refusal at `reserve` says "45 days"). It was the
  only bound in code; the prose copies in `infra/README.md`, `docs/pitch/DEPLOY-RUNBOOK.md` and the
  comment in `expiry_renewal.py` are updated. Jan 15 14:00Z − 45 days = **Dec 1 14:00Z**: a renewal
  from then on fits in one release; Nov 30 14:00Z is 46 days and refuses.
- Cost model (`preflight.py` constants, cents, each figure rounded up): December 5589 retained +
  10360 compute (746 h × 0.1388676 $/h, service and broker) + 1500 reserve = 17449 GenUI, 21249
  project; January 5589 + 4903 (353 h) + 1500 = 11992 GenUI, 15792 project. Caps 21000 / 24800
  unchanged; December's GenUI headroom is 3551 cents.
- No state schema-version field changes: the ledger stays `schema_version` 2 and the intent receipts
  `schema_version` 1. Today's canonical state (the November and December months, caps 21000 / 24800,
  the live window's recorded stop) is read unchanged; the December month is reconcile's. The tests'
  fixture moves the live window to release F's stop (2026-12-11T20:00Z, reserved Nov 15) under a
  Dec 2 clock, the shape of the live state in the release-H window.

## Checks (2026-11-15, this worktree, after `just setup`)

| Command | Result |
| --- | --- |
| `just check py-promotion py-infra py-ops` | all three passed: py-promotion `Ran 287 tests ... OK (skipped=1)`, py-infra `Ran 86 tests ... OK`, py-ops `Ran 40 tests ... OK` |
| `python3 -m unittest ops.promotion.test_expiry_renewal ops.promotion.test_month_renewal` | `Ran 17 tests ... OK` |
| `cd infra/test && python3 -m unittest preflight_test.AlwaysOnPlan` | `Ran 9 tests ... OK` |

The tests added for the ticket:

- `ops/promotion/test_expiry_renewal.py`: `test_the_renewed_expiry_less_one_release_is_jan_15_14_00`
  (the pin minus 18 hours is 2027-01-15 14:00 UTC and the plan prints it) and
  `test_the_45_day_bound_admits_the_release_h_window_and_refuses_46_days` (Dec 2 00:00Z and
  Dec 1 14:00Z admitted; Dec 1 13:59:59Z and Nov 30 14:00Z refused with "45 days"). The refusal
  table's "renewal beyond 32 days" case is now "renewal beyond 45 days" (a Nov 30 clock).
- `ops/promotion/test_month_renewal.py`: `test_refuses_any_month_but_the_pinned_january` (eight
  other values, each refused with "only the pinned month 2027-01");
  `test_once_january_begins_reconcile_owns_the_month`; `test_clean_trial_windows_add_the_retained_trial_figure`
  pins the plan's figures 5589 / 4903 / 11992 / 15792 under the 12000 / 15800 ceiling.
- `infra/test/preflight_test.py`: `test_the_45_day_backstop_reaches_jan_15_from_the_release_h_window`
  (the same four clocks against `reserve` with a Jan 15 14:00Z stop, both months reserved under the
  caps) and `test_always_on_expiry_is_a_bounded_backstop` re-pinned to 45 days (Dec 16 17:00:00Z from
  the Nov 1 17:00Z window admitted; one second more, a day more, and under ten minutes refused).
- `ops/promotion/test_runner.py` gains `RESERVES`, `reconciled` and `live_window`, the fixture
  helpers that move the live window forward with the stack parameters and stop schedules reading it.

## Break-and-restore: the 45-day bound (`infra/scripts/preflight.py`)

Break: `'max_expiry': timedelta(days=45)` → `timedelta(days=32)` by one `sed`, the message text left
at "45 days" so the failures show the bound and not the message; a byte copy was taken before
(`sha256 eedda0e96b3a0b50…`).

- `python3 -m unittest ops.promotion.test_expiry_renewal.ExpiryRenewalTests.test_the_45_day_bound_admits_the_release_h_window_and_refuses_46_days ops.promotion.test_expiry_renewal.ExpiryRenewalTests.test_plan_is_read_only_and_projects_the_next_release_into_both_months`
  → `Ran 2 tests ... FAILED (errors=3)`: the two admitted subtests (`clock='2026-12-02T00:00:00Z'`
  and `'2026-12-01T14:00:00Z'`) and the plan test each error with
  `ops.promotion.state.Refused: next release would refuse: expiry must be 10 minutes to 45 days in the future`;
  the two refused subtests (Dec 1 13:59:59Z and Nov 30 14:00Z) still pass.
- `cd infra/test && python3 -m unittest preflight_test.AlwaysOnPlan.test_the_45_day_backstop_reaches_jan_15_from_the_release_h_window preflight_test.AlwaysOnPlan.test_always_on_expiry_is_a_bounded_backstop`
  → `Ran 2 tests ... FAILED (errors=3)`: the same two admitted subtests and the Dec 16 16:59:59Z
  admission error with `ValueError: expiry must be 10 minutes to 45 days in the future`.

Restored: the byte copy put back (`cmp` equal, the same sha256; `grep -c "timedelta(days=32)"` is 0);
the two renewal modules `Ran 17 tests ... OK` and `preflight_test.AlwaysOnPlan` `Ran 9 tests ... OK`.

## Limitations

- No plan run against the live state (the ticket's second acceptance box): operator commands are
  outside this change; the operator runs `month_renewal` and `expiry_renewal` without `--apply`
  from a `main` checkout carrying the pins, in the private trial folder, before release H
  (runbook section 2.1).
- The ticket file lives on PR #156's branch (`plan/class-continuation`), not yet on `main` when this
  branch was cut; its "Branch / PR" row and log are updated once both land.
- The `just check` log carries two `argparse` usage lines on stderr from the maintenance CLI tests;
  they are the tests' own refusals, not failures.

## Review round, the same day: the pin set and the refresh deadlock

The independent review of PR #159 said GO-after-fixes. Fixed on the same branch, no rebase, no force-push.

- **BLOCKER, the pin set.** `MONTH = '2027-01'` would have refused every November release: a window
  reaching into December needs December reconciled within 24 hours (`preflight.reserve`), `reconcile`
  refreshes only the current month, and only `month_renewal` could refresh December (releases B to F
  did). Now `MONTHS = {'2026-12': {'genui': 17500, 'project': 21300}, '2027-01': {'genui': 12000,
  'project': 15800}}`: the month comes from the assessment, must be in the set and not yet begun, and
  the projection, the ceiling and the receipt key are per month. December's ceiling is amendment 2's
  row: `month_projection(…, '2026-12', …)` with the trial named is 5589 + 10360 (746 h × 0.1388676) +
  1500 = 17449 GenUI, 21249 project, under 17500 / 21300 and under the caps.
- **MAJOR, the stale-month deadlock.** `reconcile.apply` and `month_renewal` validated the live window
  with `preflight.reserve`, which requires every spanned month fresh, so with both months more than a
  day stale neither could run (the live basis of 2026-11-15 13:13Z would have reached that state a day
  later). `reserve(..., refresh=False)` is unchanged; `refresh=True`, through the new
  `reconcile.refresh_validator` both refreshes use, requires the spanned months to exist and be
  well-formed (`reconciled <= now`, the floors, the fAuth figure, the caps, the bindings) but not
  fresh, and refuses together with `activating`. Every reservation path (`runner.prepare`, the
  activation check, the renewal projections) keeps the 24-hour rule.
- MINOR: runbook 2.1 says the earliest day is Dec 2 (`reconcile observe` needs a completed day) and
  gives the refresh order before Dec 2 (reconcile November, `month_renewal 2026-12`) and from Dec 2
  (reconcile December, `month_renewal 2027-01`, `expiry_renewal`); ADR-019 amendment 2 describes the
  pin set and gains "Refreshes and freshness". NIT: the `expiry_renewal` comment.

### Checks

| Command | Result |
| --- | --- |
| `just check py-promotion py-infra py-ops` | py-promotion `Ran 290 tests ... OK (skipped=5)`, py-infra `Ran 87 tests ... OK`, py-ops `Ran 40 tests ... OK`. The skips are the pre-existing "no scan disposition" one and four `test_schema_release` tests that skip "no schema release is pending": the branch now sits on `main` 136b8d38 (PR #158, the release-F close-out), which removed the spent `schema-release.json` |
| `python3 -m unittest ops.promotion.test_expiry_renewal ops.promotion.test_month_renewal ops.promotion.test_reconcile` | `Ran 32 tests ... OK` |
| `cd infra/test && python3 -m unittest preflight_test.AlwaysOnPlan` | `Ran 10 tests ... OK` |

New tests: `test_refuses_any_month_outside_the_pin_set`,
`test_december_is_reassessed_under_a_november_clock_at_the_amendment_row`,
`test_from_a_fully_stale_ledger_reconcile_then_reassess_then_a_release_reserves` and
`test_a_reservation_still_refuses_a_stale_month_that_a_refresh_accepted` (`test_month_renewal.py`; its
refusal table gains "december begun", "december in the future" and "december below the floor" and
drops "stale december", which a refresh now accepts), and
`test_a_refresh_accepts_stale_spanned_months_but_a_reservation_refuses_them` (`preflight_test.py`).

### The reviewer's probe

`g_refusal.py` (the session scratchpad) builds release F's window with both months last reconciled
Nov 15 13:14Z under a Nov 20 clock. As written, its lines 2 and 3 call `budget.reserve` directly, a
bare reservation, which still refuses a stale month by design, and its last line reads the removed
`MONTH` name. `g_probe_after.py` asks the same question through the real commands:

```
1. G prepare, both months last fresh Nov 15: refused: ledger must be reconciled within the last 24 hours
2. reconcile apply November (default validator; December still stale): ADMITTED (reconciled)
3. month_renewal 2026-12 (November fresh, December stale until this write): ADMITTED (applied)
4. G prepare, both months fresh: ADMITTED (['2026-11', '2026-12'])
5. G prepare with December stale again (must refuse): refused: ledger must be reconciled within the last 24 hours
month_renewal.MONTHS = ['2026-12', '2027-01']
```

### Break-and-restore: the refresh validator (`infra/scripts/preflight.py`)

Break: the refresh branch's `require(reconciled <= now, …)` made `now - 24 h <= reconciled <= now`,
freshness back in the validator; a byte copy was taken before (`sha256 f1523eb5b72c1e72…`).

- `python3 -m unittest ops.promotion.test_month_renewal.MonthRenewalTests.test_from_a_fully_stale_ledger_reconcile_then_reassess_then_a_release_reserves ops.promotion.test_month_renewal.MonthRenewalTests.test_a_reservation_still_refuses_a_stale_month_that_a_refresh_accepted`
  → `Ran 2 tests ... FAILED (errors=2)`: the first errors at `reconcile.apply` with
  `ValueError: ledger month reconciled in the future` (the refresh branch's message, now firing on
  staleness), the second with `Refused: live window would refuse the updated ledger: ledger month reconciled in the future`.
- `preflight_test.AlwaysOnPlan.test_a_refresh_accepts_stale_spanned_months_but_a_reservation_refuses_them`
  → `FAILED (errors=3)`, one per stale subtest (`2026-11`, `2026-12`, `both`).
- `g_probe_after.py` under the break: steps 2 and 3 refused, the deadlock reproduced.

Restored: the byte copy put back (`cmp` equal, the same sha256); the three modules `Ran 32 tests ... OK`
and `preflight_test.AlwaysOnPlan` `Ran 10 tests ... OK`.
