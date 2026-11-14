"""Restore the previous release after a schema release's migration failed (APP-175).

An operator command, never a workflow permission. `plan` is read-only. `apply`
repeats every check, records an immutable plan, takes the fenced lease and then
puts the stack back on the baseline template with the previous release's
parameters, through a reviewed change set run by the restricted CloudFormation
role, as preparation does. It activates both services and requires them
healthy. Every mutation is journaled and nothing is replayed: any failure keeps
the lease and records the outcome.

It applies only while the database must still be at the prior schema version:
the migration never started, or its task exited by itself with the migration's
own refusal code, 1, which it uses only before COMMIT is sent (the runner applies
every migration in one transaction). It also proves it: the previous images
require exactly the prior version on every connection, so they become healthy
only if that holds. A migration that exited 0 committed the new version; one
that exited with the reserved code 4 may have; any other code is not the
program's own. Each of those refuses: read the schema version and fix forward
(docs/pitch/BETA-PROMOTION.md).
"""
import hashlib
import json
import re
import uuid

from . import guards, schema_release
from .aws import body
from .expiry_policy import operator_identity
from .maintenance import no_pending_changesets, operation_receipt, terminal_proof
from .candidate import REPOSITORY, api
from .runner import preflight
from .state import Refused, digest, encoded, no_duplicates, now, require
from datetime import timedelta

REQUIRED = ('schema-snapshot', 'stop-main', 'stop-broker', 'changeset-execute')
PREPARED = re.compile(r'template-upload|changeset-create|(service|qualification|operator)-(manifest-[0-9a-f]{16}'
                      r'|[0-9a-f]{16}-(begin|complete|part-(0|[1-9][0-9]*)))\Z')
MAX_RECEIPT_BYTES = 4 * 1024 * 1024


def read_receipt(store, path):
    try:
        raw = store.s3.get_object(Bucket=store.bucket, Key=store.prefix + '/' + path)['Body'].read(MAX_RECEIPT_BYTES + 1)
    except Exception:
        raise Refused('receipt unavailable; keep the lock') from None
    require(len(raw) <= MAX_RECEIPT_BYTES, 'receipt exceeds bound')
    return raw


def migration_outcome(store, active):
    """Prove the database cannot have moved: no migration, or its own exit 1 before COMMIT."""
    entries = [x for x in active['journal'] if x['name'] == 'schema-migrate']
    if not entries:
        return {'started': False}
    entry = entries[0]
    receipt = operation_receipt(store, entry, active['promotion_id'], 'migration start')
    started = receipt['response']
    tasks = started.get('tasks') if isinstance(started, dict) else None
    require(isinstance(tasks, list) and len(tasks) == 1 and not started.get('failures')
            and isinstance(tasks[0].get('taskArn'), str), 'migration start is not one known task')
    arn = tasks[0]['taskArn']
    # Immutable: the bucket refuses any promotions/ write without If-None-Match.
    path = 'promotions/' + active['promotion_id'] + '/schema-migration-observed.json'
    raw = read_receipt(store, path)
    observed = json.loads(raw, object_pairs_hook=no_duplicates)
    rows = observed.get('tasks') if isinstance(observed, dict) else None
    require(isinstance(rows, list) and len(rows) == 1 and not observed.get('failures')
            and rows[0].get('taskArn') == arn and rows[0].get('lastStatus') == 'STOPPED'
            and len(rows[0].get('containers') or []) == 1, 'migration outcome is not a stopped task')
    code = rows[0]['containers'][0].get('exitCode')
    require(type(code) is int, 'migration exit code unknown; reconcile')
    require(code != 0, 'the migration exited 0 and committed; fix forward')
    require(code != schema_release.MIGRATION_COMMIT_OUTCOME_UNKNOWN,
            'migration commit outcome unknown (exit 4); read the schema version and fix forward if it moved')
    # Only the program's own refusal proves the transaction did not commit: the container
    # exited by itself with the one code the migration uses before COMMIT is sent. A task
    # stopped from outside or ended by a signal (128+N, e.g. 137 or 143) may have stopped
    # just after the commit (review of c856fb8); any other code, such as Python's 120 for
    # a failed flush at shutdown, is not the program's own (APP-196).
    require(rows[0].get('stopCode') == 'EssentialContainerExited' and code == schema_release.MIGRATION_REFUSED,
            'the migration did not fail by itself before its commit (stopped from outside, by a signal or with '
            'another code) and may have committed; reconcile')
    return {'started': True, 'task_arn': arn, 'exit_code': code, 'stop_code': rows[0].get('stopCode'),
            'observed_receipt': path, 'observed_sha256': hashlib.sha256(raw).hexdigest()}


def plan(runner, expected_state_sha256, read_run=api):
    """Read-only: every precondition, the restore change-set request and the ledger plan."""
    store = runner.store
    require(isinstance(expected_state_sha256, str) and re.fullmatch(r'[0-9a-f]{64}', expected_state_sha256), 'exact reviewed state hash required')
    state = store.load()
    require(digest(state) == expected_state_sha256, 'state changed since operator review')
    active = state.get('active')
    packet = (active or {}).get('packet') or {}
    release = packet.get('schema_release')
    require(active and active['phase'] == 'executing' and release, 'only an executing schema release can be restored')
    journal = active['journal']
    names = [x['name'] for x in journal]
    require(all(x['status'] == 'confirmed' for x in journal), 'unknown operation must be reconciled')
    require(len(names) == len(set(names)) and set(REQUIRED) <= set(names)
            and all(name in REQUIRED or name == 'schema-migrate' or PREPARED.fullmatch(name) for name in names),
            'restore requires the stopped execution and nothing after the migration')
    migration = migration_outcome(store, active)
    identity = runner.aws.identity()
    operator_identity(identity, state)
    run = read_run(f'repos/{REPOSITORY}/actions/runs/{active["authorization"]["run_id"]}')
    clock = runner.clock()
    terminal_proof(run, active, clock)
    # The stack is on the candidate, both services stopped, every boundary exact.
    stopped = runner.boundary(0, packet['parameters'], after=True)
    inventory = no_pending_changesets(runner, state['authority']['stack_id'])
    template = state['baseline']['template']; template_raw = encoded(template)
    template_key = 'promotions/' + active['promotion_id'] + '/template.json'
    require(hashlib.sha256(template_raw).hexdigest() == packet['template_sha256']
            and read_receipt(store, template_key) == template_raw, 'baseline template object differs')
    previous = state['live']['parameters']
    changed = {k for k in set(previous) | set(packet['parameters']) if previous.get(k) != packet['parameters'].get(k)}
    require(changed <= set(guards.IMAGE_KEYS.values()) | {'DeploymentExpiresAtUtc'} and previous.get('ServiceDesiredCount') == '0',
            'previous parameters are not a runtime-only restore')
    expiry = preflight().timestamp(previous['DeploymentExpiresAtUtc'])
    require(expiry > clock + timedelta(hours=1), 'previous release expires too soon to restore')
    ledger, ledger_receipt = preflight().abandon(state['ledger'], active['promotion_id'], clock)
    create = {'StackName': state['authority']['stack_id'], 'ChangeSetName': 'forma-restore-' + active['promotion_id'],
              'ChangeSetType': 'UPDATE', 'TemplateURL': f'https://{store.bucket}.s3.{guards.REGION}.amazonaws.com/{store.prefix}/{template_key}',
              'Parameters': [{'ParameterKey': k, 'ParameterValue': v} for k, v in sorted(previous.items())],
              'Capabilities': ['CAPABILITY_IAM'], 'RoleARN': state['authority']['execution_role_arn'],
              'ClientToken': 'restore-' + active['promotion_id'],
              'Description': 'Restore previous release after failed schema migration ' + active['candidate_sha']}
    receipt = {'schema_version': 1, 'operation': 'restore-after-failed-migration', 'observed_at_utc': now(),
               'before_state_sha256': expected_state_sha256, 'promotion_id': active['promotion_id'],
               'candidate_sha': active['candidate_sha'], 'restored_candidate_sha': state['live']['candidate_sha'],
               'schema_release': release, 'migration': migration, 'identity': identity, 'run': run,
               'stopped_boundary': stopped, 'change_set_inventory': inventory, 'create_request': create,
               'previous_parameters': previous, 'ledger_receipt': ledger_receipt,
               'limits': 'Restores the previous images and template only. The baseline, the schema policy and the snapshot are unchanged.'}
    public = {'status': 'planned', 'promotion_id': active['promotion_id'], 'before_state_sha256': expected_state_sha256,
              'migration_started': migration['started'], 'migration_exit_code': migration.get('exit_code'),
              'restores_candidate_sha': state['live']['candidate_sha'], 'previous_expires_at_utc': previous['DeploymentExpiresAtUtc'],
              'windows_preserved': len(state['ledger']['windows'])}
    return {'receipt': receipt, 'public': public}


def apply(runner, expected_state_sha256, read_run=api):
    result = plan(runner, expected_state_sha256, read_run)
    store = runner.store; receipt = result['receipt']; promotion = receipt['promotion_id']
    prefix = 'promotions/' + promotion + '/'
    state = store.load()
    require(digest(state) == expected_state_sha256, 'state changed during the restore plan')
    active = state['active']; previous = receipt['previous_parameters']; create = receipt['create_request']
    # One receipt per attempt: a phase write that fails leaves the state unchanged, and a
    # later attempt on the same reviewed state must not collide with this one (APP-196).
    plan_key = prefix + 'restore-plan-' + expected_state_sha256[:16] + '-' + uuid.uuid4().hex + '.json'
    plan_sha256 = store.immutable(plan_key, receipt)
    runner.deadline = None  # No execute budget: an operator command, bounded wait by wait.
    store.owner = active['owner']  # The fenced lease passes to the operator, as for abandonment.
    store.change(lambda d: d['active'].update(phase='restoring', restore_requested_at_utc=now(),
                                              restore_plan_receipt=plan_key, restore_plan_sha256=plan_sha256))
    try:
        created = runner.mutate('restore-changeset-create', 'cloudformation', 'create_change_set', **create)
        arn = created['Id']
        described = runner.aws.wait('cloudformation', 'describe_change_set', lambda r: r['Status'] in ['CREATE_COMPLETE', 'FAILED'],
                                    ChangeSetName=arn, StackName=create['StackName'])
        template = state['baseline']['template']
        guards.changeset(described, previous, state['authority']['execution_role_arn'], template, create)
        observed = body(runner.read('cloudformation', 'get_template', ChangeSetName=arn, StackName=create['StackName'], TemplateStage='Original')['TemplateBody'])
        require(observed == template, 'restore change-set template differs')
        runner.mutate('restore-changeset-execute', 'cloudformation', 'execute_change_set', ChangeSetName=arn,
                      StackName=create['StackName'], ClientRequestToken='restore-execute-' + promotion)
        completed = runner.aws.wait('cloudformation', 'describe_stacks', lambda r: r['Stacks'][0]['StackStatus'] not in ['UPDATE_IN_PROGRESS', 'UPDATE_COMPLETE_CLEANUP_IN_PROGRESS'],
                                    timeout=1200, StackName=create['StackName'])['Stacks'][0]
        require(completed['StackStatus'] == 'UPDATE_COMPLETE', 'restore stack update failed; services remain stopped, reconcile')
        runner.settle_services(guards.outputs(completed))
        registry = state['live']['registry']
        restored = runner.boundary(0, previous, after=True, registry_override=registry)
        store.immutable(prefix + 'restore-stopped.json', restored)
        require(preflight().timestamp(previous['DeploymentExpiresAtUtc']) > runner.clock() + timedelta(minutes=10), 'previous release expires too soon')
        store.change(lambda d: d['active'].update(phase='restore-activation-intended', restore_activation_requested_at_utc=now()))
        outputs = restored['outputs']
        for label, key in [('broker', 'SharedBrokerServiceName'), ('main', 'ServiceName')]:
            runner.mutate('restore-activate-' + label, 'ecs', 'update_service', cluster=outputs['ClusterName'], service=outputs[key], desiredCount=1)
        # The previous images require the prior schema version on every connection.
        runner.settle_services(outputs, require_healthy=True)
        healthy = runner.boundary(1, previous, after=True, registry_override=registry)
        probes = runner.probes()
        store.immutable(prefix + 'restored.json', {'healthy': healthy, 'public_probes': probes, 'migration': receipt['migration'],
                                                   'restored_candidate_sha': receipt['restored_candidate_sha']})
    except Exception as error:
        try:
            store.immutable(prefix + 'restore-outcome.json', {'status': 'failed-lease-held', 'error': type(error).__name__,
                'phase': store.document['active']['phase'], 'journal': store.document['active']['journal'], 'observed_at_utc': now()})
        except Exception:
            pass  # Never hide the original failure behind a receipt failure.
        raise
    def finish(document):
        require(document['active']['phase'] == 'restore-activation-intended', 'restore phase changed')
        ledger, _ = preflight().abandon(document['ledger'], promotion, runner.clock())
        document['ledger'] = ledger
        document['live'].update(outputs=outputs, stack_execution_role_arn=document['authority']['execution_role_arn'])
        document['baseline'].update(definitions=healthy['definitions'], schedules=healthy['schedules'])
        document['completed'][promotion] = {'status': 'restored-after-failed-migration', 'candidate_sha': active['candidate_sha'],
            'restored_candidate_sha': receipt['restored_candidate_sha'], 'observed_at_utc': now(),
            'schema_snapshot': active['packet']['schema_release']['snapshot_identifier'], 'migration': receipt['migration'],
            'restore_plan_receipt': plan_key, 'restore_plan_sha256': plan_sha256, 'ledger_sha256': digest(ledger)}
        document['active'] = None
    store.change(finish)
    final = store.load()
    result['public'].update(status='restored', revision=final['revision'], state_sha256=digest(final))
    return result
