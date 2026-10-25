import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, readFile, rm, symlink, truncate } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { prepareHostedProductCache, loadHostedProductCacheConfig } from '../../tools/ci/pipeline/hosted-cache-command.mjs';

async function fixture(t) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'hosted-command-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const repositoryRoot = path.join(root, 'repo'); await mkdir(repositoryRoot);
  const git = args => {
    const result = spawnSync('git', args, { cwd: repositoryRoot, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr); return result.stdout.trim();
  };
  git(['init', '-q']); git(['config', 'user.name', 'Fixture']); git(['config', 'user.email', 'fixture@example.invalid']);
  await writeFile(path.join(repositoryRoot, 'source'), 'exact source');
  git(['add', '.']); git(['commit', '-qm', 'source']);
  const sourceCommit = git(['rev-parse', 'HEAD']), sourceTree = git(['rev-parse', 'HEAD^{tree}']);
  const options = { repositoryRoot, sourceSha: sourceCommit, engine: '/engine/sccache', toolchainRoot: '/toolchain',
    destination: path.join(root, 'snapshot'), manifestFile: path.join(root, 'snapshot.json'),
    cacheRoot: path.join(root, 'cache'), taskRoot: path.join(root, 'task') };
  let prepares = 0;
  const dependencies = {
    async prepareSnapshot(args) {
      prepares++; assert.equal(args.cacheMode, 'audited-dependencies');
      assert.equal(args.acquisitionFile, `${options.destination}.acquisition.json`);
      await mkdir(args.destination);
      return { schema: 'hosted-linux-cargo-snapshot-v1', root: args.destination, sourceCommit, sourceTree, cacheMode: args.cacheMode };
    },
    async derivePolicy(_snapshot, policyOptions) {
      assert.equal(policyOptions.cacheMode, 'audited-dependencies');
      return { policy: { schema: 3 }, namespace: 'a'.repeat(64) };
    },
  };
  const configFile = path.join(root, 'target.json');
  const prepare = () => prepareHostedProductCache(options, dependencies);
  const configure = async () => { const result = await prepare(); await writeFile(configFile, JSON.stringify(result.config)); return result; };
  const load = () => loadHostedProductCacheConfig(configFile, { repositoryRoot, sourceSha: sourceCommit });
  return { root, options, dependencies, configFile, prepare, configure, load, prepares: () => prepares };
}

test('prepares source-bound external manifest and loads only private target arguments', async t => {
  const f = await fixture(t), result = await f.configure();
  assert.equal(result.namespace, 'a'.repeat(64));
  assert.deepEqual(result.managedPaths, [path.join(f.options.cacheRoot, 'policy.json'), path.join(f.options.cacheRoot, 'objects')]);
  const loaded = await f.load();
  assert.equal(loaded.snapshot.sourceCommit, f.options.sourceSha);
  assert.equal(loaded.taskRoot, f.options.taskRoot);
  assert.equal(loaded.cacheMode, 'audited-dependencies');
});
test('supported preparation writes a fresh external config and leaves task creation to the builder', async t => {
  const f = await fixture(t); f.options.configFile = f.configFile;
  const result = await f.prepare();
  assert.equal(result.configFile, f.configFile);
  assert.equal((await f.load()).snapshot.sourceCommit, f.options.sourceSha);
  await assert.rejects(readFile(path.join(f.options.taskRoot, 'target')), { code: 'ENOENT' });
});
test('existing configuration is preserved before any acquisition or manifest output', async t => {
  const f = await fixture(t); f.options.configFile = f.configFile;
  await writeFile(f.configFile, 'preserved existing config');
  await assert.rejects(f.prepare(), /output exists/);
  assert.equal(f.prepares(), 0);
  assert.equal(await readFile(f.configFile, 'utf8'), 'preserved existing config');
  await assert.rejects(readFile(f.options.manifestFile), { code: 'ENOENT' });
});
for (const variant of ['wrong-source', 'dirty', 'inside', 'symlink', 'existing', 'overlap']) {
  test(`preparation refuses ${variant} before snapshot acquisition`, async t => {
    const f = await fixture(t);
    if (variant === 'wrong-source') f.options.sourceSha = '0'.repeat(40);
    if (variant === 'dirty') await writeFile(path.join(f.options.repositoryRoot, 'source'), 'dirty');
    if (variant === 'inside') f.options.manifestFile = path.join(f.options.repositoryRoot, 'manifest.json');
    if (variant === 'symlink') await symlink('/tmp', f.options.destination);
    if (variant === 'existing') await writeFile(f.options.manifestFile, 'preserved');
    if (variant === 'overlap') f.options.taskRoot = f.options.cacheRoot;
    await assert.rejects(f.prepare()); assert.equal(f.prepares(), 0);
    if (variant === 'existing') assert.equal(await readFile(f.options.manifestFile, 'utf8'), 'preserved');
  });
}
test('snapshot cannot substitute another source tree', async t => {
  const f = await fixture(t), prepare = f.dependencies.prepareSnapshot;
  f.dependencies.prepareSnapshot = async args => ({ ...await prepare(args), sourceTree: 'b'.repeat(40) });
  await assert.rejects(f.prepare(), /source\/tree differs/);
  await assert.rejects(readFile(f.options.manifestFile), { code: 'ENOENT' });
});
test('preparation rechecks source after snapshot acquisition before publishing manifest', async t => {
  const f = await fixture(t), prepare = f.dependencies.prepareSnapshot;
  f.dependencies.prepareSnapshot = async args => {
    const snapshot = await prepare(args);
    await writeFile(path.join(f.options.repositoryRoot, 'source'), 'changed during preparation');
    return snapshot;
  };
  await assert.rejects(f.prepare());
  await assert.rejects(readFile(f.options.manifestFile), { code: 'ENOENT' });
});
for (const variant of ['wrong-source', 'dirty', 'inside', 'symlink', 'oversized', 'task-exists', 'unknown-field', 'wrong-mode']) {
  test(`target configuration refuses ${variant}`, async t => {
    const f = await fixture(t); await f.configure();
    if (variant === 'wrong-source') {
      const snapshot = JSON.parse(await readFile(f.options.manifestFile)); snapshot.sourceCommit = '0'.repeat(40);
      await writeFile(f.options.manifestFile, JSON.stringify(snapshot));
    }
    if (variant === 'dirty') await writeFile(path.join(f.options.repositoryRoot, 'source'), 'dirty');
    if (variant === 'inside') {
      const config = JSON.parse(await readFile(f.configFile)); config.taskRoot = path.join(f.options.repositoryRoot, 'target');
      await writeFile(f.configFile, JSON.stringify(config));
    }
    if (variant === 'symlink') { await rm(f.configFile); await symlink(f.options.manifestFile, f.configFile); }
    if (variant === 'oversized') await truncate(f.configFile, 32 * 1024 ** 2 + 1);
    if (variant === 'task-exists') { await mkdir(f.options.taskRoot); await writeFile(path.join(f.options.taskRoot, 'preserved'), 'proof'); }
    if (variant === 'unknown-field') {
      const config = JSON.parse(await readFile(f.configFile)); config.fallback = true;
      await writeFile(f.configFile, JSON.stringify(config));
    }
    if (variant === 'wrong-mode') {
      const snapshot = JSON.parse(await readFile(f.options.manifestFile)); snapshot.cacheMode = 'exact-snapshot';
      await writeFile(f.options.manifestFile, JSON.stringify(snapshot));
    }
    await assert.rejects(f.load());
    if (variant === 'task-exists') assert.equal(await readFile(path.join(f.options.taskRoot, 'preserved'), 'utf8'), 'proof');
  });
}
