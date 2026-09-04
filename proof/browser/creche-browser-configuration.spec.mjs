import { spawn } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { reviewAndBirth } from "./creche-test-actions.mjs";

let entrance;

test.beforeEach(async () => { entrance = await startCreche(); });
test.afterEach(() => entrance?.child.kill());

test("one reviewed distribution fabricates materially different, capability-enforcing browser Hosts", async ({ browser }) => {
  const viewerPage = await browser.newPage();
  const richPage = await browser.newPage();
  const capabilityPage = await browser.newPage();
  const viewerRelease = await installBrowserRelease(viewerPage);
  const richRelease = await installBrowserRelease(richPage);
  expect(richRelease.bundle_sha256).toBe(viewerRelease.bundle_sha256);

  const viewer = await fabricateBrowserHost(viewerPage, { preset: "Minimal" });
  const rich = await fabricateBrowserHost(richPage, {
    preset: "Interactive",
    add: ["browser/indexeddb@1", "browser/websocket@1", "browser/webserial@1"],
  });

  expect(viewer.obtainment.distribution_sha256).toBe(rich.obtainment.distribution_sha256);
  expect(viewer.obtainment.compiler_started).toBe(false);
  expect(rich.obtainment.compiler_started).toBe(false);
  expect(viewer.binding.browser_profile_id).not.toBe(rich.binding.browser_profile_id);
  expect(viewer.obtainment.build_id).not.toBe(rich.obtainment.build_id);
  expect(viewer.obtainment.image_id).not.toBe(rich.obtainment.image_id);
  expect(viewer.realization.host_id).not.toBe(rich.realization.host_id);
  expect(viewer.realization.boot_id).not.toBe(rich.realization.boot_id);
  expect(viewer.realization.boot_observed).toBe(false);
  expect(viewer.realization.join_created).toBe(false);

  await capabilityPage.goto(`http://127.0.0.1:${process.env.CONDUIT_BROWSER_HOST_PORT ?? "4173"}/proof/browser/signal-dom-host.test.html`);
  const viewerUse = await exerciseProfile(capabilityPage, viewer.realization, false);
  const richUse = await exerciseProfile(capabilityPage, rich.realization, true);
  expect(viewerUse.pointer).toBe("UnsupportedInput");
  expect(viewerUse.storage).toBe("ImplementationNotSelected");
  expect(richUse.storage).toEqual({ plan_id: "plan/rich/storage", value: { count: 1 } });
  expect(richUse.pointer).toMatchObject({ plan_id: "plan/rich/pointer", schema: "input/pointer-event@1" });

  const websocket = rich.realization.inspection.find(({ implementation_id }) => implementation_id === "browser/websocket@1");
  expect(websocket).toMatchObject({ configured: true, offered: false, refusal: "EndpointUnavailable" });
  const refreshed = await capabilityPage.evaluate(async ({ bootTruth }) => {
    const boot = await import(new URL("../../targets/browser/host/assets/browser-boot-profile.mjs", location.href).href);
    return boot.refreshBrowserBootTruth(bootTruth, {
      "browser/websocket@1": { api_supported: true, secure_context: true, provider_ready: true, endpoint_ready: true, authority_ready: true },
    });
  }, { bootTruth: rich.realization.boot_truth });
  expect(refreshed.image_id).toBe(rich.realization.image_id);
  expect(refreshed.profile_id).toBe(rich.realization.profile_id);
  expect(refreshed.inspection.find(({ implementation_id }) => implementation_id === "browser/websocket@1")).toMatchObject({ offered: true, refusal: null });

  const evidence = {
    schema: "conduit.browser/two-profile-capstone@1",
    distribution_sha256: viewerRelease.bundle_sha256,
    viewer: summarizeProfile(viewer, viewerUse),
    rich: summarizeProfile(rich, richUse),
    prerequisite_refresh: {
      implementation_id: "browser/websocket@1",
      before: websocket.refusal,
      after: refreshed.inspection.find(({ implementation_id }) => implementation_id === "browser/websocket@1").offer_id,
      image_id_unchanged: refreshed.image_id === rich.realization.image_id,
    },
  };
  await retainCapstoneReport(evidence);
  await viewerPage.close();
  await richPage.close();
  await capabilityPage.close();
});

test("browser outfitting is catalog-driven, editable, and handed to checked fabrication", async ({ page }) => {
  const release = await installBrowserRelease(page);
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await reviewAndBirth(page);
  await page.getByRole("button", { name: "3. Physical Host" }).click();
  const runner = page.locator(".physical-host-runner");
  await runner.locator('[data-application-key="physical-target"]').selectOption("browser/wasm32/page");
  await expect(runner.locator('[data-application-key="physical-status"]')).toHaveAttribute("data-application-component", "status");
  await expect(runner.locator('[data-application-key="physical-evidence"]')).toHaveAttribute("data-application-component", "artifact");

  await expect(runner.locator('[data-application-key="physical-stage-obtain"]')).toContainText("waiting");
  await expect(runner.locator('fieldset[data-application-key^="configuration-group-"]')).toHaveCount(6);
  await expect(runner.locator('input[type="checkbox"][data-application-action^="implementation.change-"]')).toHaveCount(11);
  await expect(runner.getByRole("checkbox", { name: /browser\/web-audio-output@1/ })).toHaveCount(0);
  await expect(runner.getByRole("checkbox", { name: /browser\/dom@1/ })).toBeChecked();
  await expect(runner.getByRole("checkbox", { name: /browser\/keyboard-events@1/ })).toBeChecked();

  await runner.getByRole("button", { name: "Minimal" }).click();
  await expect(runner.getByRole("checkbox", { name: /browser\/dom-presentation@1/ })).toBeChecked();
  await expect(runner.getByRole("checkbox", { name: /browser\/keyboard-events@1/ })).not.toBeChecked();
  await runner.getByRole("button", { name: "Interactive" }).click();
  await runner.getByRole("checkbox", { name: /browser\/indexeddb@1/ }).check();
  await runner.getByRole("button", { name: "Review Host" }).press("Enter");

  await expect(runner.locator('[data-application-key="configuration-review-values"]')).toContainText("browser/indexeddb@1");
  await expect(runner.locator(".browser-configuration pre")).toContainText("host creche-browser-page");
  await expect(runner.locator(".browser-configuration")).toContainText("self-joining Body spore (separate later step)");
  await expect(runner.locator(".browser-configuration")).toContainText("Configuration creates no HostId, BootId");
  await runner.getByRole("button", { name: "Back / Edit" }).click();
  await expect(runner.getByRole("button", { name: "Review Host" })).toBeVisible();
  await runner.getByRole("button", { name: "Review Host" }).click();
  await expect(runner.locator('[data-application-key="physical-stage-obtain"]')).not.toContainText("waiting");

  await runner.getByRole("button", { name: "Bind Body invitation" }).click();
  await expect(runner.locator('[data-application-key="physical-stage-bind"]')).not.toContainText("waiting");
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

  await runner.getByRole("button", { name: "Realize selected Host" }).click();
  await expect(runner.locator('[data-application-key="physical-stage-realize"] dd')).toHaveText("BrowserBundleLoaded");
  const realized = JSON.parse(await runner.locator("details code").textContent()).realization;
  expect(realized).toMatchObject({
    schema: "conduit.browser/creche-bundle-load@1",
    image_id: evidence.obtainment.image_id,
    profile_id: evidence.obtainment.profile_id,
    boot_module_sha256: expect.stringMatching(/^sha256:/),
  });
  expect(realized.implementation_registry.map(({ id }) => id).sort()).toEqual(
    ["browser/dom@1", "browser/indexeddb@1", "browser/keyboard-events@1", "browser/pointer-events@1"].sort(),
  );
  expect(realized.inspection.every(({ configured }) => configured)).toBe(true);
  expect(realized.inspection.some(({ implementation_id }) => implementation_id === "browser/media-devices-camera@1")).toBe(false);
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
      presentationFor: globalThis.__conduitBrowserApplication.presentationFor,
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

async function fabricateBrowserHost(page, { preset, add = [] }) {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await reviewAndBirth(page);
  await page.getByRole("button", { name: "3. Physical Host" }).click();
  const runner = page.locator(".physical-host-runner");
  await runner.locator('[data-application-key="physical-target"]').selectOption("browser/wasm32/page");
  await runner.getByRole("button", { name: preset }).click();
  for (const implementation of add) {
    const button = runner.getByRole("button").filter({ hasText: implementation });
    if ((await button.textContent()).includes("Add")) await button.click();
  }
  await runner.getByRole("button", { name: "Review Host" }).click();
  await runner.getByRole("button", { name: "Bind Body invitation" }).click();
  let evidence = JSON.parse(await runner.locator("details code").textContent());
  await runner.getByRole("button", { name: "Realize selected Host" }).click();
  await expect(runner.locator('[data-application-key="physical-stage-realize"] dd')).toHaveText("BrowserBundleLoaded");
  evidence = JSON.parse(await runner.locator("details code").textContent());
  return evidence;
}

async function exerciseProfile(page, realization, rich) {
  return page.evaluate(async ({ realization, rich }) => {
    const { openBrowserHumanInput } = await import(new URL("../../targets/browser/host/assets/browser-human-input.mjs", location.href).href);
    const { openBrowserApplicationStorage } = await import(new URL("../../targets/browser/host/assets/browser-application-storage.mjs", location.href).href);
    const root = document.createElement("button");
    root.textContent = "capstone input surface";
    root.style.cssText = "display:block;width:200px;height:100px";
    document.body.append(root);
    const input = openBrowserHumanInput({ target: root, boot: realization });
    let pointer;
    if (rich) {
      pointer = await new Promise((resolve, reject) => {
        const stop = input.observePointer((value, error) => { stop(); error ? reject(error) : resolve({ ...value, plan_id: "plan/rich/pointer" }); });
        root.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, clientX: 50, clientY: 25, buttons: 1 }));
      });
    } else {
      try { input.observePointer(() => {}); pointer = "accepted"; } catch (error) { pointer = error.code; }
    }
    let storage;
    try {
      const adapter = await openBrowserApplicationStorage("conduit.form/profile-capstone", 1, `sha256:${"a".repeat(64)}`, {
        implementationRegistry: realization.implementation_registry.map(({ id }) => id),
      });
      await adapter.writeJson("ordinary-plan", { count: 1 });
      storage = { plan_id: "plan/rich/storage", value: await adapter.readJson("ordinary-plan") };
      await adapter.clearApplication();
      adapter.close();
    } catch (error) { storage = error.code; }
    input.close();
    root.remove();
    return { pointer, storage };
  }, { realization, rich });
}

function summarizeProfile(evidence, use) {
  return {
    configuration_id: evidence.binding.browser_configuration_id,
    configuration_source: evidence.binding.browser_configuration_source,
    profile_id: evidence.binding.browser_profile_id,
    build_id: evidence.obtainment.build_id,
    image_id: evidence.obtainment.image_id,
    spore_id: evidence.binding.spore_id,
    host_id: evidence.realization.host_id,
    boot_id: evidence.realization.boot_id,
    runtime_sha256: evidence.realization.boot_truth.image.files.find(({ path }) => path === "runtime.wasm").sha256,
    boot_module_sha256: evidence.realization.boot_module_sha256,
    implementations: evidence.realization.implementation_registry,
    inspection: evidence.realization.inspection,
    active_use: use,
  };
}

async function retainCapstoneReport(evidence) {
  const root = new URL("../../target/proof/", import.meta.url);
  await mkdir(root, { recursive: true });
  const has = (profile, id) => profile.implementations.some((item) => item.id === id) ? "yes" : "no";
  const serial = evidence.rich.inspection.find(({ implementation_id }) => implementation_id === "browser/webserial@1");
  const markdown = `# Browser two-profile fabrication capstone\n\nExact evidence: \`browser-two-profile-capstone.json\`\n\n| capability | viewer | rich host |\n|---|---:|---:|\n| presentation | ${has(evidence.viewer, "browser/dom-presentation@1")} | ${has(evidence.rich, "browser/dom-presentation@1")} |\n| pointer | ${has(evidence.viewer, "browser/pointer-events@1")} | active Plan/Play |\n| storage | ${has(evidence.viewer, "browser/indexeddb@1")} | active Plan/Play |\n| WebSerial | ${has(evidence.viewer, "browser/webserial@1")} | selected / ${serial?.refusal ?? "current"} |\n| WebSocket | ${has(evidence.viewer, "browser/websocket@1")} | unavailable, then offered without IMAGE mutation |\n`;
  await writeFile(new URL("browser-two-profile-capstone.json", root), `${JSON.stringify(evidence, null, 2)}\n`);
  await writeFile(new URL("browser-two-profile-capstone.md", root), markdown);
}
