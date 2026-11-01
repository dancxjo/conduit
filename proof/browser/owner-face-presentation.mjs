// Presentation conformance shares the ordinary component's fixture and input
// path. These callbacks are renderer evidence, never Body/action receipts.
import { readFile } from 'node:fs/promises';
import { expect, test } from '@playwright/test';

export function registerOwnerFacePresentationTests(view) {
  async function present(page, current) {
    const hex = await readFile('proof/conformance/face-look/phosphor-look-v2.hex', 'utf8');
    current.component = { subject_regions: current.subjects.map(() => null), groups: [],
      selected_look: { canonical_value: Array.from(Buffer.from(hex.replace(/\s/gu, ''), 'hex')) } };
    await page.goto('/proof/browser/owner-face-look-fixture.html');
    await page.evaluate(async current => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      const element = document.createElement('conduit-owner-face'); document.body.replaceChildren(element);
      window.presentationView = current; window.presentationOriginal = JSON.stringify(current);
      window.presentationCalls = [];
      element.show(current, { invoke: async ({ action }) => presentationCalls.push(action.identity) });
    }, current);
    const element = page.locator('conduit-owner-face');
    await element.evaluate(async element => {
      const link = element.shadowRoot.querySelector('link');
      if (!link.sheet) await new Promise((resolve, reject) => {
        link.addEventListener('load', resolve, { once: true });
        link.addEventListener('error', reject, { once: true });
      });
      await document.fonts.ready;
    });
    return element;
  }

  function namedBody({ disclosure = 'Primary', name = 'Malo Lefèvre Laurent', extra = [] } = {}) {
    const current = structuredClone(view);
    current.subjects.unshift({ identity: 'fixture/body-subject', name, role: 'Body',
      disclosure, text: ['Body description'], properties: [], values: [] });
    current.subjects.push(...extra);
    return current;
  }

  test('presented Body wording heads application content without granting actions', async ({ page }) => {
    const name = '  Malo Lefèvre Laurent <img src=x> 🪴  ';
    const current = namedBody({ name }); current.interactions_admitted = false;
    const element = await present(page, current);
    const heading = element.locator('[data-face-heading]');
    expect(await heading.textContent()).toBe(name);
    await expect(heading).toHaveAttribute('data-face-heading', 'fixture/body-subject');
    await expect(heading).toBeVisible();
    await expect(heading.locator('img')).toHaveCount(0);
    await expect(element.getByRole('button', { name: 'Complete task', exact: true })).toBeDisabled();
    expect(await page.evaluate(() => JSON.stringify(presentationView))).toBe(await page.evaluate(() => presentationOriginal));
    expect(await page.evaluate(() => presentationCalls)).toEqual([]);
    const active = await present(page, namedBody());
    await active.getByRole('button', { name: 'Complete task', exact: true }).click();
    expect(await page.evaluate(() => presentationCalls)).toEqual(['fixture/complete']);
  });

  test('Body heading respects private and ambiguous subjects and avoids a Body-only duplicate', async ({ page }) => {
    for (const disclosure of ['CurrentAction', 'SelectedDetail', 'ExactProvenance']) {
      const element = await present(page, namedBody({ disclosure }));
      await expect(element.locator('[data-face-heading]')).toHaveCount(0);
      const exact = element.getByRole('heading', { name: 'Malo Lefèvre Laurent', exact: true });
      await expect(exact).toBeHidden();
      await element.locator('details[data-body-menu]>summary').click();
      await expect(exact).toBeVisible();
    }
    const ambiguous = await present(page, namedBody({ extra: [{ identity: 'fixture/second-body',
      name: 'Another Body', role: 'Body', disclosure: 'ExactProvenance', text: [], properties: [] }] }));
    await expect(ambiguous.locator('[data-face-heading]')).toHaveCount(0);
    const only = namedBody(); only.subjects = only.subjects.slice(0, 1); only.actions = [];
    const single = await present(page, only);
    await expect(single.locator('[data-face-heading]')).toHaveCount(0);
    await expect(single.getByRole('heading', { name: 'Malo Lefèvre Laurent', exact: true })).toHaveCount(1);
  });

  test('context Body wording wraps on a narrow screen without horizontal overflow', async ({ page }) => {
    await page.setViewportSize({ width: 320, height: 600 });
    const name = 'Notre maison — laboratoire des plantes et des voix partagées';
    const element = await present(page, namedBody({ name, disclosure: 'Context' }));
    const heading = element.locator('[data-face-heading]');
    await expect(heading).toBeVisible();
    expect(await heading.textContent()).toBe(name);
    expect(await heading.evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  });

  test('view-only permission remains distinct from unavailable source actions', async ({ page }) => {
    for (const [show_state, interactions_admitted] of [['prepared', true], ['available', false], ['available', true]]) {
      const current = structuredClone(view); Object.assign(current, { show_state, interactions_admitted });
      current.actions.push({ ...current.actions[0], identity: 'fixture/unavailable',
        name: 'Unavailable action', availability: 'unavailable', explanation: 'Checkpoint storage is unavailable.' });
      const element = await present(page, current);
      const available = element.locator('[data-owner-action="fixture/complete"]');
      const enabled = show_state === 'available' && interactions_admitted;
      await expect(available.getByText('View only. Actions are not enabled in this view.', { exact: true })).toHaveCount(enabled ? 0 : 1);
      await expect(available.getByRole('button')).toBeEnabled({ enabled });
      const unavailable = element.locator('[data-owner-action="fixture/unavailable"]');
      await expect(unavailable.getByText('Checkpoint storage is unavailable.', { exact: true })).toHaveCount(1);
      await expect(unavailable.getByRole('button')).toBeDisabled();
      expect(await page.evaluate(() => JSON.stringify(presentationView))).toBe(await page.evaluate(() => presentationOriginal));
    }
  });

  test('selected Look keeps supplied names and unfamiliar semantic labels', async ({ page }) => {
    const current = structuredClone(view); const roles = ['Info', 'Collection', 'Status', 'Item'];
    current.subjects = roles.map(role => ({ identity: `fixture/${role}`, role,
      name: `Supplied ${role} name`, disclosure: 'Primary', text: [], properties: [] }));
    current.subjects.push({ ...current.subjects[0], identity: 'fixture/unknown',
      semantic_role: 'example/unfamiliar@1', name: 'Unfamiliar reading' });
    current.actions[0].target = 'fixture/Item';
    const element = await present(page, current);
    for (const role of roles) {
      await expect(element.getByRole('heading', { name: `Supplied ${role} name`, exact: true })).toBeVisible();
      await expect(element.locator(`[data-face-role=${role}]>.owner-face-role`).first()).toBeHidden();
    }
    await expect(element.getByText('example/unfamiliar@1', { exact: true })).toBeVisible();
    await element.getByRole('button', { name: 'Complete task', exact: true }).click();
    expect(await page.evaluate(() => presentationCalls)).toEqual(['fixture/complete']);
    expect(await page.evaluate(() => JSON.stringify(presentationView))).toBe(await page.evaluate(() => presentationOriginal));
  });
}
