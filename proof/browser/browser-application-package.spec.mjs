import { expect, test } from "@playwright/test";
import { openBookStep, startBook, startStaticProduct } from "./book-test-server.mjs";

let entrance;

test.beforeEach(async () => { entrance = await startBook(); });
test.afterEach(() => entrance?.child.kill());

test("Book drafts and an open reviewed Back endure a same-browser reload", async ({ page }) => {
  entrance.child.kill();
  entrance = await startStaticProduct("target/book-product", "/conduit/book/");
  await page.goto(`${entrance.url}same-face-different-implementation/`);
  await expect(page.locator("#host-state")).toHaveText("Browser Host ready");
  await expect(page.getByRole("heading", { name: "Same Face, different implementation" })).toBeVisible();
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
});

test("browser Host refuses malformed and escaping application packages before launch", async ({ page }) => {
  for (const manifest of [
    { schema: "wrong", identity: "conduit.application/book", version: 1, resources: [] },
    {
      schema: "conduit.browser/application-package@1", identity: "conduit.application/book", version: 1,
      resources: [
        { role: "application-module", path: "https://example.com/book.mjs", maximum_bytes: 1024 },
        { role: "runtime", path: "runtime.wasm", maximum_bytes: 1024 },
      ],
    },
    {
      schema: "conduit.browser/application-package@1", identity: "conduit.application/book", version: 1,
      resources: Array.from({ length: 33 }, (_, index) => ({ role: `resource-${index}`, path: `resource-${index}`, maximum_bytes: 1 })),
    },
  ]) {
    await page.route("**/book.application.json", (route) => route.fulfill({
      status: 200, contentType: "application/json", body: JSON.stringify(manifest),
    }), { times: 1 });
    await page.goto(entrance.url);
    await expect(page.locator("#host-state")).toHaveText("Browser application refused");
    await expect(page.locator("#chapter")).not.toBeEmpty();
  }
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
