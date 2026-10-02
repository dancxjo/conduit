import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { chromium } from "@playwright/test";
import { startStaticProduct } from "./static-product-server.mjs";

const [siteRoot, reportRoot] = process.argv.slice(2);
assert(siteRoot && reportRoot && process.argv.length === 4,
  "usage: node proof/browser/verify-public-site.mjs SITE_ROOT OUTPUT_REPORTDIR");
const output = path.resolve(reportRoot);
await mkdir(output, { recursive: true });
const report = { status: "running", environment: "Chromium", pages: [], checkedLinks: [], excludedLinks: [] };
const navigation = [
  ["Home", "/conduit/"], ["Journeys", "/conduit/journeys/"],
  ["Handbook", "/conduit/handbook/"], ["Downloads", "/conduit/#get-conduit"],
  ["Open Conduit", "/conduit/workspace/"],
];
const checked = new Set();
const excluded = new Set();
let server;
let browser;
try {
  server = await startStaticProduct(path.resolve(siteRoot), "/conduit/");
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();
  const page = await context.newPage();
  page.setDefaultTimeout(15_000);
  page.setDefaultNavigationTimeout(30_000);
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  await page.goto(new URL("journeys/", server.url).href);
  const fieldLinks = await page.locator('a[href*="field-station-clock"]').evaluateAll(links => links.map(link => link.href));
  assert(fieldLinks.length > 0, "Journeys must link to the Field Station journey");
  const fieldUrl = new URL(fieldLinks[0]);
  assert.equal(fieldUrl.origin, new URL(server.url).origin, "Field Station must be a local staged page");
  assert(fieldUrl.pathname.startsWith("/conduit/journeys/"), "Field Station must belong to Journeys");
  const manifestUrl = new URL("index.json", fieldUrl);
  const manifestResponse = await context.request.get(manifestUrl.href);
  assert(manifestResponse.ok(), `Missing documentary manifest: ${manifestUrl.pathname}`);
  const journey = await manifestResponse.json();
  assert.equal(journey.schema, "conduit.user-journey/v1");
  assert.equal(journey.steps.length, 3, "Field Station must retain all three observed steps");
  assert.deepEqual(journey.steps.map(step => step.id), ["open", "reload", "lull"]);
  const routes = [
    ["home", server.url], ["journeys", new URL("journeys/", server.url).href],
    ["workspace", new URL("workspace/", server.url).href],
    ["three-bodies", new URL("journeys/current/three-bodies/", server.url).href],
    ["little-life", new URL("journeys/current/little-life/", server.url).href],
    ["handbook", new URL("handbook/", server.url).href],
    ["start-here", new URL("handbook/Start-here.html", server.url).href],
    ["field-station", fieldUrl.href], ["current-product", new URL("current-product.html", server.url).href],
    ["handbook-journey", new URL("journeys/verticals/handbook/", server.url).href],
  ];
  for (const [layout, width] of [["desktop", 1280], ["mobile", 390]]) {
    await page.setViewportSize({ width, height: 900 });
    for (const [id, url] of routes) {
      errors.length = 0;
      const response = await page.goto(url);
      assert(response?.ok(), `${layout} ${id}: navigation failed`);
      if (id === "handbook-journey") {
        const captures = page.locator('.journey-steps img');
        assert.equal(await captures.count(), 7, "Handbook journey retains every captured action");
        await decodeImages(captures);
      }
      if (id === "workspace") {
        await page.getByRole("button", { name: "Birth Body", exact: true }).waitFor();
        assert.equal(await page.getByRole("main").count(), 1, "Workspace main landmark");
      }
      if (["handbook", "start-here"].includes(id)) {
        await page.waitForFunction(() => Boolean(globalThis.__conduitApplication));
        const truth = await page.evaluate(() => globalThis.__conduitApplication.snapshot());
        for (const identity of ["hostId", "bootId", "bodyId", "planId", "playId"]) assert(truth[identity], `${id}: missing live ${identity}`);
      }
      const nav = page.getByRole("navigation", { name: "Main navigation", exact: true });
      assert.equal(await nav.count(), 1, `${id}: one common navigation required`);
      await nav.waitFor({ state: "visible" });
      for (const [label, href] of navigation) {
        const link = nav.getByRole("link", { name: label, exact: true });
        assert.equal(await link.count(), 1, `${id}: missing navigation ${label}`);
        assert(await link.isVisible(), `${layout} ${id}: navigation ${label} hidden`);
        assert.equal(await link.getAttribute("href"), href, `${id}: wrong ${label} destination`);
      }
      if (id === "current-product") await page.locator('#product-truth[aria-busy="false"]').waitFor();
      if (id === "three-bodies") await verifyThreeBodies(page);
      if (id === "little-life") {
        assert.equal(await page.getByRole("heading", { name: "Little Life", exact: true }).count(), 1);
        const images = page.locator("main img");
        assert(await images.count() > 0, "Little Life checkpoints are absent");
        await decodeImages(images);
        const scrub = page.getByRole("slider", { name: "Accepted generation checkpoint", exact: true });
        await scrub.focus();
        await scrub.press("End");
        assert.equal(await page.locator("#life-label").innerText(), "Generation 32");
        await decodeImages(page.locator("#life-frame"));
        assert((await page.locator("#life-frame").getAttribute("src")).endsWith("t032.png"));
      }
      if (id === "field-station") {
        const text = (await page.locator("main").innerText()).replace(/\s+/g, " ");
        const images = await page.locator("main img").evaluateAll(async images => {
          await Promise.all(images.map(image => image.decode()));
          return images.map(image => ({ src: image.src, width: image.naturalWidth }));
        });
        for (const step of journey.steps) {
          assert(text.includes(step.action.replace(/\s+/g, " ")), `Missing action: ${step.id}`);
          assert(text.includes(step.observation.replace(/\s+/g, " ")), `Missing observation: ${step.id}`);
          const expected = new URL(step.screenshot, fieldUrl).href;
          assert.equal(images.filter(image => image.src === expected && image.width > 0).length, 1,
            `${layout}: missing loaded documentary screenshot ${step.id}`);
        }
      }
      await page.evaluate(() => document.fonts.ready);
      const dimensions = await page.evaluate(() => ({
        viewport: document.documentElement.clientWidth,
        content: Math.max(document.documentElement.scrollWidth, document.body.scrollWidth),
      }));
      assert(dimensions.content <= dimensions.viewport + 1,
        `${layout} ${id}: horizontal overflow ${dimensions.content} > ${dimensions.viewport}`);
      assert.deepEqual(errors, [], `${id}: uncaught browser errors`);
      const links = await page.locator("a[href]").evaluateAll(links => links.map(link => link.href));
      for (const href of links) {
        const link = new URL(href);
        if (link.origin !== new URL(server.url).origin) continue;
        link.hash = "";
        const known = routes.some(([, route]) => new URL(route).pathname === link.pathname)
          || link.pathname === "/conduit/workspace/"
          || link.pathname.startsWith("/conduit/handbook/")
          || link.pathname.startsWith(fieldUrl.pathname)
          || link.pathname.startsWith("/conduit/journeys/current/three-bodies/")
          || link.pathname.startsWith("/conduit/journeys/current/little-life/");
        if (!known) {
          if (!excluded.has(link.pathname)) report.excludedLinks.push({ path: link.pathname,
            reason: "Outside the documentary page routes; runtime distributions and historical evidence are validated separately." });
          excluded.add(link.pathname);
          continue;
        }
        if (checked.has(link.href)) continue;
        const result = await context.request.get(link.href);
        assert(result.ok(), `Broken local link ${link.pathname}: HTTP ${result.status()}`);
        checked.add(link.href);
        report.checkedLinks.push({ path: link.pathname, status: result.status() });
      }
      let screenshot;
      if (["home", "journeys", "field-station", "handbook", "workspace", "three-bodies", "little-life"].includes(id)) {
        screenshot = `${layout}-${id}.png`;
        await page.screenshot({ path: path.join(output, screenshot), fullPage: true });
      }
      report.pages.push({ id, layout, width, path: new URL(url).pathname, screenshot, ...dimensions });
    }
  }
  await context.close();
  report.status = "passed";
} catch (error) {
  report.status = "failed";
  report.error = error.stack ?? String(error);
  process.exitCode = 1;
} finally {
  await browser?.close();
  if (server?.child && server.child.exitCode === null && server.child.signalCode === null) {
    await new Promise(resolve => {
      const timeout = setTimeout(() => { server.child.kill("SIGKILL"); resolve(); }, 2_000);
      server.child.once("exit", () => { clearTimeout(timeout); resolve(); });
      server.child.kill();
    });
  }
  await writeFile(path.join(output, "report.json"), `${JSON.stringify(report, null, 2)}\n`);
}


async function decodeImages(images) {
  for (const image of await images.all()) {
    await image.scrollIntoViewIfNeeded();
    assert(await image.evaluate(async element => {
      await element.decode();
      return element.naturalWidth > 0 && element.naturalHeight > 0;
    }), "Documentary image did not decode");
  }
}

async function verifyThreeBodies(page) {
  const chapters = page.getByRole("navigation", { name: "Journey chapters", exact: true });
  assert.deepEqual(await chapters.locator("a").allTextContents(), ["01 Open", "02 Start", "03 Use", "04 Recover", "05 Finish"]);
  for (const name of ["open", "start", "use", "recover", "finish"]) {
    const link = chapters.locator(`a[href="#chapter-${name}"]`);
    assert(await link.isVisible(), `Missing chapter link ${name}`);
    await link.click();
    assert.equal(new URL(page.url()).hash, `#chapter-${name}`);
    assert(await page.locator(`#chapter-${name}`).isVisible());
  }
  const browser = page.getByRole("button", { name: "Browser", exact: true });
  assert.equal(await browser.getAttribute("aria-pressed"), "true", "Browser must be initial track");
  const images = page.locator('.body-panel[data-body="browser-graphical"] img:visible');
  assert(await images.count() > 0, "Browser recording has no visible screenshots");
  await decodeImages(images);
  for (const label of ["ConduitOS", "Conversational", "Browser"]) {
    const button = page.getByRole("button", { name: label, exact: true });
    const track = await button.getAttribute("data-view");
    await button.click();
    assert.equal(await button.getAttribute("aria-pressed"), "true");
    const visible = await page.locator(".body-panel:visible").evaluateAll(panels => panels.map(panel => panel.dataset.body));
    assert(visible.length > 0 && visible.every(value => value === track), `Wrong visible ${label} track`);
  }
}
