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
