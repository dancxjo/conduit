import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { openTourStep, startTour } from "./book-test-server.mjs";

// Exact reusable declarations; the storage-dependent namesake remains unproved.
const canonical = readFileSync(new URL("../../forms/secret-knock/main.conduit", import.meta.url), "utf8");
const reusable = canonical.slice(canonical.indexOf("form normalize-durations ("));
let entrance;
test.beforeEach(async () => { entrance = await startTour(); });
test.afterEach(() => entrance?.child.kill());

test("live timing reaches canonical nested comparison and typed output", async ({ page }) => {
  await openTourStep(page, entrance, 0);
  const runner = page.locator('[data-application-component="tour-laboratory"]');
  await runner.getByLabel("Structured output").selectOption("3");
  await runner.locator("textarea").fill(`${reusable}
form zz-live-comparison {
  button: input/button(maximum-transitions = 5)
  attempt: time/pressed-button-attempt(maximum-presses = 3, maximum-transitions = 5, timeout-ms = 1000ms)
  derive: derive-intervals
  normalize: normalize-durations
  compare: compare-pattern(metric = "maximum-absolute-millionths@1", tolerance-millionths = 0)
  show: presentation/structured-info
  button.transition > attempt.transition
  attempt.events > derive.events
  derive.intervals > normalize.intervals
  normalize.normalized > compare.candidate
  normalize.normalized > compare.template
  compare.comparison > show.input
}`);
  await expect(runner.locator(".compact-patchbay")).toHaveAttribute("data-disposition", "accepted");
  await runner.getByRole("button", { name: "Run", exact: true }).click();
  const control = runner.getByRole("button", { name: "Hold to control indicator" });
  await expect(control).toBeVisible();
  await control.hover();
  try {
    await page.mouse.down();
    await page.mouse.up();
    await page.mouse.down();
    await page.mouse.up();
    await page.mouse.down();
    await expect(runner.locator('[data-application-key="play-status"]')).toContainText("Completed");
    await expect(runner.locator(".morse")).toContainText("matched: true");
    await expect(runner.locator(".morse")).toContainText("score_millionths: 1000000");
  } finally {
    await page.mouse.up();
  }
});
