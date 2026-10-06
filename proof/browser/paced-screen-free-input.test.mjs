import assert from 'node:assert/strict';
import test from 'node:test';
import { runPacedScreenFree } from './paced-screen-free-input.mjs';

const cli = `
import { createInterface } from 'node:readline';
const input = createInterface({ input: process.stdin });
let reads = 0;
process.stdout.write('body');
setImmediate(() => process.stdout.write('> '));
input.on('line', command => {
  if (command === 'read all') {
    reads++;
    process.stdout.write(reads === 1
      ? 'Stopped the stale reading; read all again for the current Face.\\nbody> '
      : 'Current Face reading completed.\\nbody> ');
  } else if (command === 'quit') {
    process.stdout.write('bye\\n', () => process.exit(0));
  } else {
    process.exit(2);
  }
});
`;

test('paced input rereads a changed Face and retains every actual command', async () => {
  const result = await runPacedScreenFree(process.execPath,
    ['--input-type=module', '-e', cli], ['read all', 'quit'], 'body> ', 3000,
    { retryStaleReadAll: 2 });
  assert.deepEqual(result.commands, ['read all', 'read all', 'quit']);
  assert.match(result.transcript, /Current Face reading completed/);
});
