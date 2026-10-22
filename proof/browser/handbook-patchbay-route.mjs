import { expect } from '@playwright/test';

// Follow the actual canonical Face disclosure and typed action, without a
// parallel selection API or a forced click on a hidden control.
export async function inspectHandbookPlot(page, checkedPlotId, bodyId) {
  const component = page.locator('.handbook-show conduit-owner-face:visible');
  await expect(component).toHaveCount(1);
  await expect(component).toHaveAttribute('aria-busy', 'false');
  await expect(component).toHaveAttribute('data-face-id', /\S+/);
  await expect(component).toHaveAttribute('data-show-id', /\S+/);
  const menu = component.locator('details[data-body-menu]');
  await expect(menu).toHaveCount(1);
  const summary = menu.locator(':scope > summary');
  await expect(summary).toHaveText('Body');
  if (!(await menu.evaluate(node => node.open))) await summary.click();
  await expect(menu).toHaveAttribute('open', '');
  const target = `plot/${checkedPlotId}`;
  const button = menu.locator('form[data-owner-action^="patchbay.plot."]')
    .locator(`button[data-owner-focus*=${JSON.stringify(JSON.stringify(target))}]`);
  await expect(button).toHaveCount(1);
  await expect(button).toHaveAccessibleName(/^Inspect /);
  await expect(button).toBeEnabled();
  const focus = JSON.parse(await button.getAttribute('data-owner-focus'));
  const action = await button.locator('..').getAttribute('data-owner-action');
  expect(action).toMatch(/^patchbay\.plot\.\d+$/);
  expect(focus).toEqual([bodyId, action, target, 'submit']);
  const previousFace = await component.getAttribute('data-face-id');
  await button.click();
  await expect(component).not.toHaveAttribute('data-face-id', previousFace);
  await expect(component).toHaveAttribute('aria-busy', 'false');
  await expect(component.locator('..').locator('[data-patchbay-status]')).toHaveText('');
}

// Inspect the same source-derived Face through its native disclosures. These
// keys are the renderer's retained local focus basis, not a second graph model.
export async function assertHandbookInspection(page, state, { requireCord = true } = {}) {
  const component = page.locator('.handbook-show conduit-owner-face:visible');
  await expect(component).toHaveAttribute('aria-busy', 'false');
  const edit = component.locator('form[data-owner-action="patchbay.edit.current"] button');
  await expect(edit).toBeEnabled();
  const focus = JSON.parse(await edit.getAttribute('data-owner-focus'));
  expect(focus).toEqual([state.bodyId, 'patchbay.edit.current', `plot/${state.selectedPlot}`, 'submit']);
  const inspection = component.locator('details[data-owner-disclosure]').filter({
    has: page.locator(':scope > summary', { hasText: 'Inspect other Face subjects' }),
  });
  await expect(inspection).toHaveCount(1);
  if (!(await inspection.evaluate(node => node.open))) await inspection.locator(':scope > summary').click();
  const facts = inspection.locator('details[data-owner-disclosure]');
  const keys = await facts.evaluateAll(nodes => nodes.map(node => JSON.parse(node.dataset.ownerDisclosure)[2]));
  expect(keys).toContain(`plan/${state.planId}`);
  expect(keys).toContain(`play/${state.playId}`);
  expect(keys.some(key => key?.startsWith('gear/'))).toBe(true);
  if (requireCord) expect(keys.some(key => key?.startsWith('cord/'))).toBe(true);
  const portKey = keys.find(key => key?.startsWith('port/') && key.includes('/output/'));
  expect(portKey).toBeTruthy();
  const port = facts.nth(keys.indexOf(portKey));
  const summary = port.locator(':scope > summary');
  if (await port.evaluate(node => node.open)) await summary.click();
  await summary.focus();
  await summary.press('Enter');
  await expect(port).toHaveAttribute('open', '');
  await expect(summary).toBeFocused();
  await expect(port).toContainText('direction: outgoing');
  return component;
}
