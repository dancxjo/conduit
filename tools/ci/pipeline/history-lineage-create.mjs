import { spawnSync } from 'node:child_process';
import { lstat, readFile, realpath, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { createCaptureLineagePolicy } from './history-lineage.mjs';

// Deliberately does not install a policy or change a ref. Cutover ownership
// reviews/commits this output only after complete source/map/backup proof.
export async function prepareCaptureLineagePolicy({ originalDirectory, mappedDirectory,
  mapFile, capturesFile, custodyFile, outputFile, command = spawnSync }) {
  const absolute = [originalDirectory, mappedDirectory, mapFile, capturesFile, custodyFile, outputFile];
  if (!absolute.every(value => typeof value === 'string' && path.isAbsolute(value))) throw new Error('Absolute lineage input/output paths required');
  originalDirectory = await realpath(originalDirectory); mappedDirectory = await realpath(mappedDirectory);
  const destination = path.join(await realpath(path.dirname(outputFile)), path.basename(outputFile));
  if ([originalDirectory, mappedDirectory].some(root => destination === root || destination.startsWith(`${root}${path.sep}`))) throw new Error('Generate lineage policy outside source repositories');
  const repository = spawnSync('git', ['-C', path.dirname(destination), 'rev-parse', '--is-inside-work-tree', '--is-inside-git-dir'],
    { encoding: 'utf8', timeout: 5000, maxBuffer: 4096 });
  if (repository.error || repository.signal) throw new Error('Cannot establish lineage output ownership');
  if (repository.status === 0 && repository.stdout.trim().split('\n').includes('true')) throw new Error('Generate lineage policy outside every Git worktree and repository');
  try { await lstat(destination); throw new Error('Lineage output already exists'); }
  catch (error) { if (error.code !== 'ENOENT') throw error; }
  async function input(file, bound) {
    const stat = await lstat(file);
    if (!stat.isFile() || stat.isSymbolicLink() || stat.size > bound) throw new Error('Unsafe or oversized lineage input');
    return readFile(file);
  }
  const mapBytes = await input(mapFile, 64 * 1024 * 1024);
  const captures = JSON.parse(await input(capturesFile, 16 * 1024));
  const custody = JSON.parse(await input(custodyFile, 32 * 1024));
  if (!Array.isArray(captures) || captures.length < 1 || captures.length > 128) throw new Error('Bounded original capture inventory required');
  const releases = new Map();
  for (const expected of [custody.mapAsset, custody.originalBackup]) {
    if (!Number.isSafeInteger(expected?.releaseId) || expected.releaseId <= 0) throw new Error('Invalid custody release ID');
    if (!releases.has(expected.releaseId)) {
      const response = command('gh', ['api', `repos/dancxjo/conduit-evidence/releases/${expected.releaseId}`],
        { encoding: 'utf8', timeout: 60000, maxBuffer: 2 * 1024 * 1024 });
      if (response.error || response.status !== 0) throw new Error('Cannot authenticate migration custody');
      releases.set(expected.releaseId, JSON.parse(response.stdout));
    }
    const release = releases.get(expected.releaseId);
    if (release.id !== expected.releaseId || release.draft !== false || release.immutable !== true ||
        !Array.isArray(release.assets) || release.assets.length > 100) throw new Error('Migration custody requires a published immutable release');
    const matches = release.assets.filter(asset => asset.id === expected.assetId);
    if (matches.length !== 1 || matches[0].state !== 'uploaded' || matches[0].size !== expected.bytes ||
        matches[0].digest !== `sha256:${expected.sha256}` || matches[0].browser_download_url !== expected.url) throw new Error('Immutable migration asset identity differs');
  }
  const policy = createCaptureLineagePolicy({ mapBytes, captures, mapAsset: custody.mapAsset, originalBackup: custody.originalBackup, originalDirectory, mappedDirectory });
  await writeFile(destination, JSON.stringify(policy, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
  return { output: destination, captures: policy.captures.length, mapRoot: policy.map.root,
    mapSha256: policy.map.asset.sha256, installed: false };
}
