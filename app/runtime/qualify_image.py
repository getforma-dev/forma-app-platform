"""Local Docker image trial; creates/removes only its own container and volume."""
import argparse
import datetime
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
    info=json.loads(subprocess.check_output(['docker','image','inspect',args.image],text=True))[0]
    report['image_id']=info['Id'];report['architecture']=info['Architecture'];report['passed']=True
finally:
    subprocess.run(['docker','rm','--force',supervisor_name],capture_output=True)
    subprocess.run(['docker','rm','--force',http_name],capture_output=True)
    subprocess.run(['docker','rm','--force',name],capture_output=True)
    removed=subprocess.run(['docker','volume','rm',volume],capture_output=True,text=True);report['temporary_volume_removed']=removed.returncode==0
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'passed':report['passed'],'image_id':report['image_id'],'architecture':report['architecture'],'output':str(args.output)}))
