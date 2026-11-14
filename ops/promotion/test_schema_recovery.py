"""APP-175: the operator restore after a schema release's migration failed.

Runs the real execute path to a failed migration in the fake AWS of
test_schema_release, then the real plan/apply of schema_recovery.
"""
import copy
import hashlib
from datetime import datetime, timedelta, timezone
import io
import json
import os
from pathlib import Path
import re
import tempfile
import unittest
import uuid
from unittest.mock import patch

from ops.promotion import schema_recovery, schema_release
from ops.promotion import test_schema_release as schema_tests
from ops.promotion.maintenance import main as maintenance_main
from ops.promotion.runner import preflight
from ops.promotion.state import Refused, digest, encoded
from ops.promotion.test_github import run as workflow_run
from ops.promotion.test_runner import NOW

FINISHED = dict(workflow_run(), status='completed', updated_at=(NOW - timedelta(minutes=66)).isoformat())


class RestoreTests(unittest.TestCase):
    def setUp(self):
        environment = patch.dict('os.environ', {'GITHUB_ACTIONS': 'false'}); environment.start(); self.addCleanup(environment.stop)

    def failed(self, kind='exit 1'):
        """A schema release whose outage started and whose migration failed in a known way."""
        self.fixture = schema_tests.SchemaLifecycleTests('test_reviewed_release_snapshots_migrates_then_activates_and_admits_the_baseline')
        self.fixture.setUp(); self.addCleanup(self.fixture.doCleanups)
        fixture = self.fixture; fixture.prepared(); aws = fixture.aws
        if kind == 'not started':
            fixture.use_fake_time(); aws.durations = [720, 900, 1200, 200]
        elif kind == 'exit 1':
            aws.migration.update(exitCode=1)
        elif kind == 'exit 0':
            aws.migration.update(exitCode=0, stopCode='TaskFailedToStart')
        elif kind == 'killed':
            aws.migration.update(exitCode=143, stopCode='UserInitiated')
        elif kind == 'oom':
            aws.migration.update(exitCode=137)
        elif kind == 'exit 1 stopped from outside':
            aws.migration.update(exitCode=1, stopCode='UserInitiated')
        elif re.fullmatch(r'exit [0-9]+', kind):
            aws.migration.update(exitCode=int(kind[5:]))
        elif kind == 'no exit code':
            aws.migration.update(exitCode=None)
        elif kind == 'unobserved':
            aws.migration.update(lastStatus='RUNNING')
        elif kind == 'start failure':
            aws.run_failures = [{'reason': 'RESOURCE:ENI'}]
        elif kind == 'lost start':
            aws.fail = 'run_task'
        with self.assertRaises((Refused, TimeoutError)) as raised: fixture.run_execute()
        self.execute_error = raised.exception
        self.store, self.aws, self.runner = fixture.store, aws, fixture.runner
        self.state = self.store.load(); self.promotion = self.state['active']['promotion_id']
        self.assertEqual(self.state['active']['phase'], 'executing'); self.assertEqual(aws.counts, {'main': 0, 'broker': 0})
        self.aws.phase = 'operator'; self.aws.restoring = True; self.offset = len(aws.calls)

    def plan(self, run=FINISHED):
        return schema_recovery.plan(self.runner, digest(self.store.load()), read_run=lambda _: run)

    def apply(self, run=FINISHED):
        return schema_recovery.apply(self.runner, digest(self.store.load()), read_run=lambda _: run)

    def rewrite_observation(self, change):
        # Simulates a different immutable observation; the real bucket refuses any overwrite.
        key = 'beta/promotions/' + self.promotion + '/schema-migration-observed.json'
        observed = json.loads(self.fixture.s3.objects[key]); change(observed); self.fixture.s3.objects[key] = encoded(observed)

    def receipt(self, name):
        return json.loads(self.fixture.s3.objects['beta/promotions/' + self.promotion + '/' + name])

    def mutations(self):
        return [(o, a.get('service'), a.get('desiredCount')) for _, o, a in self.aws.calls[self.offset:]
                if o in ['create_change_set', 'execute_change_set', 'update_service', 'run_task', 'create_db_snapshot', 'delete_change_set']]

    def assert_unchanged_and_refused(self, call=None):
        before = self.store.load()
        with self.assertRaises(Refused): (call or self.plan)()
        self.assertEqual(self.store.load(), before); self.assertEqual(self.mutations(), [])

    def test_restores_the_previous_release_after_a_failed_or_unstarted_migration(self):
        for kind in ['exit 1', 'not started']:
            with self.subTest(kind=kind):
                self.failed(kind); planned = self.plan()
                self.assertEqual(self.mutations(), []); self.assertEqual(self.store.load(), self.state)
                self.assertEqual(planned['public']['migration_started'], kind == 'exit 1')
                self.assertEqual(planned['public']['migration_exit_code'], 1 if kind == 'exit 1' else None)
                result = self.apply()
                self.assertEqual(result['public']['status'], 'restored')
                self.assertEqual(self.mutations(), [('create_change_set', None, None), ('execute_change_set', None, None),
                                                    ('update_service', 'broker', 1), ('update_service', 'main', 1)])
                create = next(a for _, o, a in self.aws.calls[self.offset:] if o == 'create_change_set')
                self.assertEqual(create['RoleARN'], self.state['authority']['execution_role_arn'])
                self.assertEqual({p['ParameterKey']: p['ParameterValue'] for p in create['Parameters']}, self.state['live']['parameters'])
                self.assertTrue(create['TemplateURL'].endswith('/beta/promotions/' + self.promotion + '/template.json'))
                self.assertFalse(any(s == 'rds' for s, _, _ in self.aws.calls[self.offset:]))
                final = self.store.load()
                self.assertIsNone(final['active']); self.assertEqual(self.aws.counts, {'main': 1, 'broker': 1})
                self.assertEqual(final['completed'][self.promotion]['status'], 'restored-after-failed-migration')
                for key in ['candidate_sha', 'window_id', 'parameters', 'registry', 'config']:
                    self.assertEqual(final['live'][key], self.state['live'][key])
                self.assertEqual(final['live']['stack_execution_role_arn'], self.state['authority']['execution_role_arn'])
                self.assertEqual(final['policy'], self.state['policy'])
                for key in ['schema_manifest_sha256', 'deployment_contract_sha256', 'template']:
                    self.assertEqual(final['baseline'][key], self.state['baseline'][key])
                window = final['ledger']['windows'][self.promotion]
                self.assertEqual(window['abandoned_supersession_of_window_id'], self.state['live']['window_id'])
                self.assertNotIn('superseded_by_window_id', final['ledger']['windows'][self.state['live']['window_id']])
                self.assertEqual(set(final['ledger']['windows']), set(self.state['ledger']['windows']))
                # One plan receipt per attempt, named in the completed record with its hash.
                completed = final['completed'][self.promotion]
                self.assertRegex(completed['restore_plan_receipt'],
                                 r'\Apromotions/' + self.promotion + '/restore-plan-' + digest(self.state)[:16] + '-[0-9a-f]{32}\.json\Z')
                plan_raw = self.fixture.s3.objects['beta/' + completed['restore_plan_receipt']]
                self.assertEqual(completed['restore_plan_sha256'], hashlib.sha256(plan_raw).hexdigest())
                self.assertEqual(completed['ledger_sha256'], digest(final['ledger']))
                self.assertNotIn('ledger_after_sha256', json.loads(plan_raw))
                for name in ['restore-stopped.json', 'restored.json', 'restore-changeset-create.json',
                             'restore-changeset-execute.json', 'restore-activate-main.json', 'restore-activate-broker.json']:
                    self.assertIn('beta/promotions/' + self.promotion + '/' + name, self.fixture.s3.objects)
                self.assertEqual(len(self.aws.snapshots), 1)

    def test_a_migration_that_may_have_committed_or_is_unknown_refuses(self):
        for kind, reason in [('exit 0', 'exited 0'), ('no exit code', 'exit code unknown'), ('unobserved', 'receipt unavailable'),
                             ('start failure', 'not one known task'), ('lost start', 'unknown operation'),
                             # Review of c856fb8: a task killed from outside or by a signal may have
                             # stopped just after the commit; only the program's own failure proves it did not.
                             ('killed', 'did not fail by itself'), ('oom', 'did not fail by itself'),
                             ('exit 1 stopped from outside', 'did not fail by itself'),
                             # APP-196 MAJOR-1: the migration's reserved code once its COMMIT may have
                             # reached the server. Only exit 1 proves the COMMIT was never sent: 2 is a
                             # usage error, 120 a failed flush at interpreter shutdown, 127/128 the bounds.
                             ('exit 4', 'commit outcome unknown'), ('exit 2', 'did not fail by itself'),
                             ('exit 120', 'did not fail by itself'), ('exit 127', 'did not fail by itself'),
                             ('exit 128', 'did not fail by itself')]:
            with self.subTest(kind=kind):
                self.failed(kind)
                with self.assertRaisesRegex(Refused, reason): self.plan()
                with self.assertRaisesRegex(Refused, reason): self.apply()
                self.assertEqual(self.store.load(), self.state); self.assertEqual(self.mutations(), [])

    def test_execute_names_an_unknown_commit_outcome_and_claims_no_version_for_a_failure(self):
        for kind, reason in [('exit 4', 'migration commit outcome unknown'), ('exit 1', 'migration failed; services remain stopped')]:
            with self.subTest(kind=kind):
                self.failed(kind)
                self.assertRegex(str(self.execute_error), reason)
                self.assertNotIn('prior version', str(self.execute_error))

    def test_every_precondition_refuses_without_a_write(self):
        def tampered(update):
            return lambda: (self.store.load(), self.store.change(update))
        def in_actions(call):
            def run():
                with patch.dict('os.environ', {'GITHUB_ACTIONS': 'true'}): return call()
            return run
        executing = 'only an executing schema release can be restored'
        journal = 'restore requires the stopped execution'
        cases = [
            ('stale review', None, lambda: schema_recovery.plan(self.runner, '0' * 64, read_run=lambda _: FINISHED),
             lambda: schema_recovery.apply(self.runner, '0' * 64, read_run=lambda _: FINISHED), 'state changed since operator review'),
            ('run in progress', None, lambda: self.plan(dict(FINISHED, status='in_progress')),
             lambda: self.apply(dict(FINISHED, status='in_progress')), 'must be terminal'),
            ('fresh credentials', None, lambda: self.plan(dict(FINISHED, updated_at=(NOW - timedelta(minutes=64)).isoformat())),
             lambda: self.apply(dict(FINISHED, updated_at=(NOW - timedelta(minutes=64)).isoformat())), 'credentials are expired'),
            ('workflow role', lambda: setattr(self.aws, 'phase', 'execute'), None, None, 'workflow and execution roles'),
            ('inside Actions', None, in_actions(lambda: self.plan()), in_actions(lambda: self.apply()), 'not workflow operations'),
            ('activation intended', tampered(lambda d: d['active'].update(phase='activation-intended')), None, None, executing),
            ('restore already begun', tampered(lambda d: d['active'].update(phase='restoring')), None, None, executing),
            ('no release', tampered(lambda d: d['active']['packet'].update(schema_release=None)), None, None, executing),
            ('activation journaled', tampered(lambda d: d['active']['journal'].append({'name': 'activate-broker', 'status': 'confirmed'})), None, None, journal),
            ('restore journaled', tampered(lambda d: d['active']['journal'].append({'name': 'restore-changeset-create', 'status': 'confirmed'})), None, None, journal),
            ('duplicate stop', tampered(lambda d: d['active']['journal'].append(copy.deepcopy(next(x for x in d['active']['journal'] if x['name'] == 'stop-main')))), None, None, journal),
            ('no change set executed', tampered(lambda d: d['active'].update(journal=[x for x in d['active']['journal'] if x['name'] != 'changeset-execute'])), None, None, journal),
            ('no snapshot', tampered(lambda d: d['active'].update(journal=[x for x in d['active']['journal'] if x['name'] != 'schema-snapshot'])), None, None, journal),
            ('services running', lambda: self.aws.counts.update(main=1), None, None, 'desired count differs'),
            ('template object changed', lambda: self.fixture.s3.objects.update({'beta/promotions/' + self.promotion + '/template.json': b'{}'}), None, None, 'template object differs'),
            ('previous release about to expire', lambda: setattr(self.runner, 'clock', lambda: datetime(2026, 11, 30, 22, 30, tzinfo=timezone.utc)), None, None, 'expires too soon'),
            ('previous parameters not runtime-only', tampered(lambda d: d['live']['parameters'].update(IngressMode='private')), None, None, 'not a runtime-only restore'),
            ('observation names another task', lambda: self.rewrite_observation(lambda o: o['tasks'][0].update(taskArn=o['tasks'][0]['taskArn'][:-4] + 'ffff')), None, None, 'not a stopped task'),
            ('observation not stopped', lambda: self.rewrite_observation(lambda o: o['tasks'][0].update(lastStatus='RUNNING')), None, None, 'not a stopped task'),
        ]
        for name, mutate, plan, apply, reason in cases:
            with self.subTest(name=name):
                self.failed('exit 1')
                if mutate: mutate()
                before = self.store.load()
                with self.assertRaisesRegex(Refused, reason): (plan or self.plan)()
                with self.assertRaisesRegex(Refused, reason): (apply or self.apply)()
                self.assertEqual(self.store.load(), before); self.assertEqual(self.mutations(), [])

    def test_previous_release_that_never_becomes_healthy_keeps_the_lock(self):
        self.failed('exit 1'); self.fixture.use_fake_time(); self.aws.running_health = None
        with self.assertRaisesRegex(Refused, 'bounded wait expired'): self.apply()
        state = self.store.load()
        self.assertEqual(state['active']['phase'], 'restore-activation-intended')
        self.assertEqual(self.receipt('restore-outcome.json')['status'], 'failed-lease-held')
        self.assertNotIn('beta/promotions/' + self.promotion + '/restored.json', self.fixture.s3.objects)
        self.assertEqual(state['ledger'], self.state['ledger']); self.assertEqual(state['live']['outputs'], self.state['live']['outputs'])
        calls = len(self.aws.calls)
        # No automatic replay: a second restore refuses on the phase.
        with self.assertRaises(Refused): schema_recovery.apply(self.runner, digest(state), read_run=lambda _: FINISHED)
        self.assertEqual(len(self.aws.calls), calls)

    def test_a_failed_restored_receipt_keeps_the_lock_and_records_the_outcome(self):
        # APP-196 NIT 3: the last receipt before the final state write is inside the outcome guard.
        self.failed('exit 1'); s3 = self.fixture.s3; original = s3.put_object
        def transport(Bucket, Key, Body, **kwargs):
            if Key.endswith('/restored.json'): raise ConnectionError('synthetic: the PUT never reached S3')
            return original(Bucket=Bucket, Key=Key, Body=Body, **kwargs)
        s3.put_object = transport
        with self.assertRaises(ConnectionError): self.apply()
        self.assertEqual(self.store.load()['active']['phase'], 'restore-activation-intended')
        self.assertEqual(self.receipt('restore-outcome.json')['phase'], 'restore-activation-intended')

    def test_a_restore_change_set_outside_the_runtime_never_executes(self):
        self.failed('exit 1'); original = self.aws._call
        def widened(s, o, a):
            value = original(s, o, a)
            if o == 'create_change_set':
                self.aws.cs['Changes'].append({'ResourceChange': {'Action': 'Modify', 'LogicalResourceId': 'TaskDefinition',
                                                                  'ResourceType': 'AWS::RDS::DBInstance', 'Replacement': 'False'}})
            return value
        self.aws._call = widened
        with self.assertRaisesRegex(Refused, 'change outside runtime'): self.apply()
        self.assertFalse(any(o == 'execute_change_set' for o, _, _ in self.mutations()))
        self.assertEqual(self.store.load()['active']['phase'], 'restoring')
        self.assertEqual(self.receipt('restore-outcome.json')['status'], 'failed-lease-held')

    def test_failed_restore_update_or_lost_response_keeps_services_stopped(self):
        for fault in ['rollback', 'execute_change_set']:
            with self.subTest(fault=fault):
                self.failed('exit 1')
                if fault == 'rollback':
                    original = self.aws.stack; executed = []
                    self.aws.stack = lambda: dict(original(), StackStatus='UPDATE_ROLLBACK_COMPLETE') if executed else original()
                    call = self.aws._call
                    def transport(s, o, a):
                        value = call(s, o, a)
                        if o == 'execute_change_set': executed.append(1)
                        return value
                    self.aws._call = transport
                    with self.assertRaisesRegex(Refused, 'restore stack update failed'): self.apply()
                    self.assertEqual(self.receipt('restore-outcome.json')['phase'], 'restoring')
                else:
                    self.aws.fail = 'execute_change_set'
                    with self.assertRaises(TimeoutError): self.apply()
                    self.assertEqual(self.store.load()['active']['journal'][-1]['status'], 'intent')
                self.assertEqual(self.aws.counts, {'main': 0, 'broker': 0})
                self.assertEqual(self.store.load()['active']['phase'], 'restoring')
                self.assertFalse(any(o == 'update_service' for o, _, _ in self.mutations()))

    def test_cli_never_overwrites_its_output_and_refuses_misplaced_flags(self):
        # The plan default itself is test_cli_without_apply_only_plans_and_with_apply_only_applies:
        # this output already exists, so the command refuses before it reaches plan or apply.
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'plan.json'; output.write_text('earlier attempt')
            arguments = ['restore-after-failed-migration', '--state-bucket', 'bucket', '--expected-state-sha256', '0' * 64, '--output', str(output)]
            with patch('ops.promotion.maintenance.AWS') as aws:
                self.assertEqual(maintenance_main(arguments), 1)
                aws.assert_not_called()
            self.assertEqual(output.read_text(), 'earlier attempt')
            for wrong in [['abandon', '--state-bucket', 'b', '--apply'], ['restore-after-failed-migration', '--state-bucket', 'b']]:
                with self.assertRaises(SystemExit): maintenance_main(wrong)

    # APP-196 MINOR-1: a failed phase write leaves the state unchanged, so it must not lock the restore out.
    def test_a_failed_phase_write_after_the_plan_receipt_can_be_retried(self):
        self.failed('exit 1'); s3 = self.fixture.s3; original = s3.put_object; lost = []
        def transport(Bucket, Key, Body, **kwargs):
            if Key == 'beta/state.json' and not lost:
                lost.append(Key); raise ConnectionError('synthetic: the PUT never reached S3')
            return original(Bucket=Bucket, Key=Key, Body=Body, **kwargs)
        s3.put_object = transport
        with self.assertRaises(ConnectionError): self.apply()
        self.assertEqual(self.store.load(), self.state); self.assertEqual(self.mutations(), [])
        self.assertEqual(len([k for k in s3.objects if '/restore-plan-' in k]), 1)
        self.assertEqual(self.apply()['public']['status'], 'restored')
        plans = [k for k in s3.objects if '/restore-plan-' in k]
        self.assertEqual(len(plans), 2)
        self.assertIn('beta/' + self.store.load()['completed'][self.promotion]['restore_plan_receipt'], plans)

    # APP-196 MINOR-3: guards G1-G6 of the review, each removed by a mutation that left the suite passing.
    def test_a_pending_change_set_refuses_the_plan(self):
        self.failed('exit 1'); original = self.aws._call
        def transport(s, o, a):
            if o == 'list_change_sets':
                return {'Summaries': [{'ChangeSetName': 'operator-drift', 'Status': 'CREATE_COMPLETE', 'ExecutionStatus': 'AVAILABLE'}]}
            return original(s, o, a)
        self.aws._call = transport
        before = self.store.load()
        for call in (self.plan, self.apply):
            with self.assertRaisesRegex(Refused, 'pending change set'): call()
        self.assertEqual(self.store.load(), before); self.assertEqual(self.mutations(), [])

    def test_a_state_change_between_the_plan_and_apply_refuses_before_any_write(self):
        self.failed('exit 1'); real = schema_recovery.plan
        def plan_then_change(*args, **kwargs):
            result = real(*args, **kwargs)
            self.store.load(); self.store.change(lambda d: d['completed'].update({'concurrent': {'status': 'synthetic'}}))
            return result
        with patch.object(schema_recovery, 'plan', side_effect=plan_then_change):
            with self.assertRaisesRegex(Refused, 'state changed during the restore plan'): self.apply()
        self.assertEqual(self.store.load()['active'], self.state['active']); self.assertEqual(self.mutations(), [])
        self.assertFalse([k for k in self.fixture.s3.objects if '/restore-' in k])

    def test_an_expiry_within_ten_minutes_at_activation_refuses_before_the_intent(self):
        self.failed('exit 1'); original = self.aws._call
        expiry = preflight().timestamp(self.state['live']['parameters']['DeploymentExpiresAtUtc'])
        def transport(s, o, a):
            value = original(s, o, a)
            if o == 'execute_change_set':  # The restore's stack update took the clock to 5 minutes before expiry.
                self.runner.clock = lambda: expiry - timedelta(minutes=5)
            return value
        self.aws._call = transport
        with self.assertRaisesRegex(Refused, 'previous release expires too soon'): self.apply()
        self.assertEqual(self.store.load()['active']['phase'], 'restoring')
        self.assertFalse(any(o == 'update_service' for o, _, _ in self.mutations()))
        self.assertEqual(self.receipt('restore-outcome.json')['phase'], 'restoring')

    def test_a_restore_change_set_whose_template_differs_never_executes(self):
        self.failed('exit 1'); original = self.aws._call
        def transport(s, o, a):
            if o == 'get_template' and 'forma-restore-' in a.get('ChangeSetName', ''):
                return {'TemplateBody': json.dumps({'Resources': {}})}
            return original(s, o, a)
        self.aws._call = transport
        with self.assertRaisesRegex(Refused, 'restore change-set template differs'): self.apply()
        self.assertEqual([o for o, _, _ in self.mutations()], ['create_change_set'])
        self.assertEqual(self.store.load()['active']['phase'], 'restoring')

    def test_a_changed_or_foreign_migration_start_receipt_refuses(self):
        other = schema_tests.MIGRATION_TASK[:-4] + 'ffff'
        def foreign(receipt):
            receipt['response']['tasks'][0]['taskArn'] = other
        for name, change in [('another task', foreign), ('another request', lambda r: r['request'].update(count=2))]:
            with self.subTest(name=name):
                self.failed('exit 1')
                key = 'beta/promotions/' + self.promotion + '/schema-migrate.json'
                receipt = json.loads(self.fixture.s3.objects[key]); change(receipt)
                self.fixture.s3.objects[key] = encoded(receipt)
                # The observation agrees with the changed receipt, so only the receipt's hash binding refuses.
                self.rewrite_observation(lambda o: o['tasks'][0].update(taskArn=receipt['response']['tasks'][0]['taskArn']))
                with self.assertRaisesRegex(Refused, 'migration start receipt bytes changed'): self.plan()
                self.assert_unchanged_and_refused(self.apply)

    def test_a_lock_naming_another_promotion_refuses(self):
        for kind in ['exit 1', 'not started']:
            with self.subTest(kind=kind):
                self.failed(kind)
                self.store.load(); self.store.change(lambda d: d['active'].update(promotion_id=str(uuid.uuid4())))
                for call in (self.plan, self.apply):
                    self.assert_unchanged_and_refused(call)
                    with self.assertRaisesRegex(Refused, 'receipt'): call()

    def cli(self, output, *extra, plan=None, apply=None):
        arguments = ['restore-after-failed-migration', '--state-bucket', 'bucket', '--expected-state-sha256', '0' * 64,
                     '--output', str(output), *extra]
        stdout, stderr = io.StringIO(), io.StringIO()
        with patch('ops.promotion.maintenance.AWS'), patch('ops.promotion.maintenance.Store'), \
             patch('ops.promotion.maintenance.Runner') as runner, \
             patch.object(schema_recovery, 'plan', **(plan or {})) as planned, \
             patch.object(schema_recovery, 'apply', **(apply or {})) as applied, \
             patch('sys.stdout', stdout), patch('sys.stderr', stderr):
            code = maintenance_main(arguments)
        return code, planned, applied, runner.return_value, stdout.getvalue(), stderr.getvalue()

    def test_cli_without_apply_only_plans_and_with_apply_only_applies(self):
        detail = {'receipt': {'private': True}, 'public': {'status': 'planned'}}
        with tempfile.TemporaryDirectory() as directory:
            for extra, expected in [((), 'plan'), (('--apply',), 'apply')]:
                with self.subTest(apply=bool(extra)):
                    output = Path(directory) / (expected + '.json')
                    code, planned, applied, runner, stdout, _ = self.cli(output, *extra, plan={'return_value': detail},
                                                                         apply={'return_value': detail})
                    self.assertEqual(code, 0)
                    called, idle = (planned, applied) if expected == 'plan' else (applied, planned)
                    called.assert_called_once_with(runner, '0' * 64); idle.assert_not_called()
                    self.assertEqual(json.loads(stdout), detail['public'])
                    self.assertEqual(json.loads(output.read_bytes()), detail)
                    self.assertEqual(os.stat(output).st_mode & 0o777, 0o600)

    # APP-196 MINOR-4: the private output names the refusal; standard output and error stay generic.
    def test_a_refused_restore_writes_its_reason_only_to_the_private_output(self):
        with tempfile.TemporaryDirectory() as directory:
            reason = 'previous release expires too soon to restore'
            for error, expected in [(Refused(reason), reason), (RuntimeError('412 synthetic-private-detail'), None)]:
                with self.subTest(error=type(error).__name__):
                    output = Path(directory) / (type(error).__name__ + '.json')
                    code, _, _, _, stdout, stderr = self.cli(output, plan={'side_effect': error})
                    self.assertEqual(code, 1)
                    written = json.loads(output.read_bytes())
                    self.assertEqual(written['status'], 'refused-or-outcome-unknown')
                    self.assertEqual(written.get('reason'), expected)
                    self.assertNotIn('synthetic-private-detail', output.read_text() + stdout + stderr)
                    self.assertEqual(stdout, ''); self.assertNotIn(reason, stderr)

    def test_the_restore_accepts_only_the_runtime_s_refusal_code(self):
        # The runtime's exit codes, read from the file the migration image runs.
        source = (Path(__file__).resolve().parents[2] / 'app/runtime/runtime.py').read_text()
        pinned = re.findall(r'^MIGRATION_COMMIT_OUTCOME_UNKNOWN = ([0-9]+)$', source, re.M)
        self.assertEqual(pinned, [str(schema_release.MIGRATION_COMMIT_OUTCOME_UNKNOWN)])
        self.assertEqual(schema_release.MIGRATION_REFUSED, 1)
        self.assertNotIn(schema_release.MIGRATION_COMMIT_OUTCOME_UNKNOWN, (0, 1, 2, 3, 120))


if __name__ == '__main__':
    unittest.main()
