"""Add (or re-assess) a reviewed future ledger month, idle, plan/apply.

`reconcile` refreshes only the current month and refuses to create one (reconcile.py), and a
release whose window reaches into a later month needs that month in `ledger.months`,
reconciled within 24 hours (infra/scripts/preflight.py reserve): every November release
reaches into December, and the Jan 15 renewal release reaches into January. No Cost
Explorer data exists for a month that has not begun, so the entry is an explicit operator
forward reservation: a month from the closed pin set MONTHS, a substantive rationale and
four nonnegative amounts, bound to the reviewed ledger hash. A later run may re-assess it
(the timestamp moves with a new rationale) but never lowers an amount. Once the month
begins, `reconcile observe/apply` owns it and this command refuses.

MONTHS is a pin set, not a free argument: one reviewed ceiling per month (ADR-019 amendment
2). It changes no window, cap, expiry or AWS resource. The live window must still validate
against the updated ledger (reconcile.refresh_validator: every check a reservation makes,
except that the other spanned months need only exist; freshness is reserve's rule at
reservation time). Apply records an immutable intent before one conditional write
(expiry_policy pattern).
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import sys

from .aws import AWS
from .expiry_policy import operator_identity, reviewed_state
from .reconcile import refresh_validator
from .runner import ROOT, preflight
from .state import Refused, Store, closed, digest, encoded, no_duplicates, require

AMOUNTS = ('prior_extra_spend_cents', 'variable_reserve_cents', 'auth_spent_or_reserved_cents', 'unallocated_cents')
# ADR-019 amendment 2 (2026-11-15, the class continuation): the closed set of months this
# command may add or re-assess, each with the ceiling Victor confirms at apply, in cents.
# The ceilings are informational beside the code caps, which stay the reservation check;
# the plan prints them so the operator compares them with the projection. The month comes
# from the assessment and must be in this set and not yet begun.
# December, the whole month to the Jan 15 stop: retained core 5589 + release compute 10360
# (744 h + 2, service and broker) + reserve 1500 = 17449 GenUI; + fAuth 3800 = 21249. It
# stays in the set because every release before Dec 1 reaches into December and must
# refresh it (releases B to F did); the first amendment's "to the Dec 16 stop" figure,
# 8000 / 11800, is superseded.
# January, Jan 1 to the Jan 15 14:00 UTC stop: 5589 + 4903 (351 h + 2) + 1500 = 11992; + 3800 = 15792.
MONTHS = {'2026-12': {'genui': 17500, 'project': 21300}, '2027-01': {'genui': 12000, 'project': 15800}}
SOURCE_FILES = ['ops/promotion/month_renewal.py', 'ops/promotion/reconcile.py', 'ops/promotion/expiry_policy.py',
                'ops/promotion/state.py', 'infra/scripts/preflight.py']


def month_projection(budget, ledger, month, amounts, shorten_hours):
    """What preflight will compute for `month` once the renewal release is reserved: the retained core
    for the full month (plus the clean-trial retained figure while any window names the trial), the
    release's compute in the month from its first day to the stop (the renewed expiry minus the
    shortening default, plus preflight's one-hour margin and two extra task-hours) and the assessed
    reserves."""
    import calendar
    from datetime import datetime, timedelta, timezone
    from .expiry_renewal import RENEWED_EXPIRY
    year, number = (int(x) for x in month.split('-'))
    clean_trial = any(w.get('clean_trial_organization_id') for w in ledger['windows'].values())
    clean = budget.CLEAN_RETAINED_MONTHLY if clean_trial else 0
    retained = budget.cents(budget.FIXED_HOURLY * calendar.monthrange(year, number)[1] * 24 + budget.FIXED_STORAGE_KEYS + clean)
    stop = budget.timestamp(RENEWED_EXPIRY) - timedelta(hours=shorten_hours)
    hours = budget.months_between(datetime(year, number, 1, tzinfo=timezone.utc), stop + timedelta(hours=1)).get(month, 0) + 2
    rate = budget.TASK_HOURLY + (budget.BROKER_HOURLY if clean_trial else 0)
    compute = budget.cents(hours * rate)
    genui = retained + compute + amounts['prior_extra_spend_cents'] + amounts['variable_reserve_cents']
    total = genui + amounts['auth_spent_or_reserved_cents'] + amounts['unallocated_cents']
    return {'retained_core_full_month_cents': retained, 'release_compute_cents': compute, 'release_stop_utc': stop.strftime('%Y-%m-%dT%H:%M:%SZ'),
            'genui_total_cents': genui, 'project_total_cents': total}


def plan(runner, expected_state_sha256, assessment):
    store = runner.store
    state = reviewed_state(store, expected_state_sha256)
    identity = runner.aws.identity()
    operator_identity(identity, state)
    budget = preflight()
    plans = budget.PLANS['always-on-beta']
    closed(assessment, ['month', 'ledger_sha256', 'rationale', 'amounts'], 'month assessment')
    month = assessment['month']
    require(month in MONTHS, 'only a pinned month is renewed here: ' + ', '.join(sorted(MONTHS)))
    require(assessment['ledger_sha256'] == digest(state['ledger']), 'assessment is bound to another ledger')
    rationale = assessment['rationale']
    require(isinstance(rationale, str) and 40 <= len(rationale.strip()) <= 600 and all(ord(c) >= 32 for c in rationale),
            'a substantive single-line forward-reservation rationale is required')
    closed(assessment['amounts'], AMOUNTS, 'assessed amounts')
    amounts = assessment['amounts']
    require(all(type(amounts[k]) is int and 0 <= amounts[k] <= plans['project_cap_cents'] for k in AMOUNTS), 'amounts must be nonnegative integer cents within the cap')
    require(amounts['variable_reserve_cents'] >= budget.MIN_VARIABLE_RESERVE, 'retain at least $10 for probes, secrets, storage, logs and requests')
    require(plans['auth_min_cents'] <= amounts['auth_spent_or_reserved_cents'] <= plans['auth_max_cents'], 'reserve FormaStack lean fAuth at its approved $33-38 figure')
    clock = runner.clock()
    require(clock.strftime('%Y-%m') < month, 'the month has begun; reconcile owns it now')
    ledger = copy.deepcopy(state['ledger'])
    require(ledger.get('operating_plan') == 'always-on-beta' and ledger.get('schema_version') == 2, 'ledger is not the always-on beta ledger')
    existing = ledger['months'].get(month)
    if existing is not None:
        for key in AMOUNTS:
            require(amounts[key] >= existing[key], 'cannot lower recorded money: ' + key)
    projection = month_projection(budget, ledger, month, amounts, state['policy']['shorten_expiry_hours'])
    require(projection['genui_total_cents'] <= plans['genui_cap_cents'] and projection['project_total_cents'] <= plans['project_cap_cents'],
            month + ' reservation exceeds the %d/%d cent caps' % (plans['genui_cap_cents'], plans['project_cap_cents']))
    ledger['months'][month] = dict(amounts, reconciled_at_utc=clock.strftime('%Y-%m-%dT%H:%M:%SZ'),
                                   basis='Operator forward reservation (ADR-019 amendment 2, 2026-11-15; no billing data for a month not begun): ' + rationale.strip())
    # The live window still validates against the updated ledger, exactly as reconcile.apply checks it:
    # every check a reservation makes, with the other spanned months present but not required fresh.
    try:
        checked, _ = refresh_validator(state['live']['config'], ledger, clock)
    except ValueError as error:
        raise Refused('live window would refuse the updated ledger: ' + str(error)) from None
    require(checked == ledger, 'month renewal would change a deployment window')
    expected_after = copy.deepcopy(state)
    expected_after['ledger'] = ledger
    expected_after['revision'] += 1
    receipt_key = 'promotions/month-renewal-' + expected_state_sha256 + '-' + month + '/intent.json'
    receipt = {'schema_version': 1, 'operation': 'add-or-reassess-ledger-month', 'month': month,
               'observed_at_utc': clock.strftime('%Y-%m-%dT%H:%M:%SZ'), 'before_state_sha256': expected_state_sha256,
               'expected_after_state_sha256': digest(expected_after), 'previous_entry': existing, 'entry': ledger['months'][month],
               'assessment': assessment, 'identity': identity, 'projection': projection, 'trial_ceiling_cents': MONTHS[month],
               'source_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
               'source_files_sha256': {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in SOURCE_FILES},
               'previous_ledger_sha256': digest(state['ledger']), 'updated_ledger_sha256': digest(ledger),
               'limits': 'One ledger month only. No window, cap, expiry, schedule or billing change.'}
    public = {'status': 'planned', 'month': month, 'created': existing is None, 'before_revision': state['revision'],
              'before_state_sha256': expected_state_sha256, 'expected_after_state_sha256': digest(expected_after),
              'amounts': dict(amounts), 'month_projection': projection, 'trial_ceiling_cents': MONTHS[month],
              'genui_cap_cents': plans['genui_cap_cents'], 'project_cap_cents': plans['project_cap_cents'],
              'windows_preserved': len(ledger['windows']), 'windows_changed': False}
    return {'receipt_key': receipt_key, 'receipt': receipt, 'public': public}, expected_after


def run(runner, expected_state_sha256, assessment, apply=False):
    result, expected_after = plan(runner, expected_state_sha256, assessment)
    if not apply:
        return result
    store = runner.store
    reviewed_state(store, expected_state_sha256)
    receipt_sha256 = store.immutable(result['receipt_key'], result['receipt'])
    reviewed_state(store, expected_state_sha256)
    store.save(expected_after)
    actual = store.load()
    require(actual == expected_after, 'month write readback differs; reconcile before retry')
    result['public'].update(status='applied', revision=actual['revision'], state_sha256=digest(actual))
    result['receipt_sha256'] = receipt_sha256
    return result


def read(path):
    return json.loads(Path(path).read_text(), object_pairs_hook=no_duplicates)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--state-bucket', required=True)
    parser.add_argument('--state-prefix', required=True)
    parser.add_argument('--expected-state-sha256', required=True)
    parser.add_argument('--assessment', required=True, type=Path,
                        help='JSON {"month": one of ' + ', '.join(sorted(MONTHS)) + ', "ledger_sha256": ..., "rationale": ..., "amounts": {the four cent fields}}')
    parser.add_argument('--output', required=True, type=Path, help='new private plan/result file; never overwritten')
    parser.add_argument('--apply', action='store_true', help='perform the reviewed ledger-month CAS; without it, plan only')
    args = parser.parse_args(argv)
    try:
        fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, 'wb') as output:
            try:
                require(os.environ.get('GITHUB_ACTIONS', '').lower() != 'true', 'month renewal is not a workflow operation')
                aws = AWS()
                store = Store(aws.client('s3'), args.state_bucket, args.state_prefix)
                from .runner import Runner
                result = run(Runner(aws, store), args.expected_state_sha256, read(args.assessment), args.apply)
                output.write(encoded(result)); output.flush(); os.fsync(output.fileno())
            except Exception:
                try:
                    output.seek(0); output.truncate()
                    output.write(encoded({'status': 'refused-or-outcome-unknown',
                                          'instruction': 'Inspect immutable intent and canonical state before retry; no automatic replay.'}))
                    output.flush(); os.fsync(output.fileno())
                except OSError:
                    pass
                raise
        print(json.dumps(result['public'], sort_keys=True))
    except Exception:
        print('Month renewal refused or write outcome unknown. Inspect the immutable intent and canonical state before retry; nothing was repeated.', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
