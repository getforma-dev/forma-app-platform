import { test } from 'node:test';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

test('runtime negative runner preserves isolation, refusal and cleanup boundaries locally', () => {
  execFileSync('python3', ['-B', '-m', 'unittest', 'discover', '-s', 'test', '-p', 'managed_runtime_negatives_test.py', '-v'], {
    cwd: fileURLToPath(new URL('../', import.meta.url)), stdio: 'pipe',
    timeout: 30_000, killSignal: 'SIGKILL',
  });
});
