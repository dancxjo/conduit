import { expect, test } from "@playwright/test";

// Real IndexedDB realization proof. Resource admission and task interpretation
// remain the runtime/Plot owners; this layer stores opaque bounded bytes.
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

// The fake ABI only supplies an already-admitted Plan and records Host acknowledgements.
// The production Body Host performs the effect against real IndexedDB on each page load.
async function bodySnapshotEffect(page, operation) {
  return page.evaluate(async ({ operation }) => {
    const { acquireBrowserBodyHost } = await import("/targets/browser/host/assets/browser-body-host.mjs");
    const { openBrowserApplicationStorage } = await import("/targets/browser/host/assets/browser-application-storage.mjs");
    const storage = await openBrowserApplicationStorage("proof/body-snapshot-effect", 1,
      `sha256:${"c".repeat(64)}`, { implementationRegistry: ["browser/indexeddb@1"] });
    const publish = operation !== "read";
    const contract = publish ? "conduit.host/resource-snapshot-publish@1" : "conduit.host/resource-snapshot-read@1";
    const kind = publish ? "resource/snapshot-publish" : "resource/snapshot-read";
    const memory = new WebAssembly.Memory({ initial: 8 });
    const encoder = new TextEncoder();
    let length = 0, acknowledgement = null;
    const output = value => {
      const bytes = encoder.encode(JSON.stringify(value));
      new Uint8Array(memory.buffer, 1024, bytes.length).set(bytes);
      length = bytes.length;
    };
    const receipt = { schema: "conduit.tour/manifestation-receipt@3", disposition: "completed", active_play_id: "play" };
    const effect = {
      schema: "conduit.browser/resource-effect@1", effect_kind: publish ? "resource-publish" : "resource-read",
      host_id: "host", boot_id: "boot", active_play_id: "play", placement_id: "snapshot-placement",
      request_sequence: 0, key: `resource/${"3".repeat(64)}`, record: publish ? [67, 68, 82, 83] : null,
    };
    new Uint8Array(memory.buffer).set(encoder.encode("conduit.browser/runtime-abi"), 0);
    const api = {
      memory,
      conduit_browser_runtime_abi_identity_ptr: () => 0,
      conduit_browser_runtime_abi_identity_len: () => 27,
      conduit_browser_runtime_abi_revision: () => 1,
      conduit_browser_plot_output_ptr: () => 1024,
      conduit_browser_plot_output_len: () => length,
      conduit_browser_body_input_ptr: () => 256 * 1024,
      conduit_browser_body_input_capacity: () => 256 * 1024,
      conduit_browser_plot_input_ptr: () => 128 * 1024,
      conduit_browser_plot_input_capacity: () => 64 * 1024,
      conduit_browser_plot_pending_capacity: () => 1,
      conduit_browser_plot_human_machinery() {
        output({ schema: "conduit.browser/selected-human-machinery@1", limits: { maximum_gears: 16 }, implementations: [] });
        return 0;
      },
      conduit_browser_body_start() {
        output({ schema: "conduit.browser/body-started@1", play: {
          active_play_id: "play", plan_id: "body-plan", wake_id: "wake", body_id: "body",
        }, progress: effect });
        return 0;
      },
      conduit_browser_plot_poll_effect() { output({ disposition: "waiting" }); return 0; },
      conduit_browser_plot_complete_effect(playLength, placementLength, sequence, bytesLength) {
        const bytes = new Uint8Array(memory.buffer, 128 * 1024 + playLength + placementLength, bytesLength);
        acknowledgement = { sequence, bytes: Array.from(bytes) };
        output(receipt);return 0;
      },
      conduit_browser_plot_refuse_effect(playLength, placementLength, sequence, disposition, detail) {
        acknowledgement = { sequence, disposition, detail };
        output({ ...receipt, disposition: "failed" });return 0;
      },
      conduit_tour_cancel() { output({ ...receipt, disposition: "cancelled" });return 0; },
    };
    const placement = {
      placement_id: "snapshot-placement", gear_id: "snapshot", kind_id: kind,
      capability_id: contract, host_calls: [{ contract_id: contract }],
      authority: [{ contract_id: "authority/resource-snapshot@1", host_call_contract_id: contract,
        subject_kind: kind, host_id: "host", boot_id: "boot", capability_id: contract }],
      resources: [{ pool_id: "snapshot-pool", class_id: "resource/snapshot@1", units: 1,
        content: { owner_host: "host", owner_boot: "boot", base_id: "browser/indexeddb",
          residence_profile: "browser/indexeddb@1", contract: { retention: "ExternalDurable",
            sharing: "SingleWriterPublished", access: publish ? "WriteCandidatePublish" : "ReadPublished" } } }],
    };
    const proposal = { schema: "conduit.body/execution-proposal@1", wake: {
      wake_id: "wake", lifecycle: "AwaitingPlan", plans: [],
    }, plan: { plan_id: "body-plan", body_id: "body", plots: [{ plan: {
      fragments: [{ host_id: "host", boot_id: "boot", offer_generation: 1, placements: [placement] }],
    } }] } };
    const root = document.createElement("main");document.body.append(root);
    if (operation === "wrong-grant") {
      placement.authority[0].host_call_contract_id = "conduit.host/foreign@1";
      let refusal;
      try { acquireBrowserBodyHost({ api, hostId: "host", bootId: "boot", proposal,
        inputTarget: root, outputRoot: root, storage }); }
      catch (error) { refusal = error.message; }
      root.remove();storage.close();
      return { refusal, acknowledgement };
    }
    const owner = acquireBrowserBodyHost({ api, hostId: "host", bootId: "boot", proposal,
      inputTarget: root, outputRoot: root, storage });
    const observed = owner.observations();
    owner.start(1);
    const result = await owner.run();
    owner.close();root.remove();
    if (operation === "read") await storage.clearApplication();
    storage.close();
    return { observed: observed.map(item => [item.pool_id, item.unreserved_units]),
      disposition: result.disposition, acknowledgement };
  }, { operation });
}

test("Body Host acknowledges exact snapshot publication and reads that generation after reload", async ({ page }) => {
  await page.goto("/proof/browser/signal-dom-host.test.html");
  const unauthorized = await bodySnapshotEffect(page, "wrong-grant");
  expect(unauthorized).toEqual({ refusal: "snapshot binding lacks its selected durable storage or exact authority",
    acknowledgement: null });
  const published = await bodySnapshotEffect(page, "publish");
  expect(published).toEqual({ observed: [["snapshot-pool", 1]], disposition: "completed",
    acknowledgement: { sequence: 0, bytes: [] } });
  const duplicate = await bodySnapshotEffect(page, "duplicate");
  expect(duplicate).toEqual({ observed: [["snapshot-pool", 1]], disposition: "failed",
    acknowledgement: { sequence: 0, disposition: 2, detail: 211 } });
  await page.reload();
  const restored = await bodySnapshotEffect(page, "read");
  expect(restored).toEqual({ observed: [["snapshot-pool", 1]], disposition: "completed",
    acknowledgement: { sequence: 0, bytes: [67, 68, 82, 83] } });
});
