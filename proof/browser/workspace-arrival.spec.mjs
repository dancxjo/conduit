import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";

let entrance;
test.beforeEach(async () => { entrance = await startStaticProduct("target/workspace-product", "/conduit/workspace/"); });
test.afterEach(() => entrance?.child.kill());

test("Birth arrives in listening Forms; foreground changes and reload preserve the Body", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1280, height: 1000 });
  await page.goto(entrance.url);
  const birth = page.locator(".body-birth-runner");
  await expect(birth.getByRole("heading", { name: "A Body of your own" })).toBeVisible();
  await expect(birth.getByRole("checkbox", { name: "Memory Lantern", exact: true })).toBeChecked();
  await birth.getByLabel("Friendly Body name", { exact: true }).fill("Roseau");
  await birth.getByRole("checkbox", { name: "Desk Telegraph", exact: true }).check();
  await page.screenshot({ path: testInfo.outputPath("workspace-birth.png"), fullPage: true });
  await birth.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-body-name]")).toHaveText("Roseau");
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  await expect(page.locator("[data-wake-body]")).toBeHidden();
  const identity = () => page.evaluate(() => globalThis.__conduitWorkspace.current());
  const first = await identity();
  await expect(page.getByRole("navigation", { name: "Your Forms" }).getByRole("button")).toHaveCount(2);
  await page.getByRole("navigation", { name: "Your Forms" }).getByRole("button", { name: "Memory Lantern", exact: true }).click();
  const output = page.locator("[data-form-output] output:visible");
  await page.keyboard.press("h");
  await expect(output).toHaveText("h");
  await page.keyboard.press("i");
  await expect(output).toHaveText("hi");
  await page.getByRole("navigation", { name: "Your Forms" }).getByRole("button", { name: "Desk Telegraph", exact: true }).click();
  await page.keyboard.press("o");
  await page.keyboard.press("k");
  await page.keyboard.press("Enter");
  await expect(output).toHaveText("ok");
  await page.getByRole("navigation", { name: "Your Forms" }).getByRole("button", { name: "Memory Lantern", exact: true }).click();
  await expect(output).toHaveText("hi");
  await expect(page.locator("[data-flow-label]")).toHaveText("keyboard → keymap → edit → text");
  await page.keyboard.press("Backspace");
  await expect(output).toHaveText("h");
  expect((await identity()).active_play_id).toBe(first.active_play_id);
  expect((await identity()).plan_id).toBe(first.plan_id);
  await page.screenshot({ path: testInfo.outputPath("workspace-listening.png"), fullPage: true });

  await page.getByRole("navigation", { name: "Your Forms" }).getByRole("button", { name: "Desk Telegraph", exact: true }).click();
  await page.evaluate(() => globalThis.__conduitWorkspace.settled());
  await page.reload();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  await expect(page.locator("#surface-title")).toHaveText("Desk Telegraph");
  const restored = await identity();
  expect(restored.body_id).toBe(first.body_id);
  expect(restored.host_id).toBe(first.host_id);
  expect(restored.boot_id).not.toBe(first.boot_id);
  expect(restored.plan_id).not.toBe(first.plan_id);
  expect(restored.active_play_id).not.toBe(first.active_play_id);
  await page.keyboard.press("n");
  await page.keyboard.press("Enter");
  await expect(output).toHaveText("n");
  await page.screenshot({ path: testInfo.outputPath("workspace-returned.png"), fullPage: true });
  await page.getByRole("button", { name: "Lull Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Lulled");
  expect((await identity()).body_id).toBe(first.body_id);
});

test("a failed started-state save cancels the real Play before dispatching effects", async ({ page }) => {
  await page.addInitScript(() => {
    const put = IDBObjectStore.prototype.put;
    globalThis.__failedBodyWrites = 0;
    IDBObjectStore.prototype.put = function(record, ...args) {
      if (record.key === "body-session" && typeof record.value === "string") {
        const value = JSON.parse(record.value);
        if (value.schema === "conduit.workspace/body@1" && value.evidence.wakes?.some(wake => wake.lifecycle === "Playing")) {
          globalThis.__failedBodyWrites += 1;
          throw new DOMException("Injected Body storage exhaustion", "QuotaExceededError");
        }
      }
      return put.call(this, record, ...args);
    };
  });
  await page.goto(entrance.url);
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Stopped");
  await expect(page.locator("#surface-guidance")).toContainText("Your Body could not be saved");
  await expect(page.getByRole("button", { name: "Wake Body", exact: true })).toBeDisabled();
  await expect(page.locator("#form-input")).toHaveAttribute("aria-disabled", "true");
  const result = await page.evaluate(() => ({ state: globalThis.__conduitWorkspace.state(), writes: globalThis.__failedBodyWrites }));
  expect(result.writes).toBe(1);
  expect(result.state.terminal.disposition).toBe("cancelled");
  expect(result.state.refusal.code).toBe("QuotaExceededError");
  expect(result.state.terminal.active_play_id).toBe(result.state.play.active_play_id);
  expect(result.state.terminal.manifestation_completions).toBe(0);
  await expect(page.locator("[data-form-output] output")).toHaveCount(0);
});

test("a second window cannot recover a Body while its first Host is alive", async ({ page, context }) => {
  await page.goto(entrance.url);
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  const other = await context.newPage();
  await other.goto(entrance.url);
  await expect(other.locator("[data-workspace-notice]")).toContainText("This Body is open in another window");
  await expect(other.locator("[data-workspace-surface]")).toBeHidden();
  await page.locator("#form-input").focus();
  await page.keyboard.press("a");
  await expect(page.locator("[data-form-output] output:visible")).toHaveText("a");
  await other.close();
});

test("Birth and the listening surface fit a narrow window", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(entrance.url);
  await expect(page.getByRole("heading", { name: "A Body of your own" })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.getByRole("button", { name: "Birth Body", exact: true }).click();
  await expect(page.locator("[data-play-state]")).toHaveText("Playing");
  await page.keyboard.press("h");
  await expect(page.locator("[data-form-output] output:visible")).toHaveText("h");
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.screenshot({ path: testInfo.outputPath("workspace-narrow.png"), fullPage: true });
});
