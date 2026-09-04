import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { expect, test } from "@playwright/test";

function startServer() {
  const process = spawn("target/debug/patchbay-html", ["--recursive-form-proof"], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  const errors = [];
  process.stderr.setEncoding("utf8");
  process.stderr.on("data", (chunk) => errors.push(chunk));
  const lines = createInterface({ input: process.stdout });
  const url = new Promise((resolve, reject) => {
    lines.once("line", (line) => resolve(line.replace("PATCHBAY_HTML_URL=", "")));
    process.once("exit", (code) => reject(new Error(`Patchbay HTML exited ${code}: ${errors.join("")}`)));
  });
  return { process, lines, url };
}

test("recursive Form Gears open through stable Faces without changing execution truth", async ({ page }) => {
  const server = startServer();
  try {
    const url = await server.url;
    await page.goto(url);
    const before = await (await fetch(`${url}/api/snapshot`)).json();
    expect(before.presentation.basis.plan_id).not.toBeNull();
    expect(before.presentation.properties.filter((property) =>
      property.name === "reviewed-back" && property.value.Text === "available")).toHaveLength(4);
    expect(before.debugger ?? null).toBeNull();

    const outer = page.locator('.faceplate-back-control[aria-label*="patchbay-capstone/canvas"]').first();
    await expect(outer).toHaveAttribute("aria-expanded", "false");
    await page.getByRole("button", { name: "Plan", exact: true }).click();
    await expect(page.locator("article", { has: outer }).locator(".faceplate-clue"))
      .toHaveText("recursive");
    const collapsedNodes = await page.locator(".flow-faceplate").count();
    await outer.press("Enter");
    await expect(outer).toHaveAttribute("aria-expanded", "true");
    expect(await page.locator(".flow-faceplate").count()).toBeGreaterThan(collapsedNodes);

    const nested = page.locator('.faceplate-back-control[aria-expanded="false"]').first();
    await expect(nested).toBeVisible();
    const nestedLabel = await nested.getAttribute("aria-label");
    await nested.click();
    await expect(page.getByRole("button", { name: nestedLabel.replace("Open", "Close") }))
      .toHaveAttribute("aria-expanded", "true");

    await outer.press("Enter");
    await expect(outer).toHaveAttribute("aria-expanded", "false");
    const after = await (await fetch(`${url}/api/snapshot`)).json();
    expect(after.presentation.identity).toBe(before.presentation.identity);
    expect(after.presentation.basis).toEqual(before.presentation.basis);
    expect(after.renderer.plan).toEqual(before.renderer.plan);
    expect(after.debugger ?? null).toBeNull();
  } finally {
    server.lines.close();
    if (server.process.exitCode === null) server.process.kill("SIGTERM");
  }
});
