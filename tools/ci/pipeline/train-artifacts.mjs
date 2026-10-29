// Acquire immutable Actions archives. Extraction and landing are separate boundaries.
import { spawn } from 'node:child_process';
import { createReadStream } from 'node:fs';
import { lstat, mkdir, open, realpath, statfs, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { verifyTrainRun } from './train.mjs';
import { TARGETS } from './targets.mjs';

const MAX_ARCHIVE = 4 * 1024 ** 3;
const MAX_TOTAL = 32 * 1024 ** 3;
const fail = message => { throw new Error(message); };
const positive = value => Number.isSafeInteger(value) && value > 0;

export async function acquireTrainArtifacts({ manifest, observation, directory,
  command = boundedCommand, targets = TARGETS, validate = verifyTrainRun }) {
  const verified = validate(manifest, observation);
  const initial = observation.run;
  if (!positive(initial.run_attempt) || !positive(initial.repository?.id)
      || !positive(initial.head_repository?.id)) fail('Missing run attempt or repository identity');
  const base = `repos/${verified.repository}`;
  const api = async endpoint => {
    const result = await command('gh', ['api', endpoint], { encoding: 'utf8', maxBuffer: 2 * 1024 ** 2 });
    if (result.error || result.status !== 0) fail('Artifact API observation failed');
    return JSON.parse(result.stdout);
  };
  const refresh = async () => {
    const run = await api(`${base}/actions/runs/${verified.integrationRun}`);
    validate(manifest, { ...observation, run });
    if (run.id !== initial.id || run.run_attempt !== initial.run_attempt
        || run.repository?.id !== initial.repository.id
        || run.head_repository?.id !== initial.head_repository.id) fail('Integration run attempt changed');
  };
  await refresh();
  const inventory = await api(`${base}/actions/runs/${verified.integrationRun}/artifacts?per_page=100`);
  if (!Number.isSafeInteger(inventory.total_count) || inventory.total_count < 0
      || inventory.total_count > 100 || !Array.isArray(inventory.artifacts)
      || inventory.artifacts.length !== inventory.total_count) fail('Incomplete bounded artifact inventory');
  const expected = new Set(targets.map(target => `product-${target.id}`));
  if (!expected.size || expected.size !== targets.length || expected.size > 32
      || [...expected].some(name => !/^product-[a-z0-9][a-z0-9_-]*$/.test(name))) fail('Invalid required product inventory');
  const products = inventory.artifacts.filter(artifact => artifact.name?.startsWith('product-'));
  const ids = new Set(); const names = new Set(); let total = 0;
  for (const artifact of products) {
    const owner = artifact.workflow_run;
    if (!expected.has(artifact.name) || names.has(artifact.name) || !positive(artifact.id)
        || ids.has(artifact.id) || artifact.expired !== false
        || !positive(artifact.size_in_bytes) || artifact.size_in_bytes > MAX_ARCHIVE
        || !/^sha256:[a-f0-9]{64}$/.test(artifact.digest ?? '')
        || owner?.id !== initial.id || owner.head_sha !== verified.source
        || owner.repository_id !== initial.repository.id
        || owner.head_repository_id !== initial.head_repository.id) fail('Invalid run-bound product artifact');
    names.add(artifact.name); ids.add(artifact.id); total += artifact.size_in_bytes;
  }
  if (products.length !== expected.size || total > MAX_TOTAL) fail('Missing or oversized train products');
  if (!path.isAbsolute(directory) || await realpath(path.dirname(directory)) !== path.dirname(directory)) fail('Fresh absolute real-parent output required');
  const space = await statfs(path.dirname(directory), { bigint: true });
  if (space.bavail * space.bsize < BigInt(total) + 1024n ** 3n) fail('Insufficient artifact acquisition headroom');
  await mkdir(directory); // Existing destinations, including symlinks, refuse.
  const acquired = [];
  for (const artifact of products.sort((a, b) => a.name.localeCompare(b.name))) {
    const filename = `${artifact.name}.zip`;
    const destination = path.join(directory, filename);
    const handle = await open(destination, 'wx', 0o600);
    try {
      const result = await command('gh', ['api', `${base}/actions/artifacts/${artifact.id}/zip`],
        { output: handle, maximumBytes: artifact.size_in_bytes, maxBuffer: 65536, timeout: 10 * 60 * 1000 });
      if (result.error || result.status !== 0) fail('Artifact archive download failed; evidence retained');
    } finally { await handle.close(); }
    const stat = await lstat(destination);
    if (!stat.isFile() || stat.isSymbolicLink() || stat.size !== artifact.size_in_bytes) fail('Artifact archive size mismatch');
    let bytes = 0; const hash = createHash('sha256');
    for await (const chunk of createReadStream(destination)) {
      bytes += chunk.length;
      if (bytes > MAX_ARCHIVE || bytes > artifact.size_in_bytes) fail('Artifact archive exceeded bound');
      hash.update(chunk);
    }
    if (`sha256:${hash.digest('hex')}` !== artifact.digest) fail('Artifact archive digest mismatch');
    acquired.push({ name: artifact.name, id: artifact.id, archive: filename,
      bytes, digest: artifact.digest });
  }
  await refresh();
  const receipt = { schema: 'conduit.train-artifact-acquisition/v1', repository: verified.repository,
    repositoryId: initial.repository.id, manifestDigest: manifest.digest, source: verified.source,
    tree: verified.tree, runId: initial.id, runAttempt: initial.run_attempt, artifacts: acquired };
  await writeFile(path.join(directory, 'acquisition.json'), `${JSON.stringify(receipt, null, 2)}\n`,
    { flag: 'wx', mode: 0o600 });
  return receipt;
}

// Pin the trusted session's child credentials/cwd. Per-call output options
// cannot replace them, and later caller mutations do not rotate this command.
export function createBoundedTrainArtifactCommand({ env, directory } = {}) {
  const childEnvironment = env === undefined ? undefined : { ...env };
  return (program, args, options) => boundedCommand(program, args, options,
    { env: childEnvironment, cwd: directory });
}

// Byte ceilings apply while downloading, before disk can grow beyond admission.
async function boundedCommand(program, args, options, { env, cwd } = {}) {
  const child = spawn(program, args, { stdio: ['ignore', 'pipe', 'pipe'], env, cwd });
  let bytes = 0; let stderr = 0; const chunks = [];
  const timer = setTimeout(() => child.kill('SIGKILL'), options.timeout ?? 60000);
  const completion = new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('close', status => resolve(status));
  });
  // Spawn refusal can precede stdout closure; observe its rejection immediately
  // while preserving the original rejection for the command's awaited result.
  completion.catch(() => {});
  child.stderr.on('data', chunk => {
    stderr += chunk.length;
    if (stderr > options.maxBuffer) child.kill('SIGKILL');
  });
  try {
    for await (const chunk of child.stdout) {
      bytes += chunk.length;
      if (bytes > (options.maximumBytes ?? options.maxBuffer)) {
        child.kill('SIGKILL');
        fail('Artifact command byte bound exceeded');
      }
      if (options.output) {
        let offset = 0;
        while (offset < chunk.length) {
          const written = await options.output.write(chunk, offset, chunk.length - offset);
          if (!written.bytesWritten) fail('Artifact archive write made no progress');
          offset += written.bytesWritten;
        }
      } else chunks.push(chunk);
    }
    return { status: await completion, stdout: Buffer.concat(chunks).toString('utf8') };
  } finally {
    clearTimeout(timer);
    if (child.exitCode === null) child.kill('SIGKILL');
    await completion.catch(() => {});
  }
}
