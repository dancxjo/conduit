// Live native browser encounter with a retained Thermostat Body. The caller
// supplies its installed owner and exact packaged Handbook. One Chromium,
// context and attempt; no fixture Face, forced clicks or workflow retries.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { startStaticProduct } from './static-product-server.mjs';

const [ownerCwdArg, binaryArg, stateArg, handbookArg, outputArg, playwrightArg] = process.argv.slice(2);
assert.ok(playwrightArg, 'usage: thermostat-owner-browser.mjs OWNER-CWD INSTALLED-BIN STATE HANDBOOK NEW-OUTPUT PINNED-PLAYWRIGHT');
const ownerCwd = path.resolve(ownerCwdArg);
const binary = path.resolve(ownerCwd, binaryArg);
const state = path.resolve(ownerCwd, stateArg);
const handbook = path.resolve(handbookArg);
const output = path.resolve(outputArg);
assert.equal(existsSync(output), false, 'evidence directory must be new');
const installed = JSON.parse(await readFile(path.join(state, 'installation.json')));
assert.equal(path.resolve(ownerCwd, installed.product_executable), binary);
const manifest = JSON.parse(await readFile(path.join(handbook, 'application.application.json')));
const bundle = JSON.parse(await readFile(path.join(handbook, 'sdk/bundle/browser-bundle-release.json')));
const ui = manifest.resources.find(resource => resource.role === 'owner-participation');
const digest = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
assert.equal(digest(await readFile(path.join(ownerCwd, 'targets/browser/handbook/owner-participation.mjs'))), ui.sha256);
const git = args => {
  const result = spawnSync('git', args, { cwd: ownerCwd, encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout.trim();
};
const source = git(['rev-parse', 'HEAD']);
const clean = !git(['status', '--porcelain', '--', 'targets/browser/handbook']);
const sourceRecord = {
  owner_source_commit: installed.release_source_identity,
  browser_runtime_source_commit: bundle.source_identity,
  handbook_ui_source_commit: source,
  handbook_ui_source_clean: clean,
  handbook_package_digest: manifest.package_digest,
  source_relation: clean && source === installed.release_source_identity && source === bundle.source_identity
    ? 'exact-source' : 'development-cross-source',
};
const expectedVersion = JSON.parse(await readFile(new URL('./package.json', import.meta.url))).devDependencies['@playwright/test'];
assert.equal(JSON.parse(await readFile(path.join(path.dirname(playwrightArg), 'package.json'))).version, expectedVersion);
const { chromium } = await import(pathToFileURL(playwrightArg));
const owner = args => {
  const result = spawnSync(binary, args, { cwd: ownerCwd, encoding: 'utf8', timeout: 15_000, maxBuffer: 1024 * 1024 });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
};
const ownerFace = () => {
  const result = owner(['body', 'face', '--state-dir', state, '--json']);
  return { schema: result.schema, presentation: result.presentation, presentation_revision_decimal: result.presentation_revision_decimal };
};
const ownerStatus = () => owner(['body', 'status', '--state-dir', state, '--json']);
const before = ownerFace();
const bodyId = before.presentation.basis.body_id;
await mkdir(output, { mode: 0o700 });
let server, browser, page;
const errors = [];
const revisions = [];
let runBasis;
try {
  server = await startStaticProduct(handbook);
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1280, height: 1000 } });
  page = await context.newPage();
  page.on('pageerror', error => errors.push(error.message));
  // Test-only observation of the actual SDK session returned by normal Join.
  // The admitted SDK uses blob modules, and Handbook exposes only a Host
  // snapshot. Observe the real frozen BrowserHost without another SDK import.
  // Both wrappers delegate unchanged and restore their original functions.
  await page.addInitScript(() => {
    const freeze = Object.freeze;
    Object.freeze = function (value) {
      if (value?.constructor?.name === 'BrowserHost' && typeof value.participate === 'function') {
        const prototype = Object.getPrototypeOf(value);
        const participate = prototype.participate;
        prototype.participate = async function (...args) {
          const session = await Reflect.apply(participate, this, args);
          globalThis.__thermostatProofSession = session;
          prototype.participate = participate;
          return session;
        };
        Object.freeze = freeze;
      }
      return Reflect.apply(freeze, Object, [value]);
    };
  });
  await page.goto(`${server.url}?participate=owner#your-handbook`);
  await page.locator('[data-owner-key]').waitFor();
  const identity = await page.evaluate(() => globalThis.__conduitOwnerParticipation.admissionIdentity());
  const window = owner(['body', 'browser-window', '--state-dir', state,
    '--expected-host-id', identity.hostId, '--new-host-verifying-key', JSON.stringify(identity.verifyingKey),
    '--maximum-millis', '60000', '--authorize-window']);
  assert.equal(window.body_id, bodyId);
  await page.getByLabel('Body ID').fill(bodyId);
  await page.getByLabel('Owner window URL').fill(window.url);
  await page.getByRole('button', { name: 'Join this Body', exact: true }).click();
  await page.waitForFunction(() => globalThis.__conduitOwnerParticipation.presence() === 'available', null, { timeout: 15_000 });
  await page.getByRole('button', { name: 'Inspect current wardrobe', exact: true }).click();
  await page.waitForFunction(hostId => {
    try { return !document.querySelector('[data-owner-wardrobe-refresh]').disabled
      && JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent).route_descriptions.some(route => route.host_id === hostId); }
    catch { return false; }
  }, identity.hostId, { timeout: 15_000 });
  const wardrobe = async () => JSON.parse(await page.locator('[data-owner-wardrobe-evidence]').textContent());
  let currentWardrobe = await wardrobe();
  const route = currentWardrobe.route_descriptions.find(route => route.host_id === identity.hostId);
  for (const verb of ['Wear', 'Prefer only']) {
    const button = page.getByRole('button', { name: `${verb} ${route.mask_name}`, exact: true });
    if (await button.isEnabled()) {
      const prior = currentWardrobe.wardrobe_revision_decimal;
      await button.click();
      await page.waitForFunction(revision => {
        try { return !document.querySelector('[data-owner-wardrobe-refresh]').disabled
          && JSON.parse(document.querySelector('[data-owner-wardrobe-evidence]').textContent).wardrobe_revision_decimal !== revision; }
        catch { return false; }
      }, prior, { timeout: 15_000 });
      currentWardrobe = await wardrobe();
    }
  }
  assert.equal(currentWardrobe.selected.route_id, route.route_id);
  const currentView = () => page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  const acknowledged = () => page.waitForFunction(() => {
    const view = globalThis.__conduitOwnerParticipation.face();
    return view?.show_state === 'available'
      && document.querySelector('[data-owner-show-acknowledged]')?.dataset.ownerShowAcknowledged === view.show_id
      && !document.querySelector('[data-owner-face-refresh]').disabled;
  }, null, { timeout: 15_000 });
  const refresh = async () => {
    await page.getByRole('button', { name: 'Refresh this Face', exact: true }).click();
    await acknowledged();
  };
  await refresh();
  owner(['body', 'start', '--state-dir', state, '--maximum-millis', '900000']);
  const startDeadline = Date.now() + 15_000;
  while (!ownerStatus().realization?.play?.active_play_id) {
    assert.ok(Date.now() < startDeadline, 'the installed Body did not start its retained scan Play');
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  await refresh();
  await page.getByRole('group', { name: 'Mode', exact: true }).waitFor();
  const liveStatus = ownerStatus();
  runBasis = { plan_id: liveStatus.realization.plan.plan_id,
    active_play_id: liveStatus.realization.play.active_play_id };
  assert.ok(runBasis.plan_id && runBasis.active_play_id);
  const observe = async label => {
    const view = await currentView();
    const canonical = ownerFace();
    const status = ownerStatus();
    assert.equal(view.body_id, bodyId);
    assert.equal(view.show_state, 'available');
    assert.equal(view.interactions_admitted, true);
    assert.equal(canonical.presentation.identity, view.face_id);
    assert.equal(status.biography.body_id, bodyId);
    assert.equal(status.realization.plan.plan_id, runBasis.plan_id);
    assert.equal(status.realization.play.active_play_id, runBasis.active_play_id);
    assert.equal(canonical.presentation.basis.plan_id, null);
    assert.equal(canonical.presentation.basis.active_play_id, null);
    const provenance = canonical.presentation.subjects.filter(subject => subject.identity.startsWith('contribution/foreground/'));
    assert.ok(provenance.some(subject => {
      const properties = canonical.presentation.properties.filter(property => property.subject === subject.identity);
      return properties.some(property => property.name === 'plan-id' && property.value.Identity === runBasis.plan_id)
        && properties.some(property => property.name === 'active-play-id' && property.value.Identity === runBasis.active_play_id);
    }), 'canonical foreground contribution must retain this live Body Plan/Play provenance');
    for (const field of ['mask_plot_id', 'mask_plan_id', 'mask_play_id', 'show_id', 'face_id']) assert.ok(view[field]);
    const target = view.subjects.find(subject => subject.name === 'Target temperature');
    assert.ok(target.values.some(value => value.kind === 'value/quantity@1'));
    const observation = { label, observed_at_unix_ms: Date.now(), view, owner: canonical,
      retained_body_execution: { body_id: status.biography.body_id, ...runBasis } };
    revisions.push(observation);
    return observation;
  };
  const targetWording = async expected => {
    const view = await currentView();
    const target = view.subjects.find(subject => subject.name === 'Target temperature');
    assert.equal(target.values.find(value => value.kind === 'value/quantity@1').text, expected);
    assert.equal(await page.locator('[data-face-role="Info"] .owner-face-value').filter({ hasText: expected }).count(), 1);
  };
  const action = async (control, label, keyboard = false) => {
    const prior = await currentView();
    if (keyboard) { await control.focus(); await page.keyboard.press('Enter'); }
    else await control.click();
    await page.waitForFunction(revision => globalThis.__conduitOwnerParticipation.face()?.face_revision !== revision
      && globalThis.__conduitOwnerParticipation.face()?.show_state === 'available', prior.face_revision, { timeout: 15_000 });
    await acknowledged();
    return observe(label);
  };
  const lower = () => page.getByRole('button', { name: 'Lower target by 0.5°C', exact: true });
  const raise = () => page.getByRole('button', { name: 'Raise target by 0.5°C', exact: true });
  await observe('initial');
  await targetWording('21 °C');
  const initialView = await currentView();
  assert.ok(initialView.subjects.find(subject => subject.name === 'Current temperature')
    .text.includes('Sensor unavailable'));
  assert.ok(initialView.subjects.some(subject => subject.role === 'Region'
    && subject.text.includes('Requested settings; equipment operation is unconfirmed.')));
  await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'thermostat-desktop.png'), animations: 'disabled' });
  await action(raise(), 'raise-keyboard', true);
  await targetWording('21.5 °C');
  await action(raise(), 'raise-to-22');
  await action(raise(), 'raise-to-22.5');
  await targetWording('22.5 °C');
  await action(lower(), 'lower');
  for (const family of [
    ['Mode', ['Heat', 'Cool', 'Auto', 'Off']],
    ['Fan', ['On', 'Auto']],
    ['Preset', ['Eco', 'Sleep', 'Comfort']],
  ]) {
    for (const name of family[1]) {
      const radio = page.getByRole('group', { name: family[0], exact: true }).getByRole('radio', { name, exact: true });
      await action(radio, `${family[0]}-${name}`);
      assert.equal(await radio.isChecked(), true);
    }
  }
  await targetWording('21 °C');
  await action(page.getByRole('group', { name: 'Mode', exact: true }).getByRole('radio', { name: 'Cool', exact: true }), 'cool-preset-recalculation');
  await targetWording('24 °C');
  await action(page.getByRole('group', { name: 'Mode', exact: true }).getByRole('radio', { name: 'Off', exact: true }), 'off-preset-recalculation');
  const staleView = await currentView();
  const staleAction = staleView.actions.find(candidate => candidate.name === 'Lower target by 0.5°C');
  await action(raise(), 'advance-before-stale');
  const beforeStale = ownerFace();
  const stale = await page.evaluate(async ({ view, action }) => {
    try {
      const result = await globalThis.__thermostatProofSession.submitOwnerFaceInteraction({
        view, actionId: action.identity, target: action.target, arguments: [], sequence: 9999,
      });
      return { accepted: result.accepted, outcome: result };
    } catch (error) { return { accepted: false, refusal: error.message }; }
  }, { view: staleView, action: staleAction });
  assert.equal(stale.accepted, false, 'previous acknowledged Show must refuse after a new action');
  assert.deepEqual(ownerFace().presentation.properties, beforeStale.presentation.properties, 'stale Show cannot mutate thermostat');
  await refresh();
  await observe('refresh-after-stale');
  await targetWording('21.5 °C');
  for (let target = 220; target <= 300; target += 5) {
    await action(raise(), `upper-limit-${target}`);
    await targetWording(`${target % 10 ? (target / 10).toFixed(1) : target / 10} °C`);
  }
  assert.equal(await raise().isDisabled(), true);
  assert.equal(await lower().isEnabled(), true);
  for (let target = 295; target >= 100; target -= 5) {
    await action(lower(), `lower-limit-${target}`);
    await targetWording(`${target % 10 ? (target / 10).toFixed(1) : target / 10} °C`);
  }
  assert.equal(await lower().isDisabled(), true);
  assert.equal(await raise().isEnabled(), true);
  await action(page.getByRole('group', { name: 'Preset', exact: true }).getByRole('radio', { name: 'Comfort', exact: true }), 'restore-comfort');
  await refresh();
  await observe('same-page-reencounter');
  await page.setViewportSize({ width: 390, height: 844 });
  await page.locator('[data-owner-face]').screenshot({ path: path.join(output, 'thermostat-mobile.png'), animations: 'disabled' });
  await page.locator('conduit-owner-face').screenshot({ path: path.join(output, 'thermostat-mobile-component.png'), animations: 'disabled' });
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
  assert.deepEqual(errors, []);
  await writeFile(path.join(output, 'receipt.json'), `${JSON.stringify({
    schema: 'conduit.proof/thermostat-owner-browser@1', ...sourceRecord,
    proof_class: 'live-local-installed-owner-browser', body_id: bodyId,
    body_plan_id: runBasis.plan_id, body_play_id: runBasis.active_play_id,
    browser_host_id: identity.hostId, browser_boot_id: identity.bootId,
    route, wardrobe: currentWardrobe, revisions, stale_show: { prior: staleView, ...stale },
    same_page_reencounter: true, reload_rejoin: false, physical_sensor_or_equipment: false,
    screenshots: ['thermostat-desktop.png', 'thermostat-mobile.png', 'thermostat-mobile-component.png'],
  }, null, 2)}\n`);
  console.log(`Thermostat owner browser proof passed: ${output}`);
} catch (error) {
  const diagnostics = [writeFile(path.join(output, 'failure.json'), `${JSON.stringify({ error: String(error), ...sourceRecord, page_errors: errors, revisions }, null, 2)}\n`)];
  if (page) {
    diagnostics.push(page.screenshot({ path: path.join(output, 'browser-failed.png'), fullPage: true }));
    diagnostics.push(page.content().then(html => writeFile(path.join(output, 'browser-failed.html'), html)));
    diagnostics.push(currentFailureFace(page).then(value => writeFile(path.join(output, 'browser-failed-face.json'), JSON.stringify(value, null, 2))));
  }
  await Promise.allSettled(diagnostics);
  throw error;
} finally { await browser?.close(); server?.child.kill(); }
function currentFailureFace(page) { return page.evaluate(() => globalThis.__conduitOwnerParticipation?.face()); }
