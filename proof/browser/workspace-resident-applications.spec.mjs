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

test("resident Tour keeps state across foreground switches in one browser Body Play", async ({ page }, testInfo) => {
  await page.goto(entrance.url);
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  for (const title of ["Tour", "Patchbay"]) {
    await page.getByRole("checkbox", { name: title, exact: true }).check();
  }
  await page.getByLabel("Friendly Body name", { exact: true }).fill("Wayfinder");
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const born = await current(page);
  expect(born.initial_forms).toHaveLength(3);

  await page.locator("[data-checked-form-id]").filter({ hasText: "Tour" }).click();
  await expect(visibleFormOutput(page)).toContainText("Tour");
  await page.getByRole("button", { name: "Run Form", exact: true }).click();
  await expect(visibleFormOutput(page)).toContainText("HELLO");
  await page.screenshot({ path: testInfo.outputPath("resident-tour-result.png"), fullPage: true });

  await page.locator("[data-checked-form-id]").filter({ hasText: "Memory Lantern" }).click();
  await page.locator("#form-input").focus();
  await page.keyboard.type("kept");
  await expect(visibleFormOutput(page)).toHaveText("kept");
  await page.locator("[data-checked-form-id]").filter({ hasText: "Tour" }).click();
  await expect(visibleFormOutput(page)).toContainText("HELLO");

  await page.locator("[data-checked-form-id]").filter({ hasText: "Patchbay" }).click();
  const patchbay = visibleFormOutput(page);
  await expect(patchbay).toContainText("tour/events");
  await expect(patchbay).toContainText("Body Plan");
  await page.getByRole("button", { name: "Inspect next", exact: true }).click();
  await expect(patchbay.locator('[data-application-key="identity"]')).toContainText("Plan");
  await page.getByRole("button", { name: "Edit current", exact: true }).click();
  await page.screenshot({ path: testInfo.outputPath("resident-patchbay-inspection.png"), fullPage: true });

  const after = await current(page);
  expect(after.body_id).toBe(born.body_id);
  expect(after.plan_id).toBe(born.plan_id);
  expect(after.active_play_id).toBe(born.active_play_id);
  await page.getByRole("button", { name: "Lull Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
});

test("resident Patchbay inspects and begins an exact edit in one browser Body Play", async ({ page }, testInfo) => {
  await page.goto(entrance.url);
  await page.getByRole("checkbox", { name: "Startup Chime", exact: true }).uncheck();
  await page.getByRole("checkbox", { name: "Patchbay", exact: true }).check();
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const born = await current(page);
  await page.locator("[data-checked-form-id]").filter({ hasText: "Patchbay" }).click();
  const patchbay = visibleFormOutput(page);
  await expect(patchbay).toContainText("memory_lantern");
  await expect(patchbay).toContainText("Body Plan");
  await page.getByRole("button", { name: "Inspect next", exact: true }).click();
  await page.getByRole("button", { name: "Edit current", exact: true }).click();
  await expect(patchbay.locator('[data-application-key="edit-request"]')).toContainText("no authority was inferred");
  expect((await current(page)).active_play_id).toBe(born.active_play_id);
});
