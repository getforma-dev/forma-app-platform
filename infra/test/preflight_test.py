import copy
from datetime import datetime, timedelta, timezone
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

PATH = Path(__file__).parents[1] / 'scripts/preflight.py'
spec = importlib.util.spec_from_file_location('preflight', PATH)
preflight = importlib.util.module_from_spec(spec)
spec.loader.exec_module(preflight)
NOW = datetime(2026, 10, 28, 12, tzinfo=timezone.utc)


def inputs(now=NOW):
    config = {'window_id': '00000000-0000-4000-8000-000000000001', 'account': '877573132739', 'region': 'us-east-1',
              'hostname': 'beta.getforma.dev', 'operator': 'Synthetic test operator', 'ingress_mode': 'operator',
              'operator_ipv4_cidr': '8.8.8.8/32', 'deployment_expires_at_utc': (now + timedelta(hours=48)).strftime('%Y-%m-%dT%H:%M:%SZ'),
              'tls_certificate_arn': 'arn:aws:acm:us-east-1:877573132739:certificate/00000000-0000-4000-8000-000000000001',
              'image_digest': '877573132739.dkr.ecr.us-east-1.amazonaws.com/genui@sha256:' + 'a' * 64,
              'image_architecture': 'ARM64', 'service_desired_count': 0, 'extra_task_hours': 2}
    ledger = {'schema_version': 1, 'scope': 'genui-beta-incremental', 'months': {}, 'windows': {}}
    for month in preflight.months_between(now, now + timedelta(hours=49)):
        ledger['months'][month] = {'reconciled_at_utc': now.strftime('%Y-%m-%dT%H:%M:%SZ'), 'basis': 'Synthetic fixture, not AWS billing evidence',
                                   'prior_extra_spend_cents': 0, 'variable_reserve_cents': 1000,
                                   'auth_spent_or_reserved_cents': 500, 'unallocated_cents': 2500}
    return config, ledger


class Preflight(unittest.TestCase):
    def activation_fixture(self):
        config, ledger = inputs(); config['service_desired_count'] = 1
        _, report = preflight.reserve(config, ledger, NOW)
        outputs = {'DeploymentExpiryScheduleArn': 'arn:aws:scheduler:us-east-1:877573132739:schedule/owned-group/expiry',
                   'DeploymentExpiryGroup': 'owned-group', 'DeploymentExpiryRoleArn': 'arn:aws:iam::877573132739:role/owned-expiry',
                   'ClusterName': 'owned-cluster', 'ServiceName': 'owned-service', 'TaskDefinition': 'arn:aws:ecs:us-east-1:877573132739:task-definition/owned-task:1'}
        cluster = 'arn:aws:ecs:us-east-1:877573132739:cluster/owned-cluster'
        service = 'arn:aws:ecs:us-east-1:877573132739:service/owned-cluster/owned-service'
        stack = {'StackStatus': 'CREATE_COMPLETE', 'Parameters': [{'ParameterKey': k, 'ParameterValue': str(v)} for k, v in report['cloudformation_parameters'].items()],
                 'Outputs': [{'OutputKey': k, 'OutputValue': v} for k, v in outputs.items()]}
        schedule = {'Arn': outputs['DeploymentExpiryScheduleArn'], 'State': 'ENABLED', 'ScheduleExpressionTimezone': 'UTC',
                    'ScheduleExpression': 'at(' + config['deployment_expires_at_utc'][:-1] + ')', 'FlexibleTimeWindow': {'Mode': 'OFF'},
                    'Target': {'Arn': 'arn:aws:scheduler:::aws-sdk:ecs:updateService', 'RoleArn': outputs['DeploymentExpiryRoleArn'],
                               'Input': json.dumps({'Cluster': cluster, 'Service': service, 'DesiredCount': 0}),
                               'RetryPolicy': {'MaximumEventAgeInSeconds': 900, 'MaximumRetryAttempts': 3}}}
        current = {'serviceArn': service, 'clusterArn': cluster, 'taskDefinition': outputs['TaskDefinition'], 'status': 'ACTIVE', 'desiredCount': 0}
        calls = []
        def call(*args):
            calls.append(args)
            if args[:2] == ('sts', 'get-caller-identity'): return {'Account': '877573132739'}
            if args[:2] == ('cloudformation', 'describe-stacks'): return {'Stacks': [stack]}
            if args[:2] == ('scheduler', 'get-schedule'): return schedule
            if args[:2] == ('ecs', 'describe-services'): return {'services': [dict(current)], 'failures': []}
            if args[:2] == ('ecs', 'update-service'):
                current['desiredCount'] = int(args[-1]); return {'service': dict(current)}
            raise AssertionError(args)
        return config, report, stack, schedule, current, calls, call

    def test_activation_checks_live_stop_and_service_before_count_one(self):
        config, report, _, _, _, calls, call = self.activation_fixture()
        self.assertEqual(report['cloudformation_parameters']['ServiceDesiredCount'], '0')
        outcome = preflight.activate(config, report, call, lambda: NOW)
        self.assertEqual(outcome['status'], 'activation_requested_not_healthy')
        self.assertEqual([x[:2] for x in calls], [('sts', 'get-caller-identity'), ('cloudformation', 'describe-stacks'), ('scheduler', 'get-schedule'), ('ecs', 'describe-services'), ('ecs', 'update-service'), ('ecs', 'describe-services')])
        self.assertEqual(calls[2], ('scheduler', 'get-schedule', '--group-name', 'owned-group', '--name', 'expiry'))

    def test_activation_refuses_stale_stack_schedule_wrong_service_and_near_expiry(self):
        for mutation in ('stack', 'disabled', 'schedule_arn', 'group', 'target', 'role', 'task', 'near_expiry'):
            config, report, stack, schedule, current, calls, call = self.activation_fixture()
            clock = lambda: NOW
            if mutation == 'stack': stack['StackStatus'] = 'CREATE_IN_PROGRESS'
            if mutation == 'disabled': schedule['State'] = 'DISABLED'
            if mutation == 'schedule_arn': schedule['Arn'] += '-another-schedule'
            if mutation == 'group':
                next(item for item in stack['Outputs'] if item['OutputKey'] == 'DeploymentExpiryGroup')['OutputValue'] = 'default'
            if mutation == 'target': schedule['Target']['Input'] = json.dumps({'Cluster': current['clusterArn'], 'Service': 'another-service', 'DesiredCount': 0})
            if mutation == 'role': schedule['Target']['RoleArn'] = 'arn:aws:iam::877573132739:role/another-role'
            if mutation == 'task': current['taskDefinition'] += '9'
            if mutation == 'near_expiry': clock = lambda: NOW + timedelta(hours=48) - timedelta(minutes=9)
            with self.assertRaises(ValueError): preflight.activate(config, report, call, clock)
            self.assertFalse(any(x[:2] == ('ecs', 'update-service') for x in calls), mutation)

    def test_stop_count_requires_integer_zero_not_boolean_float_or_string(self):
        for value in [False, 0.0, '0', None, True, 1]:
            config, report, _, schedule, _, calls, call = self.activation_fixture()
            payload = json.loads(schedule['Target']['Input']); payload['DesiredCount'] = value
            schedule['Target']['Input'] = json.dumps(payload)
            with self.assertRaisesRegex(ValueError, 'integer zero'):
                preflight.activate(config, report, call, lambda: NOW)
            self.assertFalse(any(x[:2] == ('ecs', 'update-service') for x in calls), repr(value))

    def test_live_service_count_must_be_an_integer_within_the_plan(self):
        for value in [False, 0.0, True, 1.0, 2, -1]:
            config, report, _, _, current, calls, call = self.activation_fixture()
            current['desiredCount'] = value
            with self.assertRaisesRegex(ValueError, 'outside the bounded plan'):
                preflight.activate(config, report, call, lambda: NOW)
            self.assertFalse(any(x[:2] == ('ecs', 'update-service') for x in calls), repr(value))

    def test_activation_late_response_requests_stop_instead_of_reporting_success(self):
        config, report, _, _, current, calls, call = self.activation_fixture()
        clock_values = iter([NOW, NOW + timedelta(hours=49)])
        with self.assertRaisesRegex(ValueError, 'stop requested'):
            preflight.activate(config, report, call, lambda: next(clock_values))
        self.assertEqual([x[-1] for x in calls if x[:2] == ('ecs', 'update-service')], ['1', '0'])
        self.assertEqual(current['desiredCount'], 0)

    def test_public_transition_reuses_window_without_resetting_expiry_or_reservation(self):
        config, ledger = inputs()
        updated, report = preflight.reserve(config, ledger, NOW)
        self.assertEqual(report['cost_by_month']['2026-10']['retained_core_full_month_cents'], 5229)
        self.assertEqual(report['cost_by_month']['2026-10']['all_window_compute_reservations_cents'], 620)
        self.assertEqual(report['cost_by_month']['2026-10']['genui_total_cents'], 6849)
        config.update(ingress_mode='public', service_desired_count=1)
        second, public = preflight.reserve(config, updated, NOW + timedelta(minutes=5))
        self.assertEqual(updated, second)
        self.assertTrue(public['reused_reservation'])
        self.assertEqual(public['cloudformation_parameters']['DeploymentExpiresAtUtc'], config['deployment_expires_at_utc'])
        self.assertEqual(public['cloudformation_parameters']['IngressMode'], 'public')

    def test_window_identity_rejects_extended_time_or_changed_artifact(self):
        config, ledger = inputs()
        updated, _ = preflight.reserve(config, ledger, NOW)
        for field, value in [('deployment_expires_at_utc', '2026-10-30T11:00:00Z'), ('image_architecture', 'X86_64')]:
            changed = dict(config, **{field: value})
            with self.assertRaisesRegex(ValueError, 'already has different'):
                preflight.reserve(changed, updated, NOW)

    def test_new_window_cannot_reset_prior_reservations(self):
        config, ledger = inputs()
        updated, _ = preflight.reserve(config, ledger, NOW)
        config['window_id'] = '00000000-0000-4000-8000-000000000002'
        with self.assertRaisesRegex(ValueError, 'budget exceeded'):
            preflight.reserve(config, updated, NOW)
        self.assertEqual(len(updated['windows']), 1)

    def test_known_spending_and_auth_overrun_reduce_remaining_budget(self):
        for changes in [{'prior_extra_spend_cents': 152}, {'auth_spent_or_reserved_cents': 652}]:
            config, ledger = inputs()
            ledger['months']['2026-10'].update(changes)
            with self.assertRaisesRegex(ValueError, 'budget exceeded'):
                preflight.reserve(config, ledger, NOW)

    def test_calendar_expiry_and_future_boundaries(self):
        for expiry in ['2026-02-30T12:00:00Z', '2026-10-28T12:00:00Z', '2026-10-30T12:00:01Z', '2026-10-29T12:00:00+00:00', '2026-10-29T12:00:00Z\n']:
            config, ledger = inputs(); config['deployment_expires_at_utc'] = expiry
            with self.assertRaises(ValueError): preflight.reserve(config, ledger, NOW)

    def test_month_rollover_reserves_retained_resources_in_both_months(self):
        now = datetime(2026, 12, 31, 12, tzinfo=timezone.utc)
        config, ledger = inputs(now)
        _, report = preflight.reserve(config, ledger, now)
        self.assertEqual(set(report['cost_by_month']), {'2026-12', '2027-01'})
        self.assertEqual(report['cost_by_month']['2027-01']['retained_core_full_month_cents'], 5229)
        self.assertEqual(report['cost_by_month']['2027-01']['all_window_compute_reservations_cents'], 474)
        # Reserving its two extra task-hours only in December would incorrectly
        # admit this January ledger within $70.
        ledger['months']['2027-01']['prior_extra_spend_cents'] = 320
        with self.assertRaisesRegex(ValueError, 'budget exceeded for 2027-01'):
            preflight.reserve(config, ledger, now)
        ledger['months']['2027-01']['prior_extra_spend_cents'] = 0
        del ledger['months']['2027-01']
        with self.assertRaisesRegex(ValueError, 'missing reconciled ledger month'):
            preflight.reserve(config, ledger, now)

    def test_refuses_stale_ledger_missing_allowances_and_invalid_cidr(self):
        config, ledger = inputs()
        for field, value in [('reconciled_at_utc', '2026-10-27T11:59:59Z'), ('variable_reserve_cents', 999), ('auth_spent_or_reserved_cents', 499), ('unallocated_cents', 0), ('prior_extra_spend_cents', True)]:
            bad = copy.deepcopy(ledger); bad['months']['2026-10'][field] = value
            with self.assertRaises(ValueError): preflight.reserve(config, bad, NOW)
        for network in ['0.0.0.0/0', '8.8.8.0/24', '127.0.0.1/32', '10.0.0.1/32', '203.0.113.1/32', '::/0']:
            with self.assertRaises(ValueError): preflight.reserve(dict(config, operator_ipv4_cidr=network), ledger, NOW)

    def test_cli_persists_private_reservation_and_retry_does_not_charge_twice(self):
        now = datetime.now(timezone.utc).replace(microsecond=0)
        config, ledger = inputs(now)
        # A whole-second deadline is slightly less than 48 hours at process start.
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'input.json'; book = Path(directory) / 'ledger.json'
            source.write_text(json.dumps(config)); book.write_text(json.dumps(ledger)); book.chmod(0o600)
            command = [sys.executable, str(PATH), '--input', str(source), '--ledger', str(book)]
            first = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(first.returncode, 0, first.stderr)
            self.assertEqual(book.stat().st_mode & 0o777, 0o600)
            saved = book.read_bytes()
            second = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(second.returncode, 0, second.stderr)
            self.assertTrue(json.loads(second.stdout)['reused_reservation'])
            self.assertEqual(saved, book.read_bytes())
            book.chmod(0o644)
            failure = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(failure.returncode, 0)
            self.assertIn('private regular file', failure.stderr)

    def test_cli_will_not_initialize_a_missing_ledger(self):
        config, _ = inputs(datetime.now(timezone.utc).replace(microsecond=0))
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'input.json'; source.write_text(json.dumps(config))
            book = Path(directory) / 'missing.json'
            result = subprocess.run([sys.executable, str(PATH), '--input', str(source), '--ledger', str(book)], capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(book.exists())

    def test_concurrent_preflights_cannot_spend_the_same_remaining_allowance(self):
        now = datetime.now(timezone.utc).replace(microsecond=0)
        config, ledger = inputs(now)
        _, report = preflight.reserve(config, ledger, now)
        month = next(iter(report['cost_by_month']))
        ledger['months'][month]['prior_extra_spend_cents'] = 7000 - report['cost_by_month'][month]['genui_total_cents']
        with tempfile.TemporaryDirectory() as directory:
            book = Path(directory) / 'ledger.json'; book.write_text(json.dumps(ledger)); book.chmod(0o600)
            running = []
            for index in [1, 2]:
                candidate = dict(config, window_id=f'00000000-0000-4000-8000-00000000000{index}')
                source = Path(directory) / f'input-{index}.json'; source.write_text(json.dumps(candidate))
                running.append(subprocess.Popen([sys.executable, str(PATH), '--input', str(source), '--ledger', str(book)], stdout=subprocess.PIPE, stderr=subprocess.PIPE))
            results = [process.communicate() for process in running]
            self.assertEqual(sorted(p.returncode for p in running), [0, 1], results)
            self.assertEqual(len(json.loads(book.read_text())['windows']), 1)


if __name__ == '__main__':
    unittest.main()
