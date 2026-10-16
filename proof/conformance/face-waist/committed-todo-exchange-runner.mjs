// Both proofs use one Cargo feature context; the owner runs before its consumer.
import { spawnSync } from 'node:child_process';

const context = ['test', '--locked', '-p', 'conduit', '--bin', 'conduit',
  '-p', 'conduit-browser-runtime', '--lib'];
const proofs = [
  ['verified_three_item_todo_and_completion_preserve_nonvisual_meaning'],
  ['checked_browser_mask_consumes_actual_committed_todo_exchange', '--', '--ignored'],
];

export function runCheckpointMasks(env, {
  spawn = spawnSync,
  stdout = text => process.stdout.write(text),
  stderr = text => process.stderr.write(text),
} = {}) {
  for (const proof of proofs) {
    const result = spawn('cargo', [...context, ...proof], {
      env, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024,
      // Keep compilation visible while retaining test stdout for the proof gate.
      stdio: ['ignore', 'pipe', 'inherit'],
    });
    stdout(result.stdout ?? '');
    stderr(result.stderr ?? '');
    if (result.error || result.status !== 0) throw new Error(`checkpoint Mask test failed: ${result.error ?? result.signal ?? result.status}`);
    const output = result.stdout ?? '';
    const started = [...output.matchAll(/^running (\d+) tests?$/gm)].reduce((sum, match) => sum + Number(match[1]), 0);
    const summaries = [...output.matchAll(/^test result: ok\. (\d+) passed; (\d+) failed;/gm)];
    const passed = summaries.reduce((sum, match) => sum + Number(match[1]), 0);
    if (started !== 1 || passed !== 1 || summaries.some(match => Number(match[2]) !== 0)) {
      throw new Error('checkpoint Mask proof did not execute exactly one required test');
    }
  }
}
