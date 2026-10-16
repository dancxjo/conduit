// Fresh owner checkpoint execution followed by checked browser Mask consumption.
// This establishes fixture execution and simulated Show acknowledgement only.
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runCheckpointMasks } from './committed-todo-exchange-runner.mjs';

const scratch = mkdtempSync(join(tmpdir(), 'conduit-committed-todo-mask-'));
try {
  runCheckpointMasks({ ...process.env, CONDUIT_CROSS_MASK_TODO_EXCHANGE: join(scratch, 'exchange.json') });
} finally {
  rmSync(scratch, { recursive: true });
}
