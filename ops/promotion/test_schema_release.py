"""APP-175 reviewed schema release: the file, its scope, and the lifecycle in fake AWS.

The lifecycle tests run the real prepare/execute code against the synthetic AWS
of test_runner. The release file, the candidate's schema manifest and the
infrastructure digest are synthetic there; separate tests bind the committed
file to this tree's HEAD.
"""
import copy
from datetime import datetime, timedelta, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from ops.promotion import guards, runner as runner_module, schema_release
from ops.promotion.candidate import deployment_contract_digest, infrastructure_digest
from ops.promotion.github import validate_summary
from ops.promotion.maintenance import abandon
from ops.promotion.runner import Runner, SCHEMA_SEQUENCE, SEQUENCE
from ops.promotion.state import Refused, Store, digest, encoded
from ops.promotion.test_artifacts import bundle
from ops.promotion.test_candidate import SHA
from ops.promotion.test_github import approval, run as workflow_run
from ops.promotion.test_runner import NOW, FakeAWS, fixture
from ops.promotion.test_state import MemoryS3

ROOT = Path(__file__).resolve().parents[2]
REVIEWED = schema_release.REVIEWED_SCHEMA_RELEASE
CANDIDATE = encoded({'schema_version': 1, 'database_version': 22, 'migrations': [], 'catalog': []})
INFRA = '6' * 64
FROM = {'database_version': 20, 'schema_manifest_sha256': '8' * 64, 'deployment_contract_sha256': '9' * 64}
CONTRACT = '7' * 64
ENDPOINT = 'genuisyntheticfoundation-databaseb269d8bb-abc123.c0abcdefghij.us-east-1.rds.amazonaws.com'
INSTANCE = 'genuisyntheticfoundation-databaseb269d8bb-abc123'
SUBNETS = ['subnet-0123456789abcdef0', 'subnet-0fedcba9876543210']
GROUP = 'sg-0123456789abcdef0'
MIGRATION_TASK = f'arn:aws:ecs:{guards.REGION}:{guards.ACCOUNT}:task/beta/' + 'a1' * 16
PREPARED_ID = '6f1c2a3b-4d5e-4f60-8a7b-9c0d1e2f3a4b'


def spec(candidate=CANDIDATE, contract=CONTRACT, infra=INFRA):
    return {'schema_version': 1, 'evidence': REVIEWED['evidence'], 'expires_at_utc': REVIEWED['expires_at_utc'],
            'from': dict(FROM), 'infrastructure_sha256': infra,
            'to': {'database_version': 22, 'schema_manifest_sha256': hashlib.sha256(candidate).hexdigest(),
                   'deployment_contract_sha256': contract}}


class SpecFile:
    """Point schema_release at a temporary file (None removes it) and pin the
    source constant to a synthetic reviewed release, `spec()` by default."""

    def __init__(self, test, value, pinned=None):
        directory = tempfile.TemporaryDirectory(); test.addCleanup(directory.cleanup)
        self.path = Path(directory.name) / 'schema-release.json'
        self.write(value)
        for name, replacement in [('SPEC', self.path), ('REVIEWED_SCHEMA_RELEASE', {})]:
            patcher = patch.object(schema_release, name, replacement); patcher.start(); test.addCleanup(patcher.stop)
        self.pin(pinned or spec())

    def pin(self, value):
        schema_release.REVIEWED_SCHEMA_RELEASE.clear()
        schema_release.REVIEWED_SCHEMA_RELEASE.update({k: copy.deepcopy(v) for k, v in value.items() if k != 'schema_version'})

    def write(self, value):
        if value is None:
            self.path.unlink(missing_ok=True)
        elif isinstance(value, bytes):
            self.path.write_bytes(value)
        else:
            self.path.write_text(value if isinstance(value, str) else json.dumps(value, indent=2, sort_keys=True) + '\n')


class SpecFileTests(unittest.TestCase):
    """Editing schema-release.json alone must never widen what is accepted."""

    def load(self, value):
        SpecFile(self, value)
        return schema_release.load()

    def test_missing_file_means_no_release(self):
        self.assertEqual(self.load(None), (None, None))

    def test_exact_shape_loads_with_the_byte_hash(self):
        value, sha = self.load(spec())
        self.assertEqual(value, spec())
        self.assertEqual(sha, hashlib.sha256((json.dumps(spec(), indent=2, sort_keys=True) + '\n').encode()).hexdigest())

    def test_duplicate_keys_and_invalid_json_refuse(self):
        text = json.dumps(spec(), indent=2, sort_keys=True)
        for changed in [text.replace('"expires_at_utc": "2026-11-20T00:00:00Z"',
                            '"expires_at_utc": "2026-11-20T00:00:00Z", "expires_at_utc": "2099-01-01T00:00:00Z"'),
                        text.replace('"to": {', '"to": {"database_version": 23, ', 1),
                        text[:-3], '', 'null', '[]', '"x"', b'\xff\xfe{}']:
            with self.subTest(changed=changed[:40]):
                self.assertNotEqual(changed, text)
                with self.assertRaises(Refused): self.load(changed)

    def test_widening_or_reshaping_refuses(self):
        mutations = [
            lambda v: v.update(expires_at_utc='2099-01-01T00:00:00Z'),
            lambda v: v.update(expires_at_utc='2026-11-21T00:00:00Z'),
            lambda v: v.update(expires_at_utc='soon'),
            lambda v: v.update(evidence='evidence/APP-173/SCAN-DISPOSITION.md'),
            lambda v: v['from'].update(database_version=19),
            lambda v: v['from'].update(database_version=21),
            lambda v: v['to'].update(database_version=23),
            lambda v: v['to'].update(database_version=21),
            lambda v: v['to'].update(database_version=22.0),
            lambda v: v['to'].update(database_version='22'),
            lambda v: v['from'].update(database_version=True),
            lambda v: v.update(schema_version=2),
            lambda v: v.update(schema_version=True),
            lambda v: v.update(schema_version=1.0),
            lambda v: v.update(note='x'),
            lambda v: v.pop('infrastructure_sha256'),
            lambda v: v.update(infrastructure_sha256='X' * 64),
            lambda v: v.update(infrastructure_sha256=['6' * 64]),
            lambda v: v['from'].update(schema_manifest_sha256='8' * 63),
            lambda v: v['to'].update(deployment_contract_sha256=None),
            lambda v: v['to'].update(extra='x'),
            lambda v: v['from'].pop('deployment_contract_sha256'),
            lambda v: v.update(to=[v['to']]),
            # A release whose two ends share a digest cannot apply to any change.
            lambda v: v['to'].update(schema_manifest_sha256=v['from']['schema_manifest_sha256']),
            lambda v: v['to'].update(deployment_contract_sha256=v['from']['deployment_contract_sha256']),
            # Well-formed, but retargeted away from the digests pinned in source.
            lambda v: v['from'].update(schema_manifest_sha256='1' * 64),
            lambda v: v['from'].update(deployment_contract_sha256='1' * 64),
            lambda v: v['to'].update(schema_manifest_sha256='1' * 64),
            lambda v: v['to'].update(deployment_contract_sha256='1' * 64),
            lambda v: v.update(infrastructure_sha256='1' * 64),
        ]
        for mutate in mutations:
            with self.subTest(mutate=mutate):
                value = spec(); mutate(value)
                with self.assertRaises(Refused): self.load(value)
        with self.assertRaises(Refused): self.load([['schema_version', 1]])


@unittest.skipUnless(schema_release.SPEC.exists(), 'no schema release is pending')
class CommittedReleaseTests(unittest.TestCase):
    """CI fails when the tree drifts from the reviewed release."""

    def test_committed_release_is_bound_to_this_tree(self):
        value, _ = schema_release.load()
        manifest = subprocess.check_output(['git', '-C', str(ROOT), 'show', 'HEAD:app/migrations/manifest.json'])
        self.assertEqual(value['infrastructure_sha256'], infrastructure_digest(ROOT, 'HEAD'))
        self.assertEqual(value['to']['deployment_contract_sha256'], deployment_contract_digest(ROOT, 'HEAD'))
        self.assertEqual(value['to']['schema_manifest_sha256'], hashlib.sha256(manifest).hexdigest())
        self.assertEqual(value['to']['database_version'], json.loads(manifest)['database_version'])

    def test_committed_release_applies_only_to_its_reviewed_baseline(self):
        value, sha = schema_release.load()
        baseline = {k: value['from'][k] for k in schema_release.DIGESTS}
        source = {k: value['to'][k] for k in schema_release.DIGESTS}
        clock = datetime(2026, 11, 6, tzinfo=timezone.utc)
        head = subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], text=True).strip()
        self.assertEqual(schema_release.applicable(source, baseline, 20, ROOT, clock, head),
                         {'from_version': 20, 'to_version': 22, 'spec_sha256': sha})
        for policy, moment, candidate in [(21, clock, head), (20, schema_release.expiry(value), head), (20, clock, 'f' * 40)]:
            with self.assertRaises(Refused): schema_release.applicable(source, baseline, policy, ROOT, moment, candidate)

    def test_committed_release_equals_the_source_pin(self):
        value, _ = schema_release.load()
        self.assertEqual({k: v for k, v in value.items() if k != 'schema_version'}, schema_release.REVIEWED_SCHEMA_RELEASE)


class ApplicabilityTests(unittest.TestCase):
    def setUp(self):
        self.file = SpecFile(self, spec())
        self.candidate = CANDIDATE; self.infra = INFRA
        self.head = SHA
        for name, value in [('candidate_manifest', lambda repo: self.candidate), ('infrastructure_digest', lambda repo: self.infra),
                            ('checkout_head', lambda repo: self.head)]:
            patcher = patch.object(schema_release, name, value); patcher.start(); self.addCleanup(patcher.stop)
        self.source = {'schema_manifest_sha256': hashlib.sha256(CANDIDATE).hexdigest(), 'deployment_contract_sha256': CONTRACT}
        self.baseline = {k: FROM[k] for k in schema_release.DIGESTS}
        self.clock = NOW; self.policy = 20; self.sha = SHA

    def check(self):
        return schema_release.applicable(self.source, self.baseline, self.policy, ROOT, self.clock, self.sha)

    def test_exact_release_applies(self):
        result = self.check()
        self.assertEqual(result, {'from_version': 20, 'to_version': 22, 'spec_sha256': schema_release.load()[1]})

    def test_every_scope_mismatch_refuses(self):
        cases = [
            ('no file', lambda: self.file.write(None)),
            ('baseline schema', lambda: self.baseline.update(schema_manifest_sha256='0' * 64)),
            ('baseline contract', lambda: self.baseline.update(deployment_contract_sha256='0' * 64)),
            ('candidate schema', lambda: self.source.update(schema_manifest_sha256='0' * 64)),
            ('candidate contract', lambda: self.source.update(deployment_contract_sha256='0' * 64)),
            ('contract-only change', lambda: self.source.update(schema_manifest_sha256=self.baseline['schema_manifest_sha256'])),
            ('infrastructure', lambda: setattr(self, 'infra', '5' * 64)),
            ('at expiry', lambda: setattr(self, 'clock', datetime(2026, 11, 20, tzinfo=timezone.utc))),
            ('after expiry', lambda: setattr(self, 'clock', datetime(2026, 12, 1, tzinfo=timezone.utc))),
            ('policy 19', lambda: setattr(self, 'policy', 19)),
            ('policy 22', lambda: setattr(self, 'policy', 22)),
            ('policy text', lambda: setattr(self, 'policy', '20')),
            ('candidate bytes', lambda: setattr(self, 'candidate', CANDIDATE + b' ')),
        ]
        for name, mutate in [('another checkout', lambda: setattr(self, 'head', 'b' * 40)), ('short candidate', lambda: setattr(self, 'sha', SHA[:7])),
                             ('no candidate', lambda: setattr(self, 'sha', None))]:
            with self.subTest(name=name):
                self.setUp(); mutate()
                with self.assertRaisesRegex(Refused, 'checkout differs from the candidate'): self.check()
        for name, mutate in cases:
            with self.subTest(name=name):
                self.setUp(); mutate()
                with self.assertRaisesRegex(Refused, 'separate reviewed release required'): self.check()

    def test_candidate_manifest_version_must_be_the_reviewed_target(self):
        for version in [21, 23, 22.0, True, None]:
            with self.subTest(version=version):
                self.setUp()
                self.candidate = encoded({'schema_version': 1, 'database_version': version})
                self.file.write(spec(candidate=self.candidate)); self.file.pin(spec(candidate=self.candidate))
                self.source['schema_manifest_sha256'] = hashlib.sha256(self.candidate).hexdigest()
                with self.assertRaises(Refused): self.check()
        self.setUp(); self.candidate = b'{"database_version": 22, "database_version": 22}'
        self.file.write(spec(candidate=self.candidate)); self.file.pin(spec(candidate=self.candidate))
        self.source['schema_manifest_sha256'] = hashlib.sha256(self.candidate).hexdigest()
        with self.assertRaises(Refused): self.check()

    def test_identifiers_and_requests_are_closed(self):
        self.assertEqual(schema_release.snapshot_identifier(PREPARED_ID, 22), 'forma-schema-release-6f1c2a3b-v22')
        for promotion, version in [('6F1C2A3B-4D5E-4F60-8A7B-9C0D1E2F3A4B', 22), ('../x', 22), (None, 22), (PREPARED_ID, True), (PREPARED_ID, 0)]:
            with self.subTest(promotion=promotion, version=version), self.assertRaises(Refused):
                schema_release.snapshot_identifier(promotion, version)
        self.assertEqual(schema_release.database_instance(ENDPOINT), INSTANCE)
        for endpoint in [None, INSTANCE, 'Genui.c0abcdefghij.us-east-1.rds.amazonaws.com', '1genui.c0.us-east-1.rds.amazonaws.com',
                         'genui-.c0.us-east-1.rds.amazonaws.com', 'gen--ui.c0.us-east-1.rds.amazonaws.com',
                         'genui.c0.us-west-2.rds.amazonaws.com', 'genui.c0.us-east-1.rds.amazonaws.com.evil',
                         'a' * 64 + '.c0.us-east-1.rds.amazonaws.com']:
            with self.subTest(endpoint=endpoint), self.assertRaises(Refused):
                schema_release.database_instance(endpoint)
        outputs = {'MigrationTaskDefinition': f'arn:aws:ecs:{guards.REGION}:{guards.ACCOUNT}:task-definition/Migration:7',
                   'ClusterName': 'beta', 'PublicTaskSubnets': ','.join(SUBNETS), 'TaskSecurityGroup': GROUP}
        request = schema_release.migration_request(outputs, PREPARED_ID)
        self.assertEqual(request, {'cluster': 'beta', 'taskDefinition': outputs['MigrationTaskDefinition'], 'count': 1,
            'launchType': 'FARGATE', 'platformVersion': '1.4.0', 'startedBy': 'forma-schema-release', 'enableExecuteCommand': False,
            'networkConfiguration': {'awsvpcConfiguration': {'subnets': SUBNETS, 'securityGroups': [GROUP], 'assignPublicIp': 'ENABLED'}},
            'clientToken': hashlib.sha256((PREPARED_ID + ':schema-migrate').encode()).hexdigest()})
        self.assertEqual(len(request['clientToken']), 64)
        for key, bad in [('MigrationTaskDefinition', f'arn:aws:ecs:{guards.REGION}:111111111111:task-definition/Migration:7'),
                         ('MigrationTaskDefinition', None), ('ClusterName', 'beta/other'),
                         ('PublicTaskSubnets', SUBNETS[0]), ('PublicTaskSubnets', SUBNETS[0] + ',' + SUBNETS[0]),
                         ('PublicTaskSubnets', ','.join(SUBNETS + ['subnet-0aaaaaaaaaaaaaaaa'])), ('PublicTaskSubnets', None),
                         ('TaskSecurityGroup', 'sg-1,sg-2'), ('TaskSecurityGroup', None)]:
            with self.subTest(key=key, bad=bad), self.assertRaises(Refused):
                schema_release.migration_request(dict(outputs, **{key: bad}), PREPARED_ID)


class AWSError(Exception):
    """A botocore-shaped client error."""
    def __init__(self, code):
        super().__init__(code); self.response = {'Error': {'Code': code}}


class SchemaAWS(FakeAWS):
    """FakeAWS plus RDS snapshots and the one migration task."""

    def __init__(self, *args):
        super().__init__(*args)
        self.snapshots = {}; self.snapshot_status = 'available'; self.run_failures = []; self.migrated = False
        self.migration = {'exitCode': 0, 'stopCode': 'EssentialContainerExited', 'lastStatus': 'STOPPED'}
        self.waits = []; self.time = None; self.durations = []; self.readback_error = None; self.restoring = False

    def wait(self, service, operation, predicate, **args):
        timeout = args.pop('timeout', None)
        self.waits.append((operation, timeout, args.pop('retry_codes', None)))
        if self.time is not None and timeout is not None:
            # By default the worst case: every wait uses its whole bound.
            self.time.now += self.durations.pop(0) if self.durations else timeout
        value = self.call(service, operation, **args)
        if not predicate(value):
            raise Refused('bounded wait expired; retain lease and reconcile')
        return value

    def persisted(self):
        return json.loads(self.s3.objects['beta/state.json'])

    def migration_task(self, status):
        live = self.state['live']
        return {'taskArn': MIGRATION_TASK, 'taskDefinitionArn': live['outputs']['MigrationTaskDefinition'],
                'clusterArn': f'arn:aws:ecs:{guards.REGION}:{guards.ACCOUNT}:cluster/beta', 'startedBy': 'forma-schema-release',
                'lastStatus': status, **({'stopCode': self.migration['stopCode']} if status == 'STOPPED' else {}),
                'containers': [{'name': 'workspace', 'image': live['parameters']['ImageDigest'],
                                'imageDigest': self.manifest['images']['service']['index_digest'], 'lastStatus': status,
                                **({'exitCode': self.migration['exitCode']} if status == 'STOPPED' else {})}]}

    def _call(self, s, o, a):
        if s == 'rds' and o == 'create_db_snapshot':
            # Before the outage: services serving, change set unexecuted, phase still prepared.
            assert self.counts == {'main': 1, 'broker': 1} and self.state['live']['outputs']['MigrationTaskDefinition'].endswith(':1')
            assert self.persisted()['active']['phase'] == 'prepared'
            snapshot = {'DBSnapshotIdentifier': a['DBSnapshotIdentifier'], 'DBInstanceIdentifier': a['DBInstanceIdentifier'],
                        'Status': 'creating', 'SnapshotType': 'manual', 'Encrypted': True}
            self.snapshots[a['DBSnapshotIdentifier']] = snapshot
            return {'DBSnapshot': dict(snapshot)}
        if s == 'rds' and o == 'describe_db_snapshots':
            if self.readback_error: raise AWSError(self.readback_error)
            if a['DBSnapshotIdentifier'] not in self.snapshots: raise AWSError('DBSnapshotNotFound')
            snapshot = self.snapshots[a['DBSnapshotIdentifier']]; snapshot['Status'] = self.snapshot_status
            return {'DBSnapshots': [dict(snapshot)]}
        if s == 'ecs' and o == 'run_task':
            assert self.counts == {'main': 0, 'broker': 0} and self.persisted()['active']['phase'] == 'executing'
            assert [x['Status'] for x in self.snapshots.values()] == ['available']
            assert a['taskDefinition'] == self.state['live']['outputs']['MigrationTaskDefinition']
            self.migrated = True
            return {'tasks': [] if self.run_failures else [self.migration_task('PROVISIONING')], 'failures': self.run_failures}
        if s == 'ecs' and o == 'describe_tasks' and a['tasks'] == [MIGRATION_TASK]:
            return {'tasks': [self.migration_task(self.migration['lastStatus'])], 'failures': []}
        if s == 'ecs' and o == 'update_service' and a['desiredCount'] == 1 and self.restoring:
            # The operator restore activates the previous release under its own phase.
            assert self.persisted()['active']['phase'] == 'restore-activation-intended'
            assert self.state['live']['parameters'] == self.original['live']['parameters']
            self.counts[a['service']] = 1
            return {'service': next(x for x in self.services() if x['serviceName'] == a['service'])}
        if s == 'ecs' and o == 'describe_tasks' and a['tasks'] != [MIGRATION_TASK] and self.restoring:
            value = super()._call(s, o, a)
            if self.state['live']['parameters'] == self.original['live']['parameters']:
                for task in value['tasks']:
                    role = 'service' if task['taskDefinitionArn'] == self.state['live']['outputs']['TaskDefinition'] else 'operator'
                    task['containers'][0]['imageDigest'] = self.state['live']['registry'][role]['index_digest']
            return value
        if s == 'ecs' and o == 'update_service' and a['desiredCount'] == 1:
            assert self.migrated or not self.snapshots, 'activation before the migration'
        if s == 'ecs' and o == 'update_service' and a['desiredCount'] == 0:
            assert all(x['Status'] == 'available' for x in self.snapshots.values()), 'outage before the snapshot'
        return super()._call(s, o, a)


class FakeTime:
    def __init__(self):
        self.now = 1000.0

    def monotonic(self):
        return self.now

    def sleep(self, seconds):
        self.now += seconds


class SchemaLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.build()

    def use_fake_time(self):
        clock = FakeTime(); self.aws.time = clock
        patcher = patch.object(runner_module, 'time', clock); patcher.start(); self.addCleanup(patcher.stop)
        return clock

    def abandon(self):
        """The documented operator abandonment, 66 minutes after the workflow ended."""
        self.aws.phase = 'operator'; original = self.aws._call
        def transport(s, o, a):
            if o == 'delete_change_set': self.aws.cs = None; return {}
            return original(s, o, a)
        self.aws._call = transport
        finished = dict(workflow_run(), status='completed', updated_at=(NOW - timedelta(minutes=66)).isoformat())
        return abandon(self.runner, digest(self.store.load()), read_run=lambda _: finished)

    def assert_serving_and_prepared(self, journal):
        persisted = self.store.load()
        self.assertEqual(persisted['active']['phase'], 'prepared')
        self.assertEqual(self.aws.counts, {'main': 1, 'broker': 1})
        self.assertFalse(any(o in ['update_service', 'execute_change_set', 'run_task'] for _, o, _ in self.aws.calls))
        self.assertEqual([(x['name'], x['status']) for x in persisted['active']['journal'] if x['name'].startswith('schema-')], journal)

    def build(self, release=True, candidate=CANDIDATE, policy=20, spec_value='default'):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.directory = Path(self.tmp.name); self.manifest = bundle(self.directory)
        if release:
            source = dict(self.manifest['source'], schema_manifest_sha256=hashlib.sha256(candidate).hexdigest(), deployment_contract_sha256=CONTRACT)
            self.manifest['source'] = source
            path = self.directory / 'candidate.json'; value = json.loads(path.read_bytes()); value['source'] = source
            path.write_bytes(encoded(value))
            self.manifest['reports']['candidate.json'] = hashlib.sha256(path.read_bytes()).hexdigest()
            (self.directory / 'manifest.json').write_bytes(encoded(self.manifest))
        self.initial = fixture(self.manifest)
        if release:
            self.initial['baseline'].update({k: FROM[k] for k in schema_release.DIGESTS})
        self.initial['policy']['schema_version'] = policy
        self.initial['live']['outputs'].update(DatabaseEndpoint=ENDPOINT, PublicTaskSubnets=','.join(SUBNETS), TaskSecurityGroup=GROUP)
        self.s3 = MemoryS3(self.initial); self.store = Store(self.s3, 'bucket', 'beta')
        self.aws = SchemaAWS(self.initial, self.s3, self.manifest)
        self.runner = Runner(self.aws, self.store, clock=lambda: NOW, probes=lambda: {'synthetic': True}, repo=ROOT)
        self.request = dict(candidate_sha=self.manifest['candidate_sha'],
            qualified_artifact_manifest_sha256=hashlib.sha256((self.directory / 'manifest.json').read_bytes()).hexdigest(),
            authorization=dict(actor='fixture', run_id='42', run_attempt='1', repository=guards.REPOSITORY, workflow_ref=guards.WORKFLOW))
        self.file = SpecFile(self, spec(candidate=candidate) if spec_value == 'default' else spec_value, pinned=spec(candidate=candidate))
        self.candidate = candidate; self.infra = INFRA
        patchers = [patch('ops.promotion.candidate.check_current'),
                    patch.object(schema_release, 'candidate_manifest', lambda repo: self.candidate),
                    patch.object(schema_release, 'infrastructure_digest', lambda repo: self.infra),
                    patch.object(schema_release, 'checkout_head', lambda repo: self.request['candidate_sha'])]
        self.current_mock = patchers[0].start()
        for patcher in patchers[1:]: patcher.start()
        for patcher in patchers: self.addCleanup(patcher.stop)

    def prepared(self):
        result = self.runner.prepare(self.request, self.directory); validate_summary(result, self.request['candidate_sha'])
        self.execute = dict(self.request, promotion_id=result['promotion_id'], packet_sha256=result['packet_sha256']); self.aws.phase = 'execute'
        return result

    def run_execute(self):
        return self.runner.execute(self.execute, self.directory, approval())

    def mutations(self):
        return [(o, a.get('service'), a.get('desiredCount')) for _, o, a in self.aws.calls
                if o in ['update_service', 'execute_change_set', 'create_db_snapshot', 'run_task']]

    def receipt(self, promotion, name):
        return json.loads(self.s3.objects['beta/promotions/' + promotion + '/' + name])

    def test_reviewed_release_snapshots_migrates_then_activates_and_admits_the_baseline(self):
        result = self.prepared(); promotion = result['promotion_id']
        sha = schema_release.load()[1]
        self.assertEqual(result['review']['schema_release'], {'from_version': 20, 'to_version': 22, 'spec_sha256': sha})
        self.assertEqual(result['review']['schema_version'], 20)
        self.assertEqual(result['review']['sequence'], SCHEMA_SEQUENCE)
        self.assertEqual(result['review']['preserved'], {'schema': False, 'template': True, 'trial': True, 'aliases': True, 'secrets': True})
        identifier = 'forma-schema-release-' + promotion[:8] + '-v22'
        packet = self.store.load()['active']['packet']
        self.assertEqual(packet['schema_release'], {'from_version': 20, 'to_version': 22, 'spec_sha256': sha, 'snapshot_identifier': identifier})
        self.assertFalse(any(o in ['create_db_snapshot', 'run_task'] for _, o, _ in self.aws.calls))

        summary = self.run_execute(); validate_summary(summary, self.request['candidate_sha'])
        self.assertEqual(self.mutations(), [('create_db_snapshot', None, None), ('update_service', 'main', 0), ('update_service', 'broker', 0),
            ('execute_change_set', None, None), ('run_task', None, None), ('update_service', 'broker', 1), ('update_service', 'main', 1)])
        # Prepare checks the trunk twice; execute rechecks before and after the snapshot.
        self.assertEqual(self.current_mock.call_count, 4)
        snapshot = next(a for _, o, a in self.aws.calls if o == 'create_db_snapshot')
        self.assertEqual(snapshot, {'DBInstanceIdentifier': INSTANCE, 'DBSnapshotIdentifier': identifier})
        run = next(a for _, o, a in self.aws.calls if o == 'run_task')
        self.assertEqual(run, {'cluster': 'beta', 'taskDefinition': self.aws.state['live']['outputs']['MigrationTaskDefinition'], 'count': 1,
            'launchType': 'FARGATE', 'platformVersion': '1.4.0', 'startedBy': 'forma-schema-release', 'enableExecuteCommand': False,
            'networkConfiguration': {'awsvpcConfiguration': {'subnets': SUBNETS, 'securityGroups': [GROUP], 'assignPublicIp': 'ENABLED'}},
            'clientToken': hashlib.sha256((promotion + ':schema-migrate').encode()).hexdigest()})
        self.assertTrue(run['taskDefinition'].endswith('MigrationTaskDefinition:2'))
        self.assertIn(('describe_db_snapshots', 720, {'DBSnapshotNotFound'}), self.aws.waits)
        self.assertIn(('describe_tasks', 720, None), self.aws.waits)

        final = self.store.load()
        self.assertIsNone(final['active']); self.assertEqual(self.aws.counts, {'main': 1, 'broker': 1})
        self.assertEqual(final['policy']['schema_version'], 22)
        self.assertEqual({k: final['baseline'][k] for k in schema_release.DIGESTS}, {k: self.manifest['source'][k] for k in schema_release.DIGESTS})
        record = {'from_version': 20, 'to_version': 22, 'spec_sha256': sha, 'snapshot_identifier': identifier, 'migration_task_arn': MIGRATION_TASK}
        self.assertEqual(final['completed'][promotion], dict(summary, schema_release=record))
        self.assertEqual(self.receipt(promotion, 'schema-release.json'), record)
        self.assertEqual(self.receipt(promotion, 'verified.json')['schema_release'], record)
        for name in ['schema-snapshot.json', 'schema-migrate.json', 'schema-snapshot-observed.json',
                     'schema-migration-observed.json', 'stopped-after-migration.json']:
            self.assertIn('beta/promotions/' + promotion + '/' + name, self.s3.objects)
        self.assertEqual(self.receipt(promotion, 'schema-migrate.json')['request']['arguments'], run)
        # The admitted baseline now treats this candidate as an ordinary promotion.
        self.assertIsNone(self.runner.reviewed_release(self.manifest['source'], self.request['candidate_sha']))
        self.assertEqual(self.receipt(promotion, 'schema-migration-observed.json')['tasks'][0]['containers'][0]['exitCode'], 0)

    def test_scope_mismatch_refuses_before_the_lock(self):
        cases = [
            ('no file', lambda: self.file.write(None)),
            ('widened expiry', lambda: self.file.write(dict(spec(), expires_at_utc='2099-01-01T00:00:00Z'))),
            ('duplicate key', lambda: self.file.write(json.dumps(spec()).replace('{', '{"schema_version": 1, ', 1))),
            ('bad type', lambda: self.file.write(dict(spec(), to=dict(spec()['to'], database_version='22')))),
            ('retargeted JSON', lambda: self.file.write(dict(spec(), to=dict(spec()['to'], deployment_contract_sha256='0' * 64)))),
            ('baseline differs', lambda: (self.initial['baseline'].update(deployment_contract_sha256='0' * 64), self.s3.objects.update({'beta/state.json': encoded(self.initial)}))),
            ('another checkout', lambda: self.enterContext(patch.object(schema_release, 'checkout_head', lambda repo: 'b' * 40))),
            ('infrastructure', lambda: setattr(self, 'infra', '5' * 64)),
            ('expired', lambda: setattr(self.runner, 'clock', lambda: datetime(2026, 11, 20, tzinfo=timezone.utc))),
            ('policy version', lambda: (self.initial['policy'].update(schema_version=19), self.s3.objects.update({'beta/state.json': encoded(self.initial)}))),
        ]
        for name, mutate in cases:
            with self.subTest(name=name):
                self.build(); mutate()
                with self.assertRaises(Refused): self.runner.prepare(self.request, self.directory)
                persisted = self.store.load()
                self.assertIsNone(persisted['active']); self.assertEqual(persisted['revision'], 0)
                self.assertEqual(self.aws.calls, [])

    def test_candidate_manifest_version_mismatch_refuses_before_the_lock(self):
        candidate = encoded({'schema_version': 1, 'database_version': 23})
        self.build(candidate=candidate)
        with self.assertRaises(Refused): self.runner.prepare(self.request, self.directory)
        self.assertIsNone(self.store.load()['active']); self.assertEqual(self.aws.calls, [])

    def test_normal_promotion_never_reads_the_release_file(self):
        for value in [spec(), '{"not": "a release"', None]:
            with self.subTest(value=str(value)[:20]):
                self.build(release=False, policy=19, spec_value=value)
                result = self.prepared()
                self.assertIsNone(result['review']['schema_release'])
                self.assertEqual(result['review']['sequence'], SEQUENCE)
                self.assertTrue(result['review']['preserved']['schema'])
                self.assertIsNone(self.store.load()['active']['packet']['schema_release'])
                summary = self.run_execute(); validate_summary(summary, self.request['candidate_sha'])
                self.assertEqual(self.mutations(), [('update_service', 'main', 0), ('update_service', 'broker', 0), ('execute_change_set', None, None),
                    ('update_service', 'broker', 1), ('update_service', 'main', 1)])
                final = self.store.load()
                self.assertEqual(final['policy']['schema_version'], 19)
                self.assertNotIn('schema_release', final['completed'][summary['promotion_id']])
                self.assertEqual({k: final['baseline'][k] for k in schema_release.DIGESTS}, {k: self.initial['baseline'][k] for k in schema_release.DIGESTS})

    def test_release_changed_or_expired_after_preparation_cannot_stop(self):
        def reformatted():
            self.file.write(json.dumps(spec(), sort_keys=True))
        def expires_during_final_checks(_):
            self.runner.clock = lambda: datetime(2026, 11, 20, tzinfo=timezone.utc)
        cases = [('reformatted', reformatted), ('removed', lambda: self.file.write(None)),
                 ('infrastructure', lambda: setattr(self, 'infra', '5' * 64)),
                 ('final checks', lambda: setattr(self.current_mock, 'side_effect', expires_during_final_checks))]
        for name, mutate in cases:
            with self.subTest(name=name):
                self.build(); self.prepared(); mutate()
                with self.assertRaises(Refused): self.run_execute()
                self.assertFalse(any(o in ['update_service', 'execute_change_set', 'create_db_snapshot', 'run_task'] for _, o, _ in self.aws.calls))
                self.assertEqual(self.store.load()['active']['phase'], 'prepared')

    def test_release_expiring_after_the_snapshot_never_stops_and_can_be_abandoned(self):
        self.prepared(); calls = []
        def second_check_expires(_):
            calls.append(1)
            if len(calls) == 2: self.runner.clock = lambda: datetime(2026, 11, 20, tzinfo=timezone.utc)
        self.current_mock.side_effect = second_check_expires
        with self.assertRaisesRegex(Refused, 'separate reviewed release required'): self.run_execute()
        self.assert_serving_and_prepared([('schema-snapshot', 'confirmed')])
        identifier = self.store.load()['active']['packet']['schema_release']['snapshot_identifier']
        self.runner.clock = lambda: NOW; before = len(self.aws.calls)
        result = self.abandon()
        self.assertEqual(result['status'], 'abandoned'); self.assertIsNone(self.store.load()['active'])
        self.assertFalse(any(s == 'rds' for s, _, _ in self.aws.calls[before:]))
        self.assertEqual(list(self.aws.snapshots), [identifier])
        promotion = result['promotion_id']
        retained = {'snapshot_identifier': identifier, 'journal_status': 'confirmed', 'readback': None,
                    'request': {'service': 'rds', 'operation': 'create_db_snapshot',
                                'arguments': {'DBInstanceIdentifier': INSTANCE, 'DBSnapshotIdentifier': identifier}}}
        self.assertEqual(self.receipt(promotion, 'abandon-fence.json')['retained_schema_snapshot'], retained)
        self.assertEqual(self.receipt(promotion, 'abandoned.json')['retained_schema_snapshot'], retained)

    def test_release_changed_before_approval_is_refused_even_if_restored_later(self):
        self.prepared(); original = self.file.path.read_text()
        self.file.write(json.dumps(spec(), sort_keys=True))
        self.current_mock.side_effect = lambda _: self.file.write(original)
        with self.assertRaisesRegex(Refused, 'schema release changed after preparation'): self.run_execute()
        promotion = self.execute['promotion_id']
        self.assertNotIn('beta/promotions/' + promotion + '/approval.json', self.s3.objects)
        self.assertFalse(any(o in ['update_service', 'execute_change_set'] for _, o, _ in self.aws.calls))
        self.assertEqual(self.store.load()['active']['phase'], 'prepared')

    def assert_stopped_without_activation(self, journal):
        persisted = self.store.load()
        self.assertEqual(persisted['active']['phase'], 'executing')
        self.assertEqual(self.aws.counts, {'main': 0, 'broker': 0})
        self.assertFalse(any(o == 'update_service' and a['desiredCount'] == 1 for _, o, a in self.aws.calls))
        self.assertEqual([(x['name'], x['status']) for x in persisted['active']['journal'] if x['name'].startswith('schema-')], journal)
        self.assertEqual(persisted['policy']['schema_version'], 20)
        self.assertEqual({k: persisted['baseline'][k] for k in schema_release.DIGESTS}, {k: FROM[k] for k in schema_release.DIGESTS})
        calls = len(self.aws.calls)
        with self.assertRaises(Refused): self.run_execute()
        self.assertEqual(len(self.aws.calls), calls)

    def test_failed_migration_keeps_services_stopped_and_never_activates(self):
        failures = [('exit 1', lambda: self.aws.migration.update(exitCode=1)),
                    ('no exit code', lambda: self.aws.migration.update(exitCode=None)),
                    ('task failed to start', lambda: self.aws.migration.update(stopCode='TaskFailedToStart')),
                    ('still running at the bound', lambda: self.aws.migration.update(lastStatus='RUNNING'))]
        for name, mutate in failures:
            with self.subTest(name=name):
                self.build(); self.prepared(); mutate()
                with self.assertRaises(Refused): self.run_execute()
                self.assert_stopped_without_activation([('schema-snapshot', 'confirmed'), ('schema-migrate', 'confirmed')])
                promotion = self.execute['promotion_id']
                self.assertNotIn('beta/promotions/' + promotion + '/schema-release.json', self.s3.objects)
                if name != 'still running at the bound':
                    self.assertIn('beta/promotions/' + promotion + '/schema-migration-observed.json', self.s3.objects)

    def test_migration_task_start_failure_keeps_services_stopped(self):
        self.prepared(); self.aws.run_failures = [{'reason': 'RESOURCE:MEMORY'}]
        with self.assertRaises(Refused): self.run_execute()
        self.assert_stopped_without_activation([('schema-snapshot', 'confirmed'), ('schema-migrate', 'confirmed')])

    def test_snapshot_failure_or_timeout_leaves_beta_serving_and_abandonable(self):
        for status in ['failed', 'creating', 'denied']:
            with self.subTest(status=status):
                self.build(); self.prepared()
                if status == 'denied':
                    original = self.aws._call
                    def denied(s, o, a):
                        if o == 'create_db_snapshot': raise Refused('AccessDenied')
                        return original(s, o, a)
                    self.aws._call = denied
                else:
                    self.aws.snapshot_status = status
                with self.assertRaises(Refused): self.run_execute()
                if status == 'denied':
                    # A raised call is an unknown outcome. Abandonment reads the name back.
                    self.assert_serving_and_prepared([('schema-snapshot', 'intent')])
                    result = self.abandon(); retained = self.receipt(result['promotion_id'], 'abandoned.json')['retained_schema_snapshot']
                    self.assertEqual((retained['journal_status'], retained['readback']['present'], retained['readback']['error_code']),
                                     ('intent', False, 'DBSnapshotNotFound'))
                    self.assertIsNone(self.store.load()['active'])
                    continue
                self.assert_serving_and_prepared([('schema-snapshot', 'confirmed')])
                calls = len(self.aws.calls)
                # The same run cannot execute again: its approval record already exists.
                with self.assertRaises(RuntimeError): self.run_execute()
                self.assertEqual(len(self.aws.calls), calls)
                result = self.abandon()
                self.assertEqual(result['status'], 'abandoned')
                self.assertFalse(any(o in ['update_service', 'execute_change_set', 'run_task'] for _, o, _ in self.aws.calls))

    def test_lost_snapshot_or_migration_ack_holds_the_lease_and_never_replays(self):
        for fault, journal, counts in [('create_db_snapshot', [('schema-snapshot', 'intent')], {'main': 1, 'broker': 1}),
                                       ('run_task', [('schema-snapshot', 'confirmed'), ('schema-migrate', 'intent')], {'main': 0, 'broker': 0})]:
            with self.subTest(fault=fault):
                self.build(); self.prepared(); self.aws.fail = fault
                with self.assertRaises(TimeoutError): self.run_execute()
                persisted = self.store.load()
                self.assertEqual([(x['name'], x['status']) for x in persisted['active']['journal'] if x['name'].startswith('schema-')], journal)
                self.assertEqual(self.aws.counts, counts)
                calls = len(self.aws.calls)
                with self.assertRaises(Refused): self.run_execute()
                self.assertEqual(len(self.aws.calls), calls)
                if fault == 'create_db_snapshot':
                    # The snapshot was created although its response was lost: read back as present.
                    result = self.abandon(); retained = self.receipt(result['promotion_id'], 'abandoned.json')['retained_schema_snapshot']
                    self.assertEqual((retained['readback']['present'], retained['readback']['status']), (True, 'available'))
                    self.assertEqual(self.receipt(result['promotion_id'], 'schema-snapshot-readback.json')['readback'], retained['readback'])
                    rds = [o for s, o, _ in self.aws.calls[calls:] if s == 'rds']
                    self.assertEqual(rds, ['describe_db_snapshots'])

    def test_wrong_database_endpoint_or_migration_image_refuses_before_activation(self):
        self.build(); self.initial['live']['outputs']['DatabaseEndpoint'] = 'db.c0.us-west-2.rds.amazonaws.com'
        self.s3.objects['beta/state.json'] = encoded(self.initial); self.aws.state = copy.deepcopy(self.initial)
        self.prepared()
        with self.assertRaises(Refused): self.run_execute()
        self.assertFalse(any(o == 'create_db_snapshot' for _, o, _ in self.aws.calls))
        self.assert_serving_and_prepared([])
        self.build(); self.prepared(); original = self.aws.migration_task
        def other_image(status):
            task = original(status); task['containers'][0]['imageDigest'] = 'sha256:' + '0' * 64; return task
        self.aws.migration_task = other_image
        with self.assertRaises(Refused): self.run_execute()
        self.assert_stopped_without_activation([('schema-snapshot', 'confirmed'), ('schema-migrate', 'confirmed')])

    def test_migration_needs_its_verified_request_and_a_confirmed_snapshot(self):
        self.prepared(); promotion = self.execute['promotion_id']
        self.store.load(); packet = self.store.document['active']['packet']
        after = self.runner.boundary(1, require_settled=False)
        with self.assertRaisesRegex(Refused, 'no confirmed pre-migration snapshot'): self.runner.migrate_schema(promotion, packet, after)
        self.store.change(lambda d: d['active']['journal'].append({'name': 'schema-snapshot', 'status': 'confirmed'}))
        for name, change, reason in [('definition', lambda a: a['definitions']['MigrationTaskDefinition'].update(taskDefinitionArn='other'), 'differs from the verified boundary'),
                                     ('subnets', lambda a: a['outputs'].update(PublicTaskSubnets=SUBNETS[0]), 'unexpected task subnets')]:
            with self.subTest(name=name):
                changed = copy.deepcopy(after); change(changed)
                with self.assertRaisesRegex(Refused, reason): self.runner.migrate_schema(promotion, packet, changed)
        self.assertFalse(any(o == 'run_task' for _, o, _ in self.aws.calls))
        with self.assertRaises(Refused):
            self.runner.snapshot_schema(promotion, packet, dict(after['outputs'], DatabaseEndpoint='db'))
        with self.assertRaises(Refused):
            self.runner.snapshot_schema(promotion, dict(packet, schema_release=dict(packet['schema_release'], snapshot_identifier='other')), after['outputs'])
        self.assertFalse(any(o == 'create_db_snapshot' for _, o, _ in self.aws.calls))

    def test_execute_budget_constants_leave_room_for_the_outage(self):
        budget, reserve = runner_module.EXECUTE_BUDGET_SECONDS, runner_module.OUTAGE_RESERVE_SECONDS
        self.assertLessEqual(budget, 55 * 60); self.assertLess(budget, 3600)
        # The pre-outage snapshot can never by itself push the outage below its reserve.
        self.assertLessEqual(schema_release.SNAPSHOT_WAIT_SECONDS + reserve, budget)
        self.assertLess(schema_release.MIGRATION_WAIT_SECONDS, reserve)

    def test_worst_case_waits_refuse_inside_the_budget_rather_than_overrun(self):
        for release in [True, False]:
            with self.subTest(release=release):
                self.build(release=release, policy=20 if release else 19); self.prepared(); clock = self.use_fake_time()
                start = clock.now
                with self.assertRaisesRegex(Refused, 'execute time'): self.run_execute()
                self.assertLessEqual(clock.now - start, runner_module.EXECUTE_BUDGET_SECONDS)
                if release:
                    # Too little time is left to observe a migration, so none starts.
                    self.assertEqual(self.store.load()['active']['phase'], 'executing')
                    self.assertFalse(any(o == 'run_task' for _, o, _ in self.aws.calls))
                    self.assertFalse(any(o == 'update_service' and a['desiredCount'] == 1 for _, o, a in self.aws.calls))
                else:
                    # A normal release still activates; only its verification runs out of time.
                    self.assertEqual(self.store.load()['active']['phase'], 'activation-intended')
                self.assertTrue(all(timeout is None or timeout <= runner_module.EXECUTE_BUDGET_SECONDS for _, timeout, _ in self.aws.waits))

    def test_a_slow_snapshot_that_leaves_too_little_time_never_stops(self):
        self.prepared(); clock = self.use_fake_time()
        self.current_mock.side_effect = lambda _: setattr(clock, 'now', clock.now + 600)
        with self.assertRaisesRegex(Refused, 'too little execute time left to start the outage'): self.run_execute()
        self.assert_serving_and_prepared([('schema-snapshot', 'confirmed')])
        self.assertEqual(self.abandon()['status'], 'abandoned')

    def test_too_little_time_to_observe_the_migration_never_starts_it(self):
        self.prepared(); clock = self.use_fake_time(); start = clock.now
        self.aws.durations = [720, 900, 1200, 200]  # snapshot, stop settle, stack update, settle
        with self.assertRaisesRegex(Refused, 'too little execute time left to observe the migration'): self.run_execute()
        self.assertFalse(any(o == 'run_task' for _, o, _ in self.aws.calls))
        self.assertEqual(self.aws.counts, {'main': 0, 'broker': 0})
        self.assertLessEqual(clock.now - start, runner_module.EXECUTE_BUDGET_SECONDS)

    def test_late_outage_waits_are_clipped_to_the_budget(self):
        self.prepared(); clock = self.use_fake_time(); start = clock.now; checks = []
        def slow_second_check(_):
            checks.append(1)
            if len(checks) == 2: clock.now += 700
        self.current_mock.side_effect = slow_second_check
        with self.assertRaisesRegex(Refused, 'execute time'): self.run_execute()
        stack = [timeout for operation, timeout, _ in self.aws.waits if operation == 'describe_stacks']
        self.assertEqual(len(stack), 1); self.assertLess(stack[0], runner_module.STACK_UPDATE_WAIT_SECONDS)
        self.assertLessEqual(clock.now - start, runner_module.EXECUTE_BUDGET_SECONDS)

    def test_slow_rechecks_never_start_a_snapshot_they_cannot_observe(self):
        self.prepared(); clock = self.use_fake_time()
        self.current_mock.side_effect = lambda _: setattr(clock, 'now', clock.now + 2700)
        with self.assertRaisesRegex(Refused, 'too little execute time left to observe the snapshot'): self.run_execute()
        self.assertFalse(any(o == 'create_db_snapshot' for _, o, _ in self.aws.calls))
        self.assert_serving_and_prepared([])

    def test_the_scan_delta_binds_the_promoted_candidates_contract(self):
        """Fable dry run, 2026-11-12: a schema release must be able to carry a disposition, or no
        release could ship once main holds a migration. Every scan-delta check (prepare, the final
        checks and execute) binds the candidate's contract, never the baseline's it replaces."""
        from ops.promotion import registry as registry_module
        contracts = []
        def recording(current, baseline, contract=None, clock=None):
            contracts.append(contract); return registry_module.guard_scan_delta(current, baseline, contract, clock)
        with patch('ops.promotion.runner.guard_scan_delta', side_effect=recording):
            self.prepared(); self.run_execute()
        baseline = self.store.load()['baseline']['deployment_contract_sha256']
        self.assertGreaterEqual(len(contracts), 3)
        self.assertEqual(set(contracts), {CONTRACT})
        self.assertNotIn(self.initial['baseline']['deployment_contract_sha256'], contracts)
        self.assertEqual(baseline, CONTRACT)  # finish() admits the candidate's contract as the new baseline

    def test_introduced_findings_without_a_disposition_for_the_candidate_contract_refuse(self):
        delta_calls = []
        def refusing(current, baseline, contract=None, clock=None):
            delta_calls.append(contract)
            raise Refused('new scan findings require separately reviewed disposition: operator')
        with patch('ops.promotion.runner.guard_scan_delta', side_effect=refusing):
            with self.assertRaisesRegex(Refused, 'new scan findings require separately reviewed disposition'):
                self.runner.prepare(self.request, self.directory)
        self.assertEqual(delta_calls, [CONTRACT])
        self.assertFalse(any(o == 'create_change_set' or (o == 'put_object' and a.get('Key', '').endswith('template.json'))
                             for _, o, a in self.aws.calls))
        self.assertEqual(self.store.load()['active']['phase'], 'preparing')


class SummaryTests(unittest.TestCase):
    def setUp(self):
        SpecFile(self, spec())
        self.sha = schema_release.load()[1]
        from ops.promotion.test_github import SHA
        self.candidate = SHA
        self.value = {'status': 'prepared', 'candidate_sha': SHA, 'promotion_id': PREPARED_ID, 'packet_sha256': 'b' * 64,
            'cost_genui_cents': 16984, 'cost_total_cents': 20784, 'expires_at_utc': '2026-11-30T12:00:00Z',
            'review': {'images': {n: 'sha256:' + 'c' * 64 for n in guards.REPOS}, 'predecessor_candidate_sha': 'd' * 40,
                'source_tree_sha': 'e' * 40, 'deployment_contract_sha256': CONTRACT, 'schema_version': 20,
                'schema_release': {'from_version': 20, 'to_version': 22, 'spec_sha256': self.sha},
                'preserved': {'schema': False, 'template': True, 'trial': True, 'aliases': True, 'secrets': True},
                'changes': [{'logical_id': 'MigrationTask', 'resource_type': 'AWS::ECS::TaskDefinition', 'action': 'Modify', 'replacement': 'True'}],
                'scan': {n: {'findings': 24, 'introduced': 0, 'reviewed': [], 'reviewed_count': 0, 'unreviewed': 0,
                    'disposition_sha256': None, 'review_expires_at_utc': None} for n in guards.REPOS},
                'sequence': list(SCHEMA_SEQUENCE), 'outage_required': True}}

    def test_release_projection_is_exact_and_bound_to_the_committed_file(self):
        self.assertEqual(validate_summary(copy.deepcopy(self.value), self.candidate), self.value)
        for mutate in [lambda r: r.update(schema_version=19), lambda r: r['schema_release'].update(from_version=19),
                       lambda r: r['schema_release'].update(to_version=23), lambda r: r['schema_release'].update(to_version=True),
                       lambda r: r['schema_release'].update(spec_sha256='0' * 64), lambda r: r['schema_release'].update(snapshot='x'),
                       lambda r: r['schema_release'].pop('spec_sha256'), lambda r: r.update(schema_release='20->22'),
                       lambda r: r.update(deployment_contract_sha256='0' * 64),
                       lambda r: r['preserved'].update(schema=True), lambda r: r.update(sequence=list(SEQUENCE)),
                       lambda r: r.update(schema_release=None), lambda r: r.pop('schema_release')]:
            with self.subTest(mutate=mutate):
                changed = copy.deepcopy(self.value); mutate(changed['review'])
                with self.assertRaises(ValueError): validate_summary(changed, self.candidate)


if __name__ == '__main__':
    unittest.main()
