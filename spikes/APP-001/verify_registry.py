#!/usr/bin/env python3
"""Verify downloaded published archives; retain metadata, never credentials."""
import base64
import hashlib
import json
import platform
import subprocess
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VERSION = '3.2.0'
COMMIT = 'fc76ea8531f0e3a85d0224087384e41f52040e3c'

def fetch(url):
    return subprocess.check_output(['curl', '--fail', '--silent', '--show-error',
                                    '--location', '--max-time', '90',
                                    '--user-agent', 'forma-app-platform-APP-001', url])

records = []
for name in ('core', 'compiler', 'build', 'router', 'create-app'):
    endpoint = f'https://registry.npmjs.org/@getforma%2F{name}/{VERSION}'
    meta = json.loads(fetch(endpoint))
    assert meta['version'] == VERSION
    raw = fetch(meta['dist']['tarball'])
    integrity = meta['dist']['integrity']
    algorithm, expected = integrity.split('-', 1)
    assert base64.b64encode(hashlib.new(algorithm, raw).digest()).decode() == expected
    records.append({'ecosystem': 'npm', 'package': meta['name'], 'version': VERSION,
                    'endpoint': endpoint, 'archive': meta['dist']['tarball'],
                    'integrity': integrity, 'sha256': hashlib.sha256(raw).hexdigest(),
                    'git_head': meta.get('gitHead'), 'engines': meta.get('engines'),
                    'role': 'optional; not installed' if name == 'router' else 'selected'})
for name in ('forma-ir', 'forma-ir-derive', 'forma-server'):
    endpoint = f'https://crates.io/api/v1/crates/{name}/{VERSION}'
    meta = json.loads(fetch(endpoint))['version']
    assert meta['num'] == VERSION and not meta['yanked']
    archive = f'https://static.crates.io/crates/{name}/{name}-{VERSION}.crate'
    raw = fetch(archive)
    actual = hashlib.sha256(raw).hexdigest()
    assert actual == meta['checksum']
    records.append({'ecosystem': 'crates.io', 'package': name, 'version': VERSION,
                    'endpoint': endpoint, 'archive': archive, 'sha256': actual,
                    'rust_version': meta.get('rust_version'), 'role': 'selected'})
npm_lock_path = ROOT/'spikes/APP-001/minimal/admin/package-lock.json'
cargo_lock_path = ROOT/'spikes/APP-001/minimal/Cargo.lock'
npm_lock = json.loads(npm_lock_path.read_text())['packages']
cargo_lock = cargo_lock_path.read_text()
for record in records:
    name = record['package']
    if name in ('@getforma/core', '@getforma/compiler', '@getforma/build'):
        entry = npm_lock['node_modules/'+name]
        assert entry['version'] == VERSION and entry['integrity'] == record['integrity']
    elif record['ecosystem'] == 'crates.io':
        entry = next(block for block in cargo_lock.split('[[package]]')
                     if f'name = "{name}"\n' in block)
        assert f'version = "{VERSION}"' in entry
        assert f'checksum = "{record["sha256"]}"' in entry
tag = subprocess.check_output(['git', 'ls-remote', 'https://github.com/getforma-dev/forma.git',
                               'refs/tags/v3.2.0^{}'], text=True).strip()
assert tag.split()[0] == COMMIT, tag
report = {'verified_at': datetime.now(timezone.utc).isoformat(), 'source_commit': COMMIT,
          'tag': 'v3.2.0', 'host': platform.platform(), 'archives': records,
          'lock_sha256': {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
                          for p in (npm_lock_path, cargo_lock_path)},
          'result': 'All eight downloaded archives match published registry checksums; tag identity verified.'}
output = ROOT/'evidence/APP-001/registry.json'
output.write_text(json.dumps(report, indent=2)+'\n')
print(report['result'])
