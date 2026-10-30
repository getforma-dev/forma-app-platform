import { test } from 'node:test';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const cwd = fileURLToPath(new URL('../', import.meta.url));

test('APP-078 window request composer: private code minting, documented overrides, armed qualification manifest, change-set review', () => {
  execFileSync('python3', ['-B', '-m', 'unittest', 'discover', '-s', 'test', '-p', 'window_requests_test.py', '-v'], { cwd, stdio: 'pipe' });
});

test('APP-078 read-only snapshot admits only allowlisted describe/get/list calls and redacts private values', () => {
  execFileSync('python3', ['-B', '-m', 'unittest', 'discover', '-s', 'test', '-p', 'readonly_snapshot_test.py', '-v'], { cwd, stdio: 'pipe' });
});
