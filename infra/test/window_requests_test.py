"""APP-078 window request composer tests. Local only: no AWS call."""
import base64
import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import tempfile
import time
import unittest

PATH = Path(__file__).parents[1] / 'scripts/window_requests.py'
spec = importlib.util.spec_from_file_location('window_requests', PATH)
wr = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wr)
WINDOW = '6f2b1c9e-0d4a-4f3e-9b8a-5c1d2e3f4a5b'
NORTHSTAR, CANARY = wr.ORGANIZATIONS
OUTPUTS = {'Stacks': [{'StackName': 'GenuiSyntheticFoundation', 'Outputs': [
    {'OutputKey': 'ClusterName', 'OutputValue': 'GenuiSyntheticFoundation-ClusterEB0386A7-HMlpmHwGMVSN'},
    {'OutputKey': 'PublicTaskSubnets', 'OutputValue': 'subnet-06ef6d4f9a7db394e,subnet-0e259603502741ffe'},
    {'OutputKey': 'TaskSecurityGroup', 'OutputValue': 'sg-00b28ce2a112f530b'},
    {'OutputKey': 'OperatorTaskDefinition', 'OutputValue': 'arn:aws:ecs:us-east-1:877573132739:task-definition/GenuiSyntheticFoundationOperatorTask534EBA45:1'},
    {'OutputKey': 'QualificationTaskDefinition', 'OutputValue': 'arn:aws:ecs:us-east-1:877573132739:task-definition/GenuiSyntheticFoundationQualificationTaskB0C9:1'}]}]}
HANDLES = json.loads((wr.REPOSITORY / 'evidence/APP-009/handles.json').read_text())['handles']


def run(*argv):
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = wr.main(list(argv))
    return code, out.getvalue(), err.getvalue()


class Composer(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.private = self.root / 'private'; self.private.mkdir(mode=0o700)
        self.outputs = self.root / 'outputs.json'; self.outputs.write_text(json.dumps(OUTPUTS))

    def tearDown(self):
        self.directory.cleanup()

    def test_mint_code_keeps_the_secret_in_a_private_file_and_prints_only_its_digest(self):
        code, out, err = run('mint-code', '--organization', NORTHSTAR, '--label', 'victor', '--out-dir', str(self.private))
        self.assertEqual((code, err), (0, ''))
        result = json.loads(out)
        saved = Path(result['private_file'])
        self.assertEqual(stat.S_IMODE(saved.stat().st_mode), 0o600)
        record = json.loads(saved.read_text())
        self.assertNotIn(record['code'], out)
        self.assertNotIn(record['code'].replace('-', ''), out)
        admission = wr.admission_operator()
        self.assertEqual(admission.code_digest(admission.normalize_code(record['code'])), result['code_digest'])
        # Never overwritten; an open directory, a foreign organization or an odd label refuses.
        self.assertEqual(run('mint-code', '--organization', NORTHSTAR, '--label', 'victor', '--out-dir', str(self.private))[0], 1)
        self.private.chmod(0o755)
        self.assertEqual(run('mint-code', '--organization', NORTHSTAR, '--label', 'other', '--out-dir', str(self.private))[0], 1)
        self.private.chmod(0o700)
        self.assertEqual(run('mint-code', '--organization', '00000000-0000-4000-8000-000000000003', '--label', 'x', '--out-dir', str(self.private))[0], 1)
        self.assertEqual(run('mint-code', '--organization', NORTHSTAR, '--label', 'Victor Code', '--out-dir', str(self.private))[0], 1)

    def operator(self, *extra):
        output = self.root / 'request.json'
        if output.exists():
            output.unlink()
        code, out, err = run('operator-task', '--outputs', str(self.outputs), '--window-id', WINDOW, '--output', str(output), *extra)
        return code, (json.loads(output.read_text()) if code == 0 else None), err

    def test_issue_handle_installs_the_app009_handles(self):
        for entry in HANDLES:
            code, request, err = self.operator('--step', 'handle-' + entry['organization_id'][-1], '--override', 'admission-issue-handle',
                                               '--set=--organization=' + entry['organization_id'], '--set=--handle=' + entry['handle'])
            self.assertEqual(code, 0, err)
            self.assertEqual(request['overrides'], {'containerOverrides': [{'name': 'operator', 'command': [
                'admission', 'issue-handle', '--organization', entry['organization_id'], '--handle', entry['handle']]}]})
            self.assertEqual((request['launchType'], request['platformVersion'], request['count']), ('FARGATE', '1.4.0', 1))
            self.assertEqual(request['networkConfiguration']['awsvpcConfiguration']['assignPublicIp'], 'ENABLED')
            self.assertEqual(request['taskDefinition'], OUTPUTS['Stacks'][0]['Outputs'][3]['OutputValue'])
            self.assertEqual(len(request['clientToken']), 64)
        self.assertNotEqual(wr.client_token(WINDOW, 'a'), wr.client_token(WINDOW, 'b'))
        self.assertEqual(wr.client_token(WINDOW, 'a'), wr.client_token(WINDOW, 'a'))

    def test_preminted_issue_carries_only_the_digest_and_reviewed_grants(self):
        digest = 'sha256:' + hashlib.sha256(b'synthetic').hexdigest()
        code, request, err = self.operator('--step', 'issue-victor', '--override', 'admission-issue-preminted',
                                           '--set=--code-digest=' + digest, '--set=--issued-by=app078-operator',
                                           '--add=--grant=run:00000000-0000-4000-8000-000000000065')
        self.assertEqual(code, 0, err)
        command = request['overrides']['containerOverrides'][0]['command']
        self.assertEqual(command[:2], ['admission', 'issue'])
        self.assertEqual(command[command.index('--code-digest') + 1], digest)
        self.assertEqual(command.count('--grant'), 2)
        for bad in (['--set=--code-digest=not-a-digest'], ['--set=--provider-email=a@b.test'], ['--add=--provider-email=a@b.test'],
                    ['--set=--grant=admin_policy'], ['--set=--organization=NORTHSTAR']):
            self.assertEqual(self.operator('--step', 'bad', '--override', 'admission-issue-preminted', *bad)[0], 1, bad)
        self.assertEqual(self.operator('--step', 'bad', '--override', 'admission-issue')[0], 1)  # undocumented: raw issue

    def test_inline_registration_uses_the_printed_record(self):
        plan = json.loads((wr.REPOSITORY / 'evidence/APP-077/upload-sources/plan-in-image.json').read_text())
        record_path = self.root / 'record.json'; record_path.write_text(json.dumps(plan['registration'][0]))
        code, request, err = self.operator('--step', 'register', '--override', 'admission-register-source-revision-inline', '--record', str(record_path))
        self.assertEqual(code, 0, err)
        command = request['overrides']['containerOverrides'][0]['command']
        self.assertEqual(json.loads(command[command.index('--record-json') + 1]), plan['registration'][0])
        tampered = dict(plan['registration'][0], row_count=7, revision_id='00000000-0000-4000-8000-0000000000f1')
        record_path.write_text(json.dumps(tampered))
        self.assertEqual(self.operator('--step', 'register', '--override', 'admission-register-source-revision-inline', '--record', str(record_path))[0], 1)
        self.assertEqual(self.operator('--step', 'register', '--override', 'migrate', '--record', str(record_path))[0], 1)

    def test_every_documented_override_composes_and_missing_task_output_refuses(self):
        document = json.loads((wr.REPOSITORY / 'app/operator/overrides.json').read_text())
        for item in document['overrides']:
            if item['name'] == 'admission-register-source-revision-inline':
                continue  # its example contract digest is a placeholder; covered above with the real record
            with self.subTest(name=item['name']):
                self.assertEqual(self.operator('--step', 'x', '--override', item['name'])[0], 0)
        outputs = json.loads(json.dumps(OUTPUTS))
        outputs['Stacks'][0]['Outputs'] = [o for o in outputs['Stacks'][0]['Outputs'] if o['OutputKey'] != 'OperatorTaskDefinition']
        self.outputs.write_text(json.dumps(outputs))
        code, _, err = self.operator('--step', 'x', '--override', 'verify-catalog')
        self.assertEqual(code, 1); self.assertIn('OperatorTaskDefinition', err)
        with self.assertRaises(wr.Refused):
            wr.client_token('not-a-uuid', 'x')

    def test_qualification_request_is_armed_bounded_and_accepted_by_the_supervisor(self):
        harness = wr.load(wr.REPOSITORY / 'app/qualification/local_harness.py', 'h')
        output = self.root / 'qualification.json'
        now = int(time.time())
        code, out, err = run('qualification-task', '--outputs', str(self.outputs), '--window-id', WINDOW, '--step', 'qualification',
                             '--output', str(output), '--rds-resource-id', harness.RESOURCE_ID, '--now', str(now))
        self.assertEqual(code, 0, err)
        result, request = json.loads(out), json.loads(output.read_text())
        self.assertLessEqual(result['override_bytes'], 8192)
        environment = {e['name']: e['value'] for e in request['overrides']['containerOverrides'][0]['environment']}
        self.assertEqual(set(environment), {'GENUI_APP008_MANAGED_QUALIFICATION', 'GENUI_APP008_MANAGED_MANIFEST_JSON'})
        supervisor = wr.load(wr.REPOSITORY / 'app/runtime/qualification_supervisor.py', 's')
        _, manifest = supervisor.take_manifest(dict(environment), now, supervisor.REVIEWED_CA_SHA256)
        self.assertEqual((manifest['run_id'], manifest['expires_at'] - manifest['issued_at']), (result['run_id'], 600))
        self.assertEqual(manifest['pins'], harness.expected_pins())
        self.assertEqual(request['overrides']['containerOverrides'][0]['name'], 'qualification')
        code, _, err = run('qualification-task', '--outputs', str(self.outputs), '--window-id', WINDOW, '--step', 'q2',
                           '--output', str(self.root / 'q2.json'), '--rds-resource-id', 'db-OTHERRESTOREDINSTANCE000000')
        self.assertEqual(code, 1); self.assertIn('stop', err)

    def test_change_set_request_binds_the_receipt_and_content_addressed_template(self):
        parameters = {'Operator': 'Synthetic', 'MonthlyBudgetUsd': '170', 'IngressMode': 'public', 'OperatorIpv4Cidr': '8.8.8.8/32',
                      'DeploymentExpiresAtUtc': '2026-11-30T23:00:00Z', 'TlsCertificateArn': 'arn:aws:acm:us-east-1:877573132739:certificate/x',
                      'ImageDigest': 'd', 'ImageArchitecture': 'ARM64', 'ServiceDesiredCount': '0', 'QualificationConfigSecretArn': '',
                      'QualificationImageDigest': 'q', 'OperatorImageDigest': 'o'}
        receipt = self.root / 'receipt.json'
        receipt.write_text(json.dumps({'status': 'reserved_local_plan_only', 'window_id': WINDOW, 'cloudformation_parameters': parameters}))
        template = self.root / 'template.json'
        template.write_text(json.dumps({'Parameters': {k: {} for k in list(parameters) + ['BootstrapVersion']}, 'Resources': {}}))
        output = self.root / 'change-set.json'
        code, out, err = run('change-set', '--receipt', str(receipt), '--template', str(template), '--window-id', WINDOW, '--output', str(output))
        self.assertEqual(code, 0, err)
        result, request = json.loads(out), json.loads(output.read_text())
        digest = hashlib.sha256(template.read_bytes()).hexdigest()
        self.assertEqual(request['TemplateURL'], 'https://cdk-hnb659fds-assets-877573132739-us-east-1.s3.us-east-1.amazonaws.com/%s.json' % digest)
        self.assertEqual(result['upload']['checksum_sha256_base64'], base64.b64encode(bytes.fromhex(digest)).decode())
        self.assertEqual((request['ChangeSetType'], request['Capabilities']), ('UPDATE', ['CAPABILITY_IAM']))
        values = {p['ParameterKey']: p.get('ParameterValue', p.get('UsePreviousValue')) for p in request['Parameters']}
        self.assertEqual(values, dict(parameters, BootstrapVersion=True))
        # A receipt for another window, or a template with another parameter set, refuses.
        self.assertEqual(run('change-set', '--receipt', str(receipt), '--template', str(template), '--window-id',
                             '00000000-0000-4000-8000-000000000009', '--output', str(output))[0], 1)
        template.write_text(json.dumps({'Parameters': {k: {} for k in parameters}, 'Resources': {}}))
        self.assertEqual(run('change-set', '--receipt', str(receipt), '--template', str(template), '--window-id', WINDOW, '--output', str(output))[0], 1)

    def test_change_set_review_refuses_protected_replacement_and_parameter_drift(self):
        def described(changes, status='CREATE_COMPLETE'):
            return {'StackName': 'GenuiSyntheticFoundation', 'Status': status, 'ExecutionStatus': 'AVAILABLE',
                    'Parameters': [{'ParameterKey': 'ImageDigest', 'ParameterValue': 'd'}, {'ParameterKey': 'BootstrapVersion', 'ParameterValue': '/cdk'}],
                    'Changes': [{'Type': 'Resource', 'ResourceChange': c} for c in changes]}
        receipt = self.root / 'receipt.json'; receipt.write_text(json.dumps({'cloudformation_parameters': {'ImageDigest': 'd'}}))
        path = self.root / 'describe.json'
        benign = [{'Action': 'Modify', 'ResourceType': 'AWS::ECS::TaskDefinition', 'LogicalResourceId': 'Task', 'Replacement': 'True'},
                  {'Action': 'Add', 'ResourceType': 'AWS::ECS::TaskDefinition', 'LogicalResourceId': 'OperatorTask'},
                  {'Action': 'Modify', 'ResourceType': 'AWS::ECS::Service', 'LogicalResourceId': 'Service', 'Replacement': 'False'}]
        path.write_text(json.dumps(described(benign)))
        self.assertEqual(run('review-change-set', '--describe', str(path), '--receipt', str(receipt))[0], 0)
        for bad in ({'Action': 'Modify', 'ResourceType': 'AWS::RDS::DBInstance', 'LogicalResourceId': 'Database', 'Replacement': 'Conditional'},
                    {'Action': 'Modify', 'ResourceType': 'AWS::ElasticLoadBalancingV2::LoadBalancer', 'LogicalResourceId': 'LoadBalancer', 'Replacement': 'True'},
                    {'Action': 'Remove', 'ResourceType': 'AWS::KMS::Key', 'LogicalResourceId': 'northstarKey'},
                    {'Action': 'Modify', 'ResourceType': 'AWS::ECS::Service', 'LogicalResourceId': 'Service', 'Replacement': 'True'}):
            path.write_text(json.dumps(described(benign + [bad])))
            self.assertEqual(run('review-change-set', '--describe', str(path))[0], 1, bad)
        path.write_text(json.dumps(described(benign, status='FAILED')))
        self.assertEqual(run('review-change-set', '--describe', str(path))[0], 1)
        receipt.write_text(json.dumps({'cloudformation_parameters': {'ImageDigest': 'other'}}))
        path.write_text(json.dumps(described(benign)))
        self.assertEqual(run('review-change-set', '--describe', str(path), '--receipt', str(receipt))[0], 1)


if __name__ == '__main__':
    unittest.main()
