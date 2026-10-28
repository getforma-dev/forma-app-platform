#!/usr/bin/env python3
"""Reproduce one published-registry canvas candidate, bounded to two Rust jobs."""
import hashlib
import json
import os
import platform
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
EVIDENCE = ROOT / 'evidence/APP-003'
EVIDENCE.mkdir(parents=True, exist_ok=True)
report = {'scope': 'APP-003 consumer preparation, not upstream acceptance', 'environment': {'platform': platform.platform()}, 'steps': [], 'passed': False}
def run(command, cwd=HERE, timeout=300):
    result = subprocess.run(command, cwd=cwd, env={**os.environ, 'CARGO_BUILD_JOBS':'2'}, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
    report['steps'].append({'command':command,'cwd':str(cwd.relative_to(ROOT)),'exit':result.returncode,'output':result.stdout})
    print('PASS' if result.returncode==0 else 'FAIL', ' '.join(command), flush=True)
    if result.returncode: raise RuntimeError(result.stdout[-4000:])
    return result.stdout
try:
    for tool in [['node','--version'],['npm','--version'],['rustc','--version'],['cargo','--version']]: run(tool)
    run(['npm','ci'],HERE/'admin')
    resolved=json.loads(run(['npm','ls','--json'],HERE/'admin'))
    assert all(resolved['dependencies'][name]['version']=='3.2.0' for name in ['@getforma/core','@getforma/build','@getforma/compiler'])
    (EVIDENCE/'npm-resolved.json').write_text(json.dumps(resolved,indent=2)+'\n')
    run(['npm','run','typecheck'],HERE/'admin')
    run(['npm','run','build'],HERE/'admin')
    for filename in ['size-report.json','manifest.json']:
        (EVIDENCE/filename).write_bytes((HERE/'admin/dist'/filename).read_bytes())
    sizes=json.loads((EVIDENCE/'size-report.json').read_text()); route=sizes['routes'][0]
    disposition={'app_forma_target':28100,'published_forma_cap':28420,'canvas_total_cap':373760,'estimated_forma_gzip':route['forma']['gzip'],'javascript_gzip_including_worker':route['javascript']['gzip'],'router_gzip':route['router']['gzip'],'class':route['class'],'canvas':route['canvas'],'app_target_fits':route['forma']['gzip']<=28100,'published_caps_fit':not route['over'],'override_used':False}
    (EVIDENCE/'budget.json').write_text(json.dumps(disposition,indent=2)+'\n')
    assert disposition['app_target_fits'] and disposition['published_caps_fit'], 'budget blocker; no override allowed'
    run(['node','refusals.mjs'],HERE/'admin')
    run(['cargo','fmt','--check'])
    run(['cargo','test','--locked'])
    run(['cargo','build','--release','--locked'])
    out=run(['target/release/genui-canvas-spike','--bench'])
    render=json.loads(next(line.removeprefix('BENCH_JSON ') for line in out.splitlines() if line.startswith('BENCH_JSON ')))
    assert render['over_cap']['code']=='RENDER-502'
    (EVIDENCE/'render.json').write_text(json.dumps(render,indent=2)+'\n')
    run(['node','verify_browser.mjs'],timeout=300)
    report['passed']=True
finally:
    (EVIDENCE/'qualification.json').write_text(json.dumps(report,indent=2)+'\n')
    source_files=[p for p in HERE.rglob('*') if p.is_file() and not any(s in p.relative_to(HERE).parts for s in ['node_modules','target','dist','.cache','__pycache__'])]
    source_files.append(ROOT/'.github/workflows/canvas.yml')
    identity={'source_sha256':{str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(source_files)},'base_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'qualification_passed':report['passed']}
    (EVIDENCE/'identity.json').write_text(json.dumps(identity,indent=2)+'\n')
