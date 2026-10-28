#!/usr/bin/env python3
"""Real native HTTP route/disabled-default/privacy check; no provider requests."""
import argparse
import base64
import json
import os
from pathlib import Path
import queue
import secrets
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request
import uuid


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def capability():
    return base64.urlsafe_b64encode(secrets.token_bytes(32)).decode().rstrip('=')


def run(binary, directory=None):
    reports = []
    with tempfile.TemporaryDirectory(prefix='genui-auth006-', dir=directory) as private:
        path = Path(private)
        path.chmod(0o700)
        current = int(time.time())
        runtime = {
            'schema_version': 1, 'generation': str(uuid.uuid4()),
            'issued_at': current, 'expires_at': current + 500,
            'database': {'host': 'synthetic.invalid', 'port': 5432, 'name': 'genui',
                         'roles': {name: {'username': 'genui_' + name, 'token': 'unit-only-' + name,
                                          'expires_at': current + 500}
                                   for name in ('workspace', 'control', 'data', 'worker')}},
            'storage': {},
        }
        qualification = {
            'run_id': 'synthetic-native-http',
            'app_source_revision': '0' * 40,
            'provider_source_revision': 'dfba787295b880816fd95295b507a4527c51e62f',
            'expires_at': current + 500, 'max_attempts': 1, 'max_flows': 1,
            'synthetic_tenant': True, 'auth_origin': 'https://auth-staging.getforma.dev',
            'tenant_slug': 'synthetic', 'tenant_id': 'ten_00000000-0000-4000-8000-000000000001',
            'client_id': 'synthetic-client', 'issuer': 'synthetic-issuer',
            'audience': 'synthetic-audience', 'max_access_lifetime_seconds': 900,
            'expected_email': 'callback@example.test', 'expected_subject': None,
            'expected_email_verified': False, 'operator_token': capability(),
            'invite_token': 'synthetic-confidential-invitation',
        }
        for name, value in [('runtime.json', runtime), ('qualification.json', qualification)]:
            file = path / name
            file.write_text(json.dumps(value))
            file.chmod(0o600)
        environment = {k: v for k, v in os.environ.items()
                       if not k.startswith(('AWS_', 'GENUI_')) and k not in ('RUST_LOG', 'HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY')}
        environment.update({'GENUI_RUNTIME_CONFIG_FILE': str(path / 'runtime.json'),
                            'GENUI_HELPER_PATH': '/deliberately-absent-managed-probe.py',
                            'AWS_EC2_METADATA_DISABLED': 'true', 'FORMA_PORT': '0',
                            'FORMA_BIND_ADDRESS': '127.0.0.1'})
        opener = urllib.request.build_opener(NoRedirect(), urllib.request.ProxyHandler({}))
        for enabled in (False, True, True):
            env = dict(environment)
            if enabled:
                env['GENUI_AUTH_QUALIFICATION_CONFIG_FILE'] = str(path / 'qualification.json')
            process = subprocess.Popen([binary], env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            output, messages = [], queue.Queue()

            def collect():
                for line in process.stdout:
                    output.append(line)
                    messages.put(line.strip())
                messages.put(None)

            thread = threading.Thread(target=collect, daemon=True)
            thread.start()
            supplied_code, supplied_state = capability(), capability()
            try:
                deadline = time.monotonic() + 15
                while True:
                    line = messages.get(timeout=max(0.01, deadline - time.monotonic()))
                    assert line is not None, 'native server exited before listener'
                    if line.startswith('FORMA_LISTENING http://127.0.0.1:'):
                        origin = line.split(' ', 1)[1]
                        break
                    assert time.monotonic() < deadline

                def request(route, method='GET', data=None, headers=None):
                    req = urllib.request.Request(origin + route, method=method, data=data, headers=headers or {})
                    try:
                        response = opener.open(req, timeout=3)
                    except urllib.error.HTTPError as error:
                        response = error
                    with response:
                        return response.code, response.headers, response.read()

                status, headers, body = request('/auth/callback?code=' + supplied_code + '&state=' + supplied_state)
                assert status == (303 if enabled else 404)
                if enabled:
                    assert headers['Location'] == '/auth/qualification/result'
                    assert headers['Referrer-Policy'] == 'no-referrer'
                    assert headers['Cache-Control'] == 'no-store'
                    assert 'Max-Age=0' in headers['Set-Cookie']
                    assert request('/auth/qualification/start', 'POST', b'')[0] == 403
                    operator = {'Authorization': 'Bearer ' + qualification['operator_token']}
                    # Rejected before PAR: no real or simulated provider request.
                    assert request('/auth/qualification/start', 'POST', b'{"tenant":"foreign"}', operator)[0] == 403
                    receipt_status, _, receipt_body = request('/auth/qualification/status', headers=operator)
                    assert receipt_status == 200
                    receipt = json.loads(receipt_body)
                    assert receipt['counts']['starts'] == receipt['counts']['callback_exchanges'] == 0
                    assert receipt['counts']['identity_checked'] == receipt['pending_flows'] == 0
                    assert receipt['workspace_access_granted'] is False
                    assert receipt['provider_revocation_proved'] is False
                    assert request('/auth/qualification/status')[0] == 403
                    assert b'No application access was granted' in request('/auth/qualification/result')[2]
                for route in ('/workspace', '/api/query', '/api/import', '/api/generate', '/api/export'):
                    assert request(route)[0] == 404
                assert request('/health/live')[0] == 200
                assert request('/health/ready')[0] == 503
                for value in (supplied_code, supplied_state, qualification['operator_token'], qualification['invite_token']):
                    assert value.encode() not in body
                    assert value not in str(headers)
                reports.append({'enabled': enabled, 'callback_status': status,
                                'protected_routes': 404, 'provider_requests': 0,
                                'managed_readiness_without_probe': 503})
            finally:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
                thread.join(timeout=2)
            for value in (supplied_code, supplied_state, qualification['operator_token'], qualification['invite_token']):
                assert value not in ''.join(output), 'native output disclosed synthetic capability'
    return {'scope': 'local native HTTP routing, default-off and privacy; no hosted identity or mailbox evidence',
            'runs': reports, 'enabled_process_restarted': True, 'private_files_removed': True,
            'synthetic_capabilities_in_native_output': False, 'passed': True}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    parser.add_argument('--directory')
    args = parser.parse_args()
    print(json.dumps(run(args.binary, args.directory), sort_keys=True))
