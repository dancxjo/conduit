import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function startServer() {
  const process = spawn("target/debug/patchbay-html", [], { stdio: ["ignore", "pipe", "pipe"] });
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

const exactPixels = {
  animations: "disabled",
  caret: "hide",
  scale: "css",
  maxDiffPixels: 0,
};

test("stable Patchbay rendering contracts remain exact", async ({ page }) => {
  const server = startServer();
  try {
    await page.goto(await server.url);
    await expect(page.locator("#status")).toContainText("Manifestation Available");
    await expect(page.locator("#graph [data-subject]").first()).toBeVisible();
    await expect(page.locator("#form")).toHaveScreenshot("form-geometry.png", exactPixels);

    const first = page.locator("#subjects button").first();
    await first.click();
    await expect(first).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator("#interaction-proof")).toContainText("Succeeded");
    await expect(page.locator("#form .split")).toHaveScreenshot("selected-subject.png", exactPixels);

    await page.locator("#theme").click();
    await expect(page.locator("body")).toHaveClass(/high-contrast/);
    await expect(page.locator("#theme")).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator("#form")).toHaveScreenshot("high-contrast-form.png", exactPixels);
  } finally {
    server.lines.close();
    if (server.process.exitCode === null) server.process.kill("SIGTERM");
  }
});
