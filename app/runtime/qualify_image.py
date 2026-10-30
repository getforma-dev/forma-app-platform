"""Local Docker image trial; creates/removes only its own container and volume."""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import subprocess
import uuid

parser=argparse.ArgumentParser();parser.add_argument('--image',required=True);parser.add_argument('--output',required=True,type=Path);args=parser.parse_args()
run_id=uuid.uuid4().hex[:10];volume='genui-app007-runtime-'+run_id;name='genui-app007-smoke-'+run_id;http_name=name+'-http';supervisor_name=name+'-supervisor'
controls=['--network=none','--read-only','--cap-drop=ALL','--security-opt=no-new-privileges','--cpus=2','--memory=1g','--memory-swap=1g','--pids-limit=64','--mount',f'type=volume,src={volume},dst=/run/genui','--entrypoint','python3']
code="import os,stat,json; from pathlib import Path; import runtime; p=os.stat('/run/genui'); assert os.getuid()==65532 and p.st_uid==65532 and stat.S_IMODE(p.st_mode)==0o700; f=Path('/run/genui/atomic-probe.json'); runtime.atomic_write(f,{'unit_only':True}); assert stat.S_IMODE(f.stat().st_mode)==0o600; f.unlink(); print(json.dumps({'uid':os.getuid(),'volume_mode':'0700','atomic_file_mode':'0600','passed':True}))"
report={'recorded_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'scope':'Local image/volume/HTTP security qualification; no managed service calls','passed':False}
try:
    cid=subprocess.check_output(['docker','create','--name',name,*controls,args.image,'-c',code],text=True).strip()
    inspection=json.loads(subprocess.check_output(['docker','inspect',cid],text=True))[0]
    first=subprocess.run(['docker','start','--attach',cid],capture_output=True,text=True,timeout=20)
    assert first.returncode==0,first.stderr
    state=json.loads(subprocess.check_output(['docker','inspect',cid],text=True))[0]['State'];assert state['ExitCode']==0
    report['ownership_and_atomic_write']=json.loads(first.stdout);report['state']=state
    host=inspection['HostConfig']
    report['controls']={'user':inspection['Config']['User'],'host':{k:host[k] for k in ['NetworkMode','ReadonlyRootfs','CapDrop','SecurityOpt','Memory','MemorySwap','NanoCpus','PidsLimit']},'mounts':[{'destination':m['Destination'],'read_write':m['RW'],'type':m['Type']} for m in inspection['Mounts']]}
    subprocess.run(['docker','rm',cid],check=True,capture_output=True)
    command=['docker','run','--rm','--name',http_name,'-i',*controls,args.image,'-','--binary','/app/genui','--directory','/run/genui']
    smoke=subprocess.run(command,input=(Path(__file__).parent/'smoke_http.py').read_text(),capture_output=True,text=True,timeout=30)
    assert smoke.returncode==0,smoke.stderr+smoke.stdout
    report['http_checks']=[json.loads(line) for line in smoke.stdout.splitlines()]
    command=['docker','run','--rm','--name',supervisor_name,'-i',*controls,args.image,'-','--binary','/app/genui','--directory','/run/genui']
    supervisor=subprocess.run(command,input=(Path(__file__).parent/'smoke_supervisor.py').read_text(),capture_output=True,text=True,timeout=60)
    assert supervisor.returncode==0,'synthetic supervisor image smoke refused'
    report['supervisor_checks']=json.loads(supervisor.stdout)
    assert report['supervisor_checks']['passed']
    # ADR-009 D9.1: the parser artifacts are present, root-owned, unwritable by the
    # runtime user and exactly the bytes parser-manifest.json names; startup accepts
    # that manifest and refuses a tampered copy.
    parser_code=("import hashlib,json,os,stat,subprocess; from pathlib import Path\n"
        "m=json.loads(Path('/app/parser-manifest.json').read_text()); assert os.environ['GENUI_PARSER_MANIFEST_FILE']=='/app/parser-manifest.json' and m['schema']=='genui-parser-manifest/1'\n"
        "out={}\n"
        "for k,path in (('guest','/app/parser/parser_guest.wasm'),('host','/app/parser/parser-host')):\n"
        " p=Path(path); st=p.lstat(); b=p.read_bytes()\n"
        " assert stat.S_ISREG(st.st_mode) and st.st_uid==0 and not st.st_mode&0o022 and not os.access(p,os.W_OK), path\n"
        " assert m[k]['path']==path and hashlib.sha256(b).hexdigest()==m[k]['sha256'] and len(b)==m[k]['bytes'], path\n"
        " out[k]={'path':path,'sha256':m[k]['sha256'],'bytes':len(b),'mode':oct(stat.S_IMODE(st.st_mode))}\n"
        "assert os.access('/app/parser/parser-host',os.X_OK)\n"
        "r=subprocess.run(['/app/genui'],capture_output=True,text=True,timeout=20)\n"
        "assert r.returncode==1 and 'parser manifest refused' not in r.stderr and 'configuration refused' in r.stderr, r.stderr\n"
        "bad=dict(m,guest=dict(m['guest'],sha256='0'*64)); t=Path('/run/genui/tampered-parser-manifest.json'); t.write_text(json.dumps(bad))\n"
        "r2=subprocess.run(['/app/genui'],env=dict(os.environ,GENUI_PARSER_MANIFEST_FILE=str(t)),capture_output=True,text=True,timeout=20); t.unlink()\n"
        "assert r2.returncode==1 and 'parser manifest refused' in r2.stderr, r2.stderr\n"
        "out['startup']={'manifest_accepted_then':r.stderr.strip(),'tampered_manifest':r2.stderr.strip()}; out['source']=m['source']\n"
        "print(json.dumps(out))")
    artifacts=subprocess.run(['docker','run','--rm','--name',name+'-parser',*controls,args.image,'-c',parser_code],capture_output=True,text=True,timeout=60)
    assert artifacts.returncode==0,artifacts.stderr
    report['parser_artifacts']=json.loads(artifacts.stdout)
    # One framed job through the image's own parser-host, environment cleared,
    # under the same container controls.
    fixtures=Path(__file__).resolve().parents[2]/'parser/fixtures'
    job=[*controls[:-2],'--entrypoint','/usr/bin/env',args.image,'-i','/app/parser/parser-host','--job','--wasm','/app/parser/parser_guest.wasm','--wasm-sha256',report['parser_artifacts']['guest']['sha256']]
    report['parser_jobs']=[]
    for fixture,config,code,expect in (('literal.csv',{'profile':'demo','format':'csv'},0,'parsed'),('valid.xlsx',{'profile':'demo','format':'xlsx'},2,'sheet_selection_required'),('formula.xlsx',{'profile':'demo','format':'xlsx','sheet':'Inventory'},2,'formula_refused')):
        body=json.dumps(config).encode();data=(fixtures/fixture).read_bytes()
        ran=subprocess.run(['docker','run','--rm','-i','--name',name+'-job',*job],input=len(body).to_bytes(4,'big')+body+data,capture_output=True,timeout=60)
        document=json.loads(ran.stdout);line=json.loads(ran.stderr)
        assert ran.returncode==code and line['wasm_sha256']=='sha256:'+report['parser_artifacts']['guest']['sha256'] and line['input_sha256'] in (None,'sha256:'+hashlib.sha256(data).hexdigest()),(fixture,ran.returncode,ran.stderr)
        assert (document['status']=='parsed') if code==0 else (document['reason']==expect),(fixture,document)
        report['parser_jobs'].append({'fixture':fixture,'exit':ran.returncode,'status':document['status'],'reason':document.get('reason'),'available_sheets':document.get('available_sheets'),'report':line})
    info=json.loads(subprocess.check_output(['docker','image','inspect',args.image],text=True))[0]
    report['image_id']=info['Id'];report['architecture']=info['Architecture'];report['passed']=True
finally:
    subprocess.run(['docker','rm','--force',supervisor_name],capture_output=True)
    subprocess.run(['docker','rm','--force',name+'-parser'],capture_output=True)
    subprocess.run(['docker','rm','--force',name+'-job'],capture_output=True)
    subprocess.run(['docker','rm','--force',http_name],capture_output=True)
    subprocess.run(['docker','rm','--force',name],capture_output=True)
    removed=subprocess.run(['docker','volume','rm',volume],capture_output=True,text=True);report['temporary_volume_removed']=removed.returncode==0
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'passed':report['passed'],'image_id':report['image_id'],'architecture':report['architecture'],'output':str(args.output)}))
