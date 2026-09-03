import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { installB7Devices } from "./b7-fixture.mjs";
import { openBookStep, startBook, startStaticProduct } from "./book-test-server.mjs";
import { downloadArtifact, sha256 } from "./download-artifact.mjs";

let entrance;

function browserApplicationPackageDigest(manifest) {
  const lines = [
    "conduit.browser/application-package-content@1",
    `application\0${manifest.application_id}`,
    `state\0${manifest.state_compatibility.identity}\0${manifest.state_compatibility.version}`,
  ];
  for (const implementation of manifest.host_implementations) lines.push(`host-implementation\0${implementation}`);
  for (const resource of manifest.resources) {
    const dependencies = resource.dependencies.map(({ role, specifier }) => `${role}=${specifier}`).join(",");
    lines.push(`resource\0${resource.role}\0${resource.kind}\0${resource.path}\0${resource.maximum_bytes}\0${resource.sha256}\0${dependencies}`);
  }
  return `sha256:${createHash("sha256").update(`${lines.join("\n")}\n`).digest("hex")}`;
}

async function startCreche() {
  const child = spawn("target/debug/conduit-browser-host", ["--application", "target/creche-product", "--mount", "/creche/", "--no-open"], {
    cwd: new URL("../..", import.meta.url).pathname,
    env: process.env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`Crèche was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/CONDUIT_BROWSER_HOST_URL=(http:\/\/127\.0\.0\.1:\d+\/creche\/)/);
      if (match) { clearTimeout(timeout); resolve(match[1]); }
    };
    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("exit", (code) => { clearTimeout(timeout); reject(new Error(`Crèche exited (${code})\n${output}`)); });
  });
  return { child, url };
}

async function openStep(page, index) {
  await openBookStep(page, entrance, index);
}

async function openStandaloneCreche(page) {
  entrance.child.kill();
  entrance = await startCreche();
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
}

async function birthStandaloneBody(page, { attachFirstHost = false, sourceVariant = null } = {}) {
  await openStandaloneCreche(page);
  const birth = page.locator(".body-birth-runner");
  if (sourceVariant) {
    const sourceDisclosure = birth.locator('[data-application-key="seed-source"]');
    await sourceDisclosure.locator("summary").click();
    const source = sourceDisclosure.getByRole("textbox", { name: "Conduit Seed source" });
    await source.fill((await source.inputValue()).replace('"SOS"', `"SOS ${sourceVariant}"`));
  }
  await birth.getByRole("button", { name: "Birth Body" }).click();
  const identity = await birth.evaluate((element) => ({
    bodyId: element.dataset.bodyId,
    birthSignId: element.dataset.birthSignId,
  }));
  if (attachFirstHost) {
    await page.getByRole("button", { name: "2. First Host" }).click();
    await page.getByRole("button", { name: "Give this Body its first Host" }).click();
  }
  return identity;
}

async function installEsp32Release(page, { id, chipId, releaseName, headerOffset = 0 }) {
  const bytes = Buffer.alloc(headerOffset + 1500);
  for (let index = headerOffset; index < bytes.length; index += 1) bytes[index] = (index - headerOffset) & 0xff;
  bytes[headerOffset] = 0xe9;
  bytes.writeUInt16LE(chipId, headerOffset + 12);
  const artifactName = `esp32-${releaseName}-generic-release.bin`;
  const manifestName = `esp32-${releaseName}-generic-release.json`;
  await page.route(`**/artifacts/${artifactName}`, (route) => route.fulfill({
    status: 200,
    contentType: "application/octet-stream",
    body: bytes,
  }));
  await page.route(`**/artifacts/${manifestName}`, (route) => route.fulfill({
    status: 200,
    contentType: "application/json",
    body: JSON.stringify({
      schema: "conduit.release/target-artifact@1",
      target_id: id,
      image_id: `conduit-release/${releaseName}@fixture`,
      source_identity: "git:browser-proof-fixture",
      artifact_layout: { format: "espressif-merged-image", flash_offset: 0 },
      artifact_sha256: `sha256:${createHash("sha256").update(bytes).digest("hex")}`,
      bytes: bytes.length,
      segments: [{ offset: 0, path: `./${artifactName}`, bytes: bytes.length }],
    }),
  }));
  return bytes;
}

async function installHostRelease(page, manifestName) {
  const root = new URL("../../target/creche-product/artifacts/", import.meta.url);
  const manifest = JSON.parse(await readFile(new URL(manifestName, root), "utf8"));
  for (const file of manifest.files) {
    const bytes = await readFile(new URL(file.path, root));
    await page.route(`**/artifacts/${file.path}`, (route) => route.fulfill({
      status: 200,
      contentType: file.media_type,
      body: bytes,
    }));
  }
  await page.route(`**/artifacts/${manifestName}`, (route) => route.fulfill({
    status: 200,
    contentType: "application/json",
    body: JSON.stringify(manifest),
  }));
  return manifest;
}

test.beforeEach(async () => {
  entrance = await startBook();
});

test.afterEach(() => entrance?.child.kill());

test("every Book page and Crèche step has a direct, history-aware route", async ({ page }) => {
  const bookPages = [
    ["a-form-you-can-run", "A Form you can run"],
    ["faces-backs-and-implementation", "Faces, Backs, and implementation"],
    ["hosts-make-forms-real", "Hosts make Forms real"],
    ["one-form-across-several-hosts", "One Form across several Hosts"],
    ["the-body-one-computer-one-machine-or-many", "The Body: one computer, one machine or many"],
    ["many-forms-one-body-wide-realization", "Many Forms, one Body-wide realization"],
    ["birth-spores-and-the-creche", "Birth, spores, and the Crèche"],
  ];
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  await expect(page.locator('#host-state [data-application-component="success-status"]')).toHaveText("Browser Host ready");
  for (let index = 0; index < bookPages.length; index += 1) {
    const [slug, title] = bookPages[index];
    await expect(page).toHaveURL(new RegExp(`/book/${slug}/$`));
    await expect(page.getByRole("heading", { level: 1, name: title })).toBeVisible();
    if (index + 1 < bookPages.length) await page.getByRole("button", { name: "Next" }).click();
  }
  await page.reload();
  await expect(page).toHaveURL(new RegExp(`/book/${bookPages.at(-1)[0]}/$`));
  await expect(page.locator("#chapter")).toContainText("Birth, spores, and the Cr");
  await page.goBack();
  await expect(page).toHaveURL(new RegExp(`/book/${bookPages.at(-2)[0]}/$`));

  await openStandaloneCreche(page);
  await expect(page.locator('#host-state [data-application-component="success-status"]')).toHaveText("Crèche ready");
  await expect(page.locator('.creche-steps [data-application-component="navigation"]')).toBeVisible();
  const steps = ["birth", "first-host", "physical-host", "graduate"];
  await expect(page).toHaveURL(/\/creche\/birth\/$/);
  for (let index = 1; index < steps.length; index += 1) {
    await page.getByRole("button", { name: `${index + 1}.`, exact: false }).click();
    await expect(page).toHaveURL(new RegExp(`/creche/${steps[index]}/$`));
  }
  await page.reload();
  await expect(page.getByRole("heading", { level: 2, name: "Graduate" })).toBeVisible();
  await page.goBack();
  await expect(page).toHaveURL(/\/creche\/physical-host\/$/);
});

test("Book route mutation crosses the finite Browser Host operation boundary", async ({ page }) => {
  const [source, routing, manifest] = await Promise.all([
    page.request.get(new URL("book.mjs", entrance.url).href).then((response) => response.text()),
    page.request.get(new URL("book-routing.mjs", entrance.url).href).then((response) => response.text()),
    page.request.get(new URL("book.application.json", entrance.url).href).then((response) => response.json()),
  ]);
  expect(source).toContain('from "./book-routing.mjs"');
  expect(source).not.toMatch(/\bhistory\.(?:pushState|replaceState)\s*\(/);
  expect(routing).toContain('from "./browser-host-operations.mjs"');
  expect(routing).not.toMatch(/\bhistory\.(?:pushState|replaceState)\s*\(/);
  expect(manifest.resources.some((resource) => resource.role === "browser-host-operations")).toBe(true);
  expect(manifest.resources.some((resource) => resource.role === "book-routing")).toBe(true);

  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  await page.getByRole("button", { name: "Next" }).click();
  await expect(page).toHaveURL(/\/book\/faces-backs-and-implementation\/$/);
  await page.goBack();
  await expect(page).toHaveURL(/\/book\/a-form-you-can-run\/$/);
});

test("the Book navigation remains legible and interactive in both theme modes", async ({ page }) => {
  for (const colorScheme of ["dark", "light"]) {
    await page.emulateMedia({ colorScheme });
    await page.goto(entrance.url);
    await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
    await page.getByRole("button", { name: "Next" }).click();
    await page.mouse.move(0, 0);

    const navigation = page.locator('[data-application-slot="book-navigation"]');
    const progress = navigation.locator('[data-application-key="progress"]');
    const previous = navigation.getByRole("button", { name: "Previous" });
    const next = navigation.getByRole("button", { name: "Next" });
    const dark = colorScheme === "dark";

    await expect(navigation.locator('[data-application-component="navigation"]')).toHaveCSS(
      "background-color",
      dark ? "rgb(12, 18, 28)" : "rgb(255, 255, 255)",
    );
    await expect(progress).toHaveCSS("color", dark ? "rgb(233, 163, 37)" : "rgb(154, 91, 0)");
    await expect(previous).toHaveCSS("background-color", dark ? "rgb(5, 7, 11)" : "rgb(238, 245, 248)");
    await expect(previous).toHaveCSS("color", dark ? "rgb(147, 210, 247)" : "rgb(23, 54, 77)");
    await expect(next).toHaveCSS("background-color", dark ? "rgb(233, 163, 37)" : "rgb(154, 91, 0)");
    await expect(next).toHaveCSS("color", dark ? "rgb(5, 7, 11)" : "rgb(238, 245, 248)");

    await previous.hover();
    await expect(previous).toHaveCSS("border-color", dark ? "rgb(233, 163, 37)" : "rgb(154, 91, 0)");
    await page.keyboard.press("Tab");
    await previous.focus();
    await expect(previous).toHaveCSS("outline-color", dark ? "rgb(244, 196, 0)" : "rgb(119, 93, 0)");
  }
});

test("every executable listing uses the real Patchbay renderer for checked Form truth", async ({ page }) => {
  let projectedListings = 0;
  for (let pageIndex = 0; pageIndex < 7; pageIndex += 1) {
    await openStep(page, pageIndex);
    const runners = page.locator(".runner");
    const count = await runners.count();
    projectedListings += count;
    for (let runnerIndex = 0; runnerIndex < count; runnerIndex += 1) {
      const runner = runners.nth(runnerIndex);
      const patchbay = runner.locator(".compact-patchbay");
      const source = runner.locator('[data-application-component="textarea"]');
      await expect(runner.locator('[data-application-component="form-field"]')).toHaveCount(1);
      await expect(runner.locator('[data-application-component="field-label"]')).toHaveCount(1);
      await expect(runner.locator('[data-application-component="field-help"]')).toHaveCount(1);
      await expect(source).toHaveAttribute("data-application-action", "book.source.input");
      await expect(source).toHaveAttribute("maxlength", "65536");
      await expect(source).toHaveAttribute("aria-describedby", /application-\d+-description-4/);
      await expect(patchbay).toHaveAttribute("data-disposition", "accepted");
      await expect(patchbay.locator(".book-flow-root").first()).toHaveAttribute("data-renderer", "react-flow");
      await expect(patchbay.locator(".react-flow").first()).toBeVisible();
      await expect(patchbay.locator(".flow-faceplate").first()).toBeVisible();
      await expect(patchbay.locator(".compact-patchbay-text li").first()).toContainText("Gear");
      await expect(runner.locator(".exact-evidence")).not.toHaveAttribute("open", "");
      await expect(runner.locator(".exact-projection")).toContainText("Checked Form");
      await expect(runner.locator('.exact-projection [data-application-component="definition-table"]')).toBeAttached();
      await expect(patchbay).not.toContainText("Host ID");
      await expect(patchbay).not.toContainText("Implementation");
    }
  }
  expect(projectedListings).toBe(7);
});

test("same-named input and output Ports keep distinct animated Cords", async ({ page }) => {
  await openStep(page, 0);
  const patchbay = page.locator(".runner").first().locator(".compact-patchbay");
  await expect(patchbay.locator(".react-flow__node")).toHaveCount(3);
  await expect(patchbay.locator(".react-flow__edge")).toHaveCount(2);
  await expect(patchbay.locator(".react-flow__edge.animated")).toHaveCount(2);
  await expect(patchbay.locator(".react-flow__edge-path").first()).toHaveCSS(
    "animation-name",
    "conduit-cord-flow",
  );
  await expect(patchbay.locator(".react-flow__edge-text")).toHaveCount(0);
  await expect(patchbay.locator(".compact-patchbay-text")).toContainText(
    "meet-one-gear/words output text to meet-one-gear/change input text",
  );
  await expect(patchbay.locator(".compact-patchbay-text")).toContainText(
    "meet-one-gear/change output text to meet-one-gear/result input text",
  );

  await page.emulateMedia({ reducedMotion: "reduce" });
  await expect(patchbay.locator(".react-flow__edge-path").first()).toHaveCSS("animation-name", "none");
  await expect(patchbay.locator(".react-flow__edge-path").first()).toHaveCSS("stroke-dasharray", "none");
});

test("Book tour pairs narrative with Patchbay above source and output, then stacks coherently", async ({ page }) => {
  for (const width of [1440, 1600]) {
    await page.setViewportSize({ width, height: 1000 });
    await openStep(page, 1);
    const measures = await page.evaluate(() => ({
      copy: document.querySelector(".chapter-copy").getBoundingClientRect().width,
      workbench: document.querySelector(".book-workbench").getBoundingClientRect().width,
      graph: document.querySelector(".book-flow-root").getBoundingClientRect().width,
      copyLeft: document.querySelector(".chapter-copy").getBoundingClientRect().left,
      copyTop: document.querySelector(".chapter-copy").getBoundingClientRect().top,
      workbenchLeft: document.querySelector(".book-workbench").getBoundingClientRect().left,
      workbenchTop: document.querySelector(".book-workbench").getBoundingClientRect().top,
      patchbay: document.querySelector(".compact-patchbay").getBoundingClientRect().toJSON(),
      source: document.querySelector('[data-application-key="source-editor"]').getBoundingClientRect().toJSON(),
      result: document.querySelector(".result").getBoundingClientRect().toJSON(),
      scroll: document.documentElement.scrollWidth,
      viewport: innerWidth,
    }));
    expect(measures.copy).toBeLessThanOrEqual(740);
    expect(measures.copy / measures.workbench).toBeGreaterThan(.85);
    expect(measures.copy / measures.workbench).toBeLessThan(1.15);
    expect(measures.workbenchLeft).toBeGreaterThan(measures.copyLeft + measures.copy * .9);
    expect(Math.abs(measures.workbenchTop - measures.copyTop)).toBeLessThan(2);
    expect(measures.graph).toBeGreaterThan(300);
    expect(measures.patchbay.bottom).toBeLessThanOrEqual(measures.source.top + 1);
    expect(measures.patchbay.bottom).toBeLessThanOrEqual(measures.result.top + 1);
    expect(Math.abs(measures.source.top - measures.result.top)).toBeLessThan(2);
    expect(measures.scroll).toBeLessThanOrEqual(measures.viewport);
  }

  await page.setViewportSize({ width: 700, height: 1000 });
  await openStep(page, 1);
  const narrow = await page.evaluate(() => {
    const copy = document.querySelector(".chapter-copy").getBoundingClientRect();
    const workbench = document.querySelector(".book-workbench").getBoundingClientRect();
    const patchbay = document.querySelector(".compact-patchbay").getBoundingClientRect();
    const source = document.querySelector('[data-application-key="source-editor"]').getBoundingClientRect();
    const result = document.querySelector(".result").getBoundingClientRect();
    return {
      copyBottom: copy.bottom,
      workbenchTop: workbench.top,
      patchbayBottom: patchbay.bottom,
      sourceTop: source.top,
      sourceBottom: source.bottom,
      resultTop: result.top,
      scroll: document.documentElement.scrollWidth,
      viewport: innerWidth,
    };
  });
  expect(narrow.workbenchTop).toBeGreaterThanOrEqual(narrow.copyBottom - 1);
  expect(narrow.sourceTop).toBeGreaterThanOrEqual(narrow.patchbayBottom - 1);
  expect(narrow.resultTop).toBeGreaterThanOrEqual(narrow.sourceBottom - 1);
  expect(narrow.scroll).toBeLessThanOrEqual(narrow.viewport);

  const listing = page.locator(".runner").first().locator("textarea");
  await listing.focus();
  const beforeResize = await listing.evaluate((element) => {
    element.setSelectionRange(7, 7);
    return element.getBoundingClientRect().height;
  });
  await listing.evaluate((element) => { element.style.height = "28rem"; });
  await expect.poll(() => listing.evaluate((element) => element.getBoundingClientRect().height)).toBeGreaterThan(beforeResize);
  await page.setViewportSize({ width: 1200, height: 1000 });
  const interaction = await page.evaluate(() => ({
    activeLabel: document.activeElement?.labels?.[0]?.textContent,
    selection: document.activeElement?.selectionStart,
    scroll: document.documentElement.scrollWidth,
    viewport: innerWidth,
  }));
  expect(interaction.activeLabel).toBe("Conduit · editable");
  expect(interaction.selection).toBe(7);
  expect(interaction.scroll).toBeLessThanOrEqual(interaction.viewport);
});

test("Book Patchbay shows an invalid Form, marks its broken Cord, and explains the repair", async ({ page }) => {
  await openStep(page, 0);
  const runner = page.locator(".runner").first();
  const listing = runner.locator("textarea");
  const patchbay = runner.locator(".compact-patchbay");
  const original = await listing.inputValue();
  const originalSource = await patchbay.getAttribute("data-source-document-id");
  const highlight = runner.locator(".syntax-highlight");
  await expect(listing).toHaveAttribute("data-syntax-disposition", "accepted");
  await expect(highlight.locator(".syntax-keyword").first()).toHaveText("form");
  await expect(highlight.locator(".syntax-identity").first()).not.toBeEmpty();
  await expect(highlight).toHaveCSS("color", "rgb(233, 241, 236)");

  await listing.fill('form unfinished { value=text/literal("still typing');
  await expect(listing).toHaveAttribute("data-syntax-disposition", "accepted");
  await expect(highlight.locator(".syntax-string")).toHaveText('"still typing');
  await expect(patchbay).toHaveAttribute("data-disposition", "refused");
  await expect(patchbay.locator(".flow-faceplate")).toHaveCount(0);
  await expect(patchbay.locator(".compact-patchbay-refusal")).toContainText("parse compact Book Patchbay");

  await listing.fill(original.replace("change: text/upper", "change: text/morse(80)"));
  await expect(patchbay).toHaveAttribute("data-disposition", "invalid");
  await expect(patchbay.locator(".flow-faceplate")).toHaveCount(3);
  await expect(patchbay.locator(".flow-faceplate.diagnostic-error")).toHaveCount(2);
  await expect(patchbay.locator(".react-flow__edge.diagnostic-error")).toHaveCount(1);
  await expect(patchbay.locator(".compact-patchbay-diagnostic")).toContainText("CND-FRM-045");
  await expect(patchbay.locator(".compact-patchbay-diagnostic")).toContainText("How to fix:");
  await expect(patchbay.locator(".compact-patchbay-diagnostic")).toContainText("value/text@1");

  await listing.fill(original.replace('"hello"', '"latest"'));
  await expect(patchbay).toHaveAttribute("data-disposition", "accepted");
  await expect(patchbay).not.toHaveAttribute("data-source-document-id", originalSource);
  await expect(patchbay.locator("figcaption strong")).toHaveText("meet-one-gear");

  const oversized = `form oversized {\n${Array.from({ length: 17 }, (_, index) => `g${index}: text/literal("x")`).join("\n")}\n}`;
  await listing.fill(oversized);
  await expect(patchbay).toHaveAttribute("data-disposition", "refused");
  await expect(patchbay.locator(".compact-patchbay-refusal")).toContainText("Gear bound exceeded");
});

test("read-only Conduit prose examples use the admitted canonical syntax projection", async ({ page }) => {
  await openStep(page, 6);
  const example = page.locator('.syntax-example[aria-label="Read-only Conduit example"]');
  await expect(example).toHaveCount(1);
  await expect(example).toHaveAttribute("data-syntax-disposition", "accepted");
  await expect(example.locator(".syntax-keyword")).toHaveText("form");
  await expect(example.locator(".syntax-name").first()).toHaveText("morse-network");
  await expect(example.locator(".syntax-identity").first()).toHaveText("text/literal");
  await expect(example).toContainText("message > morse > light");

  await openStep(page, 5);
  await expect(page.locator(".concept-diagram")).toHaveCount(1);
  await expect(page.locator(".concept-diagram [class^=syntax-]")).toHaveCount(0);
});

test("Book Patchbay keeps branching explicit and opens one reviewed Back beneath its Face", async ({ page }) => {
  await openStep(page, 0);
  const fanout = page.locator(".runner").nth(2).locator(".compact-patchbay");
  await expect(fanout.locator(".react-flow__edge")).toHaveCount(4);
  await expect(fanout.locator(".compact-patchbay-text")).toContainText("branch-a-cord/source output text to branch-a-cord/loud input text");
  await expect(fanout.locator(".compact-patchbay-text")).toContainText("branch-a-cord/source output text to branch-a-cord/morse input text");

  await openStep(page, 1);
  const runner = page.locator(".runner");
  const patchbay = runner.locator(".compact-patchbay");
  const sourceBefore = await runner.locator("textarea").inputValue();
  const checkedBefore = await patchbay.getAttribute("data-checked-form-id");
  const sourceIdentityBefore = await patchbay.getAttribute("data-source-document-id");
  await expect(patchbay.locator(".faceplate-back-control")).toHaveCount(1);
  await patchbay.getByRole("button", { name: "Open reviewed Back for same-morse-caller/morse" }).click();
  const back = patchbay.locator(".gear-back-expansion");
  await expect(back).toBeVisible();
  await expect(back.locator(".gear-back-flow")).toHaveAttribute("data-renderer", "react-flow");
  expect(await back.locator(".flow-faceplate").count()).toBeGreaterThan(3);
  expect(await back.getAttribute("data-checked-form-id")).toBe(checkedBefore);
  expect(await back.getAttribute("data-source-document-id")).toBe(sourceIdentityBefore);
  await expect(runner.locator("textarea")).toHaveValue(sourceBefore);
  await back.getByRole("button", { name: "Return to Face" }).click();
  await expect(back).toBeHidden();
  await expect(patchbay.getByLabel("Real Patchbay canvas").locator(".flow-faceplate")).toHaveCount(3);
  await expect(runner.locator("textarea")).toHaveValue(sourceBefore);
});

test("the staged Book and Crèche each boot with only their own product tree", async ({ page }) => {
  const book = await startStaticProduct("target/book-product");
  try {
    await page.goto(`${book.url}a-form-you-can-run/`);
    await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
    await expect(page.locator(".book-flow-root").first()).toHaveAttribute("data-renderer", "react-flow");
    await expect(page.locator(".flow-faceplate").first()).toBeVisible();
    await expect(page.locator('meta[name="conduit-application-package"]')).toHaveAttribute("content", "./book.application.json");
    await expect.poll(() => page.evaluate(() => globalThis.__conduitBrowserApplication?.manifest.applicationId)).toBe("conduit.application/book");
    const exports = await page.evaluate(() => Object.keys(globalThis.__conduitBookHost.runtime));
    expect(exports.some((name) => name.startsWith("conduit_creche_"))).toBe(false);
    expect((await page.request.get(`${book.url}creche.mjs`)).status()).toBe(404);
  } finally {
    book.child.kill();
  }

  const creche = await startStaticProduct("target/creche-product", "/conduit/creche/");
  try {
    const requestedPaths = [];
    page.on("request", (request) => requestedPaths.push(new URL(request.url()).pathname));
    await page.addInitScript(() => {
      globalThis.__crecheDeviceAuthorityRequests = 0;
      for (const name of ["serial", "usb"]) {
        const method = name === "serial" ? "requestPort" : "requestDevice";
        Object.defineProperty(navigator, name, {
          configurable: true,
          value: { [method]: async () => { globalThis.__crecheDeviceAuthorityRequests += 1; throw new Error("unexpected device authority"); } },
        });
      }
    });
    await page.goto(`${creche.url}index.html`);
    await expect(page.locator("#host-state")).toHaveText("Crèche ready");
    const exports = await page.evaluate(() => Object.keys(globalThis.__conduitCrecheHost.runtime));
    expect(exports.some((name) => name.startsWith("conduit_book_"))).toBe(false);
    expect((await page.request.get(`${creche.url}chapter-1.md`)).status()).toBe(404);
    for (const target of ["c3", "s3", "wroom"]) {
      expect((await page.request.get(`${creche.url}artifacts/esp32-${target}-generic-release.json`)).status()).toBe(200);
      expect((await page.request.get(`${creche.url}artifacts/esp32-${target}-generic-release.bin`)).status()).toBe(200);
    }
    for (const artifact of ["avr-promicro-atmega32u4-5v-16mhz.json", "promicro-atmega32u4-5v-16mhz.hex"]) {
      expect((await page.request.get(`${creche.url}artifacts/${artifact}`)).status()).toBe(200);
    }
    for (const artifact of ["hosted-linux-x86_64.json", "conduit-linux-x86_64", "hosted-windows-x86_64.json", "conduit-windows-x86_64.exe", "hosted-macos-aarch64.json", "conduit-macos-aarch64", "browser-page.json", "runtime.wasm", "index.html", "host.mjs"]) {
      expect((await page.request.get(`${creche.url}artifacts/${artifact}`)).status()).toBe(200);
    }
    for (const artifact of ["orange-pi-5-image.json", "conduitos-orange-pi-5.img", "raspios-bookworm-pi4-model-b-rev-1.5-4gb.json", "raspios-bookworm-zero-2-w-rev-1.0.json", "raspios-bookworm-zero-2-wh-rev-1.0.json", "conduit-linux-aarch64", "rpi-b-plus-image.json", "conduitos-rpi-b-plus.img", "rpi-zero-v1-image.json", "conduitos-rpi-zero-v1.img", "rpi-zero-w-v1.1-image.json", "conduitos-rpi-zero-w-v1.1.img", "rpi-zero-wh-v1.1-image.json", "conduitos-rpi-zero-wh-v1.1.img"]) {
      expect((await page.request.get(`${creche.url}artifacts/${artifact}`)).status()).toBe(200);
    }
    for (const artifact of ["conduitos-x86_64-pc-release.json", "conduitos-x86_64-pc.iso", "conduitos-aarch64-virt-release.json", "conduitos-aarch64-virt.iso", "conduitos-ia32-pc-release.json", "conduitos-ia32-pc.iso", "conduitos-riscv64-virt-release.json", "conduitos-riscv64-virt.iso", "conduitos-loongarch64-virt-release.json", "conduitos-loongarch64-virt.iso"]) {
      expect((await page.request.get(`${creche.url}artifacts/${artifact}`)).status()).toBe(200);
    }
    const birth = page.locator(".body-birth-runner");
    await birth.getByRole("button", { name: "Birth Body" }).click();
    const stagedBodyId = await birth.getAttribute("data-body-id");
    await page.getByRole("button", { name: "3. Physical Host" }).click();
    const runner = page.locator(".physical-host-runner");
    await runner.locator(".physical-target").selectOption("esp32/riscv32imc/usb-dcf8355d-esp32-c3");
    await expect(runner.locator('[data-stage="obtain"]')).toHaveClass(/complete/);
    let evidence = JSON.parse(await runner.locator("details code").textContent());
    const c3Manifest = JSON.parse(await readFile(
      new URL("../../target/creche-product/artifacts/esp32-c3-generic-release.json", import.meta.url),
      "utf8",
    ));
    expect(evidence.obtainment).toMatchObject({
      target_id: c3Manifest.target_id,
      artifact: {
        image_id: c3Manifest.image_id,
        bytes: c3Manifest.bytes,
        release_source_identity: c3Manifest.source_identity,
        release_artifact_sha256: c3Manifest.artifact_sha256,
      },
    });
    expect(requestedPaths).toContain("/conduit/creche/artifacts/esp32-c3-generic-release.json");
    expect(requestedPaths).toContain("/conduit/creche/artifacts/esp32-c3-generic-release.bin");
    expect(requestedPaths).not.toContain("/conduit/artifacts/esp32-c3-generic-release.json");
    await runner.getByRole("button", { name: "Bind Body invitation" }).click();
    await expect(runner.locator('[data-stage="bind"]')).toHaveClass(/complete/);
    const spore = runner.locator('[data-application-key="download-spore"]');
    evidence = JSON.parse(await runner.locator("details code").textContent());
    expect(evidence.binding).toMatchObject({
      target_id: c3Manifest.target_id,
      image_content_digest: evidence.obtainment.artifact.content_digest,
    });
    expect(evidence.binding.image_id).toMatch(/^image:sha256:[0-9a-f]{64}$/);
    const downloadedSpore = await downloadArtifact(page, spore);
    expect(downloadedSpore.filename).toMatch(/-c3\.bin$/);
    const { readEsp32BodySpore } = await import("../../targets/esp32/browser-deployment/index.mjs");
    const artifact = {
      magic: new TextDecoder().decode(downloadedSpore.bytes.subarray(0, 8)),
      bytes: downloadedSpore.bytes.byteLength,
      contentDigest: await sha256(downloadedSpore.bytes),
      provision: readEsp32BodySpore(downloadedSpore.bytes),
    };
    expect(artifact).toMatchObject({
      bytes: 4 * 1024 * 1024,
      provision: {
        spore_id: evidence.binding.spore_id,
        image_id: evidence.binding.image_id,
        invitation_id: evidence.binding.invitation_id,
        body_id: evidence.binding.body_id,
      },
    });
    expect(artifact.magic).not.toBe("CNDSPOR1");
    expect(artifact.contentDigest).toBe(evidence.binding.spore_artifact.content_digest);
    expect(await page.evaluate(() => globalThis.__crecheDeviceAuthorityRequests)).toBe(0);
    await page.evaluate(() => globalThis.__conduitCrecheDurability.settled());

    await page.goto(`${creche.url}index.html`);
    await expect(page.locator("#host-state")).toHaveText("Crèche ready");
    await expect(page.locator(".body-birth-runner")).toHaveAttribute("data-body-id", stagedBodyId);
    await expect(page.locator(".body-birth-runner").getByRole("button", { name: "Birth Body" })).toBeDisabled();
    await page.getByRole("button", { name: "3. Physical Host" }).click();
    const conduitosRunner = page.locator(".physical-host-runner");
    await conduitosRunner.locator(".physical-target").selectOption("conduitos/x86_64/pc");
    await expect(conduitosRunner.locator('[data-stage="obtain"]')).toHaveClass(/complete/);
    await conduitosRunner.getByRole("button", { name: "Bind Body invitation" }).click();
    const conduitosDownload = conduitosRunner.locator('[data-application-key="download-spore"]');
    evidence = JSON.parse(await conduitosRunner.locator("details code").textContent());
    expect(evidence.binding).toMatchObject({
      target_id: "conduitos/x86_64/pc",
      output: "disk-image",
      fabrication_package_id: "conduitos-image@1",
      deployment_adapter: "conduit-host-conduitos/boot-x86_64@1",
    });
    const downloadedIso = await downloadArtifact(page, conduitosDownload);
    expect(downloadedIso.filename).toMatch(/-conduitos-native\.iso$/);
    const { readBodyProvisionedMedia } = await import("../../targets/browser/host/assets/creche-native-disk.mjs");
    const nativeIso = {
      isoMagic: new TextDecoder().decode(downloadedIso.bytes.subarray(32769, 32774)),
      contentDigest: await sha256(downloadedIso.bytes),
      provision: readBodyProvisionedMedia(downloadedIso.bytes).provision,
    };
    expect(nativeIso).toMatchObject({
      isoMagic: "CD001",
      provision: {
        spore: { spore_id: evidence.binding.spore_id, body_id: evidence.binding.body_id },
        invitation_provision: { invitation_id: evidence.binding.invitation_id },
      },
    });
    expect(nativeIso.contentDigest).toBe(evidence.binding.spore_artifact.content_digest);
    expect(requestedPaths).toContain("/conduit/creche/artifacts/conduitos-x86_64-pc-release.json");
    expect(requestedPaths).toContain("/conduit/creche/artifacts/conduitos-x86_64-pc.iso");
    expect(requestedPaths).not.toContain("/conduit/artifacts/conduitos-x86_64-pc-release.json");
    expect(await page.evaluate(() => globalThis.__crecheDeviceAuthorityRequests)).toBe(0);
  } finally {
    creche.child.kill();
  }

  const browserBundle = await startStaticProduct("target/creche-product/artifacts");
  try {
    await page.goto(`${browserBundle.url}index.html`);
    await expect(page).toHaveTitle("Conduit browser Host");
    await expect(page.locator("#status")).toHaveText("Current and independently initialized");
    await expect(page.locator("#identity")).toBeVisible();
    expect(await page.evaluate(() => globalThis.__conduitBrowserHost.hostId)).toMatch(/^browser\//);
  } finally {
    browserBundle.child.kill();
  }
});

test("a missing ESP32 release in the prefixed staged Crèche refuses before binding or device authority", async ({ page }) => {
  entrance.child.kill();
  entrance = null;
  const creche = await startStaticProduct("target/creche-product", "/conduit/creche/");
  const requestedPaths = [];
  page.on("request", (request) => requestedPaths.push(new URL(request.url()).pathname));
  await page.addInitScript(() => {
    globalThis.__crecheDeviceAuthorityRequests = 0;
    for (const name of ["serial", "usb"]) {
      const method = name === "serial" ? "requestPort" : "requestDevice";
      Object.defineProperty(navigator, name, {
        configurable: true,
        value: { [method]: async () => { globalThis.__crecheDeviceAuthorityRequests += 1; throw new Error("unexpected device authority"); } },
      });
    }
  });
  await page.route("**/conduit/creche/artifacts/esp32-c3-generic-release.json", (route) => route.fulfill({ status: 404 }));
  try {
    await page.goto(`${creche.url}index.html`);
    await expect(page.locator("#host-state")).toHaveText("Crèche ready");
    await page.locator(".body-birth-runner").getByRole("button", { name: "Birth Body" }).click();
    await page.getByRole("button", { name: "3. Physical Host" }).click();
    const runner = page.locator(".physical-host-runner");
    await runner.locator(".physical-target").selectOption("esp32/riscv32imc/usb-dcf8355d-esp32-c3");
    await expect(runner.locator("details code")).toContainText('"terminal": "ArtifactUnavailable"');
    const evidence = JSON.parse(await runner.locator("details code").textContent());
    expect(evidence).toMatchObject({ binding: null, realization: null, observation: null, admission: null });
    expect(evidence.terminal).toMatchObject({
      operation: "obtain",
      terminal: "ArtifactUnavailable",
      authority_requested: false,
      artifact_work_started: false,
    });
    await expect(runner.getByRole("button", { name: "Bind Body invitation" })).toBeDisabled();
    await expect(runner.locator('[data-application-key="download-spore"]')).toHaveCount(0);
    expect(requestedPaths).toContain("/conduit/creche/artifacts/esp32-c3-generic-release.json");
    expect(requestedPaths).not.toContain("/conduit/creche/artifacts/esp32-c3-generic-release.bin");
    expect(await page.evaluate(() => globalThis.__crecheDeviceAuthorityRequests)).toBe(0);
  } finally {
    creche.child.kill();
  }
});

test("the Book renders admitted Markdown emphasis semantically and leaves raw HTML inert", async ({ page }) => {
  const body = "# Markdown proof\n\n*asterisk* _underscore_ **strong asterisk** __strong underscore__ <img src=x onerror=globalThis.__rawHtmlRan=true>";
  await page.route("**/book/chapter-1.md", (route) => route.fulfill({
    contentType: "text/markdown; charset=utf-8",
    body,
  }));
  await page.route("**/book/book.application.json", async (route) => {
    const response = await route.fetch();
    const manifest = await response.json();
    manifest.resources.find((resource) => resource.role === "chapter-1").sha256 =
      `sha256:${createHash("sha256").update(body).digest("hex")}`;
    manifest.package_digest = browserApplicationPackageDigest(manifest);
    await route.fulfill({ response, contentType: "application/json", body: JSON.stringify(manifest) });
  });
  await openStep(page, 0);
  await expect(page.locator("em")).toHaveText(["asterisk", "underscore"]);
  await expect(page.locator("strong")).toHaveText(["strong asterisk", "strong underscore"]);
  await expect(page.locator("#chapter")).not.toContainText("*asterisk*");
  await expect(page.locator("#chapter")).not.toContainText("_underscore_");
  await expect(page.locator("#chapter img")).toHaveCount(0);
  await expect(page.locator("#chapter")).toContainText("<img src=x onerror=globalThis.__rawHtmlRan=true>");
  expect(await page.evaluate(() => globalThis.__rawHtmlRan)).toBeUndefined();
});

test("the Book opens with one logical Body premise and keeps Crèche machinery later", async ({ page }) => {
  const responses = [];
  page.on("response", (response) => responses.push(new URL(response.url()).pathname));
  await openStep(page, 0);
  await expect(page.getByRole("heading", { name: "A Form you can run" })).toBeVisible();
  await expect(page).toHaveTitle(/The Book$/);
  await expect(page.locator("#chapter")).toContainText("one logical computer");
  await expect(page.locator("#chapter")).toContainText("one or many physical or virtual computers");
  await expect(page.locator("#chapter")).toContainText("Gear, Port, Cord, Form");
  await expect(page.locator("#chapter")).not.toContainText(/Crèche/i);
  await expect(page.locator(".body-birth-runner, .first-host-runner, .physical-host-runner, .graduation-runner")).toHaveCount(0);
  await expect(page.locator('[data-application-slot="book-inventory"]')).toHaveCount(0);
  await expect(page.getByRole("link", { name: "Birth a Body" })).toHaveCount(0);

  await openStep(page, 1);
  await expect(page.getByRole("heading", { name: "Faces, Backs, and implementation" })).toBeVisible();
  await expect(page.locator("#chapter")).not.toContainText(/birth|admission/i);

  await openStep(page, 6);
  const handoff = page.getByRole("link", { name: "Birth a Body" });
  await expect(handoff).toHaveAttribute("href", "../creche/");
  await expect(handoff).toHaveJSProperty("href", new URL("../creche/", entrance.url).href);
  await expect(page.locator('meta[name="conduit-creche-url"]')).toHaveAttribute("content", "../creche/");
  const bookRuntimeExports = await page.evaluate(() => Object.keys(globalThis.__conduitBookHost.runtime));
  expect(bookRuntimeExports.some((name) => name.startsWith("conduit_creche_"))).toBe(false);
  expect((await page.request.get(new URL("/creche/", entrance.url).href)).status()).toBe(404);
  expect((await page.request.get(new URL("/book/creche-lifecycle.mjs", entrance.url).href)).status()).toBe(404);
  expect(responses.some((path) => path.includes("creche-lifecycle") || path.includes("creche-physical") || path.includes("creche-graduation"))).toBe(false);
  await page.reload();
  await expect(page.getByRole("heading", { name: "Birth, spores, and the Crèche" })).toBeVisible();
  await expect(page.locator(".body-birth-runner")).toHaveCount(0);
});

test("the standalone Crèche runs the same durable birth and graduation path without Book assets", async ({ page }) => {
  entrance.child.kill();
  entrance = await startCreche();
  const responses = [];
  page.on("response", (response) => responses.push(new URL(response.url()).pathname));
  await page.goto(entrance.url);
  await expect(page).toHaveTitle("Conduit Crèche");
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  const birth = page.locator(".body-birth-runner");
  await expect(birth.locator('[data-application-key="body-program"]')).toHaveAttribute("data-application-component", "select");
  await expect(birth.locator('[data-application-key="morse-program"]')).toHaveAttribute("data-application-component", "option");
  await birth.getByLabel("Friendly Body name").fill("standalone firefly");
  await birth.getByRole("button", { name: "Birth Body" }).click();
  const bodyId = await birth.getAttribute("data-body-id");
  expect(bodyId).toMatch(/^[0-9a-f]{64}$/);
  await expect(page.locator('.creche-body-context [data-application-component="panel"]')).toContainText("standalone firefly");
  await page.getByRole("button", { name: "2. First Host" }).click();
  await expect(page.locator('[data-application-key="attach-host"]')).toHaveAttribute("data-application-action", "host.attach");
  await page.getByRole("button", { name: "Give this Body its first Host" }).click();
  await expect(page.locator('.first-host-runner [data-application-key="host-status"]')).toHaveAttribute("data-application-component", "success-status");
  await expect(page.locator('.first-host-runner [data-application-key="host-evidence"]')).toHaveAttribute("data-application-evidence", "succeeded");
  await page.getByRole("button", { name: "4. Graduate" }).click();
  await expect(page.locator('[data-application-key="without-patchbay"]')).toHaveAttribute("data-application-action", "graduate.without-patchbay");
  await page.getByRole("button", { name: "Finish without hosted Patchbay" }).click();
  await expect(page.locator('[data-application-key="graduation-status"]')).toHaveAttribute("data-application-component", "success-status");
  await expect(page.locator('[data-application-key="end-creche"]')).toHaveAttribute("data-application-action", "graduate.end");
  await expect(page.locator(".graduation-runner")).toHaveAttribute("data-body-id", bodyId);
  await expect(page.locator('.body-biography [data-application-key^="biography-record-"]')).toHaveCount(4);
  const durable = await page.evaluate(() => {
    const api = globalThis.__conduitCrecheHost.runtime;
    api.conduit_creche_biography();
    const bytes = new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len());
    return JSON.parse(new TextDecoder().decode(bytes));
  });
  expect(durable.body_id).toBe(bodyId);
  expect(durable.schema).toBe("conduit.body/biography-evidence@1");
  const crecheRuntimeExports = await page.evaluate(() => Object.keys(globalThis.__conduitCrecheHost.runtime));
  expect(crecheRuntimeExports.some((name) => name.startsWith("conduit_book_"))).toBe(false);
  expect((await page.request.get(new URL("/book/", entrance.url).href)).status()).toBe(404);
  expect(responses.some((path) => path.startsWith("/book/") || path.includes("chapter-"))).toBe(false);
});

test("the standalone Crèche birth controls remain separated at a narrow viewport", async ({ page }) => {
  entrance.child.kill();
  entrance = await startCreche();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  const runner = page.locator(".body-birth-runner");
  const [program, name, source, editor] = await Promise.all([
    runner.getByLabel("Initial program").boundingBox(),
    runner.getByLabel("Friendly Body name").boundingBox(),
    runner.locator('[data-application-key="seed-source"]').boundingBox(),
    runner.locator('[data-application-key="birth-editor"]').boundingBox(),
  ]);
  for (const box of [program, name, source, editor]) expect(box).not.toBeNull();
  expect(program.y + program.height).toBeLessThanOrEqual(name.y);
  expect(name.y + name.height).toBeLessThanOrEqual(source.y);
  for (const control of [program, name, source]) {
    expect(control.x).toBeGreaterThanOrEqual(editor.x);
    expect(control.x + control.width).toBeLessThanOrEqual(editor.x + editor.width);
  }
  await runner.locator('[data-application-key="seed-source"] summary').click();
  await expect(runner.locator('[data-application-key="seed-source"] textarea')).toBeVisible();
  const selectAppearance = await runner.getByLabel("Initial program").evaluate(
    (element) => getComputedStyle(element).appearance,
  );
  expect(selectAppearance).toBe("none");
});

test("the standalone Crèche physical Host selects use only the custom dropdown arrow", async ({ page }) => {
  entrance.child.kill();
  entrance = await startCreche();
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await birthStandaloneBody(page);
  await page.getByRole("button", { name: "3. Physical Host" }).click();
  const selects = page.locator(".physical-host-runner .physical-mode, .physical-host-runner .physical-target");
  await expect(selects).toHaveCount(2);
  const appearances = await selects.evaluateAll(
    (elements) => elements.map((element) => getComputedStyle(element).appearance),
  );
  expect(appearances).toEqual(["none", "none"]);
});

test("two Bodies seal distinct spores against the same verified packaged Pico IMAGE", async ({ page }) => {
  const prepareOne = async (variant) => {
    const birth = await birthStandaloneBody(page, { sourceVariant: variant });
    await page.getByRole("button", { name: "3. Physical Host" }).click();
    const runner = page.locator(".physical-host-runner");
    await expect(runner.locator("input[type=file]")).toHaveCount(0);
    await expect(runner.locator('[data-stage="obtain"]')).toHaveClass(/complete/);
    await runner.getByRole("button", { name: "Bind Body invitation" }).click();
    await expect(runner.locator('[data-stage="bind"]')).toHaveClass(/complete/);
    const downloaded = await downloadArtifact(page, runner.locator('[data-application-key="download-spore"]'));
    expect(downloaded.filename).toMatch(/-pico-w\.uf2$/);
    const { readRp2040BodySpore } = await import("../../targets/rp2040/browser-deployment/index.mjs");
    const artifact = {
      filename: downloaded.filename,
      bytes: downloaded.bytes.byteLength,
      contentDigest: await sha256(downloaded.bytes),
      provision: readRp2040BodySpore(downloaded.bytes),
    };
    return { birth, artifact, evidence: JSON.parse(await runner.locator("details code").textContent()) };
  };
  const first = await prepareOne("A");
  const second = await prepareOne("B");
  expect(first.birth.bodyId).not.toBe(second.birth.bodyId);
  expect(first.evidence.obtainment.artifact.content_digest).toBe("sha256:11e92a00aa1e1144faacfd25540426e57dd862b172595ef9197da02daf17ef8e");
  expect(first.evidence.obtainment.artifact.content_digest).toBe(second.evidence.obtainment.artifact.content_digest);
  expect(first.evidence.obtainment.artifact.artifact_id).toBe("conduit-pico-w-signal/pico-local-b8@1");
  expect(first.evidence.binding.spore_id).not.toBe(second.evidence.binding.spore_id);
  expect(first.evidence.binding.invitation_id).not.toBe(second.evidence.binding.invitation_id);
  expect(first.evidence.binding.image_content_digest).toBe(second.evidence.binding.image_content_digest);
  expect(first.artifact.contentDigest).toBe(first.evidence.binding.spore_artifact.content_digest);
  expect(first.artifact.provision).toMatchObject({
    spore_id: first.evidence.binding.spore_id,
    body_id: first.birth.bodyId,
    invitation_id: first.evidence.binding.invitation_id,
  });
  expect(first.artifact.contentDigest).not.toBe(second.artifact.contentDigest);
});

test("the same Crèche lifecycle consumes packaged and template-specialized fabrication", async ({ page }) => {
  const prepare = async (variant, strategy) => {
    await birthStandaloneBody(page, { sourceVariant: variant });
    await page.getByRole("button", { name: "3. Physical Host" }).click();
    const runner = page.locator(".physical-host-runner");
    await runner.locator(".fabrication-strategy").selectOption(strategy);
    await expect(runner.locator('[data-stage="obtain"]')).toHaveClass(/complete/);
    await runner.getByRole("button", { name: "Bind Body invitation" }).click();
    await expect(runner.locator('[data-stage="bind"]')).toHaveClass(/complete/);
    return JSON.parse(await runner.locator("details code").textContent());
  };
  const packaged = await prepare("fabrication-packaged", "packaged-exact");
  const specialized = await prepare("fabrication-specialized", "template-specialized");
  expect(packaged.obtainment.fabrication.strategy).toBe("packaged-exact");
  expect(specialized.obtainment.fabrication.strategy).toBe("template-specialized");
  expect(packaged.obtainment.artifact.content_digest).not.toBe(specialized.obtainment.artifact.content_digest);
  expect(packaged.binding.schema).toBe(specialized.binding.schema);
  expect(Object.keys(packaged.binding).sort()).toEqual(Object.keys(specialized.binding).sort());
  expect(packaged.binding.spore_id).not.toBe(specialized.binding.spore_id);
  expect(packaged.binding.body_id).not.toBe(specialized.binding.body_id);
});

test("the physical workflow renders one adapter-owned catalog without learning target mechanics", async ({ page }) => {
  await birthStandaloneBody(page);
  await page.getByRole("button", { name: "3. Physical Host" }).click();
  const runner = page.locator(".physical-host-runner");
  await expect(runner.locator('[data-stage="obtain"]')).toHaveClass(/complete/);

  const source = await (await page.request.get(new URL("./creche-physical.mjs", entrance.url).href)).text();
  const catalogSource = await (await page.request.get(new URL("./creche-target-catalog.mjs", entrance.url).href)).text();
  expect(source).not.toMatch(/rp2040|pico|webusb|webserial|\busb\b|serial|uf2|picoboot|baud|vendor.?id|product.?id|flash/i);
  expect(catalogSource).not.toMatch(/rp2040|pico|webusb|webserial|\busb\b|serial|uf2|picoboot|baud|vendor.?id|product.?id|flash/i);
  await expect(runner.locator(".physical-mode option")).toHaveCount(3);
  await expect(runner.locator(".physical-target optgroup")).toHaveCount(8);
  await expect(runner.locator(".physical-target optgroup").nth(0)).toHaveAttribute("label", "RP2040 boards");
  await expect(runner.locator(".physical-target optgroup").nth(1)).toHaveAttribute("label", "SparkFun Pro Micro");
  await expect(runner.locator(".physical-target optgroup").nth(2)).toHaveAttribute("label", "ESP32 boards");
  await expect(runner.locator(".physical-target optgroup").nth(3)).toHaveAttribute("label", "Hosted computers");
  await expect(runner.locator(".physical-target optgroup").nth(4)).toHaveAttribute("label", "Browser Hosts");
  await expect(runner.locator(".physical-target optgroup").nth(5)).toHaveAttribute("label", "Orange Pi computers");
  await expect(runner.locator(".physical-target optgroup").nth(6)).toHaveAttribute("label", "Raspberry Pi computers");
  await expect(runner.locator(".physical-target optgroup").nth(7)).toHaveAttribute("label", "ConduitOS machines");
  await expect(runner.locator(".physical-target option")).toHaveText([
    "Raspberry Pi Pico W · RP2040",
    "Pro Micro · ATmega32U4 · 5 V / 16 MHz",
    "ESP32-C3",
    "ESP32-S3",
    "ESP32-WROOM-32 · HW-463",
    "Hosted computer · Linux · x86_64",
    "Hosted computer · Windows · x86_64",
    "Hosted computer · macOS · arm64",
    "Browser page Host",
    "Orange Pi 5 · RK3588S · bare-metal ConduitOS",
    "Pi 4 Model B rev 1.5 (4 GB) · Raspberry Pi OS Bookworm 64-bit",
    "Pi Zero 2 W rev 1.0 · Raspberry Pi OS Bookworm 64-bit",
    "Pi Zero 2 WH rev 1.0 · Raspberry Pi OS Bookworm 64-bit",
    "Model B+ v1.2 · ARMv6 · bare-metal ConduitOS",
    "Pi Zero v1 · ARMv6 · bare-metal ConduitOS",
    "Pi Zero W v1.1 · ARMv6 · bare-metal ConduitOS",
    "Pi Zero WH v1.1 · ARMv6 · bare-metal ConduitOS",
    "x86_64 PC · product Host",
    "AArch64 virt · product Host",
    "IA-32 PC · product Host",
    "RISC-V64 virt · product Host",
    "LoongArch64 virt · product Host",
  ]);
  await expect(runner.locator(".physical-target")).toHaveValue("conduitos/thumbv6m/pico-w");

  let evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.catalog).toMatchObject({
    schema: "conduit.creche/physical-host-target-catalog@1",
    generation: 1,
  });
  expect(evidence.target_entry).toMatchObject({
    family: { id: "conduit-target-family/rp2040@1", label: "RP2040 boards" },
    target: {
      id: "conduitos/thumbv6m/pico-w",
      model_id: "raspberry-pi/pico-w@1",
      profile_id: "pico-local",
    },
    expected_join_contract: "conduit.rp2040/browser-spawn-observation@1",
  });
  expect(evidence.target_entry.fabrication_strategies.map(({ id }) => id)).toEqual([
    "packaged-exact",
    "template-specialized",
  ]);
  expect(evidence.target_entry.intentions).toEqual([
    { id: "fabricate-new", resultKind: "artifact", supported: true },
    { id: "install-existing", resultKind: "installation", supported: false },
    { id: "attach-running", resultKind: "attachment", supported: false },
  ]);
  expect(evidence.target_entry.carriers).toMatchObject({
    deployment: [{ id: "conduit-carrier/browser-picoboot@1" }],
    installation: [],
    attachment: [],
    observation: [{ id: "conduit-carrier/browser-serial-spawn@1" }],
  });
  expect(evidence.target_entry.bounds).toMatchObject({
    maximumOperations: 16,
    maximumOperationEvidenceBytes: 32768,
    maximumRetainedEvidenceBytes: 131072,
  });

  await runner.locator(".physical-mode").selectOption("install-existing");
  await expect(runner.locator("details code")).toContainText('"terminal": "UnsupportedCombination"');
  evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.intention).toMatchObject({ mode: "install-existing", result_kind: "installation", supported: false });
  expect(evidence.terminal).toMatchObject({
    mode: "install-existing",
    result_kind: "installation",
    authority_requested: false,
    artifact_work_started: false,
  });
  expect(evidence.admitted_operations).toBe(1);

  await runner.locator(".physical-mode").selectOption("attach-running");
  await expect(runner.locator("details code")).toContainText('"terminal": "UnsupportedCombination"');
  evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.intention).toMatchObject({ mode: "attach-running", result_kind: "attachment", supported: false });
  expect(evidence.terminal).toMatchObject({ authority_requested: false, artifact_work_started: false });
  expect(evidence.admitted_operations).toBe(1);
});

test("an exact browser release becomes a Body-bound spore and a newly admitted browser Host", async ({ page }) => {
  const release = await installHostRelease(page, "browser-page.json");
  const birth = await birthStandaloneBody(page, { sourceVariant: "browser-existing-computer" });
  await page.getByRole("button", { name: "3. Physical Host" }).click();
  const runner = page.locator(".physical-host-runner");
  await runner.locator(".physical-target").selectOption("browser/wasm32/page");
  await expect(runner.locator(".physical-mode")).toHaveValue("install-existing");
  await runner.getByRole("button", { name: "Review Host" }).click();
  await expect(runner.locator('[data-stage="obtain"]')).toHaveClass(/complete/);
  let evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.target_entry).toMatchObject({
    family: { id: "conduit-target-family/browser@1" },
    target: { id: "browser/wasm32/page", profile_id: "browser-wasm32-page" },
    intentions: [
      { id: "fabricate-new", supported: false },
      { id: "install-existing", supported: true },
      { id: "attach-running", supported: false },
    ],
    carriers: {
      installation: [
        { id: "conduit-carrier/browser-release-download@1" },
        { id: "conduit-carrier/browser-local-sandbox@1" },
      ],
      observation: [{ id: "conduit-carrier/browser-local-spawn@1" }],
    },
  });
  expect(evidence.obtainment).toMatchObject({
    target_id: "browser/wasm32/page",
    distribution_sha256: release.bundle_sha256,
    builder_adapter: "conduit-host-browser/bind-prebuilt@1",
    compiler_started: false,
    build_id: expect.stringMatching(/^build:sha256:/),
    image_id: expect.stringMatching(/^image:sha256:/),
  });
  expect(evidence.obtainment.image_content_digest).not.toBe(release.bundle_sha256);
  expect(evidence.obtainment.bundle_sha256).toBe(evidence.obtainment.image_content_digest);

  await runner.getByRole("button", { name: "Bind Body invitation" }).click();
  await expect(runner.locator('[data-stage="bind"]')).toHaveClass(/complete/);
  const bundleHandoff = runner.locator('[data-application-key="download-spore"]');
  await expect(bundleHandoff).toContainText("Download ZIP");
  evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.binding).toMatchObject({
    body_id: birth.bodyId,
    target_id: "browser/wasm32/page",
    output: "browser-bundle",
    fabrication_package_id: "browser-wasm@1",
    deployment_adapter: "conduit-host-browser/load@1",
    image_content_digest: evidence.obtainment.image_content_digest,
  });
  const downloadedBundle = await downloadArtifact(page, bundleHandoff);
  expect(downloadedBundle.filename).toMatch(/-browser-wasm32-page\.zip$/);
  const { readBodyBoundZip } = await import("../../targets/browser/host/assets/creche-native-zip.mjs");
  const archive = readBodyBoundZip(downloadedBundle.bytes);
  const bundle = {
    magic: new TextDecoder().decode(downloadedBundle.bytes.subarray(0, 4)),
    contentDigest: await sha256(downloadedBundle.bytes),
    files: Array.from(archive.entries.keys()),
    provision: archive.provision,
  };
  expect(bundle.magic).toBe("PK\u0003\u0004");
  expect(bundle.files).toContain("runtime.wasm");
  expect(bundle.files).toContain("conduit-browser-image.json");
  expect(bundle.provision).toMatchObject({
    schema: "conduit.spore/native-package-provision@1",
    spore: { spore_id: evidence.binding.spore_id, body_id: birth.bodyId },
    invitation_provision: { invitation_id: evidence.binding.invitation_id },
  });
  expect(bundle.contentDigest).toBe(evidence.binding.spore_artifact.content_digest);

  await runner.getByRole("button", { name: "Realize selected Host" }).click();
  await expect(runner.locator('[data-stage="realize"] span')).toHaveText("BrowserBundleLoaded");
  await runner.getByRole("button", { name: "Observe Boot and join" }).click();
  await expect(runner.locator('[data-stage="observe"]')).toHaveClass(/complete/);
  await runner.getByRole("button", { name: "Admit Part and offers" }).click();
  await expect(runner.locator('[data-stage="admit"]')).toHaveClass(/complete/);
  evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.realization).toMatchObject({
    terminal: "BrowserBundleLoaded",
    target_id: "browser/wasm32/page",
    artifact_content_digest: evidence.binding.spore_artifact.content_digest,
    boot_observed: false,
    join_created: false,
  });
  expect(evidence.observation).toMatchObject({
    schema: "conduit.browser/creche-spawn-observation@1",
    spore_id: evidence.binding.spore_id,
    image_id: evidence.binding.image_id,
  });
  expect(evidence.admission).toMatchObject({
    disposition: "admitted",
    body_id: birth.bodyId,
    offers_observed: true,
    ready: true,
    plan_id: null,
    active_play_id: null,
  });
});

test("a native Linux target produces an exact spore but refuses to invent an installer", async ({ page }) => {
  const release = await installHostRelease(page, "hosted-linux-x86_64.json");
  await birthStandaloneBody(page, { sourceVariant: "native-existing-computer" });
  await page.getByRole("button", { name: "3. Physical Host" }).click();
  const runner = page.locator(".physical-host-runner");
  await runner.locator(".physical-target").selectOption("std/x86_64/computer");
  await expect(runner.locator(".physical-mode")).toHaveValue("install-existing");
  await expect(runner.locator('[data-stage="obtain"]')).toHaveClass(/complete/);
  await runner.getByRole("button", { name: "Bind Body invitation" }).click();
  const hostedHandoff = runner.locator('[data-application-key="download-spore"]');
  await expect(hostedHandoff).toContainText("Download ZIP");
  let evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.target_entry.target_profile).toMatchObject({
    os: "linux",
    architecture: "x86_64",
    machine: "computer",
    role_profile: null,
    package_id: "hosted-native@1",
    implemented_carriers: ["conduit-carrier/browser-release-download@1"],
  });
  expect(evidence.target_entry.target_profile.platform_variants).toEqual([
    expect.objectContaining({ os: "linux", architecture: "x86_64", status: "supported" }),
    expect.objectContaining({ os: "linux", architecture: "aarch64", status: "planned" }),
    expect.objectContaining({ os: "windows", architecture: "x86_64", status: "supported" }),
    expect.objectContaining({ os: "macos", architecture: "aarch64", status: "supported" }),
  ]);
  expect(evidence.binding).toMatchObject({
    target_id: "std/x86_64/computer",
    output: "native-bundle",
    fabrication_package_id: "hosted-native@1",
    image_content_digest: release.bundle_sha256,
    spore_artifact: {
      format: "zip",
      media_type: "application/zip",
      image_content_digest: release.bundle_sha256,
      files: expect.arrayContaining([
        expect.objectContaining({ path: "conduit-linux-x86_64", mode: 0o100755 }),
      ]),
    },
  });
  const downloadedHosted = await downloadArtifact(page, hostedHandoff);
  expect(downloadedHosted.filename).toMatch(/-hosted-linux-x86_64\.zip$/);
  const { readBodyBoundZip } = await import("../../targets/browser/host/assets/creche-native-zip.mjs");
  const hostedArchive = readBodyBoundZip(downloadedHosted.bytes);
  const hostedPackage = {
    magic: new TextDecoder().decode(downloadedHosted.bytes.subarray(0, 4)),
    contentDigest: await sha256(downloadedHosted.bytes),
    files: Array.from(hostedArchive.entries.keys()),
    provision: hostedArchive.provision,
  };
  expect(hostedPackage).toMatchObject({
    magic: "PK\u0003\u0004",
    provision: {
      spore: { spore_id: evidence.binding.spore_id, body_id: evidence.binding.body_id },
      invitation_provision: { invitation_id: evidence.binding.invitation_id },
    },
  });
  expect(hostedPackage.files).toEqual(["conduit-linux-x86_64", "conduit-spore.json"]);
  expect(hostedPackage.contentDigest).toBe(evidence.binding.spore_artifact.content_digest);
  await runner.getByRole("button", { name: "Realize selected Host" }).click();
  await expect(runner.locator("details code")).toContainText('"terminal": "ExplicitInstallerRequired"');
  evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence).toMatchObject({ realization: null, observation: null, admission: null });
  expect(evidence.terminal).toMatchObject({
    operation: "realize",
    terminal: "ExplicitInstallerRequired",
    ambient_credentials_used: false,
    ambient_addresses_used: false,
    external_work_started: false,
    unavailable_carriers: ["explicit-helper", "ssh", "package-manager", "container"],
  });
});

test("Windows and macOS native releases are exact selectable Crèche targets", async ({ page }) => {
  const profiles = [
    {
      id: "std/x86_64/windows-computer",
      profileId: "hosted-windows-x86_64",
      manifest: "hosted-windows-x86_64.json",
      os: "windows",
      architecture: "x86_64",
      machine: "windows-computer",
      executable: "conduit-windows-x86_64.exe",
    },
    {
      id: "std/aarch64/macos-computer",
      profileId: "hosted-macos-aarch64",
      manifest: "hosted-macos-aarch64.json",
      os: "macos",
      architecture: "aarch64",
      machine: "macos-computer",
      executable: "conduit-macos-aarch64",
    },
  ];
  for (const [index, profile] of profiles.entries()) {
    const release = await installHostRelease(page, profile.manifest);
    await birthStandaloneBody(page, { sourceVariant: `native-${profile.os}-${index}` });
    await page.getByRole("button", { name: "3. Physical Host" }).click();
    const runner = page.locator(".physical-host-runner");
    await runner.locator(".physical-target").selectOption(profile.id);
    await expect(runner.locator('[data-stage="obtain"]')).toHaveClass(/complete/);
    await runner.getByRole("button", { name: "Bind Body invitation" }).click();
    const downloaded = await downloadArtifact(page, runner.locator('[data-application-key="download-spore"]'));
    expect(downloaded.filename).toMatch(new RegExp(`-${profile.profileId}\\.zip$`));
    const evidence = JSON.parse(await runner.locator("details code").textContent());
    expect(evidence.target_entry).toMatchObject({
      target: { id: profile.id, profile_id: profile.profileId },
      target_profile: {
        os: profile.os,
        architecture: profile.architecture,
        machine: profile.machine,
        package_id: "hosted-native@1",
      },
    });
    expect(evidence.binding).toMatchObject({
      target_id: profile.id,
      fabrication_package_id: "hosted-native@1",
      image_content_digest: release.bundle_sha256,
      spore_artifact: {
        files: expect.arrayContaining([
          expect.objectContaining({ path: profile.executable, mode: 0o100755 }),
        ]),
      },
    });
  }
});

test("the target-neutral Crèche consumes exact C3, then S3, then WROOM adapters without widening the family", async ({ page }) => {
  const profiles = [
    { id: "esp32/riscv32imc/usb-dcf8355d-esp32-c3", chipId: 5, profileId: "usb-dcf8355d-esp32-c3", releaseName: "c3" },
    { id: "esp32/xtensa-lx7/usb-54e2006398-esp32-s3", chipId: 9, profileId: "usb-54e2006398-esp32-s3", releaseName: "s3" },
    { id: "esp32/xtensa-lx6/hw-463-esp-wroom-32", chipId: 0, profileId: "hw-463-esp-wroom-32", releaseName: "wroom", headerOffset: 0x1000 },
  ];
  for (const [index, profile] of profiles.entries()) {
    await installEsp32Release(page, profile);
    await birthStandaloneBody(page, { sourceVariant: `esp32-${index}` });
    await page.getByRole("button", { name: "3. Physical Host" }).click();
    const runner = page.locator(".physical-host-runner");
    await runner.locator(".physical-target").selectOption(profile.id);
    await expect(runner.locator('[data-stage="obtain"]')).toHaveClass(/complete/);
    let evidence = JSON.parse(await runner.locator("details code").textContent());
    expect(evidence.target_entry.target.profile_id).toBe(profile.profileId);
    expect(evidence.target_entry.target_profile).toMatchObject({
      schema: "conduit.esp32/creche-target-profile@1",
      chip_id: profile.chipId,
      fabrication_strategy: expect.stringContaining("reviewed generic release IMAGE download"),
      expected_post_flash_join: "bounded serial spawn protocol 2",
      flash: {
        spore_region: {
          start: 4 * 1024 * 1024 - 4096,
          bytes: 4096,
          body_bound: true,
        },
      },
    });

    await runner.getByRole("button", { name: "Bind Body invitation" }).click();
    await expect(runner.locator('[data-stage="bind"]')).toHaveClass(/complete/);
    evidence = JSON.parse(await runner.locator("details code").textContent());
    const downloaded = await downloadArtifact(page, runner.locator('[data-application-key="download-spore"]'));
    expect(downloaded.filename).toMatch(new RegExp(`-${profile.releaseName}\\.bin$`));
    const { readEsp32BodySpore } = await import("../../targets/esp32/browser-deployment/index.mjs");
    const artifact = {
      magic: new TextDecoder().decode(downloaded.bytes.subarray(0, 8)),
      bytes: downloaded.bytes.byteLength,
      contentDigest: await sha256(downloaded.bytes),
      provision: readEsp32BodySpore(downloaded.bytes),
    };
    expect(artifact).toMatchObject({
      bytes: 4 * 1024 * 1024,
      provision: {
        spore_id: evidence.binding.spore_id,
        image_id: evidence.binding.image_id,
        invitation_id: evidence.binding.invitation_id,
        body_id: evidence.binding.body_id,
      },
    });
    expect(artifact.magic).not.toBe("CNDSPOR1");
    expect(artifact.contentDigest).toBe(evidence.binding.spore_artifact.content_digest);
    expect(evidence.binding).toMatchObject({
      target_id: profile.id,
      output: "esp32-image",
      fabrication_package_id: "conduit-host-esp32@1",
    });
    expect(evidence.binding.image_content_digest).toBe(evidence.obtainment.artifact.content_digest);
  }
});

test("an unavailable generic ESP32 release refuses before device authority or spore creation", async ({ page }) => {
  await page.route("**/artifacts/esp32-c3-generic-release.json", (route) => route.fulfill({ status: 404 }));
  await birthStandaloneBody(page, { sourceVariant: "esp32-release-absent" });
  await page.getByRole("button", { name: "3. Physical Host" }).click();
  const runner = page.locator(".physical-host-runner");
  await runner.locator(".physical-target").selectOption("esp32/riscv32imc/usb-dcf8355d-esp32-c3");
  await expect(runner.locator("details code")).toContainText('"terminal": "ArtifactUnavailable"');
  const evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence).toMatchObject({ binding: null, realization: null, observation: null, admission: null });
  expect(evidence.terminal).toMatchObject({
    operation: "obtain",
    terminal: "ArtifactUnavailable",
    authority_requested: false,
    artifact_work_started: false,
  });
});

test("the ESP32 Crèche adapter refuses a wrong serial port as its own terminal", async ({ page }) => {
  await page.addInitScript(() => {
    const port = new EventTarget();
    port.open = async () => {};
    port.close = async () => {};
    port.getInfo = () => ({ usbVendorId: 0x1234, usbProductId: 0x5678 });
    Object.defineProperty(navigator, "serial", {
      configurable: true,
      value: { requestPort: async () => port },
    });
  });
  await installEsp32Release(page, {
    id: "esp32/riscv32imc/usb-dcf8355d-esp32-c3",
    chipId: 5,
    releaseName: "c3",
  });
  await birthStandaloneBody(page, { sourceVariant: "esp32-wrong-port" });
  await page.getByRole("button", { name: "3. Physical Host" }).click();
  const runner = page.locator(".physical-host-runner");
  await runner.locator(".physical-target").selectOption("esp32/riscv32imc/usb-dcf8355d-esp32-c3");
  await expect(runner.locator('[data-stage="obtain"]')).toHaveClass(/complete/);
  await runner.getByRole("button", { name: "Bind Body invitation" }).click();
  await expect(runner.locator('[data-stage="bind"]')).toHaveClass(/complete/);
  await runner.getByRole("button", { name: "Realize selected Host" }).click();
  await expect(runner.locator("details code")).toContainText('"terminal": "WrongPort"');
  const evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.terminal).toMatchObject({ operation: "realize", terminal: "WrongPort" });
  expect(evidence.realization).toBeNull();
});

test("the physical target catalog refuses stale, duplicate, overflowing, and incompatible contributions", async ({ page }) => {
  await birthStandaloneBody(page);
  const refusals = await page.evaluate(async () => {
    const { createPhysicalHostTargetCatalog } = await import("/creche/creche-target-catalog.mjs");
    const entry = (suffix = "one", factory = null) => {
      const target = {
        id: `fixture/target-${suffix}`,
        label: `Fixture ${suffix}`,
        model_id: `fixture/model-${suffix}`,
        profile_id: `fixture/profile-${suffix}`,
      };
      const modes = [
        { id: "fabricate-new", resultKind: "artifact", supported: true },
        { id: "install-existing", resultKind: "installation", supported: false },
        { id: "attach-running", resultKind: "attachment", supported: false },
      ];
      const bounds = { maximumOperations: 5, maximumOperationEvidenceBytes: 1024, maximumRetainedEvidenceBytes: 4096 };
      const adapter = {
        schema: "conduit.creche/physical-host-target-adapter@1",
        target,
        modes,
        bounds,
        createOptions() { return null; },
        async obtain() {}, async bind() {}, async realize() {}, async observe() {}, async cancel() {},
      };
      return {
        schema: "conduit.creche/physical-host-target-entry@1",
        family: { id: "fixture/family", label: "Fixture family" },
        target,
        intentions: modes,
        fabrication_strategies: [{ id: "fixture/strategy", label: "Fixture strategy" }],
        carriers: { deployment: [], installation: [], attachment: [], observation: [] },
        bounds,
        expected_join_contract: "fixture/join@1",
        target_profile: { schema: "fixture/target-profile@1" },
        createAdapter: factory ?? (() => adapter),
      };
    };
    const evidence = {};
    for (const [name, create] of Object.entries({
      stale: () => createPhysicalHostTargetCatalog({ generation: 2, minimumGeneration: 2, contributions: [entry()] }),
      duplicate: () => createPhysicalHostTargetCatalog({ generation: 2, contributions: [entry(), entry()] }),
      overflow: () => createPhysicalHostTargetCatalog({
        generation: 2,
        bounds: { maximumEntries: 1 },
        contributions: [entry("one"), entry("two")],
      }),
      incompatible: () => {
        const catalog = createPhysicalHostTargetCatalog({
          generation: 2,
          contributions: [entry("one", () => ({
            schema: "conduit.creche/physical-host-target-adapter@1",
            target: { id: "fixture/wrong", label: "Wrong", model_id: "wrong", profile_id: "wrong" },
            modes: [], bounds: {},
            createOptions() {}, obtain() {}, bind() {}, realize() {}, observe() {}, cancel() {},
          }))],
        });
        catalog.createAdapter({ targetId: "fixture/target-one", host: globalThis.__conduitCrecheHost });
      },
    })) {
      try { create(); } catch (error) { evidence[name] = error.evidence; }
    }
    return evidence;
  });
  expect(refusals.stale).toMatchObject({ terminal: "StaleCatalogGeneration", catalog_generation: 2 });
  expect(refusals.duplicate).toMatchObject({ terminal: "DuplicateIdentity", target_id: "fixture/target-one" });
  expect(refusals.overflow).toMatchObject({ terminal: "CatalogBound", entries: 2, maximum_entries: 1 });
  expect(refusals.incompatible).toMatchObject({ terminal: "IncompatibleAdapter", target_id: "fixture/target-one" });
});

test("the physical workflow cancels one bounded catalog operation without accepting late truth", async ({ page }) => {
  await birthStandaloneBody(page);
  await page.evaluate(async () => {
    const { createPhysicalHostRunner } = await import("/creche/creche-physical.mjs");
    const { createPhysicalHostTargetCatalog } = await import("/creche/creche-target-catalog.mjs");
    let release;
    globalThis.__fixtureCancelCount = 0;
    const target = {
      id: "fixture/cancellable",
      label: "Cancellable fixture",
      model_id: "fixture/cancellable-model",
      profile_id: "fixture/cancellable-profile",
    };
    const modes = [
      { id: "fabricate-new", resultKind: "artifact", supported: true },
      { id: "install-existing", resultKind: "installation", supported: false },
      { id: "attach-running", resultKind: "attachment", supported: false },
    ];
    const bounds = { maximumOperations: 5, maximumOperationEvidenceBytes: 1024, maximumRetainedEvidenceBytes: 4096 };
    const adapter = {
      schema: "conduit.creche/physical-host-target-adapter@1",
      target,
      modes,
      bounds,
      createOptions() { return null; },
      obtain() {
        return new Promise((resolve) => {
          release = () => resolve({
            resultKind: "artifact",
            evidence: { schema: "fixture/late-obtainment@1", disposition: "late-success" },
          });
        });
      },
      async bind() {}, async realize() {}, async observe() {},
      async cancel() { globalThis.__fixtureCancelCount += 1; },
    };
    const contribution = {
      schema: "conduit.creche/physical-host-target-entry@1",
      family: { id: "fixture/family", label: "Fixture family" },
      target,
      intentions: modes,
      fabrication_strategies: [{ id: "fixture/strategy", label: "Fixture strategy" }],
      carriers: { deployment: [], installation: [], attachment: [], observation: [] },
      bounds,
      expected_join_contract: "fixture/join@1",
      target_profile: { schema: "fixture/target-profile@1" },
      createAdapter: () => adapter,
    };
    const targetCatalog = createPhysicalHostTargetCatalog({ generation: 1, contributions: [contribution] });
    const runner = createPhysicalHostRunner({ host: globalThis.__conduitCrecheHost, targetCatalog });
    runner.dataset.fixture = "cancellation";
    document.querySelector("#workspace").append(runner);
    globalThis.__releaseFixtureObtainment = release;
  });
  const runner = page.locator('[data-fixture="cancellation"]');
  await expect(runner.getByRole("button", { name: "Cancel current operation" })).toBeEnabled();
  await runner.getByRole("button", { name: "Cancel current operation" }).click();
  await expect(runner.locator("details code")).toContainText('"terminal": "Cancelled"');
  await page.evaluate(() => globalThis.__releaseFixtureObtainment());
  await expect(runner.locator('[data-stage="obtain"]')).not.toHaveClass(/complete/);
  const evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence).toMatchObject({ phase: "terminal", cancellations: 1, obtainment: null });
  expect(evidence.terminal).toMatchObject({ operation: "obtain", terminal: "Cancelled" });
  expect(await page.evaluate(() => globalThis.__fixtureCancelCount)).toBe(1);

  await page.evaluate(async () => {
    const { createPhysicalHostRunner } = await import("/creche/creche-physical.mjs");
    const { createPhysicalHostTargetCatalog } = await import("/creche/creche-target-catalog.mjs");
    const target = {
      id: "fixture/evidence-bound",
      label: "Evidence-bound fixture",
      model_id: "fixture/evidence-bound-model",
      profile_id: "fixture/evidence-bound-profile",
    };
    const modes = [
      { id: "fabricate-new", resultKind: "artifact", supported: true },
      { id: "install-existing", resultKind: "installation", supported: false },
      { id: "attach-running", resultKind: "attachment", supported: false },
    ];
    const bounds = { maximumOperations: 5, maximumOperationEvidenceBytes: 512, maximumRetainedEvidenceBytes: 4096 };
    const adapter = {
      schema: "conduit.creche/physical-host-target-adapter@1",
      target,
      modes,
      bounds,
      createOptions() { return null; },
      async obtain() {
        return {
          resultKind: "artifact",
          evidence: { schema: "fixture/oversized@1", payload: "x".repeat(1024) },
        };
      },
      async bind() {}, async realize() {}, async observe() {}, async cancel() {},
    };
    const contribution = {
      schema: "conduit.creche/physical-host-target-entry@1",
      family: { id: "fixture/family", label: "Fixture family" },
      target,
      intentions: modes,
      fabrication_strategies: [{ id: "fixture/strategy", label: "Fixture strategy" }],
      carriers: { deployment: [], installation: [], attachment: [], observation: [] },
      bounds,
      expected_join_contract: "fixture/join@1",
      target_profile: { schema: "fixture/target-profile@1" },
      createAdapter: () => adapter,
    };
    const targetCatalog = createPhysicalHostTargetCatalog({ generation: 1, contributions: [contribution] });
    const runner = createPhysicalHostRunner({ host: globalThis.__conduitCrecheHost, targetCatalog });
    runner.dataset.fixture = "evidence-bound";
    document.querySelector("#workspace").append(runner);
  });
  const bounded = page.locator('[data-fixture="evidence-bound"]');
  await expect(bounded.locator("details code")).toContainText('"terminal": "EvidenceBound"');
  const boundedEvidence = JSON.parse(await bounded.locator("details code").textContent());
  expect(boundedEvidence).toMatchObject({ phase: "terminal", admitted_operations: 1, obtainment: null });
});

test("the guided arc names each idea after the reader has met the prior one", async ({ page }) => {
  const chapterChecks = [
    { title: "A Form you can run", anchor: "Patchbay projects checked Form truth" },
    { title: "Faces, Backs, and implementation", anchor: "open the reviewed Back" },
    { title: "Hosts make Forms real", anchor: "smallest case of a later Body-wide model" },
    { title: "One Form across several Hosts", anchor: "cross-Host Cord is a Line" },
    { title: "The Body: one computer, one machine or many", anchor: "same Conduit problem at different topology and cost scales" },
    { title: "Many Forms, one Body-wide realization", anchor: "Program = Form" },
    { title: "Birth, spores, and the Crèche", anchor: "initial Form and birth provenance" },
  ];
  await openStep(page, 0);
  for (let step = 0; step < chapterChecks.length; step += 1) {
    await expect(page.getByRole("heading", { name: chapterChecks[step].title })).toBeVisible();
    await expect(page.locator("#chapter")).toContainText(chapterChecks[step].anchor);
    await expect(page.getByRole("heading", { name: "Conduit idea" })).toHaveCount(0);
    await expect(page.getByRole("heading", { name: "What the run proves" })).toHaveCount(0);
    await expect(page.getByRole("heading", { name: "Payoff" })).toHaveCount(0);
    const firstParagraph = page.locator(".chapter-copy").first().locator("p").first();
    await expect(firstParagraph).toBeVisible();
    if (step < chapterChecks.length - 1) await page.getByRole("button", { name: "Next" }).click();
  }
});

test("the first chapter builds from one Gear to branch, then hands off to Face/Back", async ({ page }) => {
  await openStep(page, 0);
  const hostId = await page.evaluate(() => globalThis.__conduitBookHost.hostId);
  let runner = page.locator(".runner").nth(1);
  await expect(runner.locator('[data-application-component="action-group"]')).toHaveCount(1);
  await expect(runner.getByRole("button", { name: "Run" })).toHaveAttribute("data-application-action", "book.run");
  await expect(runner.getByRole("button", { name: "Stop" })).toBeDisabled();
  await expect(runner.locator(".run, .stop")).toHaveCount(0);
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("MAKE THIS LOUD");
  await expect(runner.locator('[data-application-key="play-status"]')).toContainText("Completed");

  await expect(page.getByRole("heading", { name: "A Form you can run" })).toBeVisible();
  runner = page.locator(".runner").nth(2);
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("··· ——— ···");
  await expect(runner.locator('[data-application-key="play-status"]')).toContainText("Completed");

  await page.getByRole("button", { name: "Next" }).click();
  await expect(page.getByRole("heading", { name: "Faces, Backs, and implementation" })).toBeVisible();
  runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("···· · ·—·· ·—·· ———");
  await expect(runner.locator('[data-application-key="play-status"]')).toContainText("Completed");
  expect(await page.evaluate(() => globalThis.__conduitBookHost.hostId)).toBe(hostId);
});

test("the Face plate flips open its checked Back without replacing the runner", async ({ page }) => {
  await openStep(page, 1);
  const runner = page.locator(".runner");
  await expect(runner).toHaveCount(1);
  const listing = runner.locator("textarea");
  const source = await listing.inputValue();
  const patchbay = runner.locator(".compact-patchbay");
  const checked = await patchbay.getAttribute("data-checked-form-id");
  const sourceIdentity = await patchbay.getAttribute("data-source-document-id");
  await expect(runner.locator(".exact-evidence")).not.toHaveAttribute("open", "");

  await patchbay.getByRole("button", { name: "Open reviewed Back for same-morse-caller/morse" }).click();
  const back = patchbay.locator(".gear-back-expansion");
  await expect(back).toBeVisible();
  await expect(back.getByText("Inside this Gear")).toBeVisible();
  expect(await back.locator(".flow-faceplate").count()).toBeGreaterThan(3);
  await expect(back).toContainText("morse/lookup");
  expect(await back.getAttribute("data-checked-form-id")).toBe(checked);
  expect(await back.getAttribute("data-source-document-id")).toBe(sourceIdentity);
  expect(await back.getAttribute("data-expanded-form-id")).not.toBe(
    await patchbay.getAttribute("data-expanded-form-id"),
  );
  await expect(listing).toHaveValue(source);

  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("···· · ·—·· ·—·· ———");
  await expect(runner.locator('[data-application-key="play-status"]')).toContainText("Completed");
  await back.getByRole("button", { name: "Return to Face" }).click();
  await expect(back).toBeHidden();
  await expect(listing).toHaveValue(source);
});

test("Book navigation preserves executable drafts without owning lifecycle controls", async ({ page }) => {
  await openStep(page, 0);
  const hostId = await page.evaluate(() => globalThis.__conduitBookHost.hostId);
  const listing = page.locator(".runner").nth(1).locator("textarea");
  const edited = (await listing.inputValue()).replace('"make this loud"', '"reader-ready"');
  await listing.fill(edited);
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "Previous" }).click();
  await expect(page.locator(".runner").nth(1).locator("textarea")).toHaveValue(edited);
  expect(await page.evaluate(() => globalThis.__conduitBookHost.hostId)).toBe(hostId);
  await expect(page.getByRole("button", { name: "Reset this page" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Revisit birth page" })).toHaveCount(0);
});

test("unsupported capability and type mismatch remain ordinary pre-Play refusals", async ({ page }) => {
  await openStep(page, 0);
  const runner = page.locator(".runner").nth(1);
  const listing = runner.locator("textarea");
  await listing.fill(`form unavailable {
    source: text/literal("still planned")
    result: presentation/text
    missing: layout/inset
    source > result
  }`);
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator('[data-application-key="play-status"]')).toContainText(
    "refused before Play · missing-implementation-or-placement",
  );
  await listing.fill(`form wrong-type {
    source: scalar/literal(1.0)
    invert: logic/not
    result: presentation/bool-value
    source > invert > result
  }`);
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator('[data-application-key="play-status"]')).toContainText(
    "refused before Play · type-or-source",
  );
  await expect(runner.locator('[data-application-key="play-status"]'))
    .toHaveAttribute("data-application-component", "failure-status");
  await expect(runner.locator('[data-application-key="play-status"]'))
    .toHaveAttribute("aria-live", "assertive");
  await expect(runner.locator(".indicator")).toHaveAttribute("aria-label", "Indicator off");
});

test("state over time presents startup and current count through four admitted browser ticks", async ({ page }) => {
  await openStep(page, 2);
  await expect(page.getByRole("heading", { name: "Hosts make Forms real" })).toBeVisible();
  const runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("0");
  await expect(runner.locator(".morse")).toHaveText("4");
  await expect(runner.locator('[data-application-key="play-status"]')).toContainText(
    "4 planned ticks, 5 presentations",
  );
  await expect(runner.locator('[data-application-key="play-status"]'))
    .toHaveAttribute("data-application-component", "success-status");
  await expect(runner.locator('[data-application-key="play-status"]'))
    .toHaveAttribute("aria-live", "polite");
  await expect(runner.locator(".run-identities")).toContainText("Timer completions4");
  await expect(runner.locator(".run-identities")).toContainText("Manifestation completions5");
  await expect(runner.locator(".run-identities dd")).toHaveCount(15);
  await expect(runner.locator('.run-identities [data-application-component="definition-table"]')).toBeAttached();
});

test("stopping state over time cancels the pending timer without a late completion", async ({ page }) => {
  await openStep(page, 2);
  const runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("0");
  await runner.getByRole("button", { name: "Stop" }).click();
  await expect(runner.locator('[data-application-key="play-status"]')).toHaveText("Stopped. The Play was cancelled.");
  await page.waitForTimeout(650);
  await expect(runner.locator('[data-application-key="play-status"]')).toHaveText("Stopped. The Play was cancelled.");
  await expect(runner.locator(".run-identities")).not.toContainText("Terminal Sign");
  await expect(runner.locator(".morse")).toHaveText("0");
});

test("Hosts chapter shows the exact installed offers from the planning advertisement", async ({ page }) => {
  await openStep(page, 2);
  await expect(page.getByRole("heading", { name: "Hosts make Forms real" })).toBeVisible();
  const inventory = page.locator('[data-application-slot="book-inventory"]');
  await expect(inventory).toHaveCount(1);
  await expect(inventory).toHaveAttribute("data-application-revision", "1");
  await expect(inventory.locator('[data-application-component="disclosure"]')).toBeVisible();
  await expect(inventory.locator('[data-application-component="definition-table"]')).toHaveAttribute("aria-label", "Exact browser planning offers");
  const visibleInstalled = await inventory.locator('[data-application-key^="offer-available-"] dt').allTextContents();
  const advertisedInstalled = await page.evaluate(() => {
    const api = globalThis.__conduitBookHost.runtime;
    api.conduit_book_inventory();
    const bytes = new Uint8Array(api.memory.buffer, api.conduit_book_output_ptr(), api.conduit_book_output_len());
    return JSON.parse(new TextDecoder().decode(bytes)).entries
      .filter((entry) => entry.implementation_id !== null)
      .map((entry) => entry.kind_id);
  });
  expect(visibleInstalled).toEqual(advertisedInstalled);
  expect(visibleInstalled).toEqual(expect.arrayContaining([
    "time/every", "state/count", "presentation/count", "logic/select",
    "layout/viewport", "time/delay", "input/keyboard", "presentation/bool",
  ]));
  const [source, manifest] = await Promise.all([
    page.request.get(new URL("book.mjs", entrance.url).href).then((response) => response.text()),
    page.request.get(new URL("book.application.json", entrance.url).href).then((response) => response.json()),
  ]);
  expect(source).toContain('from "./book-inventory-presentation.mjs"');
  expect(source).not.toContain('className = "gear-inventory"');
  expect(manifest.resources.some((resource) => resource.role === "book-inventory-presentation")).toBe(true);
});

test("Two browser Hosts executes one unchanged Form across independent Hosts", async ({ page }) => {
  await openStep(page, 3);
  await expect(page.getByRole("heading", { name: "One Form across several Hosts" })).toBeVisible();
  const runner = page.locator(".multi-host-runner").first();
  const source = await runner.locator("textarea").inputValue();
  expect(source).not.toMatch(/HostId|BootId|browser\/|iframe|DOM|socket|address/);
  await runner.getByRole("button", { name: "Run across two Hosts" }).click();
  await expect(runner.locator('[data-application-key="play-status"]')).toContainText(
    "one immutable Plan, two independent Plays, one delivered cross-Host value",
  );
  await expect(runner.locator(".morse")).toHaveText("hello across one Cord");
  await expect(runner.locator(".host-a strong")).toHaveText("completed");
  await expect(runner.locator(".host-b strong")).toHaveText("completed");
  const identities = await page.evaluate(() => ({
    a: globalThis.__conduitBookHost,
    b: globalThis.__conduitBookPeerHost,
  }));
  expect(identities.a.hostId).not.toBe(identities.b.hostId);
  expect(identities.a.bootId).not.toBe(identities.b.bootId);
  await expect(page.locator("iframe")).toHaveCount(0);
  await expect(runner.locator('[data-application-key="cord"]')).toContainText("1 item");
  await expect(runner.locator(".run-identities")).toContainText("Terminal source receipt");
  await expect(runner.locator(".run-identities")).toContainText("Terminal sink receipt");
});

test("Plans and Plays compact and raw views project the same exact immutable Plan", async ({ page }) => {
  await openStep(page, 3);
  await expect(page.getByRole("heading", { name: "One Form across several Hosts" })).toBeVisible();
  const runner = page.locator(".multi-host-runner").nth(1);
  await expect(runner.locator(".plan-view-details")).not.toHaveAttribute("open", "");
  await runner.getByRole("button", { name: "Run across two Hosts" }).click();
  await expect(runner.locator('[data-application-key="play-status"]')).toContainText("Completed");
  const projectedPlanId = await runner.locator('[data-application-key="plan-id"]').textContent();
  const rawPlan = JSON.parse(await runner.locator('[data-application-key="raw-plan-json"] code').textContent());
  expect(rawPlan.plan_id).toBe(projectedPlanId);
  expect(rawPlan.fragments).toHaveLength(2);
  expect(new Set(rawPlan.fragments.map((fragment) => fragment.host_id)).size).toBe(2);
  await expect(runner.locator('[data-application-key="hosts"] [data-application-component="artifact"]')).toHaveCount(2);
  await expect(runner.locator('[data-application-key="hosts"]')).toContainText("text/literal");
  await expect(runner.locator('[data-application-key="hosts"]')).toContainText("presentation/text");
  await expect(runner.locator('[data-application-slot^="book-plan-evidence-"] [data-application-component="panel"]')).toHaveCount(1);
});

test("Add a physical Host keeps IMAGE, deployment, Boot, join, admission, offers, Plan, and Play distinct", async ({ page }) => {
  await installB7Devices(page);
  const birth = await birthStandaloneBody(page);
  await page.getByRole("button", { name: "3. Physical Host" }).click();
  const runner = page.locator(".physical-host-runner");
  await expect(runner.locator('[data-stage="obtain"]')).toHaveClass(/complete/);
  await expect(runner.locator(".physical-status")).toContainText("Invitation, realization, Boot, join, membership, offers, Plan, and Play remain absent");

  await runner.getByRole("button", { name: "Bind Body invitation" }).click();
  await expect(runner.locator('[data-stage="bind"]')).toHaveClass(/complete/);
  await expect(runner.locator(".physical-status")).toContainText("Realization, Boot, join, membership, offers, Plan, and Play remain absent");

  await runner.getByRole("button", { name: "Realize selected Host" }).click();
  await expect(runner.locator('[data-stage="realize"] span')).toHaveText("RebootRequested");
  await expect(runner.locator(".physical-status")).toContainText("No Boot or join has been observed, and no membership, offers, readiness, Plan, or Play has been admitted");

  await runner.getByRole("button", { name: "Observe Boot and join" }).click();
  await expect(runner.locator('[data-stage="observe"]')).toHaveClass(/complete/);
  await expect(runner.locator(".physical-status")).toContainText("Admission remains an explicit action");

  await runner.getByRole("button", { name: "Admit Part and offers" }).click();
  await expect(runner.locator('[data-stage="admit"]')).toHaveClass(/complete/);
  await expect(runner.locator(".physical-status")).toContainText("current offers are ready. No Plan or Play was created");
  const evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.binding.body_id).toBe(birth.bodyId);
  expect(evidence.binding.invitation_secret).toBe("embedded in native UF2; redacted");
  expect(evidence.realization).toMatchObject({
    terminal: "RebootRequested",
    spore_id: evidence.binding.spore_id,
    image_id: evidence.binding.image_id,
    runtime_truth_created: false,
  });
  expect(evidence.observation.boot_id).toBe("pico-boot/b7-browser-proof");
  expect(evidence.admission).toMatchObject({
    disposition: "admitted",
    body_id: birth.bodyId,
    spore_id: evidence.binding.spore_id,
    image_id: evidence.binding.image_id,
    offers_observed: true,
    ready: true,
    plan_id: null,
    active_play_id: null,
  });
  expect(birth.birthSignId).toHaveLength(64);
});

test("Add a physical Host retains a refused WebUSB acquisition as terminal", async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "usb", {
      configurable: true,
      value: {
        requestDevice: async () => {
          throw new DOMException("operator selected no BOOTSEL device", "NotFoundError");
        },
        addEventListener() {},
      },
    });
  });
  await birthStandaloneBody(page);
  await page.getByRole("button", { name: "3. Physical Host" }).click();
  const runner = page.locator(".physical-host-runner");
  await expect(runner.locator('[data-stage="obtain"]')).toHaveClass(/complete/);
  await runner.getByRole("button", { name: "Bind Body invitation" }).click();
  const deploy = runner.getByRole("button", { name: "Realize selected Host" });
  await deploy.click();
  await expect(runner.locator(".physical-status")).toContainText("This USB acquisition is terminal");
  await expect(runner.locator('[data-stage="realize"] span')).toHaveText("waiting");
  await expect(deploy).toBeDisabled();
});

test("Add a physical Host retains the exact Picoboot refusal chain", async ({ page }) => {
  await installB7Devices(page, { staleStatus: true });
  await birthStandaloneBody(page);
  await page.getByRole("button", { name: "3. Physical Host" }).click();
  const runner = page.locator(".physical-host-runner");
  await expect(runner.locator('[data-stage="obtain"]')).toHaveClass(/complete/);
  await runner.getByRole("button", { name: "Bind Body invitation" }).click();
  await runner.getByRole("button", { name: "Realize selected Host" }).click();
  const evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.terminal.target_evidence).toMatchObject({
    phase: "terminal",
    terminal: "StaleStatus",
    reboot_requested: false,
  });
  expect(evidence.terminal.target_evidence.failure_chain).toEqual([
    "StaleStatus: RP2040 deployment terminated without success",
    "StaleStatus: PICOBOOT status belongs to a different command identity",
  ]);
  await expect(runner.locator(".physical-status")).toContainText("StaleStatus");
});

test("graduation retains the same Body through an ordinary hosted Patchbay Plan", async ({ page }) => {
  const { bodyId } = await birthStandaloneBody(page, { attachFirstHost: true });
  await page.getByRole("button", { name: "4. Graduate" }).click();
  const runner = page.locator(".graduation-runner");
  const criteria = runner.locator('[data-application-key="graduation-criteria"]');
  await expect(criteria.locator('[data-application-component="panel"]')).toHaveText([
    "Durable Body identity · ready",
    "Bound BIRTH evidence · ready",
    "Current admitted Part · ready",
  ]);
  await runner.getByRole("button", { name: "Host Patchbay on this Body" }).click();
  await expect(runner).toHaveAttribute("data-body-id", bodyId);
  await expect(runner.locator('[data-application-key="graduation-identities"]')).toContainText("browser/patchbay-surface@1");
  await expect(runner.locator('[data-application-key="graduation-identities"]')).toContainText("Crèche requiredfalse");
  const biography = runner.locator(".body-biography");
  await expect(biography.locator('[data-application-key="biography-records"]')).toContainText(bodyId);
  await expect(biography.locator('[data-application-key^="biography-record-"]')).toHaveCount(4);
  await expect(biography.locator("dt")).toHaveText(["Born", "Part admitted", "Host joined", "Graduated from the Crèche"]);
  await expect(biography).toContainText("browser/patchbay-surface@1");
  await runner.getByRole("button", { name: "End the Crèche" }).click();
  await expect(page.locator(".creche-complete")).toContainText(bodyId);
  await expect(page.locator(".creche-steps")).toHaveCount(0);
  await expect(page.locator('.creche-complete .body-biography [data-application-key^="biography-record-"]')).toHaveCount(4);
  const retained = await page.evaluate(() => {
    const api = globalThis.__conduitCrecheHost.runtime;
    api.conduit_creche_current();
    const bytes = new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len());
    return JSON.parse(new TextDecoder().decode(bytes));
  });
  expect(retained.body_id).toBe(bodyId);
  expect(retained.graduation.choice).toBe("host-patchbay");
  expect(retained.graduation.patchbay_plan_id).toMatch(/^[0-9a-f]{64}$/);
  const durable = await page.evaluate(() => {
    const api = globalThis.__conduitCrecheHost.runtime;
    api.conduit_creche_biography();
    const bytes = new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len());
    return JSON.parse(new TextDecoder().decode(bytes));
  });
  expect(durable.body_id).toBe(bodyId);
  expect(durable.records).toHaveLength(4);
});

test("graduation can finish without hosting Patchbay and still retain the same Body", async ({ page }) => {
  const { bodyId } = await birthStandaloneBody(page, { attachFirstHost: true });
  await page.getByRole("button", { name: "4. Graduate" }).click();
  const runner = page.locator(".graduation-runner");
  await runner.getByRole("button", { name: "Finish without hosted Patchbay" }).click();
  await expect(runner).toHaveAttribute("data-body-id", bodyId);
  await expect(runner.locator('[data-application-key="graduation-identities"]')).toContainText("Patchbay Plannot hosted");
  const evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.choice).toBe("external-reader");
  expect(evidence.patchbay_plan_id).toBeNull();
  expect(evidence.creche_required).toBe(false);
  await expect(runner.locator('.body-biography [data-application-key^="biography-record-"]')).toHaveCount(4);
  await expect(runner.locator(".body-biography")).toContainText("compatible reader can project this same evidence later");
});

test("stopping the two-Host lesson cancels without a late manifestation", async ({ page }) => {
  await page.addInitScript(() => {
    const callbacks = [];
    globalThis.requestAnimationFrame = (callback) => {
      callbacks.push(callback);
      return callbacks.length;
    };
    globalThis.__releaseBookAnimationFrame = () => {
      for (const callback of callbacks.splice(0)) callback(performance.now());
    };
  });
  await openStep(page, 3);
  const runner = page.locator(".multi-host-runner").first();
  await runner.getByRole("button", { name: "Run across two Hosts" }).click();
  await expect(runner.locator('[data-application-key="play-status"]')).toContainText("Host A offered one value");
  await runner.getByRole("button", { name: "Stop" }).click();
  await expect(runner.locator('[data-application-key="play-status"]')).toHaveText("Stopped. The Play was cancelled.");
  await page.evaluate(() => globalThis.__releaseBookAnimationFrame());
  await expect(runner.locator('[data-application-key="play-status"]')).toHaveText("Stopped. The Play was cancelled.");
  await expect(runner.locator(".morse")).toHaveText("ready");
  await expect(runner.locator(".run-identities")).not.toContainText("Terminal source receipt");
});
