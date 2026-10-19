import { writeFile } from "node:fs/promises";
import { expect, test } from '@playwright/test';
import { startStaticProduct } from './static-product-server.mjs';
test.use({ trace: 'on' });
let entrance;
const ownerReceipt = page => page.evaluate(() => {
  const runtime = globalThis.__conduitWorkspace.host.runtime;
  return JSON.parse(new TextDecoder().decode(new Uint8Array(runtime.memory.buffer,
    runtime.conduit_creche_local_face_output_ptr(), runtime.conduit_creche_local_face_output_len())));
});
test.beforeEach(async () => { entrance = await startStaticProduct('target/workspace-product', '/conduit/workspace/'); });
test.afterEach(() => entrance?.child.kill());
const slot = page => page.locator('[data-application-slot="birth-fields"]');
const revision = page => slot(page).getAttribute('data-portable-face-revision');
const open = async page => {
  await page.goto(entrance.url);
  await expect(page.getByRole('button', { name: 'Edit Body name', exact: true })).toBeEnabled();
};

test('one source Face selects registered and generic Backs with distinct Show Plans', async ({ page }, testInfo) => {
  await open(page);
  await expect(page.locator('conduit-portable-face')).toHaveCount(1);
  const identity = await slot(page).getAttribute('data-portable-face-id');
  const firstPlan = await slot(page).getAttribute('data-portable-mask-plan');
  const name = await page.getByLabel('Friendly Body name', { exact: true }).inputValue();
  const enhanced = await ownerReceipt(page);
  await page.getByLabel('Controls on this Host').selectOption('basic');
  await expect(page.locator('[data-portable-face-back="generic"]')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Edit Body name', exact: true })).toBeEnabled();
  expect(await slot(page).getAttribute('data-portable-face-id')).toBe(identity);
  expect(await slot(page).getAttribute('data-portable-mask-plan')).not.toBe(firstPlan);
  const basic = await ownerReceipt(page);
  expect(basic.encounter.face).toEqual(enhanced.encounter.face);
  expect(basic.encounter.mask_show.show_id).not.toBe(enhanced.encounter.mask_show.show_id);
  await writeFile(testInfo.outputPath('portable-mask-correspondence.json'), JSON.stringify({
    schema: 'conduit.browser/portable-mask-correspondence@1', enhanced, basic,
  }, null, 2));
  await expect(page.getByLabel('Friendly Body name', { exact: true })).toHaveValue(name);
  await expect(page.getByLabel('Naming tradition', { exact: true })).toHaveRole('combobox');
  await page.getByLabel('Controls on this Host').selectOption('enhanced');
  await expect(page.locator('conduit-portable-face')).toHaveCount(1);
  await expect(page.getByRole('button', { name: 'Edit Body name', exact: true })).toBeEnabled();
  expect(await slot(page).getAttribute('data-portable-face-id')).toBe(identity);
});

test('native Unicode draft, caret and cancel remain provisional until explicit submit', async ({ page }) => {
  await open(page);
  const name = page.getByLabel('Friendly Body name', { exact: true });
  const original = await name.inputValue(), before = await revision(page);
  await name.fill('Éléonore 雪');
  await name.press('Home'); await name.press('ArrowRight'); await name.press('Shift+ArrowRight');
  expect(await name.evaluate(input => ({ start: input.selectionStart, end: input.selectionEnd }))).toEqual({ start: 1, end: 2 });
  expect(await revision(page)).toBe(before);
  await name.locator('..').getByRole('button', { name: 'Cancel draft', exact: true }).click();
  await expect(name).toHaveValue(original); expect(await revision(page)).toBe(before);
  await name.fill('Éléonore 雪');
  await page.getByRole('button', { name: 'Edit Body name', exact: true }).click();
  await expect(name).toHaveValue('Éléonore 雪');
  await expect(slot(page)).not.toHaveAttribute('data-portable-face-revision', before);
  await expect(page.locator('[data-source-status$="/birth-review"]')).toContainText('Name: Éléonore 雪.');
});

test('editable fixed popup highlights and chooses a draft before source commit', async ({ page }) => {
  await open(page);
  const input = page.getByLabel('Naming tradition', { exact: true });
  const before = await revision(page);
  await input.fill(''); await input.press('ArrowDown');
  await expect(input).toBeFocused(); await expect(input).toHaveAttribute('aria-expanded', 'true');
  const active = await input.getAttribute('aria-activedescendant');
  await expect(page.locator(`[id="${active}"]`)).toHaveAttribute('aria-selected', 'true');
  expect(await revision(page)).toBe(before);
  await input.press('Escape'); await expect(input).toHaveAttribute('aria-expanded', 'false');
  await input.fill(''); await page.locator('[role="option"][data-source-value="chinese"]').click();
  expect(await revision(page)).toBe(before);
  await page.getByRole('button', { name: 'Use naming tradition', exact: true }).click();
  await expect(input).toHaveValue('Chinese (romanized, family name first)');
  await expect(slot(page)).not.toHaveAttribute('data-portable-face-revision', before);
});

test('forged Shadow events and programmatic submit cannot mutate the owner', async ({ page }) => {
  await open(page); const before = await revision(page);
  const name = page.getByLabel('Friendly Body name', { exact: true }); const original = await name.inputValue();
  await name.evaluate(input => {
    input.value = 'Forged value'; input.dispatchEvent(new InputEvent('input', { bubbles: true, composed: true }));
    input.form.requestSubmit(); input.form.querySelector('button[type=submit]').click();
  });
  expect(await revision(page)).toBe(before);
  await name.locator('..').getByRole('button', { name: 'Cancel draft', exact: true }).click();
  await expect(name).toHaveValue(original);
});

for (const failure of ['absent module', 'broken stylesheet', 'foreign element']) {
  test(`${failure} selects an actual generic Mask before Show`, async ({ page }) => {
    if (failure === 'foreign element') await page.addInitScript(() => { customElements.define('conduit-portable-face', class extends HTMLElement {}); });
    else await page.route(failure === 'absent module' ? '**/portable-face-component.mjs' : '**/portable-face-look.css', route =>
      failure === 'absent module' ? route.fulfill({ status: 404, body: '' }) : route.fulfill({ status: 200, body: 'button{display:none}' }));
    await open(page);
    await expect(page.locator('[data-portable-face-back="generic"]')).toBeVisible();
    await expect(page.locator('[data-component-refusal]')).not.toBeEmpty();
    const name = page.getByLabel('Friendly Body name', { exact: true });
    await name.fill('Basic 雪'); await page.getByRole('button', { name: 'Edit Body name', exact: true }).click();
    await expect(page.locator('[data-source-status$="/birth-review"]')).toContainText('Name: Basic 雪.');
  });
}

test('required controls reflow under narrow, 200 percent CSS zoom and forced colors', async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 800 }); await page.emulateMedia({ forcedColors: 'active' }); await open(page);
  await page.locator('body').evaluate(element => { element.style.zoom = '2'; });
  await expect(page.getByLabel('Friendly Body name', { exact: true })).toBeVisible();
  const geometry = await page.locator('conduit-portable-face').evaluate(element => {
    const root = element.shadowRoot.querySelector('.portable-face'); return { width: root.scrollWidth, available: root.clientWidth };
  });
  expect(geometry.width).toBeLessThanOrEqual(geometry.available + 1);
  await page.getByLabel('Friendly Body name', { exact: true }).fill('Long Unicode 雪'.repeat(3));
  await page.getByRole('button', { name: 'Edit Body name', exact: true }).click();
  await expect(page.getByLabel('Friendly Body name', { exact: true })).toBeEnabled();
});

test('Chromium IME preedit is provisional and composition blocks submit', async ({ page, context }) => {
  await open(page); const before = await revision(page);
  const input = page.getByLabel('Friendly Body name', { exact: true });
  await input.fill(''); await input.focus();
  const protocol = await context.newCDPSession(page);
  await protocol.send('Input.imeSetComposition', { text: '雪', selectionStart: 1, selectionEnd: 1 });
  expect(await revision(page)).toBe(before);
  await input.press('Enter');
  expect(await revision(page)).toBe(before);
  await protocol.send('Input.insertText', { text: '雪' });
  await input.fill('雪');
  await page.getByRole('button', { name: 'Edit Body name', exact: true }).click();
  await expect(page.locator('[data-source-status$="/birth-review"]')).toContainText('Name: 雪.');
  await protocol.detach();
});

for (const violation of ['hidden action', 'hidden input', 'Shadow node capacity']) {
  test(`${violation} closes the Show before another source action`, async ({ page }) => {
    await open(page); const before = await revision(page);
    if (violation === 'hidden action') {
      await page.getByRole('button', { name: 'Edit Body name', exact: true }).evaluate(button => { button.style.display = 'none'; });
    } else if (violation === 'hidden input') {
      await page.getByLabel('Friendly Body name', { exact: true }).evaluate(input => { input.style.display = 'none'; });
    } else {
      await page.locator('conduit-portable-face').evaluate(element => {
        const extra = document.createDocumentFragment();
        for (let index = 0; index < 2049; index += 1) extra.append(document.createElement('span'));
        element.shadowRoot.append(extra);
      });
    }
    await page.getByRole('button', { name: 'Suggest another name', exact: true }).click();
    expect(await revision(page)).toBe(before);
    await expect(page.getByRole('button', { name: 'Suggest another name', exact: true })).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Refresh controls', exact: true })).toBeVisible();
    await page.getByRole('button', { name: 'Refresh controls', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Suggest another name', exact: true })).toBeEnabled();
    expect(await revision(page)).toBe(before);
  });
}
