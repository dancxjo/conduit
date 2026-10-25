// Optional compiler acceleration data. Never product or acceptance authority.
import { spawn, spawnSync } from 'node:child_process';
import { createReadStream } from 'node:fs';
import { createHash } from 'node:crypto';
import { lstat, mkdir, open, realpath, statfs, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { assertSourceCheckout } from './source.mjs';

const MAXIMUM = 4 * 1024 ** 3;
const ARCHIVE_LIMIT = MAXIMUM + 256 * 1024 ** 2;
const JSON_LIMIT = 32 * 1024 ** 2;
const helper = fileURLToPath(new URL('./hosted-cache-seed.py', import.meta.url));
const fail = message => { throw new Error(message); };
const hash = value => createHash('sha256').update(value).digest('hex');
const positive = value => Number.isSafeInteger(value) && value > 0;
const sha = value => typeof value === 'string' && /^[a-f0-9]{40}$/.test(value);
const schema = 'conduit.hosted-compiler-seed/v1';
const inside = (root, file) => file === root || file.startsWith(`${root}${path.sep}`);
export const hostedCacheSeedName = (namespace, attempt) => {
  if (!/^[a-f0-9]{64}$/.test(namespace ?? '') || !positive(attempt)) fail('Invalid compiler seed identity');
  return `compiler-cache-hosted-linux-${namespace}-attempt-${attempt}`;
};
function checkedPolicy(policy, namespace) {
  const text = JSON.stringify(policy);
  if (!policy || Buffer.byteLength(text) > 16384 || hash(text) !== namespace || !positive(policy.maxCacheBytes) || policy.maxCacheBytes > MAXIMUM) fail('Invalid compiler seed policy');
  return text;
}
async function freshDirectory(directory, exclude = []) {
  if (typeof directory !== 'string' || !path.isAbsolute(directory) || path.resolve(directory) !== directory
      || await realpath(path.dirname(directory)) !== path.dirname(directory)
      || exclude.some(root => inside(root, directory) || inside(directory, root))) fail('Fresh external compiler seed directory required');
  await mkdir(directory, { mode: 0o700 });
}
async function fileHash(file) {
  const stat = await lstat(file);
  if (!stat.isFile() || stat.nlink !== 1 || stat.isSymbolicLink() || stat.size > ARCHIVE_LIMIT) fail('Invalid compiler seed archive');
  const digest = createHash('sha256');
  for await (const bytes of createReadStream(file)) digest.update(bytes);
  return { bytes: stat.size, digest: `sha256:${digest.digest('hex')}` };
}
async function python(mode, request) {
  const result = await boundedSeedCommand('python3', [helper, mode], { input: JSON.stringify(request), maxBuffer: JSON_LIMIT, timeout: 10 * 60 * 1000 });
  if (result.status !== 0) fail(`Compiler seed ${mode} refused: ${result.stderr.slice(-2048)}`);
  return JSON.parse(result.stdout);
}
function producer(run, repository, source, completed = true) {
  if (!positive(run?.id) || !positive(run.run_attempt) || !sha(source) || run.head_sha !== source
      || !/^[\w.-]+\/[\w.-]+$/.test(repository ?? '')
      || run.path !== '.github/workflows/integration.yml' || run.repository?.full_name !== repository
      || run.head_repository?.full_name !== repository || !positive(run.repository?.id)
      || run.head_repository?.id !== run.repository.id
      || !((run.event === 'push' && run.head_branch === 'dev')
        || (run.event === 'workflow_dispatch' && /^train\/[a-f0-9]{64}$/.test(run.head_branch ?? '')))
      || (completed && (run.status !== 'completed' || run.conclusion !== 'success'))) fail('Invalid compiler seed Integration producer');
}
function acceptedProducer(accepted) {
  const { repository, source, tree, run, acceptanceRun } = accepted ?? {};
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository ?? '') || !sha(tree)) fail('Authenticated accepted compiler producer required');
  producer(run, repository, source);
  producer(acceptanceRun, repository, source);
  if (acceptanceRun.event !== 'push' || acceptanceRun.head_branch !== 'dev'
      || acceptanceRun.repository.id !== run.repository.id) fail('Compiler seed lacks accepted dev identity');
  // Caller must resolve acceptance/adoption using the existing source-bound
  // verifier. These identities are cross-checks, not a replacement for it.
  return { repository, source, tree, run, acceptanceRun };
}
function artifactIdentity(artifact, accepted, namespace) {
  const run = accepted.run, owner = artifact?.workflow_run;
  if (!positive(artifact?.id) || artifact.name !== hostedCacheSeedName(namespace, run.run_attempt)
      || artifact.expired !== false || !positive(artifact.size_in_bytes) || artifact.size_in_bytes > ARCHIVE_LIMIT
      || !/^sha256:[a-f0-9]{64}$/.test(artifact.digest ?? '') || owner?.id !== run.id
      || owner.head_sha !== accepted.source || owner.repository_id !== run.repository.id
      || owner.head_repository_id !== run.repository.id) fail('Invalid accepted compiler seed artifact');
  return artifact;
}
function apiReader(repository, command) {
  return async endpoint => {
    const result = await command('gh', ['api', `repos/${repository}/${endpoint}`], { maxBuffer: 2 * 1024 ** 2 });
    if (result.status !== 0 || result.error) fail('Compiler seed API observation failed');
    return JSON.parse(result.stdout);
  };
}

export async function sealHostedCacheSeed({ cacheRoot, policy, namespace, source, tree, run,
  directory, repositoryRoot = process.cwd() }) {
  const policyText = checkedPolicy(policy, namespace);
  producer(run, run?.repository?.full_name, source, false);
  if (!sha(tree)) fail('Invalid compiler seed source tree');
  if (typeof cacheRoot !== 'string' || !path.isAbsolute(cacheRoot) || await realpath(cacheRoot) !== cacheRoot) fail('Canonical external cache root required');
  const checkout = await realpath(repositoryRoot);
  assertSourceCheckout(source, checkout);
  const checkedTree = spawnSync('git', ['--no-replace-objects', 'rev-parse', `${source}^{tree}`], { cwd: checkout, encoding: 'utf8', timeout: 10000 });
  if (checkedTree.status !== 0 || checkedTree.stdout.trim() !== tree) fail('Compiler seed checkout tree differs');
  if (inside(checkout, cacheRoot) || inside(cacheRoot, checkout)) fail('Compiler cache must be external');
  await freshDirectory(directory, [cacheRoot, checkout]);
  const archive = path.join(directory, 'seed.tar');
  const receipt = await python('seal', { cacheRoot, policyText, maximum: policy.maxCacheBytes, archive,
    receipt: { schema, namespace, source, tree, runId: run.id, runAttempt: run.run_attempt,
      repository: run.repository.full_name, repositoryId: run.repository.id, job: 'target', target: 'hosted-linux' } });
  const identity = await fileHash(archive);
  assertSourceCheckout(source, checkout);
  await writeFile(path.join(directory, 'seal.json'), `${JSON.stringify({ ...receipt, archive: identity })}\n`, { flag: 'wx', mode: 0o600 });
  return { name: hostedCacheSeedName(namespace, run.run_attempt), archive, receipt, ...identity };
}

export async function acquireHostedCacheSeed({ accepted, policy, namespace, artifactId,
  directory, repositoryRoot = process.cwd(), command = boundedSeedCommand }) {
  const observed = acceptedProducer(accepted), policyText = checkedPolicy(policy, namespace);
  if (!positive(artifactId)) fail('Exact compiler seed artifact ID required');
  const api = apiReader(observed.repository, command);
  const refresh = async () => {
    for (const initial of [observed.run, observed.acceptanceRun]) {
      const actual = await api(`actions/runs/${initial.id}`);
      producer(actual, observed.repository, observed.source);
      if (actual.id !== initial.id || actual.run_attempt !== initial.run_attempt
          || actual.event !== initial.event || actual.head_branch !== initial.head_branch
          || actual.repository.id !== initial.repository.id) fail('Compiler seed producer attempt changed');
    }
  };
  await refresh();
  const artifact = artifactIdentity(await api(`actions/artifacts/${artifactId}`), observed, namespace);
  if (artifact.id !== artifactId) fail('Compiler seed API artifact ID differs');
  const checkout = await realpath(repositoryRoot);
  const space = await statfs(path.dirname(directory), { bigint: true });
  if (space.bavail * space.bsize < BigInt(artifact.size_in_bytes + policy.maxCacheBytes) + 2n * 1024n ** 3n) fail('Compiler seed reserve unavailable');
  await freshDirectory(directory, [checkout]);
  const archive = path.join(directory, 'artifact.zip');
  const fd = await open(archive, 'wx', 0o600);
  try {
    const result = await command('gh', ['api', `repos/${observed.repository}/actions/artifacts/${artifactId}/zip`],
      { output: fd, maximumBytes: artifact.size_in_bytes, maxBuffer: 65536, timeout: 10 * 60 * 1000 });
    if (result.status !== 0 || result.error) fail('Compiler seed download failed');
  } finally { await fd.close(); }
  const downloaded = await fileHash(archive);
  if (downloaded.bytes !== artifact.size_in_bytes || downloaded.digest !== artifact.digest) fail('Compiler seed archive digest/size differs');
  await refresh();
  const current = artifactIdentity(await api(`actions/artifacts/${artifactId}`), observed, namespace);
  if (current.digest !== artifact.digest || current.size_in_bytes !== artifact.size_in_bytes) fail('Compiler seed artifact changed');
  const receipt = await python('restore', { archive, directory: path.join(directory, 'restored'), policyText,
    maximum: policy.maxCacheBytes, expected: { schema, namespace, source: observed.source, tree: observed.tree,
      runId: observed.run.id, runAttempt: observed.run.run_attempt, repository: observed.repository,
      repositoryId: observed.run.repository.id, job: 'target', target: 'hosted-linux' } });
  await refresh();
  const result = { schema: 'conduit.hosted-seed-acquisition/v1', artifactId, name: artifact.name,
    namespace, ...downloaded, receipt, cacheRoot: path.join(directory, 'restored/cache') };
  await writeFile(path.join(directory, 'acquisition.json'), `${JSON.stringify(result)}\n`, { flag: 'wx', mode: 0o600 });
  return result;
}

export async function retireHostedCacheSeeds({ repository, namespace, artifactIds, directory, command = boundedSeedCommand }) {
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository ?? '') || !/^[a-f0-9]{64}$/.test(namespace ?? '')
      || !Array.isArray(artifactIds) || !artifactIds.length || artifactIds.length > 100
      || artifactIds.some(id => !positive(id)) || new Set(artifactIds).size !== artifactIds.length) fail('Explicit bounded compiler seed retirement required');
  const api = apiReader(repository, command), artifacts = [];
  for (const id of artifactIds) {
    const artifact = await api(`actions/artifacts/${id}`);
    const match = new RegExp(`^compiler-cache-hosted-linux-${namespace}-attempt-([1-9][0-9]*)$`).exec(artifact.name ?? '');
    if (artifact.id !== id || !match || !positive(Number(match[1])) || !positive(artifact.workflow_run?.id)
        || !sha(artifact.workflow_run?.head_sha) || !positive(artifact.size_in_bytes) || artifact.size_in_bytes > ARCHIVE_LIMIT
        || !/^sha256:[a-f0-9]{64}$/.test(artifact.digest ?? '')) fail('Refusing nonmanaged compiler artifact retirement');
    const run = await api(`actions/runs/${artifact.workflow_run.id}`);
    producer(run, repository, artifact.workflow_run.head_sha, false);
    if (run.id !== artifact.workflow_run.id || run.status !== 'completed' || run.run_attempt < Number(match[1])
        || artifact.workflow_run.repository_id !== run.repository.id
        || artifact.workflow_run.head_repository_id !== run.repository.id) fail('Compiler seed retirement producer differs');
    artifacts.push({ id, name: artifact.name, digest: artifact.digest, runId: artifact.workflow_run.id });
  }
  await freshDirectory(directory, [await realpath(process.cwd())]);
  const intent = { schema: 'conduit.hosted-seed-retirement/v1', repository, namespace, artifacts, completeNamespaceClear: false };
  const handle = await open(path.join(directory, 'intent.json'), 'wx', 0o600);
  try { await handle.writeFile(`${JSON.stringify(intent)}\n`); await handle.sync(); } finally { await handle.close(); }
  const parent = await open(directory, 'r');
  try { await parent.sync(); } finally { await parent.close(); }
  for (const artifact of artifacts) {
    const deleted = await command('gh', ['api', `repos/${repository}/actions/artifacts/${artifact.id}`, '--method', 'DELETE'], { maxBuffer: 65536 });
    if (deleted.status !== 0 || deleted.error) fail('Compiler seed retirement refused; intent retained');
    const readback = await command('gh', ['api', `repos/${repository}/actions/artifacts/${artifact.id}`, '--include'], { maxBuffer: 65536 });
    if (readback.status === 0 || !/^HTTP\/[\d.]+ 404(?:\s|$)/m.test(readback.stdout ?? '')) fail('Compiler seed retirement not confirmed absent');
  }
  await writeFile(path.join(directory, 'retirement.json'), `${JSON.stringify(intent)}\n`, { flag: 'wx', mode: 0o600 });
  return intent;
}

export async function boundedSeedCommand(program, args, options = {}) {
  const child = spawn(program, args, { stdio: ['pipe', 'pipe', 'pipe'] });
  const chunks = [], errors = []; let bytes = 0, stderrBytes = 0, failure;
  const done = new Promise(resolve => { child.once('error', error => { failure = error; resolve(null); }); child.once('close', resolve); });
  const timer = setTimeout(() => { failure = new Error('Compiler seed command timeout'); child.kill('SIGKILL'); }, options.timeout ?? 60000);
  child.stdin.on('error', () => {}); child.stdin.end(options.input);
  child.stderr.on('data', chunk => {
    stderrBytes += chunk.length;
    if (stderrBytes > (options.maxBuffer ?? 65536)) { failure = new Error('Compiler seed stderr bound'); child.kill('SIGKILL'); }
    else errors.push(chunk);
  });
  try {
    for await (const chunk of child.stdout) {
      bytes += chunk.length;
      if (bytes > (options.maximumBytes ?? options.maxBuffer ?? 65536)) fail('Compiler seed command byte bound');
      if (options.output) {
        let offset = 0;
        while (offset < chunk.length) { const result = await options.output.write(chunk, offset, chunk.length - offset); if (!result.bytesWritten) fail('Compiler seed write stalled'); offset += result.bytesWritten; }
      } else chunks.push(chunk);
    }
    const status = await done;
    if (failure) throw failure;
    return { status, stdout: Buffer.concat(chunks).toString(), stderr: Buffer.concat(errors).toString() };
  } finally { clearTimeout(timer); if (child.exitCode === null) child.kill('SIGKILL'); await done; }
}
