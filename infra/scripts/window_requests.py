#!/usr/bin/env python3
"""APP-078 window request composer. Local only: it calls no AWS API.

Each subcommand writes one reviewed request file that the operator then hands,
unchanged, to the AWS CLI during the approved window (``--cli-input-json
file://...``), or checks a response file the CLI returned:

  auth-configuration  compose the AuthConfiguration secret value (APP-078 B1) from
                   FormaStack's private identifiers file: the auth origin, token
                   issuer and audience, each tenant's slug, public client id and
                   `ten_` id bound to its organization, validated by the supervisor's
                   own reader (runtime.auth_configuration); the document goes only to
                   a 0600 file in an owned 0700 private directory for PutSecretValue,
                   and only organization ids, slugs and its digest are printed
  mint-code        draw an access code locally (32 CSPRNG bytes, admission
                   operator encoding); the raw code goes only to a 0600 file in
                   an owned 0700 private directory, and only its digest is printed
                   for ``admission issue --code-digest`` (the task, its override,
                   CloudTrail and the log never see the secret)
  operator-task    an ECS RunTask request for the OperatorTask from one documented
                   override of app/operator/overrides.json, with only that
                   override's flag values replaced; the result must parse, pass
                   the admission validator and be admitted in a managed task
  qualification-task  an ECS RunTask request for the QualificationTask with the
                   arming value and a run manifest issued now (600 s span); run it
                   immediately, since the supervisor refuses a manifest with less
                   than 60 s left
  change-set       a CloudFormation CreateChangeSet request (UPDATE, TemplateURL
                   under the CDK assets bucket at the template's SHA-256 key) from
                   a preflight receipt, plus the put-object parameters
  review-change-set  refuse a described change set that replaces or removes the
                   database, load balancer, keys, bucket, secrets or log group,
                   or that is not CREATE_COMPLETE/AVAILABLE

Usage: python3 infra/scripts/window_requests.py <subcommand> --help
"""
import argparse
import base64
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import secrets
import stat
import sys
import time
import uuid

REPOSITORY = Path(__file__).resolve().parents[2]
STACK = 'GenuiSyntheticFoundation'
ACCOUNT, REGION = '877573132739', 'us-east-1'
ASSETS_BUCKET = 'cdk-hnb659fds-assets-%s-%s' % (ACCOUNT, REGION)
MAX_OVERRIDE_BYTES = 8192
ORGANIZATIONS = ('00000000-0000-4000-8000-000000000001', '00000000-0000-4000-8000-000000000002')
# Resources whose replacement or removal the window never accepts: the retained
# data plane, its keys and secrets, and the load balancer whose DNS name the
# existing beta.getforma.dev record targets.
PROTECTED_TYPES = ('AWS::RDS::DBInstance', 'AWS::ElasticLoadBalancingV2::LoadBalancer', 'AWS::KMS::Key', 'AWS::S3::Bucket',
                   'AWS::SecretsManager::Secret', 'AWS::Logs::LogGroup', 'AWS::EC2::VPC', 'AWS::RDS::DBSubnetGroup',
                   'AWS::ECS::Cluster', 'AWS::ECS::Service', 'AWS::ElasticLoadBalancingV2::Listener')


class Refused(Exception):
    pass


def require(condition, message):
    if not condition:
        raise Refused(message)


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def admission_operator():
    runtime = str(REPOSITORY / 'app' / 'runtime')
    if runtime not in sys.path:
        sys.path.insert(0, runtime)
    import admission_operator as module
    return module


def operator_module():
    admission_operator()
    return load(REPOSITORY / 'app' / 'operator' / 'operator.py', 'genui_operator_package')


def runtime_module():
    admission_operator()  # puts app/runtime on the path
    import runtime
    return runtime


def no_duplicates(pairs):
    keys = [k for k, _ in pairs]
    require(len(keys) == len(set(keys)), 'duplicate JSON field')
    return dict(pairs)


def read_json(path):
    with open(path, 'r', encoding='utf-8') as stream:
        return json.load(stream, object_pairs_hook=no_duplicates)


def write_json(path, payload, private=False):
    data = (json.dumps(payload, indent=2, sort_keys=True) + '\n').encode()
    if private:
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, 'wb') as stream:
            stream.write(data)
    else:
        Path(path).write_bytes(data)
    return hashlib.sha256(data).hexdigest()


def private_directory(path):
    info = os.lstat(path)
    require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.geteuid() and info.st_mode & 0o077 == 0,
            'private directory must be an owned 0700 directory')


def stack_outputs(path):
    """`aws cloudformation describe-stacks` output (or its Outputs list) for the exact stack."""
    document = read_json(path)
    if 'Stacks' in document:
        require(len(document['Stacks']) == 1 and document['Stacks'][0]['StackName'] == STACK, 'unexpected stack')
        document = document['Stacks'][0]
    outputs = {o['OutputKey']: o['OutputValue'] for o in document['Outputs']}
    return outputs


def network(outputs):
    subnets = outputs['PublicTaskSubnets'].split(',')
    require(len(subnets) == 2 and all(re.fullmatch(r'subnet-[0-9a-f]{8,17}', s) for s in subnets), 'unexpected task subnets')
    require(re.fullmatch(r'sg-[0-9a-f]{8,17}', outputs['TaskSecurityGroup']) is not None, 'unexpected task security group')
    return {'awsvpcConfiguration': {'subnets': subnets, 'securityGroups': [outputs['TaskSecurityGroup']], 'assignPublicIp': 'ENABLED'}}


def client_token(window_id, step):
    try:
        canonical = str(uuid.UUID(window_id))
    except (ValueError, TypeError, AttributeError):
        canonical = None
    require(canonical == window_id, 'window id must be a canonical UUID')
    require(re.fullmatch(r'[a-z0-9-]{1,40}', step) is not None, 'step must be a short lowercase name')
    # RunTask clientToken: at most 64 characters, stable across a retried call.
    return hashlib.sha256((window_id + ':' + step).encode()).hexdigest()[:64]


def run_task_request(outputs, definition_key, overrides, window_id, step):
    require(definition_key in outputs, definition_key + ' output missing: the stack update with that task image has not completed')
    arn = outputs[definition_key]
    require(re.fullmatch(r'arn:aws:ecs:%s:%s:task-definition/[A-Za-z0-9_-]+:[0-9]+' % (REGION, ACCOUNT), arn) is not None, 'unexpected task definition')
    size = len(json.dumps(overrides, separators=(',', ':')).encode())
    require(size <= MAX_OVERRIDE_BYTES, 'overrides exceed 8,192 bytes')
    return {'cluster': outputs['ClusterName'], 'taskDefinition': arn, 'count': 1, 'launchType': 'FARGATE', 'platformVersion': '1.4.0',
            'networkConfiguration': network(outputs), 'overrides': overrides, 'startedBy': 'app078-' + step,
            'clientToken': client_token(window_id, step), 'enableExecuteCommand': False, 'propagateTags': 'TASK_DEFINITION'}


def mint_code(args):
    module = admission_operator()
    require(args.organization in ORGANIZATIONS, 'organization outside the two fixture organizations')
    require(re.fullmatch(r'[a-z0-9][a-z0-9-]{0,39}', args.label) is not None, 'label must be a short lowercase name')
    private_directory(args.out_dir)
    raw = secrets.token_bytes(module.DEFAULTS['code_secret_bytes'])
    code, digest = module.encode_code(raw), module.code_digest(raw)
    require(module.code_digest(module.normalize_code(code)) == digest, 'code encoding does not round-trip')
    path = Path(args.out_dir) / ('access-code-%s.json' % args.label)
    write_json(path, {'organization_id': args.organization, 'label': args.label, 'code': code, 'code_digest': digest,
                      'minted_at_utc': datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),
                      'handling': 'Deliver privately once; never paste into a task override, log, ticket or evidence.'}, private=True)
    return {'label': args.label, 'organization_id': args.organization, 'code_digest': digest, 'private_file': str(path)}


# APP-078 B1: the fields of FormaStack's private identifiers file that the auth
# configuration needs; everything else in that file is never read into the output.
APP_ORIGIN = 'https://beta.getforma.dev'
IDENTIFIER_FIELDS = ('issuer_origin', 'jwt_iss', 'jwt_aud', 'access_jwks', 'callback', 'tenants')
TENANT_FIELDS = ('slug', 'tenant_id', 'client_id', 'genui_org_uuid')


def auth_configuration(args):
    """The AuthConfiguration secret value, composed from the private identifiers file and checked
    with the supervisor's reader before it is written (0600, in an owned 0700 directory)."""
    rt = runtime_module()
    private_directory(args.output.parent)
    identifiers = read_json(args.identifiers)
    require(isinstance(identifiers, dict) and all(k in identifiers for k in IDENTIFIER_FIELDS), 'identifiers file lacks a required field')
    origin = identifiers['issuer_origin']
    # The service derives the JWKS URL and the callback itself (auth/mod.rs): the
    # identifiers must agree with what it will compute, or sign-in cannot work.
    require(identifiers['access_jwks'] == str(origin) + '/auth/jwks', 'access_jwks is not <issuer_origin>/auth/jwks, which the service derives')
    require(identifiers['callback'] == args.app_origin + '/auth/callback', 'the registered callback is not <app origin>/auth/callback')
    tenants = identifiers['tenants']
    require(isinstance(tenants, list) and tenants, 'identifiers file names no tenant')
    organizations, bindings = [], []
    for tenant in tenants:
        require(isinstance(tenant, dict) and all(k in tenant for k in TENANT_FIELDS), 'tenant entry lacks a required field')
        require(tenant['genui_org_uuid'] in ORGANIZATIONS, 'tenant names an organization outside the two fixture organizations')
        organizations.append({'organization_id': tenant['genui_org_uuid'], 'tenant_slug': tenant['slug'], 'client_id': tenant['client_id']})
        bindings.append({'issuer': identifiers['jwt_iss'], 'provider_tenant_id': tenant['tenant_id'], 'organization_id': tenant['genui_org_uuid']})
    document = {'format': rt.AUTH_CONFIGURATION_FORMAT,
                'auth_config': {'schema_version': 1, 'auth_origin': origin, 'app_origin': args.app_origin, 'issuer': identifiers['jwt_iss'],
                                'audience': identifiers['jwt_aud'], 'max_access_lifetime_seconds': args.max_access_lifetime_seconds,
                                'organizations': organizations},
                'identity_registry': {'schema_version': 1, 'registry_version': str(uuid.uuid4()), 'bindings': bindings}}
    text = json.dumps(document, indent=2, sort_keys=True) + '\n'
    # The supervisor's own validation (serve() refuses anything else at task start).
    require(rt.auth_configuration(text) is not None, 'the composed document is not one serve() accepts')
    sha = write_json(args.output, document, private=True)
    return {'private_file': str(args.output), 'sha256': sha, 'bytes': len(text.encode()),
            'organizations': [o['organization_id'] for o in organizations], 'tenant_slugs': [o['tenant_slug'] for o in organizations],
            'registry_version': document['identity_registry']['registry_version'],
            'handling': 'PutSecretValue --secret-string file://<private_file> only; never a task override, log, ticket, chat or evidence.'}


def compose_operator_command(document, name, sets, adds):
    entries = {item['name']: item for item in document['overrides']}
    require(name in entries, 'unknown documented override')
    container = entries[name]['overrides']['containerOverrides'][0]
    command = list(container['command'])
    for flag, value in sets:
        require(flag.startswith('--') and flag in command, 'flag %s is not in the documented override' % flag)
        index = command.index(flag)
        require(index + 1 < len(command), 'flag has no value')
        command[index + 1] = value
    for flag, value in adds:
        require(flag.startswith('--') and flag in command, 'flag %s is not in the documented override' % flag)
        command += [flag, value]
    return {'containerOverrides': [{'name': document['container'], 'command': command}]}


def pairs(values):
    result = []
    for value in values or []:
        flag, sep, rest = value.partition('=')
        require(sep == '=' and flag.startswith('--'), 'use --set=--flag=value')
        result.append((flag, rest))
    return result


def operator_task(args):
    op = operator_module()
    admission = admission_operator()
    document = read_json(REPOSITORY / 'app' / 'operator' / 'overrides.json')
    overrides = compose_operator_command(document, args.override, pairs(args.set), pairs(args.add))
    command = overrides['containerOverrides'][0]['command']
    if args.record is not None:
        # register-source-revision's inline form: the record upload-sources printed.
        require(args.override == 'admission-register-source-revision-inline', '--record applies only to the inline registration')
        record = read_json(args.record)
        command[command.index('--record-json') + 1] = json.dumps(record, sort_keys=True, separators=(',', ':'))
    parsed = op.parse(command)
    if parsed.command == 'admission':
        checked = admission.validate(admission.parser().parse_args(parsed.rest))
        require(not op.managed_form_refused(checked), 'this form is refused in a managed task')
    request = run_task_request(stack_outputs(args.outputs), 'OperatorTaskDefinition', overrides, args.window_id, args.step)
    sha = write_json(args.output, request)
    return {'request': str(args.output), 'sha256': sha, 'command': command, 'override_bytes': len(json.dumps(overrides, separators=(',', ':')).encode())}


def qualification_manifest(run_id, now, resource_id, lifetime=600):
    harness = load(REPOSITORY / 'app' / 'qualification' / 'local_harness.py', 'genui_local_harness')
    admission_operator()  # puts app/runtime (the supervisor's `runtime`) on the path
    supervisor = load(REPOSITORY / 'app' / 'runtime' / 'qualification_supervisor.py', 'genui_qualification_supervisor')
    require(resource_id == harness.RESOURCE_ID, 'live RDS resource id differs from the pinned qualification target; stop')
    require(harness.HOST == supervisor.HOST and harness.ORGANIZATIONS == list(supervisor.ORGANIZATIONS), 'harness and supervisor pins disagree')
    registry = harness.registry_bytes(run_id)
    require(registry == supervisor.registry_bytes(run_id), 'registry bytes disagree with the supervisor')
    ca = hashlib.sha256((REPOSITORY / 'app' / 'certs' / 'rds-global-bundle.pem').read_bytes()).hexdigest()
    require(ca == supervisor.REVIEWED_CA_SHA256, 'repository RDS CA differs from the reviewed pin')
    manifest = {'schema_version': 1, 'scope': harness.SCOPE, 'armed': True, 'run_id': run_id, 'issued_at': now,
                'expires_at': now + lifetime, 'database_host': harness.HOST, 'database_port': 5432, 'database_name': 'genui',
                'declared_rds_resource_id': resource_id, 'organizations': list(harness.ORGANIZATIONS),
                'registry_sha256': hashlib.sha256(registry).hexdigest(), 'ca_sha256': ca, 'pins': harness.expected_pins()}
    return json.dumps(manifest, separators=(',', ':'))


def qualification_task(args):
    now = int(time.time()) if args.now is None else args.now
    run_id = str(uuid.uuid4())
    manifest = qualification_manifest(run_id, now, args.rds_resource_id)
    overrides = {'containerOverrides': [{'name': 'qualification', 'environment': [
        {'name': 'GENUI_APP008_MANAGED_QUALIFICATION', 'value': 'APP-008'},
        {'name': 'GENUI_APP008_MANAGED_MANIFEST_JSON', 'value': manifest}]}]}
    request = run_task_request(stack_outputs(args.outputs), 'QualificationTaskDefinition', overrides, args.window_id, args.step)
    sha = write_json(args.output, request)
    return {'request': str(args.output), 'sha256': sha, 'run_id': run_id, 'issued_at': now, 'expires_at': now + 600,
            'manifest_sha256': hashlib.sha256(manifest.encode()).hexdigest(),
            'override_bytes': len(json.dumps(overrides, separators=(',', ':')).encode()),
            'deadline': 'Run the request now: the supervisor refuses a manifest with under 60 s left; Fargate start and image pull consume the 600 s span.'}


def change_set(args):
    receipt = read_json(args.receipt)
    require(receipt.get('status') == 'reserved_local_plan_only' and receipt.get('window_id') == args.window_id, 'receipt is not this window\'s preflight reservation')
    parameters = receipt['cloudformation_parameters']
    template = Path(args.template).read_bytes()
    document = json.loads(template)
    declared = set(document['Parameters'])
    require(set(parameters) | {'BootstrapVersion'} == declared, 'preflight parameters do not match the template parameters')
    digest = hashlib.sha256(template).hexdigest()
    key = digest + '.json'
    request = {'StackName': STACK, 'ChangeSetName': 'genui-app078-' + args.window_id, 'ChangeSetType': 'UPDATE',
               'TemplateURL': 'https://%s.s3.%s.amazonaws.com/%s' % (ASSETS_BUCKET, REGION, key),
               'Parameters': sorted([{'ParameterKey': k, 'ParameterValue': str(v)} for k, v in parameters.items()], key=lambda p: p['ParameterKey'])
               + [{'ParameterKey': 'BootstrapVersion', 'UsePreviousValue': True}],
               'Capabilities': ['CAPABILITY_IAM'], 'ClientToken': client_token(args.window_id, 'change-set'),
               'Description': 'APP-078 window ' + args.window_id, 'IncludeNestedStacks': False}
    sha = write_json(args.output, request)
    upload = {'bucket': ASSETS_BUCKET, 'key': key, 'template_sha256_hex': digest,
              'checksum_sha256_base64': base64.b64encode(bytes.fromhex(digest)).decode(), 'bytes': len(template)}
    return {'request': str(args.output), 'sha256': sha, 'upload': upload}


def review_change_set(args):
    described = read_json(args.describe)
    require(described.get('StackName') == STACK, 'unexpected stack')
    require(described.get('Status') == 'CREATE_COMPLETE' and described.get('ExecutionStatus') == 'AVAILABLE', 'change set is not complete and available')
    findings, summary = [], []
    for change in described.get('Changes', []):
        resource = change.get('ResourceChange', {})
        action, kind, logical = resource.get('Action'), resource.get('ResourceType'), resource.get('LogicalResourceId')
        replacement = resource.get('Replacement')
        summary.append({'logical_id': logical, 'type': kind, 'action': action, 'replacement': replacement})
        if kind in PROTECTED_TYPES and (action == 'Remove' or replacement in ('True', 'Conditional')):
            findings.append('%s %s %s (replacement %s) is not allowed in this window' % (action, kind, logical, replacement))
    expected_parameters = None if args.receipt is None else read_json(args.receipt)['cloudformation_parameters']
    if expected_parameters is not None:
        actual = {p['ParameterKey']: p.get('ParameterValue') for p in described.get('Parameters', []) if p['ParameterKey'] != 'BootstrapVersion'}
        if actual != {k: str(v) for k, v in expected_parameters.items()}:
            findings.append('change-set parameters differ from the preflight receipt')
    verdict = {'acceptable': not findings, 'findings': findings, 'changes': summary}
    if args.output:
        write_json(args.output, verdict)
    require(not findings, '; '.join(findings))
    return verdict


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest='command', required=True)
    p = sub.add_parser('mint-code')
    p.add_argument('--organization', required=True); p.add_argument('--label', required=True); p.add_argument('--out-dir', type=Path, required=True)
    p = sub.add_parser('auth-configuration')
    p.add_argument('--identifiers', type=Path, required=True, help="FormaStack's private identifiers JSON (read for its routing fields only)")
    p.add_argument('--output', type=Path, required=True, help='0600 file in an owned 0700 private directory')
    p.add_argument('--app-origin', default=APP_ORIGIN)
    p.add_argument('--max-access-lifetime-seconds', type=int, default=900, help='the verifier cap; the planned fAuth access lifetime is 900 s (APP-006)')
    for name in ('operator-task', 'qualification-task'):
        p = sub.add_parser(name)
        p.add_argument('--outputs', type=Path, required=True, help='describe-stacks output after the stack update')
        p.add_argument('--window-id', required=True); p.add_argument('--step', required=True); p.add_argument('--output', type=Path, required=True)
        if name == 'operator-task':
            p.add_argument('--override', required=True)
            p.add_argument('--set', action='append', help='--set=--flag=value replaces a documented flag value')
            p.add_argument('--add', action='append', help='--add=--flag=value repeats a documented flag (e.g. --grant)')
            p.add_argument('--record', type=Path, help='upload-sources record for the inline registration')
        else:
            p.add_argument('--rds-resource-id', required=True); p.add_argument('--now', type=int)
    p = sub.add_parser('change-set')
    p.add_argument('--receipt', type=Path, required=True); p.add_argument('--template', type=Path, required=True)
    p.add_argument('--window-id', required=True); p.add_argument('--output', type=Path, required=True)
    p = sub.add_parser('review-change-set')
    p.add_argument('--describe', type=Path, required=True); p.add_argument('--receipt', type=Path); p.add_argument('--output', type=Path)
    args = parser.parse_args(argv)
    handlers = {'mint-code': mint_code, 'auth-configuration': auth_configuration, 'operator-task': operator_task, 'qualification-task': qualification_task,
                'change-set': change_set, 'review-change-set': review_change_set}
    try:
        result = handlers[args.command](args)
    except (Refused, KeyError, ValueError, OSError) as error:
        print(json.dumps({'refused': str(error)}), file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == '__main__':
    sys.exit(main())
