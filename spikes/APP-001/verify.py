#!/usr/bin/env python3
"""Reproduce APP-001 without touching framework checkouts or paid services."""
import json
import os
import subprocess
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
APP = Path(__file__).resolve().parent/'minimal'
EVIDENCE = ROOT/'evidence/APP-001'
steps = [
    (APP/'admin', ['npm', 'ci']),
    (APP/'admin', ['npm', 'run', 'typecheck']),
    (APP/'admin', ['npm', 'run', 'build']),
    (APP, ['cargo', 'fmt', '--check']),
    (APP, ['cargo', 'test', '--locked']),
    (APP, ['cargo', 'build', '--locked']),
    (ROOT, ['python3', 'spikes/APP-001/verify_http.py']),
]
results = []
transcript = []
for cwd, command in steps:
    run = subprocess.run(command, cwd=cwd, env={**os.environ, 'CARGO_BUILD_JOBS': '3'},
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    results.append({'cwd': str(cwd.relative_to(ROOT)), 'command': command, 'exit': run.returncode})
    transcript.append('$ ' + ' '.join(command) + '\n' + run.stdout)
    print(f'{"PASS" if run.returncode == 0 else "FAIL"}: {" ".join(command)}', flush=True)
    (EVIDENCE/'checks.json').write_text(json.dumps({'at': datetime.now(timezone.utc).isoformat(),
                                                'steps': results}, indent=2)+'\n')
    (EVIDENCE/'checks.txt').write_text('\n'.join(transcript))
    if run.returncode:
        raise SystemExit(run.returncode)
