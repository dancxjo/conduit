import { inspectHandbookPlot, assertHandbookInspection } from './handbook-patchbay-route.mjs';
import { readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { expect, test } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { openStaticProfile, stageStaticApplications } from './static-body-application-support.mjs';

test.describe.configure({ mode: 'serial', retries: 0 });
const truth = page => page.evaluate(() => globalThis.__conduitApplication.snapshot());
const face = page => page.evaluate(() => globalThis.__conduitApplication.thermostat.face());
async function ready(page) {
  await expect(page.locator('.handbook-controls').getByRole('button', { name: 'Lull', exact: true }))
    .toBeEnabled({ timeout: 30_000 });
  await page.waitForFunction(() => Boolean(globalThis.__conduitApplication));
  return truth(page);
}
function samePlay(before, after) {
  for (const key of ['hostId', 'bootId', 'bodyId', 'planId', 'playId', 'selectedPlot']) expect(after[key], key).toBe(before[key]);
}
function selected(face, id) {
  return face.properties.find(property => property.subject === id && property.name === 'selected')?.value.Flag;
}
function exactTarget(face, numerator, denominator = 1n) {
  const quantity = face.properties.find(property => property.subject === 'thermostat/target' && property.name === 'value').value.TypedValue;
  const bytes = quantity.bytes;
  expect(bytes[0]).toBe(1); // canonical quantity encoding version
  expect(bytes[1]).toBe(1); // affine temperature point, not a delta
  const offset = bytes.length - 18;
  const exponent = Buffer.from(bytes).readInt16LE(offset);
  let coefficient = 0n;
  for (let index = 15; index >= 0; --index) coefficient = (coefficient << 8n) | BigInt(bytes[offset + 2 + index]);
  if (coefficient >> 127n) coefficient -= 1n << 128n;
  const scale = 10n ** BigInt(Math.abs(exponent));
  expect(exponent < 0 ? coefficient * denominator : coefficient * scale * denominator)
    .toBe(exponent < 0 ? BigInt(numerator) * scale : BigInt(numerator));
  return quantity;
}
async function refuse(page, request) {
  return page.evaluate(async request => {
    try { const result = await globalThis.__conduitApplication.thermostat.interact(request); return { accepted: true, result }; }
    catch (error) { return { accepted: false, message: error.message, code: error.code ?? null }; }
  }, request);
}
function requestFor(receipt, id, sequence) {
  const action = receipt.view.actions.find(action => action.identity === id);
  return { view: receipt.view, actionId: id, target: action.target, arguments: [], sequence };
}

test('static Handbook Thermostat uses one real local Play, acknowledged Face actions, refusals and recovery', async ({}, testInfo) => {
  testInfo.setTimeout(180_000);
  const site = await stageStaticApplications();
  const origin = new URL(site.url).origin;
  const url = new URL('handbook/', site.url).href;
  let session = await openStaticProfile(testInfo.outputPath('thermostat-profile'), origin);
  const receipts = { schema: 'conduit.browser/thermostat-local-proof@1',
    source_head: process.env.CONDUIT_CHECKOUT_SHA ?? null,
    runtime_sha256: createHash('sha256').update(await readFile('target/handbook-static/sdk/bundle/runtime.wasm')).digest('hex') };
  try {
    const page = await session.context.newPage();
    await page.goto(url);
    const born = await ready(page);
    receipts.browser = await page.evaluate(() => navigator.userAgent);
    await page.getByLabel('Choose an example', { exact: true }).selectOption('thermostat/main');
    const source = page.getByRole('textbox', { name: 'Plot source', exact: true });
    receipts.source = await source.inputValue();
    expect(receipts.source).toContain('thermostat/combine');
    await page.getByRole('button', { name: 'Try in my Handbook', exact: true }).click();
    await expect(page.locator('[data-check]')).toHaveText('Checked thermostat/main.');
    const thermostat = page.locator('.handbook-show [data-resident-plot]:visible conduit-owner-face');
    await expect(thermostat.getByText('Sensor unavailable', { exact: true })).toBeVisible();
    await expect(thermostat.getByText('Requested settings; equipment operation is unconfirmed.', { exact: true })).toBeVisible();
    await expect(thermostat.getByRole('button', { name: 'Lull', exact: true })).toBeHidden();
    const bodyMenu = thermostat.locator('[data-body-menu] > summary');
    await bodyMenu.click();
    await expect(thermostat.getByRole('button', { name: 'Lull', exact: true })).toBeDisabled();
    await bodyMenu.click();
    receipts.initial = await face(page);
    const playing = await truth(page);
    expect(playing.bodyId).toBe(born.bodyId);
    for (const key of ['hostId', 'bootId', 'bodyId', 'planId', 'playId', 'selectedPlot']) expect(playing[key], key).toBeTruthy();
    expect(receipts.initial.body_plan_id).toBe(playing.planId);
    expect(receipts.initial.active_play_id).toBe(playing.playId);
    const planned = playing.realization.plan.plots.find(entry => entry.plot.checked_plot_id === playing.selectedPlot);
    expect(planned).toBeTruthy();
    const scan = planned.plan.activations.find(entry => entry.Scan)?.Scan;
    expect(scan).toBeTruthy();
    expect(scan.selected_plan_id).toBe(scan.selected_plan.plan_id);
    const combine = scan.selected_plan.fragments.flatMap(fragment => fragment.placements)
      .find(placement => placement.implementation_id === 'browser/kernel-thermostat-combine@1');
    expect(combine).toBeTruthy();
    expect(combine.source_span).toBeTruthy();
    expect(combine.host_id).toBe(playing.hostId);
    expect(combine.boot_id).toBe(playing.bootId);
    receipts.selectedBack = combine;
    receipts.initialQuantity = exactTarget(receipts.initial.face, 21);
    expect(selected(receipts.initial.face, 'thermostat.mode.off')).toBe(true);
    const accessibility = await new AxeBuilder({ page })
      .include('.handbook-show conduit-owner-face').analyze();
    expect(accessibility.violations).toEqual([]);
    receipts.accessibility = { violations: accessibility.violations, passes: accessibility.passes.length };
    await page.screenshot({ path: testInfo.outputPath('01-thermostat-born.png'), fullPage: true });

    for (const [group, choices] of [
      ['Mode', ['Heat', 'Cool', 'Auto', 'Off', 'Heat']],
      ['Fan', ['On', 'Auto', 'On']],
      ['Preset', ['Eco', 'Sleep', 'Comfort', 'Eco']],
    ]) {
      for (const choice of choices) {
        await thermostat.getByRole('group', { name: group, exact: true })
          .getByRole('radio', { name: choice, exact: true }).check();
        await expect(thermostat.getByRole('group', { name: group, exact: true })
          .getByRole('radio', { name: choice, exact: true })).toBeChecked();
        await expect(thermostat.getByRole('group', { name: group, exact: true })
          .getByRole('radio', { name: choice, exact: true })).toBeEnabled();
        const settled = await face(page);
        expect(settled.view.show_state).toBe('available');
        expect(settled.view.interactions_admitted).toBe(true);
        expect(selected(settled.face, `thermostat.${group.toLowerCase()}.${choice.toLowerCase()}`)).toBe(true);
        receipts[`${group}-${choice}`] = settled;
        samePlay(playing, await truth(page));
      }
    }
    receipts.eco = await face(page);
    await thermostat.getByRole('button', { name: 'Raise target by 0.5°C', exact: true }).focus();
    await page.keyboard.press('Enter');
    await expect(thermostat.getByRole('radio', { name: 'Eco', exact: true })).not.toBeChecked();
    receipts.raised = await face(page);
    samePlay(playing, await truth(page));
    expect(receipts.raised.face.revision).toBeGreaterThan(receipts.eco.face.revision);
    expect(selected(receipts.raised.face, 'thermostat.mode.heat')).toBe(true);
    expect(selected(receipts.raised.face, 'thermostat.fan.on')).toBe(true);
    const ecoQuantity = exactTarget(receipts.eco.face, 18);
    receipts.raisedQuantity = exactTarget(receipts.raised.face, 37, 2n);
    expect(receipts.raisedQuantity.contract).toEqual(ecoQuantity.contract);
    expect(receipts.raisedQuantity.bytes.slice(0, -18)).toEqual(ecoQuantity.bytes.slice(0, -18));

    for (const [field, value] of [['show_id', 'forged/thermostat/show'], ['face_id', 'forged/thermostat/face']]) {
      const wrongIdentity = requestFor(receipts.raised, 'thermostat.raise', 9_999);
      wrongIdentity.view = { ...wrongIdentity.view, [field]: value };
      receipts[field] = await refuse(page, wrongIdentity);
      expect(receipts[field].accepted).toBe(false);
      expect((await face(page)).face).toEqual(receipts.raised.face);
      samePlay(playing, await truth(page));
    }

    receipts.stale = await refuse(page, requestFor(receipts.eco, 'thermostat.raise', 10_000));
    expect(receipts.stale.accepted).toBe(false);
    const forged = requestFor(receipts.raised, 'thermostat.raise', 10_001);
    forged.actionId = 'thermostat.forged';
    receipts.forged = await refuse(page, forged);
    expect(receipts.forged.accepted).toBe(false);
    expect((await face(page)).face).toEqual(receipts.raised.face);
    samePlay(playing, await truth(page));

    // Each click is one deliberate owner action, with a settled Face before the
    // next action. No retry or action-performing polling hides refusal behavior.
    for (let step = 0; step < 23; ++step) {
      await thermostat.getByRole('button', { name: 'Raise target by 0.5°C', exact: true }).click();
      await expect.poll(async () => (await face(page)).face.revision).toBe(receipts.raised.face.revision + step + 1);
    }
    await expect(thermostat.getByRole('button', { name: 'Raise target by 0.5°C', exact: true })).toBeDisabled();
    receipts.maximum = await face(page);
    exactTarget(receipts.maximum.face, 30);
    receipts.range = await refuse(page, requestFor(receipts.maximum, 'thermostat.raise', 10_002));
    expect(receipts.range.accepted).toBe(false);
    expect((await face(page)).face).toEqual(receipts.maximum.face);
    samePlay(playing, await truth(page));
    await page.screenshot({ path: testInfo.outputPath('02-thermostat-limit.png'), fullPage: true });

    // Exhaust the authored scan through acknowledged actions in this same Body.
    let bounded = receipts.maximum;
    for (let revision = Number(bounded.face.revision) + 1; revision <= 256; ++revision) {
      const mode = selected(bounded.face, 'thermostat.mode.heat') ? 'Cool' : 'Heat';
      await thermostat.getByRole('group', { name: 'Mode', exact: true })
        .getByRole('radio', { name: mode, exact: true }).click();
      await expect.poll(async () => {
        const current = await face(page);
        return { revision: current.face.revision, show: current.view.show_state };
      }).toEqual({ revision, show: 'available' });
      await expect(thermostat.getByRole('group', { name: 'Mode', exact: true })
        .getByRole('radio', { name: new RegExp(`^${mode}(?:$| )`) })).toBeChecked();
      bounded = await face(page);
    }
    receipts.exhausted = bounded;
    exactTarget(bounded.face, 30);
    await expect(thermostat.getByRole('radio', { name: /^Heat(?:$| )/ })).toBeDisabled();
    receipts.bound = await refuse(page, requestFor(bounded, 'thermostat.mode.heat', 10_003));
    expect(receipts.bound.accepted).toBe(false);
    expect((await face(page)).face).toEqual(bounded.face);
    samePlay(playing, await truth(page));

    await page.getByRole('button', { name: 'Open in Patchbay', exact: true }).click();
    await inspectHandbookPlot(page, playing.selectedPlot, playing.bodyId);
    const graph = await assertHandbookInspection(page, playing, { requireCord: false });
    await expect(graph).toContainText('flow/scan');
    await expect(graph).toContainText('conduit.thermostat/command@1');
    await page.locator('.handbook-application').getByText('Your body and browser', { exact: true }).click();
    const identities = page.locator('[data-identities]');
    await expect(identities).toBeVisible();
    await expect(identities).toContainText('thermostat/combine');
    await expect(identities).toContainText('browser/kernel-thermostat-combine@1');
    const inspected = JSON.parse(await identities.textContent());
    samePlay(playing, inspected);
    receipts.inspectedPlan = inspected.realization.plan;
    await page.locator('.handbook-application').getByText('Your body and browser', { exact: true }).click();
    await page.screenshot({ path: testInfo.outputPath('03-thermostat-patchbay.png'), fullPage: true });
    await page.getByRole('button', { name: 'Return to example', exact: true }).click();
    receipts.beforeLull = await truth(page);
    await page.locator('.handbook-controls').getByRole('button', { name: 'Lull', exact: true }).click();
    await expect(page.locator('.handbook-controls').getByRole('button', { name: 'Wake', exact: true })).toBeEnabled();
    await expect(thermostat.getByRole('radio', { name: /^Heat(?:$| )/ })).toBeDisabled();
    expect((await truth(page)).playId ?? null).toBeNull();
    await page.locator('.handbook-controls').getByRole('button', { name: 'Wake', exact: true }).click();
    receipts.awake = await ready(page);
    expect(receipts.awake.bodyId).toBe(playing.bodyId);
    expect(receipts.awake.playId).not.toBe(playing.playId);
    exactTarget((await face(page)).face, 21);
    await page.reload();
    receipts.recovered = await ready(page);
    expect(receipts.recovered.hostId).toBe(playing.hostId);
    expect(receipts.recovered.bodyId).toBe(playing.bodyId);
    expect(receipts.recovered.bootId).not.toBe(playing.bootId);
    expect(receipts.recovered.playId).not.toBe(receipts.awake.playId);
    expect(receipts.recovered.selectedPlot).toBe(playing.selectedPlot);
    exactTarget((await face(page)).face, 21);
    session.assertClean();
    await session.context.close();
    session = await openStaticProfile(testInfo.outputPath('thermostat-independent-profile'), origin);
    const independent = await session.context.newPage();
    await independent.goto(url);
    receipts.independent = await ready(independent);
    expect(receipts.independent.hostId).not.toBe(playing.hostId);
    expect(receipts.independent.bodyId).not.toBe(playing.bodyId);
    session.assertClean();
    await writeFile(testInfo.outputPath('thermostat-runtime-evidence.json'), `${JSON.stringify(receipts, null, 2)}\n`);
  } finally {
    await session?.context.close();
    await site.close();
  }
});
