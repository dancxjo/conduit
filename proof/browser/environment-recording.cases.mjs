import { expect, test } from "@playwright/test";
import { readFile, writeFile } from "node:fs/promises";

// Rust prepares the actual full recording chunk and snapshot envelope, then
// independently validates this returned record. Only custody bindings are fixtures.
test("full environmental chunk survives immutable IndexedDB publication and page reload", async ({ page }) => {
  const artifact = new URL("../../target/environment-recording-browser/", import.meta.url);
  const fixture = JSON.parse(await readFile(new URL("publication.json", artifact), "utf8"));
  await page.goto("/proof/browser/signal-dom-host.test.html");
  const published = await page.evaluate(async (fixture) => {
    const { openBrowserApplicationStorage } = await import("/targets/browser/host/assets/browser-application-storage.mjs");
    const { executeResourceStorageEffect } = await import("/targets/browser/host/assets/browser-resource-storage.mjs");
    const storage = await openBrowserApplicationStorage("proof/environment-recording", 1, `sha256:${"e".repeat(64)}`,
      { implementationRegistry: ["browser/indexeddb@1"] });
    await storage.clearApplication();
    const scope = { host_id: "snapshot-host", boot_id: "fixture/store/boot-one", active_play_id: "fixture/write", placement_id: "snapshot-placement", request_sequence: 0 };
    const effect = { ...scope, schema: "conduit.browser/resource-effect@1", effect_kind: "resource-publish", ...fixture };
    const result = await executeResourceStorageEffect(storage, effect, scope);
    let duplicate;
    try { await executeResourceStorageEffect(storage, effect, scope); } catch (error) { duplicate = error.code; }
    storage.close();
    return { result, duplicate };
  }, fixture);
  expect(published).toEqual({ result: { status: "completed", record: null }, duplicate: "PublishedImmutable" });
  await page.reload();
  const restored = await page.evaluate(async ({ key }) => {
    const { openBrowserApplicationStorage } = await import("/targets/browser/host/assets/browser-application-storage.mjs");
    const { executeResourceStorageEffect } = await import("/targets/browser/host/assets/browser-resource-storage.mjs");
    const storage = await openBrowserApplicationStorage("proof/environment-recording", 1, `sha256:${"e".repeat(64)}`,
      { implementationRegistry: ["browser/indexeddb@1"] });
    const scope = { host_id: "snapshot-host", boot_id: "fixture/store/boot-two", active_play_id: "fixture/read", placement_id: "snapshot-placement", request_sequence: 0 };
    const effect = { ...scope, schema: "conduit.browser/resource-effect@1", effect_kind: "resource-read", key, record: null };
    const result = await executeResourceStorageEffect(storage, effect, scope);
    let stale;
    try { await executeResourceStorageEffect(storage, { ...effect, boot_id: "fixture/store/boot-one" }, scope); } catch (error) { stale = error.code; }
    storage.close();
    return { record: Array.from(result.record), stale };
  }, fixture);
  expect(restored.stale).toBe("StaleResourceEffect");
  expect(restored.record).toEqual(fixture.record);
  await writeFile(new URL("restored.json", artifact), JSON.stringify(restored.record));
});
