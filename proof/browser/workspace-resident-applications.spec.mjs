import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./static-product-server.mjs";
import { BROWSER_COLD_PREPARATION_MS } from "./browser-preparation.mjs";

let entrance;
const current = page => page.evaluate(() => globalThis.__conduitWorkspace.current());
const visiblePlotOutput = page => page.locator("[data-plot-output] > output:not([hidden])");

test.beforeEach(async ({ page }) => {
  entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/");
  await page.setViewportSize({ width: 1280, height: 1000 });
});
test.afterEach(() => entrance?.child.kill());

test("the first body runs and can remove its resident Tutorial Plot without losing its face", async ({ page }) => {
  // Cold Birth/Wake and shelf preparation consume their existing finite budget;
  // retain the ordinary controls and all removal/identity assertions.
  test.setTimeout(20_000 + BROWSER_COLD_PREPARATION_MS);
  await page.goto(entrance.url);
  await expect(page.getByRole("checkbox", { name: "Tutorial", exact: true })).toBeChecked();
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  const tutorial = page.locator("[data-body-tutorial]");
  await expect(tutorial).toContainText("Wake this body");
  const born = await current(page);
  await expect(page.getByRole("navigation", { name: "Your plots" }).getByRole("button", { name: "Tutorial", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const resident = tutorial.locator("[data-body-tutorial-resident] > output");
  await expect(resident).toContainText("Try a Plot, inspect what happened, then try again.");
  await expect(resident).toHaveAttribute("data-active-play-id", (await current(page)).active_play_id);
  await expect(tutorial.locator("[data-body-tutorial-core]")).toBeHidden();
  await page.getByRole("button", { name: "+ Plots", exact: true }).click();
  const card = page.locator('[data-application-key^="library-plot-"]').filter({ hasText: /^Tutorial/u });
  await card.getByRole("button", { name: "Remove", exact: true }).click();
  await expect(card).toContainText("Not in your body");
  await page.getByRole("button", { name: "back to the surface", exact: true }).click();
  await expect(tutorial).toBeHidden();
  await expect(page.locator("[data-workspace-surface]")).toBeVisible();
  expect((await current(page)).initial_plots).toHaveLength(born.initial_plots.length - 1);
});

test("resident Patchbay inspects and begins an exact edit in one browser Body Play", async ({ page }, testInfo) => {
  await page.goto(entrance.url);
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  await page.getByRole("checkbox", { name: "Patchbay", exact: true }).check();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const born = await current(page);
  await page.locator("[data-checked-plot-id]").filter({ hasText: "Patchbay" }).click();
  const patchbay = visiblePlotOutput(page);
  await expect(patchbay).toContainText("Active plots on this body");
  const component = patchbay.locator('conduit-owner-face');
  await expect(component).toHaveAttribute('aria-busy', 'false');
  const menu = component.locator('details[data-body-menu]');
  await menu.locator(':scope > summary').click();
  const lantern = page.getByRole('navigation', { name: 'Your plots' })
    .getByRole('button', { name: 'Memory Lantern', exact: true });
  const checkedPlotId = await lantern.getAttribute('data-checked-plot-id');
  expect(checkedPlotId).toMatch(/^[0-9a-f]{64}$/);
  const inspect = menu.getByRole('button', { name: 'Inspect Memory Lantern', exact: true });
  await expect(inspect).toBeEnabled();
  expect(JSON.parse(await inspect.getAttribute('data-owner-focus')))
    .toEqual([born.body_id, expect.stringMatching(/^patchbay\.plot\.\d+$/), `plot/${checkedPlotId}`, 'submit']);
  const initialFace = await component.getAttribute('data-face-id');
  await inspect.click();
  await expect(component).not.toHaveAttribute('data-face-id', initialFace);
  await expect(component).toHaveAttribute('aria-busy', 'false');
  const edit = menu.locator('form[data-owner-action="patchbay.edit.current"]');
  const subject = edit.getByRole('combobox', { name: 'Subject to edit', exact: true });
  await expect(subject).toBeEnabled();
  const subjectId = await subject.locator('option').evaluateAll(options =>
    options.find(option => option.value && !option.disabled)?.value);
  expect(subjectId).toBeTruthy();
  await subject.selectOption(subjectId);
  const submit = edit.getByRole('button', { name: 'Edit a subject', exact: true });
  expect(JSON.parse(await submit.getAttribute('data-owner-focus')))
    .toEqual([born.body_id, 'patchbay.edit.current', `plot/${checkedPlotId}`, 'submit']);
  await expect(submit).toBeEnabled();
  await submit.click();
  await expect(component).toContainText('Edit requested against the exact inspected subject; no authority was inferred.');
  await page.screenshot({ path: testInfo.outputPath('resident-patchbay-exact-edit.png'), fullPage: true });
  expect((await current(page)).body_id).toBe(born.body_id);
  expect((await current(page)).active_play_id).toBe(born.active_play_id);
});
