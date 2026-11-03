#!/usr/bin/env python3
"""Local APP-007 scaffold checks; never provisions or contacts managed stores."""
import json, os, subprocess, shutil, sys, time
from pathlib import Path
root = Path(__file__).resolve().parents[1]
evidence = root/'evidence/APP-007'
python = shutil.which("python3.11") or sys.executable
venv = str(root / "app/runtime/.venv/bin/python")
# The runtime venv gets both hashed locks, as tenant-foundation.yml and
# `just setup` install them (ADR-012 D12.4): app/runtime/test_library.py
# imports contracts/validate.py, which needs jsonschema (APP-131).
commands = [
 ('.', [python,'-m','venv','app/runtime/.venv']),
 ('.', [venv,'-m','pip','install','--require-hashes','--only-binary=:all:','-r','app/runtime/requirements.lock','-r','contracts/requirements.lock']),
 ('.', [venv,'-m','unittest','discover','-s','app/runtime','-v']),
 ('app/admin', ['npm','ci']), ('app/admin',['npm','run','typecheck']), ('app/admin',['npm','test']), ('app/admin',['npm','run','build']),
 ('.',['cargo','fmt','--check','-p','forma-app-platform']), ('.',['cargo','test','--locked','-p','forma-app-platform']), ('.',['cargo','build','--locked','-p','forma-app-platform']),
 ('.',[venv,'app/runtime/smoke_http.py','--binary',str(root/'target/debug/forma-app-platform')]),
 ('infra',['npm','ci']), ('infra',['npm','run','typecheck']), ('infra',['npm','test']), ('infra',['npm','run','synth','--','--quiet']),
]

def run_checks(steps, working_root, output_directory):
    output_directory.mkdir(parents=True, exist_ok=True)
    reports = []
    for directory, args in steps:
        # The expanded TLS/database runtime suite takes ~227s locally. Keep
        # its CI budget bounded, without changing tests or other step budgets.
        timeout = 600 if args[1:] == ['-m', 'unittest', 'discover', '-s', 'app/runtime', '-v'] else 300
        # npm ci talks to the registry and can sit silent until the step budget.
        # One retry covers that stall. A second timeout still fails the check.
        attempts = 2 if args[:2] == ['npm', 'ci'] else 1
        for attempt in range(attempts):
            print('RUN: ' + ' '.join(args), flush=True)
            started = time.monotonic()
            timed_out = False
            try:
                result = subprocess.run(args, cwd=working_root/directory,
                                        env={**os.environ, 'CARGO_BUILD_JOBS': '2'},
                                        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                        text=True, timeout=timeout)
                code, output = result.returncode, result.stdout
            except subprocess.TimeoutExpired as error:
                # TimeoutExpired.output may be bytes even when text=True. Retain
                # the last named test instead of losing all buffered diagnostics.
                output = error.output or ''
                if isinstance(output, bytes):
                    output = output.decode('utf-8', errors='replace')
                code, timed_out = 124, True
            reports.append({'cwd': directory, 'command': args, 'exit': code, 'output': output,
                            'timeout_seconds': timeout, 'timed_out': timed_out, 'attempt': attempt + 1,
                            'elapsed_seconds': round(time.monotonic() - started, 3)})
            (output_directory/'local-preparation.json').write_text(json.dumps({
                'scope': 'Local scaffold only; managed acceptance pending', 'steps': reports}, indent=2)+'\n')
            print(('PASS' if code == 0 else 'FAIL') + ': ' + ' '.join(args), flush=True)
            if code == 0:
                break
            print(output, flush=True)
            if timed_out and attempt + 1 < attempts:
                print('Retrying npm ci once after the %d-second budget.' % timeout, flush=True)
                continue
            if timed_out:
                print('Step exceeded its %d-second budget; dependent steps were not run.' % timeout, flush=True)
            return code
    return 0


if __name__ == '__main__':
    raise SystemExit(run_checks(commands, root, evidence))
