"""Inspect or abandon a confirmed, never-executed promotion after fencing.

There is no automatic force-unlock. Unknown mutations or any execution require
operator reconciliation, not this command. Cancellation needs the original run
completed for 65 minutes, beyond the bootstrap roles' one-hour STS maximum.
"""
import argparse
import copy
from datetime import datetime, timedelta, timezone
import hashlib
import json
import re
import sys
from . import schema_release
from .aws import AWS, body
from .candidate import api
from .github import run_identity
from .runner import Runner, preflight
from .state import Store, closed, digest, encoded, no_duplicates, now, require
from .guards import ACCOUNT, IMAGE_KEYS, REGION, REPOSITORY, REPOS, ROLE_KEYS, definition, parameters, runtime_parameters, schedule, service


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
        operation_receipt(store, entry, window)
    return len(active['journal'])



def operation_receipt(store, entry, window):
    request = entry['request']; name = entry['name']
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
    closed(receipt, ['request', 'response', 'observed_at_utc'], 'operation receipt')
    require(receipt['request'] == request and isinstance(receipt['response'], dict), 'registry receipt request or response differs')
    observed = receipt['observed_at_utc']
    require(isinstance(observed, str) and re.fullmatch(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z', observed), 'registry receipt observation missing')
    datetime.fromisoformat(observed.replace('Z', '+00:00'))
    return receipt


def pre_outage_snapshot(active, state, store):
    """The one pre-outage mutation a prepared schema release may carry (APP-175).

    Execution takes the snapshot before it stops any service, with the phase
    still 'prepared'. A confirmed snapshot left nothing running differently, so
    it does not block abandonment; it is kept as evidence and never deleted.
    An unknown snapshot outcome is an intent and is refused like any other.
    """
    entries = [x for x in active['journal'] if x['name'].startswith('schema-')]
    if not entries:
        return None
    release = (active.get('packet') or {}).get('schema_release')
    require(active['phase'] == 'prepared' and release and len(entries) == 1
            and entries[0]['name'] == 'schema-snapshot' and entries[0]['status'] == 'confirmed',
            'only one confirmed pre-outage snapshot may be abandoned')
    identifier = schema_release.snapshot_identifier(active['promotion_id'], release['to_version'])
    expected = {'service': 'rds', 'operation': 'create_db_snapshot', 'arguments': {
        'DBInstanceIdentifier': schema_release.database_instance(state['live']['outputs'].get('DatabaseEndpoint')),
        'DBSnapshotIdentifier': identifier}}
    require(release['snapshot_identifier'] == identifier and entries[0]['request'] == expected, 'pre-outage snapshot request differs')
    operation_receipt(store, entries[0], active['promotion_id'])
    return identifier


def no_pending_changesets(runner, stack_id, absent_name=None):
    """Read every bounded inventory page; a partial or pending result refuses."""
    pages = []; token = None; seen = set()
    for _ in range(20):
        args = {'StackName': stack_id}
        if token: args['NextToken'] = token
        page = runner.read('cloudformation', 'list_change_sets', **args)
        require(isinstance(page.get('Summaries'), list), 'change-set inventory missing')
        for item in page['Summaries']:
            if absent_name:
                require(isinstance(item.get('ChangeSetName'), str) and item['ChangeSetName'] != absent_name,
                        'intended change set exists in inventory')
            require(item.get('Status') in ['CREATE_COMPLETE', 'FAILED', 'DELETE_COMPLETE']
                    and item.get('ExecutionStatus') in ['UNAVAILABLE', 'OBSOLETE', 'EXECUTE_COMPLETE', 'EXECUTE_FAILED'],
                    'pending change set requires explicit reconciliation')
        pages.append(page); token = page.get('NextToken')
        if not token: return pages
        require(isinstance(token, str) and token not in seen, 'change-set inventory pagination repeated')
        seen.add(token)
    raise RuntimeError('change-set inventory exceeds bounded complete read')


def exact_changeset_absent(runner, stack_id, name):
    try:
        runner.read('cloudformation', 'describe_change_set', StackName=stack_id, ChangeSetName=name)
    except Exception as error:
        response = getattr(error, 'response', {})
        require(response.get('Error', {}).get('Code') == 'ChangeSetNotFound'
                and response.get('ResponseMetadata', {}).get('HTTPStatusCode') == 404,
                'exact change-set absence was not established')
        return response
    raise RuntimeError('intended change set exists; use separate reconciliation')


def rejected_bootstrap_create(runner, active, state, run):
    """Correlate one definitive bootstrap-SSM rejection; never fake a success.

    CloudTrail may omit requestParameters. That absence is explicitly recorded:
    the exact request/token comes from the canonical journal, while AWS supplies
    the unique session/error/time event and authoritative change-set absence.
    """
    journal = active['journal']
    require(active['phase'] == 'preparing' and len(journal) >= 3
            and journal[-2]['name'] == 'template-upload' and journal[-2]['status'] == 'confirmed'
            and journal[-1]['name'] == 'changeset-create' and journal[-1]['status'] == 'intent'
            and all(x['status'] == 'confirmed' for x in journal[:-1]), 'not the bounded rejected-create shape')
    registry_active = copy.deepcopy(active); registry_active['journal'] = journal[:-2]
    count = confirmed_registry_only(registry_active, state, runner.store)
    window = active['promotion_id']; store = runner.store
    template_raw = encoded(state['baseline']['template']); template_sha = hashlib.sha256(template_raw).hexdigest()
    template_key = store.prefix + '/promotions/' + window + '/template.json'
    template_request = {'service':'s3','operation':'put_object','arguments':{
        'Bucket':store.bucket,'Key':template_key,'Body':{'bytes':len(template_raw),'sha256':template_sha},
        'ServerSideEncryption':'AES256','ContentType':'application/json','IfNoneMatch':'*'}}
    require(journal[-2]['request'] == template_request, 'template-upload request differs')
    operation_receipt(store, journal[-2], window)
    stored = store.s3.get_object(Bucket=store.bucket, Key=template_key)
    with stored['Body'] as stream: actual_template = stream.read(len(template_raw) + 1)
    require(actual_template == template_raw, 'actual immutable template differs from baseline')
    parameters = runtime_parameters(state['live']['parameters'], active['reservation'])
    arguments = {'StackName':state['authority']['stack_id'],'ChangeSetName':'forma-promotion-'+window,
        'ChangeSetType':'UPDATE','TemplateURL':f'https://{store.bucket}.s3.{REGION}.amazonaws.com/{template_key}',
        'Parameters':[{'ParameterKey':k,'ParameterValue':v} for k,v in sorted(parameters.items())],
        'Capabilities':['CAPABILITY_IAM'],'RoleARN':state['authority']['execution_role_arn'],
        'ClientToken':'prepare-'+window,'Description':'Qualified runtime promotion '+active['candidate_sha']}
    expected_request = {'service':'cloudformation','operation':'create_change_set','arguments':arguments}
    intent = journal[-1]
    require(intent['request'] == expected_request and intent.get('request_sha256') == digest(expected_request)
            and not intent.get('receipt') and not intent.get('receipt_sha256'), 'canonical create intent differs')
    start = datetime.fromisoformat(active['started_at_utc'].replace('Z','+00:00'))
    end = datetime.fromisoformat(run['updated_at'].replace('Z','+00:00'))
    intended = datetime.fromisoformat(intent['intended_at_utc'].replace('Z','+00:00'))
    require(start <= intended <= end and end-start <= timedelta(hours=2), 'create-call time window differs')
    role = state['authority']['prepare_role_arn']
    session_arn = f'arn:aws:sts::{ACCOUNT}:assumed-role/{role.rsplit("/",1)[-1]}/beta-plan-{active["authorization"]["run_id"]}'
    query = {'LookupAttributes':[{'AttributeKey':'EventName','AttributeValue':'CreateChangeSet'}],
             'StartTime':start,'EndTime':end,'MaxResults':50}
    pages=[]; matches=[]; token=None; seen=set(); total_bytes=0
    for _ in range(20):
        page = runner.read('cloudtrail','lookup_events',**dict(query,**({'NextToken':token} if token else {})))
        require(isinstance(page.get('Events'),list), 'CloudTrail lookup incomplete')
        total_bytes += len(encoded(page))
        require(total_bytes <= 8*1024*1024, 'CloudTrail proof exceeds bounded size')
        pages.append(page)
        for row in page['Events']:
            raw=row['CloudTrailEvent'];require(isinstance(raw,str) and len(raw.encode())<=1024*1024,'CloudTrail event exceeds bound')
            event=json.loads(raw,object_pairs_hook=no_duplicates)
            if event.get('userIdentity',{}).get('arn') == session_arn:
                require(row.get('EventId')==event.get('eventID'),'CloudTrail event identity differs')
                matches.append(event)
        token=page.get('NextToken')
        if not token: break
        require(isinstance(token,str) and token not in seen,'CloudTrail pagination repeated');seen.add(token)
        import time
        time.sleep(0.55)  # LookupEvents is limited to two requests/second.
    else: raise RuntimeError('CloudTrail lookup exceeds bounded complete read')
    require(len(matches)==1,'ambiguous or missing run-session CreateChangeSet event')
    event=matches[0]; who=event['userIdentity']
    happened=datetime.fromisoformat(event['eventTime'].replace('Z','+00:00'))
    require(event.get('eventName')=='CreateChangeSet' and event.get('eventSource')=='cloudformation.amazonaws.com'
        and event.get('awsRegion')==REGION and event.get('recipientAccountId')==ACCOUNT
        and event.get('eventType')=='AwsApiCall' and event.get('managementEvent') is True and event.get('readOnly') is False
        and who.get('type')=='AssumedRole' and who.get('accountId')==ACCOUNT
        and who.get('sessionContext',{}).get('sessionIssuer',{}).get('arn')==role
        and intended <= happened <= intended+timedelta(minutes=2) and happened <= end,
        'CloudTrail identity, action or time differs')
    execution_role=state['authority']['execution_role_arn'].rsplit('/',1)[-1]
    message=(f'User: arn:aws:sts::{ACCOUNT}:assumed-role/{execution_role}/AWSCloudFormation is not authorized to perform: '
        f'ssm:GetParameters on resource: arn:aws:ssm:{REGION}:{ACCOUNT}:parameter/cdk-bootstrap/hnb659fds/version '
        'because no identity-based policy allows the ssm:GetParameters action '
        '(Service: AWSSimpleSystemsManagement; Status Code: 400; Error Code: AccessDeniedException; Request ID: ')
    require(event.get('errorCode')=='ValidationException' and event.get('responseElements') is None
        and re.fullmatch(re.escape(message)+r'[0-9a-f-]{36}; Proxy: null\)',event.get('errorMessage','')),
        'not the definitive bootstrap-version permission rejection')
    observed_parameters=event.get('requestParameters')
    if observed_parameters is not None:
        expected_parameters={k[0].lower()+k[1:]:v for k,v in arguments.items()}
        expected_parameters['parameters']=[{'parameterKey':p['ParameterKey'],'parameterValue':p['ParameterValue']} for p in arguments['Parameters']]
        require(observed_parameters==expected_parameters,'CloudTrail request parameters differ')
    absent=exact_changeset_absent(runner,arguments['StackName'],arguments['ChangeSetName'])
    return {'outcome':'rejected-before-change-set-creation','registry_receipts_verified':count,
        'template_sha256':template_sha,'template_bytes':len(template_raw),
        'intended_request_sha256':digest(expected_request),'canonical_client_token':arguments['ClientToken'],
        'cloudtrail_request_parameters_present':observed_parameters is not None,
        'client_token_observed_in_cloudtrail':observed_parameters is not None,
        'cloudtrail_query':query,'cloudtrail_pages':pages,'cloudtrail_event':event,
        'exact_absence':absent,'change_set_name':arguments['ChangeSetName']}


def release_rolled_back_execution(runner, expected_state_sha256, read_run=api):
    """Release a lease whose change set executed and CloudFormation rolled it back.

    Activation must not have started. The app services must already be healthy on
    the previous images. This records the stack role the failed execution left
    behind and does not start, stop, or execute anything.
    """
    store = runner.store
    state = store.load()
    require(digest(state) == expected_state_sha256, 'state changed since operator review')
    active = state.get('active')
    require(active and active['phase'] == 'executing', 'only an executing promotion can be released after rollback')
    journal = active['journal']
    require(journal and all(x['status'] == 'confirmed' for x in journal), 'unknown operation must be reconciled')
    names = {x['name'] for x in journal}
    require({'stop-main', 'stop-broker', 'changeset-execute'} <= names, 'rollback release requires the stopped execution')
    require(not any(name.startswith('activate-') for name in names), 'activation started; release is refused')
    require('schema-migrate' not in names, 'a migration may have run; release is refused')
    identity = runner.aws.identity()
    require(not any('/' + state['authority'][key].rsplit('/', 1)[-1] + '/' in identity['Arn']
                    for key in ('prepare_role_arn', 'execute_role_arn')),
            'rollback release is an operator recovery command, not a workflow permission')
    run = read_run(f'repos/{REPOSITORY}/actions/runs/{active["authorization"]["run_id"]}')
    terminal_proof(run, active, runner.clock())
    store.resume(active['promotion_id'], active['authorization'])
    require(digest(store.document) == expected_state_sha256, 'state changed during fencing proof')
    stack = runner.stack()
    require(stack['StackStatus'] == 'UPDATE_ROLLBACK_COMPLETE', 'stack is not a completed rollback')
    authority_role = state['authority']['execution_role_arn']
    require(stack.get('RoleARN') == authority_role, 'stack role is not the promotion execution role')
    described = runner.read('cloudformation', 'describe_change_set', ChangeSetName=active['packet']['change_set_arn'], StackName=stack['StackId'])
    require(described.get('ExecutionStatus') in ['EXECUTE_FAILED', 'OBSOLETE', 'UNAVAILABLE'], 'change set is not a failed execution')
    state = store.document
    require(parameters(stack) == state['live']['parameters'], 'live stack parameters drifted')
    template = body(runner.read('cloudformation', 'get_template', StackName=stack['StackId'], TemplateStage='Original')['TemplateBody'])
    require(template == state['baseline']['template'], 'rollback did not restore the previous template')
    outputs = {p['OutputKey']: p['OutputValue'] for p in stack['Outputs']}
    for key, value in state['live']['outputs'].items():
        if key not in ROLE_KEYS:
            require(outputs.get(key) == value, 'retained output changed: ' + key)
    for key, role in ROLE_KEYS.items():
        current = runner.read('ecs', 'describe_task_definition', taskDefinition=outputs[key])['taskDefinition']
        definition(state['baseline']['definitions'][key], current, state['live']['parameters'][IMAGE_KEYS[role]])
    services = runner.read('ecs', 'describe_services', cluster=outputs['ClusterName'], services=[outputs['ServiceName'], outputs['SharedBrokerServiceName']])
    require(not services.get('failures') and len(services.get('services', [])) == 2, 'service lookup incomplete')
    cluster = f'arn:aws:ecs:{REGION}:{ACCOUNT}:cluster/{outputs["ClusterName"]}'
    for item in services['services']:
        broker = item['serviceName'] == outputs['SharedBrokerServiceName']
        name = outputs['SharedBrokerServiceName' if broker else 'ServiceName']
        arn = f'arn:aws:ecs:{REGION}:{ACCOUNT}:service/{outputs["ClusterName"]}/{name}'
        service(item, arn, cluster, outputs['SharedBrokerTaskDefinition' if broker else 'TaskDefinition'], 1)
    for previous in state['baseline']['schedules'].values():
        current = runner.read('scheduler', 'get_schedule', GroupName=previous['GroupName'], Name=previous['Name'])
        schedule(previous, current, state['live']['parameters']['DeploymentExpiresAtUtc'])
    ledger, receipt = preflight().abandon(store.document['ledger'], active['promotion_id'], runner.clock())
    receipt.update(reason='cloudformation update rolled back before activation', stack_status=stack['StackStatus'],
                   change_set_execution_status=described.get('ExecutionStatus'))
    store.immutable('promotions/' + active['promotion_id'] + '/rolled-back.json', receipt)
    def finish(document):
        document['ledger'] = ledger
        document['live']['stack_execution_role_arn'] = authority_role
        document['completed'][active['promotion_id']] = {'status': 'rolled-back', 'candidate_sha': active['candidate_sha'], 'observed_at_utc': now()}
        document['active'] = None
    store.change(finish)
    return {'status': 'rolled-back', 'promotion_id': active['promotion_id'], 'windows_preserved': len(ledger['windows'])}


def abandon(runner, expected_state_sha256, read_run=api, reconcile_failed_create=False):
    store=runner.store;state=store.load()
    require(digest(state)==expected_state_sha256, 'state changed since operator review')
    active=state.get('active')
    require(active and active['phase'] in ['prepared', 'preparing'], 'runtime execution cannot be abandoned')
    pre_changeset = active['phase'] == 'preparing'
    if not pre_changeset: require(active.get('packet'), 'fully prepared packet required')
    require(not reconcile_failed_create or pre_changeset, 'rejected-create recovery only applies before a packet')
    if not reconcile_failed_create:
        require(not any(x['status']!='confirmed' for x in active['journal']), 'unknown operation must be reconciled')
    require(not any(x['name'].startswith(('stop-', 'activate-', 'changeset-execute', 'schema-migrate')) for x in active['journal']), 'runtime execution cannot be abandoned')
    identity=runner.aws.identity()
    require(not any('/'+state['authority'][key].rsplit('/',1)[-1]+'/' in identity['Arn'] for key in ['prepare_role_arn','execute_role_arn']),
            'abandonment is an operator recovery command, not a workflow permission')
    run=read_run(f'repos/{REPOSITORY}/actions/runs/{active["authorization"]["run_id"]}')
    terminal_proof(run,active,runner.clock())
    if reconcile_failed_create: store.load()
    else: store.resume(active['promotion_id'],active['authorization'])
    require(digest(store.document)==expected_state_sha256,'state changed during fencing proof')
    reconciliation = rejected_bootstrap_create(runner,active,state,run) if reconcile_failed_create else None
    if reconcile_failed_create:
        # The original intent remains unchanged; only this verified rejection
        # permits operator ownership for the final abandonment CAS. No replay.
        store.owner=active['owner']
    verified_receipts = reconciliation['registry_receipts_verified'] if reconciliation else (confirmed_registry_only(active, state, store) if pre_changeset else None)
    retained_snapshot = None if pre_changeset else pre_outage_snapshot(active, state, store)
    absent_name = reconciliation['change_set_name'] if reconciliation else None
    before=runner.boundary(1)
    inventories = []
    if pre_changeset:
        inventories = no_pending_changesets(runner, before['stack']['StackId'], absent_name)
    else:
        packet=active['packet'];arn=packet['change_set_arn']
        actual=runner.read('cloudformation','describe_change_set',StackName=before['stack']['StackId'],ChangeSetName=arn)
        require(actual['ExecutionStatus']=='AVAILABLE' and actual['ChangeSetId']==arn,'prepared change set was executed or changed')
    store.immutable('promotions/'+active['promotion_id']+'/abandon-fence.json', {
        'run':run,'identity':identity,'observed_at_utc':now(),'before':before,
        'reviewed_state_sha256':expected_state_sha256,'phase':active['phase'],
        'journal':active['journal'],'change_set_inventory':inventories,
        'registry_receipts_verified':verified_receipts,'rejected_create_reconciliation':reconciliation,
        'retained_schema_snapshot':retained_snapshot})
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
    final_absence = exact_changeset_absent(runner,before['stack']['StackId'],absent_name) if reconciliation else None
    final_inventories = no_pending_changesets(runner, before['stack']['StackId'], absent_name) if pre_changeset else []
    ledger,receipt=preflight().abandon(store.document['ledger'],active['promotion_id'],runner.clock())
    receipt.update(abandoned_window=ledger['windows'][active['promotion_id']],
        previous_ledger_sha256=digest(store.document['ledger']), updated_ledger_sha256=digest(ledger),
        final_change_set_inventory=final_inventories,final_exact_change_set_absence=final_absence,
        retained_schema_snapshot=retained_snapshot)
    store.immutable('promotions/'+active['promotion_id']+'/abandoned.json',receipt)
    def finish(document):
        document['ledger']=ledger
        document['completed'][active['promotion_id']]={'status':'abandoned','candidate_sha':active['candidate_sha'],'observed_at_utc':now()}
        document['active']=None
    store.change(finish)
    return {'status':'abandoned','promotion_id':active['promotion_id'],'windows_preserved':len(ledger['windows'])}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command',choices=['inspect','abandon','release-rolled-back'])
    parser.add_argument('--state-bucket',required=True);parser.add_argument('--state-prefix',default='beta')
    parser.add_argument('--expected-state-sha256')
    parser.add_argument('--reconcile-bootstrap-create-rejection',action='store_true',help='Only the verified bootstrap-SSM CreateChangeSet rejection; no replay or general unknown-outcome bypass')
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
            runner=Runner(aws,store)
            if args.command=='release-rolled-back':
                result=release_rolled_back_execution(runner,args.expected_state_sha256)
            else:
                result=abandon(runner,args.expected_state_sha256,reconcile_failed_create=args.reconcile_bootstrap_create_rejection)
        print(json.dumps(result));return 0
    except Exception:
        print('Maintenance refused or outcome unresolved. Keep the lock and inspect private receipts.',file=sys.stderr);return 1


if __name__=='__main__':raise SystemExit(main())
