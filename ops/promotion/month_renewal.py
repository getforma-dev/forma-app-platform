"""Add (or re-assess) the reviewed December ledger month for the real-user trial, idle, plan/apply.

`reconcile` refuses to create a month (reconcile.py: only an existing current month may be
reconciled), and a release whose window reaches into December needs `ledger.months['2026-12']`
reconciled within 24 hours (infra/scripts/preflight.py reserve). No Cost Explorer data exists
for a month that has not begun, so the entry is an explicit operator forward reservation:
the pinned MONTH, a substantive rationale and four nonnegative amounts, bound to the
reviewed ledger hash. A later run may re-assess it (the timestamp moves with a new
rationale) but never lowers an amount. Once December begins, `reconcile observe/apply`
owns the month and this command refuses.

It changes no window, cap, expiry or AWS resource. The live window must still validate
against the updated ledger (as reconcile.apply checks), so November must be fresh first.
Apply records an immutable intent before one conditional write (expiry_policy pattern).
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
from .runner import ROOT, preflight
from .state import Refused, Store, closed, digest, encoded, no_duplicates, require

MONTH = '2026-12'
AMOUNTS = ('prior_extra_spend_cents', 'variable_reserve_cents', 'auth_spent_or_reserved_cents', 'unallocated_cents')
# ADR-019 amendment (2026-11-14): the trial ceilings Victor confirms at apply, as expected
# spend from Dec 1 to the Dec 16 stop. Informational beside the code caps, which stay the
# reservation check; printed in the plan so the operator compares them with the assessment.
TRIAL_CEILING_CENTS = {'genui': 8000, 'project': 11800}
SOURCE_FILES = ['ops/promotion/month_renewal.py', 'ops/promotion/reconcile.py', 'ops/promotion/expiry_policy.py',
                'ops/promotion/state.py', 'infra/scripts/preflight.py']


def december_projection(budget, ledger, amounts):
    """What preflight will compute for the month with no window compute yet: the retained core
    for the full month, the clean-trial retained figure while any window names the trial."""
    import calendar
    year, number = (int(x) for x in MONTH.split('-'))
    clean = budget.CLEAN_RETAINED_MONTHLY if any(w.get('clean_trial_organization_id') for w in ledger['windows'].values()) else 0
    retained = budget.cents(budget.FIXED_HOURLY * calendar.monthrange(year, number)[1] * 24 + budget.FIXED_STORAGE_KEYS + clean)
    genui = retained + amounts['prior_extra_spend_cents'] + amounts['variable_reserve_cents']
    total = genui + amounts['auth_spent_or_reserved_cents'] + amounts['unallocated_cents']
    return {'retained_core_full_month_cents': retained, 'genui_total_cents': genui, 'project_total_cents': total}


def plan(runner, expected_state_sha256, assessment):
    store = runner.store
    state = reviewed_state(store, expected_state_sha256)
    identity = runner.aws.identity()
    operator_identity(identity, state)
    budget = preflight()
    plans = budget.PLANS['always-on-beta']
    closed(assessment, ['month', 'ledger_sha256', 'rationale', 'amounts'], 'month assessment')
    require(assessment['month'] == MONTH, 'only the pinned month ' + MONTH + ' is renewed here')
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
    require(clock.strftime('%Y-%m') < MONTH, 'the month has begun; reconcile owns it now')
    ledger = copy.deepcopy(state['ledger'])
    require(ledger.get('operating_plan') == 'always-on-beta' and ledger.get('schema_version') == 2, 'ledger is not the always-on beta ledger')
    existing = ledger['months'].get(MONTH)
    if existing is not None:
        for key in AMOUNTS:
            require(amounts[key] >= existing[key], 'cannot lower recorded money: ' + key)
    projection = december_projection(budget, ledger, amounts)
    require(projection['genui_total_cents'] <= plans['genui_cap_cents'] and projection['project_total_cents'] <= plans['project_cap_cents'],
            'December reservation exceeds the %d/%d cent caps' % (plans['genui_cap_cents'], plans['project_cap_cents']))
    ledger['months'][MONTH] = dict(amounts, reconciled_at_utc=clock.strftime('%Y-%m-%dT%H:%M:%SZ'),
                                   basis='Operator forward reservation (ADR-019 amendment 2026-11-14; no billing data for a month not begun): ' + rationale.strip())
    # The live window still validates against the updated ledger, exactly as reconcile.apply requires.
    try:
        checked, _ = budget.reserve(state['live']['config'], ledger, clock)
    except ValueError as error:
        raise Refused('live window would refuse the updated ledger: ' + str(error)) from None
    require(checked == ledger, 'month renewal would change a deployment window')
    expected_after = copy.deepcopy(state)
    expected_after['ledger'] = ledger
    expected_after['revision'] += 1
    receipt_key = 'promotions/month-renewal-' + expected_state_sha256 + '-' + MONTH + '/intent.json'
    receipt = {'schema_version': 1, 'operation': 'add-or-reassess-ledger-month', 'month': MONTH,
               'observed_at_utc': clock.strftime('%Y-%m-%dT%H:%M:%SZ'), 'before_state_sha256': expected_state_sha256,
               'expected_after_state_sha256': digest(expected_after), 'previous_entry': existing, 'entry': ledger['months'][MONTH],
               'assessment': assessment, 'identity': identity, 'projection': projection, 'trial_ceiling_cents': TRIAL_CEILING_CENTS,
               'source_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
               'source_files_sha256': {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in SOURCE_FILES},
               'previous_ledger_sha256': digest(state['ledger']), 'updated_ledger_sha256': digest(ledger),
               'limits': 'One ledger month only. No window, cap, expiry, schedule or billing change.'}
    public = {'status': 'planned', 'month': MONTH, 'created': existing is None, 'before_revision': state['revision'],
              'before_state_sha256': expected_state_sha256, 'expected_after_state_sha256': digest(expected_after),
              'amounts': dict(amounts), 'december_projection': projection, 'trial_ceiling_cents': TRIAL_CEILING_CENTS,
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
                        help='JSON {"month": "2026-12", "ledger_sha256": ..., "rationale": ..., "amounts": {the four cent fields}}')
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
