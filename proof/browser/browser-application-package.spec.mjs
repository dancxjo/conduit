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
  await page.goto(`${entrance.url}same-face-different-implementation/`);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  await expect(page.getByRole("heading", { name: "Same Face, different implementation" })).toBeVisible();
  const admission = await page.evaluate(() => ({
    packageDigest: globalThis.__conduitBrowserApplication.manifest.packageDigest,
    stateIdentity: globalThis.__conduitBrowserApplication.manifest.stateCompatibility.identity,
    storageIdentity: globalThis.__conduitBrowserApplication.storage.applicationIdentity,
    storagePackageDigest: globalThis.__conduitBrowserApplication.storage.packageDigest,
    paths: globalThis.__conduitBrowserApplication.manifest.resources.map((resource) => resource.path),
    baseUri: document.baseURI,
  }));
  expect(admission.packageDigest).toMatch(/^sha256:[0-9a-f]{64}$/);
  expect(admission.storagePackageDigest).toBe(admission.packageDigest);
  expect(admission.storageIdentity).toBe(admission.stateIdentity);
  expect(admission.paths).toContain("book-runner-presentation.mjs");
  for (const path of admission.paths) {
    const pathname = new URL(path, admission.baseUri).pathname;
    expect(requests.filter((request) => request === pathname), path).toHaveLength(1);
  }
  await expect(page.locator('script[data-application-resource="react"]')).toHaveAttribute("src", /^blob:/);
  await expect(page.locator('style[data-application-resource="book-style"]')).toHaveCount(1);
  const runnerStatus = page.locator('[data-application-key="play-status"]');
  await expect(runnerStatus).toHaveAttribute("data-application-component", "status");
  await expect(runnerStatus).toHaveText("Edit the message or timing, then run it.");
  await expect(page.locator(".play-status")).toHaveCount(0);
  const listing = page.locator("textarea");
  const edited = (await listing.inputValue()).replace('"hello"', '"durable"');
  await listing.fill(edited);
  const patchbay = page.locator(".compact-patchbay");
  await patchbay.getByRole("button", { name: "Open reviewed Back for same-morse-caller/morse" }).click();
  await expect(patchbay.locator(".gear-back-expansion")).toBeVisible();
  await page.evaluate(() => globalThis.__conduitBookPersistence.flush());

  await page.reload();
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  await expect(page.getByRole("heading", { name: "Same Face, different implementation" })).toBeVisible();
  await expect(page.locator("textarea")).toHaveValue(edited);
  await expect(page.locator(".gear-back-expansion")).toBeVisible();
  await expect(page.locator(".gear-back-flow")).toHaveAttribute("data-renderer", "react-flow");
  for (const path of admission.paths) {
    const pathname = new URL(path, admission.baseUri).pathname;
    expect(requests.filter((request) => request === pathname), path).toHaveLength(2);
  }
});

test("Book navigation is one finite Host-manifested view with stale and pressure refusal", async ({ page }) => {
  await page.goto(entrance.url);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  const navigation = page.locator('[data-application-slot="book-navigation"]');
  await expect(navigation.locator('[data-application-component="navigation"]')).toHaveCount(1);
  await expect(navigation.locator('[data-application-key="progress"]')).toHaveText("Page 1 of 14");
  await expect(navigation.getByRole("button", { name: "Previous" })).toBeDisabled();
  await expect(navigation.getByRole("button", { name: "Next" })).toBeEnabled();

  await page.evaluate(() => {
    globalThis.__staleBookNavigationButton = document.querySelector('[data-application-key="next"]');
    globalThis.__staleBookNavigationButton.click();
  });
  await expect(page.getByRole("heading", { name: "Connect Gears" })).toBeVisible();
  await page.evaluate(() => globalThis.__staleBookNavigationButton.click());
  expect(await page.evaluate(() => globalThis.__conduitBrowserApplication.presentation.lastRefusal("book-navigation"))).toBe("stale-revision");
  await expect(page.getByRole("heading", { name: "Connect Gears" })).toBeVisible();

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
    event: { action: "proof.activate", revision: 1_000, encodedBytes: 23 },
  });
});

test("browser Host refuses malformed and escaping application packages before launch", async ({ page }) => {
  for (const [mutate, refusal] of [
    [(manifest) => { manifest.schema = "wrong"; }, "browser application package schema is unsupported"],
    [(manifest) => { manifest.resources[0].path = "https://example.com/book.mjs"; }, "application resource path escapes the application package"],
    [(manifest) => { manifest.resources.push(...Array.from({ length: 12 }, () => manifest.resources.at(-1))); }, "application package resource count is outside its admitted bound"],
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
