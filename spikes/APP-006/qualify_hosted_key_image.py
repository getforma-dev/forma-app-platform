#!/usr/bin/env python3
"""Check the actual one-shot image with no network, no secrets and a read-only root."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time
import uuid


def command(args, **kwargs):
    return subprocess.run(args, capture_output=True, timeout=30, check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--image', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        raise SystemExit('evidence output already exists')
    image = json.loads(command(['docker', 'image', 'inspect', args.image]).stdout)[0]
    assert image['Config']['User'] == '65532:65532'
    assert image['Config']['Entrypoint'] == ['/usr/local/bin/qualify-hosted-keys']
    assert not image['Config'].get('ExposedPorts')
    root = Path(__file__).resolve().parent
    source = {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
              for path in [root / 'Dockerfile', root / '.dockerignore', root / 'Cargo.toml',
                           root / 'Cargo.lock', root / 'src/lib.rs',
                           root / 'examples/hosted_key_qualification.rs', Path(__file__).resolve()]}
    report = {'schema_version': 1, 'passed': False,
              'scope': 'actual nonroot Linux image boundary negatives; no hosted issuer or token',
              'image_id': image['Id'], 'architecture': image['Architecture'], 'os': image['Os'],
              'declared_source_sha256': source, 'checks': []}
    try:
        for case, payload, expected in [
            ('empty-input', b'', 'input_size'),
            ('malformed-private-input', b'PRIVATE-IMAGE-MARKER', 'input_schema'),
            ('oversized-input', b'x' * 16385, 'input_size'),
        ]:
            name = 'genui-auth-key-boundary-' + uuid.uuid4().hex[:12]
            created = False
            row = {'case': case, 'passed': False, 'container_removed': False}
            report['checks'].append(row)
            try:
                command(['docker', 'create', '--name', name, '--network', 'none', '--read-only',
                         '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges',
                         '--pids-limit', '16', '--memory', '128m', '--cpus', '1',
                         '--interactive', image['Id']])
                created = True
                inspected = json.loads(command(['docker', 'inspect', name]).stdout)[0]
                host = inspected['HostConfig']
                assert inspected['Image'] == image['Id']
                assert host['NetworkMode'] == 'none' and host['ReadonlyRootfs'] is True
                assert host['CapDrop'] == ['ALL'] and 'no-new-privileges' in host['SecurityOpt']
                assert not inspected['Mounts']
                started = time.monotonic()
                result = subprocess.run(['docker', 'start', '--attach', '--interactive', name],
                                        input=payload, capture_output=True, timeout=8)
                observed = json.loads(result.stdout)
                state = json.loads(command(['docker', 'inspect', name]).stdout)[0]['State']
                assert result.returncode == 1 and state['ExitCode'] == 1
                assert state['Running'] is False and state['OOMKilled'] is False
                assert observed['passed'] is False and observed['failure_code'] == expected
                assert observed['checks'] == [] and not result.stderr
                assert b'PRIVATE-IMAGE-MARKER' not in result.stdout
                assert observed['compiled_verifier_source_sha256'] == source['src/lib.rs']
                assert observed['compiled_campaign_source_sha256'] == source['examples/hosted_key_qualification.rs']
                row.update(passed=True, failure_code=expected,
                           elapsed_seconds=round(time.monotonic() - started, 3),
                           process_os=observed['os'], process_architecture=observed['architecture'])
            finally:
                if created:
                    command(['docker', 'rm', '--force', '--volumes', name])
                    row['container_removed'] = True
        report['passed'] = all(row['passed'] and row['container_removed'] for row in report['checks'])
    finally:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'passed': report['passed'], 'cases': len(report['checks']),
                      'image_id': image['Id']}))


if __name__ == '__main__':
    main()
