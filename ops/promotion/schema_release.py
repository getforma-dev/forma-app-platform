"""A reviewed schema release: the one migration a runtime promotion may carry (APP-175).

Automatic promotion refuses any change to the schema manifest or deployment
contract. `schema-release.json` admits exactly one such change: from the live
baseline's digests and policy schema version to the candidate's, with the stack
source unchanged and before an expiry. Every value in it is also pinned here, in
source, so editing the JSON alone can neither widen nor retarget it: the
evidence record, both ends' versions and digests, the infrastructure digest and
the expiry. A CI test binds the `to` side and infrastructure digest to HEAD. A
missing file means no schema release.

Execution takes a manual RDS snapshot while beta still serves, after every
pre-execution recheck, then stops the services, applies the reviewed change set
and runs the stack's migration task on the candidate image before activation.
A snapshot failure leaves beta running and the release abandonable; a migration
failure leaves the services stopped and the lease held for operator
reconciliation. Nothing is retried or replayed.
"""
import hashlib
import json
from datetime import datetime
from pathlib import Path
import re
import subprocess
import uuid

from .candidate import infrastructure_digest
from .guards import ACCOUNT, REGION
from .state import Refused, no_duplicates, require

SPEC = Path(__file__).with_name('schema-release.json')
# Changing any of these needs a reviewed source change, not a JSON edit. The
# JSON must equal it exactly; see the evidence record for how each digest was
# computed. Release B (APP-194): 22 -> 24, from release A's live baseline
# (evidence/APP-175/release-2026-11-14.md). It replaces release A's spent
# 20 -> 22 pin (evidence/APP-175/SCHEMA-RELEASE.md). The expiry falls after the
# planned release (2026-11-23) and before the class (2026-11-29), so no schema
# change can promote during it.
REVIEWED_SCHEMA_RELEASE = {
    'evidence': 'evidence/APP-194/SCHEMA-RELEASE.md',
    'expires_at_utc': '2026-11-26T00:00:00Z',
    'from': {'database_version': 22,
             'schema_manifest_sha256': '7ceb859d1794e79224ed609bfb557c21e79c23020894540b7b5114044ec68176',
             'deployment_contract_sha256': '14a161a09cf93044cbf6d1281b02b56f954fa7f73ae3a17a8f54b07b7958d99b'},
    'to': {'database_version': 24,
           'schema_manifest_sha256': '176d68cbc363a822a32b747b3a52f6a45809a946bed2649e93bd65f7d84208f1',
           'deployment_contract_sha256': '88d17cb53bf77c2f2bfa0b4345953f9f74c0aa39b81b677d6105807a54bcc356'},
    'infrastructure_sha256': 'e74b620e4d9749b5eb47819a1546be1adc00765008f042abbb0b747d9e549da1',
}
FIELDS = frozenset({'schema_version', 'evidence', 'from', 'to', 'infrastructure_sha256', 'expires_at_utc'})
DIGESTS = ('schema_manifest_sha256', 'deployment_contract_sha256')
CHANGED = 'schema or deployment contract changed; separate reviewed release required'
HEX = re.compile(r'[0-9a-f]{64}\Z')
SNAPSHOT_PREFIX = 'forma-schema-release-'
STARTED_BY = 'forma-schema-release'
# Both bounds sit inside runner.EXECUTE_BUDGET_SECONDS; see the arithmetic there.
# The snapshot runs before the outage, so its bound costs no downtime.
SNAPSHOT_WAIT_SECONDS = 720
MIGRATION_WAIT_SECONDS = 720
# The migration's own exit codes (app/runtime/runtime.py module notes; APP-196).
# 0: committed. MIGRATION_REFUSED: refused or failed before COMMIT was sent, so
# the database is unchanged. MIGRATION_COMMIT_OUTCOME_UNKNOWN: COMMIT may have
# reached the server, so the database may be at either version. Every other code
# is not the program's own and proves nothing. test_schema_recovery reads the
# reserved value from runtime.py.
MIGRATION_REFUSED = 1
MIGRATION_COMMIT_OUTCOME_UNKNOWN = 4


def side(value, version):
    return (isinstance(value, dict) and set(value) == {'database_version', *DIGESTS}
            and type(value['database_version']) is int and value['database_version'] == version
            and all(isinstance(value[key], str) and HEX.fullmatch(value[key]) for key in DIGESTS))


def load():
    """The committed release and the sha256 of its exact bytes, or (None, None)."""
    try:
        raw = SPEC.read_bytes()
    except FileNotFoundError:
        return None, None
    try:
        value = json.loads(raw, object_pairs_hook=no_duplicates)
    except ValueError as error:
        raise Refused('invalid schema release') from error
    reviewed = REVIEWED_SCHEMA_RELEASE
    require(isinstance(value, dict) and set(value) == FIELDS
            and type(value['schema_version']) is int and value['schema_version'] == 1
            and value['evidence'] == reviewed['evidence']
            and value['expires_at_utc'] == reviewed['expires_at_utc']
            and isinstance(value['infrastructure_sha256'], str) and HEX.fullmatch(value['infrastructure_sha256'])
            and side(value['from'], reviewed['from']['database_version']) and side(value['to'], reviewed['to']['database_version'])
            and all(value['from'][key] != value['to'][key] for key in DIGESTS),
            'invalid schema release')
    require(value['from'] == reviewed['from'] and value['to'] == reviewed['to']
            and value['infrastructure_sha256'] == reviewed['infrastructure_sha256'],
            'schema release differs from the reviewed source')
    return value, hashlib.sha256(raw).hexdigest()


def expiry(spec):
    return datetime.fromisoformat(spec['expires_at_utc'].replace('Z', '+00:00'))


def checkout_head(repo):
    return subprocess.check_output(['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True).strip()


def candidate_manifest(repo):
    return subprocess.check_output(['git', '-C', str(repo), 'show', 'HEAD:app/migrations/manifest.json'])


def applicable(source, baseline, policy_version, repo, clock, candidate):
    """The release for this exact baseline-to-candidate change, or Refused.

    Called only when the candidate's schema manifest or deployment contract
    differs from the baseline, before the lease and again at execution.
    """
    spec, spec_sha = load()
    require(spec is not None, CHANGED)
    # The tree read below must be the candidate's own checkout.
    require(isinstance(candidate, str) and re.fullmatch(r'[0-9a-f]{40}', candidate)
            and checkout_head(repo) == candidate, 'checkout differs from the candidate')
    before, after = spec['from'], spec['to']
    require(all(before[key] == baseline[key] and after[key] == source[key] for key in DIGESTS), CHANGED)
    require(type(policy_version) is int and policy_version == before['database_version'], CHANGED)
    raw = candidate_manifest(repo)
    require(hashlib.sha256(raw).hexdigest() == source['schema_manifest_sha256'], CHANGED)
    try:
        manifest = json.loads(raw, object_pairs_hook=no_duplicates)
    except ValueError as error:
        raise Refused(CHANGED) from error
    require(isinstance(manifest, dict) and type(manifest.get('database_version')) is int
            and manifest['database_version'] == after['database_version'], CHANGED)
    require(infrastructure_digest(repo) == spec['infrastructure_sha256'], CHANGED)
    require(clock < expiry(spec), CHANGED)
    return {'from_version': before['database_version'], 'to_version': after['database_version'], 'spec_sha256': spec_sha}


def promotion_identity(promotion_id):
    try:
        canonical = str(uuid.UUID(promotion_id))
    except (ValueError, TypeError, AttributeError):
        canonical = None
    require(isinstance(promotion_id, str) and canonical == promotion_id, 'invalid promotion identity')
    return promotion_id


def snapshot_identifier(promotion_id, to_version):
    promotion_identity(promotion_id)
    require(type(to_version) is int and 0 < to_version <= 10000, 'invalid schema version')
    identifier = SNAPSHOT_PREFIX + promotion_id[:8] + '-v' + str(to_version)
    # RDS: a letter first, then letters, digits and single hyphens; no trailing hyphen.
    require(len(identifier) <= 255 and re.fullmatch(r'[a-z][a-z0-9]*(?:-[a-z0-9]+)*', identifier), 'invalid snapshot identifier')
    return identifier


def database_instance(endpoint):
    """The instance identifier is the endpoint's first DNS label."""
    match = isinstance(endpoint, str) and re.fullmatch(
        r'([a-z](?:[a-z0-9]|-(?=[a-z0-9])){0,62})\.[a-z0-9]{1,63}\.' + re.escape(REGION) + r'\.rds\.amazonaws\.com', endpoint)
    require(match, 'unexpected database endpoint')
    return match[1]


def migration_request(outputs, promotion_id):
    """RunTask for the stack's migration task, shaped as infra/scripts/window_requests.py."""
    arn = outputs.get('MigrationTaskDefinition')
    require(isinstance(arn, str) and re.fullmatch(
        r'arn:aws:ecs:%s:%s:task-definition/[A-Za-z0-9_-]{1,255}:[1-9][0-9]{0,9}' % (REGION, ACCOUNT), arn), 'unexpected migration task definition')
    cluster = outputs.get('ClusterName')
    require(isinstance(cluster, str) and re.fullmatch(r'[A-Za-z0-9_-]{1,255}', cluster), 'unexpected cluster')
    subnets = outputs.get('PublicTaskSubnets')
    subnets = subnets.split(',') if isinstance(subnets, str) else []
    require(len(subnets) == 2 and len(set(subnets)) == 2
            and all(re.fullmatch(r'subnet-[0-9a-f]{8,17}', subnet) for subnet in subnets), 'unexpected task subnets')
    group = outputs.get('TaskSecurityGroup')
    require(isinstance(group, str) and re.fullmatch(r'sg-[0-9a-f]{8,17}', group), 'unexpected task security group')
    promotion_identity(promotion_id)
    return {'cluster': cluster, 'taskDefinition': arn, 'count': 1, 'launchType': 'FARGATE', 'platformVersion': '1.4.0',
            'networkConfiguration': {'awsvpcConfiguration': {'subnets': subnets, 'securityGroups': [group], 'assignPublicIp': 'ENABLED'}},
            'startedBy': STARTED_BY, 'enableExecuteCommand': False,
            # At most 64 characters; the same promotion always names the same task.
            'clientToken': hashlib.sha256((promotion_id + ':schema-migrate').encode()).hexdigest()}


def snapshot_settled(response):
    rows = response.get('DBSnapshots') or []
    return len(rows) > 1 or (len(rows) == 1 and rows[0].get('Status') != 'creating')


def verify_snapshot(response, identifier, instance):
    rows = response.get('DBSnapshots') or []
    require(len(rows) == 1 and rows[0].get('DBSnapshotIdentifier') == identifier
            and rows[0].get('DBInstanceIdentifier') == instance and rows[0].get('Status') == 'available'
            and rows[0].get('SnapshotType') == 'manual' and rows[0].get('Encrypted') is True,
            'pre-migration snapshot unavailable; no service was stopped')


def started_task(response, request):
    cluster = f"arn:aws:ecs:{REGION}:{ACCOUNT}:cluster/{request['cluster']}"
    tasks = response.get('tasks') or []
    require(not response.get('failures') and len(tasks) == 1, 'migration task did not start; services remain stopped, reconcile')
    task = tasks[0]
    arn = task.get('taskArn')
    require(isinstance(arn, str) and re.fullmatch(
        r'arn:aws:ecs:%s:%s:task/%s/[0-9a-f]{32}' % (REGION, ACCOUNT, re.escape(request['cluster'])), arn)
            and task.get('taskDefinitionArn') == request['taskDefinition'] and task.get('clusterArn') == cluster
            and task.get('startedBy') == STARTED_BY, 'migration task identity differs; services remain stopped, reconcile')
    return arn


def task_stopped(response, arn):
    tasks = response.get('tasks') or []
    return len(tasks) == 1 and tasks[0].get('taskArn') == arn and tasks[0].get('lastStatus') == 'STOPPED'


def verify_migration(response, arn, request, container_name, image, digests):
    tasks = response.get('tasks') or []
    require(not response.get('failures') and len(tasks) == 1, 'migration task unavailable; services remain stopped, reconcile')
    task = tasks[0]
    containers = task.get('containers') or []
    require(task.get('taskArn') == arn and task.get('lastStatus') == 'STOPPED'
            and task.get('taskDefinitionArn') == request['taskDefinition'] and task.get('startedBy') == STARTED_BY
            and task.get('stopCode') == 'EssentialContainerExited' and len(containers) == 1,
            'migration task did not complete; services remain stopped, reconcile')
    container = containers[0]
    require(container.get('name') == container_name and container.get('image') == image
            and container.get('imageDigest') in digests,
            'migration ran another image; services remain stopped, reconcile')
    require(container.get('exitCode') != MIGRATION_COMMIT_OUTCOME_UNKNOWN,
            'migration commit outcome unknown; services remain stopped, read the schema version before any recovery')
    require(type(container.get('exitCode')) is int and container['exitCode'] == 0,
            'migration failed; services remain stopped, reconcile')
