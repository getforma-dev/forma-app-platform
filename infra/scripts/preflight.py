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

A redeploy of the same service (new image digests) is a new window, because a
window's inputs are immutable. It may declare `supersedes_window_id`: the
earlier window of the same plan, service and account then stays recorded, but
counts only its elapsed compute up to the supersession (plus its task-hour
allowance in the months already used); its future compute is replaced by the
new window's. A window is superseded at most once; supersession is never a
cycle, never deletes a record and never edits the earlier reservation figures.

The supersession is recorded when the new window is reserved (its count-zero
preflight, before the change set), because the budget check needs it then.
If that redeploy is given up before its activation, `--abandon WINDOW` undoes
it: the earlier window counts its whole reservation again and can be retried
and activated again (after the stack is put back on its template and
parameters), while the abandoned window keeps only its elapsed compute and
task-hour allowance and can never be retried, activated or superseded. Only a
window whose record shows no activation request can be abandoned: `--activate`
records the request time in the ledger before it calls AWS, and a record from
before that field existed is refused (its activation state is unknown).
"""
import argparse
import calendar
import copy
from datetime import datetime, timedelta, timezone
from decimal import Decimal, ROUND_CEILING, ROUND_FLOOR
import fcntl
import hashlib
import importlib.util
import ipaddress
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import tempfile
import uuid



def _load_common():
    # Beside this file, by path (never sys.path), so `python3 -I` works too.
    spec = importlib.util.spec_from_file_location('genui_infra_common', Path(__file__).resolve().parent / 'common.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


_COMMON = _load_common()
ACCOUNT, REGION, HOSTNAME, STACK = _COMMON.ACCOUNT, _COMMON.REGION, _COMMON.HOSTNAME, _COMMON.STACK
_ACCOUNT_RE, _REGION_RE = re.escape(ACCOUNT), re.escape(REGION)
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
DIGEST_RE = _ACCOUNT_RE + r'\.dkr\.ecr\.' + _REGION_RE + r'\.amazonaws\.com/[a-z0-9]+(?:[._-][a-z0-9]+)*(?:/[a-z0-9]+(?:[._-][a-z0-9]+)*)*@sha256:[0-9a-f]{64}'
# Optional inputs: missing and explicitly empty (or the default plan) mean
# disabled and are left out of the window binding, so earlier windows keep
# their hashes.
OPTIONAL_INPUTS = ('qualification_config_secret_arn', 'qualification_image_digest', 'operator_image_digest', 'operating_plan',
                   'provider_management_secret_arn', 'provider_management_url', 'supersedes_window_id')
# Stack parameters an older deployed template may lack; absent means disabled.
OPTIONAL_PARAMETERS = ('QualificationImageDigest', 'OperatorImageDigest', 'ProviderManagementSecretArn', 'ProviderManagementUrl')
# ADR-003 amendment 2026-10-30 (managed invitations, revised after FormaStack's answers; approval A12):
# FormaStack's existing management secret, attached by ARN to the operator task for the window and
# the trial and cleared afterwards. Attaching and clearing cost nothing and change no compute, so,
# like the ingress mode and the desired count, they may change within one reserved window and are
# not part of its binding. Both or neither; this account and region only.
MANAGEMENT_SECRET_RE = r'arn:aws:secretsmanager:' + _REGION_RE + ':' + _ACCOUNT_RE + r':secret:[A-Za-z0-9/_+=.@-]{1,512}-[A-Za-z0-9]{6}'
MANAGEMENT_URL_RE = r'https://[a-z0-9](?:[a-z0-9.-]{0,251}[a-z0-9])?(?::[0-9]{1,5})?'
WITHIN_WINDOW = ('ingress_mode', 'service_desired_count', 'provider_management_secret_arn', 'provider_management_url')
# Conservative Linux x86 price even when deploying an ARM64 image.
TASK_HOURLY = Decimal('0.1215248')  # 2 CPU + 8 GiB + task IPv4
FIXED_HOURLY = Decimal('0.0645')   # RDS + ALB + two ALB IPv4
FIXED_STORAGE_KEYS = Decimal('4.30')  # 20 GiB RDS + two tenant keys
# Retained window records: the original four fields never change. A window
# reserved since supersession support records its service identity; a window
# that supersedes names its predecessor; a superseded window gains the three
# SUPERSEDED fields (who, when, and the compute it still counts).
WINDOW_FIELDS = ('input_sha256', 'created_at_utc', 'expires_at_utc', 'reserved_compute_cents_by_month')
SUPERSEDED = ('superseded_by_window_id', 'superseded_at_utc', 'counted_compute_cents_by_month')
# A successor given up before activation (`--abandon`): it no longer names its predecessor as
# superseded, records which one it was going to replace, and counts only what it already used.
ABANDONED = ('abandoned_at_utc', 'abandoned_supersession_of_window_id', 'counted_compute_cents_by_month')
# '' until `--activate` first records its request time (before any AWS call); absent on records
# written before this field existed, whose activation state is unknown.
ACTIVATION = 'activation_requested_at_utc'
IDENTITY_FIELDS = ('operating_plan', 'account', 'region', 'hostname')
# Every window reserved before identities were recorded was pinned by reserve()
# to this one account, region and hostname (the GenuiSyntheticFoundation beta).
LEGACY_SERVICE = {'account': ACCOUNT, 'region': REGION, 'hostname': HOSTNAME}


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


def floor_cents(value):
    return int((value * 100).to_integral_value(rounding=ROUND_FLOOR))


def canonical_window(value, label):
    require(isinstance(value, str), label + ' must be a canonical UUID')
    try:
        canonical = str(uuid.UUID(value))
    except ValueError:
        canonical = None
    require(canonical == value, label + ' must be a canonical UUID')
    return value


def identity_of(record, ledger_plan):
    """The plan/account/region/hostname a retained window was reserved for."""
    return record.get('service_identity') or dict(LEGACY_SERVICE, operating_plan=ledger_plan)


def validate_windows(windows, ledger_plan):
    """Closed record contract plus a consistent, acyclic, single-successor supersession graph."""
    for key, old in windows.items():
        canonical_window(key, 'retained window identity')
        optional = [k for k in dict.fromkeys(('service_identity', 'supersedes_window_id', ACTIVATION) + SUPERSEDED + ABANDONED)
                    if isinstance(old, dict) and k in old]
        exact_keys(old, list(WINDOW_FIELDS) + optional, 'retained window')
        require(re.fullmatch('[0-9a-f]{64}', old['input_sha256']) is not None, 'invalid retained input digest')
        created = timestamp(old['created_at_utc']); timestamp(old['expires_at_utc'])
        reserved = old['reserved_compute_cents_by_month']
        require(isinstance(reserved, dict) and all(integer(v, 1) for v in reserved.values()), 'invalid retained reservation')
        if 'service_identity' in old:
            exact_keys(old['service_identity'], IDENTITY_FIELDS, 'retained window service identity')
            require(old['service_identity']['operating_plan'] == ledger_plan, 'retained window was reserved under another operating plan')
        if ACTIVATION in old:
            require(old[ACTIVATION] == '' or timestamp(old[ACTIVATION]) >= created, 'invalid activation request time')
        abandoned = 'abandoned_at_utc' in old or 'abandoned_supersession_of_window_id' in old
        if abandoned:
            require(all(k in old for k in ABANDONED) and not any(k in old for k in ('superseded_by_window_id', 'superseded_at_utc', 'supersedes_window_id')),
                    'an abandoned window records its time, intended predecessor and counted compute, and takes part in no supersession')
            require(old.get(ACTIVATION) == '', 'an abandoned window never requested activation')
            require(timestamp(old['abandoned_at_utc']) >= created, 'abandonment precedes the reservation')
            require(canonical_window(old['abandoned_supersession_of_window_id'], 'abandoned_supersession_of_window_id') in windows,
                    'the window an abandoned window was going to supersede is missing')
        marks = [k for k in SUPERSEDED if k in old]
        require(abandoned or len(marks) in (0, len(SUPERSEDED)), 'a superseded window records its successor, time and counted compute together')
        if 'counted_compute_cents_by_month' in old:
            counted = old['counted_compute_cents_by_month']
            require(isinstance(counted, dict) and set(counted) == set(reserved)
                    and all(integer(counted[m]) and counted[m] <= reserved[m] for m in reserved), 'invalid counted compute of a superseded or abandoned window')
        if marks and not abandoned:
            successor = windows.get(canonical_window(old['superseded_by_window_id'], 'superseded_by_window_id'))
            require(successor is not None and successor.get('supersedes_window_id') == key, 'supersession is not recorded on both windows')
            superseded_at = timestamp(old['superseded_at_utc'])
            require(superseded_at == timestamp(successor['created_at_utc']) and superseded_at >= created, 'supersession time differs from the successor reservation')
        if 'supersedes_window_id' in old:
            earlier = windows.get(canonical_window(old['supersedes_window_id'], 'supersedes_window_id'))
            require(earlier is not None and earlier.get('superseded_by_window_id') == key, 'superseded window is missing or does not name its successor')
    for key in windows:
        seen, current = {key}, windows[key].get('supersedes_window_id')
        while current is not None:
            require(current not in seen, 'supersession cycle in the ledger')
            seen.add(current)
            current = windows[current].get('supersedes_window_id')


def counted_compute(record):
    return record.get('counted_compute_cents_by_month', record['reserved_compute_cents_by_month'])


def supersede(earlier, now):
    """What the earlier window still counts once superseded at `now`.

    Elapsed compute up to `now` stays counted, and so does the task-hour
    allowance in every month already begun; the rest of the month's reserved
    window time is released (rounded down, so never more than was reserved
    for it) and months that have not begun are released entirely.
    """
    created, expires = timestamp(earlier['created_at_utc']), timestamp(earlier['expires_at_utc'])
    unused = months_between(max(now, created), expires + timedelta(hours=1))
    counted = {}
    for month, reserved in earlier['reserved_compute_cents_by_month'].items():
        year, number = (int(x) for x in month.split('-'))
        if datetime(year, number, 1, tzinfo=UTC) >= now:
            counted[month] = 0
        else:
            counted[month] = max(0, reserved - floor_cents(unused.get(month, Decimal(0)) * TASK_HOURLY))
    return counted


def abandon(ledger, window_id, now):
    """Undo an unactivated successor's supersession (see the module docstring).

    The earlier window loses its superseded marks and counts its whole original
    reservation again; the abandoned window keeps its record, counts only the
    compute it already used (as a superseded window would) and is refused by
    every later reserve, activation or supersession. Never deletes a record.
    """
    require(isinstance(ledger, dict) and isinstance(ledger.get('windows'), dict), 'invalid ledger collections')
    plan = ledger.get('operating_plan', BOUNDED)
    require(plan in PLANS and ledger.get('schema_version') == PLANS[plan]['ledger_schema'], 'unexpected ledger scope/version for the operating plan')
    windows = ledger['windows']
    validate_windows(windows, plan)
    record = windows.get(canonical_window(window_id, 'abandoned window'))
    require(record is not None, 'window ' + window_id + ' is not in the ledger')
    require('abandoned_at_utc' not in record, 'window ' + window_id + ' was already abandoned')
    require('supersedes_window_id' in record, 'only a window that supersedes another can be abandoned')
    require('superseded_by_window_id' not in record, 'window ' + window_id + ' was itself superseded; abandon its successor first')
    require(ACTIVATION in record, 'window ' + window_id + ' was reserved before activation requests were recorded; its activation state is unknown')
    require(record[ACTIVATION] == '', 'window ' + window_id + ' requested activation at ' + str(record[ACTIVATION]) + '; fix forward or supersede it')
    require(timestamp(record['created_at_utc']) <= now, 'window was reserved after this time')
    earlier_id = record['supersedes_window_id']
    updated = copy.deepcopy(ledger)
    earlier = updated['windows'][earlier_id]
    for key in ('superseded_by_window_id', 'superseded_at_utc', 'counted_compute_cents_by_month'):
        del earlier[key]
    kept = updated['windows'][window_id]
    del kept['supersedes_window_id']
    kept.update(abandoned_at_utc=now.strftime('%Y-%m-%dT%H:%M:%SZ'), abandoned_supersession_of_window_id=earlier_id,
                counted_compute_cents_by_month=supersede(record, now))
    validate_windows(updated['windows'], plan)
    return updated, {'status': 'abandoned_local_plan_only', 'window_id': window_id, 'restored_window_id': earlier_id,
                     'observed_at_utc': now.strftime('%Y-%m-%dT%H:%M:%SZ'),
                     'abandoned_counted_compute_cents_by_month': kept['counted_compute_cents_by_month'],
                     'restored_reserved_compute_cents_by_month': earlier['reserved_compute_cents_by_month'],
                     'limits': ['No AWS API was called. The live stack is unchanged: put it back on the restored window\'s template and '
                                'parameters before running that window\'s preflight --activate.',
                                'The restored window counts its whole reservation again; reconcile the ledger months as usual.']}


def plan_of(config):
    plan = config.get('operating_plan', BOUNDED) if isinstance(config, dict) else BOUNDED
    require(isinstance(plan, str) and plan in PLANS, 'unknown operating plan')
    return plan


def reserve(config, ledger, now, activating=False):
    """Reserve (or reuse) the window; `activating` also records the activation request time."""
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
    management = {key: config.get(key, '') for key in ('provider_management_secret_arn', 'provider_management_url')}
    require(isinstance(management['provider_management_secret_arn'], str)
            and re.fullmatch('(?:' + MANAGEMENT_SECRET_RE + ')?', management['provider_management_secret_arn']) is not None,
            'provider_management_secret_arn must be empty or a complete same-account/region Secrets Manager ARN')
    require(isinstance(management['provider_management_url'], str)
            and re.fullmatch('(?:' + MANAGEMENT_URL_RE + ')?', management['provider_management_url']) is not None,
            'provider_management_url must be empty or an https origin')
    require((management['provider_management_secret_arn'] == '') == (management['provider_management_url'] == ''),
            'provider_management_secret_arn and provider_management_url are set together or left empty together')
    require(not management['provider_management_secret_arn'] or task_images['operator_image_digest'],
            'an attached management secret needs the operator task (operator_image_digest)')
    require(str(uuid.UUID(config['window_id'])) == config['window_id'], 'window_id must be a canonical UUID retained across retry')
    supersedes = config.get('supersedes_window_id', '')
    if supersedes != '':
        canonical_window(supersedes, 'supersedes_window_id')
        require(supersedes != config['window_id'], 'supersession cycle: a window cannot supersede itself')
    require(config['account'] == ACCOUNT and config['region'] == REGION, 'unexpected account or region')
    require(config['hostname'] == HOSTNAME, 'unexpected beta hostname')
    require(isinstance(config['operator'], str) and 0 < len(config['operator'].strip()) <= 100, 'named operator required')
    require(config['ingress_mode'] in ('operator', 'public'), 'invalid ingress mode')
    address = ipaddress.ip_network(config['operator_ipv4_cidr'], strict=True)
    require(address.version == 4 and address.prefixlen == 32 and address.network_address.is_global, 'operator address must be a real public IPv4 /32')
    expires = timestamp(config['deployment_expires_at_utc'])
    require(now + timedelta(minutes=10) <= expires <= now + rules['max_expiry'], 'expiry must be 10 minutes to %s in the future' % ('48 hours' if plan == BOUNDED else '32 days'))
    require(re.fullmatch(r'arn:aws:acm:' + _REGION_RE + ':' + _ACCOUNT_RE + r':certificate/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}', config['tls_certificate_arn']), 'certificate must belong to the selected account/region')
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
    immutable = {k: v for k, v in config.items() if k not in WITHIN_WINDOW
                 and not (k in OPTIONAL_INPUTS and v in ('', BOUNDED))}
    encoded = json.dumps(immutable, sort_keys=True, separators=(',', ':')).encode()
    binding = hashlib.sha256(encoded).hexdigest()
    validate_windows(ledger['windows'], plan)
    existing = ledger['windows'].get(config['window_id'])
    require(existing is None or existing['input_sha256'] == binding, 'window identity already has different deployment inputs; do not overwrite an unknown outcome')
    # A superseded window no longer reserves its future compute: it can never
    # be reused, retried or activated; its successor's input is the live one.
    require(existing is None or 'superseded_by_window_id' not in existing,
            'window was superseded by ' + str((existing or {}).get('superseded_by_window_id')) + '; use the superseding window input')
    require(existing is None or 'abandoned_at_utc' not in existing,
            'window was abandoned at ' + str((existing or {}).get('abandoned_at_utc')) + '; reserve a new window instead')
    identity = {'operating_plan': plan, 'account': config['account'], 'region': config['region'], 'hostname': config['hostname']}
    active = months_between(now, expires + timedelta(hours=1))
    # A migration/rollout can occur on either side of UTC month-end. Reserve
    # its complete allowance in every touched month rather than hiding it in
    # the first month's headroom.
    for month in active:
        active[month] += config['extra_task_hours']
    reserved = existing['reserved_compute_cents_by_month'] if existing else {month: cents(hours * TASK_HOURLY) for month, hours in active.items()}
    updated = copy.deepcopy(ledger)
    record = copy.deepcopy(existing) if existing else {
        'input_sha256': binding, 'created_at_utc': now.strftime('%Y-%m-%dT%H:%M:%SZ'),
        'expires_at_utc': config['deployment_expires_at_utc'], 'reserved_compute_cents_by_month': reserved,
        'service_identity': identity, ACTIVATION: '',
    }
    if activating and not record.get(ACTIVATION):
        # Recorded before any AWS call: from here on this window may have run at count one,
        # so it can no longer be abandoned (only fixed forward or superseded).
        record[ACTIVATION] = now.strftime('%Y-%m-%dT%H:%M:%SZ')
    if existing is None and supersedes:
        earlier = ledger['windows'].get(supersedes)
        require(earlier is not None, 'superseded window ' + supersedes + ' is not in the ledger')
        require('superseded_by_window_id' not in earlier, 'window ' + supersedes + ' was already superseded by ' + str(earlier.get('superseded_by_window_id')))
        require('abandoned_at_utc' not in earlier, 'window ' + supersedes + ' was abandoned; supersede the window that is live instead')
        require(identity_of(earlier, plan) == identity, 'superseded window is for another operating plan, service or account')
        require(timestamp(earlier['created_at_utc']) <= now, 'superseded window was reserved after this reservation time')
        record['supersedes_window_id'] = supersedes
        updated['windows'][supersedes] = dict(earlier, superseded_by_window_id=config['window_id'],
                                              superseded_at_utc=record['created_at_utc'], counted_compute_cents_by_month=supersede(earlier, now))
    updated['windows'][config['window_id']] = record
    validate_windows(updated['windows'], plan)
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
        windows = sum(counted_compute(w).get(month, 0) for w in updated['windows'].values())
        released = sum(w['reserved_compute_cents_by_month'].get(month, 0) - counted_compute(w).get(month, 0) for w in updated['windows'].values())
        genui = retained + windows + entry['prior_extra_spend_cents'] + entry['variable_reserve_cents']
        total = genui + entry['auth_spent_or_reserved_cents'] + entry['unallocated_cents']
        require(genui <= rules['genui_cap_cents'] and total <= rules['project_cap_cents'], 'budget exceeded for ' + month + '; reconcile prior reservations or change the reviewed operating plan')
        reports[month] = {'retained_core_full_month_cents': retained, 'all_window_compute_reservations_cents': windows,
                          'superseded_compute_released_cents': released,
                          'prior_extra_spend_cents': entry['prior_extra_spend_cents'], 'variable_reserve_cents': entry['variable_reserve_cents'],
                          'genui_total_cents': genui, 'auth_spent_or_reserved_cents': entry['auth_spent_or_reserved_cents'],
                          'unallocated_cents': entry['unallocated_cents'], 'project_total_cents': total}
    parameters = {'Operator': config['operator'], 'MonthlyBudgetUsd': rules['monthly_budget_usd'], 'IngressMode': config['ingress_mode'],
                  'OperatorIpv4Cidr': config['operator_ipv4_cidr'], 'DeploymentExpiresAtUtc': config['deployment_expires_at_utc'],
                  'TlsCertificateArn': config['tls_certificate_arn'], 'ImageDigest': config['image_digest'],
                  'ImageArchitecture': config['image_architecture'], 'ServiceDesiredCount': '0',
                  'QualificationImageDigest': task_images['qualification_image_digest'],
                  'OperatorImageDigest': task_images['operator_image_digest'],
                  'ProviderManagementSecretArn': management['provider_management_secret_arn'],
                  'ProviderManagementUrl': management['provider_management_url']}
    return updated, {'status': 'reserved_local_plan_only', 'window_id': config['window_id'], 'input_sha256': binding,
                     'observed_at_utc': now.strftime('%Y-%m-%dT%H:%M:%SZ'), 'reused_reservation': existing is not None,
                     'superseded_window_id': record.get('supersedes_window_id'),
                     'operating_plan': plan, 'decision': rules['decision'],
                     'monthly_budget_cents': rules['project_cap_cents'], 'genui_allocation_cents': rules['genui_cap_cents'], 'cost_by_month': reports,
                     'cloudformation_parameters': parameters,
                     'limits': ['No AWS resource, certificate, image, account state or billing feed was queried.',
                                'Scheduler requests ECS desiredCount zero; it is not a billing cap, teardown or a guarantee of timely delivery.'
                                + (' For the always-on beta it is a backstop that must be renewed, with a new reconciled reservation, before it fires.' if plan == ALWAYS_ON else ''),
                                'RDS, ALB, two ALB IPv4 addresses, storage and keys remain charged after ECS stops.',
                                'Retain this ledger across failed deployments, retries and renewed windows; never reset known spending to zero.',
                                'Renewals, standalone tasks, actual charges above reserves and every later month need reconciliation; model spending remains zero.']}


# Names that reach the service only through the stack's secrets: never as plain environment values
# (a plain value would be readable in describe-task-definition, CloudTrail and the console).
SECRET_ENV_PREFIXES = ('GENUI_AUTH_QUALIFICATION_', 'GENUI_PROVIDER_CAPABILIT', 'GENUI_AUTH_CONFIG', 'GENUI_IDENTITY_REGISTRY')
SECRET_ENV_NAMES = ('ANTHROPIC_API_KEY', 'SLACK_BOT_TOKEN', 'SLACK_SIGNING_SECRET')


def bypasses_secret_channel(name):
    return name.startswith(SECRET_ENV_PREFIXES) or name in SECRET_ENV_NAMES


def activate(config, report, call, clock):
    """The only count-one path; a past at() schedule must never start compute."""
    require(config['service_desired_count'] == 1, 'activation requires explicit desired count one')
    require(call('sts', 'get-caller-identity')['Account'] == config['account'], 'activation AWS account mismatch')
    stack = call('cloudformation', 'describe-stacks', '--stack-name', STACK)['Stacks'][0]
    require(stack['StackStatus'] in ('CREATE_COMPLETE', 'UPDATE_COMPLETE'), 'stack must be complete before activation')
    parameters = {v['ParameterKey']: v['ParameterValue'] for v in stack['Parameters']}
    expected = report['cloudformation_parameters']
    # An older deployed template may still carry the retired parameter: only empty.
    require(parameters.get('QualificationConfigSecretArn', '') == '', 'deployed qualification secret parameter must be empty or absent')
    require(all(parameters.get(k, '' if k in OPTIONAL_PARAMETERS else None) == str(v) for k, v in expected.items()), 'deployed parameters differ from the reserved plan')
    outputs = {v['OutputKey']: v['OutputValue'] for v in stack['Outputs']}
    arn = outputs['DeploymentExpiryScheduleArn']
    prefix = 'arn:aws:scheduler:%s:%s:schedule/' % (REGION, ACCOUNT)
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
    cluster_arn = 'arn:aws:ecs:%s:%s:cluster/%s' % (REGION, ACCOUNT, outputs['ClusterName'])
    service_arn = 'arn:aws:ecs:%s:%s:service/%s/%s' % (REGION, ACCOUNT, outputs['ClusterName'], outputs['ServiceName'])
    require(payload == {'Cluster': cluster_arn, 'Service': service_arn, 'DesiredCount': 0}, 'stop target must address this exact cluster/service with count zero')
    definition = call('ecs', 'describe-task-definition', '--task-definition', outputs['TaskDefinition'])['taskDefinition']
    require(definition['taskDefinitionArn'] == outputs['TaskDefinition'], 'unexpected live task definition')
    containers = definition['containerDefinitions']
    require(len(containers) == 1 and containers[0]['name'] == 'workspace', 'unexpected application container boundary')
    require(containers[0]['image'] == config['image_digest'] and definition['runtimePlatform']['cpuArchitecture'] == config['image_architecture'], 'live image or architecture differs from the reserved plan')
    require(containers[0].get('entryPoint', []) == [] and containers[0].get('command', []) == [], 'live service must use the reviewed image startup')
    require(containers[0].get('environmentFiles', []) == [] and not any(bypasses_secret_channel(item['name']) for item in containers[0].get('environment', [])),
            'live qualification, capability, sign-in configuration, model key or Slack credentials cannot bypass the reviewed secret channel')
    # ADR-003 amendment 2026-10-30 and APP-078 B1: the stack's provider capability secret
    # and its hosted sign-in configuration, under the names runtime.py serve() consumes;
    # ADR-011 pitch (APP-093, APP-100, APP-096): the composer's model key and the Slack
    # app's two fields, each from the stack's own secret. Exactly these five.
    expected_secrets = [{'name': 'GENUI_PROVIDER_CAPABILITIES_JSON', 'valueFrom': outputs['ProviderCapabilitySecretArn']},
                        {'name': 'GENUI_AUTH_CONFIGURATION_JSON', 'valueFrom': outputs['AuthConfigurationSecretArn']},
                        {'name': 'ANTHROPIC_API_KEY', 'valueFrom': outputs['ComposerModelSecretArn']},
                        {'name': 'SLACK_BOT_TOKEN', 'valueFrom': outputs['SlackAppSecretArn'] + ':bot_token::'},
                        {'name': 'SLACK_SIGNING_SECRET', 'valueFrom': outputs['SlackAppSecretArn'] + ':signing_secret::'}]
    by_name = lambda entries: sorted(entries, key=lambda entry: entry['name'])
    require(by_name(containers[0].get('secrets', [])) == by_name(expected_secrets), 'live secret injection differs from the reviewed stack')
    # APP-076 item 8 / gap G11: the deny-all rollback revision (`PREVIOUS_TD`) exists on the same
    # image with no secret injected and no sign-in configuration in its environment, so a rollback
    # redeploy answers the uniform 404 on every protected route while the schema stays migrated.
    require('RollbackTaskDefinition' in outputs, 'the stack has no deny-all rollback task definition (APP-076 G11)')
    rollback = call('ecs', 'describe-task-definition', '--task-definition', outputs['RollbackTaskDefinition'])['taskDefinition']
    require(rollback['taskDefinitionArn'] == outputs['RollbackTaskDefinition'] and rollback['taskDefinitionArn'] != outputs['TaskDefinition'],
            'unexpected rollback task definition')
    fallback = rollback['containerDefinitions']
    require(len(fallback) == 1 and fallback[0]['name'] == 'workspace' and fallback[0]['image'] == config['image_digest']
            and rollback['runtimePlatform']['cpuArchitecture'] == config['image_architecture'],
            'rollback task definition must run the reserved image on one workspace container')
    require(fallback[0].get('secrets', []) == [] and fallback[0].get('environmentFiles', []) == []
            and fallback[0].get('entryPoint', []) == [] and fallback[0].get('command', []) == []
            and not any(bypasses_secret_channel(item['name']) for item in fallback[0].get('environment', [])),
            'rollback task definition must inject no secret and name no sign-in configuration: it starts deny-all')
    # A12: an attached management secret must be the named one, live, and under the AWS-managed
    # `aws/secretsmanager` key: the stack grants the operator task's execution role GetSecretValue
    # only, so a customer-managed key would fail the task start (it would need a key-policy grant).
    attached = parameters.get('ProviderManagementSecretArn', '')
    if attached:
        described = call('secretsmanager', 'describe-secret', '--secret-id', attached)
        require(described.get('ARN') == attached and 'DeletedDate' not in described,
                'the attached management secret is not the named live secret')
        require(described.get('KmsKeyId') in (None, 'alias/aws/secretsmanager'),
                'the attached management secret uses a customer-managed key; the operator task cannot decrypt it without a key-policy grant')
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
            'task_definition': outputs['TaskDefinition'], 'rollback_task_definition': outputs['RollbackTaskDefinition'],
            'expires_at_utc': config['deployment_expires_at_utc'], 'desired_count': 1,
            'limit': 'Retain the reservation on failure/unknown outcome. Verify desiredCount zero and stopped tasks at expiry; this API response is not readiness or a billing cutoff.'}


def aws_call(*args):
    environment = dict(os.environ, AWS_MAX_ATTEMPTS='1', AWS_PAGER='')
    result = subprocess.run(['aws', *args, '--region', REGION, '--output', 'json', '--cli-connect-timeout', '5', '--cli-read-timeout', '10'],
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
    parser.add_argument('--input', type=Path)
    parser.add_argument('--ledger', type=Path, required=True, help='Existing private operator ledger; never initialized or zeroed automatically')
    parser.add_argument('--activate', action='store_true', help='Explicit AWS mutation: verify completed stack/current future stop, then request desiredCount one')
    parser.add_argument('--abandon', metavar='WINDOW_ID', help='Local only: undo the supersession of a successor window that never requested activation')
    args = parser.parse_args()
    if (args.abandon is None) == (args.input is None) or (args.abandon is not None and args.activate):
        parser.error('give --input (optionally with --activate), or --abandon WINDOW_ID alone')
    # A separate persistent lock survives the atomic replacement of the ledger.
    lock = os.open(str(args.ledger) + '.lock', os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    with os.fdopen(lock, 'r+') as locked:
        fcntl.flock(locked, fcntl.LOCK_EX)
        fd = os.open(args.ledger, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(fd, 'r') as stream:
            info = os.fstat(stream.fileno())
            require(stat.S_ISREG(info.st_mode) and info.st_uid == os.geteuid() and info.st_mode & 0o077 == 0, 'ledger must be an owned private regular file')
            ledger = json.load(stream, object_pairs_hook=no_duplicates)
        if args.abandon is not None:
            updated, report = abandon(ledger, args.abandon, datetime.now(UTC))
        else:
            config = read_json(args.input)
            require(args.activate or config.get('service_desired_count') == 0, 'desired count one requires explicit --activate after deployed schedule verification')
            updated, report = reserve(config, ledger, datetime.now(UTC), activating=args.activate)
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
