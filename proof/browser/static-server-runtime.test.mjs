import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

const alias = 'target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm';
const moduleBytes = name => Buffer.from([0, 97, 115, 109, 1, 0, 0, 0, 0, 2, 1, name]);
async function fixture(t, selection) {
  const directory = await mkdtemp(path.join(tmpdir(), 'conduit-runtime-selection-'));
  const root = path.join(directory, 'served');
  await mkdir(path.dirname(path.join(root, alias)), { recursive: true });
  await writeFile(path.join(root, alias), moduleBytes(114));
  const selected = path.join(directory, 'sealed.wasm');
  if (selection === 'present') await writeFile(selected, moduleBytes(115));
  const env = { ...process.env };
  delete env.CONDUIT_BROWSER_RUNTIME_WASM;
  if (selection !== 'unset') env.CONDUIT_BROWSER_RUNTIME_WASM = selected;
  const child = spawn(process.execPath,
    [new URL('./static-server.mjs', import.meta.url).pathname, '0', root],
    { env, stdio: ['ignore', 'pipe', 'pipe'] });
  t.after(async () => {
    if (child.exitCode === null) { child.kill(); await once(child, 'exit'); }
    await rm(directory, { recursive: true, force: true });
  });
  const url = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('static server readiness absent')), 5000);
    let output = '';
    child.stdout.on('data', data => {
      output += data;
      const match = output.match(/CONDUIT_STATIC_SERVER_URL=(http:\/\/127\.0\.0\.1:\d+\/)/);
      if (match) { clearTimeout(timer); resolve(match[1]); }
    });
    child.once('exit', code => { clearTimeout(timer); reject(new Error(`static server exited ${code}`)); });
  });
  return { url, root };
}

test('explicit runtime serves the sealed bytes through the historical fixture URL', async t => {
  const { url } = await fixture(t, 'present');
  const response = await fetch(new URL(alias, url));
  assert.equal(response.status, 200);
  assert.equal(response.headers.get('content-type'), 'application/wasm');
  assert.deepEqual(Buffer.from(await response.arrayBuffer()), moduleBytes(115));
  assert.equal((await fetch(new URL('sealed.wasm', url))).status, 404);
});

test('missing explicitly selected runtime refuses without serving raw Cargo bytes', async t => {
  const { url } = await fixture(t, 'missing');
  assert.equal((await fetch(new URL(alias, url))).status, 404);
});

test('no runtime selection retains ordinary static serving', async t => {
  const { url } = await fixture(t, 'unset');
  const response = await fetch(new URL(alias, url));
  assert.equal(response.status, 200);
  assert.deepEqual(Buffer.from(await response.arrayBuffer()), moduleBytes(114));
});
