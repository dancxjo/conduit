import { expect, test } from "@playwright/test";

test("artifact handoff requires activation and carries exact admitted bytes", async ({ page }) => {
  await page.addInitScript(() => { delete globalThis.showSaveFilePicker; });
  await page.goto("/proof/browser/browser-host-operations.test.html");
  await expect(page.locator("#result")).toHaveText("ready");
  expect(await page.evaluate(() => globalThis.__browserHostOperationProof.absentActivation))
    .toBe("user-activation-required");
  const downloadPromise = page.waitForEvent("download");
  await page.locator("#artifact").click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toBe("proof.bin");
  await expect(page.locator("#result")).toHaveText("handoff-offered");
  const outcome = await page.evaluate(() => globalThis.__browserHostOperationProof.outcomes[0]);
  expect(outcome).toMatchObject({
    operationId: "artifact/download",
    hostId: "host/browser-proof",
    bootId: "boot/browser-proof",
    applicationId: "application/browser-proof",
    applicationGeneration: 3,
    authorityGeneration: 7,
    disposition: "handoff-offered",
  });
});

test("history movement is presentation-scoped and exactly correlated", async ({ page }) => {
  await page.goto("/proof/browser/browser-host-operations.test.html");
  await page.locator("#location").click();
  await expect(page).toHaveURL(/\/proof\/browser\/presented-place\/$/);
  const outcome = await page.evaluate(() => globalThis.__browserHostOperationProof.outcomes[0]);
  expect(outcome).toMatchObject({
    operationId: "location/proof",
    kind: "location",
    disposition: "completed",
    path: "/proof/browser/presented-place/",
  });
  expect(outcome.membership).toBeUndefined();
  expect(outcome.lifecycle).toBeUndefined();
});

test("profile-gated device choice returns bounded browser-visible resource truth only", async ({ page }) => {
  await page.goto("/proof/browser/browser-host-operations.test.html");
  await page.locator("#device").click();
  await expect(page.locator("#result")).toHaveText("completed");
  const state = await page.evaluate(() => globalThis.__browserHostOperationProof);
  expect(state.outcomes[0]).toMatchObject({
    operationId: "device/proof",
    disposition: "completed",
    resource: {
      handle: "browser-resource/device/proof",
      vendorId: 0x1209,
      productId: 42,
      serialNumber: null,
    },
  });
  expect(state.outcomes[0].resource.membership).toBeUndefined();
  expect(state.outcomes[0].resource.planId).toBeUndefined();
  expect(await page.evaluate(() => globalThis.__browserHostOperationProof.active())).toBe(0);
});
