import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { openTourStep } from "./tour-test-server.mjs";

const canonical = readFileSync(new URL("../../forms/secret-knock/main.conduit", import.meta.url), "utf8");
export function registerPatternComparisonTests(getEntrance) {
test("canonical Secret Knock runs its stored template and nested recognizer", async ({ page }) => {
  await openTourStep(page, getEntrance(), 0);
  const runner = page.locator('[data-application-component="tour-laboratory"]');
  await runner.getByLabel("Structured output").selectOption("3");
  await runner.locator("textarea").fill(canonical);
  await expect(runner.locator(".compact-patchbay")).toHaveAttribute("data-disposition", "accepted");
  await runner.getByRole("button", { name: "Run", exact: true }).click();
  const control = runner.getByRole("button", { name: "Hold to control indicator" });
  await expect(control).toBeVisible();
  await control.hover();
  try {
    await page.mouse.down();
    await page.waitForTimeout(30);
    await page.mouse.up();
    await page.waitForTimeout(100);
    await page.mouse.down();
    await page.waitForTimeout(30);
    await page.mouse.up();
    await page.waitForTimeout(300);
    await page.mouse.down();
    await expect(runner.locator('[data-application-key="play-status"]')).toContainText("Completed");
    await expect(runner.locator(".morse")).toContainText("matched: true");
    await expect(runner.locator(".morse")).toContainText("score_millionths:");
  } finally {
    await page.mouse.up();
  }
});
}
