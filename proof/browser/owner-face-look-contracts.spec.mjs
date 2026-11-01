import { readFile } from "node:fs/promises";
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

test('canonical selected Look yields its colors to forced system colors', async ({ page }) => {
  const wire = Buffer.from((await readFile('proof/conformance/face-look/phosphor-look-v2.hex', 'utf8')).replace(/\s/gu, ''), 'hex');
  const configured = { ...view, component: {
    maximum_columns: 1, reflow_width: 720, gutter: 12, subject_regions: [0], groups: [0], fallback: null,
    selected_look: { canonical_value: Array.from(wire) },
    expression: { expression: { identity: 'conduit.look/phosphor@1', density: 'compact', palette: 'editorial', typography: 'editorial', version: 2, text_scale_percent: 100 } },
  } };
  let releaseStylesheet, stylesheetRequested;
  const stylesheetGate = new Promise(resolve => { releaseStylesheet = resolve; });
  const requested = new Promise(resolve => { stylesheetRequested = resolve; });
  await page.route('**/owner-face-look.css', async route => {
    stylesheetRequested();
    await stylesheetGate;
    await route.continue();
  });
  await page.emulateMedia({ forcedColors: 'active' });
  await page.goto('/proof/browser/owner-face-look-fixture.html');
  await page.evaluate(async current => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    const element = document.createElement('conduit-owner-face');
    document.body.replaceChildren(element);
    element.show(current, { invoke: async () => {} });
  }, configured);
  const element = page.locator('conduit-owner-face');
  await expect(element).toHaveAttribute('data-component-look', 'conduit.look/phosphor@1');
  const readTokens = () => element.evaluate(element => {
    const style = getComputedStyle(element);
    return Object.fromEntries(['paper', 'card', 'ink', 'muted', 'line', 'accent', 'control', 'focus']
      .map(key => [key, style.getPropertyValue(`--face-${key}`).trim()]));
  });
  await requested;
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
  // The selected Look is already admitted, but its stylesheet is still pending.
  expect(Object.values(await readTokens())).toEqual(Array(8).fill(''));
  releaseStylesheet();
  await expect.poll(readTokens).toEqual({ paper: 'Canvas', card: 'Canvas', ink: 'CanvasText', muted: 'CanvasText',
    line: 'CanvasText', accent: 'LinkText', control: 'ButtonText', focus: 'Highlight' });
  await expect(element.getByRole('button', { name: 'Complete task', exact: true })).toBeEnabled();
});

// Typography changes must preserve the independently admitted palette and shape.
test('typography preserves admitted paper ink and subject radius', async ({ page }) => {
  await page.goto('/proof/browser/owner-face-look-fixture.html');
  await page.evaluate(async () => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    document.body.replaceChildren(document.createElement('conduit-owner-face'));
  });
  const element = page.locator('conduit-owner-face');
  const showTypography = intent => element.evaluate((element, { view, intent }) => {
    const current = structuredClone(view);
    current.subjects[0].role = 'Body';
    current.component = { maximum_columns: 1, reflow_width: 720, gutter: 12,
      subject_regions: current.subjects.map(() => null), groups: [],
      expression: { expression: { identity: 'fixture/selected-look', version: 2,
        density: 'compact', palette: 'phosphor', typography: intent, text_scale_percent: 100 } } };
    element.show(current, { invoke: async () => {} });
  }, { view, intent });
  const readAppearance = () => element.evaluate(element => {
    const title = element.shadowRoot.querySelector('[data-text-purpose="Title"]');
    const button = element.shadowRoot.querySelector('button');
    return { family: getComputedStyle(title).fontFamily,
      paper: getComputedStyle(element.shadowRoot.querySelector('.owner-face-document')).backgroundColor,
      ink: getComputedStyle(title).color,
      radius: getComputedStyle(element.shadowRoot.querySelector('.owner-face-subject')).borderRadius,
      text: title.textContent, action: button.textContent, disabled: button.disabled,
      palette: element.dataset.componentPalette, density: element.dataset.componentDensity };
  });
  await showTypography('editorial');
  // Poll actual applied styling; an animation frame alone is not CSS readiness.
  await expect.poll(async () => (await readAppearance()).family).toContain('Georgia');
  const editorial = await readAppearance();
  await showTypography('engineering');
  await expect.poll(async () => (await readAppearance()).family).not.toBe(editorial.family);
  const engineering = await readAppearance();
  for (const key of ['paper', 'ink', 'radius', 'text', 'action', 'disabled', 'palette', 'density']) {
    expect(engineering[key], key).toEqual(editorial[key]);
  }
});

// Appearance-only: real component/native controls, no Body/action receipt.
test('disabled controls retain selected Look ink without fading', async ({ page }) => {
  const wire = Buffer.from((await readFile('proof/conformance/face-look/phosphor-look-v2.hex', 'utf8')).replace(/\s/gu, ''), 'hex');
  const current = structuredClone(view);
  current.actions.push({ ...current.actions[0], identity: 'fixture/edit', name: 'Edit task', arguments: [
    { name: 'title', value_name: 'Title', value_kind: 'value/text', maximum_bytes: 64, choices: [] },
    { name: 'notes', value_name: 'Notes', value_kind: 'value/text', maximum_bytes: 64, choices: [], current_value: 'Recorded notes' },
    { name: 'priority', value_name: 'Priority', value_kind: 'value/text', maximum_bytes: 64, choices: ['Normal', 'High'] },
  ] });
  current.subjects.push(
    { identity: 'fixture/group', name: 'View', role: 'Collection', disclosure: 'Primary', text: [], values: [], flags: [], unmodeled_property_count: 0,
      semantic_role: 'conduit.face/choice-group@1', choice_multiplicity: 'exclusive' },
    { identity: 'fixture/option', name: 'Room', role: 'Info', disclosure: 'Primary', text: [], values: [], flags: [{ name: 'selected', value: true }], unmodeled_property_count: 0 },
  );
  current.relationships.push({ kind: 'Contains', source: 'fixture/group', target: 'fixture/option' });
  current.actions.push({ ...current.actions[0], identity: 'fixture/choose', target: 'fixture/option', name: 'Choose Room' });
  current.component = { maximum_columns: 1, reflow_width: 720, gutter: 12, subject_regions: current.subjects.map(() => null), groups: [], fallback: null,
    selected_look: { canonical_value: Array.from(wire) },
    expression: { expression: { identity: 'conduit.look/phosphor@1', density: 'compact', palette: 'phosphor', typography: 'editorial', version: 2, text_scale_percent: 100 } } };
  await page.goto('/proof/browser/owner-face-look-fixture.html');
  await page.evaluate(async current => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    const element = document.createElement('conduit-owner-face');
    document.body.replaceChildren(element);
    window.lookView = current;
    window.lookOriginal = JSON.stringify(current);
    window.lookInvocations = 0;
    element.show(current, { invoke: async () => { window.lookInvocations += 1; } });
  }, current);
  const element = page.locator('conduit-owner-face');
  await expect(element.locator('.owner-face-document')).toHaveCSS('padding-top', '13px');
  await expect(element.getByRole('button', { name: 'Complete task', exact: true })).toBeEnabled();
  await expect(element.getByRole('textbox', { name: 'Title', exact: true })).toBeVisible();
  await expect(element.getByRole('textbox', { name: 'Notes', exact: true })).toBeVisible();
  await expect(element.getByRole('combobox', { name: 'Priority', exact: true })).toBeVisible();
  await expect(element.locator('.owner-face-choice')).toHaveCount(1);
  const read = () => element.evaluate(element => Array.from(element.shadowRoot.querySelectorAll('button,input:not([type=radio]),select,textarea,.owner-face-choice'), node => {
    const style = getComputedStyle(node);
    return { opacity: style.opacity, color: style.color, background: style.backgroundColor, border: style.borderTopStyle };
  }));
  for (const forcedColors of ['none', 'active']) {
    await page.emulateMedia({ forcedColors });
    const before = await read();
    await element.evaluate(element => { element.disabled = true; });
    await expect(element.getByRole('button', { name: 'Complete task', exact: true })).toBeDisabled();
    await expect(element.locator('.owner-face-choice input')).toHaveCSS('opacity', '0');
    await expect(element.locator('.owner-face-choice input')).toBeChecked();
    const disabled = await read();
    expect(disabled.length).toBeGreaterThanOrEqual(5);
    for (let i = 0; i < disabled.length; i += 1) {
      expect(disabled[i].opacity).toBe('1');
      expect(disabled[i].border).toBe('dashed');
      // Forced system colors remain the platform's choice, including disabled ink.
      if (forcedColors === 'none') {
        expect(disabled[i].color).toBe(before[i].color);
        expect(disabled[i].background).toBe(before[i].background);
      }
    }
    await element.evaluate(element => { element.disabled = false; });
    await expect(element.getByRole('button', { name: 'Complete task', exact: true })).toBeEnabled();
  }
  expect(await page.evaluate(() => JSON.stringify(window.lookView))).toBe(await page.evaluate(() => window.lookOriginal));
  expect(await page.evaluate(() => window.lookInvocations)).toBe(0);
});
