import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";

export function registerFireflyMultiHostTests(openStep) {
  test("canonical Firefly follower runs through two browser production kernels", async ({ page }) => {
    const source = await readFile(
      new URL("../../forms/firefly-line-follower/main.conduit", import.meta.url),
      "utf8",
    );
    await openStep(page, 3);
    const runner = page.locator(".multi-host-runner").first();
    await runner.locator("textarea").fill(source);
    await runner.getByRole("button", { name: "Run across two Hosts" }).click();

    const status = runner.locator('[data-application-key="play-status"]');
    await expect(status).toContainText("4 delivered cross-Host values");
    await expect(runner.locator(".morse")).toContainText("period 262 ms");
    await expect(runner.locator("textarea")).toHaveValue(source);
    await expect(runner.locator(".run-identities")).toContainText("Terminal source receipt");
    await expect(runner.locator(".run-identities")).toContainText("Terminal sink receipt");

    const identities = await page.evaluate(() => ({
      source: globalThis.__conduitTourHost.hostId,
      sink: globalThis.__conduitTourPeerHost.hostId,
    }));
    expect(identities.source).not.toBe(identities.sink);
  });
}
