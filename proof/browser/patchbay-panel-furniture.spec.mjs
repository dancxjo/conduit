import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { readFile, writeFile } from "node:fs/promises";
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

test("Escape dismisses the focused surface when Body inspection leaves Library open", async ({ page }, testInfo) => {
  const server = startServer();
  try {
    await page.goto(await server.url);
    await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
    await page.locator("#toggle-palette").click();
    await page.locator("#body-summary").click();
    await expect(page.locator("#palette")).toBeVisible();
    await expect(page.locator("#inspector")).toBeVisible();
    await page.locator("#library-query").focus();
    await page.keyboard.press("Escape");
    await expect(page.locator("#palette")).toBeHidden();
    await expect(page.locator("#inspector")).toBeVisible();
    await expect(page.locator("#toggle-palette")).toBeFocused();
    await page.locator("#inspector .furniture-bar button").first().focus();
    await page.screenshot({ path: testInfo.outputPath("focused-inspection.png") });
    await page.keyboard.press("Escape");
    await expect(page.locator("#inspector")).toBeHidden();
    await expect(page.locator("#toggle-inspector")).toBeFocused();
  } finally {
    server.lines.close();
    server.process.kill("SIGTERM");
  }
});

test("Patchbay graph consumes canonical typography and surface roles", async ({ page }, testInfo) => {
  const server = startServer();
  try {
    await page.goto(await server.url);
    await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
    const before = await page.evaluate(async () => (await (await fetch('/api/snapshot')).json()).face);
    await page.getByRole("button", { name: "Plot", exact: true }).click();
    await expect(page.locator("body")).toHaveCSS("font-family", /Georgia/);
    await expect(page.locator(".flow-frontplate header").first()).toHaveCSS("font-family", /Georgia/);
    await expect(page.locator(".react-flow__node").filter({ hasText: "text/upper" })).toBeVisible();
    await page.getByRole("button", { name: "Fit", exact: true }).click();
    const tokens = await page.locator("body").evaluate(node => {
      const style = getComputedStyle(node);
      return ['reading-paper', 'workbench-canvas', 'bootstrap-surface'].map(role => style.getPropertyValue(`--conduit-${role}`).trim());
    });
    expect(tokens).toEqual(['#0C121C', '#05070B', '#090D16']);
    const after = await page.evaluate(async () => (await (await fetch('/api/snapshot')).json()).face);
    expect(after).toEqual(before);
    await expect.poll(() => page.evaluate(() => {
      const graph = document.querySelector('#flow-root').getBoundingClientRect();
      const heading = document.querySelector('.canvas-heading').getBoundingClientRect();
      return graph.top >= heading.bottom && graph.height > 0;
    })).toBe(true);
    await expect.poll(() => page.locator('#flow-root').evaluate(root => {
      const bounds = root.getBoundingClientRect();
      return [...root.querySelectorAll('.react-flow__node')].every(node => {
        const rect = node.getBoundingClientRect();
        return rect.top >= bounds.top && rect.bottom <= bounds.bottom && rect.left >= bounds.left && rect.right <= bounds.right;
      });
    })).toBe(true);
    await page.screenshot({ path: testInfo.outputPath('canonical-look-graph.png') });
  } finally {
    server.lines.close();
    server.process.kill("SIGTERM");
  }
});

test("workspace status and tools reserve canvas space instead of covering nodes", async ({ page }, testInfo) => {
  const server = startServer();
  try {
    await page.goto(await server.url);
    await expect(page.locator("body")).toHaveAttribute("data-application-ready", "true");
    const clearance = () => page.evaluate(() => {
      const graph = document.querySelector('#flow-root').getBoundingClientRect();
      return ['current-status', 'temporal-context', 'front-door-actions', 'workspace-controls', 'flow-notices'].every(id => {
        const node = document.getElementById(id);
        if (node.hidden) return true;
        return node.getBoundingClientRect().top >= graph.bottom;
      });
    });
    await expect.poll(clearance).toBe(true);
    await page.locator('#workspace-controls > summary').click();
    await expect(page.getByRole('textbox', { name: 'Workspace document', exact: true })).toBeAttached();
    await expect.poll(clearance).toBe(true);
    await page.getByRole('textbox', { name: 'Layout name', exact: true }).focus();
    await expect(page.getByRole('textbox', { name: 'Layout name', exact: true })).toBeInViewport();
    await page.locator('#workspace-controls > summary').click();
    await page.getByRole('button', { name: 'Plot', exact: true }).click();
    await expect(page.locator('.react-flow__node').filter({ hasText: 'text/upper' })).toBeVisible();
    await page.getByRole('button', { name: 'Fit', exact: true }).click();
    await expect(page.locator('#flow-notices')).toContainText('omitted from diagram. Exact Face facts remain available.');
    await page.screenshot({ path: testInfo.outputPath('reserved-status-wide.png') });
    await page.setViewportSize({ width: 390, height: 1000 });
    await page.addStyleTag({ content: 'html{font-size:32px!important}' });
    await page.emulateMedia({ forcedColors: 'active', reducedMotion: 'reduce' });
    await expect.poll(clearance).toBe(true);
    await expect(page.locator('#current-status')).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)).toBe(false);
    await page.locator('#workspace-controls > summary').click();
    await page.getByRole('textbox', { name: 'Layout name', exact: true }).focus();
    await expect(page.getByRole('textbox', { name: 'Layout name', exact: true })).toBeInViewport();
    await page.screenshot({ path: testInfo.outputPath('reserved-status-narrow.png') });
  } finally {
    server.lines.close();
    server.process.kill('SIGTERM');
  }
});


test("enlarged narrow shell retains a useful scrollable graph viewport", async ({ page }, info) => {
  const server=startServer();
  try {
    await page.setViewportSize({width:390,height:844});
    await page.goto(await server.url);
    await expect(page.locator('body')).toHaveAttribute('data-application-ready','true');
    await page.getByRole('button',{name:'Plot',exact:true}).click();
    await expect(page.locator('.flow-frontplate').first()).toBeVisible();
    await page.getByRole('combobox',{name:'Look',exact:true}).selectOption('conduit.look/navy@1');
    await page.addStyleTag({content:'html{font-size:32px!important}'});
    await page.emulateMedia({forcedColors:'active',reducedMotion:'reduce'});
    await expect(page.locator('html')).toHaveCSS('font-size','32px');
    await page.getByRole('button',{name:'Arrange',exact:true}).click();
    const overlaps=()=>page.locator('.flow-frontplate').evaluateAll(nodes=>{
      const rects=nodes.map(node=>node.getBoundingClientRect());let count=0;
      for(let a=0;a<rects.length;a++)for(let b=a+1;b<rects.length;b++)
        if(Math.min(rects[a].right,rects[b].right)-Math.max(rects[a].left,rects[b].left)>1 && Math.min(rects[a].bottom,rects[b].bottom)-Math.max(rects[a].top,rects[b].top)>1)count++;
      return count;
    });
    await expect.poll(overlaps).toBe(0);
    await expect.poll(()=>page.locator('.flow-frontplate').evaluateAll(nodes=>nodes.every(node=>{
      const card=node.getBoundingClientRect(),box=node.closest('.react-flow__node').getBoundingClientRect();
      return ["left","top","right","bottom"].every(edge=>Math.abs(card[edge]-box[edge])<=1);
    }))).toBe(true);
    const baseline=await (await page.request.get(`${await server.url}/api/snapshot`)).json();
    const graph=page.locator('#flow-root');
    await graph.scrollIntoViewIfNeeded();
    const geometry=await graph.evaluate(node=>{
      const r=node.getBoundingClientRect(),top=Math.max(0,r.top),bottom=Math.min(innerHeight,r.bottom);
      return {viewport:{width:innerWidth,height:innerHeight},graph:{top:r.top,bottom:r.bottom,width:r.width,height:r.height},
        visibleHeight:Math.max(0,bottom-top),documentHeight:document.documentElement.scrollHeight,
        horizontalOverflow:document.documentElement.scrollWidth>innerWidth,
        graphHitSamples:[.2,.5,.8].map(fraction=>node.contains(document.elementFromPoint(r.left+r.width/2,top+(bottom-top)*fraction)))};
    });
    await writeFile(info.outputPath('narrow-graph-geometry.json'),JSON.stringify(geometry,null,2));
    await page.screenshot({path:info.outputPath('narrow-graph.png'),fullPage:true,animations:'disabled'});
    await page.screenshot({path:info.outputPath('narrow-graph-viewport.png'),animations:'disabled'});
    expect(geometry.graph.height).toBeGreaterThanOrEqual(320);
    expect(geometry.visibleHeight).toBeGreaterThanOrEqual(320);
    expect(geometry.documentHeight).toBeGreaterThan(geometry.viewport.height);
    expect(geometry.horizontalOverflow).toBe(false);
    expect(geometry.graphHitSamples).toEqual([true,true,true]);
    const tools=page.locator('#workspace-controls > summary');
    await tools.focus();await expect(tools).toBeFocused();await expect(tools).toBeInViewport();
    const current=await (await page.request.get(`${await server.url}/api/snapshot`)).json();
    expect(current.face).toEqual(baseline.face);
    await graph.scrollIntoViewIfNeeded();
    await page.screenshot({path:info.outputPath('narrow-graph-returned.png'),fullPage:true,animations:'disabled'});
  } finally {server.lines.close();server.process.kill('SIGTERM');}
});
