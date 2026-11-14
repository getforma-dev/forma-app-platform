"""Operator budget maintenance; never a timestamp-only refresh or deployment.

Observe a fresh account-level Cost Explorer response, inspect it, and provide an
explicit assessment bound to that snapshot and the current ledger. Attribution,
billing lag and future reserves remain operator judgments. This narrow command
cannot lower recorded money, rewrite windows, renew expiry or increase caps.
Workflow roles deliberately have no Cost Explorer permission.
"""
import argparse
import copy
from datetime import datetime, timedelta, timezone
from decimal import Decimal
import json
from pathlib import Path
import re
import sys
import uuid

from .aws import AWS
from .guards import ACCOUNT
from .runner import preflight
from .state import Store, closed, digest, encoded, no_duplicates, require

MONEY = ('prior_extra_spend_cents', 'variable_reserve_cents',
         'auth_spent_or_reserved_cents', 'unallocated_cents')


def timestamp(value):
    require(isinstance(value, str) and re.fullmatch(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z', value),
            'UTC timestamp required')
    return datetime.fromisoformat(value.replace('Z', '+00:00'))


def observe(aws, clock):
    identity = aws.identity()
    require(identity['Account'] == ACCOUNT, 'billing account differs')
    # Only completed UTC days. A month rollover needs its separately reviewed
    # new-month reservation; do not present yesterday's month as today's basis.
    require(clock.day > 1, 'no completed day in this month; use reviewed month renewal')
    query = {'TimePeriod': {'Start': clock.strftime('%Y-%m-01'), 'End': clock.strftime('%Y-%m-%d')},
             'Granularity': 'MONTHLY', 'Metrics': ['UnblendedCost'],
             'Filter': {'Dimensions': {'Key': 'LINKED_ACCOUNT', 'Values': [ACCOUNT]}}}
    response = aws.call('ce', 'get_cost_and_usage', **query)
    require(not response.get('NextPageToken'), 'billing pagination requires separate reconciliation')
    result = {'schema_version': 1, 'account': ACCOUNT, 'observed_at_utc': clock.strftime('%Y-%m-%dT%H:%M:%SZ'),
              'query': query, 'response': response}
    validate_observation(result, clock)
    return result


def validate_observation(value, clock):
    closed(value, ['schema_version', 'account', 'observed_at_utc', 'query', 'response'], 'billing observation')
    require(value['schema_version'] == 1 and value['account'] == ACCOUNT, 'foreign billing observation')
    observed = timestamp(value['observed_at_utc'])
    require(clock - timedelta(hours=1) <= observed <= clock, 'fresh billing observation required')
    require(observed.strftime('%Y-%m') == clock.strftime('%Y-%m') and observed.day > 1,
            'billing month differs or has no completed day')
    expected = {'TimePeriod': {'Start': observed.strftime('%Y-%m-01'), 'End': observed.strftime('%Y-%m-%d')},
                'Granularity': 'MONTHLY', 'Metrics': ['UnblendedCost'],
                'Filter': {'Dimensions': {'Key': 'LINKED_ACCOUNT', 'Values': [ACCOUNT]}}}
    require(value['query'] == expected, 'billing scope differs')
    response = value['response']
    require(not response.get('NextPageToken') and response.get('ResponseMetadata', {}).get('HTTPStatusCode') == 200
            and response['ResponseMetadata'].get('RequestId'), 'incomplete billing response')
    periods = response.get('ResultsByTime', [])
    require(len(periods) == 1 and periods[0].get('TimePeriod') == expected['TimePeriod']
            and not periods[0].get('Groups') and type(periods[0].get('Estimated')) is bool,
            'billing period incomplete')
    metric = periods[0].get('Total', {}).get('UnblendedCost', {})
    amount = Decimal(metric.get('Amount', 'NaN'))
    require(metric.get('Unit') == 'USD' and amount.is_finite(), 'billing amount invalid')
    # Credits can make the account total negative. They never lower this ledger.
    return str(amount)


def assessed_ledger(state, assessment, observation, clock):
    closed(assessment, ['month', 'ledger_sha256', 'observation_sha256', 'account_total_usd',
                       'rationale', 'billing_lag_and_query_cost_covered', 'amounts'], 'assessment')
    amount = validate_observation(observation, clock)
    month = clock.strftime('%Y-%m'); ledger = state['ledger']
    require(assessment['month'] == month and month in ledger['months'], 'only an existing current month may be reconciled')
    require(assessment['ledger_sha256'] == digest(ledger) and assessment['observation_sha256'] == digest(observation),
            'assessment is stale or bound to different evidence')
    require(assessment['account_total_usd'] == amount and assessment['billing_lag_and_query_cost_covered'] is True,
            'operator must assess the observed account total, billing lag and query cost')
    rationale = assessment['rationale']
    require(isinstance(rationale, str) and 40 <= len(rationale.strip()) <= 600
            and all(ord(c) >= 32 for c in rationale), 'a substantive single-line reserve assessment is required')
    closed(assessment['amounts'], MONEY, 'assessed amounts')
    for field in MONEY:
        proposed = assessment['amounts'][field]
        require(type(proposed) is int and ledger['months'][month][field] <= proposed <= preflight().PLANS['always-on-beta']['project_cap_cents'],
                'cannot reduce recorded money or exceed the existing cap')
    updated = copy.deepcopy(ledger)
    updated['months'][month].update(assessment['amounts'],
        reconciled_at_utc=observation['observed_at_utc'],
        basis='Cost Explorer observation sha256:' + digest(observation) + '; operator assessment: ' + rationale)
    return updated


def apply(store, assessment, observation, clock, verified_observation, validator=None):
    state = store.load()
    require(state.get('active') is None, 'deployment or recovery holds the canonical lock')
    ledger = assessed_ledger(state, assessment, observation, clock)
    validate_observation(verified_observation, clock)
    require(verified_observation['query'] == observation['query']
            and verified_observation['response']['ResultsByTime'] == observation['response']['ResultsByTime'],
            'fresh AWS billing response differs; inspect it and reassess')
    # The CLI obtains this second response directly from AWS. A synthetically
    # shaped local file is never the sole machine evidence for a basis refresh.
    ledger['months'][assessment['month']]['reconciled_at_utc'] = verified_observation['observed_at_utc']
    ledger['months'][assessment['month']]['basis'] = ('Cost Explorer verified observation sha256:'
        + digest(verified_observation) + '; operator assessment: ' + assessment['rationale'])
    # The actual existing window checks unchanged caps, reserve floors, expiry,
    # schema/trial bindings and all historical money. Discard its proposed ledger.
    validator = validator or preflight().reserve
    checked, _ = validator(state['live']['config'], ledger, clock)
    require(checked == ledger, 'reconciliation would change a deployment window')
    receipt = 'promotions/reconciliation-' + str(uuid.uuid4()) + '/assessment.json'
    receipt_sha = store.immutable(receipt, {'assessment': assessment, 'observation': observation,
        'verified_observation': verified_observation,
        'previous_ledger_sha256': digest(state['ledger']), 'updated_ledger_sha256': digest(ledger)})
    def update(document):
        require(document.get('active') is None, 'deployment lock changed')
        document['ledger'] = ledger
        document['last_reconciliation'] = {'receipt': receipt, 'sha256': receipt_sha,
                                           'month': assessment['month'], 'observed_at_utc': verified_observation['observed_at_utc']}
    # A racing release or lost response fails CAS. No automatic retry or overwrite.
    store.change(update)
    return {'status': 'reconciled', 'month': assessment['month'], 'ledger_sha256': digest(ledger),
            'windows_preserved': len(ledger['windows'])}


def read(path):
    return json.loads(path.read_text(), object_pairs_hook=no_duplicates)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['observe', 'apply'])
    parser.add_argument('--state-bucket', required=True)
    parser.add_argument('--state-prefix', default='beta')
    parser.add_argument('--observation', type=Path)
    parser.add_argument('--assessment', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    try:
        require(not args.output.exists(), 'never overwrite an observation or assessment result')
        aws = AWS(); aws.identity()
        store = Store(aws.client('s3'), args.state_bucket, args.state_prefix)
        clock = datetime.now(timezone.utc)
        if args.command == 'observe':
            state = store.load(); observation = observe(aws, clock)
            result = {'observation': observation, 'assessment_template': {
                'month': clock.strftime('%Y-%m'), 'ledger_sha256': digest(state['ledger']),
                'observation_sha256': digest(observation), 'account_total_usd': validate_observation(observation, clock),
                'rationale': '', 'billing_lag_and_query_cost_covered': False,
                'amounts': {k: state['ledger']['months'][clock.strftime('%Y-%m')][k] for k in MONEY}}}
        else:
            require(args.observation and args.assessment, 'observation and explicit assessment required')
            assessment = read(args.assessment); observation = read(args.observation)['observation']
            state = store.load()
            require(state.get('active') is None, 'deployment or recovery holds the canonical lock')
            assessed_ledger(state, assessment, observation, clock)
            verified_observation = observe(aws, clock)
            result = apply(store, assessment, observation, clock, verified_observation)
        with args.output.open('xb') as output:
            args.output.chmod(0o600); output.write(encoded(result))
        print(json.dumps({'status': 'observed' if args.command == 'observe' else result['status']}))
    except Exception:
        print('Reconciliation refused or write outcome unknown. Inspect private evidence and canonical state before retry.', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
