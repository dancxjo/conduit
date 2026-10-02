import { appendFileSync, mkdirSync, readdirSync, existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { planChanges } from './plan.mjs';
import { sealTarget, verifyBundle } from './receipts.mjs';
import { TARGETS, setupTarget, runTarget } from './targets.mjs';
import { setupCi, setupUnit } from './setup.mjs';

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
      await setupCi();
      break;
    case 'setup-unit':
      if (args.length) throw new Error('setup-unit accepts no arguments');
      setupUnit();
      break;
    case 'scan': {
      if (args.length !== 2) throw new Error('usage: scan <base-sha|all> <head-sha>');
      const [base, head] = args;
      exactSha(head);
      const paths = base === 'all' ? [] : run('git', ['diff', '--name-only', '--no-renames', '-z', `${exactSha(base)}...${head}`], true).split('\0').filter(Boolean);
      const plan = planChanges(paths, { full: base === 'all' });
      output('docs-only', String(plan.docsOnly));
      output('matrix', { include: TARGETS.filter(item => plan.families.includes(item.family)) });
      output('sha', head);
      break;
    }
    case 'preflight': {
      if (args.length !== 1) throw new Error('usage: preflight <base-sha>');
      run('git', ['diff', '--check', exactSha(args[0]), 'HEAD']);
      run('cargo', ['fmt', '--all', '--check']);
      run('cargo', ['metadata', '--locked', '--no-deps', '--format-version', '1'], true);
      const specs = readdirSync('proof/ci').filter(name => name.startsWith('pipeline-') && name.endsWith('.spec.mjs')).map(name => `proof/ci/${name}`);
      run('node', ['--test', ...specs]);
      // Syntax validation is required in CI, not a silently optional dependency.
      run(resolve('target/ci-tools/actionlint'), []);
      break;
    }
    case 'setup':
      if (args.length !== 1) throw new Error('usage: setup <target>');
      target(args[0]);
      await setupTarget(args[0]);
      break;
    case 'target': {
      if (args.length !== 2) throw new Error('usage: target <target> <sha>');
      const item = target(args[0]);
      const sha = exactSha(args[1]);
      if (run('git', ['rev-parse', 'HEAD'], true) !== sha) throw new Error('target checkout does not match source SHA');
      run('git', ['diff', '--exit-code', 'HEAD', '--']);
      const directory = resolve('target/pipeline', item.id);
      if (existsSync(directory)) throw new Error(`refusing stale target output: ${directory}`);
      mkdirSync(directory, { recursive: true });
      await runTarget(item.id, directory);
      run('git', ['diff', '--exit-code', 'HEAD', '--']);
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
