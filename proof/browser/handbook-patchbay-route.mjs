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
  await button.click();
}
