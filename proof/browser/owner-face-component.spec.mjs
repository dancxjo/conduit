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

test('application controls remain primary while Body management is disclosed separately', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  const scoped = {
    ...view,
    subjects: [
      { ...view.subjects[0], identity: 'fixture/body', name: 'Living room', role: 'Body' },
      { ...view.subjects[0], identity: 'fixture/plot', name: 'Resident application', role: 'Plot', disclosure: 'Context' },
      { ...view.subjects[0], name: 'Target temperature', values: [{ name: 'Target', text: '21 °C' }] },
    ],
    actions: [
      { ...view.actions[0], identity: 'fixture/lull', target: 'fixture/body', name: 'Lull' },
      { ...view.actions[0], identity: 'fixture/library', target: 'fixture/plot', name: 'Open Plots library' },
      { ...view.actions[0], name: 'Raise target' },
    ],
  };
  await page.evaluate(async view => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    document.body.replaceChildren();
    const element = document.createElement('conduit-owner-face');
    globalThis.calls = [];
    element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
    document.body.append(element);
  }, scoped);
  const element = page.locator('conduit-owner-face');
  const menu = element.locator('[data-body-menu]');
  await expect(element.getByRole('button', { name: 'Raise target', exact: true })).toBeVisible();
  await expect(element.getByRole('button', { name: 'Lull', exact: true })).toBeHidden();
  await expect(element.getByRole('button', { name: 'Open Plots library', exact: true })).toBeHidden();
  await menu.locator(':scope > summary').focus();
  await page.keyboard.press('Enter');
  await expect(element.getByRole('button', { name: 'Lull', exact: true })).toBeVisible();
  await expect(element.getByRole('button', { name: 'Open Plots library', exact: true })).toBeVisible();
  // A refreshed Show keeps the user's disclosure choice, with no extra actions.
  await element.evaluate((node, view) => node.show(view, { invoke: async call => globalThis.calls.push(call) }), scoped);
  await expect(menu).toHaveAttribute('open', '');
  await element.getByRole('button', { name: 'Lull', exact: true }).click();
  expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view: scoped, action: scoped.actions[0], arguments: [] }]);
});

test('a Body without primary application content keeps its arrival controls visible', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  const idle = { ...view,
    subjects: [{ ...view.subjects[0], identity: 'fixture/body', name: 'Living room', role: 'Body' }],
    actions: [{ ...view.actions[0], identity: 'fixture/wake', target: 'fixture/body', name: 'Wake' }],
  };
  await page.evaluate(async view => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    document.body.replaceChildren();
    const element = document.createElement('conduit-owner-face');
    element.show(view, { invoke: async () => {} });
    document.body.append(element);
  }, idle);
  const element = page.locator('conduit-owner-face');
  await expect(element.getByRole('button', { name: 'Wake', exact: true })).toBeVisible();
  await expect(element.locator('[data-body-menu]')).toHaveCount(0);
});

for (const reuseView of [false, true]) {
  test(`replaced owner controls cannot invoke the current route (reuse view: ${reuseView})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    await page.evaluate(async ({ view, reuseView }) => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      document.body.replaceChildren();
      const element = document.createElement('conduit-owner-face');
      document.body.append(element);
      globalThis.calls = [];
      const invoke = async call => { globalThis.calls.push(call); };
      element.show(view, { invoke });
      const obsolete = element.shadowRoot.querySelector('form');
      const next = reuseView ? view : { ...view, face_revision: 2, show_id: 'fixture/next-show' };
      element.show(next, { invoke });
      if (obsolete.isConnected) throw new Error('fixture did not replace the old control');
      // Adversarial stale event, deliberately separate from trusted user proof.
      obsolete.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
      globalThis.currentView = next;
    }, { view, reuseView });
    expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
    const current = page.locator('conduit-owner-face').getByRole('button', { name: 'Complete task', exact: true });
    await expect(current).toBeEnabled();
    await current.focus();
    await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls)).toEqual([{
      view: reuseView ? view : { ...view, face_revision: 2, show_id: 'fixture/next-show' },
      action: view.actions[0], arguments: [],
    }]);
    await expect(current).toBeDisabled();
  });
}

test('a replaced exclusive-choice listener cannot dispatch or reset current selection', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  const choices = {
    ...view,
    subjects: [
      { ...view.subjects[0], identity: 'fixture/group', name: 'Mode',
        semantic_role: 'conduit.face/choice-group@1', choice_multiplicity: 'exclusive' },
      ...[true, false].map((selected, index) => ({ ...view.subjects[0],
        identity: `fixture/option/${index}`, name: `Mode ${index}`,
        flags: [{ name: 'selected', value: selected }] })),
    ],
    relationships: [0, 1].map(index => ({ kind: 'Contains', source: 'fixture/group', target: `fixture/option/${index}` })),
    actions: [0, 1].map(index => ({ ...view.actions[0], identity: `fixture/select/${index}`,
      target: `fixture/option/${index}`, name: `Select ${index}` })),
  };
  await page.evaluate(async view => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    document.body.replaceChildren();
    const element = document.createElement('conduit-owner-face');
    document.body.append(element);
    globalThis.calls = [];
    const invoke = async call => { globalThis.calls.push(call); };
    element.show(view, { invoke });
    const obsolete = element.shadowRoot.querySelector('input[type=radio]');
    if (!obsolete) throw new Error('fixture did not render an exclusive choice');
    element.show(view, { invoke });
    obsolete.dispatchEvent(new Event('change', { bubbles: true }));
  }, choices);
  expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
  const element = page.locator('conduit-owner-face');
  await expect(element.getByRole('radio', { name: 'Mode 0', exact: true })).toBeChecked();
  const current = element.getByRole('radio', { name: 'Mode 1', exact: true });
  await expect(current).toBeEnabled();
  await current.focus();
  await page.keyboard.press('Space');
  expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view: choices, action: choices.actions[1], arguments: [] }]);
  await expect(current).toBeDisabled();
});

for (const scenario of ['unsupported', 'utf8', 'choice']) {
  test(`current forms refuse invalid ${scenario} arguments before invoking`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const argument = { name: 'value', value_name: 'Value', value_kind: 'value/text',
      maximum_bytes: 4, choices: [] };
    if (scenario === 'unsupported') argument.value_kind = 'value/unknown';
    if (scenario === 'choice') argument.choices = ['one', 'two'];
    const currentView = { ...view, actions: [{ ...view.actions[0], arguments: [argument] }] };
    await page.evaluate(async ({ view, scenario }) => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      document.body.replaceChildren();
      const element = document.createElement('conduit-owner-face');
      document.body.append(element);
      globalThis.calls = [];
      element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
      const form = element.shadowRoot.querySelector('form');
      if (scenario === 'utf8') form.querySelector('input').value = 'ééé';
      if (scenario === 'choice') {
        const select = form.querySelector('select');
        select.add(new Option('forged', 'forged'));
        select.value = 'forged';
      }
      // Adversarial event intentionally bypasses normal browser form admission.
      form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    }, { view: currentView, scenario });
    expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
    const element = page.locator('conduit-owner-face');
    const button = element.getByRole('button', { name: 'Complete task', exact: true });
    if (scenario === 'unsupported') {
      await expect(button).toBeDisabled();
      await expect(element.getByText('This input has no supported browser form.', { exact: true })).toBeVisible();
      return;
    }
    await expect(button).toBeEnabled();
    if (scenario === 'utf8') {
      const input = element.getByLabel('Value', { exact: true });
      expect(await input.evaluate(node => node.validationMessage)).toBe('Use at most 4 UTF-8 bytes.');
      await input.fill('éé');
    } else await element.getByRole('combobox').selectOption('one');
    await button.focus();
    await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view: currentView,
      action: currentView.actions[0], arguments: [{ name: 'value', value: scenario === 'utf8' ? 'éé' : 'one' }] }]);
  });
}

test('argument names come from the current Face caption rather than choice contents', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  const withArgument = name => ({ ...view, actions: [{ ...view.actions[0], arguments: [{
    name: 'priority', value_name: name, value_kind: 'value/text', maximum_bytes: 16,
    choices: ['Normal', 'Urgent'],
  }] }] });
  const initial = withArgument('Priority');
  await page.evaluate(async view => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    document.body.replaceChildren();
    const element = document.createElement('conduit-owner-face');
    document.body.append(element);
    globalThis.calls = [];
    element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
  }, initial);
  const element = page.locator('conduit-owner-face');
  const select = element.getByRole('combobox');
  await expect(select).toHaveAccessibleName('Priority');
  await expect(select).toHaveAttribute('required', '');
  await select.selectOption('Urgent');
  await expect(select).toHaveAccessibleName('Priority');
  const revised = { ...withArgument('Delivery priority'), face_revision: 2, show_id: 'fixture/revised-priority' };
  await element.evaluate((node, view) => node.show(view, {
    invoke: async call => { globalThis.calls.push(call); },
  }), revised);
  await expect(select).toHaveAccessibleName('Delivery priority');
  await select.selectOption('Normal');
  const button = element.getByRole('button', { name: 'Complete task', exact: true });
  await button.focus();
  await page.keyboard.press('Enter');
  expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view: revised,
    action: revised.actions[0], arguments: [{ name: 'priority', value: 'Normal' }] }]);
});

test('named Face values expose their labels and exact values without styling', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  await page.evaluate(async view => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    document.body.replaceChildren();
    const element = document.createElement('conduit-owner-face');
    document.body.append(element);
    element.show(view, { invoke: async () => {} });
    element.shadowRoot.querySelector('link').remove();
  }, view);
  const element = page.locator('conduit-owner-face');
  await expect(element.getByRole('term')).toHaveText('Completion');
  await expect(element.getByRole('definition')).toHaveText('Open');
  await expect(element.locator('[part=value]')).toHaveText('Open');
  const revised = { ...view, face_revision: 2, show_id: 'fixture/completed', subjects: [{
    ...view.subjects[0], values: [{ name: 'Completion', text: 'Done' }, { name: 'Count', text: '1' }],
  }] };
  await element.evaluate((node, view) => node.show(view, { invoke: async () => {} }), revised);
  await expect(element.getByRole('term')).toHaveText(['Completion', 'Count']);
  await expect(element.getByRole('definition')).toHaveText(['Done', '1']);
});
