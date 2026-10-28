import hashlib,json,pathlib,shutil,subprocess,tempfile,uuid
repo=pathlib.Path(__file__).resolve().parents[2]
name='genui-app005-independent-'+uuid.uuid4().hex[:10]
image=name+':review'
result={'scope':'Independent Linux process controls with exact query module and dependencies; no analytical benchmark claim','source_hashes':{}}
with tempfile.TemporaryDirectory(prefix=name) as tmp:
    ctx=pathlib.Path(tmp); (ctx/'contracts').mkdir()
    paths=[('spikes/APP-005/'+x,x) for x in ('requirements.lock','fixture.py','query.py','processes.py','test_processes.py')]+[('contracts/'+x,'contracts/'+x) for x in ('requirements.lock','schema.json','validate.py')]
    for source,target in paths:
        shutil.copyfile(repo/source,ctx/target)
        result['source_hashes'][source]=hashlib.sha256((ctx/target).read_bytes()).hexdigest()
    dockerfile='FROM python:3.11.14-slim-bookworm@sha256:65a93d69fa75478d554f4ad27c85c1e69fa184956261b4301ebaf6dbb0a3543d\nWORKDIR /app\nCOPY requirements.lock /app/requirements.lock\nCOPY contracts/requirements.lock /app/contracts/requirements.lock\nRUN python -m pip install --no-cache-dir --require-hashes --only-binary=:all: -r /app/requirements.lock -r /app/contracts/requirements.lock\nCOPY contracts /app/contracts\nCOPY fixture.py query.py processes.py test_processes.py /app/\nENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1 FORMA_CONTRACTS=/app/contracts FORMA_QUERY_TMP=/tmp\nUSER 65532:65532\nENTRYPOINT ["python","/app/test_processes.py"]\n'
    (ctx/'Dockerfile').write_text(dockerfile); result['dockerfile']=dockerfile
    try:
        build=subprocess.run(['docker','build','--tag',image,str(ctx)],text=True,capture_output=True,timeout=120)
        pathlib.Path('/tmp/genui-app005-review-broker-build.log').write_text(build.stdout+build.stderr)
        assert build.returncode==0,build.stderr
        result['image_id']=json.loads(subprocess.check_output(['docker','image','inspect',image],text=True))[0]['Id']
        command=['docker','create','--name',name,'--network=none','--read-only','--cap-drop=ALL','--security-opt=no-new-privileges','--cpus=2','--memory=1g','--memory-swap=1g','--pids-limit=64','--tmpfs','/tmp:rw,noexec,nosuid,nodev,size=134217728,uid=65532,gid=65532,mode=700',image]
        cid=subprocess.check_output(command,text=True).strip()
        run=subprocess.run(['docker','start','--attach',cid],text=True,capture_output=True,timeout=45)
        result['exit_code']=run.returncode;result['stderr']=run.stderr
        result['controls']=json.loads(run.stdout)
        inspected=json.loads(subprocess.check_output(['docker','inspect',cid],text=True))[0]
        result['container_state']=inspected['State']
        result['host_config']={k:inspected['HostConfig'][k] for k in ('NetworkMode','ReadonlyRootfs','Memory','MemorySwap','NanoCpus','PidsLimit','CapDrop','SecurityOpt','Tmpfs')}
        assert run.returncode==0 and result['controls']['passed'],run.stderr
    finally:
        result['container_removed']=subprocess.run(['docker','rm','--force',name],capture_output=True).returncode==0
        result['image_removed']=subprocess.run(['docker','image','rm',image],capture_output=True).returncode==0
        pathlib.Path('/tmp/genui-app005-review-broker.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'passed':result['controls']['passed'],'image':result['image_id'],'controls':len(result['controls']['controls']),'container_removed':result['container_removed']}))
