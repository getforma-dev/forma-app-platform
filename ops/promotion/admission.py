"""Operator back office for the real-user trial organization (the 2026-11-14 decisions, W3).

One organization, Victor's, on the beta. Every command here runs with an authorized
non-workflow operator identity against the reviewed canonical state, proves the stack
carries the operator task and the provider-management binding, and then either runs one
documented operator-task override (app/operator/overrides.json) through ECS RunTask or
acts with the operator's own credentials where no task can (S3 object versions, the
service restart). Every AWS mutation leaves an immutable receipt in the state bucket;
nothing is retried, replayed or forced.

issue           mint N codes locally (the task never sees a secret), write the one-shot
                OperatorInvitationInput secret, run one `admission issue --code-digest
                --provider-input-from-secret --note` task per code, wait for each, and
                ALWAYS empty the secret afterwards, even on failure. The codes go only
                to the new private 0600 --output file. One restart (`restart`) then
                hands every capability issued so far to the service, up to 128.
list            codes, status, notes and redemptions of the organization (identifiers only).
revoke          revoke one code by id; the task drops its capability and provider invitation.
grant-admin     the organization's admin_policy row for one member (the Admin page).
delete-uploads  list one member's uploaded box files through the task, then delete every
                S3 version and delete marker of those keys with the operator's identity.
restart         force a new deployment of the service, behind --announced: the service
                reloads the capability secret at task start, and sessions sign out.

Not here: changing the organization's display name (the operator has only `provision
--display-name`, which creates), membership removal (the in-product Admin page), and
anything the workflow roles do. The expected state hash binds the command to the state
the operator reviewed; a deployment holding the lock refuses.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import secrets
import sys
import uuid

from .aws import AWS
from .expiry_policy import operator_identity, reviewed_state
from .guards import ROLE_KEYS
from . import guards
from .runner import ROOT, Runner
from .state import Store, digest, encoded, require

OVERRIDES = ROOT / 'app/operator/overrides.json'
WINDOW_REQUESTS = ROOT / 'infra/scripts/window_requests.py'
EMPTY_INPUT = '{}'
PROVIDER_ROLE = 'user'
TASK_WAIT_SECONDS = 900
RESTART_WAIT_SECONDS = 900
LOG_EVENT_BYTES_MAX = 1024 * 1024
# The stack must carry these for any operator task (infra/src/stack.ts: the operator task
# definition exists only with OperatorImageDigest; the managed invitation path needs the
# attached management secret and its URL; the clean trial names its organization).
REQUIRED_PARAMETERS = ('OperatorImageDigest', 'ProviderManagementSecretArn', 'ProviderManagementUrl', 'CleanTrialOrganizationId')
REQUIRED_OUTPUTS = ('OperatorTaskDefinition', 'ClusterName', 'ServiceName', 'TaskDefinition', 'PublicTaskSubnets', 'TaskSecurityGroup',
                    'OperatorInvitationInputSecretArn', 'ProviderCapabilitySecretArn', 'EvidenceBucket')
UUID_RE = re.compile(r'[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}')
EMAIL_RE = re.compile(r'[^@\s]{1,64}@[^@\s]{1,255}')
ISSUE_OVERRIDES = {True: 'admission-issue-trial-invitation', False: 'admission-issue-trial-invitation-unnoted'}


def modules():
    """The request composer and the operator's own validators, loaded by path (never installed)."""
    spec = importlib.util.spec_from_file_location('genui_window_requests', WINDOW_REQUESTS)
    window_requests = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(window_requests)
    admission = window_requests.admission_operator()
    runtime = window_requests.runtime_module()
    operator = window_requests.operator_module()
    return window_requests, admission, runtime, operator


def stack_ready(stack, organization, state):
    """Read-only proof, before any RunTask, that the operator channel exists for this organization."""
    outputs = guards.outputs(stack)
    parameters = guards.parameters(stack)
    for key in REQUIRED_PARAMETERS:
        require(parameters.get(key), 'stack parameter missing or empty: ' + key)
    for key in REQUIRED_OUTPUTS:
        require(isinstance(outputs.get(key), str) and outputs[key], 'stack output missing: ' + key)
    require(parameters['CleanTrialOrganizationId'] == organization, 'organization differs from the stack\'s clean-trial organization')
    require(state['live']['config'].get('clean_trial_organization_id') == organization, 'organization differs from the canonical clean-trial organization')
    require(parameters == state['live']['parameters'], 'live stack parameters drifted')
    return outputs


class Trial:
    """One reviewed operator session: identity, idle canonical state, stack outputs and receipts."""

    def __init__(self, aws, store, expected_state_sha256, organization, run_id=None):
        require(isinstance(organization, str) and UUID_RE.fullmatch(organization), 'organization must be a canonical UUID')
        self.aws, self.store, self.organization = aws, store, organization
        self.identity = aws.identity()
        self.state = reviewed_state(store, expected_state_sha256)
        operator_identity(self.identity, self.state)
        self.stack = aws.call('cloudformation', 'describe_stacks', StackName=self.state['authority']['stack_id'])['Stacks'][0]
        self.outputs = stack_ready(self.stack, organization, self.state)
        self.run_id = run_id or str(uuid.uuid4())
        self.window_requests, self.admission, self.runtime, self.operator = modules()
        self.document = json.loads(OVERRIDES.read_text(), object_pairs_hook=self.window_requests.no_duplicates)
        self.receipts = []
        self.task_runs = 0

    def receipt(self, name, value):
        key = 'promotions/admission-' + self.run_id + '/' + name + '.json'
        sha = self.store.immutable(key, dict(value, observed_at_utc=_now(), identity_arn=self.identity['Arn'],
                                             reviewed_state_sha256=digest(self.state), organization_id=self.organization))
        self.receipts.append({'key': key, 'sha256': sha})
        return sha

    # --- operator tasks -----------------------------------------------------------------

    def compose(self, override, sets):
        overrides = self.window_requests.compose_operator_command(self.document, override, sets, [])
        command = overrides['containerOverrides'][0]['command']
        parsed = self.operator.parse(command)
        require(parsed.command == 'admission', 'only admission overrides run here')
        checked = self.admission.validate(self.admission.parser().parse_args(parsed.rest))
        require(not self.operator.managed_form_refused(checked), 'this form is refused in a managed task')
        require(checked.organization == self.organization, 'override names another organization')
        return overrides, command

    def operator_task(self, override, sets, step):
        """RunTask one documented override, wait for it to stop, and read its one JSON result line."""
        overrides, command = self.compose(override, sets)
        # One logical run, one client token and one pair of receipts: never reused within a session.
        self.task_runs += 1
        step = '%s-%d' % (step, self.task_runs)
        request = self.window_requests.run_task_request(self.outputs, 'OperatorTaskDefinition', overrides, self.run_id, step)
        response = self.aws.call('ecs', 'run_task', **request)
        tasks = response.get('tasks') or []
        self.receipt(step + '-run', {'request': request, 'response': response})
        require(not response.get('failures') and len(tasks) == 1 and isinstance(tasks[0].get('taskArn'), str), 'RunTask did not start exactly one task; reconcile before another run')
        arn = tasks[0]['taskArn']
        stopped = self.aws.wait('ecs', 'describe_tasks', lambda r: (r.get('tasks') or [{}])[0].get('lastStatus') == 'STOPPED',
                                timeout=TASK_WAIT_SECONDS, cluster=self.outputs['ClusterName'], tasks=[arn])
        task = stopped['tasks'][0]
        containers = task.get('containers') or []
        exit_code = containers[0].get('exitCode') if len(containers) == 1 else None
        result = self.task_result(arn)
        self.receipt(step + '-stopped', {'task_arn': arn, 'command': command, 'stop_code': task.get('stopCode'), 'stopped_reason': task.get('stoppedReason'),
                                         'exit_code': exit_code, 'result': result})
        return {'task_arn': arn, 'exit_code': exit_code, 'result': result, 'command': command}

    def task_result(self, arn):
        """The last JSON object the task printed (its result or its fixed refusal), from its log stream."""
        definition = self.aws.call('ecs', 'describe_task_definition', taskDefinition=self.outputs['OperatorTaskDefinition'])['taskDefinition']
        container = definition['containerDefinitions'][0]
        options = container.get('logConfiguration', {}).get('options', {})
        group, prefix = options.get('awslogs-group'), options.get('awslogs-stream-prefix')
        require(group and prefix, 'operator task definition has no awslogs configuration')
        stream = prefix + '/' + container['name'] + '/' + arn.rsplit('/', 1)[-1]
        events = self.aws.call('logs', 'get_log_events', logGroupName=group, logStreamName=stream, startFromHead=True)
        result = None
        for event in events.get('events') or []:
            message = event.get('message')
            if not isinstance(message, str) or len(message.encode()) > LOG_EVENT_BYTES_MAX:
                continue
            try:
                value = json.loads(message)
            except ValueError:
                continue
            if isinstance(value, dict):
                result = value
        return result

    # --- commands -----------------------------------------------------------------------

    def secret(self, arn):
        value = self.aws.call('secretsmanager', 'get_secret_value', SecretId=arn)
        text = value.get('SecretString')
        require(isinstance(text, str), 'secret has no string value')
        return text

    def put_secret(self, arn, text, name):
        response = self.aws.call('secretsmanager', 'put_secret_value', SecretId=arn, SecretString=text, ClientRequestToken=str(uuid.uuid4()))
        self.receipt(name, {'secret_arn': arn, 'bytes': len(text.encode()), 'sha256': hashlib.sha256(text.encode()).hexdigest(),
                            'version_id': response.get('VersionId')})

    def issue(self, recipients, identifiers, count, with_notes, expires_days, output):
        admission = self.admission
        require(type(count) is int and count == len(recipients), '--count must equal the number of recipients')
        require(1 <= count <= admission.INVITATION_INPUT_ENTRIES_MAX, 'a wave is 1 to %d codes' % admission.INVITATION_INPUT_ENTRIES_MAX)
        require(type(expires_days) is int and 1 <= expires_days <= admission.DEFAULTS['code_expiry_days_max'], 'expiry days out of range')
        emails = []
        for entry in recipients:
            require(isinstance(entry, dict) and set(entry) <= {'email', 'note'} and 'email' in entry, 'recipient fields are email and note')
            email = entry['email']
            require(isinstance(email, str) and email.isascii() and email.isprintable() and EMAIL_RE.fullmatch(email) and email == email.strip(),
                    'recipient address malformed')
            require(email.lower() not in emails, 'recipient address repeated')
            emails.append(email.lower())
            if with_notes:
                note = entry.get('note')
                require(isinstance(note, str) and 1 <= len(note.encode()) <= admission.NOTE_BYTES_MAX and note.isprintable() and note == note.strip(),
                        'every recipient needs a note (1 to %d printable bytes); use --without-notes before migration 0023' % admission.NOTE_BYTES_MAX)
        issuer, tenant = self.binding(identifiers)
        # Bounds before any write: the service loads at most 128 capabilities per task start,
        # and the one-shot input secret must be the empty document the last run left.
        capability_arn, input_arn = self.outputs['ProviderCapabilitySecretArn'], self.outputs['OperatorInvitationInputSecretArn']
        current = self.runtime.capability_map(self.secret(capability_arn))
        require(len(current) + count <= self.runtime.CAPABILITIES_MAX,
                'capability secret would exceed %d entries; revoke or reconcile first' % self.runtime.CAPABILITIES_MAX)
        require(json.loads(self.secret(input_arn)) == {}, 'invitation input secret is not empty; a previous run did not clear it: inspect and clear it first')
        minted = []
        for entry in recipients:
            raw = secrets.token_bytes(admission.DEFAULTS['code_secret_bytes'])
            minted.append({'email': entry['email'], 'note': entry.get('note') if with_notes else None,
                           'code': admission.encode_code(raw), 'code_digest': admission.code_digest(raw)})
        document = {'format': admission.INVITATION_INPUT_FORMAT, 'invitations': [
            {'organization_id': self.organization, 'code_digest': m['code_digest'], 'email': m['email'], 'role': PROVIDER_ROLE,
             'issuer': issuer, 'tenant': tenant} for m in minted]}
        text = json.dumps(document, indent=2, sort_keys=True) + '\n'
        require(len(admission.invitation_input(text)) == count, 'the composed input is not one the operator accepts')
        record = {'status': 'minted', 'organization_id': self.organization, 'run_id': self.run_id, 'count': count, 'with_notes': with_notes,
                  'expires_days': expires_days, 'codes': [dict(m, code_id=None, outcome='not-issued') for m in minted],
                  'input_cleared': None, 'capabilities_before': len(current),
                  'handling': 'Deliver each code privately once, by your own email. Never paste it into a task, log, ticket, chat or evidence.'}
        _write(output, record)  # The codes exist on disk before anything reaches AWS.
        issued = failed = 0
        self.put_secret(input_arn, text, 'invitation-input-written')
        try:
            for index, entry in enumerate(record['codes']):
                sets = [('--organization', self.organization), ('--code-digest', entry['code_digest']), ('--expires-days', str(expires_days))]
                if with_notes:
                    sets.append(('--note', entry['note']))
                outcome = self.operator_task(ISSUE_OVERRIDES[with_notes], sets, 'issue')
                result = outcome['result'] or {}
                entry.update(task_arn=outcome['task_arn'], exit_code=outcome['exit_code'])
                if outcome['exit_code'] == 0 and result.get('code_digest') == entry['code_digest']:
                    entry.update(outcome='issued', code_id=result.get('code_id'), expires_at=result.get('expires_at'),
                                 provider_invitation=result.get('provider_invitation'))
                    issued += 1
                else:
                    entry.update(outcome='failed', refused=result.get('refused'))
                    failed += 1
                    break  # The rest of the wave stays 'not-issued'; the secret is cleared below.
        finally:
            # Always: the input holds the addresses and must not outlive the wave.
            try:
                self.put_secret(input_arn, EMPTY_INPUT, 'invitation-input-cleared')
                record['input_cleared'] = json.loads(self.secret(input_arn)) == {}
            except Exception:
                record['input_cleared'] = False
            record['status'] = 'issued' if failed == 0 and issued == count else 'incomplete'
            record['issued'], record['failed'] = issued, failed
            _write(output, record)
        require(record['input_cleared'] is True, 'invitation input secret was not confirmed empty; clear it before anything else')
        require(failed == 0 and issued == count, 'wave incomplete: the private output names each code\'s outcome; issue the rest in a new wave')
        after = self.runtime.capability_map(self.secret(capability_arn))
        public = {'status': record['status'], 'issued': issued, 'failed': failed, 'not_issued': count - issued - failed,
                  'capabilities_before': len(current), 'capabilities_after': len(after), 'capabilities_max': self.runtime.CAPABILITIES_MAX,
                  'code_digests': [m['code_digest'] for m in minted], 'input_cleared': True, 'receipts': self.receipts,
                  'next': 'One announced `restart` hands every capability to the service.'}
        self.receipt('issue-summary', dict(public, receipts=None))
        return public

    def binding(self, identifiers):
        """The token issuer and the organization's provider tenant from FormaStack's private identifiers file."""
        require(isinstance(identifiers, dict) and isinstance(identifiers.get('jwt_iss'), str) and isinstance(identifiers.get('tenants'), list),
                'identifiers file lacks jwt_iss or tenants')
        tenants = [t for t in identifiers['tenants'] if isinstance(t, dict) and t.get('genui_org_uuid') == self.organization]
        require(len(tenants) == 1 and isinstance(tenants[0].get('tenant_id'), str), 'identifiers file binds no single tenant to this organization')
        return identifiers['jwt_iss'], tenants[0]['tenant_id']

    def list_codes(self):
        outcome = self.operator_task('admission-list-trial', [('--organization', self.organization)], 'list')
        require(outcome['exit_code'] == 0 and isinstance(outcome['result'], dict) and 'codes' in outcome['result'], 'list task did not succeed')
        codes = outcome['result']['codes']
        by_status = {}
        for code in codes:
            by_status[code['status']] = by_status.get(code['status'], 0) + 1
        return {'status': 'listed', 'count': len(codes), 'by_status': by_status, 'truncated': outcome['result'].get('truncated'),
                'codes': codes, 'receipts': self.receipts}

    def revoke(self, code_id):
        require(isinstance(code_id, str) and UUID_RE.fullmatch(code_id), 'code id must be a canonical UUID')
        outcome = self.operator_task('admission-revoke-trial', [('--organization', self.organization), ('--code-id', code_id)], 'revoke')
        require(outcome['exit_code'] == 0 and isinstance(outcome['result'], dict), 'revoke task did not succeed')
        return dict(outcome['result'], status='revoked', receipts=self.receipts)

    def grant_admin(self, principal):
        require(isinstance(principal, str) and UUID_RE.fullmatch(principal), 'principal must be a canonical UUID')
        outcome = self.operator_task('admission-grant-admin', [('--organization', self.organization), ('--principal', principal)], 'grant-admin')
        require(outcome['exit_code'] == 0 and isinstance(outcome['result'], dict) and outcome['result'].get('permission') == 'admin_policy',
                'grant-admin task did not succeed')
        return dict(outcome['result'], status='granted', receipts=self.receipts)

    def delete_uploads(self, principal):
        """Every S3 version and delete marker of the member's box-file keys, with the operator's identity."""
        require(isinstance(principal, str) and UUID_RE.fullmatch(principal), 'principal must be a canonical UUID')
        outcome = self.operator_task('admission-list-box-files', [('--organization', self.organization), ('--principal', principal)], 'list-box-files')
        listed = outcome['result']
        require(outcome['exit_code'] == 0 and isinstance(listed, dict) and isinstance(listed.get('files'), list), 'list-box-files task did not succeed')
        require(listed.get('truncated') is False, 'more files than one listing shows; delete in several passes is not supported, inspect first')
        bucket = self.outputs['EvidenceBucket']
        pattern = re.compile(r'o/' + re.escape(self.organization) + r'/b/' + UUID_RE.pattern + r'/f/' + UUID_RE.pattern)
        deleted = []
        for entry in listed['files']:
            key = entry.get('object_key')
            require(isinstance(key, str) and pattern.fullmatch(key), 'object key outside this organization\'s box files: refused')
            versions = self.versions(bucket, key)
            for version in versions:
                self.aws.call('s3', 'delete_object', Bucket=bucket, Key=key, VersionId=version['VersionId'])
            remaining = self.versions(bucket, key)
            self.receipt('delete-' + entry.get('file_id', key.rsplit('/', 1)[-1]), {'bucket': bucket, 'key': key, 'deleted_versions': versions, 'remaining': remaining})
            require(not remaining, 'object versions remain after deletion: ' + key)
            deleted.append({'key': key, 'versions': len(versions)})
        return {'status': 'deleted', 'principal_id': principal, 'files': len(deleted), 'versions': sum(d['versions'] for d in deleted),
                'keys': [d['key'] for d in deleted], 'receipts': self.receipts,
                'note': 'The box_files rows stay until the end-of-trial purge; the bytes are gone.'}

    def versions(self, bucket, key):
        response = self.aws.call('s3', 'list_object_versions', Bucket=bucket, Prefix=key)
        require(not response.get('IsTruncated'), 'object version listing truncated; inspect before deleting')
        found = []
        for group in ('Versions', 'DeleteMarkers'):
            for item in response.get(group) or []:
                if item.get('Key') == key:
                    require(isinstance(item.get('VersionId'), str) and item['VersionId'], 'object version without an id')
                    found.append({'VersionId': item['VersionId'], 'kind': group})
        return found

    def restart(self, announced):
        require(announced is True, 'a restart signs every member out: announce it first, then pass --announced')
        cluster, service = self.outputs['ClusterName'], self.outputs['ServiceName']
        runner = Runner(self.aws, self.store)
        self.store.load()
        before = runner.boundary(1)
        response = self.aws.call('ecs', 'update_service', cluster=cluster, service=service, forceNewDeployment=True)
        self.receipt('restart-requested', {'cluster': cluster, 'service': service, 'deployment_id': (response.get('service') or {}).get('deployments', [{}])[0].get('id')})
        def settled(result):
            rows = [r for r in result.get('services') or [] if r.get('serviceName') == service]
            return not result.get('failures') and len(rows) == 1 and len(rows[0].get('deployments') or []) == 1 \
                and rows[0]['deployments'][0].get('rolloutState') == 'COMPLETED' and rows[0].get('runningCount') == rows[0].get('desiredCount') == 1
        self.aws.wait('ecs', 'describe_services', settled, timeout=RESTART_WAIT_SECONDS, cluster=cluster, services=[service])
        after = runner.boundary(1)  # The same images, template, parameters and schedules; healthy targets.
        main_before = next(r for r in before['services']['services'] if r.get('serviceName') == service)
        main_after = next(r for r in after['services']['services'] if r.get('serviceName') == service)
        self.receipt('restart-settled', {'before_task_definition': main_before.get('taskDefinition'),
                                         'after': {k: main_after.get(k) for k in ('taskDefinition', 'runningCount', 'desiredCount')}})
        return {'status': 'restarted', 'service': service, 'receipts': self.receipts}


def _now():
    from .state import now
    return now()


def _write(path, value):
    with open(path, 'r+b') as output:
        output.seek(0); output.truncate()
        output.write(encoded(value)); output.flush(); os.fsync(output.fileno())


def read_json(path):
    return json.loads(Path(path).read_text(), object_pairs_hook=lambda pairs: _unique(pairs))


def _unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON field')
        result[key] = value
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('command', choices=['issue', 'list', 'revoke', 'grant-admin', 'delete-uploads', 'restart'])
    parser.add_argument('--state-bucket', required=True)
    parser.add_argument('--state-prefix', default='beta')
    parser.add_argument('--expected-state-sha256', required=True)
    parser.add_argument('--organization', required=True, help='the trial organization UUID (must equal the stack\'s CleanTrialOrganizationId)')
    parser.add_argument('--output', type=Path, required=True, help='new private 0600 result file; for issue it holds the codes; never overwritten')
    parser.add_argument('--count', type=int, help='issue: the number of codes, equal to the recipients in --notes-file')
    parser.add_argument('--notes-file', type=Path, help='issue: private JSON {"recipients": [{"email": ..., "note": ...}]}')
    parser.add_argument('--identifiers', type=Path, help="issue: FormaStack's private identifiers JSON (jwt_iss and the tenant of this organization)")
    parser.add_argument('--expires-days', type=int, default=14, help='issue: code expiry, 1 to 30 days')
    parser.add_argument('--without-notes', action='store_true', help='issue: before migration 0023 (no access_codes.note column)')
    parser.add_argument('--code', help='revoke: the code id')
    parser.add_argument('--principal', help='grant-admin, delete-uploads: the member\'s principal id')
    parser.add_argument('--announced', action='store_true', help='restart: the members were told the service restarts now')
    args = parser.parse_args(argv)
    if args.command == 'issue' and not (args.count and args.notes_file and args.identifiers):
        parser.error('issue needs --count, --notes-file and --identifiers')
    if args.command == 'revoke' and not args.code:
        parser.error('revoke needs --code')
    if args.command in ('grant-admin', 'delete-uploads') and not args.principal:
        parser.error(args.command + ' needs --principal')
    output = None
    try:
        # Reserve the private output before any AWS work; never overwrite an attempt.
        fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        os.close(fd)
        output = args.output
        require(os.environ.get('GITHUB_ACTIONS', '').lower() != 'true', 'admission is not a workflow operation')
        aws = AWS()
        store = Store(aws.client('s3'), args.state_bucket, args.state_prefix)
        trial = Trial(aws, store, args.expected_state_sha256, args.organization)
        if args.command == 'issue':
            recipients = read_json(args.notes_file)
            require(isinstance(recipients, dict) and set(recipients) == {'recipients'} and isinstance(recipients['recipients'], list),
                    'notes file must hold exactly a "recipients" list')
            result = trial.issue(recipients['recipients'], read_json(args.identifiers), args.count, not args.without_notes, args.expires_days, output)
        elif args.command == 'list':
            result = trial.list_codes()
        elif args.command == 'revoke':
            result = trial.revoke(args.code)
        elif args.command == 'grant-admin':
            result = trial.grant_admin(args.principal)
        elif args.command == 'delete-uploads':
            result = trial.delete_uploads(args.principal)
        else:
            result = trial.restart(args.announced)
        if args.command != 'issue':
            _write(output, result)
        public = {k: v for k, v in result.items() if k not in ('codes',)}
        print(json.dumps(public, sort_keys=True, default=str))
        return 0
    except Exception as error:
        if output is not None and args.command != 'issue':
            try:
                _write(output, {'status': 'refused-or-outcome-unknown', 'instruction': 'Inspect the receipts and the task before any retry; nothing is repeated automatically.'})
            except OSError:
                pass
        reason = str(error) if type(error).__name__ == 'Refused' else 'refused or outcome unknown'
        print('Admission refused or outcome unknown: ' + reason + '. Inspect private receipts; nothing was repeated.', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
