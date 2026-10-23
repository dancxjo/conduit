import { expect, test } from "@playwright/test";
import { readFile, writeFile } from "node:fs/promises";

test("IndexedDB serves the original recorder kernel requests after page reload", async ({ page }) => {
  const artifact = new URL("../../target/environment-recording-kernel-browser/", import.meta.url);
  const fixture = JSON.parse(await readFile(new URL("requests.json", artifact), "utf8"));
  await page.goto("/proof/browser/signal-dom-host.test.html");
  await page.evaluate(async ({ publications }) => {
    const { openBrowserApplicationStorage } = await import("/targets/browser/host/assets/browser-application-storage.mjs");
    const { executeResourceStorageEffect } = await import("/targets/browser/host/assets/browser-resource-storage.mjs");
    const storage = await openBrowserApplicationStorage("proof/environment-kernel-reads", 1, `sha256:${"d".repeat(64)}`,
      { implementationRegistry: ["browser/indexeddb@1"] });
    await storage.clearApplication();
    for (const [index, publication] of publications.entries()) {
      // Publication custody is fixture setup. Read effects below originate in
      // the production kernel and its prepared browser realization.
      const pending = { host_id: "fixture/recorder", boot_id: "fixture/store/old-boot",
        active_play_id: "fixture/write", placement_id: "snapshot-placement", request_sequence: index };
      await executeResourceStorageEffect(storage, { ...pending, schema: "conduit.browser/resource-effect@1",
        effect_kind: "resource-publish", ...publication }, pending);
    }
    storage.close();
  }, fixture);
  await page.reload();
  const returned = await page.evaluate(async ({ reads }) => {
    const { openBrowserApplicationStorage } = await import("/targets/browser/host/assets/browser-application-storage.mjs");
    const { executeResourceStorageEffect } = await import("/targets/browser/host/assets/browser-resource-storage.mjs");
    const storage = await openBrowserApplicationStorage("proof/environment-kernel-reads", 1, `sha256:${"d".repeat(64)}`,
      { implementationRegistry: ["browser/indexeddb@1"] });
    const records = [];
    for (const effect of reads) {
      const { host_id, boot_id, active_play_id, placement_id, request_sequence } = effect;
      const pending = { host_id, boot_id, active_play_id, placement_id, request_sequence };
      const result = await executeResourceStorageEffect(storage, effect, pending);
      records.push({ ...pending, record: Array.from(result.record) });
    }
    storage.close();
    return records;
  }, fixture);
  expect(returned).toHaveLength(15);
  await writeFile(new URL("restored.json", artifact), JSON.stringify(returned));
});
