// Live, attended-local acceptance over an installed Linux owner and the
// packaged Handbook. No membership or runtime state is constructed by this driver.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawn, spawnSync } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { startStaticProduct } from './static-product-server.mjs';

const [binaryArgument, handbookArgument, outputArgument, playwrightArgument] = process.argv.slice(2);
if (!binaryArgument || !handbookArgument || !outputArgument || !playwrightArgument) {
  throw new Error('usage: handbook-owner-join.mjs INSTALLED-OWNER HANDBOOK-PACKAGE NEW-EVIDENCE-DIR PINNED-PLAYWRIGHT');
}
const binary = path.resolve(binaryArgument);
const handbook = path.resolve(handbookArgument);
const output = path.resolve(outputArgument);
const { chromium } = await import(pathToFileURL(path.resolve(playwrightArgument)));
const digest = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
const ownerBytes = await readFile(binary);
const app = await readFile(path.join(handbook, 'application.application.json'));
const releaseDir = path.join(output, 'release');
await mkdir(releaseDir, { recursive: true });
const name = 'conduit-linux-x86_64';
await writeFile(path.join(releaseDir, name), ownerBytes);
const file = { path: name, bytes: ownerBytes.length, sha256: digest(ownerBytes) };
await writeFile(path.join(releaseDir, 'release.json'), JSON.stringify({
  schema: 'conduit.release/host-bundle@1', source_identity: 'live-handbook-owner-join',
  bundle_sha256: digest(Buffer.from(`conduit.release/host-bundle-content@1\0${name}\0${file.sha256}\n`)),
  files: [file],
}));
const state = path.join(output, 'owner-state');
const install = spawnSync(binary, ['host', 'service', 'install', path.join(releaseDir, 'release.json'), '--state-dir', state, '--no-start'], { encoding: 'utf8' });
assert.equal(install.status, 0, install.stderr);
const installation = JSON.parse(await readFile(path.join(state, 'installation.json')));
const source = path.join(output, 'hello.conduit');
await writeFile(source, 'plot hello {\n show: presentation/text\n "Hello." >> show\n}.');
const owner = spawn(installation.product_executable, ['body', 'own', source, '--state-dir', state], { stdio: ['pipe', 'pipe', 'pipe'] });
const records = [], errors = [], waiters = [];
owner.stderr.on('data', chunk => errors.push(chunk.toString()));
createInterface({ input: owner.stdout }).on('line', line => {
  try { records.push(JSON.parse(line)); } catch (error) { errors.push(String(error)); }
  for (const waiter of [...waiters]) waiter();
});
const waitRecord = predicate => new Promise((resolve, reject) => {
  const timeout = setTimeout(() => { cleanup(); reject(new Error(`owner record deadline: ${errors.join('')}`)); }, 25_000);
  function cleanup() { clearTimeout(timeout); const index = waiters.indexOf(check); if (index >= 0) waiters.splice(index, 1); }
  function check() { const found = records.find(predicate); if (found) { cleanup(); resolve(found); } }
  waiters.push(check); check();
});
let server, browser;
try {
  server = await startStaticProduct(handbook);
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await context.newPage();
  const pageErrors = [], browserMessages = [];
  page.on('pageerror', error => pageErrors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') browserMessages.push(message.text()); });
  const initial = await waitRecord(record => record.schema === 'conduit.body/owner-truth@1');
  await page.goto(`${server.url}?participate=owner#your-handbook`);
  await page.locator('[data-owner-key]').waitFor();
  const identity = await page.evaluate(() => globalThis.__conduitOwnerParticipation.admissionIdentity());
  assert.equal(identity.hostId, await page.locator('[data-owner-host]').textContent());
  assert.equal(identity.bootId, await page.locator('[data-owner-boot]').textContent());
  owner.stdin.write(`${JSON.stringify({ operation: 'admit-browser', expected_host_id: identity.hostId,
    new_host_verifying_key: identity.verifyingKey, maximum_millis: 12000 })}\n`);
  const window = await waitRecord(record => record.schema === 'conduit.body/browser-admission-window@1');
  assert.equal(window.body_id, initial.biography.body_id);
  await page.getByLabel('Body ID').fill(window.body_id);
  await page.getByLabel('Owner window URL').fill(window.url);
  await page.getByRole('button', { name: 'Join this Body' }).click();
  try {
    await page.waitForFunction(() => globalThis.__conduitOwnerParticipation?.presence() === 'available', null, { timeout: 12_000 });
  } catch (error) {
    throw new Error(`browser presence deadline: ${await page.locator('[data-owner-status]').textContent()}; ${pageErrors.join('; ')}; ${browserMessages.join('; ')}`, { cause: error });
  }
  const admitted = await page.evaluate(() => ({
    credential: globalThis.__conduitOwnerParticipation.credential(),
    biography: globalThis.__conduitOwnerParticipation.biography(),
    host: globalThis.__conduitOwnerParticipation.host(),
  }));
  assert.equal(admitted.credential.body_id, initial.biography.body_id);
  assert.equal(admitted.credential.host_id, identity.hostId);
  assert.equal(admitted.credential.boot_id, identity.bootId);
  assert.equal(admitted.biography.membership.parts.length, 2);
  assert.equal(admitted.biography.membership.parts.find(part => part.part_id === admitted.credential.part_id).current.boot_id, identity.bootId);
  assert.equal(await page.locator('[data-owner-result]').textContent(),
    `Body ${window.body_id} admitted Part ${admitted.credential.part_id} on this Host and Boot. The Linux owner remains authoritative; no Plan or Play was transferred.`);
  const screenshot = path.join(output, 'browser-admitted.png');
  await page.screenshot({ path: screenshot, fullPage: true });
  await page.getByRole('button', { name: 'Leave this window' }).click();
  await waitRecord(record => record.schema === 'conduit.body/browser-session-ended@1');
  owner.stdin.write('{"operation":"close"}\n'); owner.stdin.end();
  const code = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error('owner exit deadline')), 20_000);
    owner.once('exit', code => { clearTimeout(timeout); resolve(code); });
  });
  assert.equal(code, 0, errors.join(''));
  assert.deepEqual(pageErrors, []);
  const final = JSON.parse(await readFile(path.join(state, 'body/biography.json')));
  assert.equal(final.body_id, initial.biography.body_id);
  assert.equal(final.membership.parts.length, 2);
  await writeFile(path.join(output, 'report.json'), `${JSON.stringify({
    schema: 'conduit.body/handbook-owner-join@1', ownerSha256: digest(ownerBytes),
    handbookPackageSha256: digest(app), browserBundleSource: JSON.parse(await readFile(path.join(handbook, 'sdk/bundle/conduit-browser-image.json'))).reviewed_distribution.source_commit,
    bodyId: final.body_id, browserHostId: identity.hostId, browserBootId: identity.bootId,
    browserPartId: admitted.credential.part_id, finalPartCount: final.membership.parts.length,
    screenshot: 'browser-admitted.png', screenshotSha256: digest(await readFile(screenshot)),
    ownerWindow: 'loopback', remoteExecution: false, serviceOwned: false,
  }, null, 2)}\n`);
  console.log(`PASS: ${path.join(output, 'report.json')}`);
} finally {
  if (owner.exitCode === null) owner.kill();
  await browser?.close();
  if (server) server.child.kill();
  await writeFile(path.join(output, 'owner-records.json'), JSON.stringify({ records, errors }, null, 2));
}
