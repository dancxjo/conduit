import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile, lstat, mkdir, symlink } from 'node:fs/promises';
import path from 'node:path';
import { fixture } from './helpers/train-fixture.mjs';
import { acquireTrainArtifacts } from '../../tools/ci/pipeline/train-artifacts.mjs';

async function setup(t) {
  const f = await fixture(t);
  f.observation.run.run_attempt = 2;
  f.observation.run.repository = { ...f.observation.run.repository, id: 11 };
  f.observation.run.head_repository = { ...f.observation.run.head_repository, id: 11 };
  const bytes = Buffer.from('immutable archive bytes');
  const artifact = { name: 'product-hosted-linux', id: 456, expired: false,
    size_in_bytes: bytes.length, digest: `sha256:${createHash('sha256').update(bytes).digest('hex')}`,
    workflow_run: { id: f.observation.run.id, repository_id: 11, head_repository_id: 11, head_sha: f.source } };
  const inventory = { total_count: 1, artifacts: [artifact] };
  const directory = path.join(f.directory, 'acquired');
  let reads = 0; let downloads = 0; let lastRun = null;
  const command = async (program, args, options) => {
    assert.equal(program, 'gh');
    assert.equal(args[0], 'api');
    if (args[1].endsWith('/zip')) {
      downloads++;
      assert.equal(args[1], 'repos/example/conduit/actions/artifacts/456/zip');
      await options.output.write(bytes);
      return { status: 0 };
    }
    if (args[1].includes('/artifacts?')) return { status: 0, stdout: JSON.stringify(inventory) };
    reads++;
    return { status: 0, stdout: JSON.stringify(reads > 1 && lastRun ? lastRun : f.observation.run) };
  };
  return { ...f, artifact, inventory, bytes, directory,
    setLastRun: value => { lastRun = value; }, downloads: () => downloads,
    acquire: overrides => acquireTrainArtifacts({ manifest: f.manifest, observation: f.observation,
      directory, command, targets: [{ id: 'hosted-linux', proofClass: 'executable' }], ...overrides }) };
}

test('acquires exact immutable IDs and binds successful run attempt', async t => {
  const f = await setup(t);
  const receipt = await f.acquire();
  assert.equal(receipt.runAttempt, 2);
  assert.equal(receipt.source, f.source);
  assert.equal(receipt.manifestDigest, f.manifest.digest);
  assert.deepEqual(await readFile(path.join(f.directory, 'product-hosted-linux.zip')), f.bytes);
  assert.deepEqual(JSON.parse(await readFile(path.join(f.directory, 'acquisition.json'), 'utf8')), receipt);
});

for (const [name, mutate] of [
  ['expired', f => { f.artifact.expired = true; }],
  ['missing digest', f => { delete f.artifact.digest; }],
  ['foreign run', f => { f.artifact.workflow_run.id++; }],
  ['foreign source', f => { f.artifact.workflow_run.head_sha = 'a'.repeat(40); }],
  ['foreign repository', f => { f.artifact.workflow_run.repository_id++; }],
  ['extra product', f => { f.inventory.artifacts.push({ ...f.artifact, name: 'product-surprise', id: 457 }); f.inventory.total_count++; }],
  ['duplicate ID', f => { f.inventory.artifacts.push({ ...f.artifact }); f.inventory.total_count++; }],
  ['missing product', f => { f.inventory.artifacts = []; f.inventory.total_count = 0; }],
  ['unbounded inventory', f => { f.inventory.total_count = 101; }],
  ['missing attempt', f => { delete f.observation.run.run_attempt; }],
]) test(`refuses ${name} before download`, async t => {
  const f = await setup(t); mutate(f);
  await assert.rejects(f.acquire()); assert.equal(f.downloads(), 0);
});

test('changed rerun attempt after acquisition preserves archives without success receipt', async t => {
  const f = await setup(t);
  f.setLastRun({ ...f.observation.run, run_attempt: 3 });
  await assert.rejects(f.acquire(), /attempt changed/);
  assert.equal((await lstat(path.join(f.directory, 'product-hosted-linux.zip'))).isFile(), true);
  await assert.rejects(lstat(path.join(f.directory, 'acquisition.json')), { code: 'ENOENT' });
});

test('digest mismatch preserves failed evidence', async t => {
  const f = await setup(t); f.artifact.digest = `sha256:${'0'.repeat(64)}`;
  await assert.rejects(f.acquire(), /digest mismatch/);
  assert.equal(f.downloads(), 1);
  await assert.rejects(lstat(path.join(f.directory, 'acquisition.json')), { code: 'ENOENT' });
});

for (const kind of ['directory', 'symlink']) test(`refuses existing ${kind} destination`, async t => {
  const f = await setup(t);
  if (kind === 'directory') await mkdir(f.directory);
  else await symlink(f.directory + '-missing', f.directory);
  await assert.rejects(f.acquire()); assert.equal(f.downloads(), 0);
});

test('stale attempt before acquisition refuses before download', async t => {
  const f = await setup(t);
  const run = { ...f.observation.run, run_attempt: 3 };
  const command = async () => ({ status: 0, stdout: JSON.stringify(run) });
  await assert.rejects(f.acquire({ command }), /attempt changed/);
  await assert.rejects(lstat(f.directory), { code: 'ENOENT' });
});
