import { spawnSync } from 'node:child_process';
import { lstat, readFile, realpath, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { assertSourceCheckout } from '../pipeline/source.mjs';
import { prepareCompositorSnapshot, hashFile, snapshotInventory } from './cargo-snapshot.mjs';
import { runCompositorCargoCache, clearCompositorCargoCache } from './cargo-sandbox.mjs';

const fail = reason => { throw new Error(reason); };
const inside = (root, file) => file === root || file.startsWith(`${root}${path.sep}`);
const git = args => {
  const result = spawnSync('git', args, { encoding: 'utf8', maxBuffer: 2 * 1024 * 1024 });
  if (result.error || result.status !== 0) fail('Compiler cache source observation failed');
  return result.stdout.trim();
};
async function fresh(file) {
  try { await lstat(file); fail('Compiler cache output exists'); }
  catch (error) { if (error.code !== 'ENOENT') throw error; }
}

async function verifyGitSource(snapshot) {
  const prefix = snapshot.sourcePath;
  if (!/^[a-f0-9]{40}$/.test(snapshot.sourceCommit ?? '') || !/^[a-f0-9]{40}$/.test(snapshot.sourceTree ?? '') ||
      prefix !== 'targets/conduitos/compositor-core' ||
      git(['rev-parse', `${snapshot.sourceCommit}:${prefix}`]) !== snapshot.sourceTree) fail('Missing compiler cache Git custody');
  const tracked = git(['ls-tree', '-r', '--name-only', `${snapshot.sourceCommit}:${prefix}`]).split('\n').filter(Boolean);
  const inventory = await snapshotInventory(path.join(snapshot.root, 'source'));
  const copied = inventory.files.filter(file => file.path !== 'Cargo.lock');
  if (JSON.stringify(copied.map(file => file.path).sort()) !== JSON.stringify([...tracked].sort())) fail('Snapshot includes source outside its Git tree');
  for (const file of copied) {
    const result = spawnSync('git', ['show', `${snapshot.sourceCommit}:${prefix}/${file.path}`], { maxBuffer: 2 * 1024 * 1024 });
    if (result.error || result.status !== 0 || createHash('sha256').update(result.stdout).digest('hex') !== file.sha256 ||
        await hashFile(path.join(snapshot.root, 'source', file.path)) !== file.sha256) fail('Copied source differs from Git source');
  }
}

export async function prepareCargoCache(engine, toolchainRoot, destination, manifestFile) {
  for (const file of [engine, toolchainRoot, destination, manifestFile]) if (!path.isAbsolute(file)) fail('Absolute cache paths required');
  destination = path.join(await realpath(path.dirname(destination)), path.basename(destination));
  manifestFile = path.join(await realpath(path.dirname(manifestFile)), path.basename(manifestFile));
  const checkout = await realpath(process.cwd());
  if (inside(checkout, destination) || inside(checkout, manifestFile) || inside(destination, manifestFile)) fail('Snapshots and manifests must be outside source');
  await fresh(manifestFile);
  const sourceCommit = git(['rev-parse', 'HEAD']);
  assertSourceCheckout(sourceCommit);
  const prefix = 'targets/conduitos/compositor-core';
  const sourceTree = git(['rev-parse', `${sourceCommit}:${prefix}`]);
  const snapshot = await prepareCompositorSnapshot({ packageRoot: path.resolve(prefix), toolchainRoot, engine, destination });
  // Compare every copied source input with exact tracked Git bytes. The generated
  // dependency-free standalone Cargo.lock has separate snapshot custody.
  const manifest = { ...snapshot, sourceCommit, sourceTree, sourcePath: prefix };
  await verifyGitSource(manifest);
  assertSourceCheckout(sourceCommit);
  await writeFile(manifestFile, `${JSON.stringify(manifest, null, 2)}\n`, { flag: 'wx', mode: 0o600 });
  return manifest;
}

export async function buildCargoCache(manifestFile, cacheRoot, taskRoot) {
  const stat = await lstat(manifestFile);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 2 * 1024 * 1024) fail('Invalid compiler cache manifest');
  const snapshot = JSON.parse(await readFile(manifestFile, 'utf8'));
  await verifyGitSource(snapshot);
  return runCompositorCargoCache({ snapshot, cacheRoot, taskRoot });
}

export async function clearCargoCache(cacheRoot, expectedNamespace, receiptFile) {
  if (!path.isAbsolute(receiptFile) || !path.isAbsolute(cacheRoot)) fail('External absolute eviction receipt required');
  cacheRoot = await realpath(cacheRoot);
  receiptFile = path.join(await realpath(path.dirname(receiptFile)), path.basename(receiptFile));
  if (inside(cacheRoot, receiptFile)) fail('External absolute eviction receipt required');
  await fresh(receiptFile);
  const receipt = await clearCompositorCargoCache({ cacheRoot, expectedNamespace });
  await writeFile(receiptFile, `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx', mode: 0o600 });
  return receipt;
}
