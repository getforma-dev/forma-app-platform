#!/usr/bin/env python3
"""Probe the real Rust HTTP response; this is not a browser adoption test."""
import json
import os
import queue
import re
import subprocess
import threading
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
APP = Path(__file__).resolve().parent/'minimal'
proc = subprocess.Popen([str(APP/'target/debug/minimal')], cwd=APP,
                        env={**os.environ, 'FORMA_PORT': '0'}, stdout=subprocess.PIPE,
                        stderr=subprocess.STDOUT, text=True)
lines = queue.Queue()
def read():
    for line in proc.stdout:
        lines.put(line)
threading.Thread(target=read, daemon=True).start()
try:
    url = None
    for _ in range(30):
        line = lines.get(timeout=10)
        if 'FORMA_LISTENING ' in line:
            url = line.strip().split('FORMA_LISTENING ', 1)[1]
            break
    assert url, 'No readiness signal from owned process'
    def get(path):
        with urllib.request.urlopen(url+path, timeout=10) as response:
            return response.read().decode(), dict(response.headers)
    html, headers = get('/')
    again, headers2 = get('/')
    about, about_headers = get('/about')
    assert 'Count:' in html and re.search(r'>7<', html), 'Rust count absent before JavaScript'
    assert '<button' in html and 'Increment' in html
    assert 'About this app' in about and not re.search(r'<script\b', about), 'About must be no-JS SSR'
    for h in (headers, headers2, about_headers):
        normalized = {k.lower(): v for k, v in h.items()}
        assert normalized['cache-control'] == 'no-store'
        assert normalized['x-content-type-options'] == 'nosniff'
        assert "default-src 'none'" in normalized['content-security-policy']
    def nonce(h):
        csp = next(v for k,v in h.items() if k.lower() == 'content-security-policy')
        return re.search(r"'nonce-([^']+)'", csp).group(1)
    n1, n2 = nonce(headers), nonce(headers2)
    assert n1 != n2
    assert f'nonce="{n1}"' in html and f'nonce="{n2}"' in again
    manifest = json.loads((APP/'admin/dist/manifest.json').read_text())
    sizes = json.loads((APP/'admin/dist/size-report.json').read_text())
    report = {'checks': ['typed count 7 in raw HTML', 'Counter controls in initial HTML',
                         'about renders with no script', 'all responses no-store and nosniff',
                         'CSP nonce matches HTML and changes on next response'],
              'result': 'passed', 'browser_adoption': 'not tested; APP-003 gate remains open',
              'html_bytes': len(html.encode()), 'about_bytes': len(about.encode()),
              'size_report': sizes}
    (ROOT/'evidence/APP-001/http.json').write_text(json.dumps(report, indent=2)+'\n')
    print('PASS: typed HTTP SSR, static no-JS page, CSP/nonce/no-store response policy')
finally:
    proc.terminate()
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()
