from pathlib import Path
import json,subprocess,tempfile,uuid,shutil
base=Path(tempfile.mkdtemp(prefix='app005-supervision-review-'));base.chmod(0o755)
(base/'processes.py').write_bytes(subprocess.check_output(['git','show','315c292:spikes/APP-005/processes.py']))
(base/'query.py').write_text('class Refused(ValueError): pass\nclass ContractError(ValueError): pass\nclass Deadline(TimeoutError): pass\ndef execute_query(*args,**kwargs): raise AssertionError("not invoked in supervisor-only control")\n')
(base/'probe.py').write_text('''import json,os,signal,threading,time
import multiprocessing.forkserver
import processes
if __name__ == "__main__":
 first=processes._supervise(processes._write,{"ok":1})
 pid=multiprocessing.forkserver._forkserver._forkserver_pid
 os.kill(pid,signal.SIGSTOP)
 result=[]
 def call():
  try: result.append(processes._supervise(processes._write,{"ok":2}))
  except BaseException as error: result.append(type(error).__name__+": "+str(error))
 started=time.monotonic();worker=threading.Thread(target=call,daemon=True);worker.start()
 try:
  worker.join(6.2)
  stalled=worker.is_alive();elapsed=time.monotonic()-started
 finally:
  os.kill(pid,signal.SIGCONT)
 worker.join(10)
 print(json.dumps({"scope":"exact reviewed supervisor; query import stubbed; no database query","initial_child":first,"still_blocked_after_5s_backstop":stalled,"observed_seconds_before_resuming_owned_server":elapsed,"result_after_server_resumed":result,"joined":not worker.is_alive()}))
''')
name='app005-review-'+uuid.uuid4().hex[:10]
cmd=['docker','run','--rm','--name',name,'--network=none','--read-only','--user=65532:65532','--cap-drop=ALL','--security-opt=no-new-privileges','--cpus=2','--memory=3g','--memory-swap=3g','--pids-limit=64','--tmpfs','/tmp:rw,noexec,nosuid,nodev,size=67108864,uid=65532,gid=65532,mode=700','--mount',f'type=bind,source={base},target=/app,readonly','--workdir','/app','--env','PYTHONDONTWRITEBYTECODE=1','python:3.11.14-slim-bookworm@sha256:65a93d69fa75478d554f4ad27c85c1e69fa184956261b4301ebaf6dbb0a3543d','python','/app/probe.py']
try:
 r=subprocess.run(cmd,capture_output=True,text=True,timeout=30);print(r.stdout);print(r.stderr);print('exit',r.returncode)
 Path('/tmp/app005-review-forkserver-stall.json').write_text(r.stdout)
finally:
 subprocess.run(['docker','rm','--force',name],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 shutil.rmtree(base)
