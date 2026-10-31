#!/usr/bin/env python3
"""Local APP-007 scaffold checks; never provisions or contacts managed stores."""
import json, os, subprocess, shutil, sys
from pathlib import Path
root = Path(__file__).resolve().parents[1]
evidence = root/'evidence/APP-007'
python = shutil.which("python3.11") or sys.executable
venv = str(root / "app/runtime/.venv/bin/python")
commands = [
 ('.', [python,'-m','venv','app/runtime/.venv']),
 ('.', [venv,'-m','pip','install','--require-hashes','--only-binary=:all:','-r','app/runtime/requirements.lock']),
 ('.', [venv,'-m','unittest','discover','-s','app/runtime','-v']),
 ('app/admin', ['npm','ci']), ('app/admin',['npm','run','typecheck']), ('app/admin',['npm','run','build']),
 ('.',['cargo','fmt','--check','-p','forma-app-platform']), ('.',['cargo','test','--locked','-p','forma-app-platform']), ('.',['cargo','build','--locked','-p','forma-app-platform']),
 ('.',[venv,'app/runtime/smoke_http.py','--binary',str(root/'target/debug/forma-app-platform')]),
 ('infra',['npm','ci']), ('infra',['npm','run','typecheck']), ('infra',['npm','test']), ('infra',['npm','run','synth','--','--quiet']),
]
evidence.mkdir(parents=True,exist_ok=True)
reports=[]
for directory, args in commands:
 p=subprocess.run(args,cwd=root/directory,env={**os.environ,'CARGO_BUILD_JOBS':'2'},stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=300)
 reports.append({'cwd':directory,'command':args,'exit':p.returncode,'output':p.stdout})
 (evidence/'local-preparation.json').write_text(json.dumps({'scope':'Local scaffold only; managed acceptance pending','steps':reports},indent=2)+'\n')
 print(('PASS' if p.returncode==0 else 'FAIL')+': '+' '.join(args),flush=True)
 if p.returncode:raise SystemExit(p.returncode)
