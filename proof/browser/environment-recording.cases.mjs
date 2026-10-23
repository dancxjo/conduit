import { expect, test } from "@playwright/test";
import { readFile, writeFile } from "node:fs/promises";

// Rust prepares the actual full recording chunk and snapshot envelope, then
// independently validates this returned record. Only custody bindings are fixtures.
test("overnight chunks and catalog survive immutable IndexedDB publication and page reload", async ({ page }) => {
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
    const outcomes = [];
    for (const [index, publication] of fixture.publications.entries()) {
      const pending = { ...scope, request_sequence: index };
      const effect = { ...pending, schema: "conduit.browser/resource-effect@1", effect_kind: "resource-publish", ...publication };
      const result = await executeResourceStorageEffect(storage, effect, pending);
      let duplicate;
      try { await executeResourceStorageEffect(storage, effect, pending); } catch (error) { duplicate = error.code; }
      outcomes.push({ result, duplicate });
    }
    storage.close();
    return outcomes;
  }, fixture);
  expect(published).toEqual(Array(16).fill({ result: { status: "completed", record: null }, duplicate: "PublishedImmutable" }));
  await page.reload();
  const restored = await page.evaluate(async ({ publications }) => {
    const { openBrowserApplicationStorage } = await import("/targets/browser/host/assets/browser-application-storage.mjs");
    const { executeResourceStorageEffect } = await import("/targets/browser/host/assets/browser-resource-storage.mjs");
    const storage = await openBrowserApplicationStorage("proof/environment-recording", 1, `sha256:${"e".repeat(64)}`,
      { implementationRegistry: ["browser/indexeddb@1"] });
    const scope = { host_id: "snapshot-host", boot_id: "fixture/store/boot-two", active_play_id: "fixture/read", placement_id: "snapshot-placement", request_sequence: 0 };
    const records = [];
    const stale = [];
    for (const [index, { key }] of publications.entries()) {
      const pending = { ...scope, request_sequence: index };
      const effect = { ...pending, schema: "conduit.browser/resource-effect@1", effect_kind: "resource-read", key, record: null };
      const result = await executeResourceStorageEffect(storage, effect, pending);
      try { await executeResourceStorageEffect(storage, { ...effect, boot_id: "fixture/store/boot-one" }, pending); stale.push("unexpected-success"); }
      catch (error) { stale.push(error.code); }
      records.push([key, Array.from(result.record)]);
    }
    const missingScope = { ...scope, request_sequence: publications.length };
    let missing;
    try { await executeResourceStorageEffect(storage, { ...missingScope,
      schema: "conduit.browser/resource-effect@1", effect_kind: "resource-read",
      key: `resource/${"0".repeat(64)}`, record: null }, missingScope); }
    catch (error) { missing = error.code; }
    storage.close();
    return { records, stale, missing };
  }, fixture);
  expect(restored.missing).toBe("ResourceMissing");
  expect(restored.stale).toEqual(Array(16).fill("StaleResourceEffect"));
  expect(restored.records).toEqual(fixture.publications.map(({ key, record }) => [key, record]));
  await writeFile(new URL("restored.json", artifact), JSON.stringify(restored.records));
});
