import { appendFileSync, mkdirSync, readdirSync, existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { planChanges } from './plan.mjs';
import { sealTarget, verifyBundle } from './receipts.mjs';
import { TARGETS, setupTarget, runTarget } from './targets.mjs';
import { setupCi, setupUnit } from './setup.mjs';
import { assertSourceCheckout } from './source.mjs';
import { emitAcquisitionKey, measureAcquisition } from './acquisition/metrics.mjs';
import { retainedOneBodyEvidence } from './one-body-evidence.mjs';

function run(program, args, capture = false) {
  const result = spawnSync(program, args, { stdio: capture ? ['ignore', 'pipe', 'inherit'] : 'inherit', encoding: 'utf8' });
  if (result.error || result.status !== 0) throw new Error(`${program} ${args.join(' ')} failed: ${result.error ?? result.status}`);
  return result.stdout?.trim();
}
function exactSha(value) {
  if (!/^[a-f0-9]{40}$/.test(value ?? '')) throw new Error('expected full commit SHA');
  return value;
}
function output(key, value) {
  const line = `${key}=${typeof value === 'string' ? value : JSON.stringify(value)}\n`;
  process.stdout.write(line);
  if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, line);
}
function target(id) {
  const found = TARGETS.find(item => item.id === id);
  if (!found) throw new Error(`unknown target: ${id}`);
  return found;
}

const [command, ...args] = process.argv.slice(2);
try {
  switch (command) {
    case 'setup-ci':
      if (args.length) throw new Error('setup-ci accepts no arguments');
      await measureAcquisition('preflight', setupCi);
      break;
    case 'setup-unit':
      if (args.length) throw new Error('setup-unit accepts no arguments');
      await measureAcquisition('unit', setupUnit);
      break;
    case 'acquisition-key':
      if (args.length !== 1) throw new Error('usage: acquisition-key <target|preflight|unit>');
      if (!['preflight', 'unit'].includes(args[0])) target(args[0]);
      emitAcquisitionKey(args[0]);
      break;
    case 'scan': {
      if (args.length !== 2) throw new Error('usage: scan <base-sha|all> <head-sha>');
      const [base, head] = args;
      exactSha(head);
      const paths = base === 'all' ? [] : run('git', ['diff', '--name-only', '--no-renames', '-z', `${exactSha(base)}...${head}`], true).split('\0').filter(Boolean);
      const plan = planChanges(paths, { full: base === 'all' });
      output('docs-only', String(plan.docsOnly));
      output('unit-matrix', { shard: plan.unitShards });
      output('conduitos-proof', String(plan.conduitosProof));
      output('matrix', { include: TARGETS.filter(item => plan.families.includes(item.family)) });
      output('sha', head);
      break;
    }
    case 'preflight': {
      if (args.length !== 1) throw new Error('usage: preflight <base-sha>');
      const oneBodyEvidence = retainedOneBodyEvidence();
      if (oneBodyEvidence) {
        // The browser target has a shallow checkout. Verify ancestry here,
        // where CI retains the full history, before any expensive target work.
        run('git', ['merge-base', '--is-ancestor', oneBodyEvidence.sourceCommit, 'HEAD']);
      }
      run('git', ['diff', '--check', exactSha(args[0]), 'HEAD']);
      run('cargo', ['fmt', '--all', '--check']);
      run('cargo', ['metadata', '--locked', '--no-deps', '--format-version', '1'], true);
      run('cargo', ['xtask', 'ci', 'standalone-locks', '--locked']);
      const specs = readdirSync('proof/ci').filter(name => name.startsWith('pipeline-') && name.endsWith('.spec.mjs')).map(name => `proof/ci/${name}`);
      run('node', ['--test', ...specs]);
      // Syntax validation is required in CI, not a silently optional dependency.
      run(resolve('target/ci-tools/actionlint'), []);
      break;
    }
    case 'setup':
      if (args.length !== 1) throw new Error('usage: setup <target>');
      target(args[0]);
      await measureAcquisition(args[0], () => setupTarget(args[0]));
      break;
    case 'target': {
      if (args.length !== 2) throw new Error('usage: target <target> <sha>');
      const item = target(args[0]);
      const sha = exactSha(args[1]);
      assertSourceCheckout(sha);
      const directory = resolve('target/pipeline', item.id);
      if (existsSync(directory)) throw new Error(`refusing stale target output: ${directory}`);
      mkdirSync(directory, { recursive: true });
      await runTarget(item.id, directory);
      assertSourceCheckout(sha);
      await sealTarget({ directory, target: item.id, sha, proofClass: item.proofClass });
      console.log(`PASS: ${item.id} (${item.proofClass}) at ${sha}`);
      break;
    }
    case 'verify':
      if (args.length !== 2) throw new Error('usage: verify <bundle-directory> <sha>');
      await verifyBundle({ directory: resolve(args[0]), sha: exactSha(args[1]), targets: TARGETS });
      console.log('All required target artifacts and source identities verified.');
      break;
    case 'publish': {
      const { publish } = await import('./publish.mjs');
      if (args.length !== 2) throw new Error('usage: publish <integration-run-id> <bundle-directory>');
      await publish(args[0], resolve(args[1]));
      break;
    }
    default: throw new Error('usage: cargo xtask ci pipeline <scan|preflight|unit|setup|target|verify|publish>');
  }
} catch (error) {
  console.error(`FAILED: pipeline/${command}: ${error.message}`);
  process.exitCode = 1;
}
