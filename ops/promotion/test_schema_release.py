"""APP-175 reviewed schema release: the file, its scope, and the lifecycle in fake AWS.

The lifecycle tests run the real prepare/execute code against the synthetic AWS
of test_runner. The release file, the candidate's schema manifest and the
infrastructure digest are synthetic there; separate tests bind the committed
file to this tree's HEAD.
"""
import copy
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from ops.promotion import guards, schema_release
from ops.promotion.candidate import deployment_contract_digest, infrastructure_digest
from ops.promotion.github import validate_summary
from ops.promotion.runner import Runner, SCHEMA_SEQUENCE, SEQUENCE
from ops.promotion.state import Refused, Store, encoded
from ops.promotion.test_artifacts import bundle
from ops.promotion.test_github import approval
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
    """Point schema_release at a temporary file; None removes it."""

    def __init__(self, test, value):
        directory = tempfile.TemporaryDirectory(); test.addCleanup(directory.cleanup)
        self.path = Path(directory.name) / 'schema-release.json'
        self.write(value)
        patcher = patch.object(schema_release, 'SPEC', self.path); patcher.start(); test.addCleanup(patcher.stop)

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
        self.assertEqual(schema_release.applicable(source, baseline, 20, ROOT, clock),
                         {'from_version': 20, 'to_version': 22, 'spec_sha256': sha})
        for policy, moment in [(21, clock), (20, schema_release.expiry(value))]:
            with self.assertRaises(Refused): schema_release.applicable(source, baseline, policy, ROOT, moment)


class ApplicabilityTests(unittest.TestCase):
    def setUp(self):
        self.file = SpecFile(self, spec())
        self.candidate = CANDIDATE; self.infra = INFRA
        for name, value in [('candidate_manifest', lambda repo: self.candidate), ('infrastructure_digest', lambda repo: self.infra)]:
            patcher = patch.object(schema_release, name, value); patcher.start(); self.addCleanup(patcher.stop)
        self.source = {'schema_manifest_sha256': hashlib.sha256(CANDIDATE).hexdigest(), 'deployment_contract_sha256': CONTRACT}
        self.baseline = {k: FROM[k] for k in schema_release.DIGESTS}
        self.clock = NOW; self.policy = 20

    def check(self):
        return schema_release.applicable(self.source, self.baseline, self.policy, ROOT, self.clock)

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
        for name, mutate in cases:
            with self.subTest(name=name):
                self.setUp(); mutate()
                with self.assertRaisesRegex(Refused, 'separate reviewed release required'): self.check()

    def test_candidate_manifest_version_must_be_the_reviewed_target(self):
        for version in [21, 23, 22.0, True, None]:
            with self.subTest(version=version):
                self.setUp()
                self.candidate = encoded({'schema_version': 1, 'database_version': version})
                self.file.write(spec(candidate=self.candidate))
                self.source['schema_manifest_sha256'] = hashlib.sha256(self.candidate).hexdigest()
                with self.assertRaises(Refused): self.check()
        self.setUp(); self.candidate = b'{"database_version": 22, "database_version": 22}'
        self.file.write(spec(candidate=self.candidate)); self.source['schema_manifest_sha256'] = hashlib.sha256(self.candidate).hexdigest()
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


class SchemaAWS(FakeAWS):
    """FakeAWS plus RDS snapshots and the one migration task."""

    def __init__(self, *args):
        super().__init__(*args)
        self.snapshots = {}; self.snapshot_status = 'available'; self.run_failures = []; self.migrated = False
        self.migration = {'exitCode': 0, 'stopCode': 'EssentialContainerExited', 'lastStatus': 'STOPPED'}
        self.waits = []

    def wait(self, service, operation, predicate, **args):
        self.waits.append((operation, args.pop('timeout', None), args.pop('retry_codes', None)))
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
            assert self.counts == {'main': 0, 'broker': 0} and self.state['live']['outputs']['MigrationTaskDefinition'].endswith(':2')
            assert self.persisted()['active']['phase'] == 'executing'
            snapshot = {'DBSnapshotIdentifier': a['DBSnapshotIdentifier'], 'DBInstanceIdentifier': a['DBInstanceIdentifier'],
                        'Status': 'creating', 'SnapshotType': 'manual', 'Encrypted': True}
            self.snapshots[a['DBSnapshotIdentifier']] = snapshot
            return {'DBSnapshot': dict(snapshot)}
        if s == 'rds' and o == 'describe_db_snapshots':
            snapshot = self.snapshots[a['DBSnapshotIdentifier']]; snapshot['Status'] = self.snapshot_status
            return {'DBSnapshots': [dict(snapshot)]}
        if s == 'ecs' and o == 'run_task':
            assert self.counts == {'main': 0, 'broker': 0}
            assert [x['Status'] for x in self.snapshots.values()] == ['available']
            assert a['taskDefinition'] == self.state['live']['outputs']['MigrationTaskDefinition']
            self.migrated = True
            return {'tasks': [] if self.run_failures else [self.migration_task('PROVISIONING')], 'failures': self.run_failures}
        if s == 'ecs' and o == 'describe_tasks' and a['tasks'] == [MIGRATION_TASK]:
            return {'tasks': [self.migration_task(self.migration['lastStatus'])], 'failures': []}
        if s == 'ecs' and o == 'update_service' and a['desiredCount'] == 1:
            assert self.migrated or not self.snapshots, 'activation before the migration'
        return super()._call(s, o, a)


class SchemaLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.build()

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
        self.file = SpecFile(self, spec(candidate=candidate) if spec_value == 'default' else spec_value)
        self.candidate = candidate; self.infra = INFRA
        patchers = [patch('ops.promotion.candidate.check_current'),
                    patch.object(schema_release, 'candidate_manifest', lambda repo: self.candidate),
                    patch.object(schema_release, 'infrastructure_digest', lambda repo: self.infra)]
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
        self.assertEqual(self.mutations(), [('update_service', 'main', 0), ('update_service', 'broker', 0), ('execute_change_set', None, None),
            ('create_db_snapshot', None, None), ('run_task', None, None), ('update_service', 'broker', 1), ('update_service', 'main', 1)])
        snapshot = next(a for _, o, a in self.aws.calls if o == 'create_db_snapshot')
        self.assertEqual(snapshot, {'DBInstanceIdentifier': INSTANCE, 'DBSnapshotIdentifier': identifier})
        run = next(a for _, o, a in self.aws.calls if o == 'run_task')
        self.assertEqual(run, {'cluster': 'beta', 'taskDefinition': self.aws.state['live']['outputs']['MigrationTaskDefinition'], 'count': 1,
            'launchType': 'FARGATE', 'platformVersion': '1.4.0', 'startedBy': 'forma-schema-release', 'enableExecuteCommand': False,
            'networkConfiguration': {'awsvpcConfiguration': {'subnets': SUBNETS, 'securityGroups': [GROUP], 'assignPublicIp': 'ENABLED'}},
            'clientToken': hashlib.sha256((promotion + ':schema-migrate').encode()).hexdigest()})
        self.assertTrue(run['taskDefinition'].endswith('MigrationTaskDefinition:2'))
        self.assertIn(('describe_db_snapshots', 1200, {'DBSnapshotNotFound'}), self.aws.waits)
        self.assertIn(('describe_tasks', 1800, None), self.aws.waits)

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
        self.assertIsNone(self.runner.reviewed_release(self.manifest['source']))
        self.assertEqual(self.receipt(promotion, 'schema-migration-observed.json')['tasks'][0]['containers'][0]['exitCode'], 0)

    def test_scope_mismatch_refuses_before_the_lock(self):
        cases = [
            ('no file', lambda: self.file.write(None)),
            ('widened expiry', lambda: self.file.write(dict(spec(), expires_at_utc='2099-01-01T00:00:00Z'))),
            ('duplicate key', lambda: self.file.write(json.dumps(spec()).replace('{', '{"schema_version": 1, ', 1))),
            ('bad type', lambda: self.file.write(dict(spec(), to=dict(spec()['to'], database_version='22')))),
            ('from differs from baseline', lambda: self.file.write(dict(spec(), **{'from': dict(FROM, deployment_contract_sha256='0' * 64)}))),
            ('to differs from candidate', lambda: self.file.write(dict(spec(), to=dict(spec()['to'], deployment_contract_sha256='0' * 64)))),
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

    def test_snapshot_failure_or_timeout_never_migrates(self):
        for status in ['failed', 'creating']:
            with self.subTest(status=status):
                self.build(); self.prepared(); self.aws.snapshot_status = status
                with self.assertRaises(Refused): self.run_execute()
                self.assertFalse(any(o == 'run_task' for _, o, _ in self.aws.calls))
                self.assert_stopped_without_activation([('schema-snapshot', 'confirmed')])

    def test_lost_snapshot_or_migration_ack_holds_the_lease_and_never_replays(self):
        for fault, journal in [('create_db_snapshot', [('schema-snapshot', 'intent')]),
                               ('run_task', [('schema-snapshot', 'confirmed'), ('schema-migrate', 'intent')])]:
            with self.subTest(fault=fault):
                self.build(); self.prepared(); self.aws.fail = fault
                with self.assertRaises(TimeoutError): self.run_execute()
                persisted = self.store.load()
                self.assertEqual([(x['name'], x['status']) for x in persisted['active']['journal'] if x['name'].startswith('schema-')], journal)
                self.assertEqual(self.aws.counts, {'main': 0, 'broker': 0})
                calls = len(self.aws.calls)
                with self.assertRaises(Refused): self.run_execute()
                self.assertEqual(len(self.aws.calls), calls)

    def test_wrong_database_endpoint_or_migration_image_refuses_before_activation(self):
        self.build(); self.initial['live']['outputs']['DatabaseEndpoint'] = 'db.c0.us-west-2.rds.amazonaws.com'
        self.s3.objects['beta/state.json'] = encoded(self.initial); self.aws.state = copy.deepcopy(self.initial)
        self.prepared()
        with self.assertRaises(Refused): self.run_execute()
        self.assertFalse(any(o in ['create_db_snapshot', 'run_task'] for _, o, _ in self.aws.calls))
        self.assert_stopped_without_activation([])
        self.build(); self.prepared(); original = self.aws.migration_task
        def other_image(status):
            task = original(status); task['containers'][0]['imageDigest'] = 'sha256:' + '0' * 64; return task
        self.aws.migration_task = other_image
        with self.assertRaises(Refused): self.run_execute()
        self.assert_stopped_without_activation([('schema-snapshot', 'confirmed'), ('schema-migrate', 'confirmed')])

    def test_migration_requests_are_validated_before_the_snapshot(self):
        self.prepared(); promotion = self.execute['promotion_id']
        self.store.load(); packet = self.store.document['active']['packet']
        after = self.runner.boundary(1, require_settled=False)
        for name, change in [('definition', lambda a: a['definitions']['MigrationTaskDefinition'].update(taskDefinitionArn='other')),
                             ('subnets', lambda a: a['outputs'].update(PublicTaskSubnets=SUBNETS[0])),
                             ('endpoint', lambda a: a['outputs'].update(DatabaseEndpoint='db'))]:
            with self.subTest(name=name):
                changed = copy.deepcopy(after); change(changed)
                with self.assertRaises(Refused): self.runner.migrate_schema(promotion, packet, changed)
                self.assertFalse(any(o in ['create_db_snapshot', 'run_task'] for _, o, _ in self.aws.calls))

    def test_release_with_new_scan_findings_refuses_before_a_change_set(self):
        # A reviewed disposition binds the baseline contract, which a schema release replaces.
        delta = {role: {'introduced': 1 if role == 'operator' else 0} for role in guards.REPOS}
        with patch('ops.promotion.runner.guard_scan_delta', return_value=delta):
            with self.assertRaisesRegex(Refused, 'schema release cannot introduce scan findings'):
                self.runner.prepare(self.request, self.directory)
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
