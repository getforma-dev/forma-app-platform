#!/usr/bin/env python3
"""Real native HTTP route/harness-retirement/privacy check; no provider requests.

APP-009 (ADR-006 §retirement, ADR-007 D7.13): the release binary owns
/auth/callback through the product `auth` module and refuses the operator
qualification configuration at startup; the harness routes are absent. With no
provider configuration a callback is the uniform neutral sign-in failure (400)
and protected routes are the uniform 404 problem. The APP-006 harness itself
now compiles only with the `qualification-harness` feature.
"""
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


UNIFORM_404 = b'{"code":"not_found"}'


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
        secrets_in_play = (qualification['operator_token'], qualification['invite_token'])

        # D7.13: the release binary refuses the harness configuration outright.
        env = dict(environment, GENUI_AUTH_QUALIFICATION_CONFIG_FILE=str(path / 'qualification.json'))
        refused = subprocess.run([binary], env=env, capture_output=True, text=True, timeout=15)
        assert refused.returncode != 0 and 'FORMA_LISTENING' not in refused.stdout
        assert 'not part of this binary' in refused.stderr
        for value in secrets_in_play:
            assert value not in refused.stdout + refused.stderr
        reports.append({'harness_configuration': 'refused at startup', 'exit_code': refused.returncode,
                        'listener_bound': False})

        for restart in (1, 2):
            process = subprocess.Popen([binary], env=environment, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
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

                cookie = {'Cookie': '__Host-genui_prelogin=' + capability() + capability()[:21]}
                status, headers, body = request('/auth/callback?code=' + supplied_code + '&state=' + supplied_state, headers=cookie)
                # The product callback: uniform neutral failure, no session, no provider call.
                assert status == 400 and headers.get('Location') is None
                assert headers['Cache-Control'] == 'no-store' or 'no-store' in headers['Cache-Control']
                assert '__Host-genui_prelogin=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0' in headers.get_all('Set-Cookie')
                assert b'Sign-in could not continue' in body
                for route, method in (('/auth/qualification/start', 'POST'), ('/auth/qualification/status', 'GET'),
                                      ('/auth/qualification/result', 'GET')):
                    code, hdrs, payload = request(route, method, b'' if method == 'POST' else None,
                                                  {'Sec-Fetch-Site': 'same-origin'})
                    assert (code, payload) == (404, UNIFORM_404) and hdrs['Content-Type'] == 'application/problem+json'
                for route in ('/workspace', '/auth/pending', '/api/query', '/api/import', '/api/generate', '/api/export', '/v1/anything'):
                    code, hdrs, payload = request(route)
                    assert (code, payload) == (404, UNIFORM_404), route
                    assert hdrs['Cache-Control'] == 'no-store' and hdrs['X-Content-Type-Options'] == 'nosniff'
                assert request('/auth/signout', 'POST', b'', {'Sec-Fetch-Site': 'cross-site'})[0] == 403
                signin_status, _, signin_body = request('/signin')
                assert signin_status == 200 and b'Sign in' in signin_body and b'<script src' not in signin_body
                assert request('/health/live')[0] == 200
                assert request('/health/ready')[0] == 503
                for value in (supplied_code, supplied_state) + secrets_in_play:
                    assert value.encode() not in body
                    assert value not in str(headers)
                reports.append({'run': restart, 'harness_routes': 404, 'callback_status': status,
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
            for value in (supplied_code, supplied_state) + secrets_in_play:
                assert value not in ''.join(output), 'native output disclosed synthetic capability'
    return {'scope': 'local native HTTP routing, harness retirement and privacy; no hosted identity or mailbox evidence',
            'runs': reports, 'process_restarted': True, 'private_files_removed': True,
            'synthetic_capabilities_in_native_output': False, 'passed': True}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    parser.add_argument('--directory')
    args = parser.parse_args()
    print(json.dumps(run(args.binary, args.directory), sort_keys=True))
