// Internal acceptance driver: actual installed owner + supplied production SDK.
// Inputs are explicit; the report retains both executable hash and SDK source.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import { spawn, spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { createInterface } from 'node:readline';
import { setTimeout as pause } from 'node:timers/promises';
const [binary, sdk, output, playwrightModule] = process.argv.slice(2).map(value => path.resolve(value));
if (!binary || !sdk || !output || !playwrightModule) throw new Error('binary, SDK directory, evidence directory and pinned Playwright module required');
if (existsSync(output)) throw new Error(`evidence directory already exists: ${output}`);
if (Buffer.byteLength(path.join(output, 'state', 'control.sock')) >= 108) throw new Error('evidence directory is too long for the installed owner control socket');
const { chromium } = await import(pathToFileURL(playwrightModule));
const digest = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
await mkdir(output, { recursive: true });
const bundle = path.join(output, 'bundle');
await mkdir(bundle);
const bytes = await readFile(binary);
const name = 'conduit-linux-x86_64';
await writeFile(path.join(bundle, name), bytes);
const files = [{ path: name, bytes: bytes.length, sha256: digest(bytes) }];
const bundleSha = digest(Buffer.from(`conduit.release/host-bundle-content@1\0${name}\0${files[0].sha256}\n`));
await writeFile(path.join(bundle, 'release.json'), JSON.stringify({ schema: 'conduit.release/host-bundle@1', source_identity: 'local-owner-participant-proof', bundle_sha256: bundleSha, files }));
const state = path.join(output, 'state');
const installed = spawnSync(binary, ['host', 'service', 'install', path.join(bundle, 'release.json'), '--state-dir', state, '--no-start'], { encoding: 'utf8' });
assert.equal(installed.status, 0, installed.stderr);
const installation = JSON.parse(await readFile(path.join(state, 'installation.json')));
const source = path.join(output, 'hello.conduit');
await writeFile(source, 'plot hello {\n show: presentation/text\n "Hello." >> show\n}.');
const owner = spawn(installation.product_executable, ['body', 'own', source, '--state-dir', state], { stdio: ['pipe', 'pipe', 'pipe'] });
const records = [], errors = [], waiters = [];
owner.stderr.on('data', chunk => errors.push(chunk.toString()));
const lines = createInterface({ input: owner.stdout });
lines.on('line', line => {
  try { records.push(JSON.parse(line)); }
  catch (error) { errors.push(String(error)); }
  for (const check of [...waiters]) check();
});
const waitRecord = predicate => new Promise((resolve, reject) => {
  const timer = setTimeout(() => { cleanup(); reject(new Error(`owner record deadline: ${errors.join('')}`)); }, 20000);
  function cleanup() { clearTimeout(timer); const index = waiters.indexOf(check); if (index >= 0) waiters.splice(index, 1); }
  function check() { const record = records.find(predicate); if (record) { cleanup(); resolve(record); } }
  waiters.push(check); check();
});
const exitOf = child => child.exitCode !== null ? Promise.resolve(child.exitCode) : new Promise((resolve, reject) => {
  const timer = setTimeout(() => reject(new Error('owner exit deadline')), 20000);
  child.once('exit', code => { clearTimeout(timer); resolve(code); });
});
const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url, 'http://localhost');
    if (url.pathname === '/') { response.setHeader('Content-Type', 'text/html'); response.end('<!doctype html><title>Owner participant proof</title><main id="host"></main>'); return; }
    const relative = decodeURIComponent(url.pathname).replace(/^\/+/, '');
    const file = path.resolve(sdk, relative);
    if (!file.startsWith(`${sdk}${path.sep}`)) throw new Error('outside SDK');
    response.setHeader('Content-Type', file.endsWith('.wasm') ? 'application/wasm' : /\.m?js$/.test(file) ? 'text/javascript' : 'application/json');
    response.end(await readFile(file));
  } catch { response.statusCode = 404; response.end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext();
const page = await context.newPage();
const pageErrors = [];
page.on('pageerror', error => pageErrors.push(error.message));
let service;
try {
  const initial = await waitRecord(record => record.schema === 'conduit.body/owner-truth@1');
  owner.stdin.write('{"operation":"close"}\n');
  owner.stdin.end();
  assert.equal(await exitOf(owner), 0, errors.join(''));
  service = spawn(installation.product_executable, ['host', 'service', 'run', '--state-dir', state], { stdio: ['ignore', 'ignore', 'pipe'] });
  service.stderr.on('data', chunk => errors.push(chunk.toString()));
  for (let attempt = 0; attempt < 100 && !existsSync(path.join(state, 'control.sock')); attempt++) {
    if (service.exitCode !== null) throw new Error(`installed service exited: ${errors.join('')}`);
    await pause(50);
  }
  assert.equal(existsSync(path.join(state, 'control.sock')), true, `installed service did not start: ${errors.join('')}`);
  const url = `http://127.0.0.1:${server.address().port}/`;
  const openHost = async () => {
    await page.goto(url);
    return page.evaluate(async () => {
      const { Conduit } = await import('/browser-sdk.mjs');
      globalThis.host = await Conduit.browser({ root: document.querySelector('#host') });
      return { host: host.id, boot: host.bootId, verifyingKey: host.admissionIdentity().verifyingKey };
    });
  };
  const first = await openHost();
  const opened = spawnSync(installation.product_executable,
    ['body', 'browser-window', '--state-dir', state, '--expected-host-id', first.host,
      '--new-host-verifying-key', JSON.stringify(first.verifyingKey), '--maximum-millis', '60000', '--authorize-window'],
    { encoding: 'utf8', timeout: 10_000 });
  assert.equal(opened.status, 0, opened.stderr);
  const window = JSON.parse(opened.stdout);
  assert.equal(window.body_id, initial.biography.body_id);
  async function participate() {
    await page.evaluate(async ({ invitation, body }) => {
      globalThis.participant = await host.participate({ invitation, expectedBodyId: body,
        retainedCredential: JSON.parse(localStorage.getItem('owner-credential') ?? 'null'),
        onCredential: credential => localStorage.setItem('owner-credential', JSON.stringify(credential)),
        reconnectPresence: false,
      });
    }, { invitation: window.url, body: initial.biography.body_id });
    await page.waitForFunction(() => participant.presenceState() === 'available');
    return page.evaluate(() => ({ credential: participant.membershipCredential(), biography: participant.biographyEvidence(), offer: participant.offerEvidence() }));
  }
  const joined = await participate();
  assert.equal(joined.credential.body_id, initial.biography.body_id);
  assert.equal(joined.credential.host_id, first.host);
  assert.equal(joined.credential.boot_id, first.boot);
  assert.equal(joined.biography.membership.parts.length, 2);
  assert.equal(joined.offer.stage, 'AdmittedMembership');
  assert.equal(joined.offer.capabilities.length, 0);
  const capability = await page.evaluate(() => {
    const id = participant.advertisement.capabilities[0].capability_id;
    participant.requestOfferEvidence({ capabilityIds: [id] });
    return id;
  });
  await page.waitForFunction(() => participant.offerEvidence()?.stage === 'Planning');
  const detailed = await page.evaluate(() => participant.offerEvidence());
  assert.equal(detailed.proof_class, 'SelfReported');
  assert.equal(detailed.host_id, first.host);
  assert.equal(detailed.boot_id, first.boot);
  assert.equal(detailed.capabilities.length, 1);
  assert.equal(detailed.capabilities[0].capability_id, capability);
  assert.equal(detailed.capability_summary.length, 0);
  assert.equal(detailed.offer_generation, joined.offer.offer_generation);
  const persisted = JSON.parse(await readFile(path.join(state, 'body/biography.json')));
  assert.equal(persisted.body_id, initial.biography.body_id);
  assert.equal(persisted.membership.parts.find(part => part.part_id === joined.credential.part_id).current.boot_id, first.boot);
  // Real page teardown drops the first carrier; the owner must retain its Part offline.
  await page.goto('about:blank');
  let lost;
  for (let attempt = 0; attempt < 100; attempt++) {
    lost = JSON.parse(await readFile(path.join(state, 'body/biography.json')));
    if (lost.membership.parts.find(part => part.part_id === joined.credential.part_id)?.current === null) break;
    await pause(50);
  }
  assert.equal(lost.membership.parts.find(part => part.part_id === joined.credential.part_id).current, null);
  const second = await openHost();
  assert.equal(second.host, first.host);
  assert.notEqual(second.boot, first.boot);
  const returned = await participate();
  assert.equal(returned.credential.part_id, joined.credential.part_id);
  assert.equal(returned.credential.boot_id, second.boot);
  assert.equal(returned.biography.membership.parts.length, 2);
  // Keep this carrier open until the explicit finite admission window closes.
  await page.goto('about:blank');
  let final;
  for (let attempt = 0; attempt < 100; attempt++) {
    final = JSON.parse(await readFile(path.join(state, 'body/biography.json')));
    if (final.membership.parts.find(part => part.part_id === joined.credential.part_id)?.current === null) break;
    await pause(50);
  }
  assert.equal(final.body_id, initial.biography.body_id);
  assert.equal(final.membership.parts.find(part => part.part_id === joined.credential.part_id).current, null);
  assert.deepEqual(pageErrors, []);
  const image = JSON.parse(await readFile(path.join(sdk, 'bundle/conduit-browser-image.json')));
  await writeFile(path.join(output, 'report.json'), JSON.stringify({ schema: 'conduit.body/browser-owner-proof@1',
    executableSha256: files[0].sha256, guestSource: image.reviewed_distribution.source_commit,
    first, second, joined, planningOffer: detailed, returned, final, records, pageErrors,
    remoteExecution: false, sameCommitClaim: false, serviceOwned: true,
  }, null, 2));
  console.log(`PASS: ${path.join(output, 'report.json')}`);
} finally {
  if (owner.exitCode === null) owner.kill();
  if (service?.exitCode === null) service.kill();
  await browser.close();
  server.closeAllConnections();
  await new Promise(resolve => server.close(resolve));
  await writeFile(path.join(output, 'owner-records.json'), JSON.stringify({ records, errors, pageErrors }, null, 2));
}
