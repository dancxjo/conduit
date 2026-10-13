// Fresh owner checkpoint execution followed by checked browser Mask consumption.
// This establishes fixture execution and simulated Show acknowledgement only.
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

const scratch = mkdtempSync(join(tmpdir(), 'conduit-committed-todo-mask-'));
const env = { ...process.env, CONDUIT_CROSS_MASK_TODO_EXCHANGE: join(scratch, 'exchange.json') };
function test(args) {
  const result = spawnSync('cargo', ['test', '--locked', ...args], {
    env, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024,
  });
  process.stdout.write(result.stdout ?? '');
  process.stderr.write(result.stderr ?? '');
  if (result.error || result.status !== 0) throw new Error(`checkpoint Mask test failed: ${result.error ?? result.status}`);
  if (!/^running 1 test$/m.test(result.stdout ?? '')
      || !/^test result: ok\. 1 passed; 0 failed;/m.test(result.stdout ?? '')) {
    throw new Error('checkpoint Mask proof did not execute exactly one required test');
  }
}
try {
  test(['-p', 'conduit', '--bin', 'conduit',
    'verified_three_item_todo_and_completion_preserve_nonvisual_meaning']);
  test(['-p', 'conduit-browser-runtime', '--lib',
    'checked_browser_mask_consumes_actual_committed_todo_exchange', '--', '--ignored']);
} finally {
  rmSync(scratch, { recursive: true });
}
