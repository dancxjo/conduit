import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";

let entrance;
const current = page => page.evaluate(() => globalThis.__conduitWorkspace.current());
const visibleFormOutput = page => page.locator("[data-form-output] > output:not([hidden])");

test.beforeEach(async ({ page }) => {
  entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/");
  await page.setViewportSize({ width: 1280, height: 1000 });
});
test.afterEach(() => entrance?.child.kill());

test("the first body runs and can remove its resident Tutorial Form without losing its face", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.getByRole("checkbox", { name: "Tutorial", exact: true })).toBeChecked();
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  const tutorial = page.locator("[data-body-tutorial]");
  await expect(tutorial).toContainText("Wake this body");
  const born = await current(page);
  await expect(page.getByRole("navigation", { name: "Your forms" }).getByRole("button", { name: "Tutorial", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const resident = tutorial.locator("[data-body-tutorial-resident] > output");
  await expect(resident).toContainText("finite Body may remain awake");
  await expect(resident).toHaveAttribute("data-active-play-id", (await current(page)).active_play_id);
  await expect(tutorial.locator("[data-body-tutorial-core]")).toBeHidden();
  await page.getByRole("button", { name: "+ Forms", exact: true }).click();
  const card = page.locator('[data-application-key^="library-form-"]').filter({ hasText: /^Tutorial/u });
  await card.getByRole("button", { name: "Remove", exact: true }).click();
  await expect(card).toContainText("Not in your body");
  await page.getByRole("button", { name: "back to the surface", exact: true }).click();
  await expect(tutorial).toBeHidden();
  await expect(page.locator("[data-workspace-surface]")).toBeVisible();
  expect((await current(page)).initial_forms).toHaveLength(born.initial_forms.length - 1);
});

test("resident Patchbay inspects and begins an exact edit in one browser Body Play", async ({ page }, testInfo) => {
  await page.goto(entrance.url);
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  await page.getByRole("checkbox", { name: "Patchbay", exact: true }).check();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await page.getByRole("button", { name: "wake body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const born = await current(page);
  await page.locator("[data-checked-form-id]").filter({ hasText: "Patchbay" }).click();
  const patchbay = visibleFormOutput(page);
  await expect(patchbay).toContainText("Active forms on this body");
  await patchbay.getByRole("button", { name: "Memory Lantern", exact: true }).click();
  await expect(patchbay).toContainText("memory_lantern");
  await expect(patchbay).toContainText("Body Plan");
  await page.getByRole("button", { name: "Inspect next", exact: true }).click();
  await page.getByRole("button", { name: "Edit current", exact: true }).click();
  await expect(patchbay.locator('[data-application-key="edit-request"]')).toContainText("no authority was inferred");
  expect((await current(page)).active_play_id).toBe(born.active_play_id);
});
