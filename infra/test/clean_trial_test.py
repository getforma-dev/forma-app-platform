"""Local request/budget qualification. Synthetic identifiers; no AWS/provider IO."""
import argparse
import copy
from datetime import timedelta
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest

from preflight_test import always_on, WINDOW, preflight

ROOT = Path(__file__).parents[2]
spec = importlib.util.spec_from_file_location('clean_trial_requests', ROOT / 'infra/scripts/window_requests.py')
requests = importlib.util.module_from_spec(spec); spec.loader.exec_module(requests)
TRIAL = {'organization_id': '00000000-0000-4000-8000-0000000000e1', 'tenant_slug': 'forma-trial', 'provider_tenant_id': 'ten_trial_test'}
MANAGEMENT = 'arn:aws:secretsmanager:us-east-1:877573132739:secret:fauth-beta/management/trial-AbCdEf'


class CleanTrialRequests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name); self.root.chmod(0o700)
        self.trial = self.root / 'trial.json'; self.trial.write_text(json.dumps(TRIAL))
        self.identifiers = self.root / 'identifiers.json'
        self.ids = {'issuer_origin': 'https://auth.example.test', 'jwt_iss': 'https://auth.example.test', 'jwt_aud': 'genui',
                    'access_jwks': 'https://auth.example.test/auth/jwks', 'callback': 'https://beta.getforma.dev/auth/callback',
                    'shared_callback': 'https://beta.getforma.dev/auth/shared/callback',
                    'tenants': [{'slug': TRIAL['tenant_slug'], 'tenant_id': TRIAL['provider_tenant_id'], 'client_id': 'trial-client', 'shared_client_id': 'trial-shared-client', 'genui_org_uuid': TRIAL['organization_id']}]}
        self.identifiers.write_text(json.dumps(self.ids))
    def tearDown(self):
        self.directory.cleanup()
    def args(self, **extra):
        return argparse.Namespace(clean_trial=self.trial, **extra)
    def test_only_reviewed_fresh_org_can_mint_and_auth_has_both_callbacks(self):
        args = self.args(organization=TRIAL['organization_id'], label='owner', out_dir=self.root)
        result = requests.mint_code(args)
        self.assertEqual(result['organization_id'], TRIAL['organization_id'])
        self.assertNotIn('code', result)
        args.organization = requests.ORGANIZATIONS[0]
        with self.assertRaises(requests.Refused): requests.mint_code(args)
        auth = self.args(identifiers=self.identifiers, output=self.root/'auth.json', app_origin='https://beta.getforma.dev', max_access_lifetime_seconds=900)
        requests.auth_configuration(auth)
        document = json.loads(auth.output.read_text())
        self.assertEqual([x['organization_id'] for x in document['auth_config']['organizations']], [TRIAL['organization_id']])
        self.assertEqual(document['auth_config']['organizations'][0]['shared_client_id'], 'trial-shared-client')
        self.assertEqual(os.stat(auth.output).st_mode & 0o777, 0o600)
        auth.output.unlink()
        for change in ({'shared_callback': 'https://beta.getforma.dev/auth/callback'}, {'tenants': self.ids['tenants'] * 2},
                       {'tenants': [{k:v for k,v in self.ids['tenants'][0].items() if k != 'shared_client_id'}]},
                       {'tenants': [dict(self.ids['tenants'][0], shared_client_id='trial-client')]}):
            self.identifiers.write_text(json.dumps(dict(self.ids, **change)))
            with self.assertRaises(requests.Refused): requests.auth_configuration(auth)
            self.assertFalse(auth.output.exists())
    def test_broker_pair_is_private_same_token_bound_to_stack_and_never_silently_rotated(self):
        outputs = {'CleanTrialOrganization': TRIAL['organization_id'], 'CleanTrialProviderTenant': TRIAL['provider_tenant_id'],
                   'CleanTrialProviderSlug': TRIAL['tenant_slug'], 'SharedBrokerStateSecretArn': 'arn:aws:secretsmanager:us-east-1:877573132739:secret:broker-state-AbCdEf'}
        output_file = self.root/'outputs.json'; output_file.write_text(json.dumps({'Outputs': [{'OutputKey': k, 'OutputValue': v} for k,v in outputs.items()]}))
        args = self.args(outputs=output_file, out_dir=self.root, app_origin='https://beta.getforma.dev')
        result = requests.shared_broker_configuration(args)
        client = json.loads((self.root/'shared-broker-client.json').read_text())
        server = json.loads((self.root/'shared-broker-server.json').read_text())
        self.assertEqual(client['token'], server['client_token']); self.assertEqual(len(client['token']), 43)
        self.assertNotIn(client['token'], json.dumps(result)); self.assertEqual(server['port'], 3001)
        for path in result['private_files']: self.assertEqual(os.stat(path).st_mode & 0o777, 0o600)
        with self.assertRaises(requests.Refused): requests.shared_broker_configuration(args)
        self.assertEqual(json.loads((self.root/'shared-broker-client.json').read_text()), client)
    def test_clean_inputs_refuse_unknown_and_fixture_identity(self):
        for change in ({'organization_id': requests.ORGANIZATIONS[0]}, {'tenant_slug': 'canary'}, {'provider_tenant_id': 'other'}, {'token': 'forbidden'}):
            self.trial.write_text(json.dumps(dict(TRIAL, **change)))
            with self.assertRaises(requests.Refused): requests.clean_trial(self.args())


class CleanTrialBudget(unittest.TestCase):
    def configuration(self):
        config, ledger = always_on()
        config.update(clean_trial_organization_id=TRIAL['organization_id'], provider_management_secret_arn=MANAGEMENT, provider_management_url='https://auth-staging.getforma.dev')
        return config, ledger
    def test_broker_compute_and_retained_resources_are_explicit_and_supersede_at_correct_rate(self):
        config, ledger = self.configuration()
        baseline, baseline_report = preflight.reserve({k:v for k,v in config.items() if k != 'clean_trial_organization_id'}, ledger, WINDOW)
        updated, report = preflight.reserve(config, ledger, WINDOW)
        first = report['cost_by_month']['2026-11']; old = baseline_report['cost_by_month']['2026-11']
        self.assertEqual(first['retained_core_full_month_cents'] - old['retained_core_full_month_cents'], 360)
        self.assertGreater(first['all_window_compute_reservations_cents'], old['all_window_compute_reservations_cents'])
        self.assertEqual(report['cloudformation_parameters']['CleanTrialOrganizationId'], TRIAL['organization_id'])
        record = updated['windows'][config['window_id']]
        self.assertEqual(record['clean_trial_organization_id'], TRIAL['organization_id'])
        future = WINDOW + timedelta(hours=1)
        remaining = preflight.supersede(record, future)['2026-11']
        self.assertLess(remaining, 100, 'only elapsed service+broker and the standalone task allowance remain')
        config2 = dict(config, window_id='00000000-0000-4000-8000-0000000000e2', supersedes_window_id=config['window_id'])
        after, second = preflight.reserve(config2, updated, future)
        self.assertEqual(second['cost_by_month']['2026-11']['retained_core_full_month_cents'], first['retained_core_full_month_cents'])
        self.assertLess(second['cost_by_month']['2026-11']['genui_total_cents'], 21000)
    def test_fresh_organization_management_binding_and_budget_are_required(self):
        for change in ({'clean_trial_organization_id': requests.ORGANIZATIONS[0]}, {'clean_trial_organization_id': True}, {'provider_management_secret_arn':'', 'provider_management_url':''}):
            config, ledger = self.configuration(); config.update(change)
            with self.assertRaises(ValueError): preflight.reserve(config, ledger, WINDOW)
        config, ledger = self.configuration(); ledger['months']['2026-11']['prior_extra_spend_cents'] = 6000  # past the 21000-cent GenUI cap
        with self.assertRaisesRegex(ValueError, 'budget exceeded'): preflight.reserve(config, ledger, WINDOW)
