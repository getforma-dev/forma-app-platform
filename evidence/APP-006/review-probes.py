#!/usr/bin/env python3
"""Run focused independent probes in a disposable, byte-matched source copy."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SOURCE = ROOT / 'spikes/APP-006'
paths = ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'src/lib.rs', 'src/tests.rs', 'source-contract.json']
before = {name: hashlib.sha256((SOURCE/name).read_bytes()).hexdigest() for name in paths}
scratch = HERE / '.cache'
scratch.mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix='review-', dir=scratch) as location:
    work = Path(location)
    for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']:
        shutil.copy2(SOURCE/name,work/name)
    for name in ['src','fixtures']:
        shutil.copytree(SOURCE/name,work/name)
    assert hashlib.sha256((work/'src/lib.rs').read_bytes()).hexdigest() == before['src/lib.rs']
    tests = work/'src/tests.rs'
    tests.write_text(tests.read_text() + '\n' + (HERE/'review-probes.rs').read_text())
    command = ['cargo','test','--locked','review_','--','--test-threads=1','--nocapture']
    result = subprocess.run(command,cwd=work,env={**os.environ,'CARGO_BUILD_JOBS':'2','CARGO_TARGET_DIR':str(scratch/'target')},text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=180)
after = {name: hashlib.sha256((SOURCE/name).read_bytes()).hexdigest() for name in paths}
assert after == before, 'candidate changed during review'
report = {'scope':'independent supplemental local verifier probes; no implementation edits, hosted auth or credentials',
          'app_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
          'candidate_source_sha256':before,'command':command,'exit':result.returncode,'output':result.stdout,
          'candidate_unchanged':after==before,'passed':result.returncode==0}
(HERE/'review-probes.json').write_text(json.dumps(report,indent=2)+'\n')
print(result.stdout)
raise SystemExit(result.returncode)
