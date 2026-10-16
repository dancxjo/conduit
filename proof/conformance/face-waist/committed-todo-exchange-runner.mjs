// Both proofs use one Cargo feature context; the owner runs before its consumer.
import { spawnSync } from 'node:child_process';

const defaultContext = ['test', '--locked', '-p', 'conduit', '--bin', 'conduit',
  '-p', 'conduit-browser-runtime', '--lib'];
const proofs = [
  ['verified_three_item_todo_and_completion_preserve_nonvisual_meaning'],
  ['checked_browser_mask_consumes_actual_committed_todo_exchange', '--', '--ignored'],
];

// The parent supplies its canonical Step, never an arbitrary command or filter.
export function productCargoContext(encoded) {
  const payload = JSON.parse(encoded);
  const args = payload?.args;
  if (payload?.schema !== 'conduit.ci/product-cargo-context@1'
      || !Array.isArray(args) || args.length < 3 || args.length > 128
      || args.some(arg => typeof arg !== 'string' || arg.length > 1024)
      || args[0] !== 'test') throw new Error('invalid product Cargo context');
  const packages = new Set();
  const name = /^[a-z0-9][a-z0-9_-]*$/;
  const feature = /^[a-z0-9][a-z0-9_-]*\/[a-z0-9][a-z0-9_-]*$/;
  for (let i = 1; i < args.length; i++) {
    if (args[i] === '--no-fail-fast' || args[i] === '--locked') continue;
    if (args[i] === '-p' && name.test(args[i + 1] ?? '')) {
      packages.add(args[++i]);
    } else if (args[i] === '--features' && typeof args[i + 1] === 'string'
        && args[i + 1].split(',').every(value => feature.test(value))) {
      i++;
    } else {
      throw new Error('unsupported product Cargo context argument');
    }
  }
  if (!packages.has('conduit')) throw new Error('product context is missing its owner');
  return ['test', '--locked', ...args.slice(1).filter(arg => arg !== '--locked'),
    ...(!packages.has('conduit-browser-runtime') ? ['-p', 'conduit-browser-runtime'] : []),
    '--lib', '--bin', 'conduit'];
}

export function runCheckpointMasks(env, {
  context = defaultContext,
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
