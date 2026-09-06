import { expect, test } from "@playwright/test";

test("ordered button transitions survive gaps between Host requests and refuse overflow", async ({ page }) => {
  await page.goto("/proof/browser/browser-human-input.test.html");
  await expect(page.locator("#status")).toHaveText("ready");
  await page.evaluate(() => {
    globalThis.buttonFirst = globalThis.__conduitHumanInput.adapter.nextButton();
  });
  const bounds = await page.locator("#surface").boundingBox();
  await page.mouse.move(bounds.x + 20, bounds.y + 20);
  await page.mouse.down();
  await page.mouse.up();
  const transitions = await page.evaluate(async () => [
    await globalThis.buttonFirst,
    await globalThis.__conduitHumanInput.adapter.nextButton(),
  ]);
  expect(transitions.map(({ pressed, sequence }) => [pressed, sequence])).toEqual([[true, 0], [false, 1]]);
  for (let index = 0; index < 5; index += 1) {
    await page.mouse.down();
    await page.mouse.up();
  }
  expect(await page.evaluate(() => globalThis.__conduitHumanInput.adapter.nextButton().catch((error) => error.code))).toBe("Pressure");
});

test("queued button release cannot survive a replacement Boot", async ({ page }) => {
  await page.goto("/proof/browser/browser-human-input.test.html");
  await expect(page.locator("#status")).toHaveText("ready");
  await page.evaluate(() => { globalThis.buttonFirst = globalThis.__conduitHumanInput.adapter.nextButton(); });
  const bounds = await page.locator("#surface").boundingBox();
  await page.mouse.move(bounds.x + 20, bounds.y + 20);
  await page.mouse.down();
  await page.mouse.up();
  expect(await page.evaluate(async () => (await globalThis.buttonFirst).pressed)).toBe(true);
  await page.evaluate(() => globalThis.__conduitHumanInput.staleBoot());
  expect(await page.evaluate(() => globalThis.__conduitHumanInput.adapter.nextButton().catch((error) => error.code))).toBe("StaleBoot");
});

test("selected keyboard and pointer adapt real Chromium actions to portable values", async ({ page }) => {
  const failures = [];
  page.on("pageerror", (error) => failures.push(error.stack ?? String(error)));
  await page.goto("/proof/browser/browser-human-input.test.html");
  await expect(page.locator("#status")).toHaveText("ready");
  const surface = page.locator("#surface");
  await surface.focus();
  const pendingKey = page.evaluate(() => globalThis.__conduitHumanInput.acquireKeyboard());
  await page.keyboard.down("Shift");
  await page.keyboard.press("KeyA");
  await page.keyboard.up("Shift");
  await pendingKey;
  await expect(page.locator("#keyboard")).toContainText('"schema":"input/key-event@1"');
  const keyboard = JSON.parse(await page.locator("#keyboard").textContent());
  expect(keyboard.canonical_bytes).toEqual([0x04, 0, 2]);
  expect(keyboard.owner).toMatchObject({
    host_id: "browser/human-input-proof",
    boot_id: "browser/human-input-proof-boot",
    offer_generation: 1,
  });

  const bounds = await surface.boundingBox();
  await page.mouse.move(bounds.x + 100, bounds.y + 50);
  await page.mouse.down();
  await page.mouse.up();
  await expect(page.locator("#pointer")).toContainText('"schema":"input/pointer-event@1"');
  const pointer = JSON.parse(await page.locator("#pointer").textContent());
  expect(pointer).toMatchObject({
    position_x: 250000,
    position_y: 250000,
    primary_pressed: false,
    queue_capacity: 1,
    sequence: 1,
  });
  expect(failures).toEqual([]);
});

test("profile omission, finite pressure, cancellation, and stale Boot stay distinct", async ({ page }) => {
  await page.goto("/proof/browser/browser-human-input.test.html?profile=viewer");
  await expect(page.locator("#status")).toHaveText("ready");
  expect(await page.evaluate(async () => (await globalThis.__conduitHumanInput.acquireKeyboard()).code)).toBe("UnsupportedInput");
  await expect(page.locator("#pointer")).toHaveText("UnsupportedInput");

  await page.goto("/proof/browser/browser-human-input.test.html");
  const outcomes = await page.evaluate(async () => {
    const requests = Array.from({ length: 9 }, () => globalThis.__conduitHumanInput.adapter.nextKeyboard().catch((error) => error.code));
    const pressure = await requests[8];
    globalThis.__conduitHumanInput.adapter.cancelPending();
    return { pressure, cancelled: await requests[0] };
  });
  expect(outcomes).toEqual({ pressure: "Pressure", cancelled: "Cancelled" });

  await page.evaluate(() => globalThis.__conduitHumanInput.staleBoot());
  const pending = page.evaluate(() => globalThis.__conduitHumanInput.acquireKeyboard());
  await page.locator("#surface").focus();
  await page.keyboard.press("KeyB");
  await pending;
  await expect(page.locator("#keyboard")).toHaveText("StaleBoot");
});

test("focus loss is recoverable while page loss is terminal", async ({ page }) => {
  await page.goto("/proof/browser/browser-human-input.test.html");
  const focusPending = page.evaluate(() => globalThis.__conduitHumanInput.acquireKeyboard());
  await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  await focusPending;
  await expect(page.locator("#keyboard")).toHaveText("FocusLost");

  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  const pagePending = page.evaluate(() => globalThis.__conduitHumanInput.acquireKeyboard());
  await page.evaluate(() => window.dispatchEvent(new Event("pagehide")));
  await pagePending;
  await expect(page.locator("#keyboard")).toHaveText("PageLost");
  expect(await page.evaluate(async () => (await globalThis.__conduitHumanInput.acquireKeyboard()).code)).toBe("PageLost");
});
