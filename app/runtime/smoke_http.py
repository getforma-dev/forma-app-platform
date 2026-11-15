"""Local HTTP/security smoke: a positive mock helper cannot replace native pools."""
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
    helper = Path(directory) / 'synthetic_probe.py'
    marker = Path(directory) / 'probe-called'
    helper.write_text('''import hashlib,json,os,time
from pathlib import Path
path=Path(os.environ['GENUI_RUNTIME_CONFIG_FILE'])
raw=path.read_bytes(); value=json.loads(raw)
path.with_name('probe-called').write_text('called')
print(json.dumps({'generation':value['generation'],'file_sha256':hashlib.sha256(raw).hexdigest(),
 'expires_at':value['expires_at'],'checked_at':int(time.time())}))
''')
    env = {k:v for k,v in os.environ.items() if not k.startswith(('AWS_','GENUI_'))}
    env.update({'GENUI_RUNTIME_CONFIG_FILE':str(path),'GENUI_HELPER_PATH':str(helper),'AWS_EC2_METADATA_DISABLED':'true','FORMA_PORT':'0','FORMA_BIND_ADDRESS':'127.0.0.1'})
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
            # APP-219: with GENUI_LOG_FORMAT unset every log line is one JSON object, no ANSI.
            logged=json.loads(line); assert isinstance(logged,dict) and '\x1b' not in line
            assert all(isinstance(logged.get(key),str) for key in ('timestamp','level','target')), line
            assert time.monotonic()<deadline, 'listener timeout'
        origin=line.split(' ',1)[1]
        with urllib.request.urlopen(origin+'/health/live',timeout=3) as response: assert response.status==200
        for _ in range(20):
            try: urllib.request.urlopen(origin+'/health/ready',timeout=3)
            except urllib.error.HTTPError as error: assert error.code==503 and error.read()==b''
            else: raise AssertionError('unverified managed dependencies reported ready')
            time.sleep(.05)
        assert not marker.exists(), 'missing native registry launched a helper-only readiness path'
        pages=[]
        for _ in range(2):
            with urllib.request.urlopen(origin,timeout=3) as response:
                body=response.read();csp=response.headers['Content-Security-Policy'];assert 'no-store' in response.headers['Cache-Control']
                # APP-215 (infra M4): HSTS on every answer, one year with subdomains, no preload.
                assert response.headers['Strict-Transport-Security']=='max-age=31536000; includeSubDomains'
                assert response.status==200 and b'<title>Forma Canvas</title>' in body and b'Your work.' in body
                assert b'href="/invite"' in body and b'href="/signin"' in body
                assert b'<script' not in body and b'data-forma-island' not in body
                # The zero-JS home has no script to carry the nonce; freshness
                # is a response-header property even when the body needs none.
                nonce=re.search(r"nonce-([^' ]+)",csp).group(1);pages.append(nonce)
        assert pages[0]!=pages[1]
        with urllib.request.urlopen(origin+'/_assets/space-grotesk-latin-5.3.0.woff2',timeout=3) as response:
            assert response.status==200 and response.headers.get_content_type()=='font/woff2'
            assert response.read().startswith(b'wOF2')
        # APP-009 (ADR-006): the native sign-in page, and the uniform 404
        # problem for protected and unknown paths alike.
        with urllib.request.urlopen(origin+'/signin',timeout=3) as response:
            assert response.status==200 and 'no-store' in response.headers['Cache-Control'] and b'Sign in' in response.read()
        # APP-074: the native invitation entry (no script) and the landing links to it.
        with urllib.request.urlopen(origin+'/invite',timeout=3) as response:
            body=response.read()
            assert response.status==200 and 'no-store' in response.headers['Cache-Control']
            assert b'action="/invite/validate"' in body and b'name="code"' in body and b'<script src' not in body
        with urllib.request.urlopen(origin,timeout=3) as response: assert b'href="/invite"' in response.read()
        # APP-223: the public beta notice page and the shared site footer, no
        # cookie; any other method is the uniform 404, as for /about.
        with urllib.request.urlopen(origin+'/notice',timeout=3) as response:
            body=response.read()
            assert response.status==200 and response.headers['Content-Type']=='text/html; charset=utf-8' and response.headers['Cache-Control']=='no-store'
            assert "frame-ancestors 'none'" in response.headers['Content-Security-Policy'] and response.headers.get('Set-Cookie') is None
            assert b'data-notice-version="1"' in body and b'<footer class="site-footer">' in body and b'<script' not in body
        try: urllib.request.urlopen(urllib.request.Request(origin+'/notice',data=b'',method='POST'),timeout=3)
        except urllib.error.HTTPError as error: assert error.code==404 and error.read()==b'{"code":"not_found","status":404}'
        else: raise AssertionError('POST /notice answered')
        # APP-223: RFC 9116's security.txt from the product router, exact bytes.
        with urllib.request.urlopen(origin+'/.well-known/security.txt',timeout=3) as response:
            assert response.status==200 and response.headers['Content-Type']=='text/plain; charset=utf-8'
            assert response.headers['X-Content-Type-Options']=='nosniff' and response.headers['Referrer-Policy']=='no-referrer'
            assert response.read()==b'Contact: mailto:security@getforma.dev\nExpires: 2027-11-01T00:00:00.000Z\nPreferred-Languages: en\nCanonical: https://beta.getforma.dev/.well-known/security.txt\n'
        try: urllib.request.urlopen(urllib.request.Request(origin+'/.well-known/security.txt',data=b'',method='POST'),timeout=3)
        except urllib.error.HTTPError as error: assert error.code==404 and error.read()==b'{"code":"not_found","status":404}'
        else: raise AssertionError('POST /.well-known/security.txt answered')
        for route in ('/workspace','/auth/pending','/v1/unknown','/invite/validate'):
            try: urllib.request.urlopen(origin+route,timeout=3)
            except urllib.error.HTTPError as error:
                assert error.code==404 and error.read()==b'{"code":"not_found","status":404}' and error.headers['Content-Type']=='application/problem+json'
                assert error.headers['Strict-Transport-Security']=='max-age=31536000; includeSubDomains'
            else: raise AssertionError('protected route answered without a session')
        # APP-217: a browser navigation to the same protected path gets the
        # themed 404 page instead, with its status, policy and headers; the
        # page names nothing from the request, and its mascot is served.
        navigation={'Sec-Fetch-Mode':'navigate','Sec-Fetch-Dest':'document','Accept':'text/html'}
        try: urllib.request.urlopen(urllib.request.Request(origin+'/workspace?probe=zq7',headers=navigation),timeout=3)
        except urllib.error.HTTPError as error:
            body=error.read()
            assert error.code==404 and error.headers['Content-Type']=='text/html; charset=utf-8' and error.headers['Cache-Control']=='no-store'
            assert "frame-ancestors 'none'" in error.headers['Content-Security-Policy'] and error.headers.get('Set-Cookie') is None
            assert error.headers['Strict-Transport-Security']=='max-age=31536000; includeSubDomains'
            assert b'Specimen not found' in body and b'<script' not in body and b'/workspace' not in body and b'zq7' not in body
        else: raise AssertionError('protected route answered without a session')
        with urllib.request.urlopen(origin+'/_assets/forma-isotope.svg',timeout=3) as response:
            assert response.status==200 and response.headers.get_content_type()=='image/svg+xml' and response.read().startswith(b'<svg')
        print(json.dumps({'scope':'synthetic local HTTP denial smoke; not managed acceptance','live':200,'ready_without_native_registry':503,'positive_mock_helper_not_launched':True,'native_home_zero_js':True,'fresh_csp_nonces':True,'no_store':True,'hsts':True,'signin_page':200,'invite_page':200,'notice_page':200,'security_txt':200,'protected_uniform_404':True,'navigation_error_page':404}))
    finally:
        process.terminate()
        try: process.communicate(timeout=5)
        except subprocess.TimeoutExpired:process.kill();process.communicate()
    path.chmod(0o644)
    refused=subprocess.run([args.binary],env=env,capture_output=True,text=True,timeout=5)
    assert refused.returncode!=0 and 'unit-only' not in refused.stdout+refused.stderr
    print(json.dumps({'scope':'startup credential permission failure','passed':True,'credential_values_disclosed':False}))
    path.chmod(0o600)
    # APP-222 (A13): an out-of-range GENUI_LIMIT_* setting, and APP-219: an unknown log
    # format, stop the process before it binds, naming the setting.
    for name,value in (('GENUI_LIMIT_MCP_TOKEN_PER_HOUR','0'),('GENUI_LIMIT_MCP_MEMBER_PER_HOUR','100001'),('GENUI_LIMIT_AGENT_TOKEN_MINTS_PER_HOUR','many'),('GENUI_LOG_FORMAT','text')):
        refused=subprocess.run([args.binary],env={**env,name:value},capture_output=True,text=True,timeout=5)
        assert refused.returncode!=0 and 'FORMA_LISTENING' not in refused.stdout and name in refused.stderr, (name,refused.stderr)
    print(json.dumps({'scope':'out-of-range limit and log settings refuse to start','passed':True}))
    # Reach the private registry loader: an invalid hostname would refuse in
    # Settings::new first and would not exercise this missing-file boundary.
    document['database']['host'] = 'synthetic-test.us-east-1.rds.amazonaws.com'
    path.write_text(json.dumps(document))
    env['GENUI_IDENTITY_REGISTRY_FILE'] = str(Path(directory) / 'absent-registry.json')
    refused=subprocess.run([args.binary],env=env,capture_output=True,text=True,timeout=5)
    assert refused.returncode!=0 and 'FORMA_LISTENING' not in refused.stdout
    assert 'tenant storage configuration refused' in refused.stderr
    assert 'unit-only' not in refused.stdout+refused.stderr
    print(json.dumps({'scope':'accepted RDS hostname with missing private registry cannot bind a listener','passed':True}))
