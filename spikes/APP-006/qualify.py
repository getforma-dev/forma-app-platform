#!/usr/bin/env python3
"""Run the local mock-only consumer qualification and retain exact source identity."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time
import tomllib
from verify_source import ROOT, verify

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--upstream-checkout', type=Path)
args = parser.parse_args()
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=True)
for previous in ('01-fmt.log', '02-clippy.log', '03-test.log', '04-build.log', '05-private-pipe.log', 'qualification.json'):
    (output / previous).unlink(missing_ok=True)
repo = ROOT.parents[1]
jobs = os.environ.get('CARGO_BUILD_JOBS', '2')
if jobs not in {'1', '2'}:
    raise SystemExit('CARGO_BUILD_JOBS must be 1 or 2')
env = dict(os.environ, CARGO_BUILD_JOBS=jobs)
inputs = sorted(p for p in ROOT.rglob('*') if p.is_file() and not any(x in {'target', '__pycache__'} for x in p.relative_to(ROOT).parts))
workflow = repo / '.github/workflows/auth-contract.yml'
if workflow.exists():
    inputs.append(workflow)
identity = {str(p.relative_to(repo)): hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs}
start = datetime.datetime.now(datetime.timezone.utc).isoformat()
record = {'started_utc': start, 'scope': 'local 247 and 248 access-only consumer with ephemeral keys and loopback mock HTTP; no hosted auth, app membership, KMS custody or live rotation acceptance',
          'app_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(),
          'source_sha256': identity, 'platform': platform.platform(),
          'rustc': subprocess.check_output(['rustc', '-vV'], cwd=ROOT, text=True).strip(),
          'source_contract': verify(args.upstream_checkout), 'checks': []}
lock = tomllib.loads((ROOT / 'Cargo.lock').read_text())
checksums = {(p['name'],p['version']): p.get('checksum') for p in lock['package']}
metadata = json.loads(subprocess.check_output(['cargo','metadata','--locked','--format-version','1'], cwd=ROOT, env=env))
record['registry_dependencies'] = [{'name':p['name'],'version':p['version'],'license':p['license'],'repository':p['repository'],'checksum':checksums.get((p['name'],p['version']))} for p in metadata['packages'] if p['source'] is not None]
commands = [('fmt', ['cargo','fmt','--all','--check']),
            ('clippy', ['cargo','clippy','--locked','--all-targets','--','-D','warnings']),
            ('test', ['cargo','test','--locked','--all-targets','--','--test-threads=2','--nocapture']),
            ('build', ['cargo','build','--locked','--example','hosted_key_qualification']),
            ('private-pipe', ['python3','qualify_hosted_key_boundary.py','--executable','target/debug/examples/hosted_key_qualification'])]
for index, (name, command) in enumerate(commands, 1):
    started = time.monotonic()
    result = subprocess.run(command, cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    filename = f'{index:02d}-{name}.log'
    (output / filename).write_text(result.stdout)
    record['checks'].append({'command':command,'cwd':'spikes/APP-006','exit_code':result.returncode,'elapsed_seconds':round(time.monotonic()-started,3),'output':filename})
    print(f'{name}: exit={result.returncode}', flush=True)
    if result.returncode:
        break
record['passed'] = len(record['checks']) == len(commands) and all(c['exit_code']==0 for c in record['checks'])
record['finished_utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
(output / 'qualification.json').write_text(json.dumps(record,indent=2)+'\n')
raise SystemExit(0 if record['passed'] else 1)
