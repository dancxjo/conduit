import { expect, test } from '@playwright/test';

// Renderer fixture only: the callback is not a Body/action receipt.
const view = {
  body_id: 'fixture/body', face_id: 'fixture/face', face_revision: 1, show_id: 'fixture/show',
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

for (const unavailable of ['disconnected', 'hidden']) {
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
    const obsolete = element.shadowRoot.querySelector('input[type=radio]');
    if (!obsolete) throw new Error('fixture did not render an exclusive choice');
    if (unavailable === 'disconnected') {
      element.remove();
      document.body.append(element);
      element.disabled = false;
    } else obsolete.hidden = true;
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

for (const hidden of ['attribute', 'visibility', 'disclosure', 'fieldset']) {
  test(`hidden owner actions refuse dispatch (${hidden})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    await page.evaluate(async ({ view, hidden }) => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      document.body.replaceChildren();
      const element = document.createElement('conduit-owner-face');
      document.body.append(element);
      globalThis.calls = [];
      element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
      const form = element.shadowRoot.querySelector('form');
      if (hidden === 'attribute') form.hidden = true;
      if (hidden === 'visibility') form.style.visibility = 'hidden';
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
    await element.evaluate(node => {
      const form = node.shadowRoot.querySelector('form');
      form.hidden = false; form.style.visibility = '';
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
for (const interruption of [null, 'prepare', 'acknowledge']) {
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
      prepareOwnerFaceMask: async () => {
        revision++;
        await pause('prepare');
        return { ...shown, face_revision: String(revision), show_id: `fixture/show/${revision}`,
          show_state: 'prepared', interactions_admitted: false };
      },
      acknowledgeOwnerFaceMask: async prepared => {
        await pause('acknowledge');
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
  await page.getByRole('button', { name: 'Refresh this Face', exact: true }).click();
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

for (const malformed of ['missing', 'non-array', 'negative-bound', 'choice-bytes', 'too-many']) {
  test(`malformed owner argument contracts refuse without losing other actions (${malformed})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const broken = { ...view.actions[0], identity: 'fixture/broken', name: 'Broken action', arguments: [{
      name: 'value', value_name: 'Value', value_kind: 'value/text', maximum_bytes: 4, choices: [],
    }] };
    if (malformed === 'missing') delete broken.arguments;
    if (malformed === 'non-array') broken.arguments = {};
    if (malformed === 'too-many') broken.arguments = Array.from({ length: 65 }, (_, index) => ({
      name: `value/${index}`, value_name: `Value ${index}`, value_kind: 'value/text', maximum_bytes: 4, choices: [],
    }));
    if (malformed === 'negative-bound') broken.arguments[0].maximum_bytes = -1;
    if (malformed === 'choice-bytes') broken.arguments[0].choices = ['ééé'];
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

for (const enriched of ['collection', 'item', 'status']) {
  test(`collection layout preserves additional ${enriched} semantics`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const subject = (identity, name, role) => ({ identity, name, role, disclosure: 'Primary', text: [], properties: [], values: [] });
    const collection = subject('fixture/list', 'Tasks', 'Collection');
    const item = { ...subject('fixture/item', 'Read', 'Item'), flags: [{ name: 'complete', value: false }] };
    const status = { ...subject('fixture/status', 'Progress', 'Status'), text: ['1 thing left · 0 completed'] };
    const extra = { collection, item, status }[enriched];
    extra.properties = ['Important authored fact'];
    extra.values = [{ name: 'Priority', text: 'Urgent' }];
    if (enriched !== 'status') extra.text = ['Additional explanation'];
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
    await expect(element.getByRole('term')).toHaveText('Priority');
    await expect(element.getByRole('definition')).toHaveText('Urgent');
    if (enriched !== 'status') await expect(element.getByText('Additional explanation', { exact: true })).toBeVisible();
    await element.locator('[part=subject]').filter({ hasText: 'Important authored fact' })
      .getByText('Exact facts', { exact: true }).click();
    await expect(element.getByText('Important authored fact', { exact: true })).toBeVisible();
    const action = element.getByRole('button', { name: /^Complete task/ });
    await action.focus(); await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view: currentView,
      action: currentView.actions[0], arguments: [] }]);
  });
}

for (const enriched of ['group', 'option']) {
  test(`exclusive-choice layout preserves additional ${enriched} semantics`, async ({ page }) => {
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
    extra.values = [{ name: 'Authored value', text: 'Exact meaning' }];
    extra.properties = ['Important authored fact'];
    extra.text = ['Additional explanation'];
    await page.evaluate(async view => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      document.body.replaceChildren(); globalThis.calls = [];
      const element = document.createElement('conduit-owner-face'); document.body.append(element);
      element.show(view, { invoke: async call => { globalThis.calls.push(call); } });
    }, choices);
    const element = page.locator('conduit-owner-face');
    await expect(element.getByRole('term')).toHaveText('Authored value');
    await expect(element.getByRole('definition')).toHaveText('Exact meaning');
    await expect(element.getByText('Additional explanation', { exact: true })).toBeVisible();
    await element.locator('[part=subject]').filter({ hasText: 'Important authored fact' })
      .getByText('Exact facts', { exact: true }).click();
    await expect(element.getByText('Important authored fact', { exact: true })).toBeVisible();
    const selectedFacts = element.locator('[part=subject]').filter({ hasText: 'selected: true' });
    await selectedFacts.getByText('Exact facts', { exact: true }).click();
    await expect(selectedFacts.getByText('selected: true', { exact: true })).toBeVisible();
    const action = element.getByRole('button', { name: 'Select 1', exact: true });
    await action.focus(); await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.calls)).toEqual([{ view: choices,
      action: choices.actions[1], arguments: [] }]);
  });
}
