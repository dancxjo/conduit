import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, open, readFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { createBoundedTrainArtifactCommand } from '../../tools/ci/pipeline/train-artifacts.mjs';

async function fixture(t) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'artifact-command-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const directory = path.join(root, 'child'); await mkdir(directory);
  return { root, directory };
}
const digest = value => createHash('sha256').update(value).digest('hex');
const runChild = (command, source, options = {}) => command(process.execPath, ['-e', source], { maxBuffer: 4096, timeout: 5000, ...options });

test('tiny actual child receives only captured fixture env/cwd, never credential argv', async t => {
  const f = await fixture(t), credential = 'opaque-fixture-artifact-session-value';
  const env = { CONDUIT_FIXTURE_SESSION: credential, CONDUIT_FIXTURE_SCOPE: 'isolated' };
  const hadGlobalFixture = Object.hasOwn(process.env, 'CONDUIT_FIXTURE_SESSION');
  const command = createBoundedTrainArtifactCommand({ env, directory: f.directory });
  env.CONDUIT_FIXTURE_SESSION = 'later-caller-rotation';
  const result = await runChild(command, `
    const crypto = require('node:crypto');
    const credential = process.env.CONDUIT_FIXTURE_SESSION || '';
    process.stdout.write(JSON.stringify({
      digest: crypto.createHash('sha256').update(credential).digest('hex'),
      cwd: process.cwd(),
      argvClean: !!credential && process.argv.every(arg => !arg.includes(credential)),
      keys: Object.keys(process.env).sort(), scope: process.env.CONDUIT_FIXTURE_SCOPE
    }));`, { env: { CONDUIT_FIXTURE_SESSION: 'wrong-per-call-environment' }, cwd: f.root });
  assert.equal(result.status, 0);
  const child = JSON.parse(result.stdout);
  assert.equal(child.digest, digest(credential)); assert.equal(child.cwd, f.directory);
  assert.equal(child.argvClean, true); assert.equal(child.scope, 'isolated');
  assert.deepEqual(child.keys, ['CONDUIT_FIXTURE_SCOPE', 'CONDUIT_FIXTURE_SESSION']);
  assert.ok(Object.hasOwn(process.env, 'CONDUIT_FIXTURE_SESSION') === hadGlobalFixture, 'parent environment remains untouched');
});

test('factory without explicit context preserves inherited cwd behavior', async () => {
  const command = createBoundedTrainArtifactCommand();
  const result = await runChild(command, 'process.stdout.write(process.cwd())');
  assert.equal(result.status, 0); assert.equal(result.stdout, process.cwd());
});

test('streams exact bounded bytes through actual output handle and partial writes', async t => {
  const f = await fixture(t), command = createBoundedTrainArtifactCommand({ env: {}, directory: f.directory });
  const outputFile = path.join(f.root, 'archive.bin'), handle = await open(outputFile, 'wx', 0o600);
  try {
    const result = await runChild(command, 'process.stdout.write(Buffer.alloc(513, 120))', {
      maximumBytes: 513, maxBuffer: 16,
      output: { write: (bytes, offset, length) => handle.write(bytes, offset, Math.min(length, 7)) },
    });
    assert.equal(result.status, 0); assert.equal(result.stdout, '');
  } finally { await handle.close(); }
  const bytes = await readFile(outputFile);
  assert.equal(bytes.length, 513); assert.ok(bytes.every(byte => byte === 120), 'exact fixture bytes retained');
});

test('stdout byte overrun refuses before appending excess chunk', async t => {
  const f = await fixture(t), command = createBoundedTrainArtifactCommand({ env: {}, directory: f.directory });
  let writes = 0;
  await assert.rejects(runChild(command, 'process.stdout.write(Buffer.alloc(65536, 120)); setInterval(() => {}, 1000)', {
    maximumBytes: 32, output: { async write() { writes++; return { bytesWritten: 1 }; } },
  }), /byte bound exceeded/);
  assert.equal(writes, 0);
});

test('buffer-only metadata output retains its maxBuffer bound', async () => {
  const command = createBoundedTrainArtifactCommand({ env: {} });
  await assert.rejects(runChild(command, 'process.stdout.write(Buffer.alloc(65536, 120)); setInterval(() => {}, 1000)', { maxBuffer: 16 }), /byte bound exceeded/);
});

test('stderr excess kills actual child without echoing diagnostics', async () => {
  const command = createBoundedTrainArtifactCommand({ env: {} });
  const result = await runChild(command, 'process.stderr.write("fixture-only-diagnostic".repeat(1024)); setInterval(() => {}, 1000)', { maxBuffer: 16 });
  assert.ok(result.status !== 0, 'stderr bound refuses running child');
  assert.equal(result.stdout, ''); assert.equal(result.stderr, undefined);
});

test('timeout kills tiny actual child and does not report success', async () => {
  const command = createBoundedTrainArtifactCommand({ env: {} });
  const before = Date.now();
  const result = await runChild(command, 'setInterval(() => {}, 1000)', { timeout: 50 });
  assert.ok(result.status !== 0, 'timed-out child is unsuccessful');
  assert.ok(Date.now() - before < 5000, 'timeout remains bounded');
});

test('output that makes no progress refuses and terminates child', async () => {
  const command = createBoundedTrainArtifactCommand({ env: {} });
  await assert.rejects(runChild(command, 'process.stdout.write("fixture"); setInterval(() => {}, 1000)', {
    maximumBytes: 64, output: { async write() { return { bytesWritten: 0 }; } },
  }), /write made no progress/);
});

test('failed output handle refuses without retrying or inventing archive bytes', async t => {
  const f = await fixture(t), command = createBoundedTrainArtifactCommand({ env: {}, directory: f.directory });
  const handle = await open(path.join(f.root, 'closed.bin'), 'wx', 0o600); await handle.close();
  await assert.rejects(runChild(command, 'process.stdout.write("fixture"); setInterval(() => {}, 1000)', {
    maximumBytes: 64, output: handle,
  }));
  assert.equal((await readFile(path.join(f.root, 'closed.bin'))).length, 0);
});

test('nonexistent command and nonexistent cwd refuse', async t => {
  const f = await fixture(t), command = createBoundedTrainArtifactCommand({ env: {}, directory: f.directory });
  await assert.rejects(command(path.join(f.root, 'missing-command'), [], { maxBuffer: 64, timeout: 1000 }));
  const missingDirectory = createBoundedTrainArtifactCommand({ env: {}, directory: path.join(f.root, 'missing-directory') });
  await assert.rejects(runChild(missingDirectory, 'process.stdout.write("never")'));
});
