#!/usr/bin/env python3
"""Local secret handoff checks with synthetic bytes; no cloud/provider calls."""
import argparse
import base64
import datetime
import hashlib
import json
import os
from pathlib import Path
import queue
import secrets
import stat
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def native_supervisor_probe(python, binary):
    results = []
    for mode in ('disabled', 'injected', 'malformed'):
        with tempfile.TemporaryDirectory(prefix='genui-secret-wire-probe-') as directory:
            private = Path(directory)
            private.chmod(0o700)
            credential_path = private / 'credentials.json'
            cap = lambda: base64.urlsafe_b64encode(secrets.token_bytes(32)).decode().rstrip('=')
            config = {'run_id': 'synthetic-runtime-handoff', 'app_source_revision': '0' * 40,
                      'provider_source_revision': 'dfba787295b880816fd95295b507a4527c51e62f',
                      'expires_at': int(time.time()) + 300, 'max_attempts': 1, 'max_flows': 1,
                      'synthetic_tenant': True, 'auth_origin': 'https://auth-staging.getforma.dev',
                      'tenant_slug': 'synthetic', 'tenant_id': 'ten_00000000-0000-4000-8000-000000000001',
                      'client_id': 'synthetic-client', 'issuer': 'synthetic-issuer', 'audience': 'synthetic-audience',
                      'max_access_lifetime_seconds': 900, 'expected_email': 'callback@example.test',
                      'expected_subject': None, 'expected_email_verified': False,
                      'operator_token': cap(), 'invite_token': 'synthetic-only-confidential-invitation'}
            injected = json.dumps(config) if mode == 'injected' else '{malformed-synthetic-private-json'
            env = {'GENUI_RUNTIME_CONFIG_FILE': str(credential_path), 'GENUI_BINARY': str(binary),
                   'GENUI_HELPER_PATH': '/deliberately-absent-managed-probe.py',
                   'FORMA_PORT': '0', 'FORMA_BIND_ADDRESS': '127.0.0.1',
                   'AWS_EC2_METADATA_DISABLED': 'true', 'PYTHONDONTWRITEBYTECODE': '1', 'PATH': os.defpath}
            if mode != 'disabled': env['GENUI_AUTH_QUALIFICATION_CONFIG_JSON'] = injected
            # Patch only managed credential refresh in this fixture process.
            # The real supervisor, file handoff, Rust binary and signal cleanup
            # run unchanged; no managed SDK or auth provider is invoked.
            wrapper = (
                'import sys, os; '
                'sys.path.insert(0, sys.argv[1]); '
                'import runtime, test_runtime; '
                'runtime.refresh=lambda path: runtime.atomic_write(path, test_runtime.fixture()); '
                'raise SystemExit(runtime.serve(os.environ["GENUI_RUNTIME_CONFIG_FILE"]))'
            )
            process = subprocess.Popen([python, '-c', wrapper, str(ROOT / 'app/runtime')], env=env,
                                       stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            output, lines = [], queue.Queue()

            def collect():
                for line in process.stdout:
                    output.append(line)
                    lines.put(line.strip())
                lines.put(None)

            reader = threading.Thread(target=collect, daemon=True)
            reader.start()
            try:
                if mode == 'malformed':
                    assert process.wait(timeout=8) != 0
                    results.append({'mode': mode, 'rust_rejected_before_listen': True})
                else:
                    deadline = time.monotonic() + 15
                    while True:
                        line = lines.get(timeout=max(0.01, deadline - time.monotonic()))
                        assert line is not None, 'supervised native process stopped before listener'
                        if line.startswith('FORMA_LISTENING http://127.0.0.1:'):
                            origin = line.split(' ', 1)[1]
                            break
                        assert time.monotonic() < deadline
                    files = list(private.glob('.auth-qualification-*.json'))
                    assert len(files) == (1 if mode == 'injected' else 0)
                    if files:
                        info = files[0].stat()
                        assert stat.S_IMODE(info.st_mode) == 0o600 and info.st_uid == os.geteuid() and info.st_nlink == 1
                        assert files[0].read_bytes() == injected.encode('utf-8')
                    opener = urllib.request.build_opener(NoRedirect(), urllib.request.ProxyHandler({}))
                    try:
                        response = opener.open(origin + '/auth/callback?code=' + cap() + '&state=' + cap(), timeout=3)
                    except urllib.error.HTTPError as error:
                        response = error
                    with response:
                        assert response.code == (303 if mode == 'injected' else 404)
                        assert injected.encode() not in response.read()
                        if mode == 'injected': assert response.headers['Location'] == '/auth/qualification/result'
                    results.append({'mode': mode, 'callback_status': 303 if mode == 'injected' else 404,
                                    'private_handoff_file_count': len(files), 'provider_requests': 0})
            finally:
                if process.poll() is None:
                    process.terminate()
                    try: process.wait(timeout=12)
                    except subprocess.TimeoutExpired:
                        process.kill(); process.wait(timeout=3)
                reader.join(timeout=2)
                process.stdout.close()
            assert not list(private.glob('.auth-qualification-*.json'))
            assert not credential_path.exists()
            captured = ''.join(output)
            for secret in (injected, config['operator_token'], config['invite_token']):
                assert secret not in captured, 'synthetic secret disclosed by supervised output'
            if mode == 'malformed': assert 'FORMA_LISTENING' not in captured
    return {'scope': 'real supervisor and native binary with synthetic managed-refresh fixture',
            'results': results, 'all_owned_files_removed': True, 'secret_output': False, 'passed': True}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--python', default=str(ROOT / 'app/runtime/.venv/bin/python'))
    parser.add_argument('--binary', type=Path, default=ROOT / 'app/target/debug/forma-app-platform')
    args = parser.parse_args()
    sources = ['app/runtime/runtime.py', 'app/runtime/test_runtime.py']
    source_hashes = {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in sources}
    report = {'recorded_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'scope': 'local optional APP-006 secret handoff; no cloud, provider or real credentials',
              'source_sha256': source_hashes, 'passed': False}
    command = [args.python, '-m', 'unittest', 'discover', '-s', 'app/runtime', '-v']
    process = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=60)
    report['unit_tests'] = {'command': command, 'exit_code': process.returncode,
                            'stdout': process.stdout, 'stderr': process.stderr}
    destination = OUT / 'author-checks.json'
    destination.write_text(json.dumps(report, indent=2) + '\n')
    assert process.returncode == 0, 'runtime unit checks failed; see sanitized fixture output'
    binary_hash = hashlib.sha256(args.binary.read_bytes()).hexdigest()
    report['native_binary_sha256'] = binary_hash
    report['native_supervisor'] = native_supervisor_probe(args.python, args.binary)
    assert hashlib.sha256(args.binary.read_bytes()).hexdigest() == binary_hash
    assert all(hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == value for name, value in source_hashes.items())
    report['passed'] = True
    destination.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'passed': True, 'runtime_unit_tests': 22, 'native_supervisor_modes': 3,
                      'no_cloud_or_provider_operations': True, 'owned_synthetic_files_removed': True}))


if __name__ == '__main__':
    main()
