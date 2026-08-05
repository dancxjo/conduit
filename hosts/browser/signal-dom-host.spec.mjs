import { expect, test } from "@playwright/test";

test("two actual page hosts retain bounded exactly correlated DOM receipts", async ({ page }) => {
  const failures = [];
  page.on("pageerror", (error) => failures.push(error.stack ?? String(error)));
  page.on("console", (message) => {
    if (message.type() === "error") failures.push(message.text());
  });

  await page.goto("/hosts/browser/signal-dom-host.test.html");
  await expect(page.locator("#result")).toHaveText("ok");
  await expect(page.locator("#host-a output")).toHaveCount(16);
  await expect(page.locator("#host-b output")).toHaveCount(16);
  await expect(page.locator("#host-a output").last()).toHaveAttribute("data-sequence", "15");
  await expect(page.locator("#host-b output").last()).toHaveAttribute("data-sequence", "15");

  const result = await page.evaluate(() => globalThis.__conduitBrowserDomResults);
  expect(failures).toEqual([]);
  expect(result).toEqual({
    completionCount: 32,
    hostAReceipts: 16,
    hostBReceipts: 16,
    hostATimers: 15,
    hostBTimers: 15,
    hostARequestedTimerMs: 3750,
    hostBRequestedTimerMs: 3750,
    hostAStatus: 1,
    hostBStatus: 1,
    mismatchCode: -6,
    duplicateCode: "CND-BRW-S4-003",
    overflowCode: "CND-BRW-S4-004",
    itemOverflowCode: "CND-BRW-S4-004",
    byteOverflowCode: "CND-BRW-S4-004",
    malformedCode: "CND-BRW-S4-002",
    duplicateRuntimeCode: -10,
    malformedRuntimeCode: -9,
    cancellationCode: -11,
    terminalFailureCode: -13,
    hostACapacityStable: true,
    hostBCapacityStable: true,
    sharedSourceIdentity: true,
    distinctActivePlayIdentity: true,
  });
});
