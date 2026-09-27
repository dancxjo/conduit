import { expect, test } from "@playwright/test";
import { prepareFieldStationPackage } from "./field-station-package.mjs";

test.beforeAll(async ({}, testInfo) => {
  testInfo.setTimeout(120_000);
  await prepareFieldStationPackage();
});

test("an external page births, wakes, inspects, and lulls the canonical Clock through the public SDK", async ({ page }) => {
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.goto("/proof/browser/field-station/");
  await expect(page.locator("#status")).toHaveText("Clock Body awake", { timeout: 15_000 });

  const running = await page.evaluate(() => ({
    identities: __conduitFieldStation.identities,
    playState: __conduitFieldStation.play.state,
  }));
  expect(running.playState).toBe("playing");
  expect(Object.values(running.identities).every((identity) => typeof identity === "string" && identity.length > 0)).toBe(true);
  expect(new Set(Object.values(running.identities)).size).toBe(6);
  for (const [name, identity] of Object.entries(running.identities)) {
    const selector = name.replace(/Id$/, "").toLowerCase();
    await expect(page.locator(`[data-identity="${selector}"]`)).toHaveText(identity);
  }

  await page.getByRole("button", { name: "Lull Clock Body", exact: true }).click();
  await expect(page.locator("#status")).toHaveText("Clock Body lulled");
  const lulled = await page.evaluate(() => ({
    snapshot: __conduitFieldStation.lullSnapshot,
    playState: __conduitFieldStation.play.state,
    receipts: __conduitFieldStation.play.receipts,
  }));
  expect(lulled.playState).toBe("terminal");
  expect(lulled.receipts).toHaveLength(1);
  expect(lulled.receipts[0].active_play_id).toBe(running.identities.playId);
  expect(lulled.receipts[0].disposition).toBe("cancelled");
  expect(lulled.snapshot.evidence.body_id).toBe(running.identities.bodyId);
  expect(lulled.snapshot.evidence.body.state).toBe("Lulled");
  const wake = lulled.snapshot.evidence.wakes.find(({ wake_id }) => wake_id === running.identities.wakeId);
  expect(wake?.lifecycle).toBe("Lulled");
  expect(wake?.plans).toContainEqual(expect.objectContaining({
    plan_id: running.identities.planId,
    active_play_id: running.identities.playId,
  }));
  expect(pageErrors).toEqual([]);
});
