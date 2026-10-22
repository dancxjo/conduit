import { expect, test } from '@playwright/test';

// Renderer fixture only: the callback is not a Body/action receipt.
const view = {
  body_id: 'fixture/body', face_id: 'fixture/face', face_revision: 1, show_id: 'fixture/show',
  show_state: 'available', interactions_admitted: true,
  subjects: [{ identity: 'fixture/task', name: 'Read the handbook', role: 'Info',
    disclosure: 'Primary', text: [], properties: [], unmodeled_property_count: 0,
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
        palette: 'phosphor', typography: 'editorial', version: 2, text_scale_percent: 100 } } },
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
        values: [], semantic_role: 'conduit.face/choice-group@1', choice_multiplicity: 'exclusive' },
      ...[true, false].map((selected, index) => ({ ...view.subjects[0],
        identity: `fixture/option/${index}`, name: `Mode ${index}`,
        values: [], flags: [{ name: 'selected', value: selected }] })),
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
    const look = element.shadowRoot.querySelector('link');
    if (!look.sheet) await new Promise((resolve, reject) => {
      look.addEventListener('load', resolve, { once: true });
      look.addEventListener('error', () => reject(new Error('owner Look did not load')), { once: true });
    });
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

for (const scenario of ['unsupported', 'utf8', 'choice', 'minimum', 'required']) {
  test(`current forms refuse invalid ${scenario} arguments before invoking`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const argument = { name: 'value', value_name: 'Value', value_kind: 'value/text',
      maximum_bytes: 4, choices: [] };
    if (scenario === 'unsupported') argument.value_kind = 'value/unknown';
    if (scenario === 'choice') argument.choices = ['one', 'two'];
    if (scenario === 'minimum') argument.minimum_bytes = 3;
    if (scenario === 'required') argument.minimum_bytes = 1;
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
      if (scenario === 'minimum') form.querySelector('input').value = 'é';
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
    if (['utf8', 'minimum', 'required'].includes(scenario)) {
      const input = element.getByLabel('Value', { exact: true });
      await expect(input).toHaveJSProperty('required', scenario !== 'utf8');
      expect(await input.evaluate(node => node.validationMessage)).toBe(scenario === 'utf8'
        ? 'Use at most 4 UTF-8 bytes.' : `Use at least ${scenario === 'minimum' ? 3 : 1} UTF-8 bytes.`);
      await input.fill('éé');
    } else await element.getByRole('combobox').selectOption('one');
    await button.focus();
    await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view: currentView,
      action: currentView.actions[0], arguments: [{ name: 'value', value: scenario === 'choice' ? 'one' : 'éé' }] }]);
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
  await expect(element.getByLabel('Priority', { exact: true })).toHaveCount(1);
  await expect(select).toHaveAttribute('required', '');
  await select.selectOption('Urgent');
  await expect(select).toHaveAccessibleName('Priority');
  await expect(element.getByLabel('Priority', { exact: true })).toHaveCount(1);
  const revised = { ...withArgument('Delivery priority'), face_revision: 2, show_id: 'fixture/revised-priority' };
  await element.evaluate((node, view) => node.show(view, {
    invoke: async call => { globalThis.calls.push(call); },
  }), revised);
  await expect(select).toHaveAccessibleName('Delivery priority');
  await expect(element.getByLabel('Delivery priority', { exact: true })).toHaveCount(1);
  await expect(element.getByLabel('Priority', { exact: true })).toHaveCount(0);
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

for (const control of ['submit', 'argument']) {
  test(`current Face refresh preserves ${control} focus by semantic identity`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const withInput = { ...view, actions: [{ ...view.actions[0], arguments: [{
      name: 'text', value_name: 'Text', value_kind: 'value/text', maximum_bytes: 32, choices: [],
    }] }] };
    await page.evaluate(async view => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      document.body.replaceChildren();
      const element = document.createElement('conduit-owner-face');
      document.body.append(element);
      globalThis.calls = [];
      element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
    }, withInput);
    const element = page.locator('conduit-owner-face');
    const focused = control === 'submit'
      ? element.getByRole('button', { name: 'Complete task', exact: true })
      : element.getByRole('textbox', { name: 'Text', exact: true });
    if (control === 'argument') await focused.fill('abcdef');
    await focused.focus();
    if (control === 'argument') {
      await page.keyboard.press('Home');
      await page.keyboard.press('ArrowRight'); await page.keyboard.press('ArrowRight');
      await page.keyboard.press('ArrowRight'); await page.keyboard.press('ArrowRight');
      await page.keyboard.press('Shift+ArrowLeft'); await page.keyboard.press('Shift+ArrowLeft');
    }
    const next = { ...withInput, face_revision: 2, show_id: 'fixture/refreshed' };
    await element.evaluate((node, view) => node.show(view, {
      invoke: async call => { globalThis.calls.push(call); },
    }), next);
    await expect(focused).toBeFocused();
    if (control === 'argument') {
      expect(await focused.evaluate(node => [node.selectionStart, node.selectionEnd, node.selectionDirection]))
        .toEqual([2, 4, 'backward']);
      await page.keyboard.type('X');
      await expect(focused).toHaveValue('abXef');
      await page.keyboard.press('Shift+ArrowLeft');
      await element.evaluate((node, view) => {
        node.disabled = true;
        node.show({ ...view, show_state: 'prepared', interactions_admitted: false }, { invoke: async () => {} });
        node.show(view, { invoke: async call => { globalThis.calls.push(call); } });
      }, next);
      expect(await focused.evaluate(node => [node.selectionStart, node.selectionEnd, node.selectionDirection]))
        .toEqual([2, 3, 'backward']);
      await page.keyboard.press('Backspace');
      await expect(focused).toHaveValue('abef');
    }
    expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
    await element.evaluate((node, view) => node.show({ ...view, actions: [{
      ...view.actions[0], availability: 'unavailable',
    }] }, { invoke: async () => {} }), next);
    await expect(focused).not.toBeFocused();
    await expect(element.getByRole('button', { name: 'Complete task', exact: true })).toBeDisabled();
  });
}

test('Face refresh neither transfers focus to a different target nor steals external focus', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  await page.evaluate(async view => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    document.body.replaceChildren();
    const element = document.createElement('conduit-owner-face');
    const outside = document.createElement('button'); outside.textContent = 'Outside';
    document.body.append(element, outside);
    element.show(view, { invoke: async () => {} });
  }, view);
  const element = page.locator('conduit-owner-face');
  const control = element.getByRole('button', { name: 'Complete task', exact: true });
  await control.focus();
  const changed = { ...view, subjects: [{ ...view.subjects[0], identity: 'fixture/other' }],
    actions: [{ ...view.actions[0], target: 'fixture/other' }] };
  await element.evaluate((node, view) => node.show(view, { invoke: async () => {} }), changed);
  await expect(control).not.toBeFocused();
  const outside = page.getByRole('button', { name: 'Outside', exact: true });
  await control.focus();
  await element.evaluate((node, view) => {
    node.disabled = true;
    node.show({ ...view, show_state: 'prepared', interactions_admitted: false }, { invoke: async () => {} });
  }, changed);
  await outside.focus();
  await element.evaluate((node, view) => node.show(view, { invoke: async () => {} }), changed);
  await expect(outside).toBeFocused();
});

for (const kind of ['text', 'choice']) {
  test(`unsent ${kind} survives only an unchanged same-Body argument contract`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const argument = { name: 'value', value_name: 'Value', value_kind: 'value/text',
      maximum_bytes: 16, choices: kind === 'choice' ? ['one', 'two'] : [] };
    const currentView = { ...view, actions: [{ ...view.actions[0], arguments: [argument] }] };
    await page.evaluate(async view => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      document.body.replaceChildren();
      const element = document.createElement('conduit-owner-face');
      document.body.append(element);
      globalThis.calls = [];
      globalThis.refresh = next => element.show(next, { invoke: async call => { globalThis.calls.push(call); } });
      globalThis.refresh(view);
    }, currentView);
    const element = page.locator('conduit-owner-face');
    const input = element.getByRole(kind === 'text' ? 'textbox' : 'combobox');
    const value = kind === 'text' ? 'é draft' : 'two';
    const edit = async () => kind === 'text' ? input.fill(value) : input.selectOption(value);
    await input.evaluate((node, kind) => {
      if (kind === 'text') node.value = 'x'.repeat(17);
      else { node.add(new Option('foreign', 'foreign')); node.value = 'foreign'; }
    }, kind);
    await page.evaluate(next => globalThis.refresh(next), currentView);
    await expect(input).toHaveValue('');
    await edit();
    const next = { ...currentView, face_revision: 2, show_id: 'fixture/draft-refresh' };
    await page.evaluate(next => globalThis.refresh(next), next);
    await expect(input).toHaveValue(value);
    expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
    const changed = { ...next, actions: [{ ...next.actions[0], arguments: [{
      ...argument, maximum_bytes: 8,
    }] }] };
    await page.evaluate(next => globalThis.refresh(next), changed);
    await expect(input).toHaveValue('');
    await edit();
    await input.focus();
    await page.evaluate(next => globalThis.refresh({ ...next, body_id: 'fixture/other-body' }), changed);
    await expect(input).toHaveValue('');
    await expect(input).not.toBeFocused();
    await edit();
    const button = element.getByRole('button', { name: 'Complete task', exact: true });
    await button.focus();
    await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls.length)).toBe(1);
    await page.evaluate(next => globalThis.refresh({ ...next, body_id: 'fixture/other-body' }), changed);
    await expect(input).toHaveValue('');
  });
}

for (const reattach of [false, true]) {
  test(`disconnected owner forms lose their route (reattach: ${reattach})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    await page.evaluate(async ({ view, reattach }) => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      document.body.replaceChildren();
      const element = document.createElement('conduit-owner-face');
      document.body.append(element);
      globalThis.calls = [];
      globalThis.detachedOwner = element;
      element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
      const form = element.shadowRoot.querySelector('form');
      element.remove();
      if (reattach) document.body.append(element);
      // Re-enabling controls cannot restore the disconnected Show's bindings.
      element.disabled = false;
      form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    }, { view, reattach });
    expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
    await page.evaluate(view => {
      const element = globalThis.detachedOwner;
      if (!element.isConnected) document.body.append(element);
      element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
    }, view);
    const button = page.locator('conduit-owner-face').getByRole('button', { name: 'Complete task', exact: true });
    await expect(button).toBeEnabled();
    await button.focus();
    await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view, action: view.actions[0], arguments: [] }]);
  });
}

for (const unavailable of ['disconnected', 'hidden', 'transparent', 'collapsed']) {
test(`a ${unavailable} exclusive-choice listener cannot dispatch or reset current selection`, async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  const choices = {
    ...view,
    subjects: [
      { ...view.subjects[0], identity: 'fixture/group', name: 'Mode',
        values: [], semantic_role: 'conduit.face/choice-group@1', choice_multiplicity: 'exclusive' },
      ...[true, false].map((selected, index) => ({ ...view.subjects[0],
        identity: `fixture/option/${index}`, name: `Mode ${index}`,
        values: [], flags: [{ name: 'selected', value: selected }] })),
    ],
    relationships: [0, 1].map(index => ({ kind: 'Contains', source: 'fixture/group', target: `fixture/option/${index}` })),
    actions: [0, 1].map(index => ({ ...view.actions[0], identity: `fixture/select/${index}`,
      target: `fixture/option/${index}`, name: `Select ${index}` })),
  };
  await page.evaluate(async ({ view, unavailable }) => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    document.body.replaceChildren();
    const element = document.createElement('conduit-owner-face');
    document.body.append(element);
    globalThis.calls = [];
    const invoke = async call => { globalThis.calls.push(call); };
    element.show(view, { invoke });
    const look = element.shadowRoot.querySelector('link');
    if (!look.sheet) await new Promise((resolve, reject) => {
      look.addEventListener('load', resolve, { once: true });
      look.addEventListener('error', () => reject(new Error('owner Look did not load')), { once: true });
    });
    const obsolete = element.shadowRoot.querySelector('input[type=radio]');
    if (!obsolete) throw new Error('fixture did not render an exclusive choice');
    if (unavailable === 'disconnected') {
      element.remove();
      document.body.append(element);
      element.disabled = false;
    } else if (unavailable === 'transparent') obsolete.closest('label').style.opacity = '0';
    else if (unavailable === 'collapsed') obsolete.closest('label').style.transform = 'scale(0)';
    else obsolete.hidden = true;
    obsolete.dispatchEvent(new Event('change', { bubbles: true }));
  }, { view: choices, unavailable });
  expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
  const element = page.locator('conduit-owner-face');
  await element.evaluate((node, view) => node.show(view, {
    invoke: async call => { globalThis.calls.push(call); },
  }), choices);
  await expect(element.getByRole('radio', { name: 'Mode 0', exact: true })).toBeChecked();
  const current = element.getByRole('radio', { name: 'Mode 1', exact: true });
  await expect(current).toBeEnabled();
  await current.focus();
  await page.keyboard.press('Space');
  expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view: choices, action: choices.actions[1], arguments: [] }]);
  await expect(current).toBeDisabled();
});
}

for (const hidden of ['attribute', 'owner-attribute', 'visibility', 'opacity', 'owner-opacity', 'scale', 'owner-scale', 'disclosure', 'fieldset']) {
  test(`hidden owner actions refuse dispatch (${hidden})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    await page.evaluate(async ({ view, hidden }) => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      document.body.replaceChildren();
      const element = document.createElement('conduit-owner-face');
      document.body.append(element);
      globalThis.calls = [];
      element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
      const look = element.shadowRoot.querySelector('link');
      if (!look.sheet) await new Promise((resolve, reject) => {
        look.addEventListener('load', resolve, { once: true });
        look.addEventListener('error', () => reject(new Error('owner Look did not load')), { once: true });
      });
      const form = element.shadowRoot.querySelector('form');
      if (hidden === 'attribute') form.hidden = true;
      if (hidden === 'owner-attribute') element.hidden = true;
      if (hidden === 'visibility') form.style.visibility = 'hidden';
      if (hidden === 'opacity') form.style.opacity = '0';
      if (hidden === 'owner-opacity') element.style.opacity = '0';
      if (hidden === 'scale') form.style.transform = 'scale(0)';
      if (hidden === 'owner-scale') element.style.transform = 'scale(0)';
      if (hidden === 'disclosure') {
        const details = document.createElement('details');
        const summary = document.createElement('summary'); summary.textContent = 'Actions';
        form.before(details); details.append(summary, form);
      }
      if (hidden === 'fieldset') {
        const fieldset = document.createElement('fieldset'); fieldset.disabled = true;
        form.before(fieldset); fieldset.append(form);
      }
      // Deliberately adversarial: ordinary users cannot activate hidden/disabled controls.
      form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    }, { view, hidden });
    expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
    const element = page.locator('conduit-owner-face');
    if (hidden === 'attribute' || hidden === 'owner-attribute') await expect(element.locator('form')).toBeHidden();
    await element.evaluate(node => {
      const form = node.shadowRoot.querySelector('form');
      form.hidden = false; node.hidden = false; form.style.visibility = ''; form.style.opacity = ''; node.style.opacity = ''; form.style.transform = ''; node.style.transform = '';
      const fieldset = form.closest('fieldset');
      if (fieldset) fieldset.disabled = false;
    });
    if (hidden === 'disclosure') {
      await element.getByText('Actions', { exact: true }).focus();
      await page.keyboard.press('Enter');
    }
    const button = element.getByRole('button', { name: 'Complete task', exact: true });
    await expect(button).toBeVisible();
    await button.focus();
    await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view, action: view.actions[0], arguments: [] }]);
  });
}

// Production Handbook orchestration with a fixture SDK, not a real owner Body.
for (const interruption of [null, 'prepare', 'acknowledge', 'inaccessible']) {
test(`Handbook refresh preserves current controls and refuses interrupted ${interruption ?? 'none'} routes`, async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  await page.evaluate(async ({ view, interruption }) => {
    const { startOwnerParticipation } = await import('/targets/browser/handbook/owner-participation.mjs');
    document.body.innerHTML = `<header class="handbook-introduction"><h1></h1><p class="lede"></p></header>
      <a class="handbook-start"></a><div class="handbook-first-steps"></div>
      <div class="handbook-next"></div><div class="handbook-local-note"></div><main></main>`;
    const shown = { ...view, body_id: 'body/fixture', face_revision: '1',
      actions: [{ ...view.actions[0], arguments: [{ name: 'draft', value_name: 'Draft',
        value_kind: 'value/text', maximum_bytes: 16, choices: [] }] }],
      mask_plot_id: 'fixture/mask', mask_plan_id: 'fixture/plan', mask_play_id: 'fixture/play',
      route: { plan_id: 'fixture/route', owner_host_id: 'host/owner', owner_boot_id: 'boot/owner',
        mask_host_id: 'host/browser', mask_boot_id: 'boot/browser' } };
    globalThis.calls = [];
    let revision = 0;
    let pendingPrepared = false;
    globalThis.discardedMasks = 0;
    let onState;
    const pause = async stage => {
      if (revision === 2 && stage === interruption) {
        await new Promise(resolve => { globalThis.releaseFace = resolve; });
      }
    };
    globalThis.interruptRoute = () => {
      onState('offline');
      // Presence may recover before the old async result arrives.
      onState('admitted');
      globalThis.releaseFace();
    };
    const participation = {
      hostId: 'host/browser', bootId: 'boot/browser',
      presenceState: () => 'available', membershipCredential: () => ({ body_id: shown.body_id }),
      biographyEvidence: () => null,
      discardOwnerFaceMask: () => { pendingPrepared = false; globalThis.discardedMasks++; },
      prepareOwnerFaceMask: async () => {
        if (pendingPrepared) throw new Error('owner Face Mask refused (-8)');
        pendingPrepared = true;
        revision++;
        await pause('prepare');
        return { ...shown, face_revision: String(revision), show_id: `fixture/show/${revision}`,
          show_state: 'prepared', interactions_admitted: false };
      },
      acknowledgeOwnerFaceMask: async prepared => {
        await pause('acknowledge');
        pendingPrepared = false;
        return { ...prepared, show_state: 'available', interactions_admitted: true };
      },
      inspectOwnerWardrobe: async () => { throw new Error('No wardrobe in this renderer fixture'); },
      submitOwnerFaceInteraction: async call => { globalThis.calls.push(call); return { accepted: true }; },
    };
    await startOwnerParticipation({
      storage: { readJson: async () => null, writeJson: async () => {} },
      browser: async () => ({ id: 'host/browser', bootId: 'boot/browser',
        admissionIdentity: () => ({ hostId: 'host/browser', bootId: 'boot/browser', verifyingKey: [] }),
        participate: async options => { onState = options.onState; return participation; } }),
    }, document.querySelector('main'));
  }, { view, interruption });
  await page.getByLabel('Body ID', { exact: true }).fill('body/fixture');
  await page.getByLabel('Owner window URL', { exact: true }).fill('ws://127.0.0.1:41000/conduit');
  await page.getByRole('button', { name: 'Join this Body', exact: true }).click();
  const owner = page.locator('conduit-owner-face');
  await expect(page.locator('main')).toHaveAttribute('data-owner-face-shown', 'true');
  const button = owner.getByRole('button', { name: 'Complete task', exact: true });
  await expect(button).toBeEnabled();
  const draft = owner.getByRole('textbox', { name: 'Draft', exact: true });
  await draft.fill('é draft');
  await owner.evaluate(node => {
    globalThis.originalOwner = node;
    globalThis.disconnects = 0;
    const observer = new MutationObserver(records => {
      for (const record of records) for (const removed of record.removedNodes) {
        if (removed === node) globalThis.disconnects++;
      }
    });
    observer.observe(node.parentNode, { childList: true });
  });
  if (interruption === 'inaccessible') await owner.evaluate(node => { node.hidden = true; });
  await page.getByRole('button', { name: 'Refresh this Face', exact: true }).click();
  if (interruption === 'inaccessible') {
    await expect(page.locator('[data-owner-face-status]')).toHaveAttribute('data-refused', 'true');
    await expect(page.locator('main')).not.toHaveAttribute('data-owner-show-acknowledged');
    expect(await page.evaluate(() => globalThis.discardedMasks)).toBe(1);
    await owner.evaluate(node => { node.hidden = false; });
    await page.getByRole('button', { name: 'Refresh this Face', exact: true }).click();
    await expect(page.locator('main')).toHaveAttribute('data-owner-show-acknowledged', 'fixture/show/3');
    await expect(button).toBeEnabled();
    expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
    return;
  }
  if (interruption) {
    await expect.poll(() => page.evaluate(() => typeof globalThis.releaseFace)).toBe('function');
    await page.evaluate(() => globalThis.interruptRoute());
    await expect(page.locator('[data-owner-face-status]')).toHaveAttribute('data-refused', 'true');
    await expect(page.locator('[data-owner-face-status]')).toContainText('The owner route changed during Face preparation.');
    await expect(page.locator('main')).not.toHaveAttribute('data-owner-show-acknowledged');
    await expect(page.locator('main')).not.toHaveAttribute('data-owner-face-shown', 'true');
    await expect(button).toBeDisabled();
    await owner.evaluate(node => node.shadowRoot.querySelector('form')
      .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
    expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
    await page.getByRole('button', { name: 'Refresh this Face', exact: true }).click();
  }
  const currentRevision = interruption ? '3' : '2';
  await expect(owner).toHaveAttribute('data-face-revision', currentRevision);
  await expect(button).toBeEnabled();
  await expect(draft).toHaveValue('é draft');
  expect(await owner.evaluate(node => node === globalThis.originalOwner && globalThis.disconnects === 0)).toBe(true);
  await button.focus();
  await page.keyboard.press('Enter');
  await expect.poll(() => page.evaluate(() => globalThis.calls.length)).toBe(1);
  await expect(owner).toHaveAttribute('data-face-revision', String(Number(currentRevision) + 1));
  await expect(draft).toHaveValue('');
  await expect(button).toBeEnabled();
  await expect(button).toBeFocused();
  expect(await page.evaluate(() => globalThis.calls[0])).toMatchObject({
    view: { show_id: `fixture/show/${currentRevision}`, face_revision: currentRevision },
    actionId: view.actions[0].identity, target: view.actions[0].target, arguments: [{ name: 'draft', value: 'é draft' }], sequence: 1,
  });
});
}

for (const malformed of ['missing', 'non-array', 'negative-bound', 'negative-minimum', 'reversed-bounds', 'choice-bytes', 'too-many',
  'missing-name', 'empty-name', 'duplicate-name', 'missing-label', 'empty-label', 'choice-unicode']) {
  test(`malformed owner argument contracts refuse without losing other actions (${malformed})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const broken = { ...view.actions[0], identity: 'fixture/broken', name: 'Broken action', arguments: [{
      name: 'value', value_name: 'Value', value_kind: 'value/text', maximum_bytes: 4, choices: [],
    }] };
    if (malformed === 'missing-name') delete broken.arguments[0].name;
    if (malformed === 'empty-name') broken.arguments[0].name = '   ';
    if (malformed === 'duplicate-name') broken.arguments.push({ ...broken.arguments[0], value_name: 'Other value' });
    if (malformed === 'missing-label') delete broken.arguments[0].value_name;
    if (malformed === 'empty-label') broken.arguments[0].value_name = '   ';
    if (malformed === 'missing') delete broken.arguments;
    if (malformed === 'non-array') broken.arguments = {};
    if (malformed === 'too-many') broken.arguments = Array.from({ length: 65 }, (_, index) => ({
      name: `value/${index}`, value_name: `Value ${index}`, value_kind: 'value/text', maximum_bytes: 4, choices: [],
    }));
    if (malformed === 'negative-bound') broken.arguments[0].maximum_bytes = -1;
    if (malformed === 'negative-minimum') broken.arguments[0].minimum_bytes = -1;
    if (malformed === 'reversed-bounds') broken.arguments[0].minimum_bytes = 5;
    if (malformed === 'choice-bytes') broken.arguments[0].choices = ['ééé'];
    if (malformed === 'choice-unicode') broken.arguments[0].choices = ['\ud800'];
    await page.evaluate(async view => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      document.body.replaceChildren();
      globalThis.calls = [];
      const element = document.createElement('conduit-owner-face'); document.body.append(element);
      element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
    }, { ...view, actions: [broken, ...view.actions] });
    const element = page.locator('conduit-owner-face');
    await expect(element.getByRole('button', { name: 'Broken action', exact: true })).toBeDisabled();
    await expect(element.getByText('This input has no supported browser form.', { exact: true })).toBeVisible();
    await expect(element.locator('form[data-owner-action="fixture/broken"] input')).toHaveCount(0);
    await element.locator('form[data-owner-action="fixture/broken"]').evaluate(form =>
      form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })));
    expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
    const current = element.getByRole('button', { name: 'Complete task', exact: true });
    await current.focus(); await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls.length)).toBe(1);
    expect(await page.evaluate(() => globalThis.calls[0].action.identity)).toBe('fixture/complete');
  });
}

for (const malformed of ['ambiguous-flag', 'duplicate-option', 'missing-arguments', 'non-array-flags']) {
  test(`exclusive-choice contracts use reachable generic fallback (${malformed})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const choices = { ...view,
      subjects: [
        { ...view.subjects[0], identity: 'fixture/group', name: 'Mode',
          values: [], semantic_role: 'conduit.face/choice-group@1', choice_multiplicity: 'exclusive' },
        ...[0, 1].map(index => ({ ...view.subjects[0], identity: `fixture/option/${index}`,
          name: `Mode ${index}`, values: [], flags: [{ name: 'selected', value: false }] })),
      ],
      relationships: [0, 1].map(index => ({ kind: 'Contains', source: 'fixture/group', target: `fixture/option/${index}` })),
      actions: [0, 1].map(index => ({ ...view.actions[0], identity: `fixture/select/${index}`,
        target: `fixture/option/${index}`, name: `Select ${index}` })),
    };
    if (malformed === 'ambiguous-flag') choices.subjects[1].flags.unshift({ name: 'selected', value: 'false' });
    if (malformed === 'duplicate-option') choices.relationships.push(choices.relationships[0]);
    if (malformed === 'missing-arguments') delete choices.actions[0].arguments;
    if (malformed === 'non-array-flags') choices.subjects[1].flags = {};
    await page.evaluate(async view => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      document.body.replaceChildren(); globalThis.calls = [];
      const element = document.createElement('conduit-owner-face'); document.body.append(element);
      element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
    }, choices);
    const element = page.locator('conduit-owner-face');
    await expect(element.getByRole('radio')).toHaveCount(0);
    expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
    const action = element.getByRole('button', { name: 'Select 1', exact: true });
    await expect(action).toBeEnabled();
    await action.focus(); await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view: choices, action: choices.actions[1], arguments: [] }]);
  });
}

for (const fact of ['content', 'flag']) {
for (const enriched of ['collection', 'item', 'status']) {
  test(`collection layout preserves additional ${enriched} semantics (${fact})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const subject = (identity, name, role) => ({ identity, name, role, disclosure: 'Primary', text: [], properties: [], unmodeled_property_count: 0, values: [] });
    const collection = subject('fixture/list', 'Tasks', 'Collection');
    const item = { ...subject('fixture/item', 'Read', 'Item'), flags: [{ name: 'complete', value: false }] };
    const status = { ...subject('fixture/status', 'Progress', 'Status'), text: ['1 thing left · 0 completed'] };
    const extra = { collection, item, status }[enriched];
    if (fact === 'flag') extra.flags = [...(extra.flags ?? []), { name: 'authored-flag', value: true }];
    else {
      extra.properties = ['Important authored fact']; extra.unmodeled_property_count = 1;
      extra.values = [{ name: 'Priority', text: 'Urgent' }];
      if (enriched !== 'status') extra.text = ['Additional explanation'];
    }
    const currentView = { ...view, subjects: [collection, item, status],
      relationships: [item, status].map(subject => ({ kind: 'Contains', source: collection.identity, target: subject.identity })),
      actions: [{ ...view.actions[0], target: item.identity }],
    };
    await page.evaluate(async view => {
      const { projectFaceCollections } = await import('/targets/browser/handbook/owner-participation.mjs');
      document.body.replaceChildren(); globalThis.calls = [];
      const element = document.createElement('conduit-owner-face'); document.body.append(element);
      element.show(view, { projectCollections: projectFaceCollections,
        invoke: async call => { globalThis.calls.push(call); } });
    }, currentView);
    const element = page.locator('conduit-owner-face');
    if (fact === 'flag') {
      const facts = element.locator('[part=subject]').filter({ hasText: 'authored-flag: true' });
      await expect(facts).toHaveCount(1);
      await facts.getByText('Exact facts', { exact: true }).click();
      await expect(facts.getByText('authored-flag: true', { exact: true })).toBeVisible();
    } else {
    await expect(element.getByRole('term')).toHaveText('Priority');
    await expect(element.getByRole('definition')).toHaveText('Urgent');
    if (enriched !== 'status') await expect(element.getByText('Additional explanation', { exact: true })).toBeVisible();
    await element.locator('[part=subject]').filter({ hasText: 'Important authored fact' })
      .getByText('Exact facts', { exact: true }).click();
    await expect(element.getByText('Important authored fact', { exact: true })).toBeVisible();
    }
    const action = element.getByRole('button', { name: /^Complete task/ });
    await action.focus(); await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view: currentView,
      action: currentView.actions[0], arguments: [] }]);
  });
}

}

for (const fact of ['content', 'flag']) {
for (const enriched of ['group', 'option']) {
  test(`exclusive-choice layout preserves additional ${enriched} semantics (${fact})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const choices = { ...view,
      subjects: [
        { ...view.subjects[0], identity: 'fixture/group', name: 'Mode',
          values: [], semantic_role: 'conduit.face/choice-group@1', choice_multiplicity: 'exclusive' },
        ...[0, 1].map(index => ({ ...view.subjects[0], identity: `fixture/option/${index}`,
          name: `Mode ${index}`, values: [], flags: [{ name: 'selected', value: index === 0 }] })),
      ],
      relationships: [0, 1].map(index => ({ kind: 'Contains', source: 'fixture/group', target: `fixture/option/${index}` })),
      actions: [0, 1].map(index => ({ ...view.actions[0], identity: `fixture/select/${index}`,
        target: `fixture/option/${index}`, name: `Select ${index}` })),
    };
    const extra = choices.subjects[enriched === 'group' ? 0 : 2];
    if (fact === 'flag') extra.flags = [...(extra.flags ?? []), { name: 'authored-flag', value: true }];
    else {
      extra.values = [{ name: 'Authored value', text: 'Exact meaning' }];
      extra.properties = ['Important authored fact']; extra.unmodeled_property_count = 1;
      extra.text = ['Additional explanation'];
    }
    await page.evaluate(async view => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      document.body.replaceChildren(); globalThis.calls = [];
      const element = document.createElement('conduit-owner-face'); document.body.append(element);
      element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
    }, choices);
    const element = page.locator('conduit-owner-face');
    if (fact === 'flag') {
      const facts = element.locator('[part=subject]').filter({ hasText: 'authored-flag: true' });
      await expect(facts).toHaveCount(1);
      await facts.getByText('Exact facts', { exact: true }).click();
      await expect(facts.getByText('authored-flag: true', { exact: true })).toBeVisible();
    } else {
    await expect(element.getByRole('term')).toHaveText('Authored value');
    await expect(element.getByRole('definition')).toHaveText('Exact meaning');
    await expect(element.getByText('Additional explanation', { exact: true })).toBeVisible();
    await element.locator('[part=subject]').filter({ hasText: 'Important authored fact' })
      .getByText('Exact facts', { exact: true }).click();
    await expect(element.getByText('Important authored fact', { exact: true })).toBeVisible();
    }
    const selectedFacts = element.locator('[part=subject]').filter({ hasText: 'selected: true' });
    await selectedFacts.getByText('Exact facts', { exact: true }).click();
    await expect(selectedFacts.getByText('selected: true', { exact: true })).toBeVisible();
    const action = element.getByRole('button', { name: 'Select 1', exact: true });
    await action.focus(); await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view: choices,
      action: choices.actions[1], arguments: [] }]);
  });
}

}

for (const scope of ['owner', 'light-ancestor', 'shadow-ancestor', 'outer-shadow-host', 'slotted-ancestor']) {
  test(`inert owner controls refuse dispatch (${scope})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    await page.evaluate(async ({ view, scope }) => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      document.body.replaceChildren(); globalThis.calls = [];
      const element = document.createElement('conduit-owner-face'); document.body.append(element);
      element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
      let inert = element;
      if (scope === 'light-ancestor' || scope === 'outer-shadow-host') {
        inert = document.createElement('div'); element.before(inert);
        if (scope === 'outer-shadow-host') inert.attachShadow({ mode: 'open' }).append(element);
        else inert.append(element);
        // Moving a surface requires a fresh current render.
        element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
      }
      if (scope === 'slotted-ancestor') {
        const host = document.createElement('div'); element.before(host);
        inert = document.createElement('div');
        inert.append(document.createElement('slot'));
        host.attachShadow({ mode: 'open' }).append(inert); host.append(element);
        element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
      }
      if (scope === 'shadow-ancestor') inert = element.shadowRoot.querySelector('[part=document]');
      inert.inert = true;
      globalThis.inertContainer = inert;
      element.shadowRoot.querySelector('form')
        .dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    }, { view, scope });
    expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
    await page.evaluate(() => { globalThis.inertContainer.inert = false; });
    const action = page.locator('conduit-owner-face').getByRole('button', { name: 'Complete task', exact: true });
    await expect(action).toBeEnabled();
    await action.focus(); await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view, action: view.actions[0], arguments: [] }]);
  });
}

for (const membership of ['partial', 'duplicate', 'duplicate-without-status', 'without-status']) {
  test(`collection fallback does not invent item totals (${membership})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const collection = { ...view.subjects[0], identity: 'fixture/list', name: 'Tasks', role: 'Collection', values: [] };
    const item = { ...view.subjects[0], identity: 'fixture/item', name: 'Read', role: 'Item', values: [],
      flags: [{ name: 'complete', value: false }] };
    const count = membership === 'partial' ? '20 things left · 0 completed' : '1 thing left · 0 completed';
    const status = { ...view.subjects[0], identity: 'fixture/status', name: 'Progress', role: 'Status', values: [], text: [count] };
    const relation = { kind: 'Contains', source: collection.identity, target: item.identity };
    const currentView = { ...view, subjects: [collection, item], relationships: [relation],
      actions: [{ ...view.actions[0], target: item.identity }] };
    if (membership.startsWith('duplicate')) currentView.relationships.push({ ...relation });
    if (!membership.includes('without-status')) {
      currentView.subjects.push(status);
      currentView.relationships.push({ kind: 'Contains', source: collection.identity, target: status.identity });
    }
    await page.evaluate(async view => {
      const { projectFaceCollections } = await import('/targets/browser/handbook/owner-participation.mjs');
      document.body.replaceChildren(); globalThis.calls = [];
      const element = document.createElement('conduit-owner-face'); document.body.append(element);
      element.show(view, { projectCollections: projectFaceCollections,
        invoke: async call => { globalThis.calls.push(call); } });
    }, currentView);
    const element = page.locator('conduit-owner-face');
    await expect(element.getByText(!membership.startsWith('duplicate') ? '1 thing left · 0 completed' : '2 things left · 0 completed', { exact: true })).toHaveCount(0);
    if (!membership.includes('without-status')) await expect(element.getByText(count, { exact: true })).toBeVisible();
    const action = element.getByRole('button', { name: /^Complete task/ });
    await expect(action).toHaveCount(1);
    await action.focus(); await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view: currentView,
      action: currentView.actions[0], arguments: [] }]);
  });
}

for (const kind of ['facts', 'completed']) {
  test(`local ${kind} disclosure survives refresh within its encounter`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const currentView = kind === 'facts' ? { ...view, subjects: [{ ...view.subjects[0], properties: ['Authored fact'] }] }
      : { ...view, subjects: [
        { ...view.subjects[0], identity: 'fixture/list', name: 'Tasks', role: 'Collection', values: [] },
        { ...view.subjects[0], role: 'Item', name: 'Archived task', values: [], disclosure: 'SelectedDetail',
          flags: [{ name: 'complete', value: true }] },
        { ...view.subjects[0], identity: 'fixture/status', role: 'Status', values: [], text: ['0 things left · 1 completed'] },
      ], relationships: ['fixture/task', 'fixture/status'].map(target => ({ kind: 'Contains', source: 'fixture/list', target })),
      actions: [{ ...view.actions[0], name: 'Reopen' }],
    };
    await page.evaluate(async view => {
      const { projectFaceCollections } = await import('/targets/browser/handbook/owner-participation.mjs');
      document.body.replaceChildren(); globalThis.calls = [];
      const element = document.createElement('conduit-owner-face'); document.body.append(element);
      globalThis.refreshDisclosure = next => element.show(next, { projectCollections: projectFaceCollections,
        invoke: async call => { globalThis.calls.push(call); } });
      globalThis.refreshDisclosure(view);
    }, currentView);
    const element = page.locator('conduit-owner-face');
    const summary = element.getByText(kind === 'facts' ? 'Exact facts' : 'Show 1 completed item', { exact: true });
    await summary.focus(); await page.keyboard.press('Enter');
    await expect.poll(() => summary.evaluate(node => node.parentElement.open)).toBe(true);
    const keptFocus = kind === 'facts' ? summary : element.getByRole('button', { name: /^Reopen/ });
    await keptFocus.focus();
    const next = { ...currentView, face_revision: 2, show_id: 'fixture/disclosure-refresh' };
    await page.evaluate(next => {
      globalThis.refreshDisclosure({ ...next, show_state: 'prepared', interactions_admitted: false });
      globalThis.refreshDisclosure(next);
    }, next);
    await expect.poll(() => summary.evaluate(node => node.parentElement.open)).toBe(true);
    await expect(keptFocus).toBeFocused();
    await summary.focus();
    await page.keyboard.press('Enter');
    await page.evaluate(next => globalThis.refreshDisclosure(next), next);
    await expect.poll(() => summary.evaluate(node => node.parentElement.open)).toBe(false);
    await expect(summary).toBeFocused();
    await page.keyboard.press('Enter');
    const other = { ...next, body_id: 'fixture/other-body' };
    await page.evaluate(next => globalThis.refreshDisclosure(next), other);
    await expect.poll(() => summary.evaluate(node => node.parentElement.open)).toBe(false);
    await expect(summary).not.toBeFocused();
    await summary.focus(); await page.keyboard.press('Enter');
    await element.evaluate((node, next) => {
      node.remove(); document.body.append(node); globalThis.refreshDisclosure(next);
    }, other);
    await expect.poll(() => summary.evaluate(node => node.parentElement.open)).toBe(false);
    expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
    if (kind === 'completed') { await summary.focus(); await page.keyboard.press('Enter'); }
    const action = element.getByRole('button', { name: kind === 'facts' ? 'Complete task' : /^Reopen/ });
    await action.focus(); await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view: other, action: other.actions[0], arguments: [] }]);
  });
}

test('owner refresh does not restore keyboard focus into a transparent encounter', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  await page.evaluate(async view => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    document.body.replaceChildren(); globalThis.calls = [];
    const element = document.createElement('conduit-owner-face'); document.body.append(element);
    element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
  }, view);
  const element = page.locator('conduit-owner-face');
  const action = element.getByRole('button', { name: 'Complete task', exact: true });
  await action.focus();
  await element.evaluate((node, view) => {
    node.disabled = true;
    node.style.opacity = '0';
    node.show({ ...view, face_revision: 2, show_id: 'fixture/transparent-refresh' }, { invoke: async () => {} });
  }, view);
  await expect(action).not.toBeFocused();
  await element.evaluate((node, view) => {
    node.style.opacity = '';
    node.show({ ...view, face_revision: 3, show_id: 'fixture/visible-refresh' }, { invoke: async () => {} });
  }, view);
  // Becoming visible does not revive a discarded focus request.
  await expect(action).not.toBeFocused();
  await action.focus();
  await element.evaluate((node, view) => node.show(view, { invoke: async () => {} }), view);
  await expect(action).toBeFocused();
  expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
});

for (const unavailable of ['disabled', 'inert', 'hidden', 'transparent']) {
  test(`owner forms refuse unavailable argument controls (${unavailable})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const inputView = { ...view, actions: [{ ...view.actions[0], arguments: [{ name: 'text', value_name: 'Text',
      value_kind: 'value/text', maximum_bytes: 32, choices: [] }] }] };
    await page.evaluate(async ({ view, unavailable }) => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      document.body.replaceChildren(); globalThis.calls = [];
      const element = document.createElement('conduit-owner-face'); document.body.append(element);
      element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
      const input = element.shadowRoot.querySelector('input'); input.value = 'unseen argument';
      if (unavailable === 'disabled') input.disabled = true;
      if (unavailable === 'inert') input.inert = true;
      if (unavailable === 'hidden') input.hidden = true;
      if (unavailable === 'transparent') input.style.opacity = '0';
      // Adversarial submission cannot admit an argument the user cannot edit.
      element.shadowRoot.querySelector('form').dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    }, { view: inputView, unavailable });
    expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
    const element = page.locator('conduit-owner-face');
    await element.evaluate(node => {
      const input = node.shadowRoot.querySelector('input');
      input.disabled = false; input.inert = false; input.hidden = false; input.style.opacity = '';
    });
    const input = element.getByRole('textbox', { name: 'Text', exact: true });
    await input.fill('visible argument');
    await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view: inputView, action: inputView.actions[0],
      arguments: [{ name: 'text', value: 'visible argument' }] }]);
  });
}

test('owner forms do not dispatch an argument while text composition is active', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  const inputView = { ...view, actions: [{ ...view.actions[0], arguments: ['first', 'second'].map(name => ({
    name, value_name: name, value_kind: 'value/text', maximum_bytes: 32, choices: [] })) }] };
  await page.evaluate(async view => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    document.body.replaceChildren(); globalThis.calls = [];
    const element = document.createElement('conduit-owner-face'); document.body.append(element);
    element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
  }, inputView);
  const element = page.locator('conduit-owner-face');
  await element.getByRole('textbox', { name: 'first', exact: true }).fill('interim text');
  await element.getByRole('textbox', { name: 'second', exact: true }).fill('other argument');
  await element.evaluate(node => {
    const input = node.shadowRoot.querySelector('input');
    input.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true, data: 'interim text' }));
    // Deliberately adversarial: submitting another control cannot commit an
    // argument whose input-method composition has not finished.
    node.shadowRoot.querySelector('form').dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
  });
  expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
  await element.evaluate(node => node.shadowRoot.querySelector('input')
    .dispatchEvent(new CompositionEvent('compositionend', { bubbles: true, data: '日本語' })));
  const first = element.getByRole('textbox', { name: 'first', exact: true });
  await first.fill('日本語');
  await page.keyboard.press('Enter');
  expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view: inputView, action: inputView.actions[0],
    arguments: [{ name: 'first', value: '日本語' }, { name: 'second', value: 'other argument' }] }]);
});

for (const value of ['\ud800', '\udfff']) {
  test(`owner forms refuse ill-formed Unicode text (${value.charCodeAt(0).toString(16)})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const inputView = { ...view, actions: [{ ...view.actions[0], arguments: [{ name: 'text', value_name: 'Text',
      value_kind: 'value/text', maximum_bytes: 8, choices: [] }] }] };
    await page.evaluate(async ({ view, value }) => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      document.body.replaceChildren(); globalThis.calls = [];
      const element = document.createElement('conduit-owner-face'); document.body.append(element);
      element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
      const input = element.shadowRoot.querySelector('input'); input.value = value;
      input.dispatchEvent(new Event('input', { bubbles: true }));
      element.shadowRoot.querySelector('form').dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    }, { view: inputView, value });
    expect(await page.evaluate(() => globalThis.calls)).toEqual([]);
    const input = page.locator('conduit-owner-face').getByRole('textbox', { name: 'Text', exact: true });
    expect(await input.evaluate(node => node.validationMessage)).toBe('Use valid Unicode text.');
    await input.fill('é 😀'); await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls[0].arguments)).toEqual([{ name: 'text', value: 'é 😀' }]);
  });
}

for (const media of ['normal', 'forced-colors', 'reduced-motion']) {
for (const zoom of [1, 2]) {
for (const form of ['text', 'choice']) {
  test(`owner actions remain within a narrow Unicode layout at CSS magnification ${zoom} (${form}, ${media})`, async ({ page }) => {
    await page.setViewportSize({ width: 320, height: 720 });
    await page.emulateMedia({ forcedColors: media === 'forced-colors' ? 'active' : 'none',
      reducedMotion: media === 'reduced-motion' ? 'reduce' : 'no-preference' });
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const long = '漢字🙂'.repeat(20);
    const choice = 'W'.repeat(200);
    const inputView = { ...view, subjects: [{ ...view.subjects[0], name: long,
      values: [{ name: 'Meaning', text: long }] }], actions: [{ ...view.actions[0], name: long, arguments: [{
        name: 'draft', value_name: long, value_kind: 'value/text', maximum_bytes: 256, choices: form === 'choice' ? [choice] : [],
      }] }] };
    await page.evaluate(async ({ view, zoom, media }) => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      document.body.replaceChildren(); document.documentElement.style.zoom = String(zoom); globalThis.calls = [];
      const element = document.createElement('conduit-owner-face'); document.body.append(element);
      element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
      if (media === 'reduced-motion') {
        const motion = document.createElement('style');
        motion.textContent = '@keyframes fixturePulse{from{opacity:.5}to{opacity:1}}button{animation:fixturePulse 1s infinite;transition:color 2s}';
        element.shadowRoot.append(motion);
      }
    }, { view: inputView, zoom, media });
    await page.waitForFunction(() => document.querySelector('conduit-owner-face').shadowRoot.querySelector('link').sheet);
    const element = page.locator('conduit-owner-face');
    const input = element.getByRole(form === 'choice' ? 'combobox' : 'textbox', { name: long, exact: true });
    const action = element.getByRole('button', { name: long, exact: true });
    const bounds = await action.boundingBox();
    expect(bounds.x).toBeGreaterThanOrEqual(0);
    expect(bounds.x + bounds.width).toBeLessThanOrEqual(321);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1)).toBe(true);
    if (form === 'choice') await input.selectOption(choice); else await input.fill('é 😀');
    await action.focus();
    const style = await action.evaluate(node => { const style = getComputedStyle(node); return {
      outline: style.outlineStyle, outlineWidth: parseFloat(style.outlineWidth), color: style.color,
      background: style.backgroundColor, animation: style.animationName, transition: style.transitionDuration }; });
    expect(style.outline).toBe('solid'); expect(style.outlineWidth).toBeGreaterThanOrEqual(3);
    if (media === 'forced-colors') expect(style.color).not.toBe(style.background);
    if (media === 'reduced-motion') { expect(style.animation).toBe('none'); expect(style.transition).toBe('0s'); }
    await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view: inputView, action: inputView.actions[0],
      arguments: [{ name: 'draft', value: form === 'choice' ? choice : 'é 😀' }] }]);
  });
}
}
}

for (const selected of [' ', 'a\nb', 'a  b']) {
  test(`owner choice labels expose significant whitespace (${JSON.stringify(selected)})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const choices = [' ', 'a\nb', 'a b', 'a  b'];
    const labels = ['"\\u0020"', '"a\\nb"', '"a\\u0020b"', '"a\\u0020\\u0020b"'];
    const inputView = { ...view, actions: [{ ...view.actions[0], arguments: [{ name: 'value', value_name: 'Whitespace choice',
      value_kind: 'value/text', maximum_bytes: 32, choices }] }] };
    await page.evaluate(async view => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      document.body.replaceChildren(); globalThis.calls = [];
      const element = document.createElement('conduit-owner-face'); document.body.append(element);
      element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
    }, inputView);
    const element = page.locator('conduit-owner-face');
    const select = element.getByRole('combobox', { name: 'Whitespace choice', exact: true });
    expect(await select.locator('option').evaluateAll(options => options.slice(1).map(option => option.label))).toEqual(labels);
    for (const label of labels) await expect(select.getByRole('option', { name: label, exact: true })).toHaveCount(1);
    await select.selectOption(selected);
    const button = element.getByRole('button', { name: 'Complete task', exact: true });
    await button.focus(); await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view: inputView, action: inputView.actions[0],
      arguments: [{ name: 'value', value: selected }] }]);
  });
}

test('an unchecked exclusive-choice change cannot dispatch an owner action', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  const choices = {
    ...view,
    subjects: [
      { ...view.subjects[0], identity: 'fixture/group', name: 'Mode',
        values: [], semantic_role: 'conduit.face/choice-group@1', choice_multiplicity: 'exclusive' },
      ...[true, false].map((selected, index) => ({ ...view.subjects[0],
        identity: `fixture/option/${index}`, name: `Mode ${index}`,
        values: [], flags: [{ name: 'selected', value: selected }] })),
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
    const look = element.shadowRoot.querySelector('link');
    if (!look.sheet) await new Promise((resolve, reject) => {
      look.addEventListener('load', resolve, { once: true });
      look.addEventListener('error', () => reject(new Error('owner Look did not load')), { once: true });
    });
    const obsolete = element.shadowRoot.querySelectorAll('input[type=radio]')[1];
    if (!obsolete) throw new Error('fixture did not render an exclusive choice');
    if (obsolete.checked) throw new Error('fixture radio must be unchecked');
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

test('nested choice metadata retains the generic facts instead of becoming a radio option', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  const choices = { ...view, subjects: [
    { ...view.subjects[0], identity: 'fixture/group', name: 'Mode', values: [],
      semantic_role: 'conduit.face/choice-group@1', choice_multiplicity: 'exclusive',
      properties: ['choice-multiplicity: exclusive'] },
    { ...view.subjects[0], identity: 'fixture/nested', name: 'Nested choices', values: [],
      semantic_role: 'conduit.face/choice-group@1', choice_multiplicity: 'exclusive',
      properties: ['choice-multiplicity: exclusive', 'selected: true'],
      flags: [{name: 'selected', value: true}] },
  ], relationships: [{kind: 'Contains', source: 'fixture/group', target: 'fixture/nested'}],
  actions: [{...view.actions[0], identity: 'fixture/select-nested', target: 'fixture/nested'}] };
  await page.evaluate(async view => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    document.body.replaceChildren();
    const element = document.createElement('conduit-owner-face'); document.body.append(element);
    element.show(view, {invoke: async () => {}});
  }, choices);
  const element = page.locator('conduit-owner-face');
  await expect(element.getByRole('radio')).toHaveCount(0);
  await expect(element.getByText('choice-multiplicity: exclusive', {exact: true})).toHaveCount(2);
});

test('admitted typography changes title family independently of palette, density and actions', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  const observations = await page.evaluate(async original => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    const element = document.createElement('conduit-owner-face');
    document.body.replaceChildren(element);
    const component = { maximum_columns: 1, reflow_width: 720, gutter: 12,
      subject_regions: original.subjects.map(() => null), groups: [],
      expression: { expression: { identity: 'fixture/selected-look', version: 2,
        density: 'compact', palette: 'phosphor', typography: 'editorial', text_scale_percent: 100 } } };
    const current = structuredClone(original);
    current.subjects[0].role = 'Body';
    current.component = component;
    let invocations = 0;
    const result = [];
    for (const intent of ['editorial', 'engineering']) {
      component.expression.expression.typography = intent;
      element.show(current, { invoke: async () => { invocations++; } });
      await new Promise(resolve => requestAnimationFrame(resolve));
      const title = element.shadowRoot.querySelector('[data-text-purpose="Title"]');
      const button = element.shadowRoot.querySelector('button');
      result.push({ intent: element.dataset.componentTypography,
        palette: element.dataset.componentPalette, density: element.dataset.componentDensity,
        family: getComputedStyle(title).fontFamily, text: title.textContent,
        action: button.textContent, disabled: button.disabled });
    }
    return { result, invocations };
  }, view);
  expect(observations.invocations).toBe(0);
  expect(observations.result[0].intent).toBe('editorial');
  expect(observations.result[1].intent).toBe('engineering');
  expect(observations.result[0].family).not.toBe(observations.result[1].family);
  for (const key of ['palette', 'density', 'text', 'action', 'disabled']) {
    expect(observations.result[0][key]).toEqual(observations.result[1][key]);
  }
});
