import { test, expect } from "@playwright/test";
import { startStaticProduct } from "./book-test-server.mjs";

let entrance;
test.beforeEach(async () => { entrance = await startStaticProduct("."); });
test.afterEach(() => entrance?.child.kill());

test("pointer and deterministic input causally reach Conduit quantity output", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(new URL("proof/browser/quantity-controller.test.html", entrance.url).href);
  await expect(page.locator("#status")).toHaveText("Ready");
  await expect(page.locator("#output")).toHaveText("Waiting for input");
  const controller = page.getByRole("button", { name: "Pointer controller", exact: true });
  const box = await controller.boundingBox();
  expect(box).not.toBeNull();
  const plays = [];
  for (const [fraction, expected] of [[0.25, "5015 Hz"], [0.5, "10010 Hz"]]) {
    await page.mouse.click(box.x + box.width * fraction, box.y + box.height / 2);
    await expect(page.locator("#output")).toHaveText(expected);
    const evidence = JSON.parse(await page.locator("#evidence").textContent());
    expect(evidence.inputMode).toBe("pointer");
    expect(evidence.effect.text).toBe(expected);
    expect(evidence.effect.expanded_gears.filter(({ kind_id }) => kind_id.startsWith("structured-info/selector-")).length).toBe(2);
    expect(evidence.effect.expanded_gears.some(({ kind_id }) => kind_id === "math/map-quantity")).toBe(true);
    expect(evidence.receipt.disposition).toBe("completed");
    expect(evidence.acquisition.active_play_id).toBe(evidence.effect.active_play_id);
    expect(evidence.receipt.active_play_id).toBe(evidence.effect.active_play_id);
    plays.push(evidence.effect.active_play_id);
  }
  await page.getByRole("button", { name: "Deterministic input: 0.75", exact: true }).click();
  await expect(page.locator("#output")).toHaveText("15005 Hz");
  const alternate = JSON.parse(await page.locator("#evidence").textContent());
  expect(alternate.inputMode).toBe("deterministic");
  expect(alternate.receipt.disposition).toBe("completed");
  plays.push(alternate.effect.active_play_id);
  expect(new Set(plays).size).toBe(3);
  expect(errors).toEqual([]);
});
