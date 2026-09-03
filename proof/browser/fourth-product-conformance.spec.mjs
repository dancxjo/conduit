import { spawn } from "node:child_process";
import { execFile } from "node:child_process";
import { cp, mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { promisify } from "node:util";
import { expect, test } from "@playwright/test";
import { startStaticProduct } from "./book-test-server.mjs";

const execute = promisify(execFile);
const repository = new URL("../..", import.meta.url).pathname;
const fixtureSource = join(repository, "proof/browser/fourth-product");
const sharedAssets = join(repository, "targets/browser/host/assets");
const bookProduct = process.env.CONDUIT_TOUR_PRODUCT_ROOT ?? "target/tour-product";
const crecheProduct = process.env.CONDUIT_CRECHE_PRODUCT_ROOT ?? "target/creche-product";
let stagedFixture;

async function stageFixture() {
  stagedFixture = await mkdtemp(join(tmpdir(), "conduit-fourth-product-"));
  for (const path of [
    "application.html", "application-presentation.mjs", "application-theme.mjs",
    "application-theme.css", "browser-application-loader.mjs", "browser-application-storage.mjs",
  ]) await cp(join(sharedAssets, path), join(stagedFixture, path));
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

function awaitUrl(child, pattern, label) {
  let output = "";
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`${label} was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(pattern);
      if (match) { clearTimeout(timeout); resolve(match[1]); }
    };
    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("exit", (code) => {
      clearTimeout(timeout);
      reject(new Error(`${label} exited (${code})\n${output}`));
    });
  });
}

async function startPatchbay() {
  const child = spawn("target/debug/patchbay-html", ["--documentary-fixture"], {
    cwd: repository,
    stdio: ["ignore", "pipe", "pipe"],
  });
  const url = await awaitUrl(child, /PATCHBAY_HTML_URL=(http:\/\/127\.0\.0\.1:\d+\/?)/, "Patchbay HTML");
  return { child, url };
}

async function expectSharedComponents(page, names) {
  for (const name of names) {
    await expect(page.locator(`[data-application-component="${name}"]:visible`).first(), name).toBeVisible();
  }
}

test.beforeAll(stageFixture);
test.afterAll(async () => { if (stagedFixture) await rm(stagedFixture, { recursive: true, force: true }); });

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
      ],
    });
    for (const path of ["application.mjs", "state.mjs"]) {
      const source = await readFile(join(fixtureSource, path), "utf8");
      expect(source, basename(path)).not.toMatch(/\b(?:document|window|navigator|fetch)\b|createElement|querySelector|innerHTML|addEventListener|\.css\b/);
    }

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
    ["Tour", () => startStaticProduct(bookProduct, "/conduit/tour/"), ["navigation", "form-field", "status"]],
    ["Crèche", () => startStaticProduct(crecheProduct, "/conduit/creche/"), ["stepper", "form-field", "disclosure"]],
    ["Patchbay", startPatchbay, ["navigation", "artifact", "disclosure"]],
    ["Field Notes", () => startStaticProduct(stagedFixture), ["navigation", "artifact", "disclosure", "progress"]],
  ];
  for (const [name, start, components] of products) {
    const entrance = await start();
    try {
      await page.goto(entrance.url);
      try { await expectSharedComponents(page, components); }
      catch (error) { throw new Error(`${name}: ${error.message}`); }
      expect(await page.locator("[data-application-component]").count(), name).toBeGreaterThan(0);
    } finally { entrance.child.kill(); }
  }
});
