"""Renew the canonical shutdown expiry for the real-user trial: one pinned value, idle, plan/apply.

The promotion derives each release's stop time from `live.config.deployment_expires_at_utc`
minus the shortening default (runner.prepare). This operator command moves that one
canonical input to the pinned RENEWED_EXPIRY so the next release (release B, about
2026-11-23) stops around 2026-12-15 14:00 UTC after its 18-hour subtraction, instead of
refusing because the live Nov 25 backstop minus 18 hours is within 24 hours.

It changes no AWS schedule, parameter, reservation or cap: the live window keeps its
recorded Nov 25 stop until a release supersedes it. Because the ledger window's input hash
binds the expiry, `reconcile apply` refuses between this renewal and the release that
consumes it; run reconcile and month_renewal first, then this, then merge, within 24 hours.
`--revert` puts the config back to exactly the live window's recorded expiry if the
release does not happen in time. Both directions are hash-bound with an immutable intent
before one conditional write, following expiry_policy.
"""
import argparse
import copy
from datetime import timedelta
import hashlib
import json
import os
from pathlib import Path
import sys
import uuid

from .aws import AWS
from .expiry_policy import operator_identity, reviewed_state
from .maintenance import no_pending_changesets
from .runner import ROOT, Runner, preflight
from .state import Refused, Store, digest, encoded, require

# The one reviewed target (the 2026-11-14 decisions, W3): a release that subtracts the
# 18-hour default stops at 2026-12-15 14:00 UTC, after the class and the Dec 14 purge.
RENEWED_EXPIRY = '2026-12-16T08:00:00Z'
SOURCE_FILES = ['ops/promotion/expiry_renewal.py', 'ops/promotion/expiry_policy.py', 'ops/promotion/runner.py',
                'ops/promotion/guards.py', 'ops/promotion/state.py', 'ops/promotion/maintenance.py', 'ops/promotion/aws.py',
                'infra/scripts/preflight.py']


def projected_reservation(budget, config, ledger, clock):
    """preflight's own validator; its refusal (a ValueError) is this command's refusal."""
    try:
        return budget.reserve(config, ledger, clock)
    except ValueError as error:
        raise Refused('next release would refuse: ' + str(error)) from None


def plan(runner, expected_state_sha256, revert=False):
    """Read-only: identity, idle reviewed state, healthy unchanged runtime, and the next release's projection."""
    store = runner.store
    state = reviewed_state(store, expected_state_sha256)
    identity = runner.aws.identity()
    operator_identity(identity, state)
    budget = preflight()
    plans = budget.PLANS['always-on-beta']
    live = state['live']
    config = live['config']
    require(config.get('operating_plan') == 'always-on-beta', 'only the always-on beta is renewed here')
    window = state['ledger']['windows'].get(live['window_id'])
    require(isinstance(window, dict) and 'superseded_by_window_id' not in window and 'abandoned_at_utc' not in window, 'live window is not the ledger head')
    recorded = window['expires_at_utc']
    require(live['parameters'].get('DeploymentExpiresAtUtc') == recorded, 'live parameters and the ledger window disagree on the expiry')
    current = config['deployment_expires_at_utc']
    if revert:
        require(current == RENEWED_EXPIRY, 'nothing to revert: the canonical config does not hold the renewed expiry')
        target = recorded
    else:
        require(current == recorded, 'canonical config already differs from its window; inspect or --revert')
        target = RENEWED_EXPIRY
        require(budget.timestamp(target) > budget.timestamp(current), 'renewal must move the expiry later')
    require(state['policy'].get('budget_genui_cents') == plans['genui_cap_cents']
            and state['policy'].get('budget_total_cents') == plans['project_cap_cents'], 'canonical policy caps differ from the code caps')
    before = runner.boundary(1)
    inventory = no_pending_changesets(runner, state['authority']['stack_id'])
    after = runner.boundary(1)
    reviewed_state(store, expected_state_sha256)
    clock = runner.clock()
    shorten = state['policy']['shorten_expiry_hours']
    prospective = None; projection = None
    if not revert:
        # preflight's reserve bounds the renewed expiry to the 32-day window itself (projected_reservation).
        next_expiry = budget.timestamp(target) - timedelta(hours=shorten)
        require(next_expiry > clock + timedelta(hours=24), 'the next release would still refuse; renew later or shorten less')
        # The next release's own reservation, projected and discarded: it proves the
        # December month exists, is fresh, and both months fit the unchanged caps.
        projection_id = str(uuid.uuid5(uuid.NAMESPACE_URL, 'forma:expiry-renewal:' + expected_state_sha256 + ':' + target))
        require(projection_id not in state['ledger']['windows'], 'projection identifier already exists')
        projection = copy.deepcopy(config)
        projection.update(window_id=projection_id, supersedes_window_id=live['window_id'], service_desired_count=0, extra_task_hours=2,
                          deployment_expires_at_utc=next_expiry.strftime('%Y-%m-%dT%H:%M:%SZ'))
        ledger = copy.deepcopy(state['ledger'])
        _, prospective = projected_reservation(budget, projection, ledger, clock)
        require(ledger == state['ledger'], 'budget validator mutated the original ledger')
        require(prospective['genui_allocation_cents'] == plans['genui_cap_cents'] and prospective['monthly_budget_cents'] == plans['project_cap_cents'],
                'caps must remain unchanged')
    expected_after = copy.deepcopy(state)
    expected_after['live']['config']['deployment_expires_at_utc'] = target
    expected_after['revision'] += 1
    operation = 'revert-expiry-renewal' if revert else 'renew-canonical-expiry'
    receipt_key = 'promotions/expiry-renewal-' + expected_state_sha256 + '-' + ('revert' if revert else target.replace(':', '')) + '/intent.json'
    receipt = {'schema_version': 1, 'operation': operation, 'observed_at_utc': clock.strftime('%Y-%m-%dT%H:%M:%SZ'),
               'before_state_sha256': expected_state_sha256, 'expected_after_state_sha256': digest(expected_after),
               'before_expiry': current, 'after_expiry': target, 'window_recorded_expiry': recorded, 'identity': identity,
               'source_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
               'source_files_sha256': {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in SOURCE_FILES},
               'ledger_sha256': digest(state['ledger']), 'live_parameters_sha256': digest(live['parameters']),
               'boundary_before': before, 'boundary_after': after, 'change_set_inventory': inventory,
               'projection_config': projection, 'projection_only': prospective,
               'limits': 'Canonical input only. No schedule, parameter, reservation, cap or billing change; the live window keeps its recorded stop.'}
    public = {'status': 'planned', 'operation': operation, 'before_revision': state['revision'],
              'before_state_sha256': expected_state_sha256, 'expected_after_state_sha256': digest(expected_after),
              'before_expiry': current, 'after_expiry': target, 'live_stop_unchanged': recorded,
              'shortening_hours': shorten, 'next_release_expiry': projection['deployment_expires_at_utc'] if projection else None,
              'cost_by_month': {month: {key: cost[key] for key in ('genui_total_cents', 'project_total_cents')}
                                for month, cost in prospective['cost_by_month'].items()} if prospective else None,
              'genui_cap_cents': plans['genui_cap_cents'], 'project_cap_cents': plans['project_cap_cents'],
              'windows_preserved': len(state['ledger']['windows']), 'ledger_changed': False, 'live_parameters_changed': False}
    return {'receipt_key': receipt_key, 'receipt': receipt, 'public': public}, expected_after


def run(runner, expected_state_sha256, apply=False, revert=False):
    result, expected_after = plan(runner, expected_state_sha256, revert)
    if not apply:
        return result
    store = runner.store
    reviewed_state(store, expected_state_sha256)
    receipt_sha256 = store.immutable(result['receipt_key'], result['receipt'])
    reviewed_state(store, expected_state_sha256)
    store.save(expected_after)  # Exactly one CAS; a lost ACK is never replayed.
    actual = store.load()
    require(actual == expected_after, 'renewal write readback differs; reconcile before retry')
    result['public'].update(status='applied', revision=actual['revision'], state_sha256=digest(actual))
    result['receipt_sha256'] = receipt_sha256
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--state-bucket', required=True)
    parser.add_argument('--state-prefix', required=True)
    parser.add_argument('--expected-state-sha256', required=True)
    parser.add_argument('--output', required=True, type=Path, help='new private plan/result file; never overwritten')
    parser.add_argument('--apply', action='store_true', help='perform the reviewed config-only CAS; without it, plan only')
    parser.add_argument('--revert', action='store_true', help='put the config back to the live window\'s recorded expiry')
    args = parser.parse_args(argv)
    try:
        fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, 'wb') as output:
            try:
                require(os.environ.get('GITHUB_ACTIONS', '').lower() != 'true', 'renewal is not a workflow operation')
                aws = AWS()
                store = Store(aws.client('s3'), args.state_bucket, args.state_prefix)
                result = run(Runner(aws, store), args.expected_state_sha256, args.apply, args.revert)
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
        print('Expiry renewal refused or write outcome unknown. Inspect the immutable intent and canonical state before retry; nothing was repeated.', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
