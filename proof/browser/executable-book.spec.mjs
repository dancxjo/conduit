import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { installB7Devices } from "./b7-fixture.mjs";

let entrance;

async function startBook() {
  const child = spawn("target/debug/conduit-browser-host", ["--book", "--no-open"], {
    cwd: new URL("../..", import.meta.url).pathname,
    env: { ...process.env, CONDUIT_BROWSER_RUNTIME_WASM: "target/conduit_book_runtime.wasm" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(
      () => reject(new Error(`executable book was not ready\n${output}`)),
      10_000,
    );
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/CONDUIT_BROWSER_HOST_URL=(http:\/\/127\.0\.0\.1:\d+\/book\/)/);
      if (match) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    };
    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("exit", (code) => {
      clearTimeout(timeout);
      reject(new Error(`executable book exited (${code})\n${output}`));
    });
  });
  return { child, url };
}

async function startCreche() {
  const child = spawn("target/debug/conduit-browser-host", ["--creche", "--no-open"], {
    cwd: new URL("../..", import.meta.url).pathname,
    env: { ...process.env, CONDUIT_BROWSER_RUNTIME_WASM: "target/conduit_creche_runtime.wasm" },
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

async function startStaticProduct(root, mount = "/") {
  const child = spawn("node", ["proof/browser/static-server.mjs", "0", root, mount], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  const url = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`staged product was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/CONDUIT_STATIC_SERVER_URL=(http:\/\/127\.0\.0\.1:\d+\/\S*)/);
      if (match) { clearTimeout(timeout); resolve(match[1]); }
    };
    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("exit", (code) => { clearTimeout(timeout); reject(new Error(`staged product exited (${code})\n${output}`)); });
  });
  return { child, url };
}

async function openStep(page, index) {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  for (let current = 0; current < index; current += 1) {
    await page.getByRole("button", { name: "Next" }).click();
  }
  await expect(page.locator(".book-progress")).toHaveText(new RegExp(`^Page ${index + 1} of \\d+$`));
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
    await birth.locator(".seed-source summary").click();
    const source = birth.locator("textarea");
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

test("the staged Book and Crèche each boot with only their own product tree", async ({ page }) => {
  const book = await startStaticProduct("target/book-product");
  try {
    await page.goto(`${book.url}index.html`);
    await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
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
    for (const artifact of ["hosted-linux-workstation.json", "hosted-linux-server.json", "conduit-linux-x86_64", "browser-page.json", "runtime.wasm", "index.html", "host.mjs"]) {
      expect((await page.request.get(`${creche.url}artifacts/${artifact}`)).status()).toBe(200);
    }
    for (const artifact of ["raspios-bookworm-pi4-model-b-rev-1.5-4gb.json", "conduit-linux-aarch64", "rpi-b-plus-image.json", "conduitos-rpi-b-plus.img"]) {
      expect((await page.request.get(`${creche.url}artifacts/${artifact}`)).status()).toBe(200);
    }
    const birth = page.locator(".body-birth-runner");
    await birth.getByRole("button", { name: "Birth Body" }).click();
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
    const spore = runner.locator(".download-spore");
    await expect(spore).toHaveAttribute("download", /\.spore$/);
    evidence = JSON.parse(await runner.locator("details code").textContent());
    expect(evidence.binding).toMatchObject({
      target_id: c3Manifest.target_id,
      image_content_digest: evidence.obtainment.artifact.content_digest,
    });
    expect(evidence.binding.image_id).toMatch(/^image:sha256:[0-9a-f]{64}$/);
    const bundle = await spore.evaluate(async (link) => {
      const bytes = new Uint8Array(await (await fetch(link.href)).arrayBuffer());
      const manifestLength = new DataView(bytes.buffer, bytes.byteOffset + 8, 4).getUint32(0, true);
      return {
        magic: new TextDecoder().decode(bytes.subarray(0, 8)),
        manifest: JSON.parse(new TextDecoder().decode(bytes.subarray(12, 12 + manifestLength))),
      };
    });
    expect(bundle).toMatchObject({
      magic: "CNDSPOR1",
      manifest: {
        schema: "conduit.spore/bundle@1",
        spore: {
          target: c3Manifest.target_id,
          image_id: evidence.binding.image_id,
          image_content_digest: evidence.binding.image_content_digest,
        },
        artifact: { layout: { release: { image_id: c3Manifest.image_id } } },
      },
    });
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
    await expect(runner.locator(".download-spore")).toHaveCount(0);
    expect(requestedPaths).toContain("/conduit/creche/artifacts/esp32-c3-generic-release.json");
    expect(requestedPaths).not.toContain("/conduit/creche/artifacts/esp32-c3-generic-release.bin");
    expect(await page.evaluate(() => globalThis.__crecheDeviceAuthorityRequests)).toBe(0);
  } finally {
    creche.child.kill();
  }
});

test("the Book renders Markdown emphasis semantically and leaves raw HTML inert", async ({ page }) => {
  await page.route("**/book/chapter-1.md", (route) => route.fulfill({
    contentType: "text/markdown; charset=utf-8",
    body: "# Markdown proof\n\n*asterisk* _underscore_ **strong asterisk** __strong underscore__ <img src=x onerror=globalThis.__rawHtmlRan=true>",
  }));
  await openStep(page, 0);
  await expect(page.locator("em")).toHaveText(["asterisk", "underscore"]);
  await expect(page.locator("strong")).toHaveText(["strong asterisk", "strong underscore"]);
  await expect(page.locator("#chapter")).not.toContainText("*asterisk*");
  await expect(page.locator("#chapter")).not.toContainText("_underscore_");
  await expect(page.locator("#chapter img")).toHaveCount(0);
  await expect(page.locator("#chapter")).toContainText("<img src=x onerror=globalThis.__rawHtmlRan=true>");
  expect(await page.evaluate(() => globalThis.__rawHtmlRan)).toBeUndefined();
});

test("the Book opens as readable documentation and hands birth to the independent Crèche", async ({ page }) => {
  const responses = [];
  page.on("response", (response) => responses.push(new URL(response.url()).pathname));
  await openStep(page, 0);
  await expect(page.getByRole("heading", { name: "Bodies begin somewhere" })).toBeVisible();
  await expect(page).toHaveTitle(/The Book$/);
  await expect(page.locator('.chapter-copy em', { hasText: "intended" })).toHaveCount(1);
  await expect(page.locator('.chapter-copy strong', { hasText: /^Body$/ })).toHaveCount(1);
  await expect(page.locator("#chapter")).not.toContainText("_intended_");
  await expect(page.locator("#chapter")).not.toContainText("**Body**");
  await expect(page.locator(".body-birth-runner, .first-host-runner, .physical-host-runner, .graduation-runner")).toHaveCount(0);
  await expect(page.locator(".gear-inventory")).toHaveCount(0);
  const handoff = page.getByRole("link", { name: "Birth a Body" });
  await expect(handoff).toHaveAttribute("href", "../creche/");
  await expect(page.locator('meta[name="conduit-creche-url"]')).toHaveAttribute("content", "../creche/");
  const bookRuntimeExports = await page.evaluate(() => Object.keys(globalThis.__conduitBookHost.runtime));
  expect(bookRuntimeExports.some((name) => name.startsWith("conduit_creche_"))).toBe(false);
  expect((await page.request.get(new URL("/creche/", entrance.url).href)).status()).toBe(404);
  expect((await page.request.get(new URL("/book/creche-lifecycle.mjs", entrance.url).href)).status()).toBe(404);
  expect(responses.some((path) => path.includes("creche-lifecycle") || path.includes("creche-physical") || path.includes("creche-graduation"))).toBe(false);
  await page.reload();
  await expect(page.getByRole("heading", { name: "Bodies begin somewhere" })).toBeVisible();
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
  await birth.getByLabel("Friendly Body name").fill("standalone firefly");
  await birth.getByRole("button", { name: "Birth Body" }).click();
  const bodyId = await birth.getAttribute("data-body-id");
  expect(bodyId).toMatch(/^[0-9a-f]{64}$/);
  await page.getByRole("button", { name: "2. First Host" }).click();
  await page.getByRole("button", { name: "Give this Body its first Host" }).click();
  await page.getByRole("button", { name: "4. Graduate" }).click();
  await page.getByRole("button", { name: "Finish without hosted Patchbay" }).click();
  await expect(page.locator(".graduation-runner")).toHaveAttribute("data-body-id", bodyId);
  await expect(page.locator(".body-biography li")).toHaveCount(4);
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
    runner.locator(".seed-source").boundingBox(),
    runner.locator(".birth-editor").boundingBox(),
  ]);
  for (const box of [program, name, source, editor]) expect(box).not.toBeNull();
  expect(program.y + program.height).toBeLessThanOrEqual(name.y);
  expect(name.y + name.height).toBeLessThanOrEqual(source.y);
  for (const control of [program, name, source]) {
    expect(control.x).toBeGreaterThanOrEqual(editor.x);
    expect(control.x + control.width).toBeLessThanOrEqual(editor.x + editor.width);
  }
  await runner.locator(".seed-source summary").click();
  await expect(runner.locator(".seed-source textarea")).toBeVisible();
  const selectAppearance = await runner.getByLabel("Initial program").evaluate(
    (element) => getComputedStyle(element).appearance,
  );
  expect(selectAppearance).toBe("none");
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
    return { birth, evidence: JSON.parse(await runner.locator("details code").textContent()) };
  };
  const first = await prepareOne("A");
  const second = await prepareOne("B");
  expect(first.birth.bodyId).not.toBe(second.birth.bodyId);
  expect(first.evidence.obtainment.artifact.content_digest).toBe("sha256:b373071c9bf76282457a5f03e59e5d5caaba21e376076b759724434efcf2bc9d");
  expect(first.evidence.obtainment.artifact.content_digest).toBe(second.evidence.obtainment.artifact.content_digest);
  expect(first.evidence.obtainment.artifact.artifact_id).toBe("conduit-pico-w-signal/pico-local-b7@1");
  expect(first.evidence.binding.spore_id).not.toBe(second.evidence.binding.spore_id);
  expect(first.evidence.binding.invitation_id).not.toBe(second.evidence.binding.invitation_id);
  expect(first.evidence.binding.image_content_digest).toBe(second.evidence.binding.image_content_digest);
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
  await expect(runner.locator(".physical-target optgroup")).toHaveCount(6);
  await expect(runner.locator(".physical-target optgroup").nth(0)).toHaveAttribute("label", "RP2040 boards");
  await expect(runner.locator(".physical-target optgroup").nth(1)).toHaveAttribute("label", "SparkFun Pro Micro");
  await expect(runner.locator(".physical-target optgroup").nth(2)).toHaveAttribute("label", "ESP32 boards");
  await expect(runner.locator(".physical-target optgroup").nth(3)).toHaveAttribute("label", "Linux computers");
  await expect(runner.locator(".physical-target optgroup").nth(4)).toHaveAttribute("label", "Browser Hosts");
  await expect(runner.locator(".physical-target optgroup").nth(5)).toHaveAttribute("label", "Raspberry Pi computers");
  await expect(runner.locator(".physical-target option")).toHaveText([
    "Raspberry Pi Pico W · RP2040",
    "Pro Micro · ATmega32U4 · 5 V / 16 MHz",
    "ESP32-C3",
    "ESP32-S3",
    "ESP32-WROOM-32 · HW-463",
    "Hosted Linux workstation",
    "Hosted Linux server",
    "Browser page Host",
    "Pi 4 Model B rev 1.5 (4 GB) · Raspberry Pi OS Bookworm 64-bit",
    "Model B+ v1.2 · ARMv6 · bare-metal ConduitOS",
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
    package_id: "browser-wasm@1",
    output: "browser-bundle",
    bundle_sha256: release.bundle_sha256,
  });

  await runner.getByRole("button", { name: "Bind Body invitation" }).click();
  await expect(runner.locator('[data-stage="bind"]')).toHaveClass(/complete/);
  await expect(runner.locator(".download-spore")).toHaveAttribute("download", /\.spore$/);
  evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.binding).toMatchObject({
    body_id: birth.bodyId,
    target_id: "browser/wasm32/page",
    output: "browser-bundle",
    fabrication_package_id: "browser-wasm@1",
    deployment_adapter: "conduit-host-browser/load@1",
    image_content_digest: release.bundle_sha256,
  });
  const bundle = await runner.locator(".download-spore").evaluate(async (link) => {
    const bytes = new Uint8Array(await (await fetch(link.href)).arrayBuffer());
    const manifestLength = new DataView(bytes.buffer, bytes.byteOffset + 8, 4).getUint32(0, true);
    return {
      magic: new TextDecoder().decode(bytes.subarray(0, 8)),
      manifest: JSON.parse(new TextDecoder().decode(bytes.subarray(12, 12 + manifestLength))),
    };
  });
  expect(bundle.magic).toBe("CNDSPOR1");
  expect(bundle.manifest.artifact.layout).toMatchObject({
    format: "browser-bundle",
    release: { target_id: "browser/wasm32/page", bundle_sha256: release.bundle_sha256 },
  });

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
  const release = await installHostRelease(page, "hosted-linux-workstation.json");
  await birthStandaloneBody(page, { sourceVariant: "native-existing-computer" });
  await page.getByRole("button", { name: "3. Physical Host" }).click();
  const runner = page.locator(".physical-host-runner");
  await runner.locator(".physical-target").selectOption("std/x86_64/workstation");
  await expect(runner.locator(".physical-mode")).toHaveValue("install-existing");
  await expect(runner.locator('[data-stage="obtain"]')).toHaveClass(/complete/);
  await runner.getByRole("button", { name: "Bind Body invitation" }).click();
  await expect(runner.locator(".download-spore")).toHaveAttribute("download", /\.spore$/);
  let evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.target_entry.target_profile).toMatchObject({
    os: "linux",
    architecture: "x86_64",
    package_id: "hosted-native@1",
    implemented_carriers: ["conduit-carrier/browser-release-download@1"],
  });
  expect(evidence.binding).toMatchObject({
    target_id: "std/x86_64/workstation",
    output: "native-bundle",
    fabrication_package_id: "hosted-native@1",
    image_content_digest: release.bundle_sha256,
  });
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
    });

    await runner.getByRole("button", { name: "Bind Body invitation" }).click();
    await expect(runner.locator('[data-stage="bind"]')).toHaveClass(/complete/);
    await expect(runner.locator(".download-spore")).toHaveAttribute("download", /\.spore$/);
    evidence = JSON.parse(await runner.locator("details code").textContent());
    const bundle = await runner.locator(".download-spore").evaluate(async (link) => {
      const bytes = new Uint8Array(await (await fetch(link.href)).arrayBuffer());
      const magic = new TextDecoder().decode(bytes.subarray(0, 8));
      const manifestLength = new DataView(bytes.buffer, bytes.byteOffset + 8, 4).getUint32(0, true);
      const manifest = JSON.parse(new TextDecoder().decode(bytes.subarray(12, 12 + manifestLength)));
      return { magic, manifest, bytes: bytes.length };
    });
    expect(bundle.magic).toBe("CNDSPOR1");
    expect(bundle.manifest).toMatchObject({
      schema: "conduit.spore/bundle@1",
      spore: {
        schema: "conduit.body/spore-manifest@2",
        target: profile.id,
        image_content_digest: evidence.binding?.image_content_digest,
      },
      artifact: {
        layout: { format: "espressif-merged-image" },
      },
    });
    expect(bundle.bytes).toBeGreaterThan(evidence.obtainment.artifact.bytes);
    expect(evidence.binding).toMatchObject({
      target_id: profile.id,
      output: "esp32-image",
      fabrication_package_id: "conduit-host-esp32@1",
    });
    expect(evidence.binding.image_content_digest).toBe(evidence.obtainment.artifact.content_digest);
  }
});

test("an unavailable generic ESP32 release refuses before device authority or spore creation", async ({ page }) => {
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
    const { createPhysicalHostTargetCatalog } = await import("./creche-target-catalog.mjs");
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
    const { createPhysicalHostRunner } = await import("./creche-physical.mjs");
    const { createPhysicalHostTargetCatalog } = await import("./creche-target-catalog.mjs");
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
    const { createPhysicalHostRunner } = await import("./creche-physical.mjs");
    const { createPhysicalHostTargetCatalog } = await import("./creche-target-catalog.mjs");
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

test("all guided pages lead with human motivation and return to the Conduit payoff", async ({
  page,
}) => {
  const anchors = [
    "build one computer out of the computers you actually have",
    "A successful deployment is only deployment",
    "useful programs will still evolve",
    "different finite machines",
    "vocabulary fragments",
    "should not have to copy its internal machinery",
    "constrained systems unnecessarily large",
    "minimal viable Conduit Host",
    "bounded and portable",
    "Machine-specific truth stays with the machine",
    "Look what just happened",
    "replaceable answer to current circumstances",
    "durable computer Conduit maintains",
    "described enduring meaning",
  ];
  await openStep(page, 0);
  for (let step = 0; step < anchors.length; step += 1) {
    await expect(page.locator("#chapter")).toContainText(anchors[step]);
    await expect(page.getByRole("heading", { name: "Conduit idea" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Payoff" })).toBeVisible();
    const firstParagraph = page.locator(".chapter-copy").first().locator("p").first();
    await expect(firstParagraph).toBeVisible();
    if (step < anchors.length - 1) await page.getByRole("button", { name: "Next" }).click();
  }
});

test("the substitution, explicit fan-out, and generic-verb pages execute in order", async ({ page }) => {
  await openStep(page, 2);
  const hostId = await page.evaluate(() => globalThis.__conduitBookHost.hostId);
  let runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("HELLO");
  await expect(runner.locator(".play-status")).toContainText("Completed");

  await page.getByRole("button", { name: "Next" }).click();
  await expect(page.getByRole("heading", { name: "Fan out explicitly" })).toBeVisible();
  runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("true");
  await expect(runner.locator(".play-status")).toContainText("Completed");

  await page.getByRole("button", { name: "Next" }).click();
  await expect(page.getByRole("heading", { name: "Use a generic verb" })).toBeVisible();
  runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("3.000000");
  await expect(runner.locator(".play-status")).toContainText("Completed");
  expect(await page.evaluate(() => globalThis.__conduitBookHost.hostId)).toBe(hostId);
});

test("the Back pages compare two realizations deliberately", async ({ page }) => {
  await openStep(page, 5);
  await expect(page.getByLabel("Morse realization")).toHaveCount(0);
  let runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("·");
  await expect(runner.locator(".play-status")).toContainText("Completed");
  await expect(runner.locator(".expansion")).toContainText("Selected realization: direct");
  await expect(runner.locator(".expansion")).not.toContainText("Opened reusable Forms");

  await page.getByRole("button", { name: "Next" }).click();
  runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("··· ——— ···");
  await expect(runner.locator(".expansion")).toContainText("Selected realization: direct");
  await expect(runner.locator(".expansion")).not.toContainText("Opened reusable Forms");

  await page.getByRole("button", { name: "Next" }).click();
  const comparison = page.locator(".realization-comparison");
  await expect(comparison.locator(".shared-face span")).toHaveText("Same requested Face");
  await expect(comparison.locator(".shared-face code")).toHaveText(
    "text/morse · text: value/text@1 → pattern: value/morse-pattern@1",
  );
  await expect(page.getByText("minimal viable Conduit Host", { exact: false })).toBeVisible();
  const direct = comparison.locator(".runner").nth(0);
  const recursive = comparison.locator(".runner").nth(1);
  await expect(direct.getByRole("heading", { name: "Direct leaf" })).toBeVisible();
  await expect(recursive.getByRole("heading", { name: "Recursive Form Back" })).toBeVisible();
  const listing = direct.locator("textarea");
  await listing.fill((await listing.inputValue()).replace('"HELLO"', '"E"'));
  await expect(recursive.locator("textarea")).toHaveValue(await listing.inputValue());
  await direct.getByRole("button", { name: "Run direct leaf" }).click();
  await expect(direct.locator(".play-status")).toContainText("Completed");
  await recursive.getByRole("button", { name: "Run recursive Back" }).click();
  await expect(recursive.locator(".play-status")).toContainText("Completed");
  await expect(direct.locator(".morse")).toHaveText("·");
  await expect(recursive.locator(".morse")).toHaveText("·");
  await expect(direct.locator(".expansion")).toContainText("Selected realization: direct");
  await expect(direct.locator(".expansion")).not.toContainText("Opened reusable Forms");
  await expect(recursive.locator(".expansion")).toContainText("Selected realization: recursive");
  await expect(recursive.locator(".expansion")).toContainText("Opened reusable Forms");
  await expect(recursive.locator(".expansion")).toContainText("morse/lookup");
  const directIdentities = await direct.locator("details dd").allTextContents();
  const recursiveIdentities = await recursive.locator("details dd").allTextContents();
  expect(directIdentities[0]).toBe(recursiveIdentities[0]);
  expect(directIdentities[1]).toBe(recursiveIdentities[1]);
  expect(directIdentities[2]).not.toBe(recursiveIdentities[2]);
  expect(directIdentities[3]).not.toBe(recursiveIdentities[3]);
});

test("Book navigation preserves executable drafts without owning lifecycle controls", async ({ page }) => {
  await openStep(page, 2);
  const hostId = await page.evaluate(() => globalThis.__conduitBookHost.hostId);
  const edited = (await page.locator("textarea").inputValue()).replace('"hello"', '"reader"');
  await page.locator("textarea").fill(edited);
  await page.getByRole("button", { name: "Next" }).click();
  await page.getByRole("button", { name: "Previous" }).click();
  await expect(page.locator("textarea")).toHaveValue(edited);
  expect(await page.evaluate(() => globalThis.__conduitBookHost.hostId)).toBe(hostId);
  await expect(page.getByRole("button", { name: "Reset this page" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Revisit birth page" })).toHaveCount(0);
});

test("unsupported capability and type mismatch remain ordinary pre-Play refusals", async ({ page }) => {
  await openStep(page, 2);
  const runner = page.locator(".runner");
  const listing = runner.locator("textarea");
  await listing.fill(`form unavailable {
    source: text/literal("still planned")
    result: presentation/text
    missing: layout/inset
    source > result
  }`);
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".play-status")).toContainText(
    "refused before Play · missing-implementation-or-placement",
  );
  await listing.fill(`form wrong-type {
    source: scalar/literal(1.0)
    invert: logic/not
    result: presentation/bool-value
    source > invert > result
  }`);
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".play-status")).toContainText(
    "refused before Play · type-or-source",
  );
  await expect(runner.locator(".indicator")).toHaveAttribute("aria-label", "Indicator off");
});

test("state over time presents startup and current count through four admitted browser ticks", async ({ page }) => {
  await openStep(page, 8);
  await expect(page.getByRole("heading", { name: "State over time" })).toBeVisible();
  const runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("0");
  await expect(runner.locator(".morse")).toHaveText("4");
  await expect(runner.locator(".play-status")).toContainText(
    "4 planned ticks, 5 presentations",
  );
  await expect(runner.locator(".play-status")).toHaveAttribute("data-timer-completions", "4");
  await expect(runner.locator(".play-status")).toHaveAttribute(
    "data-manifestation-completions",
    "5",
  );
  await expect(runner.locator("details dd")).toHaveCount(12);
});

test("stopping state over time cancels the pending timer without a late completion", async ({ page }) => {
  await openStep(page, 8);
  const runner = page.locator(".runner");
  await runner.getByRole("button", { name: "Run" }).click();
  await expect(runner.locator(".morse")).toHaveText("0");
  await runner.getByRole("button", { name: "Stop" }).click();
  await expect(runner.locator(".play-status")).toHaveText("Stopped. The Play was cancelled.");
  await page.waitForTimeout(650);
  await expect(runner.locator(".play-status")).toHaveText("Stopped. The Play was cancelled.");
  await expect(runner.locator(".play-status")).not.toHaveAttribute("data-receipt", /.+/);
  await expect(runner.locator(".morse")).toHaveText("0");
});

test("Meet the Host shows the exact installed offers from the planning advertisement", async ({ page }) => {
  await openStep(page, 9);
  await expect(page.getByRole("heading", { name: "Meet the Host" })).toBeVisible();
  const inventory = page.locator(".gear-inventory");
  await expect(inventory).toHaveCount(1);
  const visibleInstalled = await inventory.locator("li.available code").allTextContents();
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
});

test("Two browser Hosts executes one unchanged Form across independent Hosts", async ({ page }) => {
  await openStep(page, 10);
  await expect(page.getByRole("heading", { name: "Two browser Hosts" })).toBeVisible();
  const runner = page.locator(".multi-host-runner");
  const source = await runner.locator("textarea").inputValue();
  expect(source).not.toMatch(/HostId|BootId|browser\/|iframe|DOM|socket|address/);
  await runner.getByRole("button", { name: "Run across two Hosts" }).click();
  await expect(runner.locator(".play-status")).toContainText(
    "one immutable Plan, two independent Plays, one delivered cross-Host value",
  );
  await expect(runner.locator(".morse")).toHaveText("hello across one planned Cord");
  await expect(runner.locator(".host-a strong")).toHaveText("completed");
  await expect(runner.locator(".host-b strong")).toHaveText("completed");
  const identities = await page.evaluate(() => ({
    a: globalThis.__conduitBookHost,
    b: globalThis.__conduitBookPeerHost,
  }));
  expect(identities.a.hostId).not.toBe(identities.b.hostId);
  expect(identities.a.bootId).not.toBe(identities.b.bootId);
  await expect(page.locator("iframe")).toHaveCount(0);
  await expect(runner.locator(".projected-cord")).toContainText("1 item");
  await expect(runner.locator(".play-status")).toHaveAttribute("data-source-receipt", /.+/);
  await expect(runner.locator(".play-status")).toHaveAttribute("data-sink-receipt", /.+/);
});

test("Plans and Plays compact and raw views project the same exact immutable Plan", async ({ page }) => {
  await openStep(page, 11);
  await expect(page.getByRole("heading", { name: "Plans and Plays" })).toBeVisible();
  const runner = page.locator(".multi-host-runner");
  await expect(runner.locator(".plan-view-details")).toHaveAttribute("open", "");
  await runner.getByRole("button", { name: "Run across two Hosts" }).click();
  await expect(runner.locator(".play-status")).toContainText("Completed");
  const projectedPlanId = await runner.locator(".projected-plan-id").textContent();
  const rawPlan = JSON.parse(await runner.locator(".raw-plan code").textContent());
  expect(rawPlan.plan_id).toBe(projectedPlanId);
  expect(rawPlan.fragments).toHaveLength(2);
  expect(new Set(rawPlan.fragments.map((fragment) => fragment.host_id)).size).toBe(2);
  await expect(runner.locator(".projected-hosts article")).toHaveCount(2);
  await expect(runner.locator(".projected-hosts")).toContainText("text/literal");
  await expect(runner.locator(".projected-hosts")).toContainText("presentation/text");
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
  expect(evidence.binding.invitation_secret).toBe("redacted");
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
  await expect(runner.locator(".graduation-criteria li.ready")).toHaveCount(3);
  await runner.getByRole("button", { name: "Host Patchbay on this Body" }).click();
  await expect(runner).toHaveAttribute("data-body-id", bodyId);
  await expect(runner.locator(".graduation-evidence")).toContainText("browser/patchbay-surface@1");
  await expect(runner.locator(".graduation-evidence")).toContainText("Crèche requiredfalse");
  const biography = runner.locator(".body-biography");
  await expect(biography).toHaveAttribute("data-body-id", bodyId);
  await expect(biography.locator("li")).toHaveCount(4);
  await expect(biography.locator("strong")).toHaveText(["Born", "Part admitted", "Host joined", "Graduated from the Crèche"]);
  await expect(biography).toContainText("browser/patchbay-surface@1");
  await runner.getByRole("button", { name: "End the Crèche" }).click();
  await expect(page.locator(".creche-complete")).toContainText(bodyId);
  await expect(page.locator(".creche-steps")).toHaveCount(0);
  await expect(page.locator(".creche-complete .body-biography li")).toHaveCount(4);
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
  await expect(runner.locator(".graduation-evidence")).toContainText("Patchbay Plannot hosted");
  const evidence = JSON.parse(await runner.locator("details code").textContent());
  expect(evidence.choice).toBe("external-reader");
  expect(evidence.patchbay_plan_id).toBeNull();
  expect(evidence.creche_required).toBe(false);
  await expect(runner.locator(".body-biography li")).toHaveCount(4);
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
  await openStep(page, 10);
  const runner = page.locator(".multi-host-runner");
  await runner.getByRole("button", { name: "Run across two Hosts" }).click();
  await expect(runner.locator(".play-status")).toContainText("Host A offered one value");
  await runner.getByRole("button", { name: "Stop" }).click();
  await expect(runner.locator(".play-status")).toHaveText("Stopped. The Play was cancelled.");
  await page.evaluate(() => globalThis.__releaseBookAnimationFrame());
  await expect(runner.locator(".play-status")).toHaveText("Stopped. The Play was cancelled.");
  await expect(runner.locator(".morse")).toHaveText("ready");
  await expect(runner.locator(".play-status")).not.toHaveAttribute("data-source-receipt", /.+/);
});
