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

test("the legacy chapter Tutorial is absent from the ordinary Body path", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.getByRole("checkbox", { name: "Tutorial", exact: true })).toHaveCount(0);
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-body-tutorial]")).toBeVisible();
  await page.getByRole("button", { name: "+ Forms", exact: true }).click();
  await expect(page.getByText("Tutorial", { exact: true })).toHaveCount(0);
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
  await expect(patchbay).toContainText("Active Forms on this Body");
  await patchbay.getByRole("button", { name: "Memory Lantern", exact: true }).click();
  await expect(patchbay).toContainText("memory_lantern");
  await expect(patchbay).toContainText("Body Plan");
  await page.getByRole("button", { name: "Inspect next", exact: true }).click();
  await page.getByRole("button", { name: "Edit current", exact: true }).click();
  await expect(patchbay.locator('[data-application-key="edit-request"]')).toContainText("no authority was inferred");
  expect((await current(page)).active_play_id).toBe(born.active_play_id);
});
