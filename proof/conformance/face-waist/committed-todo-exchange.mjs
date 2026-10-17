// Fresh owner checkpoint execution followed by checked browser Mask consumption.
// This establishes fixture execution and simulated Show acknowledgement only.
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { productCargoContext, runCheckpointMasks } from './committed-todo-exchange-runner.mjs';

const args = process.argv.slice(2);
if (args.length && (args.length !== 2 || args[0] !== '--products-context-json')) {
  throw new Error('unsupported checkpoint Mask proof arguments');
}
const options = args.length ? { context: productCargoContext(args[1]) } : {};
const scratch = mkdtempSync(join(tmpdir(), 'conduit-committed-todo-mask-'));
try {
  runCheckpointMasks({ ...process.env, CONDUIT_CROSS_MASK_TODO_EXCHANGE: join(scratch, 'exchange.json') }, options);
} finally {
  rmSync(scratch, { recursive: true });
}
