import test from 'node:test';
import assert from 'node:assert/strict';
import { promises as fs } from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cachedHostedLinux } from '../../tools/ci/pipeline/targets/hosted-cached.mjs';
const sha = bytes => createHash('sha256').update(bytes).digest('hex');

// These fixtures exercise the product/source/custody adapter, not compilation.
// Real Cargo and canonical Hello acceptance belong to the sandbox proof.
async function fixture(t) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'hosted-product-'));
  t.after(() => fs.rm(root, { recursive: true, force: true }));
  const repositoryRoot = path.join(root, 'checkout');
  await fs.mkdir(repositoryRoot);
  const git = args => execFileSync('git', ['-C', repositoryRoot, ...args], { encoding: 'utf8' }).trim();
  git(['init', '-q']);
  await fs.writeFile(path.join(repositoryRoot, 'input.txt'), 'exact source\n');
  git(['add', 'input.txt']);
  git(['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'source fixture']);
  const snapshot = { schema: 'hosted-linux-cargo-snapshot-v1', root: path.join(root, 'snapshot'),
    sourceCommit: git(['rev-parse', 'HEAD']), sourceTree: git(['rev-parse', 'HEAD^{tree}']), sha256: 'a'.repeat(64) };
  await fs.mkdir(path.join(snapshot.root, 'source'), { recursive: true });
  await fs.copyFile(path.join(repositoryRoot, 'input.txt'), path.join(snapshot.root, 'source/input.txt'));
  const directory = path.join(root, 'product'); await fs.mkdir(directory);
  const options = { snapshot, repositoryRoot, directory, taskRoot: path.join(root, 'task'), cacheRoot: path.join(root, 'cache') };
  const binary = Buffer.from('fixture binary, never claimed executable proof');
  const observations = [];
  const observe = (callback, stage) => callback({ program: '/usr/bin/bwrap', args: ['--clearenv', stage],
    cwd: repositoryRoot, environment: { PATH: '/usr/bin:/bin', LANG: 'C' }, durationMs: 1, outcome: 'success' });
  let builds = 0, proofs = 0;
  const services = {
    observeCommand: observation => observations.push(observation),
    build: async ({ taskRoot, observeCommand }) => {
      builds++; await fs.mkdir(path.join(taskRoot, 'target/release'), { recursive: true });
      await fs.writeFile(path.join(taskRoot, 'target/release/conduit'), binary, { mode: 0o755 });
      const receipt = { snapshotSha256: snapshot.sha256, namespace: 'b'.repeat(64),
        outputs: [{ name: 'conduit', sha256: sha(binary) }] };
      await fs.writeFile(path.join(taskRoot, 'cargo-cache-receipt.json'), JSON.stringify(receipt));
      observe(observeCommand, 'build'); return receipt;
    },
    prove: async ({ taskRoot, observeCommand }) => {
      proofs++;
      const report = Buffer.from(JSON.stringify({ schema: 'fixture-report' }));
      await fs.writeFile(path.join(taskRoot, 'hello-report.json'), report);
      await fs.writeFile(path.join(taskRoot, 'hello-stdout.txt'), 'fixture stdout');
      await fs.mkdir(path.join(taskRoot, 'hello-artifacts'));
      await fs.writeFile(path.join(taskRoot, 'hello-artifacts/result.txt'), 'fresh fixture evidence');
      const receipt = { snapshotSha256: snapshot.sha256, namespace: 'b'.repeat(64),
        binarySha256: sha(binary), binaryUnchanged: true, reportSha256: sha(report) };
      await fs.writeFile(path.join(taskRoot, 'hello-proof.json'), JSON.stringify(receipt));
      observe(observeCommand, 'hello'); return receipt;
    },
  };
  return { root, options, services, observations, binary, git, counts: () => ({ builds, proofs }) };
}

test('fresh product stages exact binary, proof and source custody, not a shared target', async t => {
  const f = await fixture(t);
  await cachedHostedLinux(f.options, f.services);
  assert.deepEqual(f.counts(), { builds: 1, proofs: 1 });
  assert.deepEqual(await fs.readFile(path.join(f.options.directory, 'conduit')), f.binary);
  assert.equal((await fs.stat(path.join(f.options.directory, 'conduit'))).mode & 0o111, 0o111);
  assert.deepEqual(f.observations.map(x => x.args.at(-1)), ['build', 'hello']);
  const receipt = JSON.parse(await fs.readFile(path.join(f.options.directory, 'compiler-snapshot.json')));
  assert.equal(receipt.source, f.options.snapshot.sourceCommit);
  assert.equal(receipt.binarySha256, sha(f.binary));
  assert.equal(receipt.sharedObjectsAreProof, false);
  assert.equal((await fs.readdir(f.options.directory)).includes('target'), false);
});

test('wrong captured source and changed copied source refuse before compilation', async t => {
  for (const change of ['head', 'bytes']) {
    const f = await fixture(t);
    if (change === 'head') f.options.snapshot.sourceCommit = 'f'.repeat(40);
    else await fs.writeFile(path.join(f.options.snapshot.root, 'source/input.txt'), 'changed');
    await assert.rejects(cachedHostedLinux(f.options, f.services), /Source checkout|source-blob-mismatch/);
    assert.deepEqual(f.counts(), { builds: 0, proofs: 0 });
  }
});

test('both construction and fresh proof must independently record command custody', async t => {
  for (const missing of ['build', 'prove']) {
    const f = await fixture(t), original = f.services[missing];
    f.services[missing] = args => original({ ...args, observeCommand: () => {} });
    await assert.rejects(cachedHostedLinux(f.options, f.services), /omitted actual command custody/);
    assert.deepEqual(await fs.readdir(f.options.directory), []);
  }
});

test('stale runtime receipt, altered report and binary mutation refuse retention', async t => {
  for (const change of ['namespace', 'report', 'binary']) {
    const f = await fixture(t), prove = f.services.prove;
    f.services.prove = async args => {
      const result = await prove(args);
      if (change === 'namespace') result.namespace = 'c'.repeat(64);
      else await fs.writeFile(path.join(f.options.taskRoot, change === 'report' ? 'hello-report.json' : 'target/release/conduit'), 'changed');
      return result;
    };
    await assert.rejects(cachedHostedLinux(f.options, f.services), /custody differs/);
    assert.deepEqual(await fs.readdir(f.options.directory), []);
  }
});

test('changed checkout during proof and escaping evidence links refuse sealed product', async t => {
  for (const change of ['source', 'link']) {
    const f = await fixture(t), prove = f.services.prove;
    f.services.prove = async args => {
      const result = await prove(args);
      if (change === 'source') await fs.writeFile(path.join(f.options.repositoryRoot, 'input.txt'), 'dirty source');
      else await fs.symlink(path.join(f.options.repositoryRoot, 'input.txt'), path.join(f.options.taskRoot, 'hello-artifacts/escape'));
      return result;
    };
    await assert.rejects(cachedHostedLinux(f.options, f.services), /Source checkout|symlink/);
    await assert.rejects(fs.stat(path.join(f.options.directory, 'compiler-snapshot.json')), { code: 'ENOENT' });
  }
});
