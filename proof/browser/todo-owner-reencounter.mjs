// Actual fresh Browser Host encounter of the retained committed Todo; no Todo actions.
// The owner and Handbook package are supplied by the caller; this driver never
// creates a second Body or constructs Face state for the browser.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { startStaticProduct } from './static-product-server.mjs';

const [ownerCwdArg, binaryArg, stateArg, handbookArg, outputArg, playwrightArg,
  itemText = 'Pick up prescription', scenario = 'observe-only'] = process.argv.slice(2);
assert.ok(['observe-only'].includes(scenario), 'unknown Todo capture scenario');
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
const packageManifest = JSON.parse(await readFile(path.join(handbook, 'application.application.json')));
const browserBundle = JSON.parse(await readFile(path.join(handbook, 'sdk/bundle/browser-bundle-release.json')));
const uiResource = packageManifest.resources.find(resource => resource.role === 'owner-participation');
const digest = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
assert.equal(digest(await readFile(path.join(ownerCwd, 'targets/browser/handbook/owner-participation.mjs'))),
  uiResource.sha256, 'packaged Handbook UI differs from this source checkout');
const head = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: ownerCwd, encoding: 'utf8' });
assert.equal(head.status, 0, head.stderr);
const handbookUiSource = head.stdout.trim();
const uiStatus = spawnSync('git', ['status', '--porcelain', '--', 'targets/browser/handbook'],
  { cwd: ownerCwd, encoding: 'utf8' });
assert.equal(uiStatus.status, 0, uiStatus.stderr);
const handbookUiSourceClean = uiStatus.stdout.length === 0;
const sourceRecord = {
  owner_source_commit: installed.release_source_identity,
  browser_runtime_source_commit: browserBundle.source_identity,
  handbook_ui_source_commit: handbookUiSource,
  handbook_ui_source_clean: handbookUiSourceClean,
  handbook_package_digest: packageManifest.package_digest,
  source_relation: installed.release_source_identity === browserBundle.source_identity
    && installed.release_source_identity === handbookUiSource && handbookUiSourceClean
    ? 'exact-source' : 'development-cross-source',
};
const expectedPlaywright = JSON.parse(await readFile(path.join(ownerCwd, 'proof/browser/package.json')))
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
let server, browser, page;
const errors = [];
try {
  server = await startStaticProduct(handbook);
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  page = await context.newPage();
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
  const wardrobeIdle = () => page.waitForFunction(() =>
    document.querySelector('[data-owner-wardrobe-refresh]')?.disabled === false,
  null, { timeout: 12_000 });
  await page.waitForFunction(hostId => {
    try { return JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent)
      .route_descriptions.some(route => route.host_id === hostId); }
    catch { return false; }
  }, identity.hostId, { timeout: 12_000 });
  await wardrobeIdle();
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
    await wardrobeIdle();
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
  await page.getByRole('button', { name: 'Refresh this Face' }).click();
  await page.waitForFunction(() => {
    const view = globalThis.__conduitOwnerParticipation.face();
    return view?.show_state === 'available' && view.interactions_admitted
      && document.querySelector('[data-owner-show-acknowledged]')?.dataset.ownerShowAcknowledged === view.show_id;
  }, null, { timeout: 12_000 });
  const view = await page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  assert.equal(view.body_id, bodyId);
  assert.equal(view.route.mask_host_id, identity.hostId);
  assert.equal(view.route.mask_boot_id, identity.bootId);
  assert.notEqual(identity.hostId, beforeOwner.advertisement.host_id);
  assert.equal(view.route.owner_host_id, beforeOwner.advertisement.host_id);
  assert.equal(view.route.owner_boot_id, beforeOwner.advertisement.boot_id);
  const expectedItems = beforeOwner.presentation.subjects.filter(subject => subject.role === 'Item');
  assert.deepEqual(view.subjects.filter(subject => subject.role === 'Item').map(subject => ({ identity: subject.identity, name: subject.name })),
    expectedItems.map(subject => ({ identity: subject.identity, name: subject.name })));
  const completed = beforeOwner.presentation.properties.filter(property => property.name === 'complete' && property.value.Flag === true).length;
  assert.equal(await page.locator('.owner-face-collection-count').textContent(),
    `${expectedItems.length - completed} things left · ${completed} completed`);
  assert.equal(await page.locator('.owner-face-completed').getAttribute('open'), null);
  await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'browser-reencounter.png') });
  const afterOwner = face();
  assert.equal(afterOwner.presentation.identity, view.face_id);
  const todoProperties = value => value.presentation.properties.filter(property => property.subject.startsWith('todo/'));
  assert.deepEqual(todoProperties(afterOwner), todoProperties(beforeOwner));
  assert.deepEqual(errors, []);
  await writeFile(path.join(output, 'receipt.json'), `${JSON.stringify({
    schema: 'conduit.proof/todo-browser-reencounter@1', ...sourceRecord,
    body_id: bodyId, browser_host_id: identity.hostId, browser_boot_id: identity.bootId,
    owner_host_id: view.route.owner_host_id, owner_boot_id: view.route.owner_boot_id,
    face_id: view.face_id, face_revision: afterOwner.presentation_revision_decimal, show_id: view.show_id,
    show_state: view.show_state, interactions_admitted: view.interactions_admitted,
    mask_plot_id: view.mask_plot_id, mask_plan_id: view.mask_plan_id, mask_play_id: view.mask_play_id,
    route: view.route, todo_subjects: view.subjects.filter(subject => subject.identity.startsWith('todo/')),
    canonical_todo_properties: todoProperties(afterOwner),
    todo_actions_executed: 0, new_birth: false,
    screenshot: 'browser-reencounter.png', page_errors: errors,
    limits: ['A distinct admitted Browser Host; shared physical Linux machine', 'Checkpoint read and synthesis remain on the explicitly selected Owner Host'],
  }, null, 2)}\n`);
} finally {
  await browser?.close();
  server?.child.kill();
}
