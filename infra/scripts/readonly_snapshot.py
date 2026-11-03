#!/usr/bin/env python3
"""APP-078 read-only observation of the GenUI synthetic foundation.

Every AWS call goes through ``ReadOnlyClient``, which admits only the
operations named in ``ALLOWED`` (describe/get/list reads) and refuses any
other attribute before a request exists. Nothing here creates, updates,
deletes, starts, stops, runs, puts or executes anything. One Cost Explorer
``GetCostAndUsage`` request is made (AWS charges $0.01 for it).

Usage (boto3 from the locked runtime environment, default credentials):

    app/runtime/.venv/bin/python infra/scripts/readonly_snapshot.py \
        --private-dir .cache/operations/beta/app-078-prep/snapshot \
        --summary evidence/APP-078/prep/snapshot.json

The private directory receives every raw response (it holds endpoints and is
gitignored); the summary keeps identities, statuses, counts, digests, costs
and request ids, with the database endpoint, operator address and raw
environment removed.
"""
import argparse
import importlib.util
from datetime import datetime, timezone
import json
import os
from pathlib import Path

# The beta's account, region and stack, shared with preflight.py (loaded by path, not sys.path).
_spec = importlib.util.spec_from_file_location('genui_infra_common', Path(__file__).resolve().parent / 'common.py')
_COMMON = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_COMMON)
STACK, REGION, ACCOUNT = _COMMON.STACK, _COMMON.REGION, _COMMON.ACCOUNT
# service -> the only boto3 methods this tool may call.
ALLOWED = {
    'sts': {'get_caller_identity'},
    'cloudformation': {'describe_stacks', 'get_stack_policy', 'list_change_sets', 'list_stack_resources'},
    'ecs': {'describe_clusters', 'describe_services', 'list_tasks', 'describe_tasks', 'describe_task_definition'},
    'rds': {'describe_db_instances', 'describe_db_snapshots'},
    'ecr': {'describe_repositories', 'describe_images'},
    'scheduler': {'get_schedule'},
    'elbv2': {'describe_load_balancers', 'describe_listeners', 'describe_target_groups', 'describe_target_health'},
    'acm': {'describe_certificate'},
    'budgets': {'describe_budgets'},
    'ce': {'get_cost_and_usage'},
    'secretsmanager': {'list_secrets'},
}
REDACTED = '<private: see .cache/operations/beta>'


class Refused(Exception):
    pass


class ReadOnlyClient:
    """A boto3 client proxy that exposes only allowlisted read operations."""

    def __init__(self, service, client, record):
        if service not in ALLOWED:
            raise Refused('service not allowlisted: ' + service)
        self._service, self._client, self._record = service, client, record

    def __getattr__(self, name):
        if name not in ALLOWED[self._service]:
            raise Refused('operation not allowlisted: %s.%s' % (self._service, name))
        method = getattr(self._client, name)

        def call(**kwargs):
            response = method(**kwargs)
            meta = response.get('ResponseMetadata', {})
            self._record.append({'service': self._service, 'operation': name,
                                 'request_id': meta.get('RequestId'), 'http_status': meta.get('HTTPStatusCode')})
            return {k: v for k, v in response.items() if k != 'ResponseMetadata'}
        return call


def default_clients(record):
    import boto3  # imported lazily so tests need no AWS SDK
    session = boto3.Session(region_name=REGION)
    return lambda service: ReadOnlyClient(service, session.client(service), record)


def iso(value):
    return value.astimezone(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ') if isinstance(value, datetime) else value


def observe(client, now):
    """Collect raw responses. ``client(service)`` returns a ReadOnlyClient."""
    raw = {}
    raw['caller'] = client('sts').get_caller_identity()
    if raw['caller'].get('Account') != ACCOUNT:
        raise Refused('unexpected account')
    cfn = client('cloudformation')
    raw['stack'] = cfn.describe_stacks(StackName=STACK)['Stacks'][0]
    raw['stack_policy'] = cfn.get_stack_policy(StackName=STACK).get('StackPolicyBody')
    raw['change_sets'] = cfn.list_change_sets(StackName=STACK).get('Summaries', [])
    resources, token = [], None
    while True:
        page = cfn.list_stack_resources(StackName=STACK, **({'NextToken': token} if token else {}))
        resources += page['StackResourceSummaries']
        token = page.get('NextToken')
        if not token:
            break
    raw['stack_resources'] = resources
    outputs = {o['OutputKey']: o['OutputValue'] for o in raw['stack'].get('Outputs', [])}
    physical = {r['LogicalResourceId']: r.get('PhysicalResourceId') for r in resources}
    by_type = lambda kind: [r for r in resources if r['ResourceType'] == kind]

    ecs = client('ecs')
    cluster = outputs['ClusterName']
    raw['cluster'] = ecs.describe_clusters(clusters=[cluster])
    services = [r['PhysicalResourceId'] for r in by_type('AWS::ECS::Service')]
    raw['services'] = ecs.describe_services(cluster=cluster, services=services)
    raw['tasks'] = {}
    for status in ('RUNNING', 'STOPPED'):
        arns = ecs.list_tasks(cluster=cluster, desiredStatus=status).get('taskArns', [])
        raw['tasks'][status] = ecs.describe_tasks(cluster=cluster, tasks=arns).get('tasks', []) if arns else []
    raw['task_definitions'] = {}
    for key in ('TaskDefinition', 'MigrationTaskDefinition', 'QualificationTaskDefinition', 'OperatorTaskDefinition'):
        if key in outputs:
            raw['task_definitions'][key] = ecs.describe_task_definition(taskDefinition=outputs[key])['taskDefinition']

    rds = client('rds')
    instances = [r['PhysicalResourceId'] for r in by_type('AWS::RDS::DBInstance')]
    raw['databases'] = [rds.describe_db_instances(DBInstanceIdentifier=i)['DBInstances'][0] for i in instances]
    raw['db_snapshots'] = {i: rds.describe_db_snapshots(DBInstanceIdentifier=i).get('DBSnapshots', []) for i in instances}

    ecr = client('ecr')
    raw['repositories'] = ecr.describe_repositories().get('repositories', [])
    raw['images'] = {}
    for repository in raw['repositories']:
        name = repository['repositoryName']
        raw['images'][name] = ecr.describe_images(repositoryName=name).get('imageDetails', [])

    arn = outputs.get('DeploymentExpiryScheduleArn', '')
    if arn.count('/') == 2:
        _, group, name = arn.rsplit('/', 2)
        raw['schedule'] = client('scheduler').get_schedule(GroupName=group, Name=name)

    elb = client('elbv2')
    balancers = [r['PhysicalResourceId'] for r in by_type('AWS::ElasticLoadBalancingV2::LoadBalancer')]
    raw['load_balancers'] = elb.describe_load_balancers(LoadBalancerArns=balancers).get('LoadBalancers', []) if balancers else []
    raw['listeners'] = elb.describe_listeners(LoadBalancerArn=balancers[0]).get('Listeners', []) if balancers else []
    groups = [r['PhysicalResourceId'] for r in by_type('AWS::ElasticLoadBalancingV2::TargetGroup')]
    raw['target_health'] = {g: elb.describe_target_health(TargetGroupArn=g).get('TargetHealthDescriptions', []) for g in groups}

    parameters = {p['ParameterKey']: p.get('ParameterValue') for p in raw['stack'].get('Parameters', [])}
    certificate = parameters.get('TlsCertificateArn')
    if certificate:
        raw['certificate'] = client('acm').describe_certificate(CertificateArn=certificate)['Certificate']

    raw['budgets'] = client('budgets').describe_budgets(AccountId=ACCOUNT).get('Budgets', [])
    start = now.replace(day=1).strftime('%Y-%m-%d')
    end = now.strftime('%Y-%m-%d')
    if end == start:
        raw['cost_month_to_date'] = None
    else:
        raw['cost_month_to_date'] = client('ce').get_cost_and_usage(
            TimePeriod={'Start': start, 'End': end}, Granularity='MONTHLY', Metrics=['UnblendedCost'],
            GroupBy=[{'Type': 'DIMENSION', 'Key': 'SERVICE'}]).get('ResultsByTime', [])
    raw['qualification_secrets'] = client('secretsmanager').list_secrets(
        Filters=[{'Key': 'name', 'Values': ['genui-qualification-']}]).get('SecretList', [])
    raw['physical'] = physical
    return raw


def summarize(raw, calls, now):
    stack = raw['stack']
    parameters = {p['ParameterKey']: p.get('ParameterValue') for p in stack.get('Parameters', [])}
    if 'OperatorIpv4Cidr' in parameters:
        parameters['OperatorIpv4Cidr'] = REDACTED
    outputs = {o['OutputKey']: o['OutputValue'] for o in stack.get('Outputs', [])}
    for key in ('DatabaseEndpoint',):
        if key in outputs:
            outputs[key] = REDACTED
    services = []
    for service in raw['services'].get('services', []):
        services.append({k: service.get(k) for k in ('serviceName', 'serviceArn', 'status', 'desiredCount', 'runningCount',
                                                     'pendingCount', 'taskDefinition', 'launchType', 'platformVersion')}
                        | {'deployment_configuration': service.get('deploymentConfiguration'),
                           'deployments': [{k: iso(d.get(k)) for k in ('status', 'taskDefinition', 'desiredCount', 'runningCount', 'rolloutState', 'createdAt')}
                                           for d in service.get('deployments', [])]})
    definitions = {}
    for key, definition in raw['task_definitions'].items():
        definitions[key] = {'arn': definition['taskDefinitionArn'], 'revision': definition['revision'], 'status': definition['status'],
                            'cpu': definition.get('cpu'), 'memory': definition.get('memory'),
                            'runtime_platform': definition.get('runtimePlatform'),
                            'containers': [{'name': c['name'], 'image': c['image'], 'user': c.get('user'),
                                            'readonly_root': c.get('readonlyRootFilesystem'),
                                            'entry_point': c.get('entryPoint', []), 'command': c.get('command', []),
                                            'environment_names': sorted(e['name'] for e in c.get('environment', [])),
                                            'secret_names': sorted(s['name'] for s in c.get('secrets', []))}
                                           for c in definition['containerDefinitions']]}
    databases = []
    for db in raw['databases']:
        databases.append({'identifier': db['DBInstanceIdentifier'], 'resource_id': db['DbiResourceId'], 'arn': db['DBInstanceArn'],
                          'status': db['DBInstanceStatus'], 'engine': db['Engine'], 'engine_version': db['EngineVersion'],
                          'class': db['DBInstanceClass'], 'storage_gib': db['AllocatedStorage'], 'storage_type': db.get('StorageType'),
                          'storage_encrypted': db['StorageEncrypted'], 'publicly_accessible': db['PubliclyAccessible'],
                          'multi_az': db['MultiAZ'], 'iam_auth': db['IAMDatabaseAuthenticationEnabled'],
                          'deletion_protection': db['DeletionProtection'], 'auto_minor_version_upgrade': db['AutoMinorVersionUpgrade'],
                          'backup_retention_days': db['BackupRetentionPeriod'], 'latest_restorable_time': iso(db.get('LatestRestorableTime')),
                          'endpoint': REDACTED,
                          'snapshots': [{'id': s['DBSnapshotIdentifier'], 'type': s['SnapshotType'], 'status': s['Status'],
                                         'created': iso(s.get('SnapshotCreateTime')), 'encrypted': s['Encrypted'],
                                         'engine_version': s['EngineVersion']}
                                        for s in raw['db_snapshots'].get(db['DBInstanceIdentifier'], [])]})
    images = {}
    for repository in raw['repositories']:
        name = repository['repositoryName']
        images[name] = {'uri': repository['repositoryUri'], 'tag_mutability': repository.get('imageTagMutability'),
                        'scan_on_push': repository.get('imageScanningConfiguration', {}).get('scanOnPush'),
                        'images': sorted(({'digest': i['imageDigest'], 'tags': i.get('imageTags', []), 'pushed_at': iso(i.get('imagePushedAt')),
                                           'size_bytes': i.get('imageSizeInBytes'), 'media_type': i.get('imageManifestMediaType')}
                                          for i in raw['images'].get(name, [])), key=lambda i: i['pushed_at'] or '')}
    tasks = {status: [{'task_arn': t['taskArn'], 'group': t.get('group'), 'last_status': t.get('lastStatus'),
                       'task_definition': t.get('taskDefinitionArn'), 'created_at': iso(t.get('createdAt')),
                       'stopped_at': iso(t.get('stoppedAt')), 'stop_code': t.get('stopCode')}
                      for t in raw['tasks'][status]] for status in raw['tasks']}
    schedule = raw.get('schedule')
    schedule_summary = None if schedule is None else {
        'arn': schedule['Arn'], 'state': schedule['State'], 'expression': schedule['ScheduleExpression'],
        'timezone': schedule.get('ScheduleExpressionTimezone'), 'flexible_window': schedule.get('FlexibleTimeWindow'),
        'target_arn': schedule['Target']['Arn'], 'target_input': json.loads(schedule['Target']['Input']),
        'retry_policy': schedule['Target'].get('RetryPolicy')}
    balancers = [{'name': b['LoadBalancerName'], 'dns_name': b['DNSName'], 'state': b['State']['Code'], 'scheme': b['Scheme']}
                 for b in raw['load_balancers']]
    listeners = [{'port': l['Port'], 'protocol': l['Protocol'], 'ssl_policy': l.get('SslPolicy'),
                  'certificates': [c['CertificateArn'] for c in l.get('Certificates', [])]} for l in raw['listeners']]
    target_health = {g: [{'target': d['Target']['Id'], 'state': d['TargetHealth']['State']} for d in v] for g, v in raw['target_health'].items()}
    certificate = raw.get('certificate')
    certificate_summary = None if certificate is None else {
        'arn': certificate['CertificateArn'], 'domain': certificate['DomainName'], 'status': certificate['Status'],
        'not_after': iso(certificate.get('NotAfter')), 'in_use_by': certificate.get('InUseBy', []),
        'renewal_eligibility': certificate.get('RenewalEligibility')}
    budgets = [{'name': b['BudgetName'], 'limit': b.get('BudgetLimit'), 'time_unit': b.get('TimeUnit'),
                'actual': b.get('CalculatedSpend', {}).get('ActualSpend'), 'forecast': b.get('CalculatedSpend', {}).get('ForecastedSpend')}
               for b in raw['budgets']]
    cost = None
    if raw.get('cost_month_to_date'):
        period = raw['cost_month_to_date'][0]
        rows = [{'service': g['Keys'][0], 'usd': g['Metrics']['UnblendedCost']['Amount']} for g in period.get('Groups', [])]
        cost = {'start': period['TimePeriod']['Start'], 'end_exclusive': period['TimePeriod']['End'],
                'estimated': period.get('Estimated'), 'by_service': sorted(rows, key=lambda r: -float(r['usd'])),
                'total_usd': '%.2f' % sum(float(r['usd']) for r in rows)}
    return {
        'schema_version': 1,
        'scope': 'APP-078 preparation: read-only observation (describe/get/list only; no mutation)',
        'observed_at_utc': now.strftime('%Y-%m-%dT%H:%M:%SZ'),
        'caller_account': raw['caller']['Account'],
        'stack': {'name': stack['StackName'], 'id': stack['StackId'], 'status': stack['StackStatus'],
                  'last_updated': iso(stack.get('LastUpdatedTime')), 'termination_protection': stack.get('EnableTerminationProtection'),
                  'parameters': parameters, 'outputs': outputs,
                  'stack_policy': json.loads(raw['stack_policy']) if raw.get('stack_policy') else None,
                  'change_sets': [{'name': c['ChangeSetName'], 'status': c['Status'], 'execution_status': c.get('ExecutionStatus')} for c in raw['change_sets']],
                  'resource_count': len(raw['stack_resources'])},
        'services': services, 'task_definitions': definitions, 'tasks': tasks,
        'databases': databases, 'ecr': images, 'stop_schedule': schedule_summary,
        'load_balancers': balancers, 'listeners': listeners, 'target_health': target_health,
        'certificate': certificate_summary, 'budgets': budgets, 'cost_month_to_date': cost,
        'qualification_secrets': [s['Name'] for s in raw['qualification_secrets']],
        'requests': calls,
        'limits': ['Read-only snapshot at one instant; not a billing statement (Cost Explorer amounts are estimates and lag).',
                   'The database endpoint and operator address are kept only in the private raw file.'],
    }


def write_private(path, payload):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'w') as stream:
        json.dump(payload, stream, indent=2, default=iso, sort_keys=True)
        stream.write('\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--private-dir', type=Path, required=True)
    parser.add_argument('--summary', type=Path, required=True)
    args = parser.parse_args()
    now = datetime.now(timezone.utc)
    calls = []
    raw = observe(default_clients(calls), now)
    stamp = now.strftime('%Y%m%dT%H%M%SZ')
    write_private(args.private_dir / ('snapshot-raw-%s.json' % stamp), raw)
    summary = summarize(raw, calls, now)
    args.summary.parent.mkdir(parents=True, exist_ok=True)
    args.summary.write_text(json.dumps(summary, indent=2, default=iso) + '\n')
    print(json.dumps({'observed_at_utc': summary['observed_at_utc'], 'requests': len(calls), 'summary': str(args.summary)}))


if __name__ == '__main__':
    main()
