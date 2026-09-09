import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";

let entrance, pageErrors;
test.beforeEach(async ({ page }) => { pageErrors = []; page.on("pageerror", error => pageErrors.push(error.message)); entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/"); });
test.afterEach(async ({ page }, info) => {
  if (info.status !== info.expectedStatus) console.error((await page.locator('body').innerText()).slice(0, 6000));
  entrance?.child.kill(); expect(pageErrors).toEqual([]);
});

const current = page => page.evaluate(() => globalThis.__conduitWorkspace.current());
const card = (page, title) => page.locator('[data-application-key^="library-form-"]').filter({ hasText: title });
const openLibrary = page => page.getByRole("button", { name: "+ Forms", exact: true }).click();
async function birth(page) {
  await page.goto(entrance.url);
  const chime = page.getByRole("checkbox", { name: "Startup Chime", exact: true });
  await chime.uncheck();
  await page.getByLabel("Friendly Body name", { exact: true }).fill("Roseau");
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
}

test("Use installs into the same Body; repeated Use preserves Play and removal survives reload", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1280, height: 1000 });
  await birth(page);
  const initial = await current(page);
  await openLibrary(page);
  await expect(card(page, "Memory Lantern")).toContainText("In your Body");
  await page.screenshot({ path: testInfo.outputPath("workspace-form-library.png"), fullPage: true });
  await page.getByRole("textbox", { name: "Find a Form", exact: true }).fill("desk");
  await card(page, "Desk Telegraph").getByRole("button", { name: "Use", exact: true }).click();
  await expect(page.locator("#surface-title")).toHaveText("Desk Telegraph");
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const installed = await current(page);
  expect(installed.body_id).toBe(initial.body_id);
  expect(installed.here_part_id).toBe(initial.here_part_id);
  expect(installed.workload_revision).toBe(1);
  expect(installed.active_play_id).not.toBe(initial.active_play_id);
  expect(installed.initial_forms).toHaveLength(2);
  await page.keyboard.type("hello");
  await page.keyboard.press("Enter");
  await expect(page.locator("[data-form-output] output:visible")).toHaveText("hello");
  await page.keyboard.type("again");
  await page.keyboard.press("Enter");
  await expect(page.locator("[data-form-output] output:visible")).toHaveText("again");
  await openLibrary(page);
  await card(page, "Desk Telegraph").getByRole("button", { name: "Use", exact: true }).click();
  expect((await current(page)).active_play_id).toBe(installed.active_play_id);
  await expect(page.locator("[data-form-output] output:visible")).toHaveText("again");
  await page.screenshot({ path: testInfo.outputPath("workspace-installed-form.png"), fullPage: true });
  await openLibrary(page);
  await card(page, "Desk Telegraph").getByRole("button", { name: "Remove", exact: true }).click();
  await expect(card(page, "Desk Telegraph")).toContainText("Not in your Body");
  await page.getByRole("button", { name: "Back to the surface", exact: true }).click();
  await expect(page.locator("#surface-title")).toHaveText("Memory Lantern");
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  await page.locator("#form-input").focus();
  await page.keyboard.press("r");
  await expect(page.locator("[data-form-output] output:visible")).toHaveText("r");
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const restored = await current(page);
  expect(restored.body_id).toBe(initial.body_id);
  expect(restored.workload_revision).toBe(2);
  expect(restored.initial_forms).toEqual(initial.initial_forms);
});

test("removing the final Form retains an empty Body that can acquire Forms again", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await birth(page);
  const initial = await current(page);
  await openLibrary(page);
  await card(page, "Memory Lantern").getByRole("button", { name: "Remove", exact: true }).click();
  await expect(card(page, "Memory Lantern")).toContainText("Not in your Body");
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  expect((await current(page)).initial_forms).toHaveLength(0);
  expect((await current(page)).active_play_id).toBeUndefined();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.screenshot({ path: testInfo.outputPath("workspace-empty-library-narrow.png"), fullPage: true });
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  expect((await current(page)).body_id).toBe(initial.body_id);
  await openLibrary(page);
  await card(page, "Memory Lantern").getByRole("button", { name: "Use", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  await expect(page.getByRole("button", { name: "Lull Body", exact: true })).toBeVisible();
  await page.keyboard.press("a");
  await expect(page.locator("[data-form-output] output:visible")).toHaveText("a");
  expect((await current(page)).body_id).toBe(initial.body_id);
});

test("a failed workset save stops before replacement effects and reload recovers the saved workload", async ({ page }) => {
  await page.addInitScript(() => {
    const put = IDBObjectStore.prototype.put;
    globalThis.__failedWorksetWrites = 0;
    IDBObjectStore.prototype.put = function(record, ...args) {
      if (record.key === "body-session" && typeof record.value === "string") {
        const value = JSON.parse(record.value);
        if (value.evidence?.body.workload_revision > 0) {
          globalThis.__failedWorksetWrites++;
          throw new DOMException("Injected workset storage exhaustion", "QuotaExceededError");
        }
      }
      return put.call(this, record, ...args);
    };
  });
  await birth(page);
  const initial = await current(page);
  await openLibrary(page);
  await card(page, "Desk Telegraph").getByRole("button", { name: "Use", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Stopped");
  const failed = await page.evaluate(() => ({ current: globalThis.__conduitWorkspace.current(), state: globalThis.__conduitWorkspace.state(), writes: globalThis.__failedWorksetWrites }));
  expect(failed.writes).toBe(1);
  expect(failed.current.active_play_id).toBeUndefined();
  expect(failed.state.terminal.disposition).toBe("cancelled");
  expect(failed.state.terminal.active_play_id).toBe(initial.active_play_id);
  await expect(page.locator("[data-form-output] output")).toHaveCount(0);
  await page.reload();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const restored = await current(page);
  expect(restored.body_id).toBe(initial.body_id);
  expect(restored.workload_revision).toBe(0);
  expect(restored.initial_forms).toEqual(initial.initial_forms);
});

async function reviewedForm(page, name) {
  const source = await (await page.request.get(new URL('forms/initial-body.conduit', entrance.url).href)).text();
  return page.evaluate(({ source, name }) => {
    const api = globalThis.__conduitWorkspace.host.runtime;
    const bytes = new TextEncoder().encode(source);
    const pointer = api.conduit_creche_input_ptr();
    new Uint8Array(api.memory.buffer, pointer, bytes.length).set(bytes);
    if (api.conduit_creche_reviewed_inventory(bytes.length) < 0) throw new Error('Inventory refused');
    const inventory = JSON.parse(new TextDecoder().decode(new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len())));
    return inventory.forms.find(form => form.name === name);
  }, { source, name });
}
function handoffUrl(form) {
  const url = new URL(entrance.url);
  url.search = new URLSearchParams({ form: form.name, source_document_id: form.source_document_id, checked_form_id: form.checked_form_id });
  return url.href;
}

test('a Gallery handoff installs in the retained Body and cannot reinstall a later removed Form on reload', async ({ page }) => {
  await birth(page);
  const initial = await current(page);
  const form = await reviewedForm(page, 'desk_telegraph');
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.goto(handoffUrl(form));
  await expect(page.locator('#surface-title')).toHaveText('Desk Telegraph');
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  expect((await current(page)).body_id).toBe(initial.body_id);
  expect((await current(page)).initial_forms).toHaveLength(2);
  expect(new URL(page.url()).search).toBe('');
  await openLibrary(page);
  await card(page, 'Desk Telegraph').getByRole('button', { name: 'Remove', exact: true }).click();
  await expect(card(page, 'Desk Telegraph')).toContainText('Not in your Body');
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  expect((await current(page)).initial_forms).toEqual(initial.initial_forms);
});

test('a new Gallery arrival selects the Form in the actual Crèche; a stale handoff leaves the Body usable', async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.getByRole('heading', { name: 'A Body of your own' })).toBeVisible();
  const form = await reviewedForm(page, 'desk_telegraph');
  await page.goto(handoffUrl(form));
  await expect(page.getByRole('checkbox', { name: 'Desk Telegraph', exact: true })).toBeChecked();
  await page.getByRole('checkbox', { name: 'Startup Chime', exact: true }).uncheck();
  await page.getByRole('button', { name: 'Birth Body', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  await expect(page.locator('#surface-title')).toHaveText('Desk Telegraph');
  const initial = await current(page);
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.goto(handoffUrl({ ...form, checked_form_id: 'checked/stale' }));
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  await expect(page.locator('[data-workspace-notice]')).toContainText('checked identity has changed');
  expect((await current(page)).body_id).toBe(initial.body_id);
  expect((await current(page)).initial_forms).toEqual(initial.initial_forms);
});

for (const [title, kind] of [['Firefly Choir', 'pulse'], ['Night Radio', 'text'], ['Pocket Theremin', 'pointer'], ['Secret Knock', 'button']]) {
  test(`Gallery Form ${title} is resident and repeatedly usable in the Body`, async ({ page }, testInfo) => {
    await page.goto(entrance.url);
    await page.getByRole('checkbox', { name: 'Memory Lantern', exact: true }).uncheck();
    await page.getByRole('checkbox', { name: 'Startup Chime', exact: true }).uncheck();
    await page.getByRole('checkbox', { name: title, exact: true }).check();
    await page.getByRole('button', { name: 'Birth Body', exact: true }).click();
    await expect(page.locator('[data-play-state]')).toHaveText('Playing');
    const identity = await current(page);
    const surface = page.locator('#form-input');
    const output = page.locator('[data-form-output] output:visible');
    if (kind === 'pulse') {
      await expect.poll(async () => Number((await output.filter({ hasText: /^pulse / }).textContent())?.match(/^pulse (\d+) ·/)?.[1] ?? 0)).toBeGreaterThanOrEqual(5);
    } else if (kind === 'text') {
      await page.keyboard.type('first'); await page.keyboard.press('Enter');
      await expect(output).toContainText('first');
      await page.keyboard.type('second'); await page.keyboard.press('Enter');
      await expect(output).toContainText('second');
    } else if (kind === 'pointer') {
      await surface.click({ position: { x: 30, y: 80 } });
      await expect(output).toContainText('Hz');
      const first = await output.textContent();
      await surface.click({ position: { x: 150, y: 80 } });
      await expect(output).not.toHaveText(first);
    } else {
      await page.locator('[data-form-output] output').evaluate(element => {
        element.dataset.presentationCount = '0';
        new MutationObserver(() => element.dataset.presentationCount = String(Number(element.dataset.presentationCount) + 1))
          .observe(element, { childList: true, characterData: true, subtree: true });
      });
      await surface.hover();
      try {
        for (const transitions of [['down', 'up', 'down', 'up', 'down'], ['up', 'down', 'up', 'down', 'up', 'down']]) {
          for (const transition of transitions) await page.mouse[transition]();
          await expect(output).toContainText('matched:');
          await expect(output).toContainText('score_millionths:');
          await expect(output).toHaveAttribute('data-presentation-count', transitions.length === 5 ? '1' : '2');
        }
      } finally { await page.mouse.up(); }
    }
    expect((await current(page)).active_play_id).toBe(identity.active_play_id);
    if (kind === 'pointer' || kind === 'button') await page.screenshot({ path: testInfo.outputPath(`workspace-${kind}-form.png`), fullPage: true });
    await page.getByRole('button', { name: 'Lull Body', exact: true }).click();
    await expect(page.locator('[data-play-state]')).toHaveText('Lulled');
  });
}


test('pointer and text Forms share one Play and receive only their foreground input', async ({ page }) => {
  await birth(page);
  await openLibrary(page);
  await card(page, 'Pocket Theremin').getByRole('button', { name: 'Use', exact: true }).click();
  await expect(page.locator('[data-play-state]')).toHaveText('Playing');
  await expect(page.locator('#surface-title')).toHaveText('Pocket Theremin');
  const identity = await current(page);
  await page.locator('#form-input').click({ position: { x: 40, y: 80 } });
  await expect(page.locator('[data-form-output] output:visible')).toContainText('Hz');
  await page.keyboard.type('ignored');
  await page.locator('[data-checked-form-id]').filter({ hasText: 'Memory Lantern' }).click();
  await page.keyboard.type('hello');
  await expect(page.locator('[data-form-output] output:visible')).toHaveText('hello');
  await page.locator('[data-checked-form-id]').filter({ hasText: 'Pocket Theremin' }).click();
  await page.locator('#form-input').click({ position: { x: 160, y: 80 } });
  await expect(page.locator('[data-form-output] output:visible')).toContainText('Hz');
  await page.locator('[data-checked-form-id]').filter({ hasText: 'Memory Lantern' }).click();
  await page.keyboard.type(' again');
  await expect(page.locator('[data-form-output] output:visible')).toHaveText('hello again');
  expect((await current(page)).active_play_id).toBe(identity.active_play_id);
});
