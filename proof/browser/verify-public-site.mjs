import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
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
  if (existsSync(path.join(siteRoot, "journeys/current/one-body-five-masks/index.html"))) {
    routes.push(["one-body", new URL("journeys/current/one-body-five-masks/", server.url).href]);
  }
  const thermostatIncluded = existsSync(path.join(siteRoot, "journeys/current/thermostat/index.html"));
  if (thermostatIncluded) routes.push(["thermostat", new URL("journeys/current/thermostat/", server.url).href]);
  const sharedSpeechIncluded = existsSync(path.join(siteRoot, "evidence/shared-speech-carrier/index.html"));
  if (existsSync("site/evidence/shared-speech-carrier/manifest.json")) {
    assert(sharedSpeechIncluded, "Retained shared speech evidence must be staged into the public site");
  }
  if (sharedSpeechIncluded) routes.push(["shared-speech", new URL("evidence/shared-speech-carrier/", server.url).href]);
  const oneBodyManifestPath = path.join(siteRoot, "journeys/current/one-body-five-masks/manifest.json");
  const oneBodyDevelopment = existsSync(oneBodyManifestPath)
    && JSON.parse(readFileSync(oneBodyManifestPath, "utf8")).result === "diagnostic-incomplete";
  for (const [layout, width] of [["desktop", 1280], ["mobile", 390]]) {
    await page.setViewportSize({ width, height: 900 });
    for (const [id, url] of routes) {
      errors.length = 0;
      const response = await page.goto(url);
      assert(response?.ok(), `${layout} ${id}: navigation failed`);
      if (id === "handbook") {
        const shell = await response.text();
        assert(shell.includes('<header class="site-header">'), "Handbook navigation must be in its static HTML");
        for (const stylesheet of ["conduit.css", "chrome.css", "application.css"]) {
          assert(shell.includes(`href="${stylesheet}"`), `Handbook shell lacks ${stylesheet}`);
        }
      }
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
      if (id === "shared-speech") report.sharedSpeech = await verifySharedSpeechPage(page, context);
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
      if (id === "journeys" && thermostatIncluded) {
        const card = page.locator('.journey-card').filter({ hasText: 'Control one thermostat across Masks' });
        assert.equal(await card.count(), 1, "Thermostat journey needs one catalogue card");
        assert((await card.innerText()).includes("spoken"), "Thermostat card must describe its spoken encounter");
        assert.equal(await card.getByRole('link').getAttribute('href'), 'current/thermostat/');
      }
      if (id === "thermostat") {
        const content = page.locator('main.thermostat-journey');
        assert.equal(await content.locator('.journey-step').count(), 8, "Thermostat needs every observed chapter");
        assert.equal(await content.locator('#speech audio').count(), 1, "Thermostat needs its actual recorded output");
        await decodeImages(content.locator('img'));
        assert.equal(await content.locator('#inspect img').getAttribute('src'), 'capture/thermostat-after-terminal.png',
          'Browser return must show the captured terminal result');
        assert.equal(await content.locator('#inspect img').getAttribute('alt'), 'Browser observes the terminal’s Heat and Eco request');
        const recording = await content.locator('#speech audio').getAttribute('src');
        const response = await context.request.head(new URL(recording, page.url()).href);
        assert(response.ok() && Number(response.headers()['content-length']) > 44, "Thermostat recording is absent");
        assert.equal(response.headers()['content-type'], 'audio/wav');
        assert((await content.innerText()).includes('Settings are session-local.'), "Thermostat must disclose session reset");
        assert((await content.innerText()).includes('No physical sensor, heating or cooling is confirmed.'), "Thermostat must disclose equipment limits");
        await content.getByText('Source, receipts and limits', { exact: true }).click();
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
      if (id === "one-body") {
        const content = page.locator(oneBodyDevelopment ? "main.proof" : "main.journey");
        assert.equal(await content.count(), 1, "One Body page lacks its expected main content");
        if (oneBodyDevelopment) {
          assert((await content.innerText()).includes("not the complete eight-chapter public journey"),
            "Development recording must state its incomplete proof boundary");
          assert.equal(await content.locator("article").count(), 13,
            "Development recording needs every captured action chapter");
        } else {
          assert.equal(await content.locator("article").count(), 8,
            "One Body journey needs all eight chapters");
        }
        const images = content.locator("img");
        assert(await images.count() > 0, "One Body page needs live screenshots");
        await decodeImages(images);
        assert(await content.locator("pre").count() > 0, "One Body page needs a terminal capture");
        const audio = content.locator("audio");
        assert(await audio.count() >= 2, "One Body page needs selected direct and model speech");
        const players = oneBodyDevelopment
          ? [content.locator("#owner-selected-speech audio").first(),
            content.locator("#owner-llm-speech audio").first()]
          : ["Direct mechanical reading", "Finite model-assisted wording"].map(mode =>
            content.locator(".audio-card").filter({ hasText: mode }).first().locator("audio"));
        for (const player of players) {
          assert.equal(await player.count(), 1, "One Body page lacks a selected listener WAV");
          const src = await player.getAttribute("src");
          assert(src, "One Body audio has no source");
          const response = await context.request.head(new URL(src, page.url()).href);
          assert(response.ok() && Number(response.headers()["content-length"]) > 44,
            `One Body audio is missing or empty: ${src}`);
          assert.equal(response.headers()["content-type"], "audio/wav", `One Body audio MIME differs: ${src}`);
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
          || link.pathname.startsWith("/conduit/journeys/current/little-life/")
          || link.pathname.startsWith("/conduit/journeys/current/one-body-five-masks/")
          || link.pathname.startsWith("/conduit/evidence/shared-speech-carrier/");
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
      if (["home", "journeys", "field-station", "handbook", "workspace", "three-bodies", "little-life", "thermostat", "shared-speech"].includes(id)) {
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

async function verifySharedSpeechPage(page, context) {
  const main = page.locator("main.speech-proof");
  assert.equal(await main.getByRole("heading", { level: 1 }).innerText(), "Shared speech carrier: “you would”");
  const text = await main.innerText();
  assert(text.includes("NoAdmittedAcousticBack"));
  assert(text.includes("No physical-device playback, human listening validation or trained voice is claimed."));
  const audio = main.locator("audio");
  assert.equal(await audio.count(), 1);
  assert.notEqual(await audio.getAttribute("controls"), null);
  assert.equal(await audio.getAttribute("preload"), "none");
  assert.equal(await audio.getAttribute("autoplay"), null);
  assert.equal(await audio.getAttribute("src"), "you-would.wav");
  assert.equal(await audio.getAttribute("aria-label"), "Offline formant specimen: you would");
  async function json(name) {
    const response = await context.request.get(new URL(name, page.url()).href);
    assert(response.ok(), `Missing shared speech ${name}`);
    return response.json();
  }
  const manifest = await json("manifest.json");
  const publication = await json("publication.json");
  assert.equal(manifest.schema, "conduit.speech/shared-carrier-evidence@1");
  assert.equal(publication.schema, "conduit.speech/shared-carrier-publication@1");
  assert.match(publication.staged_source_commit, /^[a-f0-9]{40}$/);
  assert.match(manifest.producer_code_commit, /^[a-f0-9]{40}$/);
  assert.equal(publication.producer_code_commit, manifest.producer_code_commit);
  assert.deepEqual(publication.files, manifest.files);
  if (process.env.CONDUIT_CHECKOUT_SHA) assert.equal(publication.staged_source_commit, process.env.CONDUIT_CHECKOUT_SHA);
  for (const flag of ["human_listening", "device_playback", "trained_weights"]) assert.equal(manifest[flag], false);
  assert.deepEqual(Object.keys(manifest.files).sort(), ["you-would.json", "you-would.native", "you-would.wav"]);
  for (const name of ["manifest.json", "publication.json", ...Object.keys(manifest.files)]) {
    const link = main.locator(`a[href="${name}"]`);
    assert.equal(await link.count(), 1);
    assert(await link.isVisible(), `Shared speech download hidden: ${name}`);
  }
  for (const [name, expected] of Object.entries(manifest.files)) {
    const response = await context.request.get(new URL(name, page.url()).href);
    assert(response.ok(), `Missing shared speech file ${name}`);
    const bytes = await response.body();
    assert.equal(bytes.length, expected.bytes, `${name}: served size`);
    assert.equal(createHash("sha256").update(bytes).digest("hex"), expected.sha256, `${name}: served hash`);
    if (name.endsWith(".wav")) assert.equal(response.headers()["content-type"], "audio/wav");
  }
  return { stagedSource: publication.staged_source_commit, producer: manifest.producer_code_commit,
    files: manifest.files, scope: "Browser controls, layout and served bytes; no playback or human listening validation" };
}
