#!/usr/bin/env python3
"""Record local callback checks; optional local image only, never push/deploy."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
CONTEXT_INPUTS = [line[1:] for line in (ROOT / '.dockerignore').read_text().splitlines()
                  if line.startswith('!') and not line.endswith('/')]
assert all(not any(char in name for char in '*?[') for name in CONTEXT_INPUTS)
SOURCES = sorted(set(CONTEXT_INPUTS + ['app/README.md']))


def hashes():
    return {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in SOURCES}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--image-tag', help='Optional local image tag; never published')
    args = parser.parse_args()
    before = hashes()
    report = {
        'recorded_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'scope': 'local APP-006 callback and canonical verifier checks; no hosted acceptance',
        'source_sha256': before, 'source_is_uncommitted_candidate': True,
        'callback_identity_fixture': 'cfg(test) FixedIdentity; not integrated canonical cryptography',
        'canonical_verifier_checks': 'separate unchanged crate with real Ed25519 and loopback transport',
        'commands': [], 'passed': False,
    }
    destination = OUT / 'local-checks.json'
    environment = {**os.environ, 'CARGO_BUILD_JOBS': '2'}

    def execute(cwd, command, timeout=300, stdin=None):
        process = subprocess.run(command, cwd=ROOT / cwd, env=environment, input=stdin,
                                 capture_output=True, text=True, timeout=timeout)
        report['commands'].append({'cwd': cwd, 'command': command, 'exit_code': process.returncode,
                                   'stdout': process.stdout, 'stderr': process.stderr})
        destination.write_text(json.dumps(report, indent=2) + '\n')
        print(('PASS ' if process.returncode == 0 else 'FAIL ') + ' '.join(command), flush=True)
        if process.returncode:
            raise SystemExit(process.returncode)
        return process.stdout

    execute('app', ['cargo', '+1.93.1', 'fmt', '--all', '--check'])
    execute('app', ['cargo', '+1.93.1', 'clippy', '--locked', '--all-targets', '--', '-D', 'warnings'])
    execute('app', ['cargo', '+1.93.1', 'test', '--locked', '--', '--test-threads=2'])
    execute('app', ['cargo', '+1.93.1', 'build', '--locked'])
    execute('spikes/APP-006', ['cargo', '+1.93.1', 'test', '--locked', '--', '--test-threads=2'])
    output = execute('.', ['python3', str(OUT / 'http_probe.py'), '--binary', str(ROOT / 'app/target/debug/forma-app-platform')])
    report['native_http'] = json.loads(output)
    if args.image_tag:
        execute('.', ['docker', 'build', '--tag', args.image_tag, '--file', 'app/Dockerfile', '.'], timeout=1800)
        inspection = json.loads(execute('.', ['docker', 'image', 'inspect', args.image_tag]))[0]
        report['local_image'] = {'tag': args.image_tag, 'id': inspection['Id'],
                                 'architecture': inspection['Architecture'], 'os': inspection['Os'],
                                 'registry_publication': False}
        # The image's real binary is exercised on loopback in its own isolated
        # network namespace. Only synthetic temporary configuration is written.
        output = execute('.', ['docker', 'run', '--rm', '-i', '--network=none', '--read-only',
                               '--cap-drop=ALL', '--security-opt=no-new-privileges', '--cpus=2',
                               '--memory=1g', '--pids-limit=128',
                               '--tmpfs', '/tmp:rw,noexec,nosuid,size=16777216,mode=1777',
                               '--entrypoint', 'python3', args.image_tag,
                               '-', '--binary', '/app/genui', '--directory', '/tmp'],
                         stdin=(OUT / 'http_probe.py').read_text())
        report['image_http'] = json.loads(output)
    assert hashes() == before, 'source changed during qualification; requalify the changed candidate'
    execute('.', ['git', 'diff', '--check'])
    report['passed'] = True
    destination.write_text(json.dumps(report, indent=2) + '\n')
    print('Local evidence recorded; hosted gate remains open.', flush=True)


if __name__ == '__main__':
    main()
