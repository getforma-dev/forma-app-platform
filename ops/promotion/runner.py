"""Prepare a concrete runtime-only packet; execute only that approved packet.

All state and raw AWS responses remain private in S3. Public output contains
candidate/revision/cost/status only. An interrupted mutation never auto-repeats.
"""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import time
from datetime import datetime, timedelta, timezone
import urllib.error
import urllib.request
import uuid
from . import guards, schema_release
from .aws import body
from .registry import publish, guard_scan_delta
from .state import digest, encoded, now, require

ROOT = Path(__file__).resolve().parents[2]


def preflight():
    spec = importlib.util.spec_from_file_location('promotion_preflight', ROOT / 'infra/scripts/preflight.py')
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module


def public_probes(origin='https://beta.getforma.dev'):
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, *args, **kwargs):
            return None
    opener = urllib.request.build_opener(NoRedirect())
    probes = [('live', '/health/live', 'GET', [200]), ('ready', '/health/ready', 'GET', [200]),
              ('entry', '/invite', 'GET', [200]), ('canvas', '/canvas', 'GET', [404]),
              ('mcp', '/v1/mcp', 'POST', [404]), ('slack', '/v1/slack/commands', 'POST', [401, 404])]
    result = {}
    for name, path, method, statuses in probes:
        request = urllib.request.Request(origin + path, data=b'' if method == 'POST' else None, method=method)
        try:
            response = opener.open(request, timeout=15)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            data = response.read(512 * 1024 + 1)
            require(len(data) <= 512 * 1024 and response.status in statuses, 'public probe failed: ' + name)
            if name in ['canvas', 'mcp']:
                require(json.loads(data) == {'code': 'not_found', 'status': 404}, 'uniform denial changed')
            result[name] = {'status': response.status, 'body_sha256': hashlib.sha256(data).hexdigest()}
    return result


SEQUENCE = ['stop both services', 'execute reviewed change set', 'verify stopped boundaries', 'activate both services', 'verify health and images']
SCHEMA_SEQUENCE = ['snapshot database'] + SEQUENCE[:3] + ['run reviewed migration'] + SEQUENCE[3:]
# Execute time budget (APP-175). The execute job has 60 minutes, which also
# cover checkout, artifact download and install before this step, and its role
# session lasts 3600 s from just before it. The per-wait bounds alone already
# exceed that: stop settle 900 + stack update 1200 + settle 900 + activation
# settle 900 + health 900 = 4800 s, and a schema release adds its snapshot
# (720 s) and migration (720 s). So execute carries one 55-minute budget and
# clips every wait to what remains; once it is spent the run refuses with the
# lease held instead of being killed mid-call by the job or an expired session.
# Each AWS wait can overrun its bound by one 10 s poll and one call. No outage
# starts with less than OUTAGE_RESERVE_SECONDS left: at least the migration
# bound plus the observed ~4-minute normal execute, with margin. The worst case
# before the outage is the snapshot bound plus the rechecks, which leaves
# 3300 - 720 = 2580 s > 1800 s.
EXECUTE_BUDGET_SECONDS = 55 * 60
OUTAGE_RESERVE_SECONDS = 30 * 60
STACK_UPDATE_WAIT_SECONDS = 1200
SERVICE_SETTLE_WAIT_SECONDS = 900


class Runner:
    def __init__(self, aws, store, clock=lambda: datetime.now(timezone.utc), probes=public_probes, repo=None):
        # repo is the candidate checkout; the workflow runs from its root.
        self.aws, self.store, self.clock, self.probes, self.repo = aws, store, clock, probes, repo
        self.deadline = None

    def bounded(self, seconds):
        """A wait bound clipped to the execute budget; refuses once the budget is spent."""
        if self.deadline is None:
            return seconds
        remaining = self.deadline - time.monotonic()
        require(remaining > 0, 'execute time budget spent; retain lease and reconcile')
        return min(seconds, remaining)

    def read(self, service, operation, /, **arguments):
        return self.aws.call(service, operation, **arguments)

    def mutate(self, name, service, operation, /, **arguments):
        receipt_arguments = {k: ({'bytes': len(v), 'sha256': hashlib.sha256(v).hexdigest()} if isinstance(v, bytes) else v) for k, v in arguments.items()}
        return self.store.operation(name, {'service': service, 'operation': operation, 'arguments': receipt_arguments},
                                    lambda: self.aws.call(service, operation, **arguments))

    def caller(self, phase):
        identity = self.aws.identity()
        role = self.store.document['authority'][phase + '_role_arn'].rsplit('/', 1)[-1]
        require(identity['Arn'].startswith(f'arn:aws:sts::{guards.ACCOUNT}:assumed-role/{role}/'), 'unexpected promotion role')

    def stack(self):
        return self.read('cloudformation', 'describe_stacks', StackName=self.store.document['authority']['stack_id'])['Stacks'][0]

    def boundary(self, count, expected_parameters=None, after=False, require_settled=True):
        # A reviewed schema can make the predecessor image unable to serve.
        # Identity and desired count still have to match. Health is required
        # after activation, when require_settled stays at its default.
        state = self.store.document; stack = self.stack(); outputs = guards.outputs(stack)
        expected = expected_parameters or state['live']['parameters']
        require(guards.parameters(stack) == expected, 'live stack parameters drifted')
        expected_role = state['authority']['execution_role_arn'] if after else state['live']['stack_execution_role_arn']
        require(stack.get('RoleARN') == expected_role, 'stack execution role differs')
        template = body(self.read('cloudformation', 'get_template', StackName=stack['StackId'], TemplateStage='Original')['TemplateBody'])
        require(template == state['baseline']['template'], 'runtime promotion cannot change template')
        guards.retained_outputs(state['live']['outputs'], outputs, expected)
        definitions = {}
        for key, role in guards.ROLE_KEYS.items():
            td = self.read('ecs', 'describe_task_definition', taskDefinition=outputs[key])['taskDefinition']
            guards.definition(state['baseline']['definitions'][key], td, expected[guards.IMAGE_KEYS[role]])
            definitions[key] = td
        services = self.read('ecs', 'describe_services', cluster=outputs['ClusterName'],
                             services=[outputs['ServiceName'], outputs['SharedBrokerServiceName']])
        require(not services.get('failures') and len(services.get('services', [])) == 2, 'service lookup incomplete')
        cluster = f"arn:aws:ecs:{guards.REGION}:{guards.ACCOUNT}:cluster/{outputs['ClusterName']}"
        health = []; running_tasks = []
        for service in services['services']:
            broker = service['serviceName'] == outputs['SharedBrokerServiceName']
            name = outputs['SharedBrokerServiceName' if broker else 'ServiceName']
            arn = f"arn:aws:ecs:{guards.REGION}:{guards.ACCOUNT}:service/{outputs['ClusterName']}/{name}"
            guards.service(service, arn, cluster, outputs['SharedBrokerTaskDefinition' if broker else 'TaskDefinition'], count, settled=require_settled)
            if count and not require_settled:
                listed = self.read('ecs', 'list_tasks', cluster=outputs['ClusterName'], serviceName=name, desiredStatus='RUNNING')
                require(not listed.get('nextToken') and len(listed.get('taskArns') or []) <= 1, 'more than one running task')
                if listed.get('taskArns'):
                    actual = self.read('ecs', 'describe_tasks', cluster=outputs['ClusterName'], tasks=listed['taskArns'])
                    require(not actual.get('failures') and len(actual.get('tasks', [])) == 1, 'running task unavailable')
                    task = actual['tasks'][0]
                    container = (task.get('containers') or [{}])[0]
                    image_role = 'operator' if broker else 'service'
                    registry = state['active']['packet']['registry'] if after else state['live']['registry']
                    image = registry[image_role]
                    if task.get('lastStatus') == 'RUNNING' and container.get('imageDigest'):
                        require(task.get('taskDefinitionArn') == service['taskDefinition']
                                and container.get('image') == expected[guards.IMAGE_KEYS[image_role]]
                                and container.get('imageDigest') in [image['manifest_digest'], image['index_digest']],
                                'running image digest differs')
            if count and require_settled:
                listed = self.read('ecs', 'list_tasks', cluster=outputs['ClusterName'], serviceName=name, desiredStatus='RUNNING')
                require(not listed.get('nextToken') and len(listed['taskArns']) == 1, 'expected one running task')
                actual = self.read('ecs', 'describe_tasks', cluster=outputs['ClusterName'], tasks=listed['taskArns'])
                require(not actual.get('failures') and len(actual.get('tasks', [])) == 1, 'running task unavailable')
                task = actual['tasks'][0]
                require(task['lastStatus'] == 'RUNNING' and task['taskDefinitionArn'] == service['taskDefinition']
                        and len(task['containers']) == 1, 'running task boundary differs')
                image_role = 'operator' if broker else 'service'
                registry = state['active']['packet']['registry'] if after else state['live']['registry']
                image = registry[image_role]
                container = task['containers'][0]
                require(container['image'] == expected[guards.IMAGE_KEYS[image_role]]
                        and container.get('imageDigest') in [image['manifest_digest'], image['index_digest']]
                        and container.get('lastStatus') == 'RUNNING'
                        and container.get('healthStatus') == 'HEALTHY', 'running image digest or container health differs')
                running_tasks.append(actual)
                for load_balancer in service['loadBalancers']:
                    result = self.read('elbv2', 'describe_target_health', TargetGroupArn=load_balancer['targetGroupArn'])
                    require(result['TargetHealthDescriptions'] and all(t['TargetHealth']['State'] == 'healthy' for t in result['TargetHealthDescriptions']), 'load balancer not healthy')
                    health.append(result)
        schedules = {}
        for key, previous in state['baseline']['schedules'].items():
            schedule = self.read('scheduler', 'get_schedule', GroupName=previous['GroupName'], Name=previous['Name'])
            guards.schedule(previous, schedule, expected['DeploymentExpiresAtUtc']); schedules[key] = schedule
        return {'stack': stack, 'outputs': outputs, 'definitions': definitions, 'services': services, 'schedules': schedules, 'health': health, 'running_tasks': running_tasks}

    def reviewed_release(self, source):
        """None for an unchanged schema and contract; otherwise the one reviewed release, or Refused."""
        state = self.store.document
        if all(source[key] == state['baseline'][key] for key in schema_release.DIGESTS):
            return None
        return schema_release.applicable(source, state['baseline'], state['policy']['schema_version'],
                                         self.repo if self.repo is not None else Path.cwd(), self.clock())

    def bundle(self, directory, request):
        from .artifacts import validate_bundle
        require(hashlib.sha256((directory / 'manifest.json').read_bytes()).hexdigest() == request['qualified_artifact_manifest_sha256'], 'artifact manifest bytes differ')
        bundle = validate_bundle(directory, request['candidate_sha'])
        return bundle, self.reviewed_release(bundle['source'])

    def same_release(self, release, packet, promotion):
        expected = packet.get('schema_release')
        if expected is None:
            return release is None
        return release is not None and dict(release, snapshot_identifier=schema_release.snapshot_identifier(promotion, release['to_version'])) == expected

    def prepare(self, request, directory):
        guards.request(request, 'prepare')
        from .candidate import check_current
        check_current(request['candidate_sha'])
        self.store.load(); self.caller('prepare')
        bundle, release = self.bundle(directory, request)
        promotion = str(uuid.uuid4())
        if release:
            release = dict(release, snapshot_identifier=schema_release.snapshot_identifier(promotion, release['to_version']))
        self.store.acquire(promotion, request['authorization'], request['candidate_sha'])
        before = self.boundary(1, require_settled=False)
        pending = self.read('cloudformation', 'list_change_sets', StackName=before['stack']['StackId'])
        require(not pending.get('NextToken'), 'change-set inventory needs explicit reconciliation')
        require(not any(x.get('ExecutionStatus') in ['AVAILABLE', 'EXECUTE_IN_PROGRESS'] for x in pending['Summaries']), 'another change set is pending')
        state = self.store.document; config = copy.deepcopy(state['live']['config'])
        expiry = preflight().timestamp(config['deployment_expires_at_utc']) - timedelta(hours=state['policy']['shorten_expiry_hours'])
        require(expiry > self.clock() + timedelta(hours=24), 'reviewed renewal required before expiry')
        config.update(window_id=promotion, supersedes_window_id=state['live']['window_id'], service_desired_count=0,
                      extra_task_hours=2, deployment_expires_at_utc=expiry.strftime('%Y-%m-%dT%H:%M:%SZ'))
        for role, image in bundle['images'].items():
            config[{'service': 'image_digest', 'qualification': 'qualification_image_digest', 'operator': 'operator_image_digest'}[role]] = f"{guards.ACCOUNT}.dkr.ecr.{guards.REGION}.amazonaws.com/{guards.REPOS[role]}@{image['index_digest']}"
        ledger, receipt = preflight().reserve(config, state['ledger'], self.clock())
        self.store.change(lambda d: (d.update(ledger=ledger), d['active'].update(config=config, reservation=receipt,
                          artifact_manifest_sha256=request['qualified_artifact_manifest_sha256'])))
        registry = publish(self.aws, self.store, directory, bundle)
        scan_delta = guard_scan_delta(registry, state['live']['registry'],
            state['baseline']['deployment_contract_sha256'], self.clock())
        # A disposition binds the baseline contract; the approval summary binds the
        # candidate's. A schema release changes the contract, so it cannot carry one.
        require(not release or not any(delta['introduced'] for delta in scan_delta.values()),
                'a schema release cannot introduce scan findings; separate reviewed release required')
        check_current(request['candidate_sha'])
        parameters = guards.runtime_parameters(guards.parameters(before['stack']), receipt)
        template = state['baseline']['template']; template_bytes = encoded(template); template_hash = hashlib.sha256(template_bytes).hexdigest()
        template_key = self.store.prefix + '/promotions/' + promotion + '/template.json'
        self.mutate('template-upload', 's3', 'put_object', Bucket=self.store.bucket, Key=template_key,
                    Body=template_bytes, ServerSideEncryption='AES256', ContentType='application/json', IfNoneMatch='*')
        create = {'StackName': before['stack']['StackId'], 'ChangeSetName': 'forma-promotion-' + promotion,
                  'ChangeSetType': 'UPDATE', 'TemplateURL': f'https://{self.store.bucket}.s3.{guards.REGION}.amazonaws.com/{template_key}',
                  'Parameters': [{'ParameterKey': k, 'ParameterValue': v} for k, v in sorted(parameters.items())],
                  'Capabilities': ['CAPABILITY_IAM'], 'RoleARN': state['authority']['execution_role_arn'],
                  'ClientToken': 'prepare-' + promotion, 'Description': 'Qualified runtime promotion ' + request['candidate_sha']}
        created = self.mutate('changeset-create', 'cloudformation', 'create_change_set', **create)
        arn = created['Id']
        described = self.aws.wait('cloudformation', 'describe_change_set', lambda r: r['Status'] in ['CREATE_COMPLETE', 'FAILED'],
                                   ChangeSetName=arn, StackName=before['stack']['StackId'])
        guards.changeset(described, parameters, state['authority']['execution_role_arn'], template, create)
        observed_template = body(self.read('cloudformation', 'get_template', ChangeSetName=arn, StackName=before['stack']['StackId'], TemplateStage='Original')['TemplateBody'])
        require(observed_template == template, 'actual change-set template differs')
        packet = {'candidate_sha': request['candidate_sha'], 'promotion_id': promotion,
                  'artifact_manifest_sha256': request['qualified_artifact_manifest_sha256'], 'bundle': bundle,
                  'parameters': parameters, 'config': config, 'reservation': receipt, 'registry': registry,
                  'scan_delta': scan_delta, 'template_sha256': template_hash, 'create_request': create,
                  'change_set_arn': arn, 'change_set': described, 'before': before, 'schema_release': release}
        packet_sha = digest(packet)
        self.store.immutable('promotions/' + promotion + '/packet.json', packet)
        self.store.change(lambda d: d['active'].update(phase='prepared', packet=packet, packet_sha256=packet_sha))
        cost = receipt['cost_by_month'][self.clock().strftime('%Y-%m')]
        return {'status': 'prepared', 'candidate_sha': request['candidate_sha'], 'promotion_id': promotion,
                'packet_sha256': packet_sha,
                'review': {'images': {role: bundle['images'][role]['index_digest'] for role in guards.REPOS},
                    'predecessor_candidate_sha': state['live']['candidate_sha'], 'source_tree_sha': bundle['source']['tree'],
                    'deployment_contract_sha256': bundle['source']['deployment_contract_sha256'],
                    'schema_version': state['policy']['schema_version'],
                    'schema_release': release and {key: release[key] for key in ['from_version', 'to_version', 'spec_sha256']},
                    'preserved': dict(dict.fromkeys(['schema', 'template', 'trial', 'aliases', 'secrets'], True), schema=not release),
                    'changes': [{'logical_id': c['ResourceChange']['LogicalResourceId'], 'resource_type': c['ResourceChange']['ResourceType'],
                                 'action': c['ResourceChange']['Action'], 'replacement': c['ResourceChange']['Replacement']} for c in described['Changes']],
                    'scan': scan_delta,
                    'sequence': SCHEMA_SEQUENCE if release else SEQUENCE,
                    'outage_required': True},
                'cost_genui_cents': cost['genui_total_cents'],
                'cost_total_cents': cost['project_total_cents'], 'expires_at_utc': config['deployment_expires_at_utc']}

    def snapshot_schema(self, promotion, packet, outputs):
        """The pre-migration snapshot, taken while beta still serves (APP-175).

        It runs after every pre-execution recheck and before any service is
        touched, with the phase still 'prepared'. A refused, failed or slow
        snapshot therefore leaves beta running: the workflow fails before the
        outage, nothing retries, and `maintenance abandon` may release the
        prepared release after its fence, keeping the snapshot. An unknown
        CreateDBSnapshot outcome stays an unresolved intent for reconciliation.
        Writes between the snapshot and the stop are not in it; the instance's
        automated backups cover that interval.
        """
        release = packet['schema_release']
        identifier = release['snapshot_identifier']
        require(identifier == schema_release.snapshot_identifier(promotion, release['to_version']), 'snapshot identity differs')
        instance = schema_release.database_instance(outputs.get('DatabaseEndpoint'))
        # Never start a mutation whose outcome the remaining budget cannot observe.
        bound = self.bounded(schema_release.SNAPSHOT_WAIT_SECONDS)
        require(bound == schema_release.SNAPSHOT_WAIT_SECONDS, 'too little execute time left to observe the snapshot')
        created = self.mutate('schema-snapshot', 'rds', 'create_db_snapshot', DBInstanceIdentifier=instance, DBSnapshotIdentifier=identifier)
        snapshot = created.get('DBSnapshot') or {}
        require(snapshot.get('DBSnapshotIdentifier') == identifier and snapshot.get('DBInstanceIdentifier') == instance,
                'snapshot identity differs; no service was stopped')
        observed = self.aws.wait('rds', 'describe_db_snapshots', schema_release.snapshot_settled,
                                 timeout=self.bounded(bound), retry_codes={'DBSnapshotNotFound'},
                                 DBSnapshotIdentifier=identifier)
        self.store.immutable('promotions/' + promotion + '/schema-snapshot-observed.json', observed)
        schema_release.verify_snapshot(observed, identifier, instance)

    def migrate_schema(self, promotion, packet, after):
        """Run the reviewed migration once on the candidate image; services stay stopped.

        Runs after the stopped-after-execute boundary and before activation intent.
        Any failure raises with phase 'executing' and the lease held: no activation,
        no retry and no replay. The migration runner is one transaction, so a failed
        migration leaves the database at its prior version.
        """
        release = packet['schema_release']; outputs = after['outputs']
        request = schema_release.migration_request(outputs, promotion)
        definition = after['definitions']['MigrationTaskDefinition']
        require(definition['taskDefinitionArn'] == request['taskDefinition'], 'migration task definition differs from the verified boundary')
        require(any(x['name'] == 'schema-snapshot' and x['status'] == 'confirmed' for x in self.store.document['active']['journal']),
                'no confirmed pre-migration snapshot')
        bound = self.bounded(schema_release.MIGRATION_WAIT_SECONDS)
        require(bound == schema_release.MIGRATION_WAIT_SECONDS, 'too little execute time left to observe the migration')
        started = self.mutate('schema-migrate', 'ecs', 'run_task', **request)
        task = schema_release.started_task(started, request)
        stopped = self.aws.wait('ecs', 'describe_tasks', lambda r: schema_release.task_stopped(r, task),
                                timeout=self.bounded(bound), cluster=request['cluster'], tasks=[task])
        self.store.immutable('promotions/' + promotion + '/schema-migration-observed.json', stopped)
        service = packet['registry']['service']
        schema_release.verify_migration(stopped, task, request, definition['containerDefinitions'][0]['name'],
            packet['parameters'][guards.IMAGE_KEYS['service']], [service['manifest_digest'], service['index_digest']])
        record = dict(release, migration_task_arn=task)
        self.store.immutable('promotions/' + promotion + '/schema-release.json', record)
        return record

    def settle_services(self, outputs, require_healthy=False):
        names = [outputs['ServiceName'], outputs['SharedBrokerServiceName']]
        self.aws.wait('ecs', 'describe_services', lambda r: not r.get('failures') and len(r.get('services', [])) == 2
                      and all(s['runningCount'] == s['desiredCount'] and s['pendingCount'] == 0
                              and len(s['deployments']) == 1 and s['deployments'][0].get('rolloutState') == 'COMPLETED'
                              for s in r['services']),
                      timeout=self.bounded(SERVICE_SETTLE_WAIT_SECONDS), cluster=outputs['ClusterName'], services=names)
        if not require_healthy:
            return
        # A stopped service has no container to become healthy. Wait only after activation.
        deadline = time.monotonic() + self.bounded(SERVICE_SETTLE_WAIT_SECONDS)
        while True:
            ready = True
            for name in names:
                listed = self.read('ecs', 'list_tasks', cluster=outputs['ClusterName'], serviceName=name, desiredStatus='RUNNING')
                tasks = listed.get('taskArns') or []
                if listed.get('nextToken') or len(tasks) != 1:
                    ready = False
                    break
                described = self.read('ecs', 'describe_tasks', cluster=outputs['ClusterName'], tasks=tasks)
                containers = ((described.get('tasks') or [{}])[0].get('containers') or [{}])
                if described.get('failures') or containers[0].get('healthStatus') != 'HEALTHY':
                    ready = False
                    break
            if ready:
                return
            require(time.monotonic() < deadline, 'bounded wait expired; retain lease and reconcile')
            time.sleep(10)

    def final_checks(self, request, packet, promotion):
        """Recheck the original reservation, basis age, exact live state, actual
        ARN/template/parameters, trunk, scan delta and schema release immediately
        before the outage. Read-only."""
        preflight().reserve(packet['config'], self.store.document['ledger'], self.clock())
        before = self.boundary(1, require_settled=False)
        described = self.read('cloudformation', 'describe_change_set', ChangeSetName=packet['change_set_arn'], StackName=before['stack']['StackId'])
        guards.changeset(described, packet['parameters'], self.store.document['authority']['execution_role_arn'], self.store.document['baseline']['template'], packet['create_request'])
        template = body(self.read('cloudformation', 'get_template', ChangeSetName=packet['change_set_arn'], StackName=before['stack']['StackId'], TemplateStage='Original')['TemplateBody'])
        require(digest(template) == packet['template_sha256'], 'actual prepared template changed')
        from .candidate import check_current
        check_current(request['candidate_sha'])
        require(guard_scan_delta(packet['registry'], self.store.document['live']['registry'],
            self.store.document['baseline']['deployment_contract_sha256'], self.clock()) == packet['scan_delta'],
            'scan disposition expired during final execution checks')
        require(self.same_release(self.reviewed_release(packet['bundle']['source']), packet, promotion),
            'schema release expired or changed during final execution checks')
        return before

    def execute(self, request, directory, approval):
        guards.request(request, 'execute')
        self.deadline = time.monotonic() + EXECUTE_BUDGET_SECONDS
        self.store.resume(request['promotion_id'], request['authorization']); self.caller('execute')
        active = self.store.document['active']; require(active['phase'] == 'prepared', 'phase cannot be executed again')
        require(active['packet_sha256'] == request['packet_sha256'] and digest(active['packet']) == request['packet_sha256'], 'prepared packet differs')
        packet = active['packet']; _, release = self.bundle(directory, request)
        require(packet['artifact_manifest_sha256'] == request['qualified_artifact_manifest_sha256'] and packet['candidate_sha'] == request['candidate_sha'], 'candidate differs from approved packet')
        require(self.same_release(release, packet, active['promotion_id']), 'schema release changed after preparation')
        require(guard_scan_delta(packet['registry'], self.store.document['live']['registry'],
            self.store.document['baseline']['deployment_contract_sha256'], self.clock()) == packet['scan_delta'],
            'scan disposition changed after preparation')
        from .github import validate_approval
        validate_approval(approval, request['authorization']['run_id'], request['candidate_sha'])
        self.store.immutable('promotions/' + active['promotion_id'] + '/approval.json', approval)
        before = self.final_checks(request, packet, active['promotion_id'])
        if packet.get('schema_release'):
            # Snapshot before the outage, then recheck everything again so the
            # outage still starts immediately after a complete recheck.
            self.snapshot_schema(active['promotion_id'], packet, before['outputs'])
            before = self.final_checks(request, packet, active['promotion_id'])
        require(self.deadline - time.monotonic() >= OUTAGE_RESERVE_SECONDS, 'too little execute time left to start the outage')
        self.store.change(lambda d: d['active'].update(phase='executing', approval_sha256=digest(approval)))
        outputs = before['outputs']
        for label, key in [('main', 'ServiceName'), ('broker', 'SharedBrokerServiceName')]:
            self.mutate('stop-' + label, 'ecs', 'update_service', cluster=outputs['ClusterName'], service=outputs[key], desiredCount=0)
        self.settle_services(outputs); stopped = self.boundary(0)
        self.store.immutable('promotions/' + active['promotion_id'] + '/stopped-before-execute.json', stopped)
        self.mutate('changeset-execute', 'cloudformation', 'execute_change_set', ChangeSetName=packet['change_set_arn'],
                    StackName=before['stack']['StackId'], ClientRequestToken='execute-' + active['promotion_id'])
        completed = self.aws.wait('cloudformation', 'describe_stacks', lambda r: r['Stacks'][0]['StackStatus'] not in ['UPDATE_IN_PROGRESS', 'UPDATE_COMPLETE_CLEANUP_IN_PROGRESS'],
                                 timeout=self.bounded(STACK_UPDATE_WAIT_SECONDS), StackName=before['stack']['StackId'])['Stacks'][0]
        require(completed['StackStatus'] == 'UPDATE_COMPLETE', 'stack update failed; services remain stopped, reconcile')
        self.settle_services(guards.outputs(completed))
        after = self.boundary(0, packet['parameters'], after=True)
        self.store.immutable('promotions/' + active['promotion_id'] + '/stopped-after-execute.json', after)
        migrated = None
        if packet.get('schema_release'):
            migrated = self.migrate_schema(active['promotion_id'], packet, after)
            # The migration took minutes; prove nothing else moved before activation intent.
            after = self.boundary(0, packet['parameters'], after=True)
            self.store.immutable('promotions/' + active['promotion_id'] + '/stopped-after-migration.json', after)
        activation_config = dict(packet['config'], service_desired_count=1)
        ledger, _ = preflight().reserve(activation_config, self.store.document['ledger'], self.clock(), activating=True)
        self.store.change(lambda d: (d.update(ledger=ledger), d['active'].update(phase='activation-intended', activation_requested_at_utc=now())))
        # Explicit writes are journaled independently. Both exact future schedules
        # and all task/secret/alias boundaries have just passed boundary().
        require(preflight().timestamp(activation_config['deployment_expires_at_utc']) > self.clock() + timedelta(minutes=10), 'expiry too close')
        outputs = after['outputs']
        for label, key in [('broker', 'SharedBrokerServiceName'), ('main', 'ServiceName')]:
            self.mutate('activate-' + label, 'ecs', 'update_service', cluster=outputs['ClusterName'], service=outputs[key], desiredCount=1)
        self.settle_services(outputs, require_healthy=True)
        healthy = self.boundary(1, packet['parameters'], after=True)
        probes = self.probes()
        tag = 'beta/' + self.clock().strftime('%Y%m%d') + '-' + request['candidate_sha'][:7]
        summary = {'status': 'verified', 'candidate_sha': request['candidate_sha'], 'promotion_id': active['promotion_id'],
                   'packet_sha256': request['packet_sha256'], 'activated_at_utc': now(), 'tag': tag,
                   'service_revision': int(outputs['TaskDefinition'].rsplit(':', 1)[1]),
                   'broker_revision': int(outputs['SharedBrokerTaskDefinition'].rsplit(':', 1)[1])}
        verified = {'summary': summary, 'healthy': healthy, 'public_probes': probes}
        if migrated:
            verified['schema_release'] = migrated
        self.store.immutable('promotions/' + active['promotion_id'] + '/verified.json', verified)
        def finish(document):
            document['live'].update(candidate_sha=request['candidate_sha'], window_id=active['promotion_id'], config=packet['config'],
                parameters=packet['parameters'], outputs=outputs, registry=packet['registry'], stack_execution_role_arn=document['authority']['execution_role_arn'])
            document['baseline'].update(definitions=healthy['definitions'], schedules=healthy['schedules'])
            document['completed'][active['promotion_id']] = summary
            if migrated:
                # The migrated schema and its contract become the next promotion's baseline.
                require(document['policy']['schema_version'] == migrated['from_version'], 'schema policy changed during release')
                source = packet['bundle']['source']
                document['baseline'].update(schema_manifest_sha256=source['schema_manifest_sha256'],
                                            deployment_contract_sha256=source['deployment_contract_sha256'])
                document['policy']['schema_version'] = migrated['to_version']
                document['completed'][active['promotion_id']] = dict(summary, schema_release=migrated)
            document['active'] = None
        self.store.change(finish)
        return summary
