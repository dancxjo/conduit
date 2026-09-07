import { execFile } from "node:child_process";
import { cp, mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { promisify } from "node:util";
import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./tour-test-server.mjs";

const execute = promisify(execFile);
const repository = new URL("../..", import.meta.url).pathname;
const fixtureSource = join(repository, "proof/browser/fourth-product");
const sharedAssets = join(repository, "targets/browser/host/assets");
const sharedDesignSystem = join(repository, "products/shared/browser/conduit.css");
const tourProduct = process.env.CONDUIT_TOUR_PRODUCT_ROOT ?? "target/tour-product";
const patchbayProduct = process.env.CONDUIT_PATCHBAY_PRODUCT_ROOT ?? "target/patchbay-product";
const crecheProduct = process.env.CONDUIT_CRECHE_PRODUCT_ROOT ?? "target/creche-product";
let stagedFixture;

async function stageFixture() {
  stagedFixture = await mkdtemp(join(tmpdir(), "conduit-fourth-product-"));
  for (const path of [
    "application.html", "application-presentation.mjs", "application-theme.mjs",
    "browser-application-loader.mjs", "browser-application-storage.mjs",
  ]) await cp(join(sharedAssets, path), join(stagedFixture, path));
  await cp(sharedDesignSystem, join(stagedFixture, "conduit.css"));
  await cp(join(repository, "semantics/presentation/assets/product-masthead.mjs"), join(stagedFixture, "product-masthead.mjs"));
  await cp(join(stagedFixture, "application.html"), join(stagedFixture, "index.html"));
  for (const path of ["application.mjs", "state.mjs"]) {
    await cp(join(fixtureSource, path), join(stagedFixture, path));
  }
  await execute("node", [
    "targets/browser/tools/build-browser-application-package.mjs",
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

test("hosted applications cover unstyled content until admitted presentation is ready", async ({ page }) => {
  const entrance = await startStaticProduct(stagedFixture);
  let releaseTheme;
  const themeReleased = new Promise((resolve) => { releaseTheme = resolve; });
  await page.route("**/conduit.css", async (route) => {
    await themeReleased;
    await route.continue();
  });
  try {
    await page.goto(entrance.url, { waitUntil: "commit" });
    const suspense = page.locator("#conduit-suspense");
    await expect(suspense).toBeVisible();
    await expect(suspense).toHaveAttribute("aria-busy", "true");
    await expect(suspense).toHaveCSS("position", "fixed");
    await expect(suspense).toHaveCSS("background-color", "rgb(5, 7, 11)");
    releaseTheme();
    await expect(page.getByRole("heading", { name: "Field Notes" })).toBeVisible();
    await expect(suspense).toHaveCount(0);
    await expect(page.locator("html")).toHaveAttribute("data-conduit-load-state", "ready");
  } finally {
    releaseTheme();
    entrance.child.kill();
  }
});

test("one semantic ProductMasthead composition replaces product-private global chrome", async () => {
  const productSurfaces = [
    ["Tour", "products/tour/browser/tour.html", "products/tour/browser/tour.css", "products/tour/browser/tour.mjs"],
    ["Crèche", "products/creche/browser/creche.html", "products/creche/browser/creche.css", "products/creche/browser/creche.mjs"],
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
    expect(html, `${name} bootstrap suspense`).toContain('id="conduit-suspense"');
    expect(html, `${name} inline bootstrap style`).toContain("data-conduit-bootstrap");
  }
  const pages = await readFile(join(repository, "site/index.html"), "utf8");
  expect(pages).toContain("<!-- conduit-product-masthead -->");
  expect(pages).not.toMatch(/<nav[^>]*>[^]*?(?:Tour|Crèche|Patchbay)[^]*?<\/nav>/);
});

test("Tour Form Gallery ordinary controls use the shared presentation vocabulary", async () => {
  const gallery = await readFile(join(repository, "products/tour/browser/tour-inventory-presentation.mjs"), "utf8");
  const galleryComposition = gallery.slice(
    gallery.indexOf("export function createReviewedFormGallery"),
    gallery.indexOf("export function presentTourInventory"),
  );
  for (const tag of ["input", "output", "button", "a"]) {
    expect(galleryComposition, `Gallery must not directly construct generic ${tag} controls`).not.toContain(
      `createElement("${tag}")`,
    );
  }
  expect(galleryComposition, "Gallery must not construct low-level component arrays").not.toContain("component:");
  expect(galleryComposition, "Gallery must request the product-owned semantic view").toContain(
    "conduit_browser_form_reviewed_gallery_view",
  );
  const model = await readFile(join(repository, "products/tour/model/src/gallery.rs"), "utf8");
  for (const mechanism of ["FormField", "Status", "Action", "Link", "Grid"]) {
    expect(model, `Gallery must describe ${mechanism} through the shared semantic vocabulary`).toContain(
      `PresentationMechanism::${mechanism}`,
    );
  }
});

test("Tour page navigation uses product-owned semantic presentation", async () => {
  const navigation = await readFile(join(repository, "products/tour/browser/tour-navigation.mjs"), "utf8");
  const composition = navigation.slice(
    navigation.indexOf("export function createTourNavigation"),
    navigation.indexOf("export function presentTourWorkspaceSeparator"),
  );
  expect(composition, "Tour navigation must not construct low-level component arrays").not.toContain(
    "component:",
  );
  expect(composition, "Tour navigation must request its product-owned semantic view").toContain(
    "conduit_tour_navigation_view",
  );
  const model = await readFile(join(repository, "products/tour/model/src/navigation.rs"), "utf8");
  for (const mechanism of ["Navigation", "Status", "Action"]) {
    expect(model, `Tour navigation must describe ${mechanism} through shared semantics`).toContain(
      `PresentationMechanism::${mechanism}`,
    );
  }
});

test("Crèche browser configuration ordinary controls use product-owned semantics", async () => {
  const configuration = await readFile(
    join(repository, "products/creche/browser/creche-browser-configuration.mjs"),
    "utf8",
  );
  const composition = configuration.slice(
    configuration.indexOf("export function createBrowserConfigurationOutfitter"),
    configuration.indexOf("export function prepareCheckedBrowserSpore"),
  );
  expect(composition, "Crèche configuration must not construct low-level component arrays").not.toContain(
    "component:",
  );
  expect(composition, "Crèche configuration must request its product-owned semantic view").toContain(
    "conduit_creche_browser_configuration_view",
  );
  const model = await readFile(join(repository, "products/creche/model/src/lib.rs"), "utf8");
  for (const mechanism of ["ChoiceGroup", "Evidence", "DefinitionTable", "CodeBlock", "Action"]) {
    expect(model, `Crèche configuration must describe ${mechanism} through shared semantics`).toContain(
      `PresentationMechanism::${mechanism}`,
    );
  }
});

test("Crèche graduation controls and evidence use product-owned semantics", async () => {
  const graduation = await readFile(
    join(repository, "products/creche/browser/creche-graduation.mjs"),
    "utf8",
  );
  const composition = graduation.slice(
    graduation.indexOf("function presentGraduationControls"),
    graduation.indexOf("export function renderBiography"),
  );
  expect(composition, "Crèche graduation must not construct low-level component arrays").not.toContain(
    "component:",
  );
  expect(composition, "Crèche graduation must request its product-owned semantic view").toContain(
    "conduit_creche_graduation_view",
  );
  const model = await readFile(
    join(repository, "products/creche/model/src/graduation_presentation.rs"),
    "utf8",
  );
  for (const mechanism of ["Grid", "Status", "Action", "Evidence", "DefinitionTable", "CodeBlock"]) {
    expect(model, `Crèche graduation must describe ${mechanism} through shared semantics`).toContain(
      `PresentationMechanism::${mechanism}`,
    );
  }
});

test("all four web surfaces inherit one product-owned browser design system", async () => {
  const shared = await readFile(join(repository, "products/shared/browser/conduit.css"), "utf8");
  expect(shared).toContain("--conduit-font-body:");
  expect(shared).toContain("--conduit-font-editorial:");
  expect(shared).toContain("--conduit-font-mono:");
  expect(shared).toContain('[data-application-key="product-masthead"]');
  expect(shared).toContain('button[data-application-component]');

  for (const path of [
    "site/site.css",
    "products/tour/browser/tour.css",
    "products/creche/browser/creche.css",
    "products/patchbay/html/assets/app.css",
  ]) {
    const css = await readFile(join(repository, path), "utf8");
    expect(css, `${path} must not mint a private root design system`).not.toMatch(/:root\s*\{/);
    expect(css, `${path} must not select its own body typeface`).not.toMatch(/\b(?:Inter|DejaVu Sans)\b/);
    expect(css, `${path} must not redefine generic browser furniture`).not.toMatch(
      /(?:^|})\s*(?:button|select|input|textarea|summary|a:focus-visible)(?:\b|:|,)[^{]*\{/m,
    );
  }
  const tour = await readFile(join(repository, "products/tour/browser/tour.css"), "utf8");
  expect(tour.match(/h1\s*\{[^}]*font:[^}]*var\(--conduit-font-body\)/), "Tour heading uses the shared application family").not.toBeNull();

  const pages = await readFile(join(repository, "site/index.html"), "utf8");
  expect(pages).toContain('href="./conduit.css"');
  for (const path of [
    "products/tour/browser/tour.application.template.json",
    "products/creche/browser/creche.application.template.json",
    "products/patchbay/html/assets/patchbay.application.template.json",
  ]) {
    const manifest = JSON.parse(await readFile(join(repository, path), "utf8"));
    const resource = manifest.resources.find(({ role }) => role === "shared-presentation-style");
    expect(resource?.path, `${path} shared design system`).toMatch(/(?:^|\/)conduit\.css$/);
  }
});

test("fourth application is admitted without product HTML, CSS, DOM, or browser effects", async ({ page }) => {
  const entrance = await startStaticProduct(stagedFixture);
  try {
    await page.emulateMedia({ colorScheme: "dark" });
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
      if (name === "Patchbay") await page.getByText("Advanced evidence and linear presentation", { exact: true }).click();
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
