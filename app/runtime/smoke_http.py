"""Local HTTP/security smoke with a deliberately unavailable probe; no cloud IO."""
import argparse
import json
import os
from pathlib import Path
import re
import queue
import threading
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import uuid

parser = argparse.ArgumentParser(); parser.add_argument('--binary', required=True); parser.add_argument('--directory'); args = parser.parse_args()
with tempfile.TemporaryDirectory(prefix='genui-http-unit-', dir=args.directory) as directory:
    path = Path(directory) / 'credentials.json'; now = int(time.time())
    document = {'schema_version':1,'generation':str(uuid.uuid4()),'issued_at':now,'expires_at':now+500,
        'database':{'host':'synthetic.invalid','port':5432,'name':'genui','roles':{name:{'username':'genui_'+name,'token':'unit-only-'+name,'expires_at':now+500} for name in ('workspace','control','data','worker')}},'storage':{}}
    path.write_text(json.dumps(document)); path.chmod(0o600)
    env = {k:v for k,v in os.environ.items() if not k.startswith(('AWS_','GENUI_'))}
    env.update({'GENUI_RUNTIME_CONFIG_FILE':str(path),'GENUI_HELPER_PATH':'/deliberately-absent-managed-probe.py','AWS_EC2_METADATA_DISABLED':'true','FORMA_PORT':'0','FORMA_BIND_ADDRESS':'127.0.0.1'})
    process = subprocess.Popen([args.binary], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        lines=queue.Queue()
        def read_lines():
            for value in process.stdout: lines.put(value.strip())
            lines.put(None)
        threading.Thread(target=read_lines,daemon=True).start()
        deadline=time.monotonic()+15
        while True:
            line=lines.get(timeout=max(0.01,deadline-time.monotonic()))
            assert line is not None, 'server exited before binding'
            if line.startswith('FORMA_LISTENING http://127.0.0.1:'): break
            assert time.monotonic()<deadline, 'listener timeout'
        origin=line.split(' ',1)[1]
        with urllib.request.urlopen(origin+'/health/live',timeout=3) as response: assert response.status==200
        for _ in range(2):
            try: urllib.request.urlopen(origin+'/health/ready',timeout=3)
            except urllib.error.HTTPError as error: assert error.code==503 and error.read()==b''
            else: raise AssertionError('unverified managed dependencies reported ready')
        pages=[]
        for _ in range(2):
            with urllib.request.urlopen(origin,timeout=3) as response:
                body=response.read();csp=response.headers['Content-Security-Policy'];assert 'no-store' in response.headers['Cache-Control']
                assert b'Foundation workspace' in body and re.search(rb'>\s*7\s*<',body)
                nonce=re.search(r"nonce-([^' ]+)",csp).group(1);assert nonce.encode() in body;pages.append(nonce)
        assert pages[0]!=pages[1]
        print(json.dumps({'scope':'synthetic local HTTP denial smoke; not managed acceptance','live':200,'ready_without_probe':503,'native_html_count':7,'fresh_csp_nonces':True,'no_store':True}))
    finally:
        process.terminate()
        try: process.communicate(timeout=5)
        except subprocess.TimeoutExpired:process.kill();process.communicate()
    path.chmod(0o644)
    refused=subprocess.run([args.binary],env=env,capture_output=True,text=True,timeout=5)
    assert refused.returncode!=0 and 'unit-only' not in refused.stdout+refused.stderr
    print(json.dumps({'scope':'startup credential permission failure','passed':True,'credential_values_disclosed':False}))
