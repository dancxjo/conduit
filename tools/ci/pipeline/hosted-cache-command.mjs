// Source-bound orchestration; compiler objects never stand in for product proof.
import { spawnSync } from 'node:child_process';
import { constants } from 'node:fs';
import { lstat, open, realpath, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { assertSourceCheckout } from './source.mjs';

const LIMIT = 32 * 1024 ** 2;
const MODE = 'audited-dependencies';
const SCHEMA = 'conduit.hosted-cache-target/v1';
const DEFAULT_BYTES = 4 * 1024 ** 3;
const fail = message => { throw new Error(message); };
const inside = (root, file) => file === root || file.startsWith(`${root}${path.sep}`);
const sha = value => typeof value === 'string' && /^[a-f0-9]{40}$/.test(value);
function git(root, args) {
  const result = spawnSync('git', ['--no-replace-objects', ...args], { cwd: root, encoding: 'utf8', maxBuffer: 1024 ** 2 });
  if (result.error || result.status !== 0) fail('Hosted cache source observation failed');
  return result.stdout.trim();
}
async function external(file, checkout) {
  if (typeof file !== 'string' || !path.isAbsolute(file) || path.resolve(file) !== file) fail('Canonical absolute hosted cache paths required');
  const parent = await realpath(path.dirname(file));
  const forbidden = [checkout, git(checkout, ['rev-parse', '--absolute-git-dir']),
    git(checkout, ['rev-parse', '--path-format=absolute', '--git-common-dir'])];
  if (parent !== path.dirname(file) || forbidden.some(root => inside(root, file) || inside(file, root))) fail('Hosted cache paths must be outside source and Git metadata without symlinks');
  try { if ((await lstat(file)).isSymbolicLink()) fail('Hosted cache symlink refused'); }
  catch (error) { if (error.code !== 'ENOENT') throw error; }
  return file;
}
async function fresh(file) {
  try { await lstat(file); fail('Hosted cache output exists'); }
  catch (error) { if (error.code !== 'ENOENT') throw error; }
}
function disjoint(paths) {
  if (paths.some((file, index) => paths.some((other, otherIndex) => index !== otherIndex && inside(file, other)))) fail('Overlapping hosted cache paths');
}
function policyOptions(cacheMode, maxCacheBytes) {
  if (cacheMode !== MODE || !Number.isSafeInteger(maxCacheBytes) || maxCacheBytes <= 0 || maxCacheBytes > DEFAULT_BYTES) fail('Unsupported hosted cache policy');
}
function source(root, sourceSha) {
  const commit = git(root, ['rev-parse', 'HEAD']);
  if (sourceSha !== undefined && (!sha(sourceSha) || sourceSha !== commit)) fail('Hosted cache source SHA differs');
  assertSourceCheckout(commit, root);
  return { sourceCommit: commit, sourceTree: git(root, ['rev-parse', 'HEAD^{tree}']) };
}
function snapshotSource(snapshot, identity) {
  if (snapshot?.schema !== 'hosted-linux-cargo-snapshot-v1' || snapshot.sourceCommit !== identity.sourceCommit || snapshot.sourceTree !== identity.sourceTree) fail('Hosted cache snapshot source/tree differs');
}
async function readJson(file) {
  const fd = await open(file, constants.O_RDONLY | constants.O_NOFOLLOW);
  try {
    const stat = await fd.stat();
    if (!stat.isFile() || stat.nlink !== 1 || stat.size < 2 || stat.size > LIMIT) fail('Invalid or oversized hosted cache JSON');
    const bytes = Buffer.alloc(stat.size + 1);
    let count = 0;
    while (count < bytes.length) {
      const read = await fd.read(bytes, count, bytes.length - count, null);
      if (!read.bytesRead) break;
      count += read.bytesRead;
    }
    if (count !== stat.size) fail('Hosted cache JSON changed while reading');
    return JSON.parse(bytes.subarray(0, count).toString('utf8'));
  } finally { await fd.close(); }
}

export async function prepareHostedProductCache({ engine, toolchainRoot, destination, manifestFile,
  cacheRoot, taskRoot, acquisitionFile, configFile, repositoryRoot = process.cwd(), sourceSha,
  cargoHome, cacheMode = MODE, maxCacheBytes = DEFAULT_BYTES }, dependencies = {}) {
  const checkout = await realpath(repositoryRoot);
  policyOptions(cacheMode, maxCacheBytes);
  const identity = source(checkout, sourceSha);
  for (const file of [engine, toolchainRoot]) if (typeof file !== 'string' || !path.isAbsolute(file)) fail('Absolute engine/toolchain paths required');
  acquisitionFile ??= `${destination}.acquisition.json`;
  const outputs = [destination, manifestFile, cacheRoot, taskRoot, acquisitionFile, ...(configFile ? [configFile] : [])];
  for (const file of outputs) await external(file, checkout);
  disjoint(outputs);
  for (const file of [destination, manifestFile, taskRoot, acquisitionFile, ...(configFile ? [configFile] : [])]) await fresh(file);
  const prepareSnapshot = dependencies.prepareSnapshot ?? (await import('../compiler-cache/hosted-snapshot.mjs')).prepareHostedSnapshot;
  const derivePolicy = dependencies.derivePolicy ?? (await import('../compiler-cache/cargo-sandbox.mjs')).hostedCachePolicy;
  if (typeof derivePolicy !== 'function') fail('Hosted cache policy implementation unavailable');
  const snapshot = await prepareSnapshot({ repositoryRoot: checkout, toolchainRoot, engine, destination,
    cacheMode, acquisitionFile, ...(cargoHome ? { cargoHome } : {}) });
  snapshotSource(snapshot, identity);
  if (snapshot.cacheMode !== cacheMode) fail('Hosted cache snapshot mode differs');
  if (snapshot.root !== destination || await realpath(destination) !== destination) fail('Hosted cache snapshot root differs');
  const { policy, namespace } = await derivePolicy(snapshot, { maxCacheBytes, cacheMode });
  if (!policy || !/^[a-f0-9]{64}$/.test(namespace ?? '')) fail('Invalid hosted cache namespace');
  source(checkout, identity.sourceCommit);
  const bytes = `${JSON.stringify(snapshot)}\n`;
  if (Buffer.byteLength(bytes) > LIMIT) fail('Hosted cache manifest exceeds bound');
  await writeFile(manifestFile, bytes, { flag: 'wx', mode: 0o600 });
  const config = { schema: SCHEMA, snapshotManifest: manifestFile, cacheRoot, taskRoot, cacheMode, maxCacheBytes };
  if (configFile) await writeFile(configFile, `${JSON.stringify(config)}\n`, { flag: 'wx', mode: 0o600 });
  return { snapshot, policy, namespace, manifestFile, configFile,
    managedPaths: [path.join(cacheRoot, 'policy.json'), path.join(cacheRoot, 'objects')],
    config };
}

async function checkedConfiguration(config, configFile, checkout, identity, completedTask) {
  if (typeof completedTask !== 'boolean') fail('Invalid hosted task phase');
  const fields = ['schema', 'snapshotManifest', 'cacheRoot', 'taskRoot', 'cacheMode', 'maxCacheBytes'];
  if (!config || Object.keys(config).sort().join() !== fields.sort().join() || config.schema !== SCHEMA) fail('Invalid hosted cache target config');
  policyOptions(config.cacheMode, config.maxCacheBytes);
  for (const file of [config.snapshotManifest, config.cacheRoot, config.taskRoot]) await external(file, checkout);
  const snapshot = await readJson(config.snapshotManifest);
  snapshotSource(snapshot, identity);
  if (snapshot.cacheMode !== config.cacheMode) fail('Hosted cache snapshot mode differs');
  await external(snapshot.root, checkout);
  if (await realpath(snapshot.root) !== snapshot.root) fail('Hosted cache snapshot root differs');
  disjoint([configFile, config.snapshotManifest, config.cacheRoot, config.taskRoot, snapshot.root]);
  if (completedTask) {
    const task = await lstat(config.taskRoot);
    if (!task.isDirectory() || task.isSymbolicLink() || task.uid !== process.getuid()) fail('Invalid completed hosted task');
  } else await fresh(config.taskRoot);
  source(checkout, identity.sourceCommit);
  return { snapshot, snapshotManifest: config.snapshotManifest, cacheRoot: config.cacheRoot, taskRoot: config.taskRoot,
    cacheMode: config.cacheMode, maxCacheBytes: config.maxCacheBytes };
}

export async function loadHostedProductCacheConfig(configFile, { repositoryRoot = process.cwd(), sourceSha, completedTask = false } = {}) {
  const checkout = await realpath(repositoryRoot);
  const identity = source(checkout, sourceSha);
  await external(configFile, checkout);
  return checkedConfiguration(await readJson(configFile), configFile, checkout, identity, completedTask);
}

// Seed acquisition creates a new configuration, retaining the original intent.
// It cannot overwrite one or redirect the private task into shared storage.
export async function writeHostedProductCacheConfig(config, configFile, { repositoryRoot = process.cwd(), sourceSha } = {}) {
  const checkout = await realpath(repositoryRoot);
  const identity = source(checkout, sourceSha);
  await external(configFile, checkout);
  await fresh(configFile);
  await checkedConfiguration(config, configFile, checkout, identity, false);
  await writeFile(configFile, `${JSON.stringify(config)}\n`, { flag: 'wx', mode: 0o600 });
  return configFile;
}
