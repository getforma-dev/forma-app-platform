#!/usr/bin/env python3
"""Exercise real private-pipe process boundaries; never call a hosted issuer."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--executable', type=Path, required=True)
    args = parser.parse_args()
    executable = args.executable.resolve(strict=True)
    marker = 'DO-NOT-RETAIN-PRIVATE-BOUNDARY-MARKER'
    checks = []

    def check(name, expected, *, data=b'', regular=False, hold=False, arguments=()):
        started = time.monotonic()
        with tempfile.TemporaryFile() as private_file:
            if regular:
                private_file.write(marker.encode())
                private_file.seek(0)
            process = subprocess.Popen([str(executable), *arguments],
                                       stdin=private_file if regular else subprocess.PIPE,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            try:
                if hold:
                    process.stdin.write(data)
                    process.stdin.flush()
                    process.wait(timeout=6)
                    stdout, stderr = process.stdout.read(), process.stderr.read()
                    process.stdin.close()
                else:
                    stdout, stderr = process.communicate(None if regular else data, timeout=6)
            except BaseException:
                process.kill()
                process.communicate()
                raise
        elapsed = time.monotonic() - started
        report = json.loads(stdout)
        assert process.returncode == 1, (name, 'exit')
        assert report['passed'] is False, (name, 'passed')
        assert report['failure_code'] == expected, (name, 'failure_code')
        assert report['checks'] == [], (name, 'unexpected campaign activity')
        assert not stderr, (name, 'unexpected stderr')
        assert marker.encode() not in stdout + stderr, (name, 'private input disclosed')
        assert len(stdout) < 4096, (name, 'unbounded output')
        assert elapsed < 6, (name, 'process did not exit in bound')
        if expected == 'input_deadline':
            assert elapsed >= 1.8, (name, 'withheld EOF did not exercise timeout')
        checks.append({'case': name, 'passed': True, 'failure_code': expected,
                       'elapsed_seconds': round(elapsed, 3)})

    # Invalid envelopes are rejected before verifier construction or network.
    base = {'schema_version': 1, 'synthetic_tenant': True, 'run_id': 'local-boundary',
            'declared_consumer_revision': 'a' * 40, 'declared_provider_revision': 'b' * 40,
            'expected_tenant': 'ten_00000000-0000-4000-8000-000000000001',
            'expected_subject': 'usr_00000000-0000-4000-8000-000000000001',
            'issuer': 'gatewasm', 'audience': 'gatewasm-app',
            'jwks_url': 'https://auth-staging.getforma.dev/auth/jwks',
            'max_access_lifetime_seconds': 900, 'expires_at': int(time.time()) + 120,
            'access_token': marker}
    check('no-arguments', 'arguments_forbidden', arguments=('--token-forbidden',))
    check('regular-file-refused', 'input_requires_pipe', regular=True)
    check('empty-pipe', 'input_size')
    check('oversized-pipe', 'input_size', data=b'x' * 16385)
    check('oversized-without-eof', 'input_size', data=b'x' * 16385, hold=True)
    check('incomplete-without-eof', 'input_deadline', data=b'{', hold=True)
    check('malformed-private-input', 'input_schema', data=marker.encode())
    encoded = json.dumps(base).encode()
    check('duplicate-field', 'input_schema', data=b'{"schema_version":1,' + encoded[1:])
    check('unknown-field', 'input_schema', data=json.dumps(dict(base, clock_override=0)).encode())
    check('foreign-endpoint', 'input_contract',
          data=json.dumps(dict(base, jwks_url='http://127.0.0.1/auth/jwks')).encode())
    check('expired-envelope', 'input_contract', data=json.dumps(dict(base, expires_at=0)).encode())
    check('non-synthetic-envelope', 'input_contract',
          data=json.dumps(dict(base, synthetic_tenant=False)).encode())

    # Fill a real output pipe before launch and deliberately never drain it
    # while the child runs. The production 90s watchdog must terminate even
    # when reporting an empty-input refusal is blocked by output backpressure.
    read_fd, write_fd = os.pipe()
    filled = 0
    started = time.monotonic()
    try:
        os.set_blocking(write_fd, False)
        while True:
            try:
                filled += os.write(write_fd, b'X' * 4096)
            except BlockingIOError:
                break
        os.set_blocking(write_fd, True)
        process = subprocess.Popen([str(executable)], stdin=subprocess.PIPE,
                                   stdout=write_fd, stderr=subprocess.PIPE)
        os.close(write_fd)
        write_fd = -1
        try:
            _, stderr = process.communicate(b'', timeout=96)
        except BaseException:
            process.kill()
            process.communicate()
            raise
        elapsed = time.monotonic() - started
        chunks = []
        while chunk := os.read(read_fd, 65536):
            chunks.append(chunk)
        assert process.returncode == 124, 'blocked output escaped watchdog'
        assert 89 <= elapsed < 96, 'wrong watchdog bound'
        assert not stderr, 'watchdog disclosed stderr'
        assert b''.join(chunks) == b'X' * filled, 'unexpected partial report on full pipe'
        checks.append({'case': 'blocked-output-whole-process-deadline', 'passed': True,
                       'exit_code': 124, 'elapsed_seconds': round(elapsed, 3)})
    finally:
        os.close(read_fd)
        if write_fd >= 0:
            os.close(write_fd)
    print(json.dumps({'schema_version': 1, 'passed': True,
                      'scope': 'local real process/pipe boundary only; no hosted issuer or successful token',
                      'executable_sha256': hashlib.sha256(executable.read_bytes()).hexdigest(),
                      'checks': checks}, indent=2))


if __name__ == '__main__':
    main()
