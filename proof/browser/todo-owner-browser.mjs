// Live browser Todo action against one already running installed Body owner.
// The owner and Handbook package are supplied by the caller; this driver never
// creates a second Body or constructs Face state for the browser.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { startStaticProduct } from './static-product-server.mjs';

const [ownerCwdArg, binaryArg, stateArg, handbookArg, outputArg, playwrightArg,
  itemText = 'Pick up prescription'] = process.argv.slice(2);
if (!ownerCwdArg || !binaryArg || !stateArg || !handbookArg || !outputArg || !playwrightArg) {
  throw new Error('usage: todo-owner-browser.mjs OWNER-CWD INSTALLED-OWNER OWNER-STATE HANDBOOK-PACKAGE NEW-EVIDENCE-DIR PINNED-PLAYWRIGHT [NEW-ITEM-TEXT]');
}
const ownerCwd = path.resolve(ownerCwdArg);
const binary = path.resolve(ownerCwd, binaryArg);
const state = path.resolve(ownerCwd, stateArg);
const handbook = path.resolve(handbookArg);
const output = path.resolve(outputArg);
assert.equal(existsSync(output), false, 'Todo evidence directory must be new');
assert.ok(itemText && Buffer.byteLength(itemText, 'utf8') <= 64, 'Todo text must fit its admitted bound');
const installed = JSON.parse(await readFile(path.join(state, 'installation.json')));
assert.equal(path.resolve(ownerCwd, installed.product_executable), binary,
  'use the owner installed for this Body');
const expectedPlaywright = JSON.parse(await readFile(new URL('./package.json', import.meta.url)))
  .devDependencies['@playwright/test'];
const actualPlaywright = JSON.parse(await readFile(path.join(path.dirname(playwrightArg), 'package.json'))).version;
assert.equal(actualPlaywright, expectedPlaywright, 'use pinned Playwright');
const { chromium } = await import(pathToFileURL(playwrightArg));
const owner = args => {
  const result = spawnSync(binary, args, { cwd: ownerCwd, encoding: 'utf8', timeout: 10_000 });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
};
const face = () => owner(['body', 'face', '--state-dir', state, '--json']);
const beforeOwner = face();
const bodyId = beforeOwner.presentation.basis.body_id;
await mkdir(output, { mode: 0o700 });
let server, browser;
try {
  server = await startStaticProduct(handbook);
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(`${server.url}?participate=owner#your-handbook`);
  await page.locator('[data-owner-key]').waitFor();
  const identity = await page.evaluate(() => globalThis.__conduitOwnerParticipation.admissionIdentity());
  const window = owner(['body', 'browser-window', '--state-dir', state,
    '--expected-host-id', identity.hostId,
    '--new-host-verifying-key', JSON.stringify(identity.verifyingKey),
    '--maximum-millis', '60000', '--authorize-window']);
  assert.equal(window.body_id, bodyId);
  await page.getByLabel('Body ID').fill(bodyId);
  await page.getByLabel('Owner window URL').fill(window.url);
  await page.getByRole('button', { name: 'Join this Body' }).click();
  await page.waitForFunction(() => globalThis.__conduitOwnerParticipation?.presence() === 'available',
    null, { timeout: 12_000 });
  await page.getByRole('button', { name: 'Inspect current wardrobe' }).click();
  const wardrobe = async () => JSON.parse(await page.locator('[data-owner-wardrobe-evidence]').textContent());
  await page.waitForFunction(hostId => {
    try { return JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent)
      .route_descriptions.some(route => route.host_id === hostId); }
    catch { return false; }
  }, identity.hostId, { timeout: 12_000 });
  let currentWardrobe = await wardrobe();
  const description = currentWardrobe.route_descriptions.find(route => route.host_id === identity.hostId);
  assert.ok(description, 'owner must describe this browser Mask route');
  const changeWardrobe = async (verb) => {
    const previous = currentWardrobe.wardrobe_revision_decimal;
    await page.getByRole('button', { name: `${verb} ${description.mask_name}`, exact: true }).click();
    await page.waitForFunction(prior => {
      try { return JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent)
        .wardrobe_revision_decimal !== prior; } catch { return false; }
    }, previous, { timeout: 12_000 });
    currentWardrobe = await wardrobe();
  };
  if (await page.getByRole('button', { name: `Wear ${description.mask_name}`, exact: true }).isEnabled()) {
    await changeWardrobe('Wear');
  }
  if (await page.getByRole('button', { name: `Prefer only ${description.mask_name}`, exact: true }).isEnabled()) {
    await changeWardrobe('Prefer only');
  }
  assert.equal(currentWardrobe.selected?.route_id, description.route_id,
    'browser Mask must be selected before the Todo Play');
  const beforeStart = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  owner(['body', 'start', '--state-dir', state, '--maximum-millis', '60000',
    '--todo-new-list', 'Groceries']);
  let startedFace;
  for (let attempt = 0; attempt < 100; attempt++) {
    startedFace = face();
    if (startedFace.presentation.actions.some(action => action.identity === 'todo.add'
      && action.availability === 'Available')) break;
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  assert.ok(startedFace.presentation.actions.some(action => action.identity === 'todo.add'
    && action.availability === 'Available'), 'Todo Play did not admit the add action');
  assert.ok(!startedFace.presentation.subjects.some(subject => subject.name === itemText),
    'new Todo item must be absent before this browser action');
  await page.getByRole('button', { name: 'Refresh this Face' }).click();
  await page.waitForFunction(prior => {
    const current = globalThis.__conduitOwnerParticipation.face();
    return current?.face_revision !== prior && current?.actions.some(action => action.identity === 'todo.add');
  }, beforeStart.face_revision, { timeout: 12_000 });
  const prepared = await page.evaluate(() => ({
    face: globalThis.__conduitOwnerParticipation.face(),
    status: document.querySelector('[data-owner-face-status]')?.textContent,
    action: document.querySelector('[data-owner-action="todo.add"]')?.outerHTML,
  }));
  if (!prepared.face.interactions_admitted || prepared.face.show_state !== 'available') {
    await writeFile(path.join(output, 'diagnostic.json'), `${JSON.stringify(prepared, null, 2)}\n`);
    await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'browser-route-diagnostic.png') });
    throw new Error(`Todo browser Show has no admitted action return: ${prepared.status}`);
  }
  const before = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  assert.equal(before.body_id, bodyId);
  assert.equal(before.show_state, 'available');
  assert.equal(await page.locator('.owner-face-collection').count(), 1);
  assert.equal(await page.locator('.owner-face-collection-count').count(), 1);
  assert.equal(await page.locator('[data-owner-face-document] [data-face-role="Status"]').count(), 0,
    'matching progress wording must not appear twice');
  await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'browser-before.png') });
  const add = page.locator('[data-owner-action="todo.add"]');
  await add.getByRole('textbox', { name: 'Item text' }).fill(itemText);
  await add.getByRole('button', { name: 'add an item' }).click();
  await page.waitForFunction(({ prior, name }) => {
    const current = globalThis.__conduitOwnerParticipation.face();
    return current?.face_revision !== prior && current?.subjects.some(subject => subject.name === name)
      && document.querySelector('[data-owner-show-acknowledged]')?.dataset.ownerShowAcknowledged === current.show_id;
  }, { prior: before.face_revision, name: itemText }, { timeout: 12_000 });
  const after = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  assert.equal(after.body_id, bodyId);
  assert.notEqual(after.show_id, before.show_id);
  assert.equal(await page.locator('.owner-face-items li').filter({ hasText: itemText }).count(), 1);
  await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'browser-after.png') });
  const afterOwner = face();
  assert.equal(afterOwner.presentation.basis.body_id, bodyId);
  assert.ok(afterOwner.presentation.subjects.some(subject => subject.name === itemText));
  assert.deepEqual(errors, []);
  await writeFile(path.join(output, 'receipt.json'), `${JSON.stringify({
    schema: 'conduit.proof/todo-owner-browser@1', source_commit: installed.release_source_identity,
    body_id: bodyId, browser_host_id: identity.hostId, browser_boot_id: identity.bootId,
    item_text: itemText, before: { face_id: before.face_id, face_revision: before.face_revision,
      show_id: before.show_id }, after: { face_id: after.face_id,
      face_revision: after.face_revision, show_id: after.show_id },
    screenshots: ['browser-before.png', 'browser-after.png'],
  }, null, 2)}\n`);
} finally {
  await browser?.close();
  server?.child.kill();
}
