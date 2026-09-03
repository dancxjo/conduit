import { expect, test } from "@playwright/test";

test("real Chromium pointer input crosses the planned production kernel", async ({ page }) => {
  const failures = [];
  page.on("pageerror", (error) => failures.push(error.stack ?? String(error)));
  page.on("console", (message) => {
    if (message.type() === "error") failures.push(message.text());
  });
  await page.goto("/proof/browser/browser-pointer.test.html");
  await expect(page.locator("#result")).toHaveText("ready");
  const bounds = await page.locator("#surface").boundingBox();
  expect(bounds).not.toBeNull();
  await page.mouse.move(bounds.x + 100, bounds.y + 50);
  await page.mouse.down();
  await page.mouse.up();
  await expect(page.locator("#receipt")).not.toBeEmpty();
  const receipt = await page.evaluate(() => globalThis.__conduitBrowserPointerReceipt);
  expect(receipt).toMatchObject({
    schema: "input/pointer-event@1",
    position_x: 250000,
    position_y: 250000,
    primary_pressed: false,
    dropped: 0,
    queue_capacity: 1,
    sequence: 1,
  });
  expect(receipt.canonical_bytes).toBeGreaterThan(0);
  expect(receipt.value_kind).toMatch(/^structured-info\/profile-[0-9a-f]{64}@1$/);
  expect(receipt.plan_id).not.toEqual(receipt.play_id);
  expect(receipt.sign_id).not.toEqual(receipt.play_id);
  expect(receipt.source_placement_id).not.toEqual(receipt.presentation_placement_id);

  const refusals = await page.evaluate(() => ({
    invalidPosition: globalThis.__conduitBrowserPointerSource.api.conduit_browser_pointer_run(
      1000001, 0, 0, 0, 0, 0, 0, 1, 1,
    ),
    invalidButtons: globalThis.__conduitBrowserPointerSource.api.conduit_browser_pointer_run(
      0, 0, 0, 0, 2, 0, 0, 1, 1,
    ),
  }));
  expect(refusals).toEqual({ invalidPosition: -1, invalidButtons: -2 });

  await page.evaluate(() => globalThis.__conduitBrowserPointerSource.close());
  await page.mouse.move(bounds.x + 200, bounds.y + 100);
  await page.mouse.down();
  await page.mouse.up();
  expect(await page.evaluate(() => globalThis.__conduitBrowserPointerReceipt.sequence)).toBe(1);
  expect(failures).toEqual([]);
});
