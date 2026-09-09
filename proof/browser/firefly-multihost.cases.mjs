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
    await runner.getByRole("button", { name: "Stop", exact: true }).click();
    await expect(status).toContainText(/cancelled after \d+ delivered cross-Host values/);
    const delivered = Number((await status.textContent()).match(/after (\d+) delivered/)?.[1]);
    expect(delivered).toBeGreaterThanOrEqual(4);
    await expect(runner.locator(".morse")).toContainText("period 262 ms");
    await expect(runner.locator("textarea")).toHaveValue(source);
    const semanticPatchbay = runner.locator(".compact-patchbay");
    await expect(semanticPatchbay).toContainText("time/rhythm-state");
    await expect(semanticPatchbay).toContainText("time/phase-synchronize");
    await expect(semanticPatchbay).toContainText("presentation/rhythm");

    const plan = runner.locator(".plan-view");
    await expect(plan).toContainText("time/rhythm-state");
    await expect(plan).toContainText("browser/kernel-rhythm-state-source@1");
    await expect(plan).toContainText("time/phase-synchronize");
    await expect(plan).toContainText("browser/kernel-phase-synchronize@1");
    await expect(plan).toContainText("presentation/rhythm");
    await expect(plan).toContainText("tour/browser-memory-line");
    await expect(plan).toContainText("1 item / 4096 bytes");

    const run = runner.locator(".run-identities");
    await expect(run).toContainText("Active Play");
    await expect(run).toContainText("Presentation");
    await expect(run).toContainText("Terminal source receipt");
    await expect(run).toContainText("Terminal sink receipt");

    const identities = await page.evaluate(() => ({
      source: globalThis.__conduitTourHost.hostId,
      sink: globalThis.__conduitTourPeerHost.hostId,
    }));
    expect(identities.source).not.toBe(identities.sink);
  });
}
