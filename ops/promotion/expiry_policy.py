"""Plan or explicitly apply a stricter expiry default for future promotions.

This operator command never reserves a window, refreshes billing evidence,
extends expiry, changes budget caps, or changes running infrastructure. Apply
records an immutable intent before one conditional state write. An uncertain
receipt/write/readback must be reconciled; the command never retries it.
"""
import argparse
import copy
from datetime import timedelta
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import uuid

from .aws import AWS
from .guards import ACCOUNT
from .maintenance import no_pending_changesets
from .runner import ROOT, Runner, preflight
from .state import Store, digest, encoded, require

MAX_SHORTENING_HOURS = 72


def operator_identity(identity, state):
    require(identity.get('Account') == ACCOUNT, 'operator account differs')
    arn = identity.get('Arn', '')
    user = re.fullmatch(r'arn:aws:iam::' + ACCOUNT + r':user/[A-Za-z0-9_+=,.@/-]+', arn)
    role = re.fullmatch(r'arn:aws:sts::' + ACCOUNT + r':assumed-role/([A-Za-z0-9_+=,.@-]+)/[^/]+', arn)
    require(user or role, 'an authenticated operator user or role is required')
    denied = {state['authority'][key].rsplit('/', 1)[-1]
              for key in ('prepare_role_arn', 'execute_role_arn', 'execution_role_arn')}
    require(not role or role[1] not in denied, 'workflow and execution roles cannot change operator policy')
    require(os.environ.get('GITHUB_ACTIONS', '').lower() != 'true', 'policy changes are not workflow operations')


def reviewed_state(store, expected):
    require(isinstance(expected, str) and re.fullmatch(r'[0-9a-f]{64}', expected), 'exact reviewed state hash required')
    state = store.load()
    require(digest(state) == expected, 'state changed since operator review')
    require(state.get('active') is None, 'deployment or recovery holds the canonical lock')
    return state


def plan(runner, hours, expected_state_sha256):
    """Read-only actual boundary and pure budget qualification; no S3 writes."""
    store = runner.store
    state = reviewed_state(store, expected_state_sha256)
    old = state['policy']['shorten_expiry_hours']
    require(type(old) is int and 0 <= old < MAX_SHORTENING_HOURS
            and type(hours) is int and old < hours <= MAX_SHORTENING_HOURS,
            'hours must strictly increase the shortening default, at most 72')
    require(state['live']['parameters'].get('MonthlyBudgetUsd') == '170',
            'the original always-on beta budget must remain unchanged')
    require(state['policy'].get('budget_genui_cents') == 17000
            and state['policy'].get('budget_total_cents') == 20800,
            'canonical policy caps must remain 17000/20800 cents')
    identity = runner.aws.identity()
    operator_identity(identity, state)
    before = runner.boundary(1)
    inventory = no_pending_changesets(runner, state['authority']['stack_id'])
    after = runner.boundary(1)  # Both reads validate the same exact live baseline.
    reviewed_state(store, expected_state_sha256)
    clock = runner.clock()
    budget = preflight()
    config = copy.deepcopy(state['live']['config'])
    expiry = budget.timestamp(config['deployment_expires_at_utc']) - timedelta(hours=hours)
    require(expiry > clock + timedelta(hours=24), 'reviewed renewal required before expiry')
    # Stable for this exact reviewed transition. This is a projection, never a
    # real reservation, and its proposed ledger is deliberately discarded.
    projection_id = str(uuid.uuid5(uuid.NAMESPACE_URL, 'forma:expiry-policy:' + expected_state_sha256 + ':' + str(hours)))
    require(projection_id not in state['ledger']['windows'], 'projection identifier already exists')
    config.update(window_id=projection_id, supersedes_window_id=state['live']['window_id'],
                  service_desired_count=0, extra_task_hours=2,
                  deployment_expires_at_utc=expiry.strftime('%Y-%m-%dT%H:%M:%SZ'))
    ledger = copy.deepcopy(state['ledger'])
    _, prospective = budget.reserve(config, ledger, clock)
    require(prospective['operating_plan'] == 'always-on-beta'
            and prospective['genui_allocation_cents'] == 17000
            and prospective['monthly_budget_cents'] == 20800,
            'original 17000/20800 cent caps must remain unchanged')
    require(ledger == state['ledger'], 'budget validator mutated the original ledger')
    expected_after = copy.deepcopy(state)
    expected_after['policy']['shorten_expiry_hours'] = hours
    expected_after['revision'] += 1
    source_files = ['ops/promotion/expiry_policy.py', 'ops/promotion/runner.py',
                    'ops/promotion/guards.py', 'ops/promotion/state.py',
                    'ops/promotion/maintenance.py', 'ops/promotion/aws.py', 'infra/scripts/preflight.py']
    receipt_key = 'promotions/expiry-policy-' + expected_state_sha256 + '-' + str(hours) + '/intent.json'
    receipt = {'schema_version': 1, 'operation': 'increase-expiry-shortening-default',
        'observed_at_utc': clock.strftime('%Y-%m-%dT%H:%M:%SZ'),
        'before_state_sha256': expected_state_sha256, 'expected_after_state_sha256': digest(expected_after),
        'before_policy': state['policy'], 'after_policy': expected_after['policy'],
        'identity': identity, 'source_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'source_files_sha256': {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in source_files},
        'ledger_sha256': digest(state['ledger']), 'live_sha256': digest(state['live']),
        'boundary_before': before, 'boundary_after': after, 'change_set_inventory': inventory,
        'prospective_config': config, 'prospective_only': prospective,
        'limits': 'Future default only. No ledger reservation, live schedule change, cap increase, or billing refresh.'}
    public = {'status': 'planned', 'before_revision': state['revision'],
        'before_state_sha256': expected_state_sha256, 'expected_after_state_sha256': digest(expected_after),
        'previous_shortening_hours': old, 'shortening_hours': hours,
        'next_prospective_expiry': config['deployment_expires_at_utc'],
        'genui_cap_cents': prospective['genui_allocation_cents'],
        'project_cap_cents': prospective['monthly_budget_cents'],
        'cost_by_month': {month: {key: cost[key] for key in ('genui_total_cents', 'project_total_cents')}
                          for month, cost in prospective['cost_by_month'].items()},
        'windows_preserved': len(state['ledger']['windows']), 'live_changed': False, 'ledger_changed': False}
    return {'receipt_key': receipt_key, 'receipt': receipt, 'public': public}, expected_after


def run(runner, hours, expected_state_sha256, apply=False):
    result, expected_after = plan(runner, hours, expected_state_sha256)
    if not apply:
        return result
    store = runner.store
    # Recheck after observations, then retain immutable write-ahead evidence.
    # A duplicate key or unknown receipt outcome stops before the state CAS.
    reviewed_state(store, expected_state_sha256)
    receipt_sha256 = store.immutable(result['receipt_key'], result['receipt'])
    reviewed_state(store, expected_state_sha256)
    store.save(expected_after)  # Exactly one CAS; a lost ACK is never replayed.
    actual = store.load()
    require(actual == expected_after, 'policy write readback differs; reconcile before retry')
    result['public'].update(status='applied', revision=actual['revision'], state_sha256=digest(actual))
    result['receipt_sha256'] = receipt_sha256
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--state-bucket', required=True)
    parser.add_argument('--state-prefix', required=True)
    parser.add_argument('--hours', required=True, type=int)
    parser.add_argument('--expected-state-sha256', required=True)
    parser.add_argument('--output', required=True, type=Path, help='new private plan/result file; never overwritten')
    parser.add_argument('--apply', action='store_true', help='explicitly perform the reviewed policy-only CAS')
    args = parser.parse_args()
    try:
        # Reserve the private output before even constructing an AWS transport.
        # A nonexistent parent, permissions error or racing creator cannot fail
        # only after the policy CAS. Never overwrite a previous attempt.
        fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, 'wb') as output:
            try:
                aws = AWS()
                store = Store(aws.client('s3'), args.state_bucket, args.state_prefix)
                result = run(Runner(aws, store), args.hours, args.expected_state_sha256, args.apply)
                output.write(encoded(result))
                output.flush(); os.fsync(output.fileno())
            except Exception:
                # Retain the failed attempt without exception text, identities,
                # credentials, or an unsupported claim that no write occurred.
                try:
                    output.seek(0); output.truncate()
                    output.write(encoded({'status': 'refused-or-outcome-unknown',
                        'instruction': 'Inspect immutable intent and canonical state before retry; no automatic replay.'}))
                    output.flush(); os.fsync(output.fileno())
                except OSError:
                    pass  # A disk failure must not hide the original refusal.
                raise
        print(json.dumps(result['public'], sort_keys=True))
    except Exception:
        print('Expiry policy refused or write outcome unknown. Inspect the immutable intent and canonical state before retry; no operation was automatically repeated.', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
