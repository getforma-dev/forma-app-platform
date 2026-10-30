"""Image-only synthetic supervisor smoke; no managed/provider acceptance.

Pass this file on stdin to the confined image. Only runtime.refresh is replaced;
the production supervisor, owned files, native constructor and cleanup run real.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import queue
import secrets
import signal
import stat
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.request


WRAPPER = """
import os,time,uuid
import runtime
def refresh(path):
    now=int(time.time())
    value={'schema_version':1,'generation':str(uuid.uuid4()),'issued_at':now,'expires_at':now+500,
      'database':{'host':'synthetic-test.us-east-1.rds.amazonaws.com','port':5432,'name':'genui',
        'roles':{name:{'username':'genui_'+name,'token':'supervisor-only-'+name,'expires_at':now+500} for name in runtime.ROLES}},
      'storage':{'region':'us-east-1','bucket':'supervisor-only-bucket','tenants':[
        {'organization_id':org,'prefix':'o/'+org+'/','role_arn':'supervisor-only-role-'+str(i),
         'kms_key_arn':'supervisor-only-key-'+str(i),'access_key_id':'supervisor-only-id',
         'secret_access_key':'supervisor-only-secret','session_token':'supervisor-only-session','expires_at':now+500}
        for i,org in enumerate(('00000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000002'))]}}
    runtime.atomic_write(path,value)
    runtime.load(path)
runtime.refresh=refresh
# Mirror the image entrypoint (runtime.py __main__): refuse first, then serve;
# any refusal prints only the fixed marker, never the exception.
try:
    runtime.refuse_qualification_injection()
    status=runtime.serve(os.environ['GENUI_RUNTIME_CONFIG_FILE'])
except Exception:
    print('managed_dependency_operation_refused',flush=True)
    raise SystemExit(1)
raise SystemExit(status)
"""


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args):
        return None


def capability():
    return base64.urlsafe_b64encode(secrets.token_bytes(32)).decode().rstrip('=')


def private_file(path):
    info = path.lstat()
    assert stat.S_ISREG(info.st_mode) and info.st_uid == 65532
    assert stat.S_IMODE(info.st_mode) == 0o600 and info.st_nlink == 1


def run_mode(mode, binary, directory):
    with tempfile.TemporaryDirectory(prefix='supervisor-smoke-', dir=directory) as folder:
        private = Path(folder)
        assert stat.S_IMODE(private.stat().st_mode) == 0o700
        credential = private / 'credentials.json'
        config = {'run_id':'synthetic-supervisor-smoke','app_source_revision':'0'*40,
                  'provider_source_revision':'0'*40,'expires_at':int(time.time())+300,
                  'max_attempts':1,'max_flows':1,'synthetic_tenant':True,
                  'auth_origin':'https://synthetic-auth.invalid','tenant_slug':'synthetic',
                  'tenant_id':'ten_00000000-0000-4000-8000-000000000001',
                  'client_id':'synthetic-client','issuer':'synthetic-issuer','audience':'synthetic-audience',
                  'max_access_lifetime_seconds':900,'expected_email':'callback@example.test',
                  'expected_subject':None,'expected_email_verified':False,
                  'operator_token':capability(),'invite_token':'synthetic-private-invitation-'+capability()}
        injected = json.dumps(config) if mode == 'injected' else '{malformed-synthetic-private-json'
        secret_values = [injected, config['operator_token'], config['invite_token'], 'supervisor-only-']
        env = {'PATH':os.defpath,'PYTHONDONTWRITEBYTECODE':'1','PYTHONUNBUFFERED':'1',
               'GENUI_RUNTIME_CONFIG_FILE':str(credential),'GENUI_BINARY':str(binary),
               'GENUI_RDS_CA_FILE':'/etc/ssl/certs/rds-global-bundle.pem',
               'FORMA_PORT':'0','FORMA_BIND_ADDRESS':'127.0.0.1','AWS_EC2_METADATA_DISABLED':'true'}
        if mode != 'disabled':
            env['GENUI_AUTH_QUALIFICATION_CONFIG_JSON'] = injected
        process = subprocess.Popen([sys.executable, '-c', WRAPPER], env=env, start_new_session=True,
                                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        lines, output, observations, failures = queue.Queue(), bytearray(), [], []
        stop = threading.Event()
        native_pids = set()

        def collect():
            for line in process.stdout:
                if len(output) + len(line) <= 65536:
                    output.extend(line)
                else:
                    failures.append('supervisor output bound exceeded')
                lines.put(line.decode('utf-8', errors='replace').strip())
            lines.put(None)

        def inspect_child():
            # Observe the actual exec'd native process, without intercepting or
            # replacing the production Popen call. /proc is local to this image.
            while not stop.is_set():
                try:
                    children = Path(f'/proc/{process.pid}/task/{process.pid}/children').read_text().split()
                    for pid in children:
                        if Path(f'/proc/{pid}/exe').resolve() != binary:
                            continue
                        native_pids.add(int(pid))
                        raw = Path(f'/proc/{pid}/environ').read_bytes()
                        assert b'GENUI_AUTH_QUALIFICATION_CONFIG_JSON=' not in raw
                        assert all(value.encode() not in raw for value in secret_values)
                        command = Path(f'/proc/{pid}/cmdline').read_bytes()
                        assert command.rstrip(b'\0') == str(binary).encode()
                        child_env = dict(item.split(b'=', 1) for item in raw.split(b'\0') if item)
                        registry = Path(child_env[b'GENUI_IDENTITY_REGISTRY_FILE'].decode())
                        assert registry.parent == private and registry.name.startswith('.identity-registry-')
                        private_file(registry)
                        document = json.loads(registry.read_bytes())
                        assert set(document) == {'schema_version','registry_version','bindings'}
                        assert document['schema_version'] == 1 and document['bindings'] == []
                        private_file(credential)
                        auth = child_env.get(b'GENUI_AUTH_QUALIFICATION_CONFIG_FILE')
                        if mode == 'disabled':
                            assert auth is None
                        else:
                            path = Path(auth.decode())
                            assert path.parent == private and path.name.startswith('.auth-qualification-')
                            private_file(path)
                            assert path.read_bytes() == injected.encode()
                        observations.append(True)
                        return
                except (FileNotFoundError, ProcessLookupError):
                    pass
                except Exception:
                    failures.append('native private-file/environment inspection failed')
                    return
                stop.wait(.002)

        reader = threading.Thread(target=collect, daemon=True)
        inspector = threading.Thread(target=inspect_child, daemon=True)
        reader.start(); inspector.start()
        forced = False
        status = None
        try:
            if mode != 'disabled':
                # ADR-007 D7.13 (APP-009 review m6): the supervisor refuses any
                # operator qualification configuration, valid or malformed, before
                # refreshing credentials or starting the native child.
                assert process.wait(timeout=10) != 0, 'qualification configuration was accepted'
            else:
                deadline = time.monotonic() + 15
                while True:
                    line = lines.get(timeout=max(.01, deadline-time.monotonic()))
                    assert line is not None, 'supervisor exited before listener'
                    if line.startswith('FORMA_LISTENING http://127.0.0.1:'):
                        origin = line.split(' ',1)[1]
                        break
                    assert time.monotonic() < deadline, 'listener deadline exceeded'
                opener = urllib.request.build_opener(NoRedirect(), urllib.request.ProxyHandler({}))
                try:
                    response = opener.open(origin+'/auth/callback?code='+capability()+'&state='+capability(), timeout=3)
                except urllib.error.HTTPError as error:
                    response = error
                with response:
                    status = response.code
                    # The product callback without a flow: the neutral failure.
                    assert status == 400 and response.headers.get('Location') is None
                    body = response.read()
                    assert all(value.encode() not in body for value in secret_values)
                try:
                    opener.open(origin+'/health/ready', timeout=3)
                except urllib.error.HTTPError as error:
                    assert error.code == 503
                    error.close()
                else:
                    raise AssertionError('synthetic confined stores reported ready')
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=12)
                except subprocess.TimeoutExpired:
                    forced = True
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait(timeout=3)
            stop.set(); inspector.join(timeout=2); reader.join(timeout=2)
            process.stdout.close()
        assert not forced, 'supervisor did not cleanly stop within its bound'
        if mode == 'disabled':
            assert observations and not failures, 'native environment/files were not verified'
        else:
            assert not observations and not native_pids and not failures, 'a native child started under an injection'
        assert all(value.encode() not in output for value in secret_values), 'synthetic secret output'
        assert not any(Path(f'/proc/{pid}').exists() for pid in native_pids), 'native child remains'
        assert not list(private.iterdir()), 'owned auth/registry/credential files remain'
        if mode != 'disabled':
            assert b'FORMA_LISTENING' not in output and b'managed_dependency_operation_refused' in output
            # Refused before any credential refresh: no refresh attempt, no traceback.
            assert b'credential_refresh_unavailable' not in output and b'Traceback' not in output
        return {'mode':mode,'callback_status':status,'refused_before_bind':mode != 'disabled',
                'refused_by_supervisor_before_child':mode != 'disabled',
                'native_environment_observed':mode == 'disabled','raw_secret_in_child_environment':False,
                'private_file_modes':'0600','empty_registry_observed':True,'secret_output':False,
                'owned_files_removed':True,'native_child_removed':True,'passed':True}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, default=Path('/app/genui'))
    parser.add_argument('--directory', type=Path, default=Path('/run/genui'))
    args = parser.parse_args()
    assert os.geteuid() == 65532 and args.binary == Path('/app/genui')
    assert Path('/etc/ssl/certs/rds-global-bundle.pem').is_file()
    assert args.directory.is_dir() and stat.S_IMODE(args.directory.stat().st_mode) == 0o700
    results = [run_mode(mode, args.binary, args.directory) for mode in ('disabled','injected','malformed')]
    print(json.dumps({'scope':'Actual production supervisor/native image with only synthetic refresh replacement',
                      'managed_acceptance':False,'provider_acceptance':False,'results':results,'passed':True}))


if __name__ == '__main__':
    try:
        main()
    except Exception:
        print(json.dumps({'scope':'synthetic supervisor smoke','passed':False,'error':'supervisor smoke refused'}))
        raise SystemExit(1)
