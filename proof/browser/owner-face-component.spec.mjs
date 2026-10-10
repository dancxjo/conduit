import { expect, test } from '@playwright/test';

// Renderer fixture only: the callback is not a Body/action receipt.
const view = {
  face_id: 'fixture/face', face_revision: 1, show_id: 'fixture/show',
  show_state: 'available', interactions_admitted: true,
  subjects: [{ identity: 'fixture/task', name: 'Read the handbook', role: 'Info',
    disclosure: 'Primary', text: [], properties: [],
    values: [{ name: 'Completion', text: 'Open' }] }], relationships: [],
  actions: [{ identity: 'fixture/complete', target: 'fixture/task', name: 'Complete task',
    availability: 'available', disclosure: 'CurrentAction', arguments: [] }],
};

test('owner Face parts swap density while preserving controls and accessible actions', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  await page.evaluate(async view => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    document.body.replaceChildren();
    const style = document.createElement('style');
    style.textContent = `conduit-owner-face[data-look=compact]::part(document){padding:8px;gap:8px}
      conduit-owner-face[data-look=spacious]::part(document){padding:32px;gap:24px}`;
    const element = document.createElement('conduit-owner-face');
    element.dataset.look = 'compact';
    globalThis.calls = [];
    element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
    document.body.append(style, element);
    globalThis.originalControl = element.shadowRoot.querySelector('button');
    globalThis.originalMarkup = element.shadowRoot.innerHTML;
  }, view);
  const element = page.locator('conduit-owner-face');
  const button = element.getByRole('button', { name: 'Complete task', exact: true });
  await expect(button).toBeEnabled();
  await expect(element.locator('[part=value]')).toHaveText('Open');
  await expect.poll(() => element.locator('[part=document]').evaluate(node => getComputedStyle(node).padding)).toBe('8px');
  await element.evaluate(node => { node.dataset.look = 'spacious'; });
  await expect.poll(() => element.locator('[part=document]').evaluate(node => getComputedStyle(node).padding)).toBe('32px');
  expect(await element.evaluate(node => node.shadowRoot.innerHTML === globalThis.originalMarkup
    && node.shadowRoot.querySelector('button') === globalThis.originalControl)).toBe(true);
  await button.focus();
  await expect(button).toBeFocused();
  await page.keyboard.press('Enter');
  expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view, action: view.actions[0], arguments: [] }]);
  await expect(button).toBeDisabled();
  await element.evaluate((node, view) => node.show(view, {
    invoke: async call => { globalThis.calls.push(call); },
  }), view);
  await button.click();
  expect(await page.evaluate(() => globalThis.calls.length)).toBe(2);
  await element.evaluate((node, view) => node.show({ ...view, show_state: 'unavailable' }, {
    invoke: async call => { globalThis.calls.push(call); },
  }), view);
  await expect(button).toBeDisabled();
  await element.evaluate(node => {
    node.shadowRoot.querySelector('link').remove();
    document.querySelector('style').remove();
  });
  await expect(button).toHaveAccessibleName('Complete task');
  expect(await page.evaluate(() => globalThis.calls.length)).toBe(2);
});
