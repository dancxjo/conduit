import { readFile } from "node:fs/promises";
import { expect, test } from '@playwright/test';

async function showNamedBody(page, { disclosure = 'Primary', extra = [], admitted = true,
  name = 'Malo Lefèvre Laurent', palette = 'day' } = {}) {
  const current = structuredClone(view);
  current.subjects.unshift({ identity: 'fixture/body-subject', name, role: 'Body',
    disclosure, text: ['Body description'], properties: [], values: [] });
  current.subjects.push(...extra);
  current.interactions_admitted = admitted;
  const hex = await readFile(`proof/conformance/face-look/field-instrument-${palette}-look-v2.hex`, 'utf8');
  current.component = { subject_regions: current.subjects.map(() => null), groups: [],
    selected_look: { canonical_value: Array.from(Buffer.from(hex.replace(/\s/gu, ''), 'hex')) } };
  await page.goto('/proof/browser/owner-face-look-fixture.html');
  await page.evaluate(async current => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    const element = document.createElement('conduit-owner-face');
    document.body.replaceChildren(element);
    window.namedBody = current;
    window.namedBodyOriginal = JSON.stringify(current);
    window.namedBodyCalls = [];
    element.show(current, { invoke: async ({ action }) => window.namedBodyCalls.push(action.identity) });
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

test('selected Looks use supplied subject names and retain unfamiliar semantic labels', async ({ page }) => {
  for (const palette of ['day', 'night']) {
    const current = structuredClone(view);
    const roles = ['Info', 'Collection', 'Status', 'Item'];
    current.subjects = roles.map(role => ({ identity: `fixture/${role}`, role,
      name: `Supplied ${role} name`, disclosure: 'Primary', text: [], properties: [] }));
    current.subjects.push({ ...current.subjects[0], identity: 'fixture/unknown',
      semantic_role: 'example/unfamiliar@1', name: 'Unfamiliar reading' });
    current.actions[0].target = 'fixture/Item';
    const hex = await readFile(`proof/conformance/face-look/field-instrument-${palette}-look-v2.hex`, 'utf8');
    current.component = { subject_regions: current.subjects.map(() => null), groups: [],
      selected_look: { canonical_value: Array.from(Buffer.from(hex.replace(/\s/gu, ''), 'hex')) } };
    await page.goto('/proof/browser/owner-face-look-fixture.html');
    await page.evaluate(async current => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      const element = document.createElement('conduit-owner-face'); document.body.replaceChildren(element);
      window.subjectView = current; window.subjectOriginal = JSON.stringify(current); window.subjectCalls = [];
      element.show(current, { invoke: async ({ action }) => subjectCalls.push(action.identity) });
    }, current);
    const element = page.locator('conduit-owner-face');
    for (const role of roles) {
      await expect(element.getByRole('heading', { name: `Supplied ${role} name`, exact: true })).toBeVisible();
      await expect(element.locator(`[data-face-role=${role}]>.owner-face-role`).first()).toBeHidden();
    }
    await expect(element.getByText('example/unfamiliar@1', { exact: true })).toBeVisible();
    await element.getByRole('button', { name: 'Complete task', exact: true }).click();
    expect(await page.evaluate(() => subjectCalls)).toEqual(['fixture/complete']);
    expect(await page.evaluate(() => JSON.stringify(subjectView))).toBe(await page.evaluate(() => subjectOriginal));
  }
});

test('presented Body names head application content through both Looks without granting actions', async ({ page }) => {
  for (const palette of ['day', 'night']) {
    const name = '  Malo Lefèvre Laurent <img src=x> 🪴  ';
    const element = await showNamedBody(page, { name, palette, admitted: false });
    const heading = element.locator('[data-face-heading]');
    await expect(heading).toHaveAttribute('data-face-heading', 'fixture/body-subject');
    expect(await heading.textContent()).toBe(name);
    await expect(heading).toBeVisible();
    expect(await heading.locator('img').count()).toBe(0);
    expect(await heading.evaluate(node => getComputedStyle(node).fontSize)).toBe('22.5px');
    await expect(element.getByRole('button', { name: 'Complete task', exact: true })).toBeDisabled();
    expect(await page.evaluate(() => JSON.stringify(namedBody))).toBe(await page.evaluate(() => namedBodyOriginal));
    expect(await page.evaluate(() => namedBodyCalls)).toEqual([]);
  }
  const element = await showNamedBody(page);
  await element.getByRole('button', { name: 'Complete task', exact: true }).click();
  expect(await page.evaluate(() => namedBodyCalls)).toEqual(['fixture/complete']);
});

test('Body heading respects privacy and ambiguity while leaving exact subject facts available', async ({ page }) => {
  for (const disclosure of ['CurrentAction', 'SelectedDetail', 'ExactProvenance']) {
    const element = await showNamedBody(page, { disclosure });
    await expect(element.locator('[data-face-heading]')).toHaveCount(0);
    const exact = element.getByRole('heading', { name: 'Malo Lefèvre Laurent', exact: true });
    await expect(exact).toBeHidden();
    await element.locator('details[data-body-menu]>summary').click();
    await expect(exact).toBeVisible();
  }
  const element = await showNamedBody(page, { extra: [{ identity: 'fixture/second-body',
    name: 'Another Body', role: 'Body', disclosure: 'ExactProvenance', text: [], properties: [] }] });
  await expect(element.locator('[data-face-heading]')).toHaveCount(0);
});

test('presented Body heading wraps on a narrow screen without horizontal overflow', async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 600 });
  const name = 'Notre maison — laboratoire des plantes et des voix partagées';
  const element = await showNamedBody(page, { name, disclosure: 'Context' });
  const heading = element.locator('[data-face-heading]');
  await expect(heading).toBeVisible();
  expect(await heading.textContent()).toBe(name);
  expect(await heading.evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('view-only permission remains distinct from unavailable source actions', async ({ page }) => {
  await page.goto('/proof/browser/owner-face-look-fixture.html');
  for (const [show_state, interactions_admitted] of [['prepared', true], ['available', false], ['available', true]]) {
    const current = structuredClone(view);
    Object.assign(current, { show_state, interactions_admitted });
    current.actions.push({ ...current.actions[0], identity: 'fixture/unavailable',
      name: 'Unavailable action', availability: 'unavailable', explanation: 'Checkpoint storage is unavailable.' });
    await page.evaluate(async current => {
      await import('/targets/browser/handbook/owner-face-component.mjs');
      const element = document.createElement('conduit-owner-face');
      document.body.replaceChildren(element);
      window.readOnlyView = current;
      window.readOnlyOriginal = JSON.stringify(current);
      element.show(current, { invoke: async () => { throw new Error('Caption proof must not dispatch'); } });
    }, current);
    const element = page.locator('conduit-owner-face');
    const available = element.locator('[data-owner-action="fixture/complete"]');
    const enabled = show_state === 'available' && interactions_admitted;
    await expect(available.getByText('View only. Actions are not enabled in this view.', { exact: true })).toHaveCount(enabled ? 0 : 1);
    await expect(available.getByRole('button')).toBeEnabled({ enabled });
    const unavailable = element.locator('[data-owner-action="fixture/unavailable"]');
    await expect(unavailable.getByText('Checkpoint storage is unavailable.', { exact: true })).toHaveCount(1);
    await expect(unavailable.getByRole('button')).toBeDisabled();
    expect(await page.evaluate(() => JSON.stringify(readOnlyView))).toBe(await page.evaluate(() => readOnlyOriginal));
  }
});

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
  await expect(element.locator('form h5')).toHaveCSS('font-size', '20px');
  await expect(element.locator('form h5')).toHaveCSS('line-height', '30px');
  const read = () => element.evaluate(element => Array.from(element.shadowRoot.querySelectorAll('button,input:not([type=radio]),select,textarea,.owner-face-choice'), node => {
    const style = getComputedStyle(node);
    return { opacity: style.opacity, color: style.color, background: style.backgroundColor, border: style.borderTopStyle };
  }));
  for (const forcedColors of ['none', 'active']) {
    await page.emulateMedia({ forcedColors });
    const before = await read();
    if (forcedColors === 'none') {
      for (const style of before) {
        expect(style.color).toBe('rgb(147, 210, 247)');
        expect(style.background).toBe('rgb(9, 13, 22)');
      }
    }
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
  const choice = element.locator('.owner-face-choice');
  await expect(choice).toHaveCSS('min-height', '44px');
  await expect(choice).toHaveCSS('padding-top', '7px');
  await expect(choice).toHaveCSS('padding-left', '10px');
  await expect(choice).toHaveCSS('border-radius', '6px');
  expect(await choice.evaluate(node => parseFloat(getComputedStyle(node, '::before').borderTopWidth))).toBeGreaterThan(1);
  // Ordinary keyboard traversal reaches the hidden native radio, whose visible
  // label carries the selected Look's focus stroke and preserves selection.
  await page.emulateMedia({ forcedColors: 'none' });
  for (const control of [
    element.getByRole('button', { name: 'Complete task', exact: true }),
    element.getByRole('textbox', { name: 'Title', exact: true }),
    element.getByRole('textbox', { name: 'Notes', exact: true }),
    element.getByRole('combobox', { name: 'Priority', exact: true }),
    element.getByRole('button', { name: 'Edit task', exact: true }),
    choice.locator('input'),
  ]) {
    await page.keyboard.press('Tab');
    await expect(control).toBeFocused();
  }
  await expect(choice).toHaveCSS('outline-width', '3px');
  await expect(choice).toHaveCSS('outline-color', 'rgb(244, 196, 0)');
  expect(await page.evaluate(() => JSON.stringify(window.lookView))).toBe(await page.evaluate(() => window.lookOriginal));
  expect(await page.evaluate(() => window.lookInvocations)).toBe(0);
});

// Renderer proof only: the canonical Look is real; this view is a bounded
// appearance fixture, not an admitted Body/Show or an action receipt.
test('selected Look measures title and heading from body size and leading', async ({ page }) => {
  const wire = Buffer.from((await readFile('proof/conformance/face-look/phosphor-look-v2.hex', 'utf8')).replace(/\s/gu, ''), 'hex');
  const current = structuredClone(view);
  current.subjects.unshift({ ...current.subjects[0], identity: 'fixture/body-title',
    role: 'Body', name: 'Malo Lefèvre Laurent — environmental instrument', values: [] });
  current.component = { maximum_columns: 1, reflow_width: 720, gutter: 12,
    subject_regions: current.subjects.map(() => null), groups: [], fallback: null,
    selected_look: { canonical_value: Array.from(wire) },
    expression: { expression: { identity: 'conduit.look/phosphor@1', density: 'compact',
      palette: 'phosphor', typography: 'editorial', version: 2, text_scale_percent: 100 } } };
  await page.setViewportSize({ width: 360, height: 720 });
  await page.goto('/proof/browser/owner-face-look-fixture.html');
  await page.evaluate(async current => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    const element = document.createElement('conduit-owner-face');
    document.body.replaceChildren(element);
    window.lookView = current;
    window.lookInvocations = 0;
    element.show(current, { invoke: async () => { window.lookInvocations += 1; } });
  }, current);
  const element = page.locator('conduit-owner-face');
  const title = element.locator('[data-text-purpose=Title]');
  const heading = element.locator('.owner-face-subject [data-text-purpose=Heading]');
  const bodyHeading = element.locator('[data-face-heading]');
  const action = element.getByRole('button', { name: 'Complete task', exact: true });
  // Canonical Phosphor declares 16px body and 150% leading. Shared text purposes
  // use 1.5 body for Title and 1.25 body for Heading. Browser text preferences
  // remain a floor; root-relative sizing and the admitted user scale compose.
  for (const root of [16, 20]) {
    await page.evaluate(root => { document.documentElement.style.fontSize = `${root}px`; }, root);
    for (const scale of [100, 125, 200]) {
      await element.evaluate((element, { current, scale }) => {
        const next = structuredClone(current);
        next.component.expression.expression.text_scale_percent = scale;
        window.lookView = next;
        element.show(next, { invoke: async () => { window.lookInvocations += 1; } });
      }, { current, scale });
      const body = root * scale / 100;
      await expect(title).toHaveCSS('font-size', `${body * 1.5}px`);
      await expect(heading).toHaveCSS('font-size', `${body * 1.25}px`);
      await expect(bodyHeading).toHaveCSS('font-size', `${body * 1.25}px`);
      await expect(title).toHaveCSS('line-height', `${body * 1.5 * 1.5}px`);
      await expect(heading).toHaveCSS('line-height', `${body * 1.25 * 1.5}px`);
      await expect(action).toHaveCSS('font-size', `${body}px`);
      await expect(action).toBeEnabled();
      // Wrapping grows the real block; it must not clip a long Unicode name.
      expect(await title.evaluate(node => node.scrollHeight <= node.clientHeight)).toBe(true);
      expect(await title.evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
      expect(await page.evaluate(() => window.lookView.subjects.map(subject => subject.identity)))
        .toEqual(current.subjects.map(subject => subject.identity));
      expect(await page.evaluate(() => window.lookView.actions)).toEqual(current.actions);
    }
  }
  expect(await page.evaluate(() => window.lookInvocations)).toBe(0);
});

// Canonical Look and production DOM components; this bounded renderer view is
// not a joined Body, source-action execution or acknowledged Show.
for (const { name, panel, radius, body, small, palette, typography } of [
  { name: 'phosphor', panel: 13, radius: 9, body: 16, small: 14, palette: 'phosphor', typography: 'editorial' },
  { name: 'field-instrument-day', panel: 16, radius: 0, body: 18, small: 16, palette: 'editorial', typography: 'engineering' },
  { name: 'field-instrument-night', panel: 16, radius: 0, body: 18, small: 16, palette: 'phosphor', typography: 'engineering' },
]) test(`selected panel geometry reaches fieldgroups and disclosures: ${name}`, async ({ page }) => {
  const wire = Buffer.from((await readFile(`proof/conformance/face-look/${name}-look-v2.hex`, 'utf8')).replace(/\s/gu, ''), 'hex');
  const current = structuredClone(view);
  current.subjects[0].properties = ['Retained temperature record'];
  current.subjects[0].text = ['Information uses ordinary Body wording.'];
  current.subjects.push({ identity: 'fixture/copy-status', name: 'Observation status', role: 'Status',
    disclosure: 'Primary', text: ['Status uses the same Body size.'], properties: [], values: [], flags: [], unmodeled_property_count: 0 });
  current.subjects.push({ identity: 'fixture/unknown-role', name: 'Other information', role: 'Semantic',
    semantic_role: 'example/unknown-info@1', disclosure: 'Primary', text: [], properties: [], values: [], flags: [], unmodeled_property_count: 0 });
  current.actions.push({ ...current.actions[0], identity: 'fixture/unavailable', name: 'Unavailable task',
    availability: 'unavailable', explanation: 'The producer is unavailable.' });
  current.subjects.push(
    { identity: 'fixture/panel-group', name: 'Shared environmental controls', role: 'Collection', disclosure: 'Primary', text: [], values: [], flags: [], properties: [], unmodeled_property_count: 0,
      semantic_role: 'conduit.face/choice-group@1', choice_multiplicity: 'exclusive' },
    { identity: 'fixture/panel-option', name: 'Room', role: 'Info', disclosure: 'Primary', text: [], values: [], properties: [], flags: [{ name: 'selected', value: true }], unmodeled_property_count: 0 },
    { identity: 'fixture/panel-outside', name: 'Outside', role: 'Info', disclosure: 'Primary', text: [], values: [], properties: [], flags: [{ name: 'selected', value: false }], unmodeled_property_count: 0 },
  );
  current.relationships.push({ kind: 'Contains', source: 'fixture/panel-group', target: 'fixture/panel-option' });
  current.actions.push({ ...current.actions[0], identity: 'fixture/panel-choice', target: 'fixture/panel-option', name: 'Choose Room' });
  current.relationships.push({ kind: 'Contains', source: 'fixture/panel-group', target: 'fixture/panel-outside' });
  current.actions.push({ ...current.actions[0], identity: 'fixture/panel-choice-outside', target: 'fixture/panel-outside', name: 'Choose Outside' });
  current.component = { maximum_columns: 1, reflow_width: 720, gutter: 12,
    subject_regions: current.subjects.map(() => null), groups: [], fallback: null,
    selected_look: { canonical_value: Array.from(wire) },
    expression: { expression: { identity: `conduit.look/${name}@1`, density: 'compact', palette, typography, version: 2, text_scale_percent: 100 } } };
  await page.setViewportSize({ width: 360, height: 720 });
  await page.goto('/proof/browser/owner-face-look-fixture.html');
  await page.evaluate(async current => {
    await import('/targets/browser/handbook/owner-face-component.mjs');
    document.body.replaceChildren(document.createElement('conduit-owner-face'));
    window.panelView = current;
    window.panelOriginal = JSON.stringify(current);
    window.panelInvocations = 0;
  }, current);
  const element = page.locator('conduit-owner-face');
  for (const scale of [100, 200]) {
    await element.evaluate((element, scale) => {
      window.panelView.component.expression.expression.text_scale_percent = scale;
      element.show(window.panelView, { invoke: async () => { window.panelInvocations += 1; } });
    }, scale);
    const group = element.locator('.owner-face-choice-group');
    const facts = element.locator('.owner-face-subject details').first();
    for (const container of [group, facts]) {
      await expect(container).toHaveCSS('padding-top', `${panel}px`);
      await expect(container).toHaveCSS('padding-left', `${panel}px`);
      await expect(container).toHaveCSS('border-radius', `${radius}px`);
    }
    await expect(group.locator('legend')).toHaveCSS('font-size', `${body * 1.25 * scale / 100}px`);
    await expect(group.locator('legend')).toHaveCSS('line-height', `${body * 1.25 * 1.5 * scale / 100}px`);
    await expect(element.locator('.owner-face-choices')).toHaveCSS('column-gap', '4px');
    // Known readout chrome does not repeat the technical Info label. Unfamiliar
    // semantic roles retain their exact fallback label and source wording.
    await expect(element.locator('[data-face-role=Info]>.owner-face-role').first()).toBeHidden();
    await expect(element.getByRole('heading', { name: 'Read the handbook', exact: true })).toBeVisible();
    await expect(element.getByText('example/unknown-info@1', { exact: true })).toBeVisible();
    const value = element.locator('.owner-face-value').first();
    await expect(value).toHaveCSS('font-size', `${body * 2 * scale / 100}px`);
    await expect(value).toHaveCSS('line-height', `${body * 2 * 1.5 * scale / 100}px`);
    const valueLabel = element.locator('dt').first();
    await expect(valueLabel).toHaveCSS('font-size', `${small * scale / 100}px`);
    await expect(valueLabel).toHaveCSS('line-height', `${small * 1.5 * scale / 100}px`);
    for (const wording of [
      element.getByText('Information uses ordinary Body wording.', { exact: true }),
      element.getByText('Status uses the same Body size.', { exact: true }),
    ]) {
      await expect(wording).toHaveCSS('font-size', `${body * scale / 100}px`);
      await expect(wording).toHaveCSS('line-height', `${body * 1.5 * scale / 100}px`);
      await expect(wording).toHaveCSS('letter-spacing', 'normal');
    }
    const unavailable = element.getByRole('button', { name: 'Unavailable task', exact: true });
    await expect(unavailable).toBeDisabled();
    const explanation = element.getByText('The producer is unavailable.', { exact: true });
    await expect(explanation).toHaveCSS('font-size', `${small * scale / 100}px`);
    await expect(explanation).toHaveCSS('line-height', `${small * 1.5 * scale / 100}px`);
    await expect(group.locator('input').nth(0)).toBeChecked();
    await expect(group.locator('input').nth(1)).not.toBeChecked();
    const summary = facts.locator('summary');
    // Local disclosure remains ordinary browser input and does not submit an action.
    if (scale === 100) await summary.click();
    await expect(facts).toHaveAttribute('open', '');
    await expect(facts).toContainText('Retained temperature record');
    for (const forcedColors of ['none', 'active']) {
      await page.emulateMedia({ forcedColors });
      await expect(group).toHaveCSS('padding-top', `${panel}px`);
      await expect(facts).toHaveCSS('border-radius', `${radius}px`);
      expect(await group.evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
      expect(await summary.evaluate(node => node.scrollWidth <= node.clientWidth)).toBe(true);
    }
    // The only changed view field is the admitted local text-scale input above.
    expect(await page.evaluate(() => {
      const actual = structuredClone(window.panelView);
      actual.component.expression.expression.text_scale_percent = 100;
      return JSON.stringify(actual);
    })).toBe(await page.evaluate(() => window.panelOriginal));
    expect(await page.evaluate(() => window.panelInvocations)).toBe(0);
  }
});
