import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, writeFile, readFile, rm, stat, realpath } from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { withIsolatedTrainPushContext } from '../../tools/ci/pipeline/train-push-context.mjs';
const git = (args, cwd, env = process.env) => {
  const result = spawnSync('git', args, { cwd, env, encoding: 'utf8' });
  assert.ok(result.status === 0, 'fixture Git operation succeeds'); return result.stdout.trim();
};
async function fixture(t) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'train-push-fixture-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const directory = path.join(root, 'source'); git(['init', directory], root);
  git(['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '--allow-empty', '-m', 'fixture'], directory);
  return { root, directory, source: git(['rev-parse', 'HEAD'], directory) };
}
test('isolated context borrows exact objects without source hooks, URL rewrites, or ambient Git overrides', async t => {
  const f = await fixture(t), remote = path.join(f.root, 'remote.git'); git(['init', '--bare', remote], f.root);
  const marker = path.join(f.root, 'hook-ran');
  await writeFile(path.join(f.directory, '.git/hooks/pre-push'), `#!/bin/sh\necho observed > '${marker}'\n`, { mode: 0o700 });
  git(['config', 'url./missing-fixture-destination.insteadOf', remote], f.directory);
  let isolated;
  const environment = { PATH: process.env.PATH, GH_TOKEN: 'fixture-only-token',
    GIT_DIR: '/missing-fixture-dir', GIT_CONFIG_COUNT: '1', GIT_CONFIG_KEY_0: 'core.hooksPath',
    GIT_CONFIG_VALUE_0: path.join(f.directory, '.git/hooks') };
  await withIsolatedTrainPushContext({ ...f, environment }, async context => {
    isolated = context.directory;
    assert.ok((await stat(path.dirname(isolated))).mode % 512 === 448);
    assert.ok(!Object.hasOwn(context.environment, 'GH_TOKEN'));
    assert.ok(!Object.hasOwn(context.environment, 'GIT_DIR'));
    assert.ok(!Object.hasOwn(context.environment, 'GIT_CONFIG_COUNT'));
    assert.ok(git(['rev-parse', `${f.source}^{commit}`], isolated, context.environment) === f.source);
    git(['push', remote, `${f.source}:refs/heads/proved`], isolated,
      { ...context.environment, GH_TOKEN: 'fixture-only-token' });
    const config = await readFile(path.join(isolated, 'config'), 'utf8');
    assert.ok(config.includes('hooksPath') && !config.includes('insteadOf'));
    assert.ok((await readFile(path.join(isolated, 'objects/info/alternates'), 'utf8')).trim() === await realpath(path.join(f.directory, '.git/objects')));
  });
  await assert.rejects(stat(isolated), { code: 'ENOENT' });
  await assert.rejects(stat(marker), { code: 'ENOENT' });
  assert.ok(git(['rev-parse', 'refs/heads/proved'], remote) === f.source);
  assert.ok(environment.GH_TOKEN === 'fixture-only-token');
});
test('callback failure cleans private context and preserves callback identity', async t => {
  const f = await fixture(t); let isolated; const failure = new Error('fixture callback');
  await assert.rejects(withIsolatedTrainPushContext({ ...f, environment: { PATH: process.env.PATH } }, context => {
    isolated = context.directory; throw failure;
  }), error => error === failure);
  await assert.rejects(stat(isolated), { code: 'ENOENT' });
});
test('invalid source, path and subprocess responses refuse without raw diagnostics or callback', async t => {
  const f = await fixture(t); let called = false;
  const callback = () => { called = true; };
  for (const overrides of [{ source: 'invalid' }, { directory: `${f.directory}\n` },
    { command: () => ({ status: 1, stderr: 'fixture-sensitive-diagnostic' }) },
    { command: () => ({ status: 0, stdout: '/tmp/objects\nextra\n' }) }]) {
    await assert.rejects(withIsolatedTrainPushContext({ ...f, environment: {}, ...overrides }, callback),
      error => error.message === 'Isolated train push context refused');
  }
  let failedContext;
  const command = (program, args, options) => {
    if (args[0] === 'init') failedContext = args.at(-1);
    return spawnSync(program, args, options);
  };
  await assert.rejects(withIsolatedTrainPushContext({ ...f, source: 'f'.repeat(40), command, environment: { PATH: process.env.PATH } }, callback),
    error => error.message === 'Isolated train push context refused');
  await assert.rejects(stat(failedContext), { code: 'ENOENT' });
  assert.ok(!called);
});
