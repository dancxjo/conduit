import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function startServer() {
  const process = spawn("target/debug/patchbay-html", ["--documentary-fixture"], { stdio: ["ignore", "pipe", "pipe"] });
  const errors = [];
  process.stderr.setEncoding("utf8");
  process.stderr.on("data", chunk => errors.push(chunk));
  const lines = createInterface({ input: process.stdout });
  const url = new Promise((resolve, reject) => {
    lines.once("line", line => resolve(line.replace("PATCHBAY_HTML_URL=", "")));
    process.once("exit", code => reject(new Error(`Patchbay HTML exited ${code}: ${errors.join("")}`)));
  });
  return { process, lines, url };
}

test("Body truth is contextual Inspect content, not a nested application", async ({ page }) => {
  const server = startServer();
  try {
    await page.goto(await server.url);
    await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
    await expect(page.locator("#body-workbench")).toHaveCount(0);
    await expect(page.locator("[data-workbench-destination]")).toHaveCount(0);
    const bodyStatus = page.locator("#body-summary");
    await expect(bodyStatus).toHaveText(/^(?:No Body|Body: .+)/);
    await bodyStatus.click();
    await expect(page.locator("#inspector")).toBeVisible();
    if (await page.locator("#body-inspect").isVisible()) await expect(page.locator("#body-workbench-title")).toBeFocused();
    else await expect(page.locator("#inspect-title")).toBeFocused();
    await expect(page.locator("#body-membership-invitation")).toBeAttached();
    await expect(page.locator("#body-workbench-forms")).toBeAttached();
    await expect(page.locator("#body-workbench-history")).toBeAttached();
    await expect(page.locator("#body-form-query")).toHaveCount(0);
    await expect(page.locator("#body-workbench-available")).toBeAttached();
    await expect(page.locator("#body-execution-control")).toHaveCount(1);
  } finally {
    server.lines.close();
    server.process.kill("SIGTERM");
  }
});
