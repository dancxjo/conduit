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

// This exercises renderer behavior only; fixture configuration is not checked
// source custody, an owner acknowledgement, or an action execution receipt.
test('configured groups preserve interleaved controls and user text scale', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  const configured = {
    ...view,
    subjects: [0, 1, 2].map(index => ({ ...view.subjects[0],
      identity: `fixture/task/${index}`, name: `Task ${index}` })),
    actions: [0, 1, 2].map(index => ({ ...view.actions[0],
      identity: `fixture/complete/${index}`, target: `fixture/task/${index}`, name: `Complete ${index}` })),
    component: { maximum_columns: 2, reflow_width: 720, gutter: 12,
      subject_regions: [0, 1, 2], groups: [0, 1, 0], fallback: null,
      expression: { expression: { identity: 'fixture/expression', density: 'compact',
        palette: 'phosphor', text_scale_percent: 100 } } },
  };
  await page.evaluate(async view => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    document.body.replaceChildren();
    const element = document.createElement('conduit-owner-face');
    element.style.cssText = 'display:block;width:1000px';
    document.body.append(element);
    element.show(view, { invoke: async () => {} });
  }, configured);
  const element = page.locator('conduit-owner-face');
  const controls = element.getByRole('button');
  await expect(controls).toHaveText(['Complete 0', 'Complete 1', 'Complete 2']);
  await expect.poll(() => element.getAttribute('data-component-columns')).toBe('2');
  const runs = await element.evaluate(node => [...node.shadowRoot.querySelectorAll('.owner-face-document > [data-component-group]')]
    .map(run => run.dataset.componentGroup));
  expect(runs).toEqual(['0', '1', '0']);
  const originalFont = await controls.first().evaluate(node => Number.parseFloat(getComputedStyle(node).fontSize));
  await element.evaluate((node, view) => {
    view.component.expression.expression.text_scale_percent = 200;
    node.show(view, { invoke: async () => {} });
  }, configured);
  await expect(controls).toHaveText(['Complete 0', 'Complete 1', 'Complete 2']);
  await expect.poll(() => controls.first().evaluate(node => Number.parseFloat(getComputedStyle(node).fontSize)))
    .toBe(originalFont * 2);
  await controls.first().focus();
  await expect(controls.first()).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(controls.nth(1)).toBeFocused();
  await page.emulateMedia({ forcedColors: 'active', reducedMotion: 'reduce' });
  const platform = await page.evaluate(() => {
    const probe = document.createElement('div');
    probe.style.cssText = 'color:CanvasText;background:Canvas;outline-color:Highlight';
    document.body.append(probe);
    const computed = getComputedStyle(probe);
    const result = { ink: computed.color, paper: computed.backgroundColor, focus: computed.outlineColor };
    probe.remove(); return result;
  });
  await expect.poll(() => controls.first().evaluate(node => getComputedStyle(node).color)).toBe(platform.ink);
  await expect.poll(() => controls.first().evaluate(node => getComputedStyle(node).backgroundColor)).toBe(platform.paper);
  await controls.first().focus();
  await expect.poll(() => controls.first().evaluate(node => getComputedStyle(node).outlineColor)).toBe(platform.focus);
  await controls.first().evaluate(node => { node.style.animation = 'fixture-motion 1s infinite'; });
  await expect.poll(() => controls.first().evaluate(node => getComputedStyle(node).animationName)).toBe('none');
  await expect(controls).toHaveText(['Complete 0', 'Complete 1', 'Complete 2']);
  await element.evaluate(node => { node.style.width = '400px'; });
  await expect.poll(() => element.getAttribute('data-component-columns')).toBe('1');
  await expect(controls).toHaveText(['Complete 0', 'Complete 1', 'Complete 2']);
  await element.evaluate((node, view) => {
    view.component.fallback = 'recipe-reading-capacity';
    view.component.subject_regions = [null, null, null];
    view.component.groups = [];
    view.actions[1].availability = 'refused';
    node.style.width = '1000px';
    node.show(view, { invoke: async () => {} });
  }, configured);
  await expect(element).toHaveAttribute('data-component-fallback', 'recipe-reading-capacity');
  await expect.poll(() => element.getAttribute('data-component-columns')).toBe('1');
  await expect(controls).toHaveText(['Complete 0', 'Complete 1', 'Complete 2']);
  await expect(controls.first()).toBeEnabled();
  await expect(controls.nth(1)).toBeDisabled();
  await expect(controls.nth(2)).toBeEnabled();
  await expect(element.locator('.owner-component-region-run')).toHaveCount(0);
  await controls.first().focus();
  await page.keyboard.press('Tab');
  await expect(controls.nth(2)).toBeFocused();
});
