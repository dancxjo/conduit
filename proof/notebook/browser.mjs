// Candidate live-browser proof through installed owner commands and the packaged
// Handbook. No Body, Face, Show or document state is constructed by this driver.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as pause } from 'node:timers/promises';
import { chromium } from '../browser/node_modules/@playwright/test/index.mjs';
import { startStaticProduct } from '../browser/static-product-server.mjs';

const [binaryArg, handbookArg, outputArg] = process.argv.slice(2);
assert.ok(binaryArg && handbookArg && outputArg,
  'usage: browser.mjs NATIVE-RELEASE HANDBOOK-PACKAGE NEW-EVIDENCE-DIRECTORY');
const output = path.resolve(outputArg), handbook = path.resolve(handbookArg);
assert.equal(existsSync(output), false, 'preserve existing proof evidence');
await mkdir(output, { recursive: true });
const privateRoot = await mkdtemp('/tmp/conduit-notebook-browser-');
const state = path.join(privateRoot, 'state'), storage = path.join(privateRoot, 'storage');
await mkdir(storage, { mode: 0o700 });
const sha = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
const bytes = await readFile(path.resolve(binaryArg));
assert.ok(bytes.length <= 64 * 1024 * 1024, 'use the bounded optimized native image');
const name = 'conduit-linux-x86_64', binary = path.join(output, name);
await writeFile(binary, bytes, { mode: 0o755 });
const file = { path: name, bytes: bytes.length, sha256: sha(bytes) };
const manifest = { schema: 'conduit.release/host-bundle@1',
  source_identity: `candidate-notebook-browser:${file.sha256}`,
  bundle_sha256: sha(Buffer.from(`conduit.release/host-bundle-content@1\0${name}\0${file.sha256}\n`)), files: [file] };
await writeFile(path.join(output, 'release.json'), JSON.stringify(manifest, null, 2));
let sequence = 0;
const commands = [];
function call(exe, args, input) {
  const result = spawnSync(exe, args, { encoding: 'utf8', input, timeout: 30_000, maxBuffer: 4 * 1024 * 1024 });
  commands.push({ sequence: ++sequence, args, status: result.status, stdout: result.stdout, stderr: result.stderr });
  assert.equal(result.status, 0, `${args.join(' ')}: ${result.stderr}`);
  return result.stdout;
}
let installed, service, server, browser;
const services = [], contexts = [], errors = [];
const owner = args => call(installed, args);
const notebook = args => JSON.parse(owner(['body', 'notebook', '--state-dir', state, ...args]));
const stop = async child => {
  if (!child || child.exitCode !== null || child.signalCode !== null) return;
  const exited = once(child, 'exit');
  child.kill('SIGTERM');
  const timer = setTimeout(() => child.kill('SIGKILL'), 5000);
  try { await exited; } finally { clearTimeout(timer); }
};
async function startService() {
  service = spawn(installed, ['host', 'service', 'run', '--state-dir', state], { stdio: ['ignore', 'ignore', 'pipe'] });
  services.push(service);
  service.stderr.on('data', chunk => errors.push(chunk.toString()));
  const until = Date.now() + 30_000;
  while (Date.now() < until) {
    assert.equal(service.exitCode, null, `owner exited: ${errors.join('')}`);
    const probe = spawnSync(installed, ['host', 'service', 'status', '--state-dir', state, '--json'], { encoding: 'utf8', timeout: 5000 });
    if (probe.status === 0) {
      const status = JSON.parse(probe.stdout);
      if (status.process_id === service.pid) return status;
    }
    await pause(50);
  }
  throw new Error('owner readiness deadline');
}
async function enter(context, bodyId) {
  const page = await context.newPage();
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(`${server.url}?participate=owner#your-handbook`);
  await page.locator('[data-owner-key]').waitFor();
  const identity = await page.evaluate(() => globalThis.__conduitOwnerParticipation.admissionIdentity());
  const window = JSON.parse(owner(['body', 'browser-window', '--state-dir', state, '--expected-host-id', identity.hostId,
    '--new-host-verifying-key', JSON.stringify(identity.verifyingKey), '--maximum-millis', '60000', '--authorize-window']));
  assert.equal(window.body_id, bodyId);
  await page.getByLabel('Body ID').fill(bodyId);
  await page.getByLabel('Owner window URL').fill(window.url);
  await page.getByRole('button', { name: 'Join this Body', exact: true }).click();
  await page.waitForFunction(() => globalThis.__conduitOwnerParticipation?.presence() === 'available', null, { timeout: 20_000 });
  owner(['body', 'start', '--state-dir', state]);
  await page.waitForFunction(() => {
    const view = globalThis.__conduitOwnerParticipation?.face();
    return view?.show_state === 'available' && view.interactions_admitted
      && view.actions?.some(action => action.identity === 'notebook.replace' && action.availability === 'available');
  }, null, { timeout: 20_000 });
  return { page, identity };
}
async function view(page) { return page.evaluate(() => globalThis.__conduitOwnerParticipation.face()); }
async function waitText(page, text) {
  await page.waitForFunction(text => {
    const current = globalThis.__conduitOwnerParticipation?.face();
    return current?.show_state === 'available' && current.interactions_admitted
      && current.subjects?.some(subject => subject.identity === 'notebook/document' && subject.text?.includes(text));
  }, text, { timeout: 15_000 });
}
try {
  call(binary, ['host', 'service', 'install', path.join(output, 'release.json'), '--state-dir', state, '--no-start']);
  installed = JSON.parse(await readFile(path.join(state, 'installation.json'))).product_executable;
  const born = owner(['body', 'own', path.resolve('plots/notebook/main.conduit'), '--state-dir', state, '--name', 'Thoughts'], '');
  const bodyId = JSON.parse(born.trim().split('\n')[0]).biography.body_id;
  const firstBoot = await startService();
  notebook(['new']);
  notebook(['replace', '--revision', '0', '--text', 'I was here.']);
  notebook(['save', '--storage-dir', storage]);
  server = await startStaticProduct(handbook);
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } }); contexts.push(context);
  const first = await enter(context, bodyId);
  await waitText(first.page, 'I was here.');
  const before = await view(first.page);
  await first.page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'notebook-before.png') });
  const edit = first.page.locator('[data-owner-action="notebook.replace"]');
  await edit.getByLabel('Document text').fill('I was here. Again.');
  await edit.getByRole('button', { name: 'Replace text', exact: true }).click();
  await waitText(first.page, 'I was here. Again.');
  assert.equal(notebook(['show']).revision, 2);
  await first.page.locator('[data-owner-action="notebook.save"]').getByRole('button', { name: 'Save', exact: true }).click();
  await first.page.waitForFunction(() => globalThis.__conduitOwnerParticipation.face()?.subjects
    ?.some(subject => subject.identity === 'notebook/save-status' && subject.text?.includes('Saved')), null, { timeout: 15_000 });
  const after = await view(first.page);
  assert.notEqual(after.show_id, before.show_id);
  assert.equal(notebook(['show']).text, 'I was here. Again.');
  await first.page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'notebook-saved.png') });
  const execution = await readFile(path.join(state, 'body/owner-execution.json'));
  await first.page.evaluate(oldView => {
    const button = document.createElement('button'); button.id = 'notebook-stale-probe'; button.textContent = 'Attempt retired edit';
    button.onclick = async event => {
      try {
        await globalThis.__conduitOwnerParticipation.submitFaceInteraction({ view: oldView,
          actionId: 'notebook.replace', target: 'notebook/document', arguments: [{ name: 'text', value: 'Must not replace the document' }] });
        globalThis.__notebookStale = { refused: false, trusted: event.isTrusted };
      } catch (error) { globalThis.__notebookStale = { refused: true, trusted: event.isTrusted, code: error.code, message: error.message }; }
    };
    document.querySelector('[data-owner-face]').append(button);
  }, before);
  await first.page.getByRole('button', { name: 'Attempt retired edit', exact: true }).click();
  await first.page.waitForFunction(() => globalThis.__notebookStale);
  const stale = await first.page.evaluate(() => globalThis.__notebookStale);
  assert.equal(stale.refused, true); assert.equal(stale.trusted, true);
  assert.deepEqual(await readFile(path.join(state, 'body/owner-execution.json')), execution);
  assert.equal(notebook(['show']).text, 'I was here. Again.');
  await stop(service); service = null;
  await first.page.waitForFunction(() => globalThis.__conduitOwnerParticipation.presence() !== 'available', null, { timeout: 15_000 });
  const secondBoot = await startService();
  assert.notEqual(firstBoot.boot_id, secondBoot.boot_id);
  assert.equal(notebook(['show']).text, 'I was here. Again.');
  const replacement = await browser.newContext({ viewport: { width: 1280, height: 900 } }); contexts.push(replacement);
  const second = await enter(replacement, bodyId);
  assert.notEqual(first.identity.hostId, second.identity.hostId);
  await waitText(second.page, 'I was here. Again.');
  await second.page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'notebook-replacement-host.png') });
  await writeFile(path.join(output, 'proof.json'), JSON.stringify({ schema: 'conduit.notebook/browser-proof@1',
    proof_class: 'candidate-live-browser', body_id: bodyId, native_executable_sha256: file.sha256,
    first_boot: firstBoot, second_boot: secondBoot, first_browser_host: first.identity, replacement_browser_host: second.identity,
    before, after, replacement: await view(second.page), stale,
    chrome_version: browser.version(), scope: 'Actual packaged browser render, typed edit and Save; new presentation Host and owner process recover the same committed Body. Native/QEMU acceptance is separate.' }, null, 2));
  assert.deepEqual(errors, [], 'browser or owner errors');
  console.log(`Verified notebook browser proof: ${output}`);
} finally {
  await writeFile(path.join(output, 'commands.json'), JSON.stringify(commands, null, 2));
  await writeFile(path.join(output, 'errors.json'), JSON.stringify(errors, null, 2));
  if (browser) await browser.close();
  for (const child of services) await stop(child);
  if (server) await stop(server.child);
  assert.equal(path.dirname(privateRoot), '/tmp');
  assert.ok(path.basename(privateRoot).startsWith('conduit-notebook-browser-'));
  await rm(privateRoot, { recursive: true, force: true });
}
