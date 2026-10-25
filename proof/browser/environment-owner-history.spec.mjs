import { readFile } from 'node:fs/promises';
import { expect, test } from '@playwright/test';

// Exact view exported by the checked browser Mask CPU test, not a live Body.
const directory = process.env.CONDUIT_HISTORY_BROWSER_PROOF_OUTPUT
  ?? new URL('../../target/environment-owner-history/', import.meta.url).pathname;
const fixture = JSON.parse(await readFile(`${directory}/history-view.json`, 'utf8'));
async function show(page, view = fixture) {
  await page.goto('/proof/browser/environment-owner-history.test.html');
  await page.evaluate(async view => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    const element = document.createElement('conduit-owner-face');
    element.show(view, { invoke: async () => { throw Error('history must not invoke an action'); } });
    document.body.replaceChildren(element);
  }, view);
  return page.locator('conduit-owner-face');
}
test('checked history Mask view renders exact °C values and gaps at desktop and mobile widths', async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  for (const width of [1000, 390]) {
    await page.setViewportSize({ width, height: 1000 });
    const face = await show(page);
    const history = face.locator('[part=history]');
    await expect(history).toHaveAttribute('data-face-id', fixture.face_id);
    await expect(history).toHaveAttribute('data-face-revision', String(fixture.face_revision));
    await expect(history.getByRole('img')).toBeVisible();
    await expect(history.locator('circle')).toHaveCount(4);
    await expect(history).toContainText('Temperature °C');
    await history.getByText('Exact recorded values and gaps', { exact: true }).click();
    await expect(history.getByRole('columnheader', { name: 'Temperature °C (rounded to 3 places)', exact: true })).toBeVisible();
    const rows = history.getByRole('row');
    await expect(rows).toHaveCount(4);
    await expect(rows.nth(2)).toContainText('Unavailable');
    await expect(rows.nth(2)).toContainText('checksum');
    await expect(rows.nth(3)).toContainText('missing sequences 3 through 3');
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    if (width === 390) {
      expect(await history.getByRole('region', { name: 'Recorded samples table' })
        .evaluate(node => node.scrollWidth > node.clientWidth)).toBe(true);
    }
    await page.screenshot({ path: `${directory}/owner-history-${width}.png`, fullPage: true });
  }
  expect(errors).toEqual([]);
});
test('stale graph material and another recipe never become graph pixels', async ({ page }) => {
  const stale = structuredClone(fixture);
  stale.component.history.face_revision = `${BigInt(fixture.face_revision) + 1n}`;
  let face = await show(page, stale);
  await expect(face.locator('[part=history]')).toHaveCount(0);
  await expect(face).toContainText('History graph unavailable');
  const foreign = structuredClone(fixture);
  foreign.component.recipe_identity = 'conduit.face/another-recipe@1';
  face = await show(page, foreign);
  await expect(face.locator('[part=history]')).toHaveCount(0);
  await expect(face.getByRole('img')).toHaveCount(0);
  const wrongUnit = structuredClone(fixture);
  wrongUnit.component.history.temperature_unit = '°F';
  face = await show(page, wrongUnit);
  await expect(face.locator('[part=history]')).toHaveCount(0);
  await expect(face).toContainText('History graph unavailable');
  const fallback = JSON.parse(await readFile(`${directory}/history-fallback-view.json`, 'utf8'));
  face = await show(page, fallback);
  await expect(face.locator('[part=history]')).toHaveCount(0);
  await expect(face).toContainText('History graph unavailable');
});

test('history remains readable with large text, forced colors and keyboard table scrolling', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 1000 });
  await page.emulateMedia({ forcedColors: 'active', reducedMotion: 'reduce' });
  const face = await show(page);
  await page.evaluate(() => { document.documentElement.style.fontSize = '200%'; });
  const history = face.locator('[part=history]');
  await expect(history.getByRole('img')).toBeVisible();
  await expect(history.locator('circle')).toHaveCount(4);
  const summary = history.getByText('Exact recorded values and gaps', { exact: true });
  await summary.focus();
  await page.keyboard.press('Enter');
  const scroll = history.getByRole('region', { name: 'Recorded samples table' });
  await scroll.focus();
  await expect(scroll).toBeFocused();
  await page.keyboard.press('ArrowRight');
  await expect.poll(() => scroll.evaluate(node => node.scrollLeft)).toBeGreaterThan(0);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await expect(history).toHaveAttribute('data-face-id', fixture.face_id);
  await page.screenshot({ path: `${directory}/owner-history-accessible.png`, fullPage: true });
});

test('full recording chunk window reports omitted history and renders all selected points', async ({ page }) => {
  const window = JSON.parse(await readFile(`${directory}/history-window-view.json`, 'utf8'));
  const face = await show(page, window);
  const history = face.locator('[part=history]');
  await expect(history).toHaveAttribute('data-face-id', window.face_id);
  await expect(history.locator('circle')).toHaveCount(16);
  await expect(history).toContainText('56');
  await history.getByText('Exact recorded values and gaps', { exact: true }).click();
  const rows = history.getByRole('row');
  await expect(rows).toHaveCount(9);
  await expect(rows.nth(1)).toContainText('57');
  await expect(rows.nth(8)).toContainText('64');
});

test('checked unit Looks render converted values over the same Face and graph coordinates', async ({ page }) => {
  let dots;
  const plans = new Set();
  for (const [suffix, unit, value] of [['C', '°C', '19.800'], ['K', 'K', '292.950'], ['F', '°F', '67.640']]) {
    const view = JSON.parse(await readFile(`${directory}/history-unit-${suffix}.json`, 'utf8'));
    expect(view.face_id).toBe(fixture.face_id);
    expect(view.face_revision).toBe(fixture.face_revision);
    plans.add(view.mask_plan_id);
    const face = await show(page, view);
    const history = face.locator('[part=history]');
    await expect(history.getByRole('img')).toBeVisible();
    await expect(history).toContainText(`Temperature ${unit}`);
    const actual = await history.locator('circle').evaluateAll(nodes => nodes.map(node => [node.getAttribute('cx'), node.getAttribute('cy')]));
    if (dots) expect(actual).toEqual(dots); else dots = actual;
    await history.getByText('Exact recorded values and gaps', { exact: true }).click();
    await expect(history.getByRole('columnheader', { name: `Temperature ${unit} (rounded to 3 places)`, exact: true })).toBeVisible();
    await expect(history.getByRole('row').nth(1)).toContainText(value);
    await expect(history.getByRole('row').nth(1)).toContainText('19.8°C');
    await page.screenshot({ path: `${directory}/owner-history-unit-${suffix}.png`, fullPage: true });
  }
  expect(plans.size).toBe(3);
});

test('actual owner ABI view preserves complete Body context and actions beside the history graph', async ({ page }) => {
  const view = JSON.parse(await readFile(`${directory}/history-body-view.json`, 'utf8'));
  const face = await show(page, view);
  await expect(face.locator('[part=history] circle')).toHaveCount(4);
  await expect(face).toHaveAttribute('data-component-look', view.component.expression.expression.identity);
  const forms = face.locator('form[data-owner-action]');
  await expect(forms).toHaveCount(view.actions.length);
  expect(view.subjects.some(subject => subject.role === 'Body')).toBe(true);
  // Open the actual finite native disclosures rather than bypassing their
  // focus/visibility behavior to reach the retained Body controls.
  for (const summary of await face.locator('summary').all()) {
    if (!await summary.evaluate(node => node.parentElement.open)) await summary.click();
  }
  const available = view.actions.find(action => action.availability === 'available');
  expect(available).toBeTruthy();
  const button = forms.filter({ has: page.locator(`button[data-owner-focus]`) }).getByRole('button').first();
  await button.focus();
  await expect(button).toBeFocused();
  await page.screenshot({ path: `${directory}/owner-history-body.png`, fullPage: true });
});

test('history selection uses the existing finite owner request and preserves unsupported unit choices', async ({ page }) => {
  await page.goto('/proof/browser/environment-owner-history.test.html');
  await page.evaluate(async view => {
    const { createOwnerLookControls } = await import('/targets/browser/handbook/owner-look-controls.mjs');
    globalThis.historySelections = [];
    globalThis.historyControls = createOwnerLookControls(selection => globalThis.historySelections.push(selection));
    historyControls.observe(view); historyControls.setDisabled(false);
    document.body.replaceChildren(historyControls.element);
  }, fixture);
  await page.getByLabel('Layout', { exact: true }).selectOption('environment-history');
  await page.getByRole('button', { name: 'Apply face', exact: true }).click();
  expect(await page.evaluate(() => historySelections)).toEqual([
    { look: 'compact-editorial', recipe: 'environment-history', minimumTextScalePercent: 100 },
  ]);
  await page.getByLabel('Palette', { exact: true }).selectOption('phosphor');
  await page.getByRole('button', { name: 'Apply face', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('requires');
  expect(await page.evaluate(() => historySelections.length)).toBe(1);
  const kelvin = JSON.parse(await readFile(`${directory}/history-unit-K.json`, 'utf8'));
  await page.evaluate(view => historyControls.observe(view), kelvin);
  await page.getByRole('button', { name: 'Apply face', exact: true }).click();
  expect(await page.evaluate(() => historySelections.length)).toBe(1);
});
