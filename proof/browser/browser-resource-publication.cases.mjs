import { expect, test } from "@playwright/test";

// Real IndexedDB realization proof. Resource admission and task interpretation
// remain the runtime/Form owners; this layer stores opaque bounded bytes.
test("byte publication is atomic, immutable across reopen, and independent of caller buffers", async ({ page }) => {
  await page.goto("/proof/browser/signal-dom-host.test.html");
  const result = await page.evaluate(async () => {
    const { openBrowserApplicationStorage } = await import("/targets/browser/host/assets/browser-application-storage.mjs");
    const options = { implementationRegistry: ["browser/indexeddb@1"] };
    const digest = `sha256:${"a".repeat(64)}`;
    const open = () => openBrowserApplicationStorage("proof/resource-publication", 1, digest, options);
    const first = await open();
    const second = await open();
    const attempts = await Promise.allSettled([
      first.publishBytes("generation/1", new Uint8Array([1, 2, 3])),
      second.publishBytes("generation/1", new Uint8Array([4, 5, 6])),
    ]);
    const before = Array.from(await first.readBytes("generation/1"));
    const refusals = [];
    for (const write of [
      () => first.writeBytes("generation/1", new Uint8Array([9])),
      () => second.writeJson("generation/1", { replacement: true }),
      () => second.publishBytes("generation/1", new Uint8Array(before)),
    ]) {
      try { await write(); refusals.push("unexpected-success"); }
      catch (error) { refusals.push(error.code); }
    }
    const source = new Uint8Array(first.bounds.maximumValueBytes);
    source[0] = 7;
    source[source.length - 1] = 8;
    await first.publishBytes("generation/2", source);
    source.fill(0);
    first.close();
    second.close();
    const reopened = await open();
    const after = Array.from(await reopened.readBytes("generation/1"));
    const copy = await reopened.readBytes("generation/2");
    const bounds = [copy.length, copy[0], copy.at(-1)];
    copy.fill(0);
    const retained = await reopened.readBytes("generation/2");
    let oversize;
    try { await reopened.publishBytes("generation/3", new Uint8Array(reopened.bounds.maximumValueBytes + 1)); }
    catch (error) { oversize = error.code; }
    const absent = await reopened.readBytes("generation/3");
    await reopened.clearApplication();
    reopened.close();
    return {
      statuses: attempts.map((attempt) => attempt.status).sort(),
      raceRefusal: attempts.find((attempt) => attempt.status === "rejected")?.reason.code,
      before, after, refusals, bounds, retainedFirst: retained[0], oversize, absent,
    };
  });
  expect(result.statuses).toEqual(["fulfilled", "rejected"]);
  expect(result.raceRefusal).toBe("PublishedImmutable");
  expect([[1, 2, 3], [4, 5, 6]]).toContainEqual(result.before);
  expect(result.after).toEqual(result.before);
  expect(result.refusals).toEqual(Array(3).fill("PublishedImmutable"));
  expect(result.bounds).toEqual([65536, 7, 8]);
  expect(result.retainedFirst).toBe(7);
  expect(result.oversize).toBe("ValueBound");
  expect(result.absent).toBeNull();
});

test("Resource effects preserve exact pending scope and restore opaque bytes through IndexedDB", async ({ page }) => {
  await page.goto("/proof/browser/signal-dom-host.test.html");
  const result = await page.evaluate(async () => {
    const { openBrowserApplicationStorage } = await import("/targets/browser/host/assets/browser-application-storage.mjs");
    const { executeResourceStorageEffect, MAXIMUM_RESOURCE_RECORD_BYTES } = await import("/targets/browser/host/assets/browser-resource-storage.mjs");
    const open = () => openBrowserApplicationStorage("proof/resource-effects", 1, `sha256:${"b".repeat(64)}`, { implementationRegistry: ["browser/indexeddb@1"] });
    const scope = { host_id: "host/one", boot_id: "boot/one", active_play_id: "play/write", placement_id: "placement/write", request_sequence: 0 };
    const key = `resource/${"1".repeat(64)}`;
    const effect = { ...scope, schema: "conduit.browser/resource-effect@1", effect_kind: "resource-publish", key, record: [67, 68, 82, 83] };
    let storage = await open();
    const published = await executeResourceStorageEffect(storage, effect, scope);
    let duplicate;
    try { await executeResourceStorageEffect(storage, effect, scope); } catch (error) { duplicate = error.code; }
    storage.close();
    storage = await open();
    const current = { ...scope, boot_id: "boot/two", active_play_id: "play/read", placement_id: "placement/read" };
    const read = { ...effect, ...current, effect_kind: "resource-read", record: null };
    const restored = await executeResourceStorageEffect(storage, read, current);
    const refused = [];
    for (const changed of [
      { ...read, boot_id: "boot/one" }, { ...read, host_id: "foreign" },
      { ...read, request_sequence: 1 }, { ...read, active_play_id: "old-play" },
    ]) {
      try { await executeResourceStorageEffect(storage, changed, current); refused.push("unexpected-success"); }
      catch (error) { refused.push(error.code); }
    }
    let missing;
    try { await executeResourceStorageEffect(storage, { ...read, key: `resource/${"2".repeat(64)}` }, current); }
    catch (error) { missing = error.code; }
    let oversize;
    try { await executeResourceStorageEffect(storage, { ...effect, record: Array(MAXIMUM_RESOURCE_RECORD_BYTES + 1).fill(0) }, scope); }
    catch (error) { oversize = error.code; }
    const invalid = [];
    for (const [provider, request, pending] of [
      [{}, read, current],
      [{}, effect, scope],
      [storage, { ...effect, record: Array(4) }, scope],
      [storage, { ...read, record: [] }, current],
    ]) {
      try { await executeResourceStorageEffect(provider, request, pending); invalid.push("unexpected-success"); }
      catch (error) { invalid.push(error.code); }
    }
    await storage.clearApplication(); storage.close();
    return { published, restored: Array.from(restored.record), duplicate, refused, missing, oversize, invalid };
  });
  expect(result).toEqual({ published: { status: "completed", record: null }, restored: [67, 68, 82, 83],
    duplicate: "PublishedImmutable", refused: Array(4).fill("StaleResourceEffect"), missing: "ResourceMissing", oversize: "ResourceRecordBound",
    invalid: ["ImplementationNotSelected", "ImplementationNotSelected", "ResourceRecordBound", "InvalidResourceEffect"] });
});
