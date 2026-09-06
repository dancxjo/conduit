import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";

export function registerButtonMultiHostTests(openStep) {
  test("canonical button Form preserves ordered input across two browser Hosts", async ({ page }) => {
    const source = await readFile(new URL("../../forms/button-across-room/main.conduit", import.meta.url), "utf8");
    await openStep(page, 3);
    const runner = page.locator(".multi-host-runner").first();
    await runner.locator("textarea").fill(source);
    await runner.getByRole("button", { name: "Run across two Hosts" }).click();
    const status = runner.locator('[data-application-key="play-status"]');
    await expect(status).toContainText("button transition on Host A");
    const button = runner.getByRole("button", { name: "Hold to control indicator" });
    const bounds = await button.boundingBox();
    await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
    await page.mouse.down();
    await expect(runner.locator(".indicator")).toHaveAttribute("aria-label", "Indicator on");
    // Release away from controls whose layout changes when the Play retires,
    // so the synthetic click cannot select a different laboratory specimen.
    await page.mouse.move(0, 0);
    await page.mouse.up();
    await expect(status).toContainText("2 delivered cross-Host values");
    await expect(runner.locator(".indicator")).toHaveAttribute("aria-label", "Indicator off");
    await expect(runner.locator("textarea")).toHaveValue(source);
    const identities = await page.evaluate(() => ({
      source: globalThis.__conduitTourHost.hostId,
      sink: globalThis.__conduitTourPeerHost.hostId,
    }));
    expect(identities.source).not.toBe(identities.sink);
    await expect(runner.locator(".run-identities")).toContainText("Terminal source receipt");
    await expect(runner.locator(".run-identities")).toContainText("Terminal sink receipt");
  });

  test("stopping a pending two-Host input retires it before the next Play", async ({ page }) => {
    const source = await readFile(new URL("../../forms/button-across-room/main.conduit", import.meta.url), "utf8");
    await openStep(page, 3);
    const runner = page.locator(".multi-host-runner").first();
    await runner.locator("textarea").fill(source);
    const run = runner.getByRole("button", { name: "Run across two Hosts" });
    const status = runner.locator('[data-application-key="play-status"]');
    await run.click();
    await expect(status).toContainText("button transition on Host A");
    await runner.getByRole("button", { name: "Stop", exact: true }).click();
    await expect(status).toContainText("cancelled");
    await run.click();
    await expect(status).toContainText("button transition on Host A");
    await runner.getByRole("button", { name: "Hold to control indicator" }).click();
    await expect(status).toContainText("2 delivered cross-Host values");
    await expect(runner.locator(".indicator")).toHaveAttribute("aria-label", "Indicator off");
  });
}
