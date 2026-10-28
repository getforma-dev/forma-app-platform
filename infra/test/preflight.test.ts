import { test } from 'node:test';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

test('deployment preflight refuses stale/over-budget windows and durably preserves reservations', () => {
  execFileSync('python3', ['-m', 'unittest', 'discover', '-s', 'test', '-p', 'preflight_test.py', '-v'], {
    cwd: fileURLToPath(new URL('../', import.meta.url)), stdio: 'pipe',
  });
});
