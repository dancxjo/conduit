import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";

let entrance;
test.beforeEach(async () => { entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/"); });
test.afterEach(() => entrance?.child.kill());

const current = page => page.evaluate(() => globalThis.__conduitWorkspace.current());
const card = (page, title) => page.locator('[data-application-key^="library-form-"]').filter({ hasText: title });
const openLibrary = page => page.getByRole("button", { name: "+ Forms", exact: true }).click();
async function birth(page) {
  await page.goto(entrance.url);
  const chime = page.getByRole("checkbox", { name: "Startup Chime", exact: true });
  if (await chime.count()) await chime.uncheck();
  await page.getByLabel("Friendly Body name", { exact: true }).fill("Roseau");
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
}

test("Use installs into the same Body; repeated Use preserves Play and removal survives reload", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1280, height: 1000 });
  await birth(page);
  const initial = await current(page);
  await openLibrary(page);
  await expect(card(page, "Memory Lantern")).toContainText("In your Body");
  await page.screenshot({ path: testInfo.outputPath("workspace-form-library.png"), fullPage: true });
  await page.getByRole("textbox", { name: "Find a Form", exact: true }).fill("desk");
  await card(page, "Desk Telegraph").getByRole("button", { name: "Use", exact: true }).click();
  await expect(page.locator("#surface-title")).toHaveText("Desk Telegraph");
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const installed = await current(page);
  expect(installed.body_id).toBe(initial.body_id);
  expect(installed.here_part_id).toBe(initial.here_part_id);
  expect(installed.workload_revision).toBe(1);
  expect(installed.active_play_id).not.toBe(initial.active_play_id);
  expect(installed.initial_forms).toHaveLength(2);
  await page.keyboard.type("hello");
  await page.keyboard.press("Enter");
  await expect(page.locator("[data-form-output] output:visible")).toHaveText("hello");
  await page.keyboard.type("again");
  await page.keyboard.press("Enter");
  await expect(page.locator("[data-form-output] output:visible")).toHaveText("again");
  await openLibrary(page);
  await card(page, "Desk Telegraph").getByRole("button", { name: "Use", exact: true }).click();
  expect((await current(page)).active_play_id).toBe(installed.active_play_id);
  await expect(page.locator("[data-form-output] output:visible")).toHaveText("again");
  await page.screenshot({ path: testInfo.outputPath("workspace-installed-form.png"), fullPage: true });
  await openLibrary(page);
  await card(page, "Desk Telegraph").getByRole("button", { name: "Remove", exact: true }).click();
  await expect(card(page, "Desk Telegraph")).toContainText("Not in your Body");
  await page.getByRole("button", { name: "Back to the surface", exact: true }).click();
  await expect(page.locator("#surface-title")).toHaveText("Memory Lantern");
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  await page.locator("#form-input").focus();
  await page.keyboard.press("r");
  await expect(page.locator("[data-form-output] output:visible")).toHaveText("r");
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const restored = await current(page);
  expect(restored.body_id).toBe(initial.body_id);
  expect(restored.workload_revision).toBe(2);
  expect(restored.initial_forms).toEqual(initial.initial_forms);
});

test("removing the final Form retains an empty Body that can acquire Forms again", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await birth(page);
  const initial = await current(page);
  await openLibrary(page);
  await card(page, "Memory Lantern").getByRole("button", { name: "Remove", exact: true }).click();
  await expect(card(page, "Memory Lantern")).toContainText("Not in your Body");
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  expect((await current(page)).initial_forms).toHaveLength(0);
  expect((await current(page)).active_play_id).toBeUndefined();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.screenshot({ path: testInfo.outputPath("workspace-empty-library-narrow.png"), fullPage: true });
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  expect((await current(page)).body_id).toBe(initial.body_id);
  await openLibrary(page);
  await card(page, "Memory Lantern").getByRole("button", { name: "Use", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  await expect(page.getByRole("button", { name: "Lull Body", exact: true })).toBeVisible();
  await page.keyboard.press("a");
  await expect(page.locator("[data-form-output] output:visible")).toHaveText("a");
  expect((await current(page)).body_id).toBe(initial.body_id);
});

test("a failed workset save stops before replacement effects and reload recovers the saved workload", async ({ page }) => {
  await page.addInitScript(() => {
    const put = IDBObjectStore.prototype.put;
    globalThis.__failedWorksetWrites = 0;
    IDBObjectStore.prototype.put = function(record, ...args) {
      if (record.key === "body-session" && typeof record.value === "string") {
        const value = JSON.parse(record.value);
        if (value.evidence?.body.workload_revision > 0) {
          globalThis.__failedWorksetWrites++;
          throw new DOMException("Injected workset storage exhaustion", "QuotaExceededError");
        }
      }
      return put.call(this, record, ...args);
    };
  });
  await birth(page);
  const initial = await current(page);
  await openLibrary(page);
  await card(page, "Desk Telegraph").getByRole("button", { name: "Use", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Stopped");
  const failed = await page.evaluate(() => ({ current: globalThis.__conduitWorkspace.current(), state: globalThis.__conduitWorkspace.state(), writes: globalThis.__failedWorksetWrites }));
  expect(failed.writes).toBe(1);
  expect(failed.current.active_play_id).toBeUndefined();
  expect(failed.state.terminal.disposition).toBe("cancelled");
  expect(failed.state.terminal.active_play_id).toBe(initial.active_play_id);
  await expect(page.locator("[data-form-output] output")).toHaveCount(0);
  await page.reload();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const restored = await current(page);
  expect(restored.body_id).toBe(initial.body_id);
  expect(restored.workload_revision).toBe(0);
  expect(restored.initial_forms).toEqual(initial.initial_forms);
});
