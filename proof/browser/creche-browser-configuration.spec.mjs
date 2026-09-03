import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";

let entrance;

test.beforeEach(async () => { entrance = await startCreche(); });
test.afterEach(() => entrance?.child.kill());

test("browser outfitting is catalog-driven, editable, and handed to checked fabrication", async ({ page }) => {
  const release = await installBrowserRelease(page);
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await page.getByRole("button", { name: "Birth Body" }).click();
  await page.getByRole("button", { name: "3. Physical Host" }).click();
  const runner = page.locator(".physical-host-runner");
  await runner.locator(".physical-target").selectOption("browser/wasm32/page");

  await expect(runner.locator('[data-stage="obtain"]')).not.toHaveClass(/complete/);
  await expect(runner.locator(".browser-configuration fieldset")).toHaveCount(6);
  await expect(runner.locator('.browser-configuration input[type="checkbox"]')).toHaveCount(12);
  await expect(runner.locator('.browser-configuration input[value="browser/dom@1"]')).toBeChecked();
  await expect(runner.locator('.browser-configuration input[value="browser/keyboard-events@1"]')).toBeChecked();

  await runner.getByRole("button", { name: "Minimal" }).click();
  await expect(runner.locator('.browser-configuration input[value="browser/dom-presentation@1"]')).toBeChecked();
  await expect(runner.locator('.browser-configuration input[value="browser/keyboard-events@1"]')).not.toBeChecked();
  await runner.getByRole("button", { name: "Interactive" }).click();
  await runner.locator('.browser-configuration input[value="browser/indexeddb@1"]').check();
  await runner.getByRole("button", { name: "Review Host" }).press("Enter");

  await expect(runner.locator(".browser-configuration-review")).toContainText("browser/indexeddb@1");
  await expect(runner.locator(".browser-configuration pre")).toContainText("host creche-browser-page");
  await expect(runner.locator(".browser-configuration")).toContainText("self-joining Body spore (separate later step)");
  await expect(runner.locator(".browser-configuration")).toContainText("Configuration creates no HostId, BootId");
  await runner.getByRole("button", { name: "Back / Edit" }).click();
  await expect(runner.getByRole("button", { name: "Review Host" })).toBeVisible();
  await runner.getByRole("button", { name: "Review Host" }).click();
  await expect(runner.locator('[data-stage="obtain"]')).toHaveClass(/complete/);

  await runner.getByRole("button", { name: "Bind Body invitation" }).click();
  await expect(runner.locator('[data-stage="bind"]')).toHaveClass(/complete/);
  const evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.obtainment.distribution_sha256).toBe(release.bundle_sha256);
  expect(evidence.obtainment.image_content_digest).not.toBe(release.bundle_sha256);
  expect(evidence.obtainment.builder_adapter).toBe("conduit-host-browser/bind-prebuilt@1");
  expect(evidence.obtainment.compiler_started).toBe(false);
  expect(evidence.obtainment.build_id).toMatch(/^build:sha256:/);
  expect(evidence.obtainment.image_id).toMatch(/^image:sha256:/);
  expect(evidence.binding.image_content_digest).toBe(evidence.obtainment.image_content_digest);
  expect(evidence.binding.spore_artifact.files).toContainEqual(expect.objectContaining({ path: "conduit-browser-image.json" }));
  expect(evidence.binding).toMatchObject({
    target_id: "browser/wasm32/page",
    browser_configuration_id: expect.stringMatching(/^sha256:/),
    browser_profile_id: expect.stringMatching(/^sha256:/),
    browser_configuration_source: expect.stringContaining("browser/indexeddb@1"),
  });
});

test("stale restored browser choices are refused before lifecycle change", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await page.evaluate(async () => {
    const { createBrowserConfigurationOutfitter } = await import(new URL("../creche-browser-configuration.mjs", location.href).href);
    const root = document.createElement("div");
    root.id = "stale-browser-configuration";
    const outfitter = createBrowserConfigurationOutfitter({
      host: globalThis.__conduitCrecheHost,
      restoredSelection: { catalog_generation: 0, implementations: ["browser/retired@1"] },
      onChange() {},
    });
    root.append(outfitter.render());
    document.body.append(root);
  });
  await expect(page.locator("#stale-browser-configuration [role=alert]")).toContainText("StaleCatalogGeneration");
  await expect(page.locator("#stale-browser-configuration")).not.toContainText("PROFILE");
  expect(await page.evaluate(() => globalThis.__conduitCrecheHost.runtime.conduit_creche_current())).toBe(1);
});

async function startCreche() {
  const product = process.env.CONDUIT_CRECHE_PRODUCT_ROOT ?? "target/creche-product";
  const child = spawn("target/debug/conduit-browser-host", ["--application", product, "--mount", "/creche/", "--no-open"], {
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

async function installBrowserRelease(page) {
  const root = process.env.CONDUIT_CRECHE_PRODUCT_ROOT
    ? new URL(`../../${process.env.CONDUIT_CRECHE_PRODUCT_ROOT}/artifacts/`, import.meta.url)
    : new URL("../../target/creche-product/artifacts/", import.meta.url);
  const manifest = JSON.parse(await readFile(new URL("browser-page.json", root), "utf8"));
  for (const file of manifest.files) {
    const bytes = await readFile(new URL(file.path, root));
    await page.route(`**/artifacts/${file.path}`, (route) => route.fulfill({
      status: 200,
      contentType: file.media_type,
      body: bytes,
    }));
  }
  await page.route("**/artifacts/browser-page.json", (route) => route.fulfill({
    status: 200,
    contentType: "application/json",
    body: JSON.stringify(manifest),
  }));
  return manifest;
}
