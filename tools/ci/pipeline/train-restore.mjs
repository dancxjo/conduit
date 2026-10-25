// Authenticate retained archives against live Actions custody before extraction.
import { spawnSync } from 'node:child_process';
import { createReadStream } from 'node:fs';
import { createHash } from 'node:crypto';
import { lstat, mkdir, readFile, realpath, rename, statfs, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { verifyTrainRun } from './train.mjs';
import { verifyBundle } from './receipts.mjs';
import { TARGETS } from './targets.mjs';
import { verifyProductCustody } from './producer-custody.mjs';

const fail = message => { throw new Error(message); };
const positive = value => Number.isSafeInteger(value) && value > 0;
const defaultMetadata = endpoint => {
  const result = spawnSync('gh', ['api', endpoint], { encoding: 'utf8', maxBuffer: 2 * 1024 ** 2, timeout: 60000 });
  if (result.error || result.status !== 0) fail('Live artifact metadata failed');
  return JSON.parse(result.stdout);
};

export async function restoreTrainArtifacts({ manifest, observation, acquisition, directory,
  metadata = defaultMetadata, targets = TARGETS, validate = verifyTrainRun }) {
  const verified = validate(manifest, observation);
  const initial = observation.run;
  if (!positive(initial.run_attempt) || !positive(initial.repository?.id)
      || !positive(initial.head_repository?.id)) fail('Missing run attempt or repository IDs');
  const base = `repos/${verified.repository}`;
  const refresh = async () => {
    const run = await metadata(`${base}/actions/runs/${initial.id}`);
    validate(manifest, { ...observation, run });
    if (run.run_attempt !== initial.run_attempt || run.repository?.id !== initial.repository.id
        || run.head_repository?.id !== initial.head_repository.id || run.id !== initial.id) fail('Run attempt changed');
  };
  await refresh();
  for (const root of [acquisition, path.dirname(directory)]) {
    if (!path.isAbsolute(root) || await realpath(root) !== root) fail('Real absolute directories required');
  }
  const receiptPath = path.join(acquisition, 'acquisition.json');
  const receiptStat = await lstat(receiptPath);
  if (!receiptStat.isFile() || receiptStat.isSymbolicLink() || receiptStat.size > 131072) fail('Invalid acquisition receipt');
  const receipt = JSON.parse(await readFile(receiptPath, 'utf8'));
  if (receipt.schema !== 'conduit.train-artifact-acquisition/v1'
      || receipt.repository !== verified.repository || receipt.repositoryId !== initial.repository.id
      || receipt.manifestDigest !== manifest.digest || receipt.source !== verified.source
      || receipt.tree !== verified.tree || receipt.runId !== initial.id
      || receipt.runAttempt !== initial.run_attempt || !Array.isArray(receipt.artifacts)) fail('Acquisition identity mismatch');
  const expected = new Set(targets.map(target => `product-${target.id}`));
  if (!expected.size || expected.size > 32 || expected.size !== targets.length
      || [...expected].some(name => !/^product-[a-z0-9][a-z0-9_-]*$/.test(name))
      || receipt.artifacts.length !== expected.size) fail('Invalid required product set');
  const names = new Set(); const ids = new Set(); let compressed = 0;
  for (const item of receipt.artifacts) {
    if (!expected.has(item.name) || names.has(item.name) || !positive(item.id) || ids.has(item.id)
        || item.archive !== `${item.name}.zip` || !positive(item.bytes) || item.bytes > 4 * 1024 ** 3
        || !/^sha256:[a-f0-9]{64}$/.test(item.digest ?? '')) fail('Invalid acquisition product');
    names.add(item.name); ids.add(item.id); compressed += item.bytes;
    const live = await metadata(`${base}/actions/artifacts/${item.id}`);
    const owner = live.workflow_run;
    if (live.id !== item.id || live.name !== item.name || live.expired !== false
        || live.size_in_bytes !== item.bytes || live.digest !== item.digest
        || owner?.id !== initial.id || owner.head_sha !== verified.source
        || owner.repository_id !== initial.repository.id
        || owner.head_repository_id !== initial.head_repository.id) fail('Live artifact custody mismatch');
    const archive = path.join(acquisition, item.archive);
    const stat = await lstat(archive);
    if (!stat.isFile() || stat.isSymbolicLink() || stat.size !== item.bytes) fail('Archive size or type mismatch');
    const hash = createHash('sha256'); let bytes = 0;
    for await (const chunk of createReadStream(archive)) {
      bytes += chunk.length;
      if (bytes > item.bytes) fail('Archive exceeded bound');
      hash.update(chunk);
    }
    if (bytes !== item.bytes || `sha256:${hash.digest('hex')}` !== item.digest) fail('Archive digest mismatch');
  }
  if (compressed > 32 * 1024 ** 3) fail('Compressed train exceeds bound');
  const disk = await statfs(path.dirname(directory));
  if (disk.bavail * disk.bsize < 4 * 1024 ** 3) fail('Restoration reserve unavailable');
  await mkdir(directory);
  const bundle = path.join(directory, 'bundle'); await mkdir(bundle);
  let expanded = 0; const restoration = [];
  for (const item of receipt.artifacts) {
    const target = item.name.slice('product-'.length);
    const stage = path.join(directory, item.name);
    const result = spawnSync('python3', [fileURLToPath(new URL('./train-restore.py', import.meta.url)),
      path.join(acquisition, item.archive), target, stage, String(Math.min(8 * 1024 ** 3, 32 * 1024 ** 3 - expanded))],
    { encoding: 'utf8', maxBuffer: 65536, timeout: 10 * 60 * 1000 });
    if (result.error || result.status !== 0) fail(`Safe artifact restoration refused: ${result.stderr ?? result.error?.message}`);
    const restored = JSON.parse(result.stdout);
    expanded += restored.bytes;
    if (!Number.isSafeInteger(restored.bytes) || restored.bytes < 0 || expanded > 32 * 1024 ** 3) fail('Expanded train exceeds bound');
    await rename(path.join(stage, 'bundle', target), path.join(bundle, target));
    restoration.push({ name: item.name, bytes: restored.bytes, members: restored.members });
  }
  const receipts = await verifyBundle({ directory: bundle, sha: verified.source, targets });
  for (const target of targets) {
    const filename = path.join(bundle, target.id, 'producer.json');
    const stat = await lstat(filename);
    if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 2 * 1024 * 1024) fail('Invalid producer custody file');
    verifyProductCustody(JSON.parse(await readFile(filename, 'utf8')), {
      source: verified.source, tree: verified.tree, target: target.id, run: initial, manifestDigest: manifest.digest,
    });
  }
  await refresh();
  const result = { schema: 'conduit.train-artifact-restoration/v1', source: verified.source,
    tree: verified.tree, manifestDigest: manifest.digest, repository: verified.repository,
    runId: initial.id, runAttempt: initial.run_attempt, artifacts: receipt.artifacts,
    restoration, receipts, bundle };
  await writeFile(path.join(directory, 'restoration.json'), `${JSON.stringify(result, null, 2)}\n`, { flag: 'wx', mode: 0o600 });
  return result;
}
