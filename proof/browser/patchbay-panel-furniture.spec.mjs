import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { readFile } from "node:fs/promises";
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

test("bounded collections are manifested without numbered static slot farms", async () => {
  const html = await readFile("products/patchbay/html/assets/index.html", "utf8");
  const renderer = await readFile("products/patchbay/html/assets/shared-presentation.js", "utf8");
  expect(html).not.toMatch(/data-application-slot="[^"]+-\d+"/);
  expect(html.match(/data-application-collection=/g)?.length).toBeGreaterThan(10);
  expect(html.length).toBeLessThan(24_000);
  expect(renderer).toContain("requiredChunks > maximumChunks");
  expect(renderer).toContain("container.replaceChildren(...slots)");
  expect(renderer).not.toContain("innerHTML");
});

test("Patchbay has one stable Library and one contextual Inspect surface", async ({ page }) => {
  const server = startServer();
  try {
    await page.goto(await server.url);
    await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
    const navigation = page.getByRole("navigation", { name: "Patchbay workspace" });
    await expect(navigation.getByRole("button", { name: "Library" })).toHaveCount(1);
    await expect(navigation.getByRole("button", { name: "Inspect" })).toHaveCount(1);
    await expect(navigation.getByRole("button")).toHaveCount(3);
    await expect(page.locator("#palette")).toBeVisible();
    await expect(page.locator("#inspector")).toBeVisible();
    await expect(page.locator("#library-query")).toHaveCount(1);
    await expect(page.locator("#form-query,#gear-query,#body-form-query")).toHaveCount(0);
    await expect(page.locator("#toggle-parts,#toggle-truth,#toggle-structured")).toHaveCount(0);
    await expect(page.locator("#structured-navigator")).toBeAttached();
    await expect(page.locator("#deep-inspection")).toBeAttached();

    await navigation.getByRole("button", { name: "Library" }).click();
    await expect(page.locator("#palette")).toBeHidden();
    await expect(page.locator("#inspector")).toBeVisible();
    await navigation.getByRole("button", { name: "Library" }).click();
    await expect(page.locator("#palette")).toBeVisible();
  } finally {
    server.lines.close();
    server.process.kill("SIGTERM");
  }
});
