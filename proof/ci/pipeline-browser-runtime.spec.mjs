import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { once } from 'node:events';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const server = fileURLToPath(new URL('../browser/static-server.mjs', import.meta.url));
const legacy = 'target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm';
async function fixture(t, selected, mount = '/') {
  const root = await mkdtemp(path.join(tmpdir(), 'conduit-browser-runtime-'));
  const raw = path.join(root, legacy);
  await mkdir(path.dirname(raw), { recursive: true });
  await writeFile(raw, 'raw-cargo-diagnostic');
  const sealed = path.join(root, 'sealed-runtime.wasm');
  await writeFile(sealed, 'sealed-reviewed-runtime');
  const env = { ...process.env };
  delete env.CONDUIT_BROWSER_RUNTIME_WASM;
  if (selected !== undefined) env.CONDUIT_BROWSER_RUNTIME_WASM = selected === true ? sealed : selected === 'directory' ? root : selected === 'missing' ? path.join(root, 'missing.wasm') : selected;
  const child = spawn(process.execPath, [server, '0', root, mount], { env, stdio: ['ignore', 'pipe', 'pipe'] });
  t.after(async () => {
    if (child.exitCode === null) { const exited = once(child, 'exit'); child.kill('SIGTERM'); await exited; }
    await rm(root, { recursive: true, force: true });
  });
  let output = '';
  const url = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`server did not start: ${output}`)), 5000);
    child.stdout.on('data', chunk => {
      output += chunk;
      const match = output.match(/CONDUIT_STATIC_SERVER_URL=(http:\/\/127\.0\.0\.1:\d+\/[^\n]*)/);
      if (match) { clearTimeout(timer); resolve(match[1]); }
    });
    child.once('error', error => { clearTimeout(timer); reject(error); });
    child.once('exit', code => { clearTimeout(timer); reject(new Error(`server exited ${code}`)); });
  });
  return { root, raw, url };
}

test('default legacy runtime URL retains the raw Cargo artifact', async t => {
  const f = await fixture(t);
  const response = await fetch(new URL(legacy, f.url));
  assert.equal(response.status, 200);
  assert.equal(response.headers.get('content-type'), 'application/wasm');
  assert.equal(await response.text(), 'raw-cargo-diagnostic');
});

test('explicit legacy alias serves exact selected runtime without changing raw Cargo bytes', async t => {
  const f = await fixture(t, true, '/proof/');
  const response = await fetch(new URL(legacy, f.url));
  assert.equal(response.status, 200);
  assert.equal(response.headers.get('content-type'), 'application/wasm');
  assert.equal(await response.text(), 'sealed-reviewed-runtime');
  assert.equal(await readFile(f.raw, 'utf8'), 'raw-cargo-diagnostic');
  assert.equal((await fetch(new URL(`/${legacy}`, f.url))).status, 404, 'alias cannot escape mount');
  const traversal = await fetch(new URL('/proof/%2e%2e%2foutside', f.url));
  assert.equal(traversal.status, 404, 'path traversal remains refused');
});

test('missing, empty and non-file selected runtimes never fall back to raw Cargo bytes', async t => {
  for (const selected of ['missing', '', 'directory']) {
    await t.test(JSON.stringify(selected), async t => {
      const f = await fixture(t, selected);
      assert.equal((await fetch(new URL(legacy, f.url))).status, 404);
      assert.equal(await readFile(f.raw, 'utf8'), 'raw-cargo-diagnostic');
    });
  }
});

test('CI browser proof launch selects the sealed release with existing env override', async () => {
  const source = await readFile(new URL('../../tools/ci/pipeline/targets/browser.mjs', import.meta.url), 'utf8');
  assert.match(source, /CONDUIT_BROWSER_RUNTIME_WASM:\s*path\.resolve\(releases,\s*['"]runtime\.wasm['"]\)/);
  assert.match(source, /'--workers', '1', '--retries', '0'/);
});
