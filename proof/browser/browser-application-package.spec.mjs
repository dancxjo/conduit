import { expect, test } from "@playwright/test";
import { openBookStep, startBook, startStaticProduct } from "./book-test-server.mjs";

let entrance;

test.beforeEach(async () => { entrance = await startBook(); });
test.afterEach(() => entrance?.child.kill());

async function mutatePackage(page, mutate) {
  await page.route("**/book.application.json", async (route) => {
    const response = await route.fetch();
    const manifest = await response.json();
    mutate(manifest);
    await route.fulfill({ response, contentType: "application/json", body: JSON.stringify(manifest) });
  }, { times: 1 });
}

test("Book drafts and an open reviewed Back endure a same-browser reload", async ({ page }) => {
  entrance.child.kill();
  entrance = await startStaticProduct("target/book-product", "/conduit/book/");
  const requests = [];
  page.on("request", (request) => {
    if (request.url().startsWith("http:")) requests.push(new URL(request.url()).pathname);
  });
  await page.goto(`${entrance.url}faces-backs-and-implementation/`);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  await expect(page.getByRole("heading", { name: "Faces, Backs, and implementation" })).toBeVisible();
  const admission = await page.evaluate(() => ({
    packageDigest: globalThis.__conduitBrowserApplication.manifest.packageDigest,
    stateIdentity: globalThis.__conduitBrowserApplication.manifest.stateCompatibility.identity,
    storageIdentity: globalThis.__conduitBrowserApplication.storage.applicationIdentity,
    storagePackageDigest: globalThis.__conduitBrowserApplication.storage.packageDigest,
    paths: globalThis.__conduitBrowserApplication.manifest.resources.map((resource) => resource.path),
    resourceUrls: globalThis.__conduitBrowserApplication.manifest.resources.map((resource) => resource.url.href),
  }));
  expect(admission.packageDigest).toMatch(/^sha256:[0-9a-f]{64}$/);
  expect(admission.storagePackageDigest).toBe(admission.packageDigest);
  expect(admission.storageIdentity).toBe(admission.stateIdentity);
  expect(admission.paths).toContain("book-runner-presentation.mjs");
  expect(admission.paths).toContain("book-syntax-editor.mjs");
  expect(admission.paths).toContain("assets/flow.css");
  for (const [index, path] of admission.paths.entries()) {
    const pathname = new URL(admission.resourceUrls[index]).pathname;
    expect(requests.filter((request) => request === pathname), path).toHaveLength(1);
  }
  await expect(page.locator('script[data-application-resource="react"]')).toHaveAttribute("src", /^blob:/);
  await expect(page.locator('style[data-application-resource="book-style"]')).toHaveCount(1);
  await expect(page.locator('style[data-application-resource="patchbay-flow-style"]')).toHaveCount(1);
  const separatedStyles = await page.evaluate(() => {
    const application = globalThis.__conduitBrowserApplication;
    const decode = (role) => new TextDecoder().decode(application.bytes(role));
    return { book: decode("book-style"), flow: decode("patchbay-flow-style") };
  });
  expect(separatedStyles.book).not.toContain(".flow-faceplate header");
  expect(separatedStyles.book).not.toContain(".react-flow__edge-path");
  expect(separatedStyles.flow).toContain(".flow-faceplate header");
  expect(separatedStyles.flow).toContain(".react-flow__edge.animated");
  const runnerStatus = page.locator('[data-application-key="play-status"]');
  await expect(runnerStatus).toHaveAttribute("data-application-component", "status");
  await expect(runnerStatus).toHaveText("Edit the message or timing, then run it.");
  await expect(page.locator(".play-status")).toHaveCount(0);
  const listing = page.locator("textarea");
  const edited = (await listing.inputValue()).replace('"HELLO"', '"DURABLE"');
  await listing.fill(edited);
  const patchbay = page.locator(".compact-patchbay");
  await patchbay.getByRole("button", { name: "Open reviewed Back for same-morse-caller/morse" }).click();
  await expect(patchbay.locator(".gear-back-expansion")).toBeVisible();
  await page.evaluate(() => globalThis.__conduitBookPersistence.flush());

  await page.reload();
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  await expect(page.getByRole("heading", { name: "Faces, Backs, and implementation" })).toBeVisible();
  await expect(page.locator("textarea")).toHaveValue(edited);
  await expect(page.locator(".gear-back-expansion")).toBeVisible();
  await expect(page.locator(".gear-back-flow")).toHaveAttribute("data-renderer", "react-flow");
  for (const [index, path] of admission.paths.entries()) {
    const pathname = new URL(admission.resourceUrls[index]).pathname;
    expect(requests.filter((request) => request === pathname), path).toHaveLength(2);
  }
});

test("Crèche launches its exact admitted graph through bounded Host context", async ({ page }) => {
  entrance.child.kill();
  entrance = await startStaticProduct("target/creche-product", "/conduit/creche/");
  const requests = [];
  page.on("request", (request) => {
    if (request.url().startsWith("http:")) requests.push(new URL(request.url()).pathname);
  });
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await expect(page.getByRole("heading", { name: "Birth a Body" })).toBeVisible();
  const admission = await page.evaluate(() => ({
    applicationId: globalThis.__conduitBrowserApplication.manifest.applicationId,
    packageDigest: globalThis.__conduitBrowserApplication.manifest.packageDigest,
    stateIdentity: globalThis.__conduitBrowserApplication.manifest.stateCompatibility.identity,
    storageIdentity: globalThis.__conduitBrowserApplication.storage.applicationIdentity,
    storagePackageDigest: globalThis.__conduitBrowserApplication.storage.packageDigest,
    paths: globalThis.__conduitBrowserApplication.manifest.resources.map((resource) => resource.path),
    resourceUrls: globalThis.__conduitBrowserApplication.manifest.resources.map((resource) => resource.url.href),
  }));
  expect(admission.applicationId).toBe("conduit.application/creche");
  expect(admission.packageDigest).toMatch(/^sha256:[0-9a-f]{64}$/);
  expect(admission.storageIdentity).toBe(admission.stateIdentity);
  expect(admission.storagePackageDigest).toBe(admission.packageDigest);
  expect(admission.paths).toHaveLength(39);
  expect(admission.paths).toContain("targets/esp32/browser-deployment/rom-loader.mjs");
  expect(admission.paths).toContain("targets/rp2040/browser-deployment/picoboot.mjs");
  for (const [index, path] of admission.paths.entries()) {
    const pathname = new URL(admission.resourceUrls[index]).pathname;
    expect(requests.filter((request) => request === pathname), path).toHaveLength(1);
  }
  await expect(page.locator('style[data-application-resource="creche-style"]')).toHaveCount(1);
});

test("Crèche restores one validated Body session across same-browser reloads", async ({ page }) => {
  entrance.child.kill();
  entrance = await startStaticProduct("target/creche-product", "/conduit/creche/");
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  const birth = page.locator(".body-birth-runner");
  await birth.getByRole("button", { name: "Birth Body" }).click();
  const bodyId = await birth.getAttribute("data-body-id");
  await page.getByRole("button", { name: "2. First Host" }).click();
  await page.getByRole("button", { name: "Give this Body its first Host" }).click();
  await page.evaluate(() => globalThis.__conduitCrecheDurability.settled());

  await page.reload();
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await expect(page.locator('[data-application-key="body-id"]')).toHaveText(bodyId);
  await expect(page.locator('.first-host-runner [data-application-key="host-status"]')).toContainText("one admitted browser Host");
  const restored = await page.evaluate(() => {
    const api = globalThis.__conduitCrecheHost.runtime;
    const code = api.conduit_creche_current();
    const bytes = new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len());
    return { code, receipt: JSON.parse(new TextDecoder().decode(bytes)) };
  });
  expect(restored.code).toBe(0);
  expect(restored.receipt.body_id).toBe(bodyId);
  expect(restored.receipt.membership_revision).toBe(2);

  await page.getByRole("button", { name: "4. Graduate" }).click();
  await page.getByRole("button", { name: "Finish without hosted Patchbay" }).click();
  await page.evaluate(() => globalThis.__conduitCrecheDurability.settled());
  await page.reload();
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await expect(page.locator('[data-application-key="graduation-status"]')).toContainText("Graduated");
  await expect(page.locator(".graduation-runner")).toHaveAttribute("data-body-id", bodyId);
});

test("Crèche refuses changed durable Body evidence before restoring authority", async ({ page }) => {
  entrance.child.kill();
  entrance = await startStaticProduct("target/creche-product", "/conduit/creche/");
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await page.getByRole("button", { name: "Birth Body" }).click();
  await page.evaluate(() => globalThis.__conduitCrecheDurability.settled());
  await page.evaluate(async () => {
    const storage = globalThis.__conduitBrowserApplication.storage;
    const snapshot = await storage.readJson("body-session");
    snapshot.receipt.body_id += "-changed";
    await storage.writeJson("body-session", snapshot);
  });

  await page.reload();
  await expect(page.locator("#host-state")).toHaveText("Crèche unavailable");
  await expect(page.locator("#workspace")).toHaveText("durable Crèche session identities disagree");
  expect(await page.evaluate(() => globalThis.__conduitCrecheHost)).toBeUndefined();
});

test("Crèche refuses changed admitted code before application manifestation", async ({ page }) => {
  entrance.child.kill();
  entrance = await startStaticProduct("target/creche-product", "/conduit/creche/");
  await page.route("**/creche.mjs", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: `${await response.text()}\n// changed after packaging` });
  }, { times: 1 });
  await page.goto(entrance.url);
  await expect(page.locator("body")).toHaveText("application resource application-module changed identity");
  expect(await page.evaluate(() => globalThis.__conduitCrecheHost)).toBeUndefined();
});

test("Book navigation is one finite Host-manifested view with stale and pressure refusal", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  const navigation = page.locator('[data-application-slot="book-navigation"]');
  await expect(navigation.locator('[data-application-component="navigation"]')).toHaveCount(1);
  await expect(navigation.locator('[data-application-key="progress"]')).toHaveText("Page 1 of 7");
  await expect(navigation.getByRole("button", { name: "Previous" })).toBeDisabled();
  await expect(navigation.getByRole("button", { name: "Next" })).toBeEnabled();

  await page.evaluate(() => {
    globalThis.__staleBookNavigationButton = document.querySelector('[data-application-key="next"]');
    globalThis.__staleBookNavigationButton.click();
  });
  await expect(page.getByRole("heading", { name: "Faces, Backs, and implementation" })).toBeVisible();
  await page.evaluate(() => globalThis.__staleBookNavigationButton.click());
  expect(await page.evaluate(() => globalThis.__conduitBrowserApplication.presentation.lastRefusal("book-navigation"))).toBe("stale-revision");
  await expect(page.getByRole("heading", { name: "Faces, Backs, and implementation" })).toBeVisible();

  const presentationEvidence = await page.evaluate(() => {
    const presentation = globalThis.__conduitBrowserApplication.presentation;
    const before = document.querySelector('[data-application-slot="book-navigation"]').innerHTML;
    let malformedRefusal;
    try {
      presentation.present("book-navigation", {
        revision: 999,
        actions: [],
        nodes: Array.from({ length: 33 }, (_, index) => ({
          parent: index === 0 ? null : 0,
          component: "paragraph",
          key: `oversized-${index}`,
          text: "refuse before mutation",
          action: null,
        })),
      });
    } catch (error) { malformedRefusal = error.code; }
    const unchangedAfterRefusal = document.querySelector('[data-application-slot="book-navigation"]').innerHTML === before;
    presentation.present("book-navigation", {
      revision: 1_000,
      actions: [{ id: "proof.activate", event: "activate" }],
      nodes: [
        { parent: null, component: "navigation", key: "proof-nav", text: "", action: null },
        { parent: 0, component: "button", key: "proof-button", text: "Pressure", action: 0 },
      ],
    }, { eventCapacity: 1 });
    const button = document.querySelector('[data-application-key="proof-button"]');
    button.click();
    button.click();
    const event = presentation.nextEvent("book-navigation");
    return {
      malformedRefusal,
      unchangedAfterRefusal,
      pressureRefusal: presentation.lastRefusal("book-navigation"),
      event: { action: event.action, revision: event.revision, encodedBytes: event.encoded.length },
    };
  });
  expect(presentationEvidence).toEqual({
    malformedRefusal: "too-many-nodes",
    unchangedAfterRefusal: true,
    pressureRefusal: "queue-pressure",
    event: { action: "proof.activate", revision: 1_000, encodedBytes: 25 },
  });
});

test("browser Host refuses malformed and escaping application packages before launch", async ({ page }) => {
  for (const [mutate, refusal] of [
    [(manifest) => { manifest.schema = "wrong"; }, "browser application package schema is unsupported"],
    [(manifest) => { manifest.resources[0].path = "https://example.com/book.mjs"; }, "application resource path escapes the application package"],
    [(manifest) => { manifest.resources.push(...Array.from({ length: 40 }, () => manifest.resources.at(-1))); }, "application package resource count is outside its admitted bound"],
  ]) {
    await mutatePackage(page, mutate);
    await page.goto(entrance.url);
    await expect(page.locator("#host-state")).toHaveText("Browser application refused");
    await expect(page.locator("#chapter")).toHaveText(refusal);
  }
});

test("browser Host refuses changed resource bytes and a changed aggregate package identity", async ({ page }) => {
  await page.route("**/book.mjs", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: `${await response.text()}\n// changed after packaging` });
  }, { times: 1 });
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser application refused");
  await expect(page.locator("#chapter")).toHaveText("application resource application-module changed identity");
  expect(await page.evaluate(() => globalThis.__conduitBookHost)).toBeUndefined();

  await mutatePackage(page, (manifest) => { manifest.package_digest = `sha256:${"0".repeat(64)}`; });
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser application refused");
  await expect(page.locator("#chapter")).toHaveText("application package identity changed");
});

test("browser Host storage refuses capacity exhaustion and malformed durable Book state", async ({ page }) => {
  await openBookStep(page, entrance, 0);
  const capacityRefusal = await page.evaluate(async () => {
    const storage = globalThis.__conduitBrowserApplication.storage;
    await storage.writeJson("reading-state", { schema: "conduit.book/unknown-state@9", drafts: [], expandedBacks: [] });
    for (let index = 0; index < 63; index += 1) await storage.writeJson(`proof-${index}`, index);
    try {
      await storage.writeJson("proof-overflow", true);
      return "accepted";
    } catch (error) {
      return error.message;
    }
  });
  expect(capacityRefusal).toBe("application storage capacity is exhausted");
  await page.reload();
  await expect(page.locator("#host-state")).toHaveText("Browser Host unavailable");
  await expect(page.locator("#chapter")).toHaveText("persisted Book state is malformed");
});
