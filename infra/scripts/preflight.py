#!/usr/bin/env python3
"""Reserve a bounded deployment in a private local ledger.

The ledger is an operator-reconciled planning record, not a billing feed or an
AWS spending cap. Reservations survive failed/lost deployments and are never
released automatically. Keep this file outside Git with the deployment inputs.
Default mode calls no AWS API. Explicit --activate verifies the live stack and
future stop schedule, then requests ECS desiredCount one. It never provisions.

Two operating plans exist. `bounded-window` (the default; ledger schema 1) is
ADR-003's $100/month 48-hour qualification window. `always-on-beta` (ledger
schema 2) is Victor's approval of 2026-10-29 recorded in ADR-008: GenUI's
always-on synthetic beta at about $150-170/month, with FormaStack's lean fAuth
(about $33, up to $38) reserved separately. The always-on stop schedule is a
backstop at most 32 days away, renewed with a fresh reconciled reservation.
"""
import argparse
import calendar
import copy
from datetime import datetime, timedelta, timezone
from decimal import Decimal, ROUND_CEILING
import fcntl
import hashlib
import ipaddress
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import tempfile
import uuid

UTC = timezone.utc
CAP = 10000
GENUI_CAP = 7000
MIN_VARIABLE_RESERVE = 1000
BOUNDED, ALWAYS_ON = 'bounded-window', 'always-on-beta'
# Closed operating plans. Caps are code, never ledger input: a ledger names its
# plan and the plan's figures come only from the decision they cite.
PLANS = {
    BOUNDED: {'ledger_schema': 1, 'monthly_budget_usd': '100', 'genui_cap_cents': GENUI_CAP, 'project_cap_cents': CAP,
              'max_expiry': timedelta(hours=48), 'decision': 'ADR-003 $100/month bounded 48-hour window'},
    # ADR-008, "Victor's decisions of 2026-10-29 (evening)": about $150-170/month
    # for GenUI (x86-priced) on top of FormaStack's lean fAuth (about $33, up to
    # $38). The project cap is exactly those two approved upper figures.
    ALWAYS_ON: {'ledger_schema': 2, 'monthly_budget_usd': '170', 'genui_cap_cents': 17000, 'project_cap_cents': 17000 + 3800,
                'max_expiry': timedelta(days=32), 'auth_min_cents': 3300, 'auth_max_cents': 3800,
                'decision': 'ADR-008 always-on beta: GenUI about $150-170/month plus FormaStack lean fAuth about $33 (up to $38)'},
}
DIGEST_RE = r'877573132739\.dkr\.ecr\.us-east-1\.amazonaws\.com/[a-z0-9]+(?:[._-][a-z0-9]+)*(?:/[a-z0-9]+(?:[._-][a-z0-9]+)*)*@sha256:[0-9a-f]{64}'
# Optional inputs: missing and explicitly empty (or the default plan) mean
# disabled and are left out of the window binding, so earlier windows keep
# their hashes.
OPTIONAL_INPUTS = ('qualification_config_secret_arn', 'qualification_image_digest', 'operator_image_digest', 'operating_plan')
# Stack parameters an older deployed template may lack; absent means disabled.
OPTIONAL_PARAMETERS = ('QualificationImageDigest', 'OperatorImageDigest')
# Conservative Linux x86 price even when deploying an ARM64 image.
TASK_HOURLY = Decimal('0.1215248')  # 2 CPU + 8 GiB + task IPv4
FIXED_HOURLY = Decimal('0.0645')   # RDS + ALB + two ALB IPv4
FIXED_STORAGE_KEYS = Decimal('4.30')  # 20 GiB RDS + two tenant keys


def require(condition, message):
    if not condition:
        raise ValueError(message)


def exact_keys(value, keys, label):
    require(isinstance(value, dict) and set(value) == set(keys), label + ': fields do not match the closed contract')


def integer(value, minimum=0):
    return type(value) is int and minimum <= value <= 100000000


def timestamp(value):
    require(isinstance(value, str) and re.fullmatch(r'20\d{2}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z', value), 'timestamp must be whole-second UTC with Z')
    return datetime.strptime(value, '%Y-%m-%dT%H:%M:%SZ').replace(tzinfo=UTC)


def cents(value):
    return int((value * 100).to_integral_value(rounding=ROUND_CEILING))


def months_between(start, end):
    result = {}
    while start < end:
        next_month = datetime(start.year + (start.month == 12), 1 if start.month == 12 else start.month + 1, 1, tzinfo=UTC)
        stop = min(end, next_month)
        result[start.strftime('%Y-%m')] = Decimal(str((stop - start).total_seconds())) / 3600
        start = stop
    return result


def plan_of(config):
    plan = config.get('operating_plan', BOUNDED) if isinstance(config, dict) else BOUNDED
    require(isinstance(plan, str) and plan in PLANS, 'unknown operating plan')
    return plan


def reserve(config, ledger, now):
    optional = [k for k in OPTIONAL_INPUTS if isinstance(config, dict) and k in config]
    exact_keys(config, ['window_id', 'account', 'region', 'hostname', 'operator', 'ingress_mode', 'operator_ipv4_cidr', 'deployment_expires_at_utc', 'tls_certificate_arn', 'image_digest', 'image_architecture', 'service_desired_count', 'extra_task_hours'] + optional, 'deployment')
    plan = plan_of(config)
    rules = PLANS[plan]
    # APP-074 review m11: the qualification secret injection is retired (the
    # runtime refuses it in every mode). The key is accepted only empty, so
    # earlier disabled windows keep their exact binding.
    require(config.get('qualification_config_secret_arn', '') == '', 'qualification configuration secret is retired; leave it empty')
    require(isinstance(config['image_digest'], str) and re.fullmatch(DIGEST_RE, config['image_digest']), 'reviewed same-account/region ECR digest required')
    # APP-075/APP-077 standalone task images: empty (no task definition) or a
    # reviewed same-account digest that is never the service image.
    task_images = {}
    for key in ('qualification_image_digest', 'operator_image_digest'):
        value = config.get(key, '')
        require(isinstance(value, str) and re.fullmatch('(?:' + DIGEST_RE + ')?', value) is not None, key + ' must be empty or a reviewed same-account/region ECR digest')
        require(value != config['image_digest'], key + ' must never be the service image')
        task_images[key] = value
    require(not task_images['qualification_image_digest'] or task_images['qualification_image_digest'] != task_images['operator_image_digest'], 'qualification and operator images must be distinct artifacts')
    require(str(uuid.UUID(config['window_id'])) == config['window_id'], 'window_id must be a canonical UUID retained across retry')
    require(config['account'] == '877573132739' and config['region'] == 'us-east-1', 'unexpected account or region')
    require(config['hostname'] == 'beta.getforma.dev', 'unexpected beta hostname')
    require(isinstance(config['operator'], str) and 0 < len(config['operator'].strip()) <= 100, 'named operator required')
    require(config['ingress_mode'] in ('operator', 'public'), 'invalid ingress mode')
    address = ipaddress.ip_network(config['operator_ipv4_cidr'], strict=True)
    require(address.version == 4 and address.prefixlen == 32 and address.network_address.is_global, 'operator address must be a real public IPv4 /32')
    expires = timestamp(config['deployment_expires_at_utc'])
    require(now + timedelta(minutes=10) <= expires <= now + rules['max_expiry'], 'expiry must be 10 minutes to %s in the future' % ('48 hours' if plan == BOUNDED else '32 days'))
    require(re.fullmatch(r'arn:aws:acm:us-east-1:877573132739:certificate/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}', config['tls_certificate_arn']), 'certificate must belong to the selected account/region')
    require(config['image_architecture'] in ('ARM64', 'X86_64'), 'invalid image architecture')
    require(type(config['service_desired_count']) is int and config['service_desired_count'] in (0, 1), 'desired count must be zero or one')
    require(integer(config['extra_task_hours'], 2), 'reserve at least two additional task-hours for migration, qualification and deployment overlap')
    if plan == BOUNDED:
        exact_keys(ledger, ['schema_version', 'scope', 'months', 'windows'], 'ledger')
    else:
        exact_keys(ledger, ['schema_version', 'scope', 'operating_plan', 'months', 'windows'], 'ledger')
        require(ledger['operating_plan'] == plan, 'ledger operating plan differs from the deployment input')
    require(ledger['schema_version'] == rules['ledger_schema'] and ledger['scope'] == 'genui-beta-incremental', 'unexpected ledger scope/version for the operating plan')
    require(isinstance(ledger['months'], dict) and isinstance(ledger['windows'], dict), 'invalid ledger collections')
    # The same prepaid window covers migration at count zero, its /32 trial,
    # and public/count-one operation. Neither transition extends its deadline.
    # Missing and explicitly empty both mean disabled. Preserve the exact
    # binding of earlier disabled windows; enabling/changing a secret is a new
    # immutable input and cannot repurpose an already reserved window.
    immutable = {k: v for k, v in config.items() if k not in ('ingress_mode', 'service_desired_count')
                 and not (k in OPTIONAL_INPUTS and v in ('', BOUNDED))}
    encoded = json.dumps(immutable, sort_keys=True, separators=(',', ':')).encode()
    binding = hashlib.sha256(encoded).hexdigest()
    for key, old in ledger['windows'].items():
        require(str(uuid.UUID(key)) == key, 'invalid retained window identity')
        exact_keys(old, ['input_sha256', 'created_at_utc', 'expires_at_utc', 'reserved_compute_cents_by_month'], 'retained window')
        require(re.fullmatch('[0-9a-f]{64}', old['input_sha256']) is not None, 'invalid retained input digest')
        timestamp(old['created_at_utc']); timestamp(old['expires_at_utc'])
        require(isinstance(old['reserved_compute_cents_by_month'], dict) and all(integer(v, 1) for v in old['reserved_compute_cents_by_month'].values()), 'invalid retained reservation')
    existing = ledger['windows'].get(config['window_id'])
    require(existing is None or existing['input_sha256'] == binding, 'window identity already has different deployment inputs; do not overwrite an unknown outcome')
    active = months_between(now, expires + timedelta(hours=1))
    # A migration/rollout can occur on either side of UTC month-end. Reserve
    # its complete allowance in every touched month rather than hiding it in
    # the first month's headroom.
    for month in active:
        active[month] += config['extra_task_hours']
    reserved = existing['reserved_compute_cents_by_month'] if existing else {month: cents(hours * TASK_HOURLY) for month, hours in active.items()}
    updated = copy.deepcopy(ledger)
    updated['windows'][config['window_id']] = existing or {
        'input_sha256': binding, 'created_at_utc': now.strftime('%Y-%m-%dT%H:%M:%SZ'),
        'expires_at_utc': config['deployment_expires_at_utc'], 'reserved_compute_cents_by_month': reserved,
    }
    reports = {}
    for month in active:
        require(month in ledger['months'], 'missing reconciled ledger month ' + month)
        entry = ledger['months'][month]
        exact_keys(entry, ['reconciled_at_utc', 'basis', 'prior_extra_spend_cents', 'variable_reserve_cents', 'auth_spent_or_reserved_cents', 'unallocated_cents'], 'ledger month')
        reconciled = timestamp(entry['reconciled_at_utc'])
        require(now - timedelta(hours=24) <= reconciled <= now, 'ledger must be reconciled within the last 24 hours')
        require(isinstance(entry['basis'], str) and 0 < len(entry['basis'].strip()) <= 1000, 'ledger reconciliation basis required')
        require(all(integer(entry[k]) for k in ['prior_extra_spend_cents', 'variable_reserve_cents', 'auth_spent_or_reserved_cents', 'unallocated_cents']), 'ledger cents must be nonnegative integers')
        require(entry['variable_reserve_cents'] >= MIN_VARIABLE_RESERVE, 'retain at least $10 for probes, secrets, storage, logs and requests')
        if plan == BOUNDED:
            require(entry['auth_spent_or_reserved_cents'] >= 500 and entry['unallocated_cents'] == 2500, 'retain the separate $5 auth allowance and $25 unallocated')
        else:
            # FormaStack's approved lean fAuth is reserved at its quoted figure,
            # never below "about $33" and never above its quoted $38.
            require(rules['auth_min_cents'] <= entry['auth_spent_or_reserved_cents'] <= rules['auth_max_cents'], 'reserve FormaStack lean fAuth at its approved $33-38 figure')
        year, number = (int(x) for x in month.split('-'))
        retained = cents(FIXED_HOURLY * calendar.monthrange(year, number)[1] * 24 + FIXED_STORAGE_KEYS)
        windows = sum(w['reserved_compute_cents_by_month'].get(month, 0) for w in updated['windows'].values())
        genui = retained + windows + entry['prior_extra_spend_cents'] + entry['variable_reserve_cents']
        total = genui + entry['auth_spent_or_reserved_cents'] + entry['unallocated_cents']
        require(genui <= rules['genui_cap_cents'] and total <= rules['project_cap_cents'], 'budget exceeded for ' + month + '; reconcile prior reservations or change the reviewed operating plan')
        reports[month] = {'retained_core_full_month_cents': retained, 'all_window_compute_reservations_cents': windows,
                          'prior_extra_spend_cents': entry['prior_extra_spend_cents'], 'variable_reserve_cents': entry['variable_reserve_cents'],
                          'genui_total_cents': genui, 'auth_spent_or_reserved_cents': entry['auth_spent_or_reserved_cents'],
                          'unallocated_cents': entry['unallocated_cents'], 'project_total_cents': total}
    parameters = {'Operator': config['operator'], 'MonthlyBudgetUsd': rules['monthly_budget_usd'], 'IngressMode': config['ingress_mode'],
                  'OperatorIpv4Cidr': config['operator_ipv4_cidr'], 'DeploymentExpiresAtUtc': config['deployment_expires_at_utc'],
                  'TlsCertificateArn': config['tls_certificate_arn'], 'ImageDigest': config['image_digest'],
                  'ImageArchitecture': config['image_architecture'], 'ServiceDesiredCount': '0',
                  'QualificationImageDigest': task_images['qualification_image_digest'],
                  'OperatorImageDigest': task_images['operator_image_digest']}
    return updated, {'status': 'reserved_local_plan_only', 'window_id': config['window_id'], 'input_sha256': binding,
                     'observed_at_utc': now.strftime('%Y-%m-%dT%H:%M:%SZ'), 'reused_reservation': existing is not None,
                     'operating_plan': plan, 'decision': rules['decision'],
                     'monthly_budget_cents': rules['project_cap_cents'], 'genui_allocation_cents': rules['genui_cap_cents'], 'cost_by_month': reports,
                     'cloudformation_parameters': parameters,
                     'limits': ['No AWS resource, certificate, image, account state or billing feed was queried.',
                                'Scheduler requests ECS desiredCount zero; it is not a billing cap, teardown or a guarantee of timely delivery.'
                                + (' For the always-on beta it is a backstop that must be renewed, with a new reconciled reservation, before it fires.' if plan == ALWAYS_ON else ''),
                                'RDS, ALB, two ALB IPv4 addresses, storage and keys remain charged after ECS stops.',
                                'Retain this ledger across failed deployments, retries and renewed windows; never reset known spending to zero.',
                                'Renewals, standalone tasks, actual charges above reserves and every later month need reconciliation; model spending remains zero.']}


def activate(config, report, call, clock):
    """The only count-one path; a past at() schedule must never start compute."""
    require(config['service_desired_count'] == 1, 'activation requires explicit desired count one')
    require(call('sts', 'get-caller-identity')['Account'] == config['account'], 'activation AWS account mismatch')
    stack = call('cloudformation', 'describe-stacks', '--stack-name', 'GenuiSyntheticFoundation')['Stacks'][0]
    require(stack['StackStatus'] in ('CREATE_COMPLETE', 'UPDATE_COMPLETE'), 'stack must be complete before activation')
    parameters = {v['ParameterKey']: v['ParameterValue'] for v in stack['Parameters']}
    expected = report['cloudformation_parameters']
    # An older deployed template may still carry the retired parameter: only empty.
    require(parameters.get('QualificationConfigSecretArn', '') == '', 'deployed qualification secret parameter must be empty or absent')
    require(all(parameters.get(k, '' if k in OPTIONAL_PARAMETERS else None) == str(v) for k, v in expected.items()), 'deployed parameters differ from the reserved plan')
    outputs = {v['OutputKey']: v['OutputValue'] for v in stack['Outputs']}
    arn = outputs['DeploymentExpiryScheduleArn']
    prefix = 'arn:aws:scheduler:us-east-1:877573132739:schedule/'
    require(arn.startswith(prefix), 'unexpected stop schedule account/region')
    group, name = arn[len(prefix):].split('/')
    require(group == outputs['DeploymentExpiryGroup'], 'stop schedule group mismatch')
    schedule = call('scheduler', 'get-schedule', '--group-name', group, '--name', name)
    require(schedule['Arn'] == arn and schedule['State'] == 'ENABLED' and schedule['ScheduleExpressionTimezone'] == 'UTC', 'stop schedule must be enabled in UTC')
    require(schedule['ScheduleExpression'] == 'at(' + config['deployment_expires_at_utc'][:-1] + ')', 'stop schedule expiry mismatch')
    require(schedule['FlexibleTimeWindow'] == {'Mode': 'OFF'}, 'stop schedule cannot have a flexible window')
    target = schedule['Target']
    require(target['Arn'] == 'arn:aws:scheduler:::aws-sdk:ecs:updateService' and target['RoleArn'] == outputs['DeploymentExpiryRoleArn'], 'stop target/role mismatch')
    require(target['RetryPolicy'] == {'MaximumEventAgeInSeconds': 900, 'MaximumRetryAttempts': 3}, 'stop retry policy mismatch')
    payload = json.loads(target['Input'], object_pairs_hook=no_duplicates)
    exact_keys(payload, ['Cluster', 'Service', 'DesiredCount'], 'stop target')
    require(type(payload['DesiredCount']) is int and payload['DesiredCount'] == 0, 'stop target desired count must be the integer zero')
    cluster_arn = 'arn:aws:ecs:us-east-1:877573132739:cluster/' + outputs['ClusterName']
    service_arn = 'arn:aws:ecs:us-east-1:877573132739:service/' + outputs['ClusterName'] + '/' + outputs['ServiceName']
    require(payload == {'Cluster': cluster_arn, 'Service': service_arn, 'DesiredCount': 0}, 'stop target must address this exact cluster/service with count zero')
    definition = call('ecs', 'describe-task-definition', '--task-definition', outputs['TaskDefinition'])['taskDefinition']
    require(definition['taskDefinitionArn'] == outputs['TaskDefinition'], 'unexpected live task definition')
    containers = definition['containerDefinitions']
    require(len(containers) == 1 and containers[0]['name'] == 'workspace', 'unexpected application container boundary')
    require(containers[0]['image'] == config['image_digest'] and definition['runtimePlatform']['cpuArchitecture'] == config['image_architecture'], 'live image or architecture differs from the reserved plan')
    require(containers[0].get('entryPoint', []) == [] and containers[0].get('command', []) == [], 'live service must use the reviewed image startup')
    require(containers[0].get('environmentFiles', []) == [] and
            not any(item['name'].startswith(('GENUI_AUTH_QUALIFICATION_', 'GENUI_PROVIDER_CAPABILIT')) for item in containers[0].get('environment', [])),
            'live qualification or capability configuration cannot bypass the reviewed secret channel')
    # ADR-003 amendment 2026-10-30: the one injected secret is the stack's provider
    # capability secret, under the name runtime.py serve() consumes.
    expected_secrets = [{'name': 'GENUI_PROVIDER_CAPABILITIES_JSON', 'valueFrom': outputs['ProviderCapabilitySecretArn']}]
    require(containers[0].get('secrets', []) == expected_secrets, 'live secret injection differs from the reviewed stack')
    def current_service():
        result = call('ecs', 'describe-services', '--cluster', cluster_arn, '--services', service_arn)
        require(not result.get('failures') and len(result['services']) == 1, 'intended service is unavailable')
        service = result['services'][0]
        require(service['serviceArn'] == service_arn and service['clusterArn'] == cluster_arn and service['status'] == 'ACTIVE', 'unexpected live service identity/status')
        require(service['taskDefinition'] == outputs['TaskDefinition'], 'live service task revision differs from the reviewed stack')
        require(type(service['desiredCount']) is int and service['desiredCount'] in (0, 1), 'live service desired count is outside the bounded plan')
        return service
    current_service()
    observed = clock()
    require(observed + timedelta(minutes=10) <= timestamp(config['deployment_expires_at_utc']) <= observed + PLANS[plan_of(config)]['max_expiry'], 'live expiry is too close, past or beyond the bounded window')
    # UpdateService is an immediate control-plane call after the live check,
    # never a count-one request hidden behind a slow CloudFormation deployment.
    result = call('ecs', 'update-service', '--cluster', cluster_arn, '--service', service_arn, '--desired-count', '1')
    service = result['service']
    require(service['serviceArn'] == service_arn and service['clusterArn'] == cluster_arn and type(service['desiredCount']) is int and service['desiredCount'] == 1, 'activation response did not confirm the intended service')
    current = current_service()
    require(current['desiredCount'] == 1, 'activation is not current; do not restart an expired or stopped window')
    if clock() >= timestamp(config['deployment_expires_at_utc']):
        # A late/unknown control-plane response must not report active success
        # after the sole scheduled stop. Make one bounded compensating stop.
        call('ecs', 'update-service', '--cluster', cluster_arn, '--service', service_arn, '--desired-count', '0')
        raise ValueError('expiry elapsed during activation; stop requested and outcome must be reconciled')
    return {'status': 'activation_requested_not_healthy', 'observed_at_utc': observed.strftime('%Y-%m-%dT%H:%M:%SZ'),
            'cluster_arn': cluster_arn, 'service_arn': service_arn, 'schedule_arn': arn,
            'expires_at_utc': config['deployment_expires_at_utc'], 'desired_count': 1,
            'limit': 'Retain the reservation on failure/unknown outcome. Verify desiredCount zero and stopped tasks at expiry; this API response is not readiness or a billing cutoff.'}


def aws_call(*args):
    environment = dict(os.environ, AWS_MAX_ATTEMPTS='1', AWS_PAGER='')
    result = subprocess.run(['aws', *args, '--region', 'us-east-1', '--output', 'json', '--cli-connect-timeout', '5', '--cli-read-timeout', '10'],
                            capture_output=True, text=True, env=environment, timeout=20)
    require(result.returncode == 0, 'AWS operation failed or outcome is unknown; reconcile the retained window before retry')
    return json.loads(result.stdout, object_pairs_hook=no_duplicates)


def no_duplicates(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON field')
        result[key] = value
    return result


def read_json(path):
    with open(path, 'r', encoding='utf-8') as stream:
        return json.load(stream, object_pairs_hook=no_duplicates)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, required=True)
    parser.add_argument('--ledger', type=Path, required=True, help='Existing private operator ledger; never initialized or zeroed automatically')
    parser.add_argument('--activate', action='store_true', help='Explicit AWS mutation: verify completed stack/current future stop, then request desiredCount one')
    args = parser.parse_args()
    # A separate persistent lock survives the atomic replacement of the ledger.
    lock = os.open(str(args.ledger) + '.lock', os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    with os.fdopen(lock, 'r+') as locked:
        fcntl.flock(locked, fcntl.LOCK_EX)
        fd = os.open(args.ledger, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(fd, 'r') as stream:
            info = os.fstat(stream.fileno())
            require(stat.S_ISREG(info.st_mode) and info.st_uid == os.geteuid() and info.st_mode & 0o077 == 0, 'ledger must be an owned private regular file')
            ledger = json.load(stream, object_pairs_hook=no_duplicates)
        config = read_json(args.input)
        require(args.activate or config.get('service_desired_count') == 0, 'desired count one requires explicit --activate after deployed schedule verification')
        updated, report = reserve(config, ledger, datetime.now(UTC))
        fd, temporary = tempfile.mkstemp(prefix='.budget-', dir=args.ledger.parent)
        try:
            with os.fdopen(fd, 'w') as stream:
                os.fchmod(stream.fileno(), 0o600)
                json.dump(updated, stream, indent=2); stream.write('\n'); stream.flush(); os.fsync(stream.fileno())
            os.replace(temporary, args.ledger)
            directory = os.open(args.ledger.parent, os.O_RDONLY)
            try: os.fsync(directory)
            finally: os.close(directory)
        finally:
            if os.path.exists(temporary): os.unlink(temporary)
        if args.activate:
            report['activation'] = activate(config, report, aws_call, lambda: datetime.now(UTC))
            report['limits'][0] = 'Activation checked live account, completed stack, stop schedule and service identity/count; billing, certificate issuance and registry contents were not queried.'
        print(json.dumps(report, indent=2))


if __name__ == '__main__':
    try:
        main()
    except (ValueError, KeyError, TypeError, OSError, json.JSONDecodeError, subprocess.TimeoutExpired) as error:
        raise SystemExit('Preflight refused: ' + str(error))
