import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";

function startServer() {
  const process = spawn("target/debug/conduit-browser-patchbay-workbench", ["--documentary-fixture"], { stdio: ["ignore", "pipe", "pipe"] });
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
  const html = await readFile("targets/browser/patchbay-workbench/assets/index.html", "utf8");
  const renderer = await readFile("plots/patchbay/workbench/browser/shared-face.js", "utf8");
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
    await expect(page.locator("#palette")).toBeHidden();
    await expect(page.locator("#inspector")).toBeHidden();
    await expect(page.locator("#library-query")).toHaveCount(1);
    await expect(page.locator("#plot-query,#gear-query,#body-plot-query")).toHaveCount(0);
    await expect(page.locator("#toggle-parts,#toggle-truth,#toggle-structured")).toHaveCount(0);
    await expect(page.locator("#structured-navigator")).toBeAttached();
    await expect(page.locator("#deep-inspection")).toBeAttached();
    await expect(page.locator("#lens-label")).toHaveText("BODY · STRUCTURE");
    await expect(page.getByRole("button", { name: "Plot", exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Realization", exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Debug", exact: true })).toBeVisible();

    await navigation.getByRole("button", { name: "Library" }).click();
    await expect(page.locator("#palette")).toBeVisible();
    await expect(page.locator("#inspector")).toBeHidden();
    await navigation.getByRole("button", { name: "Library" }).click();
    await expect(page.locator("#palette")).toBeHidden();
    await navigation.getByRole("button", { name: "Inspect" }).focus();
    await page.keyboard.press("Enter");
    await expect(page.locator("#inspector")).toBeVisible();
    await page.getByRole("button", { name: "Debug", exact: true }).click();
    await expect(page.locator("#lens-label")).toHaveText("BODY · SIGNS");
    await expect(page.locator("#debugger-control")).toBeVisible();
    await expect(page.locator("#debugger-timeline")).toBeVisible();
    await expect(page.locator("#debugger-watches")).toBeVisible();
  } finally {
    server.lines.close();
    server.process.kill("SIGTERM");
  }
});

test("inspection evidence fits right and bottom docks at wide and forced-color narrow widths", async ({ page }) => {
  const server = startServer();
  try {
    await page.goto(await server.url);
    await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
    await page.getByRole("button", { name: "Inspect", exact: true }).click();
    await page.locator("#deep-inspection > summary").click();
    const panel = page.locator("#inspector");
    const fits = () => panel.evaluate(node => {
      const bounds = node.getBoundingClientRect();
      return node.scrollWidth <= node.clientWidth + 1 &&
        [...node.querySelectorAll('.truth-grid > section')].every(section => {
          const card = section.getBoundingClientRect();
          return card.left >= bounds.left && card.right <= bounds.right;
        });
    });
    await expect.poll(fits).toBe(true);
    await page.getByRole("button", { name: "Move Inspect to bottom", exact: true }).click();
    await expect(panel).toHaveAttribute("data-furniture-dock", "bottom");
    const bottom = await panel.boundingBox();
    expect(bottom.width).toBeGreaterThan(page.viewportSize().width * .8);
    expect(bottom.height).toBeLessThanOrEqual(page.viewportSize().height * .5);
    await expect.poll(fits).toBe(true);
    await page.setViewportSize({ width: 390, height: 1000 });
    await page.emulateMedia({ forcedColors: "active", reducedMotion: "reduce" });
    await expect.poll(fits).toBe(true);
    expect(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)).toBe(false);
  } finally {
    server.lines.close();
    server.process.kill("SIGTERM");
  }
});

test("bottom inspection remains above workspace tools with reachable enlarged controls", async ({ page }) => {
  const server = startServer();
  try {
    await page.goto(await server.url);
    await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
    await page.getByRole("button", { name: "Inspect", exact: true }).click();
    await page.getByRole("button", { name: "Move Inspect to bottom", exact: true }).click();
    await expect(page.locator("#inspector")).toHaveAttribute("data-furniture-dock", "bottom");
    const drawerCoversTools = () => page.evaluate(() => {
      const drawer = document.querySelector('#inspector');
      const a = drawer.getBoundingClientRect();
      const b = document.querySelector('#workspace-controls').getBoundingClientRect();
      const left = Math.max(a.left, b.left), right = Math.min(a.right, b.right);
      const top = Math.max(a.top, b.top), bottom = Math.min(a.bottom, b.bottom);
      return right > left && bottom > top && drawer.contains(document.elementFromPoint((left + right) / 2, (top + bottom) / 2));
    });
    await expect.poll(drawerCoversTools).toBe(true);
    await page.setViewportSize({ width: 390, height: 1000 });
    await page.addStyleTag({ content: 'html{font-size:32px!important}' });
    await page.emulateMedia({ forcedColors: "active", reducedMotion: "reduce" });
    const controls = await page.locator('#inspector .furniture-bar button').evaluateAll(nodes => nodes.map(node => {
      const r = node.getBoundingClientRect();
      return { label: node.textContent, height: r.height, left: r.left, right: r.right,
        reachable: node.contains(document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2)) };
    }));
    for (const control of controls) {
      expect(control.height, control.label).toBeGreaterThanOrEqual(44);
      expect(control.left, control.label).toBeGreaterThanOrEqual(0);
      expect(control.right, control.label).toBeLessThanOrEqual(390);
      expect(control.reachable, control.label).toBe(true);
    }
    const close = page.getByRole('button', { name: 'Close Inspect', exact: true });
    const bounds = await close.boundingBox();
    expect(bounds.height).toBeGreaterThanOrEqual(44);
    expect(bounds.x).toBeGreaterThanOrEqual(0);
    expect(bounds.x + bounds.width).toBeLessThanOrEqual(390);
    const heading = page.locator('#plan h2');
    await page.locator('#deep-inspection > summary').click();
    await heading.scrollIntoViewIfNeeded();
    expect(await heading.evaluate(node => {
      const r = node.getBoundingClientRect();
      return node.contains(document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2));
    })).toBe(true);
    await close.click();
    await expect(page.locator('#inspector')).toBeHidden();
    await expect(page.locator('#workspace-controls')).toBeVisible();
  } finally {
    server.lines.close();
    server.process.kill('SIGTERM');
  }
});

test("enlarged narrow product navigation precedes status and shell bands do not overlap", async ({ page }) => {
  const server = startServer();
  try {
    await page.setViewportSize({ width: 390, height: 1100 });
    await page.goto(await server.url);
    await expect(page.locator('body')).toHaveAttribute('data-application-ready', 'true');
    await page.addStyleTag({ content: 'html{font-size:32px!important}' });
    await page.emulateMedia({ forcedColors: 'active', reducedMotion: 'reduce' });
    const bandsFit = () => page.evaluate(() => {
      const masthead = document.querySelector('[data-application-slot="product-masthead"]').getBoundingClientRect();
      const nav = document.querySelector('nav[aria-label="Patchbay workspace"]').getBoundingClientRect();
      const heading = document.querySelector('.canvas-heading').getBoundingClientRect();
      const product = document.querySelector('[data-application-key="product-navigation"]').getBoundingClientRect();
      const status = document.querySelector('[data-application-key="product-status"]').getBoundingClientRect();
      return product.bottom <= status.top + 1 && nav.top >= masthead.bottom && heading.top >= nav.bottom;
    });
    await expect.poll(bandsFit).toBe(true);
    const links = page.locator('[data-application-key="product-navigation"] a');
    await expect(links).toHaveCount(3);
    for (const link of await links.all()) {
      await link.scrollIntoViewIfNeeded();
      await link.focus();
      expect(await link.evaluate(node => {
        const r = node.getBoundingClientRect();
        return r.height >= 44 && r.left >= 0 && r.right <= innerWidth &&
          node.contains(document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2));
      })).toBe(true);
    }
    await page.locator('[data-application-key="product-status"]').scrollIntoViewIfNeeded();
    const fit = page.getByRole('button', { name: 'Fit', exact: true });
    await fit.click();
    expect(await fit.evaluate(node => {
      const r = node.getBoundingClientRect();
      return r.height >= 44 && node.contains(document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2));
    })).toBe(true);
    expect(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)).toBe(false);
  } finally {
    server.lines.close(); server.process.kill('SIGTERM');
  }
});
