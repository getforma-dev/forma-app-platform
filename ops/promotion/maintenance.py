"""Inspect or abandon a fully prepared, never-executed promotion after fencing.

There is no automatic force-unlock. Unknown mutations or any execution require
operator reconciliation, not this command. Cancellation needs the original run
completed for 65 minutes, beyond the bootstrap roles' one-hour STS maximum.
"""
import argparse
from datetime import datetime, timedelta, timezone
import json
import sys
from .aws import AWS
from .candidate import api
from .github import run_identity
from .runner import Runner, preflight
from .state import Store, digest, now, require
from .guards import REPOSITORY


def terminal_proof(run, active, clock):
    run_identity(run, active['authorization']['run_id'], active['candidate_sha'])
    require(run.get('status') == 'completed', 'original workflow must be terminal')
    updated = datetime.fromisoformat(run['updated_at'].replace('Z', '+00:00'))
    require(updated.tzinfo is not None and updated + timedelta(minutes=65) <= clock,
            'wait until original workflow credentials are expired before abandonment')


def abandon(runner, expected_state_sha256, read_run=api):
    store=runner.store;state=store.load()
    require(digest(state)==expected_state_sha256, 'state changed since operator review')
    active=state.get('active')
    require(active and active['phase']=='prepared' and active.get('packet'), 'only a fully prepared promotion can be abandoned')
    require(not any(x['status']!='confirmed' for x in active['journal']), 'unknown operation must be reconciled')
    require(not any(x['name'].startswith(('stop-', 'activate-', 'changeset-execute')) for x in active['journal']), 'runtime execution cannot be abandoned')
    identity=runner.aws.identity()
    require(not any('/'+state['authority'][key].rsplit('/',1)[-1]+'/' in identity['Arn'] for key in ['prepare_role_arn','execute_role_arn']),
            'abandonment is an operator recovery command, not a workflow permission')
    run=read_run(f'repos/{REPOSITORY}/actions/runs/{active["authorization"]["run_id"]}')
    terminal_proof(run,active,runner.clock())
    store.resume(active['promotion_id'],active['authorization'])
    require(digest(store.document)==expected_state_sha256,'state changed during fencing proof')
    before=runner.boundary(1)
    packet=active['packet'];arn=packet['change_set_arn']
    actual=runner.read('cloudformation','describe_change_set',StackName=before['stack']['StackId'],ChangeSetName=arn)
    require(actual['ExecutionStatus']=='AVAILABLE' and actual['ChangeSetId']==arn,'prepared change set was executed or changed')
    store.immutable('promotions/'+active['promotion_id']+'/abandon-fence.json', {'run':run,'identity':identity,'observed_at_utc':now(),'before':before})
    runner.mutate('changeset-delete','cloudformation','delete_change_set',StackName=before['stack']['StackId'],ChangeSetName=arn)
    # Absence, not merely DeleteChangeSet acceptance, is required before release.
    for _ in range(30):
        inventory=runner.read('cloudformation','list_change_sets',StackName=before['stack']['StackId'])
        require(not inventory.get('NextToken'),'change-set inventory incomplete')
        if not any(x.get('ChangeSetId')==arn for x in inventory['Summaries']):break
        import time
        time.sleep(2)
    else:raise RuntimeError('change-set deletion not yet confirmed; retain lease')
    runner.boundary(1)
    ledger,receipt=preflight().abandon(store.document['ledger'],active['promotion_id'],runner.clock())
    store.immutable('promotions/'+active['promotion_id']+'/abandoned.json',receipt)
    def finish(document):
        document['ledger']=ledger
        document['completed'][active['promotion_id']]={'status':'abandoned','candidate_sha':active['candidate_sha'],'observed_at_utc':now()}
        document['active']=None
    store.change(finish)
    return {'status':'abandoned','promotion_id':active['promotion_id'],'windows_preserved':len(ledger['windows'])}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command',choices=['inspect','abandon'])
    parser.add_argument('--state-bucket',required=True);parser.add_argument('--state-prefix',default='beta')
    parser.add_argument('--expected-state-sha256')
    args=parser.parse_args()
    try:
        aws=AWS();aws.identity();store=Store(aws.client('s3'),args.state_bucket,args.state_prefix)
        if args.command=='inspect':
            state=store.load();active=state.get('active')
            result={'state_sha256':digest(state),'revision':state['revision'],'active':None if not active else {
                'promotion_id':active['promotion_id'],'candidate_sha':active['candidate_sha'],'phase':active['phase'],
                'unresolved_operations':sum(x['status']=='intent' for x in active['journal'])}}
        else:
            require(args.expected_state_sha256,'reviewed state digest required')
            result=abandon(Runner(aws,store),args.expected_state_sha256)
        print(json.dumps(result));return 0
    except Exception:
        print('Maintenance refused or outcome unresolved. Keep the lock and inspect private receipts.',file=sys.stderr);return 1


if __name__=='__main__':raise SystemExit(main())
