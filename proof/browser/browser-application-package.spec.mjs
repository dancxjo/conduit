import { expect, test } from "@playwright/test";
import { reviewAndBirth } from "./creche-test-actions.mjs";
import { openTourStep, startTour, startStaticProduct } from "./tour-test-server.mjs";

let entrance;

test.beforeEach(async () => { entrance = await startTour(); });
test.afterEach(() => entrance?.child.kill());

async function mutatePackage(page, mutate) {
  await page.route("**/tour.application.json", async (route) => {
    const response = await route.fetch();
    const manifest = await response.json();
    mutate(manifest);
    await route.fulfill({ response, contentType: "application/json", body: JSON.stringify(manifest) });
  }, { times: 1 });
}

async function deleteStoredApplicationRecord(page, applicationIdentity, version, key) {
  await page.evaluate(async ({ applicationIdentity, version, key }) => {
    const request = indexedDB.open("conduit-browser-host-applications", 2);
    const database = await new Promise((resolve, reject) => {
      request.addEventListener("success", () => resolve(request.result), { once: true });
      request.addEventListener("error", () => reject(request.error), { once: true });
    });
    const transaction = database.transaction("application-state", "readwrite");
    transaction.objectStore("application-state").delete(`${applicationIdentity}@${version}\u0000${key}`);
    await new Promise((resolve, reject) => {
      transaction.addEventListener("complete", resolve, { once: true });
      transaction.addEventListener("error", () => reject(transaction.error), { once: true });
      transaction.addEventListener("abort", () => reject(transaction.error), { once: true });
    });
    database.close();
  }, { applicationIdentity, version, key });
}

test("Tour drafts and an open reviewed Back endure a same-browser reload", async ({ page }) => {
  entrance.child.kill();
  entrance = await startStaticProduct("target/tour-product", "/conduit/tour/");
  const requests = [];
  page.on("request", (request) => {
    if (request.url().startsWith("http:")) requests.push(new URL(request.url()).pathname);
  });
  await page.goto(`${entrance.url}faces-backs-and-implementation/`);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  await expect(page.getByRole("heading", { name: "Faces, Backs, and implementation" })).toBeVisible();
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
  expect(separatedStyles.tour).not.toContain(".flow-faceplate header");
  expect(separatedStyles.tour).not.toContain(".react-flow__edge-path");
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
  await page.evaluate(() => globalThis.__conduitTourPersistence.flush());

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

test("Tour migrates the finite legacy Book reading state without changing its compatibility identity", async ({ page }) => {
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
  expect(admission.paths.length).toBeGreaterThan(0);
  expect(admission.paths.length).toBeLessThanOrEqual(64);
  expect(admission.paths).toContain("browser-host-identity.mjs");
  expect(admission.paths).toContain("creche-names.mjs");
  expect(admission.paths).toContain("creche-form-selection.mjs");
  expect(admission.paths).toContain("application-syntax-presentation.mjs");
  expect(admission.paths).toContain("creche-browser-configuration.mjs");
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
  await reviewAndBirth(page, birth);
  const bodyId = await birth.getAttribute("data-body-id");
  await page.getByRole("button", { name: "2. First Host" }).click();
  await page.getByRole("button", { name: "Give this Body its first Host" }).click();
  await page.evaluate(() => globalThis.__conduitCrecheDurability.settled());
  const firstIncarnation = await page.evaluate(() => ({
    hostId: globalThis.__conduitCrecheHost.hostId,
    bootId: globalThis.__conduitCrecheHost.bootId,
  }));

  await page.reload();
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await expect(page.locator('[data-application-key="body-id"]')).toHaveText(bodyId);
  await expect(page.locator('.first-host-runner [data-application-key="host-status"]')).toContainText("one admitted browser Host");
  const restored = await page.evaluate(() => {
    const api = globalThis.__conduitCrecheHost.runtime;
    const code = api.conduit_creche_current();
    const bytes = new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len());
    return {
      code,
      hostId: globalThis.__conduitCrecheHost.hostId,
      bootId: globalThis.__conduitCrecheHost.bootId,
      receipt: JSON.parse(new TextDecoder().decode(bytes)),
    };
  });
  expect(restored.code).toBe(0);
  expect(restored.hostId).toBe(firstIncarnation.hostId);
  expect(restored.bootId).not.toBe(firstIncarnation.bootId);
  expect(restored.receipt.body_id).toBe(bodyId);
  expect(restored.receipt.host_id).toBe(restored.hostId);
  expect(restored.receipt.boot_id).toBe(restored.bootId);
  expect(restored.receipt.membership_revision).toBe(4);
  expect(restored.receipt.raw_membership.events.at(-2).kind.HostDetached.prior_boot_id).toBe(firstIncarnation.bootId);
  expect(restored.receipt.raw_membership.events.at(-1).kind.HostAttached.observation.boot_id).toBe(restored.bootId);

  await page.getByRole("button", { name: "4. Graduate" }).click();
  await page.getByRole("button", { name: "Finish without hosted Patchbay" }).click();
  await page.evaluate(() => globalThis.__conduitCrecheDurability.settled());
  await page.reload();
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await expect(page.locator('[data-application-key="graduation-status"]')).toContainText("Graduated");
  await expect(page.locator(".graduation-runner")).toHaveAttribute("data-body-id", bodyId);
});

test("Crèche leave, rejoin, revoke, and local finish preserve Body and Host distinctions", async ({ page }) => {
  entrance.child.kill();
  entrance = await startStaticProduct("target/creche-product", "/conduit/creche/");
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await reviewAndBirth(page);
  const bodyId = await page.locator(".body-birth-runner").getAttribute("data-body-id");
  await page.getByRole("button", { name: "2. First Host" }).click();
  await page.getByRole("button", { name: "Give this Body its first Host" }).click();
  await page.getByRole("button", { name: "4. Graduate" }).click();
  await page.getByRole("button", { name: "Host Patchbay on this Body" }).click();
  await page.getByRole("button", { name: "End the Crèche" }).click();
  await page.evaluate(() => globalThis.__conduitCrecheDurability.settled());
  const durableHostId = await page.evaluate(() => globalThis.__conduitCrecheHost.hostId);

  await page.getByRole("button", { name: "Leave Body" }).click();
  await page.evaluate(() => globalThis.__conduitCrecheDurability.settled());
  const left = await page.evaluate(async () => globalThis.__conduitBrowserApplication.storage.readJson("body-session"));
  expect(left.receipt.body_id).toBe(bodyId);
  expect(left.receipt.raw_membership.parts[0].state).toBe("Admitted");
  expect(left.receipt.raw_membership.parts[0].current).toBeNull();

  await page.reload();
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  expect(await page.evaluate(() => globalThis.__conduitCrecheHost.hostId)).toBe(durableHostId);
  const returned = await page.evaluate(async () => globalThis.__conduitBrowserApplication.storage.readJson("body-session"));
  expect(returned.receipt.body_id).toBe(bodyId);
  expect(returned.receipt.raw_membership.parts[0].current.host_id).toBe(durableHostId);

  await page.getByRole("button", { name: "End the Crèche" }).click();
  await page.getByRole("button", { name: "Remove this browser from the Body" }).click();
  await page.evaluate(() => globalThis.__conduitCrecheDurability.settled());
  const revoked = await page.evaluate(async () => globalThis.__conduitBrowserApplication.storage.readJson("body-session"));
  expect(revoked.receipt.body_id).toBe(bodyId);
  expect(revoked.receipt.raw_membership.parts[0].state).toBe("Revoked");
  expect(revoked.receipt.raw_membership.parts[0].current).toBeNull();

  await page.getByRole("button", { name: "Finish and clear Crèche" }).click();
  await expect(page.getByRole("button", { name: "Birth Body" })).toBeVisible();
  expect(await page.evaluate(async () => globalThis.__conduitBrowserApplication.storage.readJson("body-session"))).toBeNull();
  expect(await page.evaluate(() => globalThis.__conduitCrecheHost.hostId)).toBe(durableHostId);
});

test("browser Host reset is explicit and app state corruption never rotates identity silently", async ({ page }) => {
  entrance.child.kill();
  entrance = await startStaticProduct("target/creche-product", "/conduit/creche/");
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  const firstHost = await page.evaluate(async () => {
    await globalThis.__conduitBrowserApplication.storage.writeJson("reset-proof", { retained: true });
    const id = globalThis.__conduitCrecheHost.hostId;
    await globalThis.__conduitCrecheHost.resetHostIdentity("conduit.browser/reset-host-identity@1");
    return id;
  });
  await page.reload();
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  const afterReset = await page.evaluate(async () => ({
    hostId: globalThis.__conduitCrecheHost.hostId,
    applicationState: await globalThis.__conduitBrowserApplication.storage.readJson("reset-proof"),
  }));
  expect(afterReset.hostId).not.toBe(firstHost);
  expect(afterReset.applicationState).toEqual({ retained: true });

  await page.evaluate(async () => {
    const request = indexedDB.open("conduit-browser-host-applications", 2);
    const database = await new Promise((resolve, reject) => {
      request.addEventListener("success", () => resolve(request.result), { once: true });
      request.addEventListener("error", () => reject(request.error), { once: true });
    });
    const transaction = database.transaction("browser-host-identity", "readwrite");
    transaction.objectStore("browser-host-identity").put({
      schema: "conduit.browser/host-identity@1",
      identity: "durable-browser-host",
      hostId: "corrupt",
      seed: [1],
    });
    await new Promise((resolve, reject) => {
      transaction.addEventListener("complete", resolve, { once: true });
      transaction.addEventListener("abort", () => reject(transaction.error), { once: true });
      transaction.addEventListener("error", () => reject(transaction.error), { once: true });
    });
  });
  await page.reload();
  await expect(page.locator("body")).toContainText("durable browser Host identity is malformed; explicit Host reset is required");
  expect(await page.evaluate(() => globalThis.__conduitCrecheHost)).toBeUndefined();
});

test("Crèche refuses changed durable Body evidence before restoring authority", async ({ page }) => {
  entrance.child.kill();
  entrance = await startStaticProduct("target/creche-product", "/conduit/creche/");
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
  await reviewAndBirth(page);
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
  await deleteStoredApplicationRecord(page, "conduit.application/creche-host-state", 1, "body-session");
  await page.reload();
  await expect(page.locator("#host-state")).toHaveText("Crèche ready");
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
  await expect(page.getByRole("heading", { name: "Faces, Backs, and implementation" })).toBeVisible();
  await page.evaluate(() => globalThis.__staleTourNavigationButton.click());
  expect(await page.evaluate(() => globalThis.__conduitBrowserApplication.presentation.lastRefusal("tour-navigation"))).toBe("stale-revision");
  await expect(page.getByRole("heading", { name: "Faces, Backs, and implementation" })).toBeVisible();

  const presentationEvidence = await page.evaluate(() => {
    const presentation = globalThis.__conduitBrowserApplication.presentation;
    const before = document.querySelector('[data-application-slot="tour-navigation"]').innerHTML;
    let malformedRefusal;
    try {
      presentation.present("tour-navigation", {
        revision: 999,
        actions: [],
        nodes: Array.from({ length: 41 }, (_, index) => ({
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
  await mutatePackage(page, (manifest) => { manifest.package_digest = `sha256:${"0".repeat(64)}`; });
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser application refused");
  await expect(page.locator("#chapter")).toHaveText("application package identity changed");
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
