"""Raise only the idle canonical reservation ceiling from 18000/21800 to 21000/24800 cents.

The ADR-019 amendment of 2026-11-14 (the real-user trial) sets the November ceiling at $210
GenUI / $248 project; the first raise (17000/20800 to 18000/21800, ADR-019 of 2026-11-03)
used this same command. It does not deploy, change the live stop time, edit a window or
refresh billing. The CloudFormation MonthlyBudgetUsd tag stays 170. Run the basis refresh
separately. Without --apply it only plans: it reads the state and reports the pair it would
write; --apply performs the one conditional write and reads it back.
"""
import argparse
import os
from datetime import datetime, timezone

from .aws import AWS
from .expiry_policy import operator_identity
from .runner import preflight
from .state import Store, digest, require

PREVIOUS = (18000, 21800)
TARGET = (21000, 24800)


def target_caps():
    plans = preflight().PLANS['always-on-beta']
    caps = (plans['genui_cap_cents'], plans['project_cap_cents'])
    require(caps == TARGET and plans['monthly_budget_usd'] == '170',
            'code caps are not the reviewed 21000/24800 raise')
    return caps


def plan_caps(store, expected_state_sha256):
    """Read-only: the idle state this raise applies to, or a refusal."""
    require(isinstance(expected_state_sha256, str) and len(expected_state_sha256) == 64, 'exact reviewed state hash required')
    state = store.load()
    require(digest(state) == expected_state_sha256, 'state changed since review')
    require(state.get('active') is None, 'deployment or recovery holds the canonical lock')
    policy = state['policy']
    current = (policy.get('budget_genui_cents'), policy.get('budget_total_cents'))
    target = target_caps()
    require(current == PREVIOUS, 'policy caps are not the 18000/21800 pair this raise replaces')
    return {'status': 'planned', 'from': list(current), 'to': list(target), 'revision': state['revision'],
            'state_sha256': expected_state_sha256, 'windows_preserved': len(state['ledger']['windows'])}


def raise_caps(store, expected_state_sha256):
    plan_caps(store, expected_state_sha256)
    state = store.document
    target = TARGET
    ledger = state['ledger']

    def update(document):
        require(document.get('active') is None, 'deployment lock changed')
        require(document['ledger'] == ledger, 'ledger must stay unchanged')
        document['policy']['budget_genui_cents'] = target[0]
        document['policy']['budget_total_cents'] = target[1]

    store.change(update)
    result = store.load()
    require(result['ledger'] == ledger, 'ledger changed during the cap raise')
    require((result['policy']['budget_genui_cents'], result['policy']['budget_total_cents']) == target, 'cap readback differs')
    return {'status': 'caps-raised', 'genui_cap_cents': target[0], 'project_cap_cents': target[1],
            'revision': result['revision'], 'state_sha256': digest(result),
            'observed_at_utc': datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--state-bucket', required=True)
    parser.add_argument('--state-prefix', default='beta')
    parser.add_argument('--expected-state-sha256', required=True)
    parser.add_argument('--apply', action='store_true', help='perform the one conditional policy write; without it, plan only')
    args = parser.parse_args()
    try:
        require(os.environ.get('GITHUB_ACTIONS', '').lower() != 'true', 'cap raise is not a workflow operation')
        aws = AWS()
        store = Store(aws.client('s3'), args.state_bucket, args.state_prefix)
        operator_identity(aws.identity(), store.load())
        result = (raise_caps if args.apply else plan_caps)(store, args.expected_state_sha256)
        print(__import__('json').dumps(result))
        return 0
    except Exception:
        print('Cap raise refused or write outcome unknown. Inspect canonical state before retry.', file=__import__('sys').stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
