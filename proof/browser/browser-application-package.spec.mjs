import { expect, test } from "@playwright/test";
import { openTourStep, startTour, startStaticProduct } from "./tour-test-server.mjs";

let entrance;

test.beforeEach(async ({}, testInfo) => {
  const creche = testInfo.title.startsWith("Crèche compatibility ");
  entrance = creche ? await startStaticProduct("target/creche-product", "/conduit/creche/") : await startTour();
});
test.afterEach(() => entrance?.child.kill());

async function mutatePackage(page, mutate) {
  await page.route("**/tour.application.json", async (route) => {
    const response = await route.fetch();
    const manifest = await response.json();
    mutate(manifest);
    await route.fulfill({ response, contentType: "application/json", body: JSON.stringify(manifest) });
  }, { times: 1 });
}

test("Tour drafts and an open reviewed Back endure a same-browser reload", async ({ page }) => {
  entrance.child.kill();
  entrance = await startStaticProduct("target/tour-product", "/conduit/tour/");
  const requests = [];
  page.on("request", (request) => {
    if (request.url().startsWith("http:")) requests.push(new URL(request.url()).pathname);
  });
  await page.goto(`${entrance.url}fronts-backs-and-implementation/`);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  await expect(page.getByRole("heading", { name: "Fronts, Backs, and implementation" })).toBeVisible();
  const admission = await page.evaluate(() => ({
    applicationId: globalThis.__conduitBrowserApplication.manifest.applicationId,
    packageDigest: globalThis.__conduitBrowserApplication.manifest.packageDigest,
    stateIdentity: globalThis.__conduitBrowserApplication.manifest.stateCompatibility.identity,
    storageIdentity: globalThis.__conduitBrowserApplication.storage.applicationIdentity,
    storagePackageDigest: globalThis.__conduitBrowserApplication.storage.packageDigest,
    paths: globalThis.__conduitBrowserApplication.manifest.resources.map((resource) => resource.path),
    resourceUrls: globalThis.__conduitBrowserApplication.manifest.resources.map((resource) => resource.url.href),
  }));
  expect(admission.packageDigest).toMatch(/^sha256:[0-9a-f]{64}$/);
  expect(admission.applicationId).toBe("conduit.application/tour");
  expect(admission.stateIdentity).toBe("conduit.application/book-reading-state");
  expect(admission.storagePackageDigest).toBe(admission.packageDigest);
  expect(admission.storageIdentity).toBe(admission.stateIdentity);
  const membershipClient = await page.evaluate(() => {
    const host = globalThis.__conduitTourHost;
    return {
      schema: host.membership.schema,
      hostId: host.hostId,
      bootId: host.bootId,
      advertisement: host.membership.advertisement(),
    };
  });
  expect(membershipClient.schema).toBe("conduit.browser/body-membership-client@1");
  expect(membershipClient.advertisement.host_id).toBe(membershipClient.hostId);
  expect(membershipClient.advertisement.boot_id).toBe(membershipClient.bootId);
  expect(admission.paths).toContain("tour-runner-presentation.mjs");
  expect(admission.paths).toContain("application-syntax-presentation.mjs");
  expect(admission.paths).toContain("assets/flow.css");
  for (const [index, path] of admission.paths.entries()) {
    const pathname = new URL(admission.resourceUrls[index]).pathname;
    expect(requests.filter((request) => request === pathname), path).toHaveLength(1);
  }
  await expect(page.locator('script[data-application-resource="react"]')).toHaveAttribute("src", /^blob:/);
  await expect(page.locator('style[data-application-resource="tour-style"]')).toHaveCount(1);
  await expect(page.locator('style[data-application-resource="patchbay-flow-style"]')).toHaveCount(1);
  const separatedStyles = await page.evaluate(() => {
    const application = globalThis.__conduitBrowserApplication;
    const decode = (role) => new TextDecoder().decode(application.bytes(role));
    return { tour: decode("tour-style"), flow: decode("patchbay-flow-style") };
  });
  expect(separatedStyles.tour).not.toContain(".flow-frontplate header");
  expect(separatedStyles.tour).not.toContain(".react-flow__edge-path");
  expect(separatedStyles.flow).toContain(".flow-frontplate header");
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
  await page.evaluate(() => globalThis.__conduitTourPersistence.flush());

  await page.reload();
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  await expect(page.getByRole("heading", { name: "Fronts, Backs, and implementation" })).toBeVisible();
  await expect(page.locator("textarea")).toHaveValue(edited);
  await expect(page.locator(".gear-back-expansion")).toBeVisible();
  await expect(page.locator(".gear-back-flow")).toHaveAttribute("data-renderer", "react-flow");
  for (const [index, path] of admission.paths.entries()) {
    const pathname = new URL(admission.resourceUrls[index]).pathname;
    expect(requests.filter((request) => request === pathname), path).toHaveLength(2);
  }
});

test.skip("Tour migrates the finite legacy Book reading state without changing its compatibility identity", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  const legacy = {
    schema: "conduit.book/reading-state@1",
    drafts: [["runner-0", "legacy draft"]],
    expandedBacks: ["same-morse-caller/morse"],
  };
  await page.evaluate((state) => globalThis.__conduitBrowserApplication.storage.writeJson("reading-state", state), legacy);

  await page.reload();
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  await page.evaluate(() => globalThis.__conduitTourPersistence.flush());
  const migrated = await page.evaluate(async () => ({
    applicationIdentity: globalThis.__conduitBrowserApplication.storage.applicationIdentity,
    state: await globalThis.__conduitBrowserApplication.storage.readJson("reading-state"),
  }));
  expect(migrated.applicationIdentity).toBe("conduit.application/book-reading-state");
  expect(migrated.state).toEqual({ ...legacy, schema: "conduit.tour/reading-state@1" });
});

test("Crèche compatibility entrance redirects to the Workspace route", async ({ page }) => {
  await page.goto(entrance.url);
  await expect.poll(() => new URL(page.url()).pathname).toBe("/conduit/workspace/");
});

test("Tour navigation is one finite Host-manifested view with stale and pressure refusal", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  const navigation = page.locator('[data-application-slot="tour-navigation"]');
  await expect(navigation.locator('[data-application-component="navigation"]')).toHaveCount(1);
  await expect(navigation.locator('[data-application-key="progress"]')).toHaveText("Page 1 of 7");
  await expect(navigation.getByRole("button", { name: "Previous" })).toBeDisabled();
  await expect(navigation.getByRole("button", { name: "Next" })).toBeEnabled();

  await page.evaluate(() => {
    globalThis.__staleTourNavigationButton = document.querySelector('[data-application-key="next"]');
    globalThis.__staleTourNavigationButton.click();
  });
  await expect(page.getByRole("heading", { name: "Fronts, Backs, and implementation" })).toBeVisible();
  await page.evaluate(() => globalThis.__staleTourNavigationButton.click());
  expect(await page.evaluate(() => globalThis.__conduitBrowserApplication.presentation.lastRefusal("tour-navigation"))).toBe("stale-revision");
  await expect(page.getByRole("heading", { name: "Fronts, Backs, and implementation" })).toBeVisible();

  const presentationEvidence = await page.evaluate(() => {
    const presentation = globalThis.__conduitBrowserApplication.presentation;
    const before = document.querySelector('[data-application-slot="tour-navigation"]').innerHTML;
    let malformedRefusal;
    try {
      presentation.present("tour-navigation", {
        revision: 999,
        actions: [],
        nodes: Array.from({ length: 129 }, (_, index) => ({
          parent: index === 0 ? null : 0,
          component: "paragraph",
          key: `oversized-${index}`,
          text: "refuse before mutation",
          action: null,
        })),
      });
    } catch (error) { malformedRefusal = error.code; }
    const unchangedAfterRefusal = document.querySelector('[data-application-slot="tour-navigation"]').innerHTML === before;
    presentation.present("tour-navigation", {
      revision: 1_000,
      actions: [{ id: "proof.activate", event: "activate" }],
      nodes: [
        { parent: null, component: "navigation", key: "proof-nav", text: "Proof navigation", action: null },
        { parent: 0, component: "button", key: "proof-button", text: "Pressure", action: 0 },
      ],
    }, { eventCapacity: 1 });
    const button = document.querySelector('[data-application-key="proof-button"]');
    button.click();
    button.click();
    const event = presentation.nextEvent("tour-navigation");
    return {
      malformedRefusal,
      unchangedAfterRefusal,
      pressureRefusal: presentation.lastRefusal("tour-navigation"),
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
    [(manifest) => { manifest.resources[0].path = "https://example.com/tour.mjs"; }, "application resource path escapes the application package"],
    [(manifest) => { manifest.resources.push(...Array.from({ length: 40 }, () => manifest.resources.at(-1))); }, "application package resource count is outside its admitted bound"],
  ]) {
    await mutatePackage(page, mutate);
    await page.goto(entrance.url);
    await expect(page.locator("#host-state")).toHaveText("Browser application refused");
    await expect(page.locator("#chapter")).toHaveText(refusal);
  }
});

test("browser Host refuses changed resource bytes before launch", async ({ page }) => {
  await page.route("**/tour.mjs", async (route) => {
    const response = await route.fetch();
    await route.fulfill({ response, body: `${await response.text()}\n// changed after packaging` });
  }, { times: 1 });
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser application refused");
  await expect(page.locator("#chapter")).toHaveText("application resource application-module changed identity");
  expect(await page.evaluate(() => globalThis.__conduitTourHost)).toBeUndefined();
});

test("browser Host refuses a changed aggregate package identity before launch", async ({ page }) => {
  const resourceUrls = new Set();
  const requestedResources = [];
  await page.route("**/*", async (route) => {
    if (resourceUrls.has(route.request().url())) {
      requestedResources.push(route.request().url());
      await route.abort();
    } else {
      await route.fallback();
    }
  });
  await mutatePackage(page, (manifest) => {
    for (const resource of manifest.resources) resourceUrls.add(new URL(resource.path, entrance.url).href);
    manifest.package_digest = `sha256:${"0".repeat(64)}`;
  });
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser application refused");
  await expect(page.locator("#chapter")).toHaveText("application package identity changed");
  expect(requestedResources).toEqual([]);
  expect(await page.evaluate(() => globalThis.__conduitTourHost)).toBeUndefined();
});

test("selected durable storage keeps scopes, lifecycle, and refusal states exact", async ({ page }) => {
  await openTourStep(page, entrance, 0);
  const result = await page.evaluate(async () => {
    const module = await import(new URL("../browser-application-storage.mjs", location.href).href);
    const digest = `sha256:${"a".repeat(64)}`;
    const codes = {};
    try { await module.openBrowserApplicationStorage("proof/omitted", 1, digest); }
    catch (error) { codes.omitted = error.code; }
    try {
      await module.openBrowserApplicationStorage("proof/unavailable", 1, digest, {
        implementationRegistry: ["browser/indexeddb@1"], indexedDb: null,
      });
    } catch (error) { codes.unavailable = error.code; }
    try {
      await module.openBrowserApplicationStorage("proof/quota", 1, digest, {
        implementationRegistry: ["browser/indexeddb@1"],
        indexedDb: { open() { throw new DOMException("full", "QuotaExceededError"); } },
      });
    } catch (error) { codes.quota = error.code; }

    const selected = { implementationRegistry: ["browser/indexeddb@1"] };
    const first = await module.openBrowserApplicationStorage("proof/application", 1, digest, selected);
    await first.writeJson("retained", { value: 7 });
    const successful = await first.readJson("retained");
    const durability = await first.durability();
    const second = await module.openBrowserApplicationStorage("proof/application", 2, digest, selected);
    try { await second.readJson("retained"); } catch (error) { codes.version = error.code; }
    second.close();

    const request = indexedDB.open("conduit-browser-host-applications", 2);
    const database = await new Promise((resolve, reject) => {
      request.addEventListener("success", () => resolve(request.result), { once: true });
      request.addEventListener("error", () => reject(request.error), { once: true });
    });
    let transaction = database.transaction(["browser-host-identity", "application-state"], "readwrite");
    transaction.objectStore("browser-host-identity").put({ identity: "scope-proof", retained: true });
    transaction.objectStore("application-state").put({
      identity: "proof/application@1\u0000retained", applicationIdentity: "proof/application",
      applicationVersion: 1, packageDigest: digest, key: "retained", value: "{", valueBytes: 1,
    });
    await new Promise((resolve) => transaction.addEventListener("complete", resolve, { once: true }));
    database.close();
    try { await first.readJson("retained"); } catch (error) { codes.corrupt = error.code; }
    await first.clearApplication();

    const identityRequest = indexedDB.open("conduit-browser-host-applications", 2);
    const identityDatabase = await new Promise((resolve) => identityRequest.addEventListener("success", () => resolve(identityRequest.result), { once: true }));
    transaction = identityDatabase.transaction("browser-host-identity", "readonly");
    const identity = await new Promise((resolve) => {
      const get = transaction.objectStore("browser-host-identity").get("scope-proof");
      get.addEventListener("success", () => resolve(get.result), { once: true });
    });
    identityDatabase.close();
    first.close();
    try { await first.readJson("retained"); } catch (error) { codes.stale = error.code; }
    return { codes, successful, durability, identity, metadata: {
      state: globalThis.__conduitBrowserApplication.storage.state,
      implementationId: globalThis.__conduitBrowserApplication.storage.implementationId,
      artifactId: globalThis.__conduitBrowserApplication.storage.artifactId,
      bounds: globalThis.__conduitBrowserApplication.storage.bounds,
    } };
  });
  expect(result.codes).toEqual({
    omitted: "ImplementationNotSelected", unavailable: "StorageUnavailable",
    quota: "QuotaExhausted", version: "VersionMismatch", corrupt: "CorruptRecord",
    stale: "StaleApplicationGeneration",
  });
  expect(result.successful).toEqual({ value: 7 });
  expect(["PersistenceGranted", "EvictionPossible", "EvictionStatusUnavailable"]).toContain(result.durability.state);
  expect(result.identity).toEqual({ identity: "scope-proof", retained: true });
  expect(result.metadata).toMatchObject({
    state: "Initialized", implementationId: "browser/indexeddb@1",
    artifactId: "browser-application-storage.mjs@1",
    bounds: { maximumRecords: 64, maximumApplicationBytes: 1024 * 1024, maximumApplications: 16 },
  });
});

test("browser Host storage refuses capacity exhaustion and malformed durable Tour state", async ({ page }) => {
  await openTourStep(page, entrance, 0);
  const capacityRefusal = await page.evaluate(async () => {
    const storage = globalThis.__conduitBrowserApplication.storage;
    await storage.writeJson("reading-state", { schema: "conduit.tour/unknown-state@9", drafts: [], expandedBacks: [] });
    for (let index = 0; index <= storage.bounds.maximumRecords; index += 1) {
      try { await storage.writeJson(`proof-${index}`, index); }
      catch (error) { return { code: error.code, message: error.message }; }
    }
    return "accepted";
  });
  expect(capacityRefusal).toEqual({ code: "ApplicationCapacityExhausted", message: "application storage capacity is exhausted" });
  await page.reload();
  await expect(page.locator("#host-state")).toHaveText("Browser Host unavailable");
  await expect(page.locator("#chapter")).toHaveText("persisted Tour state is malformed");
});
