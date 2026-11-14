"""Fake-AWS campaign for the trial back office: every refusal, the always-cleared secret, receipts."""
import copy
import json
import os
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch

from ops.promotion import admission, guards
from ops.promotion.admission import Trial
from ops.promotion.state import Refused, Store, digest, encoded
from ops.promotion.test_runner import FakeAWS, fixture
from ops.promotion.test_state import MemoryS3

ORG = '7f2a9c4e-1b3d-4a5e-8f60-123456789abc'
OTHER_ORG = '00000000-0000-4000-8000-0000000000e1'
PRINCIPAL = '2b1c3d4e-5f60-4718-9a2b-abcdef012345'
SECRETS = 'arn:aws:secretsmanager:%s:%s:secret:' % (guards.REGION, guards.ACCOUNT)
INPUT_ARN, CAPABILITY_ARN, MANAGEMENT_ARN = SECRETS + 'input-AbCdEf', SECRETS + 'capabilities-GhIjKl', SECRETS + 'management-MnOpQr'
BUCKET = 'evidence-bucket-fixture'
DIGEST = 'sha256:' + '0' * 64


def capability(n):
    hexn = '%012x' % n
    organization, invitation = ORG, '11111111-2222-4333-8444-' + hexn
    return organization + '.' + invitation, {
        'format': 'forma:fauth-invite-capability:v1', 'organization_id': organization, 'invitation_id': invitation,
        'remote_invitation_id': 'tinv_aaaaaaaa-bbbb-cccc-dddd-' + hexn, 'provider_expires_at': '2026-12-31T00:00:00Z',
        'invite_token': 'tinv_eeeeeeee-ffff-0000-1111-' + hexn}


class TrialAWS(FakeAWS):
    """FakeAWS plus the operator channel: secrets, RunTask, task logs, object versions, a forced deployment."""

    def __init__(self, state, s3, manifest):
        super().__init__(state, s3, manifest)
        self.phase = 'operator'
        self.secrets = {INPUT_ARN: '{}', CAPABILITY_ARN: '{}'}
        self.puts = []
        self.tasks = {}
        self.results = []  # (exit_code, printed result) per RunTask, in order
        self.run_requests = []
        self.objects = {}  # (bucket, key) -> [version ids]
        self.deleted = []
        self.forced = 0
        self.account = guards.ACCOUNT

    def identity(self):
        return {'Account': self.account, 'Arn': 'arn:aws:sts::%s:assumed-role/%s/fixture' % (self.account, self.phase)}

    def _call(self, s, o, a):
        if s == 'secretsmanager':
            if o == 'get_secret_value':
                return {'SecretString': self.secrets[a['SecretId']]}
            if o == 'put_secret_value':
                self.puts.append((a['SecretId'], a['SecretString']))
                self.secrets[a['SecretId']] = a['SecretString']
                return {'VersionId': 'v%d' % len(self.puts)}
        if s == 'ecs' and o == 'run_task':
            self.run_requests.append(copy.deepcopy(a))
            if not self.results:
                return {'tasks': [], 'failures': [{'reason': 'synthetic capacity'}]}
            exit_code, result = self.results.pop(0)
            arn = 'arn:aws:ecs:%s:%s:task/beta/%032x' % (guards.REGION, guards.ACCOUNT, len(self.tasks) + 1)
            self.tasks[arn] = (exit_code, result, a['overrides']['containerOverrides'][0]['command'])
            return {'tasks': [{'taskArn': arn, 'lastStatus': 'PROVISIONING'}], 'failures': []}
        if s == 'ecs' and o == 'describe_tasks' and a['tasks'][0] in self.tasks:
            exit_code, _, _ = self.tasks[a['tasks'][0]]
            return {'tasks': [{'taskArn': a['tasks'][0], 'lastStatus': 'STOPPED', 'stopCode': 'EssentialContainerExited',
                               'containers': [{'name': 'operator', 'exitCode': exit_code}]}], 'failures': []}
        if s == 'logs':
            task = next(arn for arn in self.tasks if a['logStreamName'].endswith(arn.rsplit('/', 1)[-1]))
            _, result, _ = self.tasks[task]
            return {'events': [{'message': 'not json'}, {'message': json.dumps(result)}]}
        if s == 's3' and o == 'list_object_versions':
            versions = self.objects.get((a['Bucket'], a['Prefix']), [])
            return {'IsTruncated': False, 'Versions': [{'Key': a['Prefix'], 'VersionId': v} for v in versions if not v.startswith('dm-')],
                    'DeleteMarkers': [{'Key': a['Prefix'], 'VersionId': v} for v in versions if v.startswith('dm-')]}
        if s == 's3' and o == 'delete_object':
            # m1: the intent receipt names every key and version before the first deletion.
            intents = [k for k in self.s3.objects if k.endswith('/delete-uploads-intent.json')]
            assert intents, 'delete_object before the intent receipt'
            intent = json.loads(self.s3.objects[intents[-1]])
            assert any(f['key'] == a['Key'] and any(v['VersionId'] == a['VersionId'] for v in f['versions']) for f in intent['files']), 'deletion outside the intent'
            self.deleted.append((a['Bucket'], a['Key'], a['VersionId']))
            self.objects[a['Bucket'], a['Key']].remove(a['VersionId'])
            return {}
        if s == 'ecs' and o == 'update_service' and a.get('forceNewDeployment'):
            # m2: the intent receipt precedes the one mutation.
            assert any(k.endswith('/restart-intent.json') for k in self.s3.objects), 'update_service before the intent receipt'
            self.forced += 1
            return {'service': {'deployments': [{'id': 'ecs-svc/1', 'rolloutState': 'IN_PROGRESS'}]}}
        return super()._call(s, o, a)


class AdmissionTests(unittest.TestCase):
    def setUp(self):
        environment = patch.dict('os.environ', {'GITHUB_ACTIONS': 'false'})
        environment.start(); self.addCleanup(environment.stop)
        manifest = {'source': {'schema_manifest_sha256': 'a' * 64, 'deployment_contract_sha256': 'b' * 64}}
        self.state = fixture(manifest)
        live = self.state['live']
        live['outputs'].update(PublicTaskSubnets='subnet-0123456789abcdef0,subnet-0123456789abcdef1', TaskSecurityGroup='sg-0123456789abcdef0',
                               OperatorInvitationInputSecretArn=INPUT_ARN, ProviderCapabilitySecretArn=CAPABILITY_ARN, EvidenceBucket=BUCKET)
        live['parameters'].update(ProviderManagementSecretArn=MANAGEMENT_ARN, ProviderManagementUrl='https://auth.example.test',
                                  CleanTrialOrganizationId=ORG)
        live['config']['clean_trial_organization_id'] = ORG
        operator = self.state['baseline']['definitions']['OperatorTaskDefinition']['containerDefinitions'][0]
        operator['name'] = 'operator'
        operator['logConfiguration'] = {'logDriver': 'awslogs', 'options': {'awslogs-group': '/genui/operator', 'awslogs-stream-prefix': 'genui-operator'}}
        self.s3 = MemoryS3(self.state)
        self.store = Store(self.s3, 'bucket', 'beta')
        self.aws = TrialAWS(self.state, self.s3, manifest)
        self.expected = digest(self.state)
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.output = Path(self.tmp.name) / 'result.json'
        fd = os.open(self.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600); os.close(fd)
        self.recipients = [{'email': 'ada@example.test', 'note': 'Ada L.'}, {'email': 'bob@example.test', 'note': 'Bob M.'}]
        self.identifiers = {'jwt_iss': 'https://auth.example.test/tenant', 'tenants': [
            {'genui_org_uuid': ORG, 'tenant_id': 'ten_trial', 'slug': 'trial', 'client_id': 'c1'}]}

    def replace_state(self):
        self.s3.objects['beta/state.json'] = encoded(self.state)
        self.aws.state = copy.deepcopy(self.state)
        self.expected = digest(self.state)

    def trial(self, expected=None, organization=ORG):
        return Trial(self.aws, self.store, expected or self.expected, organization, run_id='00000000-0000-4000-8000-00000000aaaa')

    def issue_results(self, count):
        return [(0, {'code_id': '00000000-0000-4000-8000-0000000000c%d' % n, 'code_digest': None, 'kind': 'member_invite', 'expires_at': '2026-12-12T00:00:00Z',
                     'provider_invitation': {'state': 'confirmed'}}) for n in range(count)]

    def receipts(self):
        return {k: json.loads(v) for k, v in self.s3.objects.items() if k.startswith('beta/promotions/admission-')}

    def assert_nothing_mutated(self):
        self.assertEqual(self.aws.puts, [])
        self.assertEqual(self.aws.run_requests, [])
        self.assertEqual(self.aws.deleted, [])
        self.assertEqual(self.aws.forced, 0)
        self.assertEqual(self.receipts(), {})
        self.assertEqual(self.s3.objects['beta/state.json'], encoded(self.state))

    # --- issue -------------------------------------------------------------------------------

    def run_issue(self, count=2, with_notes=True, recipients=None, identifiers=None, expires_days=14):
        trial = self.trial()
        # The digests are drawn inside; bind each scripted result to the request it answers.
        results = self.issue_results(count)
        original = self.aws._call
        def bind(s, o, a):
            if s == 'ecs' and o == 'run_task' and self.aws.results:
                command = a['overrides']['containerOverrides'][0]['command']
                self.aws.results[0][1]['code_digest'] = command[command.index('--code-digest') + 1]
            return original(s, o, a)
        self.aws._call = bind
        self.aws.results = results
        return trial, trial.issue(recipients if recipients is not None else self.recipients, identifiers or self.identifiers, count, with_notes, expires_days, self.output)

    def test_issue_mints_locally_writes_then_clears_the_input_and_runs_one_noted_task_per_code(self):
        trial, public = self.run_issue()
        self.assertEqual((public['status'], public['issued'], public['failed'], public['not_issued']), ('issued', 2, 0, 0))
        self.assertEqual((public['capabilities_before'], public['capabilities_max'], public['input_cleared']), (0, 128, True))
        # The one-shot input: written once with both entries, then the empty document, in that order.
        self.assertEqual([arn for arn, _ in self.aws.puts], [INPUT_ARN, INPUT_ARN])
        written = json.loads(self.aws.puts[0][1])
        self.assertEqual(written['format'], 'forma:operator-invitation-input:v1')
        self.assertEqual([e['email'] for e in written['invitations']], ['ada@example.test', 'bob@example.test'])
        self.assertTrue(all(e['role'] == 'user' and e['issuer'] == self.identifiers['jwt_iss'] and e['tenant'] == 'ten_trial' and e['organization_id'] == ORG
                            for e in written['invitations']))
        self.assertEqual(self.aws.puts[1][1], '{}')
        self.assertEqual(self.aws.secrets[INPUT_ARN], '{}')
        # One task per code, the documented override with only its flag values replaced, in the documented request shape.
        self.assertEqual(len(self.aws.run_requests), 2)
        for index, request in enumerate(self.aws.run_requests):
            command = request['overrides']['containerOverrides'][0]['command']
            self.assertEqual(command[:2], ['admission', 'issue'])
            self.assertEqual(command[command.index('--organization') + 1], ORG)
            self.assertEqual(command[command.index('--code-digest') + 1], written['invitations'][index]['code_digest'])
            self.assertEqual(command[command.index('--note') + 1], self.recipients[index]['note'])
            self.assertEqual(command[command.index('--expires-days') + 1], '14')
            self.assertIn('--provider-input-from-secret', command); self.assertIn('--authoring-only', command)
            self.assertNotIn('ada@example.test', json.dumps(request)); self.assertNotIn('bob@example.test', json.dumps(request))
            self.assertEqual((request['cluster'], request['launchType'], request['count'], request['enableExecuteCommand']), ('beta', 'FARGATE', 1, False))
            self.assertEqual(request['taskDefinition'], self.state['live']['outputs']['OperatorTaskDefinition'])
        # The codes went only to the private file; the public result and receipts carry digests, never codes or addresses.
        record = json.loads(self.output.read_bytes())
        self.assertEqual(stat.S_IMODE(self.output.stat().st_mode), 0o600)
        self.assertEqual((record['status'], record['issued'], record['input_cleared']), ('issued', 2, True))
        self.assertEqual([c['outcome'] for c in record['codes']], ['issued', 'issued'])
        for entry, invitation in zip(record['codes'], written['invitations']):
            self.assertEqual(entry['code_digest'], invitation['code_digest'])
            raw = trial.admission.normalize_code(entry['code'])
            self.assertEqual(trial.admission.code_digest(raw), entry['code_digest'])
            self.assertTrue(entry['code_id'].startswith('00000000-0000-4000-8000-0000000000c'))
        everything = json.dumps(public) + json.dumps(self.receipts())
        for private in [c['code'] for c in record['codes']] + ['ada@example.test', 'bob@example.test']:
            self.assertNotIn(private, everything)
        names = sorted(k.rsplit('/', 1)[-1] for k in self.receipts())
        self.assertEqual(names, ['invitation-input-cleared.json', 'invitation-input-written.json', 'issue-1-run.json', 'issue-1-stopped.json',
                                 'issue-2-run.json', 'issue-2-stopped.json', 'issue-summary.json'])
        self.assertEqual(public['code_digests'], [i['code_digest'] for i in written['invitations']])
        self.assertEqual(self.s3.objects['beta/state.json'], encoded(self.state))  # the canonical state is never changed

    def test_issue_without_notes_uses_the_unnoted_override_and_ignores_notes(self):
        _, public = self.run_issue(with_notes=False, recipients=[{'email': 'ada@example.test'}, {'email': 'bob@example.test', 'note': 'x'}])
        self.assertEqual(public['status'], 'issued')
        for request in self.aws.run_requests:
            self.assertNotIn('--note', request['overrides']['containerOverrides'][0]['command'])

    def test_a_failed_task_stops_the_wave_and_still_clears_the_input(self):
        trial = self.trial()
        self.aws.results = [(1, {'refused': 'note_unsupported'})]
        with self.assertRaises(Refused):
            trial.issue(self.recipients, self.identifiers, 2, True, 14, self.output)
        self.assertEqual(len(self.aws.run_requests), 1)
        self.assertEqual(self.aws.secrets[INPUT_ARN], '{}')
        self.assertEqual([arn for arn, _ in self.aws.puts], [INPUT_ARN, INPUT_ARN])
        record = json.loads(self.output.read_bytes())
        self.assertEqual((record['status'], record['issued'], record['failed'], record['input_cleared']), ('incomplete', 0, 1, True))
        self.assertEqual([c['outcome'] for c in record['codes']], ['failed', 'not-issued'])
        self.assertEqual(record['codes'][0]['refused'], 'note_unsupported')
        self.assertTrue(all('code' in c for c in record['codes']))  # the minted codes are kept for the record

    def test_a_lost_run_task_or_clear_failure_keeps_the_codes_and_reports_the_secret(self):
        trial = self.trial()
        self.aws.results = []  # RunTask answers with a failure and no task
        with self.assertRaises(Refused):
            trial.issue(self.recipients, self.identifiers, 2, True, 14, self.output)
        self.assertEqual(self.aws.secrets[INPUT_ARN], '{}')
        self.assertEqual(json.loads(self.output.read_bytes())['input_cleared'], True)
        # The clear itself failing: the file says so and the command refuses.
        self.setUp()
        trial = self.trial()
        self.aws.results = self.issue_results(2)
        original = self.aws._call
        def failing_clear(s, o, a):
            if s == 'secretsmanager' and o == 'put_secret_value' and a['SecretString'] == '{}':
                raise TimeoutError('lost')
            return original(s, o, a)
        self.aws._call = failing_clear
        with self.assertRaises(Refused):
            trial.issue(self.recipients, self.identifiers, 2, True, 14, self.output)
        record = json.loads(self.output.read_bytes())
        self.assertFalse(record['input_cleared'])
        self.assertNotEqual(self.aws.secrets[INPUT_ARN], '{}')
        self.assertEqual(len(record['codes']), 2)

    def test_every_precondition_refuses_before_any_write(self):
        cases = []
        def wrong_account(): self.aws.account = '111111111111'
        def wrong_stack_organization(): self.state['live']['parameters']['CleanTrialOrganizationId'] = OTHER_ORG; self.replace_state()
        def wrong_config_organization(): self.state['live']['config']['clean_trial_organization_id'] = OTHER_ORG; self.replace_state()
        def stale_hash(): self.expected = '0' * 64
        def held_lock(): self.state['active'] = {'owner': 'x', 'promotion_id': 'p'}; self.replace_state()
        def workflow_identity(): self.aws.phase = 'execute'
        def input_not_cleared(): self.aws.secrets[INPUT_ARN] = json.dumps({'format': 'forma:operator-invitation-input:v1', 'invitations': []})
        def too_many_capabilities(): self.aws.secrets[CAPABILITY_ARN] = json.dumps(dict(capability(n) for n in range(127)))
        def no_task_definition(): del self.state['live']['outputs']['OperatorTaskDefinition']; self.replace_state()
        def no_operator_image(): self.state['live']['parameters']['OperatorImageDigest'] = ''; self.replace_state()
        def no_management_secret(): self.state['live']['parameters']['ProviderManagementSecretArn'] = ''; self.replace_state()
        def no_management_url(): self.state['live']['parameters']['ProviderManagementUrl'] = ''; self.replace_state()
        def no_input_secret_output(): del self.state['live']['outputs']['OperatorInvitationInputSecretArn']; self.replace_state()
        def github_actions(): os.environ['GITHUB_ACTIONS'] = 'true'
        for name, prepare in [('wrong account', wrong_account), ('wrong stack organization', wrong_stack_organization),
                              ('wrong canonical organization', wrong_config_organization), ('stale state hash', stale_hash), ('held lock', held_lock),
                              ('workflow identity', workflow_identity), ('input secret not cleared', input_not_cleared),
                              ('too many capabilities', too_many_capabilities), ('no task definition', no_task_definition),
                              ('no operator image', no_operator_image), ('no management secret', no_management_secret),
                              ('no management url', no_management_url), ('no input secret output', no_input_secret_output),
                              ('github actions', github_actions)]:
            with self.subTest(case=name):
                self.setUp(); prepare()
                with self.assertRaises(Refused):
                    trial = self.trial()
                    self.aws.results = self.issue_results(2)
                    trial.issue(self.recipients, self.identifiers, 2, True, 14, self.output)
                self.assert_nothing_mutated()
                cases.append(name)
        self.assertEqual(len(cases), 14)
        # 126 existing plus 2 fits exactly; 127 plus 2 refused above.
        self.setUp(); self.aws.secrets[CAPABILITY_ARN] = json.dumps(dict(capability(n) for n in range(126)))
        _, public = self.run_issue()
        self.assertEqual((public['capabilities_before'], public['issued']), (126, 2))

    def test_recipient_and_argument_refusals_write_nothing(self):
        bad = [
            ('count differs', dict(count=3)),
            ('zero', dict(count=0, recipients=[])),
            ('repeated address', dict(recipients=[{'email': 'ada@example.test', 'note': 'a'}, {'email': 'ADA@example.test', 'note': 'b'}])),
            ('malformed address', dict(recipients=[{'email': 'ada example.test', 'note': 'a'}, {'email': 'bob@example.test', 'note': 'b'}])),
            ('missing note', dict(recipients=[{'email': 'ada@example.test'}, {'email': 'bob@example.test', 'note': 'b'}])),
            ('long note', dict(recipients=[{'email': 'ada@example.test', 'note': 'x' * 201}, {'email': 'bob@example.test', 'note': 'b'}])),
            ('unknown field', dict(recipients=[{'email': 'ada@example.test', 'note': 'a', 'phone': '1'}, {'email': 'bob@example.test', 'note': 'b'}])),
            ('expiry out of range', dict(expires_days=31)),
            ('no tenant for organization', dict(identifiers={'jwt_iss': 'https://auth.example.test/t', 'tenants': []})),
            ('two tenants', dict(identifiers={'jwt_iss': 'i', 'tenants': [{'genui_org_uuid': ORG, 'tenant_id': 'a'}, {'genui_org_uuid': ORG, 'tenant_id': 'b'}]})),
        ]
        for name, overrides in bad:
            with self.subTest(case=name):
                self.setUp()
                with self.assertRaises(Refused):
                    self.run_issue(**overrides)
                self.assert_nothing_mutated()
        with self.assertRaises(Refused):
            self.setUp(); self.aws.results = self.issue_results(65)
            self.trial().issue([{'email': 'u%d@example.test' % n, 'note': 'n%d' % n} for n in range(65)], self.identifiers, 65, True, 14, self.output)
        self.assert_nothing_mutated()

    # --- the task commands ----------------------------------------------------------------------

    def test_list_revoke_and_grant_admin_run_exactly_their_documented_overrides(self):
        trial = self.trial()
        code = '00000000-0000-4000-8000-0000000000c1'
        self.aws.results = [(0, {'organization_id': ORG, 'codes': [{'code_id': code, 'status': 'active', 'note': 'Ada L.'}], 'truncated': False}),
                            (0, {'code_id': code, 'status': 'revoked', 'changed': True, 'provider_revocations_pending': 1, 'capability_phase': 'completed'}),
                            (0, {'principal_id': PRINCIPAL, 'permission': 'admin_policy', 'changed': True, 'membership_epoch': 1})]
        listed = trial.list_codes()
        self.assertEqual((listed['status'], listed['count'], listed['by_status']), ('listed', 1, {'active': 1}))
        self.assertEqual(self.aws.run_requests[0]['overrides']['containerOverrides'][0]['command'], ['admission', 'list', '--organization', ORG])
        revoked = trial.revoke(code)
        self.assertEqual((revoked['status'], revoked['changed'], revoked['capability_phase']), ('revoked', True, 'completed'))
        self.assertEqual(self.aws.run_requests[1]['overrides']['containerOverrides'][0]['command'], ['admission', 'revoke', '--organization', ORG, '--code-id', code])
        granted = trial.grant_admin(PRINCIPAL)
        self.assertEqual((granted['status'], granted['permission']), ('granted', 'admin_policy'))
        self.assertEqual(self.aws.run_requests[2]['overrides']['containerOverrides'][0]['command'],
                         ['admission', 'grant-permission', '--organization', ORG, '--principal', PRINCIPAL, '--permission', 'admin_policy'])
        self.aws.results = [(0, {'principal_id': PRINCIPAL, 'permission': 'admin_policy', 'changed': True, 'membership_epoch': 2})]
        ungranted = trial.revoke_admin(PRINCIPAL)
        self.assertEqual((ungranted['status'], ungranted['permission'], ungranted['membership_epoch']), ('revoked-admin', 'admin_policy', 2))
        self.assertEqual(self.aws.run_requests[3]['overrides']['containerOverrides'][0]['command'],
                         ['admission', 'revoke-permission', '--organization', ORG, '--principal', PRINCIPAL, '--permission', 'admin_policy'])
        self.assertEqual(self.aws.puts, [])  # none of these touches the input secret
        for request in self.aws.run_requests:
            self.assertEqual(request['startedBy'][:7], 'app078-')
            self.assertEqual(len(request['clientToken']), 64)
        self.assertEqual(len({r['clientToken'] for r in self.aws.run_requests}), 4)
        # A failed task, a malformed id and a foreign organization in the override refuse.
        self.aws.results = [(1, {'refused': 'code_unknown'})]
        with self.assertRaises(Refused): trial.revoke(code)
        with self.assertRaises(Refused): trial.revoke('not-a-uuid')
        with self.assertRaises(Refused): trial.grant_admin(PRINCIPAL.upper())
        with self.assertRaises(Refused): trial.revoke_admin('x')
        with self.assertRaises(Refused): trial.compose('admission-list-northstar', [])
        with self.assertRaises(Refused): trial.compose('migrate', [])
        self.assertEqual(len(self.aws.run_requests), 5)

    def test_delete_uploads_removes_every_version_of_the_listed_keys_only(self):
        trial = self.trial()
        box, files = '33333333-4444-4555-8666-777777777777', ['aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeee%d' % n for n in range(2)]
        keys = ['o/%s/b/%s/f/%s' % (ORG, box, f) for f in files]
        self.aws.objects = {(BUCKET, keys[0]): ['v1', 'v2', 'dm-1'], (BUCKET, keys[1]): ['v3']}
        self.aws.results = [(0, {'organization_id': ORG, 'principal_id': PRINCIPAL, 'count': 2, 'truncated': False, 'files': [
            {'box_id': box, 'file_id': files[0], 'object_key': keys[0], 'byte_length': 10, 'content_digest': 'sha256:' + 'a' * 64, 'kind': 'pdf'},
            {'box_id': box, 'file_id': files[1], 'object_key': keys[1], 'byte_length': 20, 'content_digest': 'sha256:' + 'b' * 64, 'kind': 'csv'}]})]
        result = trial.delete_uploads(PRINCIPAL)
        self.assertEqual((result['status'], result['files'], result['versions'], result['keys']), ('deleted', 2, 4, keys))
        self.assertEqual(sorted(self.aws.deleted), sorted([(BUCKET, keys[0], 'v1'), (BUCKET, keys[0], 'v2'), (BUCKET, keys[0], 'dm-1'), (BUCKET, keys[1], 'v3')]))
        self.assertEqual(self.aws.run_requests[0]['overrides']['containerOverrides'][0]['command'],
                         ['admission', 'list-box-files', '--organization', ORG, '--principal', PRINCIPAL])
        receipts = self.receipts()
        self.assertEqual(sorted(k.rsplit('/', 1)[-1] for k in receipts),
                         ['delete-%s.json' % files[0], 'delete-%s.json' % files[1], 'delete-uploads-intent.json', 'list-box-files-1-run.json', 'list-box-files-1-stopped.json'])
        intent = next(v for k, v in receipts.items() if k.endswith('delete-uploads-intent.json'))
        self.assertEqual((intent['bucket'], intent['principal_id'], intent['versions'], [f['key'] for f in intent['files']]), (BUCKET, PRINCIPAL, 4, keys))
        self.assertEqual([v['VersionId'] for v in intent['files'][0]['versions']], ['v1', 'v2', 'dm-1'])
        # A key outside this organization's box files, a truncated listing or a failed task deletes nothing.
        for name, files_result in [
            ('foreign organization', (0, {'files': [{'box_id': box, 'file_id': files[0], 'object_key': 'o/%s/b/%s/f/%s' % (OTHER_ORG, box, files[0])}], 'truncated': False})),
            ('source object', (0, {'files': [{'box_id': box, 'file_id': files[0], 'object_key': 'o/%s/s/%s' % (ORG, files[0])}], 'truncated': False})),
            ('key names another file', (0, {'files': [{'box_id': box, 'file_id': files[1], 'object_key': keys[0]}], 'truncated': False})),
            ('file id not a uuid', (0, {'files': [{'box_id': box, 'file_id': '../x', 'object_key': keys[0]}], 'truncated': False})),
            ('box id missing', (0, {'files': [{'file_id': files[0], 'object_key': keys[0]}], 'truncated': False})),
            ('truncated', (0, {'files': [], 'truncated': True})),
            ('failed task', (1, {'refused': 'member_unknown'}))]:
            with self.subTest(case=name):
                self.aws.deleted = []; self.aws.results = [files_result]
                self.aws.objects = {(BUCKET, 'o/%s/b/%s/f/%s' % (OTHER_ORG, box, files[0])): ['v9'], (BUCKET, 'o/%s/s/%s' % (ORG, files[0])): ['v8'],
                                    (BUCKET, keys[0]): ['v7']}
                before = len(self.receipts())
                with self.assertRaises(Refused): trial.delete_uploads(PRINCIPAL)
                self.assertEqual(self.aws.deleted, [])
                self.assertFalse(any(k.endswith('delete-uploads-intent.json') for k in list(self.receipts())[before:]))

    def test_restart_needs_the_announcement_then_forces_one_deployment_and_proves_the_boundary(self):
        trial = self.trial()
        with self.assertRaises(Refused): trial.restart(False)
        self.assertEqual(self.aws.forced, 0)
        result = trial.restart(True)
        self.assertEqual((result['status'], result['service'], self.aws.forced), ('restarted', 'main', 1))
        update = [a for s, o, a in self.aws.calls if o == 'update_service']
        self.assertEqual(update, [{'cluster': 'beta', 'service': 'main', 'forceNewDeployment': True}])
        self.assertFalse(any(o in ['execute_change_set', 'create_change_set', 'run_task', 'put_secret_value'] for _, o, _ in self.aws.calls))
        self.assertEqual(sorted(k.rsplit('/', 1)[-1] for k in self.receipts()), ['restart-intent.json', 'restart-requested.json', 'restart-settled.json'])
        intent = next(v for k, v in self.receipts().items() if k.endswith('restart-intent.json'))
        self.assertEqual((intent['request'], intent['task_definition']), ({'cluster': 'beta', 'service': 'main', 'forceNewDeployment': True}, self.state['live']['outputs']['TaskDefinition']))
        self.assertEqual(self.s3.objects['beta/state.json'], encoded(self.state))
        # An unhealthy service after the restart refuses rather than reporting success.
        self.setUp(); trial = self.trial(); self.aws.running_health = 'UNHEALTHY'
        with self.assertRaises(Refused): trial.restart(True)

    def test_overrides_file_names_no_private_organization(self):
        document = json.loads(admission.OVERRIDES.read_text())
        trial_entries = [e for e in document['overrides'] if e['name'] in set(admission.ISSUE_OVERRIDES.values())
                         | {'admission-list-trial', 'admission-revoke-trial', 'admission-grant-admin', 'admission-revoke-admin', 'admission-list-box-files'}]
        self.assertEqual(len(trial_entries), 7)
        for entry in trial_entries:
            command = entry['overrides']['containerOverrides'][0]['command']
            self.assertEqual(command[command.index('--organization') + 1], OTHER_ORG)  # the documented example id, never a real one
        text = admission.OVERRIDES.read_text()
        self.assertNotIn(ORG, text)

    def test_cli_reserves_the_output_and_refuses_cleanly(self):
        missing = Path(self.tmp.name) / 'new.json'
        with patch.object(admission, 'AWS', side_effect=AssertionError('no AWS here')):
            code = admission.main(['list', '--state-bucket', 'b', '--expected-state-sha256', self.expected, '--organization', ORG, '--output', str(missing)])
        self.assertEqual(code, 1)
        self.assertEqual(stat.S_IMODE(missing.stat().st_mode), 0o600)
        self.assertEqual(json.loads(missing.read_bytes())['status'], 'refused-or-outcome-unknown')
        # An existing output is never overwritten.
        code = admission.main(['list', '--state-bucket', 'b', '--expected-state-sha256', self.expected, '--organization', ORG, '--output', str(missing)])
        self.assertEqual(code, 1)
        self.assertEqual(json.loads(missing.read_bytes())['status'], 'refused-or-outcome-unknown')


if __name__ == '__main__':
    unittest.main()
