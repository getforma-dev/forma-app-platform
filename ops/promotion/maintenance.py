"""Inspect or abandon a confirmed, never-executed promotion after fencing.

There is no automatic force-unlock. Unknown mutations or any execution require
operator reconciliation, not this command. Cancellation needs the original run
completed for 65 minutes, beyond the bootstrap roles' one-hour STS maximum.
"""
import argparse
from datetime import datetime, timedelta, timezone
import hashlib
import json
import re
import sys
from .aws import AWS
from .candidate import api
from .github import run_identity
from .runner import Runner, preflight
from .state import Store, closed, digest, no_duplicates, now, require
from .guards import REPOSITORY, REPOS


def terminal_proof(run, active, clock):
    run_identity(run, active['authorization']['run_id'], active['candidate_sha'])
    require(run.get('status') == 'completed', 'original workflow must be terminal')
    updated = datetime.fromisoformat(run['updated_at'].replace('Z', '+00:00'))
    require(updated.tzinfo is not None and updated + timedelta(minutes=65) <= clock,
            'wait until original workflow credentials are expired before abandonment')


# A PutImage receipt may contain the JSON-escaped 4 MiB manifest. Other ECR
# operation responses are small; never read an arbitrarily large S3 object.
MAX_REGISTRY_RECEIPT_BYTES = 12 * 1024 * 1024


def confirmed_registry_only(active, state, store):
    """Positive allowlist for the pre-template ECR journal, never a phase bypass."""
    require(active.get('packet') is None and not active.get('packet_sha256'), 'preparation already has a packet')
    window = active['promotion_id']
    require(active.get('config', {}).get('window_id') == window
            and active.get('reservation', {}).get('window_id') == window
            and window in state['ledger']['windows'], 'preserved reservation required')
    require(active['journal'], 'confirmed registry operations required')
    for entry in active['journal']:
        name = entry['name']; request = entry['request']
        manifest = re.fullmatch(r'(service|qualification|operator)-manifest-([0-9a-f]{16})', name)
        layer = re.fullmatch(r'(service|qualification|operator)-([0-9a-f]{16})-(begin|complete|part-(0|[1-9][0-9]*))', name)
        match = manifest or layer
        require(match is not None and isinstance(request, dict), 'only confirmed ECR uploads may be abandoned before a change set')
        role, prefix = match[1], match[2]
        require(request.get('repositoryName') == REPOS[role], 'registry repository differs')
        kind = 'manifest' if manifest else match[3]
        keys = {'repositoryName', 'digest'}
        if kind == 'manifest': keys |= {'tag', 'manifest_sha256'}
        elif kind == 'complete': keys |= {'uploadId'}
        elif kind.startswith('part-'): keys = {'repositoryName', 'uploadId', 'partFirstByte', 'partLastByte', 'part_sha256'}
        require(set(request) == keys, 'registry operation fields differ')
        if kind.startswith('part-'):
            first, last = request['partFirstByte'], request['partLastByte']
            require(type(first) is int and type(last) is int and first == int(match[4])
                    and 0 <= first <= last < first + 5 * 1024 * 1024
                    and re.fullmatch(r'[0-9a-f]{64}', request['part_sha256']), 'registry part differs')
        else:
            require(re.fullmatch(r'sha256:' + prefix + r'[0-9a-f]{48}', request['digest']), 'registry digest differs')
        if 'uploadId' in request:
            require(isinstance(request['uploadId'], str) and 0 < len(request['uploadId']) <= 256, 'registry upload identity differs')
        if kind == 'manifest':
            require(request['tag'] in [None, 'promote-' + active['candidate_sha']]
                    and request['manifest_sha256'] == request['digest'][7:], 'manifest binding differs')
        require(entry.get('request_sha256') == digest(request)
                and entry.get('receipt') == 'promotions/' + window + '/' + name + '.json'
                and re.fullmatch(r'[0-9a-f]{64}', entry.get('receipt_sha256', '')), 'confirmed registry receipt missing or changed')
        try:
            response = store.s3.get_object(Bucket=store.bucket, Key=store.prefix + '/' + entry['receipt'])
            with response['Body'] as stream:
                raw = stream.read(MAX_REGISTRY_RECEIPT_BYTES + 1)
        except Exception:
            raise RuntimeError('confirmed registry receipt unavailable; keep the lock') from None
        require(len(raw) <= MAX_REGISTRY_RECEIPT_BYTES
                and hashlib.sha256(raw).hexdigest() == entry['receipt_sha256'], 'registry receipt bytes changed or oversized')
        receipt = json.loads(raw, object_pairs_hook=no_duplicates)
        closed(receipt, ['request', 'response', 'observed_at_utc'], 'registry receipt')
        require(receipt['request'] == request and isinstance(receipt['response'], dict), 'registry receipt request or response differs')
        observed = receipt['observed_at_utc']
        require(isinstance(observed, str) and re.fullmatch(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z', observed), 'registry receipt observation missing')
        datetime.fromisoformat(observed.replace('Z', '+00:00'))
    return len(active['journal'])



def no_pending_changesets(runner, stack_id):
    """Read every bounded inventory page; a partial or pending result refuses."""
    pages = []; token = None; seen = set()
    for _ in range(20):
        args = {'StackName': stack_id}
        if token: args['NextToken'] = token
        page = runner.read('cloudformation', 'list_change_sets', **args)
        require(isinstance(page.get('Summaries'), list), 'change-set inventory missing')
        for item in page['Summaries']:
            require(item.get('Status') in ['CREATE_COMPLETE', 'FAILED', 'DELETE_COMPLETE']
                    and item.get('ExecutionStatus') in ['UNAVAILABLE', 'OBSOLETE', 'EXECUTE_COMPLETE', 'EXECUTE_FAILED'],
                    'pending change set requires explicit reconciliation')
        pages.append(page); token = page.get('NextToken')
        if not token: return pages
        require(isinstance(token, str) and token not in seen, 'change-set inventory pagination repeated')
        seen.add(token)
    raise RuntimeError('change-set inventory exceeds bounded complete read')


def abandon(runner, expected_state_sha256, read_run=api):
    store=runner.store;state=store.load()
    require(digest(state)==expected_state_sha256, 'state changed since operator review')
    active=state.get('active')
    require(active and active['phase'] in ['prepared', 'preparing'], 'runtime execution cannot be abandoned')
    pre_changeset = active['phase'] == 'preparing'
    if not pre_changeset: require(active.get('packet'), 'fully prepared packet required')
    require(not any(x['status']!='confirmed' for x in active['journal']), 'unknown operation must be reconciled')
    require(not any(x['name'].startswith(('stop-', 'activate-', 'changeset-execute')) for x in active['journal']), 'runtime execution cannot be abandoned')
    identity=runner.aws.identity()
    require(not any('/'+state['authority'][key].rsplit('/',1)[-1]+'/' in identity['Arn'] for key in ['prepare_role_arn','execute_role_arn']),
            'abandonment is an operator recovery command, not a workflow permission')
    run=read_run(f'repos/{REPOSITORY}/actions/runs/{active["authorization"]["run_id"]}')
    terminal_proof(run,active,runner.clock())
    store.resume(active['promotion_id'],active['authorization'])
    require(digest(store.document)==expected_state_sha256,'state changed during fencing proof')
    verified_receipts = confirmed_registry_only(active, state, store) if pre_changeset else None
    before=runner.boundary(1)
    inventories = []
    if pre_changeset:
        inventories = no_pending_changesets(runner, before['stack']['StackId'])
    else:
        packet=active['packet'];arn=packet['change_set_arn']
        actual=runner.read('cloudformation','describe_change_set',StackName=before['stack']['StackId'],ChangeSetName=arn)
        require(actual['ExecutionStatus']=='AVAILABLE' and actual['ChangeSetId']==arn,'prepared change set was executed or changed')
    store.immutable('promotions/'+active['promotion_id']+'/abandon-fence.json', {
        'run':run,'identity':identity,'observed_at_utc':now(),'before':before,
        'reviewed_state_sha256':expected_state_sha256,'phase':active['phase'],
        'journal':active['journal'],'change_set_inventory':inventories,
        'registry_receipts_verified':verified_receipts})
    if not pre_changeset:
        runner.mutate('changeset-delete','cloudformation','delete_change_set',StackName=before['stack']['StackId'],ChangeSetName=arn)
        # Absence, not merely DeleteChangeSet acceptance, precedes release.
        for _ in range(30):
            inventory=runner.read('cloudformation','list_change_sets',StackName=before['stack']['StackId'])
            require(not inventory.get('NextToken'),'change-set inventory incomplete')
            if not any(x.get('ChangeSetId')==arn for x in inventory['Summaries']):break
            import time
            time.sleep(2)
        else:raise RuntimeError('change-set deletion not yet confirmed; retain lease')
    runner.boundary(1)
    final_inventories = no_pending_changesets(runner, before['stack']['StackId']) if pre_changeset else []
    ledger,receipt=preflight().abandon(store.document['ledger'],active['promotion_id'],runner.clock())
    receipt.update(abandoned_window=ledger['windows'][active['promotion_id']],
        previous_ledger_sha256=digest(store.document['ledger']), updated_ledger_sha256=digest(ledger),
        final_change_set_inventory=final_inventories)
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
