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
# ADR-003 amendment 2026-10-30: the stack's provider capability secret (an output).
CAPABILITY_SECRET = 'arn:aws:secretsmanager:us-east-1:877573132739:secret:ProviderCapabilities-AbCdEf'
# APP-078 B1: the stack's hosted sign-in configuration secret (an output).
AUTH_SECRET = 'arn:aws:secretsmanager:us-east-1:877573132739:secret:AuthConfiguration-GhIjKl'
INJECTED = [{'name': 'GENUI_PROVIDER_CAPABILITIES_JSON', 'valueFrom': CAPABILITY_SECRET},
            {'name': 'GENUI_AUTH_CONFIGURATION_JSON', 'valueFrom': AUTH_SECRET}]
RETIRED = 'arn:aws:secretsmanager:us-east-1:877573132739:secret:genui-qualification-fixture-Abc123'


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
                   'ClusterName': 'owned-cluster', 'ServiceName': 'owned-service', 'TaskDefinition': 'arn:aws:ecs:us-east-1:877573132739:task-definition/owned-task:1',
                   'ProviderCapabilitySecretArn': CAPABILITY_SECRET, 'AuthConfigurationSecretArn': AUTH_SECRET}
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
        definition = {'taskDefinitionArn': outputs['TaskDefinition'], 'runtimePlatform': {'cpuArchitecture': config['image_architecture']},
                      'containerDefinitions': [{'name': 'workspace', 'image': config['image_digest'],
                                                'secrets': list(INJECTED)}]}
        calls = []
        def call(*args):
            calls.append(args)
            if args[:2] == ('sts', 'get-caller-identity'): return {'Account': '877573132739'}
            if args[:2] == ('cloudformation', 'describe-stacks'): return {'Stacks': [stack]}
            if args[:2] == ('scheduler', 'get-schedule'): return schedule
            if args[:2] == ('ecs', 'describe-task-definition'): return {'taskDefinition': copy.deepcopy(definition)}
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
        self.assertEqual([x[:2] for x in calls], [('sts', 'get-caller-identity'), ('cloudformation', 'describe-stacks'), ('scheduler', 'get-schedule'), ('ecs', 'describe-task-definition'), ('ecs', 'describe-services'), ('ecs', 'update-service'), ('ecs', 'describe-services')])
        self.assertEqual(calls[2], ('scheduler', 'get-schedule', '--group-name', 'owned-group', '--name', 'expiry'))

    def test_retired_qualification_secret_is_accepted_only_empty_and_keeps_disabled_bindings(self):
        """APP-074 review m11: the stack no longer offers the qualification secret parameter."""
        config, ledger = inputs()
        existing, report = preflight.reserve(config, ledger, NOW)
        self.assertNotIn('QualificationConfigSecretArn', report['cloudformation_parameters'])
        explicit_empty, same = preflight.reserve(dict(config, qualification_config_secret_arn=''), existing, NOW)
        self.assertEqual(explicit_empty, existing)
        self.assertEqual(same['input_sha256'], report['input_sha256'])
        for bad in (RETIRED, None, False, 1, {}, 'synthetic-not-an-arn', RETIRED + '\n'):
            with self.assertRaisesRegex(ValueError, 'retired'):
                preflight.reserve(dict(config, qualification_config_secret_arn=bad), ledger, NOW)
        with self.assertRaisesRegex(ValueError, 'closed contract'):
            preflight.reserve(dict(config, qualification_config_json='synthetic'), ledger, NOW)

    def test_an_older_stack_may_carry_the_retired_parameter_only_empty(self):
        for value, accepted in (('', True), (RETIRED, False)):
            config, report, stack, _, _, calls, call = self.activation_fixture()
            stack['Parameters'].append({'ParameterKey': 'QualificationConfigSecretArn', 'ParameterValue': value})
            if accepted:
                self.assertEqual(preflight.activate(config, report, call, lambda: NOW)['desired_count'], 1)
            else:
                with self.assertRaisesRegex(ValueError, 'qualification secret parameter'):
                    preflight.activate(config, report, call, lambda: NOW)
                self.assertFalse(any(c[:2] == ('ecs', 'update-service') for c in calls))

    def test_live_task_image_architecture_and_secret_injection_must_match(self):
        for mutation in ('definition', 'image', 'architecture', 'extra_container', 'unexpected_secret'):
            config, report, _, _, _, calls, call = self.activation_fixture()
            def changed(*args):
                value = call(*args)
                if args[:2] == ('ecs', 'describe-task-definition'):
                    definition = value['taskDefinition']; container = definition['containerDefinitions'][0]
                    if mutation == 'definition': definition['taskDefinitionArn'] += '9'
                    if mutation == 'image': container['image'] = config['image_digest'][:-1] + 'b'
                    if mutation == 'architecture': definition['runtimePlatform']['cpuArchitecture'] = 'X86_64'
                    if mutation == 'extra_container': definition['containerDefinitions'].append(dict(container))
                    if mutation == 'unexpected_secret': container['secrets'] = [{'name': 'PGPASSWORD', 'valueFrom': 'synthetic'}]
                return value
            with self.assertRaises(ValueError): preflight.activate(config, report, changed, lambda: NOW)
            self.assertFalse(any(c[:2] == ('ecs', 'update-service') for c in calls), mutation)

    def test_the_capability_secret_is_the_one_exact_live_injection(self):
        """ADR-003 amendment 2026-10-30 and APP-078 B1: exactly the stack's capability secret and its auth
        configuration secret, under runtime.py's names, in either order."""
        for mutation in ('correct', 'reordered', 'missing', 'missing_auth', 'missing_capabilities', 'wrong_arn', 'wrong_auth_arn',
                         'swapped', 'wrong_name', 'extra', 'qualification'):
            config, report, _, _, _, calls, call = self.activation_fixture()
            def changed(*args):
                value = call(*args)
                if args[:2] == ('ecs', 'describe-task-definition'):
                    secrets = copy.deepcopy(INJECTED)
                    if mutation == 'reordered': secrets.reverse()
                    if mutation == 'missing': secrets = []
                    if mutation == 'missing_auth': secrets = secrets[:1]
                    if mutation == 'missing_capabilities': secrets = secrets[1:]
                    if mutation == 'wrong_arn': secrets[0]['valueFrom'] = CAPABILITY_SECRET.replace('AbCdEf', 'GhIjKl')
                    if mutation == 'wrong_auth_arn': secrets[1]['valueFrom'] = AUTH_SECRET.replace('GhIjKl', 'MnOpQr')
                    if mutation == 'swapped': secrets[0]['valueFrom'], secrets[1]['valueFrom'] = secrets[1]['valueFrom'], secrets[0]['valueFrom']
                    if mutation == 'wrong_name': secrets[0]['name'] = 'GENUI_PROVIDER_CAPABILITY_DIR'
                    if mutation == 'extra': secrets.append({'name': 'PGPASSWORD', 'valueFrom': 'synthetic'})
                    if mutation == 'qualification': secrets.append({'name': 'GENUI_AUTH_QUALIFICATION_CONFIG_JSON', 'valueFrom': RETIRED})
                    value['taskDefinition']['containerDefinitions'][0]['secrets'] = secrets
                return value
            if mutation in ('correct', 'reordered'):
                self.assertEqual(preflight.activate(config, report, changed, lambda: NOW)['desired_count'], 1)
            else:
                with self.assertRaisesRegex(ValueError, 'secret injection differs'):
                    preflight.activate(config, report, changed, lambda: NOW)
                self.assertFalse(any(c[:2] == ('ecs', 'update-service') for c in calls), mutation)

    def test_plain_environment_files_and_startup_cannot_bypass_the_secret_channel(self):
        for mutation in ('raw_json', 'config_file', 'future_qualification_field', 'capabilities_json', 'capability_dir',
                         'auth_configuration_json', 'auth_config_file', 'identity_registry_file', 'environment_file', 'entrypoint', 'command'):
            config, report, _, _, _, calls, call = self.activation_fixture()
            def changed(*args):
                value = call(*args)
                if args[:2] == ('ecs', 'describe-task-definition'):
                    container = value['taskDefinition']['containerDefinitions'][0]
                    names = {'raw_json': 'GENUI_AUTH_QUALIFICATION_CONFIG_JSON', 'config_file': 'GENUI_AUTH_QUALIFICATION_CONFIG_FILE',
                             'future_qualification_field': 'GENUI_AUTH_QUALIFICATION_UNREVIEWED',
                             'capabilities_json': 'GENUI_PROVIDER_CAPABILITIES_JSON', 'capability_dir': 'GENUI_PROVIDER_CAPABILITY_DIR',
                             'auth_configuration_json': 'GENUI_AUTH_CONFIGURATION_JSON', 'auth_config_file': 'GENUI_AUTH_CONFIG_FILE',
                             'identity_registry_file': 'GENUI_IDENTITY_REGISTRY_FILE'}
                    if mutation in names: container['environment'] = [{'name': names[mutation], 'value': 'synthetic'}]
                    if mutation == 'environment_file': container['environmentFiles'] = [{'type': 's3', 'value': 'arn:aws:s3:::synthetic/config.env'}]
                    if mutation == 'entrypoint': container['entryPoint'] = ['sh', '-c']
                    if mutation == 'command': container['command'] = ['qualify']
                return value
            with self.assertRaises(ValueError): preflight.activate(config, report, changed, lambda: NOW)
            self.assertFalse(any(c[:2] == ('ecs', 'update-service') for c in calls), mutation)

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


WINDOW = datetime(2026, 11, 1, 17, tzinfo=timezone.utc)
QUALIFICATION = '877573132739.dkr.ecr.us-east-1.amazonaws.com/genui-qualification@sha256:' + 'b' * 64
OPERATOR = '877573132739.dkr.ecr.us-east-1.amazonaws.com/genui-operator@sha256:' + 'c' * 64


def always_on(now=WINDOW):
    """The APP-078 shape: always-on plan, backstop at the end of November."""
    config, _ = inputs(now)
    config.update(operating_plan='always-on-beta', ingress_mode='public', deployment_expires_at_utc='2026-11-30T23:00:00Z',
                  extra_task_hours=3, qualification_image_digest=QUALIFICATION, operator_image_digest=OPERATOR)
    ledger = {'schema_version': 2, 'scope': 'genui-beta-incremental', 'operating_plan': 'always-on-beta', 'windows': {},
              'months': {'2026-11': {'reconciled_at_utc': now.strftime('%Y-%m-%dT%H:%M:%SZ'), 'basis': 'Synthetic fixture, not AWS billing evidence',
                                     'prior_extra_spend_cents': 0, 'variable_reserve_cents': 1500,
                                     'auth_spent_or_reserved_cents': 3800, 'unallocated_cents': 0}}}
    return config, ledger


class AlwaysOnPlan(unittest.TestCase):
    def test_always_on_reserves_the_whole_month_within_the_approved_figure(self):
        config, ledger = always_on()
        updated, report = preflight.reserve(config, ledger, WINDOW)
        month = report['cost_by_month']['2026-11']
        self.assertEqual(set(report['cost_by_month']), {'2026-11'})
        self.assertEqual(month['retained_core_full_month_cents'], 5074)
        # 17:00 Nov 1 to 23:00 Nov 30 plus the one-hour margin is 703 hours, plus three extra task-hours.
        self.assertEqual(month['all_window_compute_reservations_cents'], 8580)
        self.assertEqual(month['genui_total_cents'], 15154)
        self.assertEqual(month['project_total_cents'], 18954)
        self.assertEqual((report['operating_plan'], report['genui_allocation_cents'], report['monthly_budget_cents']), ('always-on-beta', 17000, 20800))
        parameters = report['cloudformation_parameters']
        self.assertEqual(parameters['MonthlyBudgetUsd'], '170')
        self.assertEqual((parameters['QualificationImageDigest'], parameters['OperatorImageDigest']), (QUALIFICATION, OPERATOR))
        self.assertEqual(parameters['ServiceDesiredCount'], '0')
        self.assertIn('backstop', report['limits'][1])
        self.assertEqual(updated['operating_plan'], 'always-on-beta')

    def test_plan_and_ledger_schema_must_agree(self):
        config, ledger = always_on()
        legacy = dict(ledger, schema_version=1); del legacy['operating_plan']
        with self.assertRaises(ValueError): preflight.reserve(config, legacy, WINDOW)
        with self.assertRaisesRegex(ValueError, 'operating plan differs'):
            preflight.reserve(config, dict(ledger, operating_plan='bounded-window'), WINDOW)
        bounded, _ = inputs(WINDOW)
        with self.assertRaises(ValueError): preflight.reserve(bounded, ledger, WINDOW)
        for plan in ['always-on', 'ALWAYS-ON-BETA', '', None, 1]:
            with self.assertRaisesRegex(ValueError, 'unknown operating plan'):
                preflight.reserve(dict(config, operating_plan=plan), ledger, WINDOW)

    def test_always_on_expiry_is_a_bounded_backstop(self):
        config, ledger = always_on()
        preflight.reserve(dict(config, deployment_expires_at_utc='2026-12-03T16:59:59Z'), copy.deepcopy(ledger) | {'months': ledger['months'] | {'2026-12': ledger['months']['2026-11']}}, WINDOW)
        for expiry in ['2026-12-03T17:00:01Z', '2026-11-01T17:09:59Z']:
            with self.assertRaisesRegex(ValueError, '32 days'):
                preflight.reserve(dict(config, deployment_expires_at_utc=expiry), ledger, WINDOW)
        bounded, book = inputs(WINDOW)
        with self.assertRaisesRegex(ValueError, '48 hours'):
            preflight.reserve(dict(bounded, deployment_expires_at_utc='2026-11-03T17:00:01Z'), book, WINDOW)

    def test_always_on_auth_reservation_and_genui_cap(self):
        for auth in [3299, 3801]:
            config, ledger = always_on(); ledger['months']['2026-11']['auth_spent_or_reserved_cents'] = auth
            with self.assertRaisesRegex(ValueError, 'fAuth'): preflight.reserve(config, ledger, WINDOW)
        config, ledger = always_on(); ledger['months']['2026-11']['prior_extra_spend_cents'] = 17000 - 15154
        preflight.reserve(config, ledger, WINDOW)
        ledger['months']['2026-11']['prior_extra_spend_cents'] += 1
        with self.assertRaisesRegex(ValueError, 'budget exceeded for 2026-11'): preflight.reserve(config, ledger, WINDOW)
        config, ledger = always_on(); ledger['months']['2026-11']['unallocated_cents'] = 20800 - 18954 + 1
        with self.assertRaisesRegex(ValueError, 'budget exceeded'): preflight.reserve(config, ledger, WINDOW)
        config, ledger = always_on(); ledger['months']['2026-11']['variable_reserve_cents'] = 999
        with self.assertRaises(ValueError): preflight.reserve(config, ledger, WINDOW)

    def test_task_image_digests_are_validated_and_bound(self):
        config, ledger = always_on()
        for key in ['qualification_image_digest', 'operator_image_digest']:
            for value in ['123456789012.dkr.ecr.us-east-1.amazonaws.com/genui-operator@sha256:' + 'c' * 64,
                          '877573132739.dkr.ecr.us-east-1.amazonaws.com/genui-operator:latest', config['image_digest'], OPERATOR + '\n', None]:
                with self.assertRaises(ValueError, msg=(key, value)): preflight.reserve(dict(config, **{key: value}), ledger, WINDOW)
        with self.assertRaisesRegex(ValueError, 'distinct'):
            preflight.reserve(dict(config, qualification_image_digest=OPERATOR), ledger, WINDOW)
        updated, report = preflight.reserve(config, ledger, WINDOW)
        with self.assertRaisesRegex(ValueError, 'already has different'):
            preflight.reserve(dict(config, operator_image_digest=OPERATOR[:-1] + 'd'), updated, WINDOW)
        # Ingress mode and desired count may change within the reserved window.
        again, same = preflight.reserve(dict(config, service_desired_count=1), updated, WINDOW + timedelta(hours=2))
        self.assertEqual((again, same['input_sha256']), (updated, report['input_sha256']))

    def test_empty_task_images_and_default_plan_keep_earlier_bindings(self):
        config, ledger = inputs()
        _, report = preflight.reserve(config, ledger, NOW)
        explicit = dict(config, qualification_image_digest='', operator_image_digest='', operating_plan='bounded-window')
        _, same = preflight.reserve(explicit, ledger, NOW)
        self.assertEqual(same['input_sha256'], report['input_sha256'])
        self.assertEqual((same['cloudformation_parameters']['QualificationImageDigest'], same['cloudformation_parameters']['OperatorImageDigest']), ('', ''))
        self.assertEqual(same['cloudformation_parameters']['MonthlyBudgetUsd'], '100')

    def activation(self):
        config, ledger = always_on(); config['service_desired_count'] = 1
        _, report = preflight.reserve(config, ledger, WINDOW)
        outputs = {'DeploymentExpiryScheduleArn': 'arn:aws:scheduler:us-east-1:877573132739:schedule/owned-group/expiry',
                   'DeploymentExpiryGroup': 'owned-group', 'DeploymentExpiryRoleArn': 'arn:aws:iam::877573132739:role/owned-expiry',
                   'ClusterName': 'owned-cluster', 'ServiceName': 'owned-service', 'TaskDefinition': 'arn:aws:ecs:us-east-1:877573132739:task-definition/owned-task:3',
                   'ProviderCapabilitySecretArn': CAPABILITY_SECRET, 'AuthConfigurationSecretArn': AUTH_SECRET}
        cluster = 'arn:aws:ecs:us-east-1:877573132739:cluster/owned-cluster'
        service = 'arn:aws:ecs:us-east-1:877573132739:service/owned-cluster/owned-service'
        stack = {'StackStatus': 'UPDATE_COMPLETE', 'Parameters': [{'ParameterKey': k, 'ParameterValue': str(v)} for k, v in report['cloudformation_parameters'].items()],
                 'Outputs': [{'OutputKey': k, 'OutputValue': v} for k, v in outputs.items()]}
        schedule = {'Arn': outputs['DeploymentExpiryScheduleArn'], 'State': 'ENABLED', 'ScheduleExpressionTimezone': 'UTC',
                    'ScheduleExpression': 'at(2026-11-30T23:00:00)', 'FlexibleTimeWindow': {'Mode': 'OFF'},
                    'Target': {'Arn': 'arn:aws:scheduler:::aws-sdk:ecs:updateService', 'RoleArn': outputs['DeploymentExpiryRoleArn'],
                               'Input': json.dumps({'Cluster': cluster, 'Service': service, 'DesiredCount': 0}),
                               'RetryPolicy': {'MaximumEventAgeInSeconds': 900, 'MaximumRetryAttempts': 3}}}
        current = {'serviceArn': service, 'clusterArn': cluster, 'taskDefinition': outputs['TaskDefinition'], 'status': 'ACTIVE', 'desiredCount': 0}
        definition = {'taskDefinitionArn': outputs['TaskDefinition'], 'runtimePlatform': {'cpuArchitecture': 'ARM64'},
                      'containerDefinitions': [{'name': 'workspace', 'image': config['image_digest'],
                                                'secrets': list(INJECTED)}]}
        calls = []
        def call(*args):
            calls.append(args)
            if args[:2] == ('sts', 'get-caller-identity'): return {'Account': '877573132739'}
            if args[:2] == ('cloudformation', 'describe-stacks'): return {'Stacks': [stack]}
            if args[:2] == ('scheduler', 'get-schedule'): return schedule
            if args[:2] == ('ecs', 'describe-task-definition'): return {'taskDefinition': copy.deepcopy(definition)}
            if args[:2] == ('ecs', 'describe-services'): return {'services': [dict(current)], 'failures': []}
            if args[:2] == ('ecs', 'update-service'):
                current['desiredCount'] = int(args[-1]); return {'service': dict(current)}
            raise AssertionError(args)
        return config, report, stack, calls, call

    def test_always_on_activation_accepts_the_month_backstop_and_checks_task_images(self):
        config, report, _, calls, call = self.activation()
        self.assertEqual(preflight.activate(config, report, call, lambda: WINDOW + timedelta(hours=1))['desired_count'], 1)
        for key in ['QualificationImageDigest', 'OperatorImageDigest', 'MonthlyBudgetUsd']:
            config, report, stack, calls, call = self.activation()
            parameter = next(p for p in stack['Parameters'] if p['ParameterKey'] == key)
            parameter['ParameterValue'] = '100' if key == 'MonthlyBudgetUsd' else ''
            with self.assertRaisesRegex(ValueError, 'deployed parameters differ'):
                preflight.activate(config, report, call, lambda: WINDOW + timedelta(hours=1))
            self.assertFalse(any(c[:2] == ('ecs', 'update-service') for c in calls), key)
        # Too close to the backstop: never start compute that would stop within ten minutes.
        config, report, _, calls, call = self.activation()
        with self.assertRaises(ValueError):
            preflight.activate(config, report, call, lambda: datetime(2026, 11, 30, 22, 51, tzinfo=timezone.utc))
        self.assertFalse(any(c[:2] == ('ecs', 'update-service') for c in calls))

    def test_legacy_stack_without_task_image_parameters_is_disabled_only(self):
        config, ledger = inputs(); config['service_desired_count'] = 1
        _, report = preflight.reserve(config, ledger, NOW)
        fixture = Preflight('test_activation_checks_live_stop_and_service_before_count_one').activation_fixture()
        _, _, stack, _, _, _, call = fixture
        stack['Parameters'] = [p for p in stack['Parameters'] if p['ParameterKey'] not in ('QualificationImageDigest', 'OperatorImageDigest')]
        self.assertEqual(preflight.activate(fixture[0], fixture[1], call, lambda: NOW)['desired_count'], 1)


if __name__ == '__main__':
    unittest.main()
