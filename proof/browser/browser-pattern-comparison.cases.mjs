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
  await runner.locator(".morse").evaluate((output) => {
    output.dataset.presentationCount = "0";
    new MutationObserver(() => {
      output.dataset.presentationCount = String(Number(output.dataset.presentationCount) + 1);
    }).observe(output, { childList: true, characterData: true, subtree: true });
  });
  await control.hover();
  try {
    for (const transition of ["down", "up", "down", "up", "down", "up", "down", "up", "down", "up", "down"]) {
      await expect(runner.locator('[data-application-key="play-status"]')).toContainText("button transition");
      await page.mouse[transition]();
    }
    await expect(runner.locator(".morse")).toContainText("matched:");
    await expect(runner.locator(".morse")).toContainText("score_millionths:");
    await expect.poll(() => runner.locator(".morse").getAttribute("data-presentation-count")).toBe("2");
    const labels = await runner.locator(".run-identities dt").allTextContents();
    const values = await runner.locator(".run-identities dd").allTextContents();
    const identities = Object.fromEntries(labels.map((label, index) => [label, values[index]]));
    await runner.getByRole("button", { name: "Stop", exact: true }).click();
    await expect(runner.locator('[data-application-key="play-status"]')).toContainText("cancelled");
    console.log(`CONDUIT_FORM_EVIDENCE=${JSON.stringify({
      slug: "secret-knock",
      status: "passed",
      plan_id: identities.Plan,
      play_id: identities["Active Play"],
    })}`);
  } finally {
    await page.mouse.up();
  }
});
}
