import { execFile } from "node:child_process";
import { cp, mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { promisify } from "node:util";
import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./book-test-server.mjs";

const execute = promisify(execFile);
const repository = new URL("../..", import.meta.url).pathname;
const fixtureSource = join(repository, "proof/browser/fourth-product");
const sharedAssets = join(repository, "targets/browser/host/assets");
const tourProduct = process.env.CONDUIT_TOUR_PRODUCT_ROOT ?? "target/tour-product";
const patchbayProduct = process.env.CONDUIT_PATCHBAY_PRODUCT_ROOT ?? "target/patchbay-product";
const crecheProduct = process.env.CONDUIT_CRECHE_PRODUCT_ROOT ?? "target/creche-product";
let stagedFixture;

async function stageFixture() {
  stagedFixture = await mkdtemp(join(tmpdir(), "conduit-fourth-product-"));
  for (const path of [
    "application.html", "application-presentation.mjs", "application-theme.mjs",
    "application-theme.css", "browser-application-loader.mjs", "browser-application-storage.mjs",
  ]) await cp(join(sharedAssets, path), join(stagedFixture, path));
  await cp(join(repository, "semantics/presentation/assets/product-masthead.mjs"), join(stagedFixture, "product-masthead.mjs"));
  await cp(join(stagedFixture, "application.html"), join(stagedFixture, "index.html"));
  for (const path of ["application.mjs", "state.mjs"]) {
    await cp(join(fixtureSource, path), join(stagedFixture, path));
  }
  await execute("node", [
    "scripts/ci/build-browser-application-package.mjs",
    join(fixtureSource, "fourth.application.template.json"),
    stagedFixture,
    "application.application.json",
  ], { cwd: repository });
}

async function expectSharedComponents(page, names) {
  for (const name of names) {
    await expect(page.locator(`[data-application-component="${name}"]:visible`).first(), name).toBeVisible();
  }
}

test.beforeAll(stageFixture);
test.afterAll(async () => { if (stagedFixture) await rm(stagedFixture, { recursive: true, force: true }); });

test("one semantic ProductMasthead composition replaces product-private global chrome", async () => {
  const productSurfaces = [
    ["Tour", "targets/browser/host/assets/book.html", "targets/browser/host/assets/book.css", "targets/browser/host/assets/book.mjs"],
    ["Crèche", "targets/browser/host/assets/creche.html", "targets/browser/host/assets/creche.css", "targets/browser/host/assets/creche.mjs"],
    ["Patchbay", "products/patchbay/html/assets/index.html", "products/patchbay/html/assets/app.css", "products/patchbay/html/assets/app.js"],
  ];
  for (const [name, htmlPath, cssPath, modulePath] of productSurfaces) {
    const [html, css, module] = await Promise.all([
      readFile(join(repository, htmlPath), "utf8"),
      readFile(join(repository, cssPath), "utf8"),
      readFile(join(repository, modulePath), "utf8"),
    ]);
    expect(html.match(/data-application-slot="product-masthead"/g), `${name} masthead slot`).toHaveLength(1);
    expect(html, `${name} private global chrome`).not.toMatch(/class="(?:topbar|site-header|global-nav|wordmark)"/);
    expect(css, `${name} private global chrome CSS`).not.toMatch(/\.(?:topbar|site-header|global-nav|wordmark)\b/);
    expect(module, `${name} shared composition consumer`).toContain("createProductMasthead");
  }
  const pages = await readFile(join(repository, "site/index.html"), "utf8");
  expect(pages).toContain("<!-- conduit-product-masthead -->");
  expect(pages).not.toMatch(/<nav[^>]*>[^]*?(?:Tour|Crèche|Patchbay)[^]*?<\/nav>/);
});

test("fourth application is admitted without product HTML, CSS, DOM, or browser effects", async ({ page }) => {
  const entrance = await startStaticProduct(stagedFixture);
  try {
    await page.goto(entrance.url);
    await expect(page.getByRole("heading", { name: "Field Notes" })).toBeVisible();
    const application = page.locator('[data-application-slot="application"]');
    await expect(application).toHaveAttribute(
      "data-application-theme", "conduit.presentation/phosphor@1",
    );
    await expect(application).toHaveCSS("background-color", "rgb(5, 7, 11)");
    await expect(application).toHaveCSS("color", "rgb(147, 210, 247)");
    await expectSharedComponents(page, [
      "shell", "navigation", "button", "status", "form-field", "text-input",
      "disclosure", "progress", "artifact", "missing-evidence",
    ]);
    await page.getByText("Exact observation", { exact: true }).click();
    await expect(page.locator('[data-application-component="code-block"]')).toBeVisible();
    const manifest = await page.evaluate(() => ({
      applicationId: globalThis.__conduitBrowserApplication.manifest.applicationId,
      resources: globalThis.__conduitBrowserApplication.manifest.resources.map(({ role, kind, path }) => ({ role, kind, path })),
    }));
    expect(manifest).toEqual({
      applicationId: "conduit.application/field-notes-proof",
      resources: [
        { role: "application-module", kind: "module", path: "application.mjs" },
        { role: "runtime", kind: "module", path: "state.mjs" },
        { role: "product-masthead", kind: "module", path: "product-masthead.mjs" },
      ],
    });
    for (const path of ["application.mjs", "state.mjs"]) {
      const source = await readFile(join(fixtureSource, path), "utf8");
      expect(source, basename(path)).not.toMatch(/\b(?:document|window|navigator|fetch)\b|createElement|querySelector|innerHTML|addEventListener|\.css\b/);
    }

    const productNavigation = page.getByRole("navigation", { name: "Conduit products" });
    await expect(productNavigation.getByRole("link")).toHaveCount(5);
    await expect(productNavigation.getByRole("link", { name: "Tour" })).toHaveAttribute("href", "/conduit/tour/");

    const navigation = page.getByRole("navigation", { name: "Field Notes destinations" });
    await navigation.getByRole("button", { name: "Overview" }).focus();
    await page.keyboard.press("ArrowRight");
    await expect(navigation.getByRole("button", { name: "Work" })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.locator('[data-application-key="status"]')).toHaveText("Opened work");
    await page.getByRole("textbox", { name: "Observer name" }).fill("Grace");
    await expect(page.locator('[data-application-key="artifact-summary"]')).toHaveText("Observed by Grace");
    await page.getByRole("button", { name: "Record success" }).click();
    await expect(page.locator('[data-application-evidence="succeeded"]')).toContainText("Success remained distinct and bounded.");
  } finally { entrance.child.kill(); }
});

test("fourth application keeps every required refusal and Host failure distinct", async ({ page }) => {
  const entrance = await startStaticProduct(stagedFixture);
  try {
    const reset = async () => {
      await page.goto(entrance.url);
      await expect(page.getByRole("heading", { name: "Field Notes" })).toBeVisible();
    };

    await reset();
    await page.getByRole("button", { name: "Prove stale revision" }).click();
    await expect(page.locator('[data-application-evidence="stale"]')).toContainText("stale-revision");

    await reset();
    await page.getByRole("button", { name: "Prove unsupported mechanism" }).click();
    await expect(page.locator('[data-application-evidence="refused"]')).toContainText("unknown-component");

    await reset();
    await page.evaluate(() => globalThis.__conduitBrowserApplication.presentation.requestAction("application", "unavailable"));
    await expect(page.locator('[data-application-evidence="refused"]')).toContainText("unavailable-action");

    await reset();
    await page.evaluate(() => globalThis.__conduitBrowserApplication.presentation.requestAction("application", "name", "this value is outside the admitted bound"));
    await expect(page.locator('[data-application-evidence="refused"]')).toContainText("invalid-control-value");

    await reset();
    await page.evaluate(() => {
      const presentation = globalThis.__conduitBrowserApplication.presentation;
      presentation.requestAction("application", "pressure");
      presentation.requestAction("application", "pressure");
    });
    await expect(page.locator('[data-application-evidence="refused"]')).toContainText("queue-pressure");

    await reset();
    await page.getByRole("button", { name: "Prove Host-effect failure" }).click();
    await expect(page.locator('[data-application-evidence="failed"]')).toBeVisible();
    await expect(page.locator('[data-application-key="status"]')).toHaveText("Host effect failed");
    await expect(page.locator('[data-application-key="evidence-detail"]')).not.toHaveText(/^(?:stale-revision|queue-pressure|invalid-control-value|unavailable-action|unknown-component)$/);
  } finally { entrance.child.kill(); }
});

test("Tour, Crèche, Patchbay, and the fourth app manifest the same shared contracts", async ({ page }) => {
  const products = [
    ["Tour", "tour", () => startStaticProduct(tourProduct, "/conduit/tour/"), ["navigation", "form-field", "status"]],
    ["Crèche", "creche", () => startStaticProduct(crecheProduct, "/conduit/creche/"), ["stepper", "form-field", "disclosure"]],
    ["Patchbay", "patchbay", () => startStaticProduct(patchbayProduct, "/conduit/patchbay/"), ["navigation", "definition-table", "disclosure"]],
    ["Field Notes", null, () => startStaticProduct(stagedFixture), ["navigation", "artifact", "disclosure", "progress"]],
  ];
  const expectedLabels = ["conduit", "Tour", "Crèche", "Patchbay", "Source"];
  const expectedDestinations = ["home", "tour", "creche", "patchbay", "source"];
  for (const [name, current, start, components] of products) {
    const entrance = await start();
    try {
      await page.goto(entrance.url);
      if (name === "Patchbay") await page.getByRole("button", { name: "Exact truth", exact: true }).click();
      try { await expectSharedComponents(page, components); }
      catch (error) { throw new Error(`${name}: ${error.message}`); }
      const masthead = page.locator('[data-application-key="product-masthead"]');
      const navigation = masthead.getByRole("navigation", { name: "Conduit products" });
      const links = navigation.locator('[data-application-component="navigation-link"]');
      await expect(links, `${name} shared masthead links`).toHaveCount(5);
      expect(await links.allTextContents(), `${name} shared masthead labels`).toEqual(expectedLabels);
      expect(await links.evaluateAll((elements) => elements.map((element) => element.dataset.applicationKey)), `${name} admitted destinations`).toEqual(expectedDestinations);
      await expect(navigation.locator('[aria-current="page"]')).toHaveCount(current === null ? 0 : 1);
      if (current !== null) await expect(navigation.locator(`[data-application-key="${current}"]`)).toHaveAttribute("aria-current", "page");
      expect(await links.evaluateAll((elements) => elements.every((element) => element.tagName === "A" && element.onclick === null)), `${name} native links`).toBe(true);
      await page.setViewportSize({ width: 375, height: 800 });
      for (const key of expectedDestinations) await expect(navigation.locator(`[data-application-key="${key}"]`)).toBeVisible();
      expect(await masthead.evaluate((element) => element.scrollWidth <= element.clientWidth), `${name} bounded narrow masthead`).toBe(true);
      await page.setViewportSize({ width: 1280, height: 720 });
      expect(await page.locator("[data-application-component]").count(), name).toBeGreaterThan(0);
    } finally { entrance.child.kill(); }
  }
});
