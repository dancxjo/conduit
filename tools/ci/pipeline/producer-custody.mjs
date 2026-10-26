// Run-owned product provenance. Authenticity comes from the independently
// authenticated Actions archive; this JSON is not a signature or authority.
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { readFileSync, existsSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import os from 'node:os';

const hash = value => createHash('sha256').update(value).digest('hex');
const fail = message => { throw new Error(message); };
let active;

function invoke(program, args, directory) {
  const result = spawnSync(program, args, { cwd: directory, encoding: 'utf8',
    timeout: 60000, maxBuffer: 16 * 1024 * 1024 });
  if (result.error || result.status !== 0) fail(`Cannot observe producer identity: ${program}`);
  return result.stdout.trim();
}

export function beginProductCustody({ source, target, directory = process.cwd(), env = process.env,
  identity = (program, args) => invoke(program, args, directory) }) {
  if (active) fail('Nested product custody');
  if (!/^[a-f0-9]{40}$/.test(source) || !/^[a-z0-9][a-z0-9_-]*$/.test(target)) fail('Invalid producer source or target');
  const tree = identity('git', ['rev-parse', `${source}^{tree}`]);
  if (!/^[a-f0-9]{40}$/.test(tree)) fail('Invalid producer tree');
  const run = env.GITHUB_ACTIONS === 'true' ? {
    repository: env.GITHUB_REPOSITORY, repositoryId: Number(env.GITHUB_REPOSITORY_ID),
    id: Number(env.GITHUB_RUN_ID), attempt: Number(env.GITHUB_RUN_ATTEMPT),
    workflow: env.GITHUB_WORKFLOW_REF, workflowSource: env.GITHUB_WORKFLOW_SHA,
    job: env.GITHUB_JOB, source: env.GITHUB_SHA,
  } : null;
  if (run && (!/^[\w.-]+\/[\w.-]+$/.test(run.repository ?? '') ||
      [run.repositoryId, run.id, run.attempt].some(value => !Number.isSafeInteger(value) || value <= 0) ||
      run.source !== source || !/^[a-f0-9]{40}$/.test(run.workflowSource ?? '') ||
      typeof run.job !== 'string' || !run.job || typeof run.workflow !== 'string' || !run.workflow)) {
    fail('Incomplete Actions producer identity');
  }
  const acquisitionFile = path.join(directory, 'target/acquisition', `${target}.json`);
  let acquisition = null;
  if (existsSync(acquisitionFile)) {
    const bytes = readFileSync(acquisitionFile);
    if (bytes.length > 2 * 1024 * 1024) fail('Acquisition custody exceeds bound');
    const report = JSON.parse(bytes);
    if (report.target !== target || report.sourceSha !== source || report.outcome !== 'success') fail('Acquisition custody differs from source');
    acquisition = { sha256: hash(bytes), report };
  }
  if (run && !acquisition) fail('Actions product lacks verified tool acquisition');
  const manifestDigest = env.CONDUIT_TRAIN_MANIFEST_DIGEST || null;
  if (manifestDigest !== null && !/^[a-f0-9]{64}$/.test(manifestDigest)) fail('Invalid producer train manifest digest');
  // A complete source tree binds Cargo feature definitions, lockfiles, target
  // recipes and test source. Commands below bind the selected recipe/flags.
  // This is a command contract, not a claim that individual test names or every
  // nested rustc invocation were separately observed.
  active = {
    schema: 'conduit.pipeline-producer/v1', source, tree, target, run, manifestDigest,
    toolchain: { rustc: identity('rustc', ['--version', '--verbose']),
      cargo: identity('cargo', ['--version', '--verbose']), node: process.version, acquisition },
    environment: { sha256: hash(JSON.stringify(Object.keys(env).sort().map(key => [key, env[key]]))),
      platform: process.platform, arch: process.arch, kernel: os.release(),
      image: { os: env.ImageOS ?? null, version: env.ImageVersion ?? null } },
    selection: { inventory: 'source-bound-command-contract', policyTree: tree, commands: [] },
    started: performance.now(),
  };
}

export function recordProductCommand({ program, args, durationMs, outcome,
  environment = process.env, cwd = process.cwd() }) {
  if (!active) return;
  if (active.selection.commands.length >= 10000 || !Array.isArray(args) || args.length > 4096 ||
      Buffer.byteLength(JSON.stringify([program, args])) > 65536) fail('Producer command inventory exceeds bound');
  active.selection.commands.push({ program, args: [...args], durationMs, outcome, cwd,
    environmentSha256: hash(JSON.stringify(Object.keys(environment).sort().map(key => [key, environment[key]]))) });
}

export function finishProductCustody(directory) {
  if (!active) fail('No active product custody');
  const { started, ...receipt } = active;
  active = undefined;
  if (!receipt.selection.commands.length || receipt.selection.commands.some(command => command.outcome !== 'success')) fail('Product command contract did not succeed');
  receipt.durationMs = Math.round(performance.now() - started);
  const bytes = `${JSON.stringify(receipt, null, 2)}\n`;
  if (Buffer.byteLength(bytes) > 2 * 1024 * 1024) fail('Producer receipt exceeds bound');
  writeFileSync(path.join(directory, 'producer.json'), bytes, { flag: 'wx' });
  return receipt;
}

export function abandonProductCustody() { active = undefined; }

export function verifyProductCustody(receipt, { source, tree, target, run, manifestDigest }) {
  const manual = /^[a-f0-9]{64}$/.test(manifestDigest ?? '') &&
    run.event === 'workflow_dispatch' && /^train\/[a-f0-9]{64}$/.test(run.head_branch ?? '');
  const development = manifestDigest === null && run.event === 'push' && run.head_branch === 'dev';
  if (receipt?.schema !== 'conduit.pipeline-producer/v1' || receipt.source !== source ||
      receipt.tree !== tree || receipt.target !== target || receipt.manifestDigest !== manifestDigest ||
      (!manual && !development) || !receipt.run ||
      receipt.run.repository !== run.repository.full_name || receipt.run.repositoryId !== run.repository.id ||
      receipt.run.id !== run.id || receipt.run.attempt !== run.run_attempt || receipt.run.source !== source ||
      receipt.run.workflowSource !== source ||
      receipt.run.workflow !== `${run.repository.full_name}/.github/workflows/integration.yml@refs/heads/${run.head_branch}` ||
      typeof receipt.run.job !== 'string' || !receipt.run.job) fail('Product producer run identity mismatch');
  if (!receipt.toolchain?.rustc || !receipt.toolchain.cargo || !receipt.toolchain.node ||
      receipt.toolchain.acquisition?.report?.target !== target ||
      receipt.toolchain.acquisition.report.sourceSha !== source ||
      receipt.toolchain.acquisition.report.outcome !== 'success' ||
      receipt.toolchain.acquisition.sha256 !== hash(JSON.stringify(receipt.toolchain.acquisition.report, null, 2) + '\n')) {
    fail('Product toolchain acquisition custody mismatch');
  }
  if (!/^[a-f0-9]{64}$/.test(receipt.environment?.sha256 ?? '') || !receipt.environment.platform ||
      !receipt.environment.arch || !receipt.environment.kernel || !receipt.environment.image?.os ||
      !receipt.environment.image.version || receipt.selection?.policyTree !== tree ||
      receipt.selection.inventory !== 'source-bound-command-contract' ||
      !Array.isArray(receipt.selection.commands) || !receipt.selection.commands.length ||
      receipt.selection.commands.some(command => !command.program || !Array.isArray(command.args) ||
        typeof command.cwd !== 'string' || !command.cwd || !/^[a-f0-9]{64}$/.test(command.environmentSha256 ?? '') ||
        command.outcome !== 'success' || !Number.isFinite(command.durationMs) || command.durationMs < 0)) {
    fail('Product execution/environment custody mismatch');
  }
  return receipt;
}
