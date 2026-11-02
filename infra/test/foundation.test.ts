import { test } from 'node:test';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

test('Foundation runner retains bounded timeout and failure evidence', () => {
  execFileSync('python3', ['-m', 'unittest', 'discover', '-s', 'test', '-p', 'foundation_check_test.py', '-v'], {
    cwd: fileURLToPath(new URL('../', import.meta.url)), stdio: 'pipe',
  });
});
