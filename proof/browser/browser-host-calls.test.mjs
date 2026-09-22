import assert from "node:assert/strict";
import test from "node:test";
import {
  browserHostCallLimits,
  createBrowserHostCalls,
} from "../../targets/browser/host/assets/browser-host-calls.mjs";

const context = Object.freeze({
  hostId: "host/browser-1",
  bootId: "boot/browser-7",
  applicationId: "application/proof",
  applicationGeneration: 2,
  authorityGeneration: 3,
});
const request = (kind, callId, fields = {}) => ({
  contract: browserHostCallLimits.contract,
  kind,
  callId,
  ...context,
  ...fields,
});

test("artifact handoff admits exact bounded identity and preserves terminal distinctions", async () => {
  const handoffs = [];
  const calls = createBrowserHostCalls({
    ...context,
    artifactAdapter: {
      async handoff(value) {
        handoffs.push(value);
        return { disposition: "completed", visibleName: value.filename };
      },
    },
  });
  const completed = await calls.handoffArtifact(request("artifact-handoff", "artifact/save-1", {
    userActivation: true,
    artifactId: "artifact/spore-1",
    bytes: Uint8Array.of(1, 2, 3),
    maximumBytes: 3,
    filename: "body.spore",
    mediaType: "application/vnd.conduit.spore",
  }));
  assert.equal(completed.disposition, "completed");
  assert.equal(completed.callId, "artifact/save-1");
  assert.equal(handoffs[0].bytes.byteLength, 3);
  assert.equal(calls.activeCalls(), 0);

  for (const [name, disposition] of [
    ["AbortError", "cancelled"],
    ["NotAllowedError", "denied"],
    ["SecurityError", "policy-prerequisite-absent"],
    ["NotSupportedError", "unavailable-api"],
    ["QuotaExceededError", "resource-failure"],
    ["UnknownError", "adapter-failure"],
  ]) {
    const failing = createBrowserHostCalls({
      ...context,
      artifactAdapter: { async handoff() { throw new DOMException("failed", name); } },
    });
    const result = await failing.handoffArtifact(request("artifact-handoff", `artifact/${disposition}`, {
      userActivation: true,
      artifactId: "artifact/spore-1",
      bytes: Uint8Array.of(1),
      maximumBytes: 1,
      filename: "body.spore",
      mediaType: "application/octet-stream",
    }));
    assert.equal(result.disposition, disposition);
  }
});

test("artifact and location validation fail before platform effects", async () => {
  let effects = 0;
  const calls = createBrowserHostCalls({
    ...context,
    artifactAdapter: { async handoff() { effects += 1; } },
    locationAdapter: { move() { effects += 1; } },
  });
  const inactive = await calls.handoffArtifact(request("artifact-handoff", "artifact/inactive", {
    userActivation: false,
    artifactId: "artifact/a",
    bytes: Uint8Array.of(1),
    maximumBytes: 1,
    filename: "a.bin",
    mediaType: "application/octet-stream",
  }));
  assert.equal(inactive.disposition, "user-activation-required");
  const oversized = await calls.handoffArtifact(request("artifact-handoff", "artifact/oversized", {
    userActivation: true,
    artifactId: "artifact/a",
    bytes: new Uint8Array(browserHostCallLimits.artifactBytes + 1),
    maximumBytes: browserHostCallLimits.artifactBytes,
    filename: "a.bin",
    mediaType: "application/octet-stream",
  }));
  assert.equal(oversized.disposition, "artifact-not-admitted");
  const external = await calls.moveLocation(request("location", "location/external", {
    presentationRevision: 1,
    mode: "push",
    path: "https://example.com/ambient",
  }));
  assert.equal(external.disposition, "location-not-admitted");
  assert.equal(effects, 0);
});

test("location movement is bounded presentation state and remains correlated", async () => {
  const moves = [];
  const calls = createBrowserHostCalls({
    ...context,
    locationAdapter: {
      move(value) {
        moves.push(value);
        return { disposition: "completed", path: value.path };
      },
    },
  });
  const result = await calls.moveLocation(request("location", "location/page-2", {
    presentationRevision: 9,
    mode: "replace",
    path: "/tour/page-2/",
  }));
  assert.deepEqual(moves, [{ mode: "replace", path: "/tour/page-2/" }]);
  assert.equal(result.disposition, "completed");
  assert.equal(result.path, "/tour/page-2/");
  assert.equal(result.callId, "location/page-2");
  assert.equal("membership" in result, false);
  assert.equal("lifecycle" in result, false);
});

test("device choice is profile gated and acquisition grants no membership or Plan", async () => {
  let chooserCalls = 0;
  const deviceAdapters = {
    "browser/web-serial@1": async ({ filters }) => {
      chooserCalls += 1;
      assert.deepEqual(filters, [{ usbVendorId: 0x1209 }]);
      return { getInfo: () => ({ usbVendorId: 0x1209, usbProductId: 7 }) };
    },
  };
  const omitted = createBrowserHostCalls({ ...context, deviceAdapters, secureContext: true });
  const omittedResult = await omitted.chooseDevice(request("device-choice", "device/omitted", {
    implementationId: "browser/web-serial@1", authorized: true, userActivation: true, filters: [],
    maximumResults: 1, maximumResourceIdentityBytes: 256,
  }));
  assert.equal(omittedResult.disposition, "implementation-not-selected");
  assert.equal(chooserCalls, 0);

  const selected = createBrowserHostCalls({
    ...context,
    selectedImplementations: ["browser/web-serial@1"],
    initializedImplementations: ["browser/web-serial@1"],
    deviceAdapters,
    secureContext: true,
  });
  const acquired = await selected.chooseDevice(request("device-choice", "device/select-1", {
    implementationId: "browser/web-serial@1",
    authorized: true,
    userActivation: true,
    filters: [{ usbVendorId: 0x1209 }],
    maximumResults: 1,
    maximumResourceIdentityBytes: 256,
  }));
  assert.equal(acquired.disposition, "completed");
  assert.deepEqual(acquired.resource, {
    handle: "browser-resource/device/select-1", vendorId: 0x1209, productId: 7, serialNumber: null,
  });
  assert.equal("membership" in acquired.resource, false);
  assert.equal("authority" in acquired.resource, false);
  assert.equal("planId" in acquired.resource, false);
});

test("device outcomes and stale completions remain distinct", async () => {
  for (const [configuration, disposition] of [
    [{ selectedImplementations: ["browser/web-usb@1"] }, "implementation-not-initialized"],
    [{ selectedImplementations: ["browser/web-usb@1"], initializedImplementations: ["browser/web-usb@1"], secureContext: false }, "secure-context-required"],
  ]) {
    const calls = createBrowserHostCalls({ ...context, ...configuration });
    const result = await calls.chooseDevice(request("device-choice", `device/${disposition}`, {
      implementationId: "browser/web-usb@1", authorized: true, userActivation: true, filters: [],
      maximumResults: 1, maximumResourceIdentityBytes: 256,
    }));
    assert.equal(result.disposition, disposition);
  }

  let generation = context.authorityGeneration;
  let complete;
  const pending = new Promise((resolve) => { complete = resolve; });
  const calls = createBrowserHostCalls({
    ...context,
    currentContext: () => ({ ...context, authorityGeneration: generation }),
    selectedImplementations: ["browser/web-usb@1"],
    initializedImplementations: ["browser/web-usb@1"],
    secureContext: true,
    deviceAdapters: { "browser/web-usb@1": () => pending },
  });
  const resultPromise = calls.chooseDevice(request("device-choice", "device/stale", {
    implementationId: "browser/web-usb@1", authorized: true, userActivation: true, filters: [],
    maximumResults: 1, maximumResourceIdentityBytes: 256,
  }));
  generation += 1;
  complete({ vendorId: 1, productId: 2 });
  assert.equal((await resultPromise).disposition, "stale-completion");
  assert.equal(calls.activeCalls(), 0);
});

test("device authority, activation, API, chooser, and identity outcomes stay separate", async () => {
  const base = {
    ...context,
    selectedImplementations: ["browser/web-usb@1"],
    initializedImplementations: ["browser/web-usb@1"],
    secureContext: true,
  };
  const choose = (calls, callId, fields = {}) => calls.chooseDevice(request(
    "device-choice",
    callId,
    {
      implementationId: "browser/web-usb@1",
      authorized: true,
      userActivation: true,
      filters: [],
      maximumResults: 1,
      maximumResourceIdentityBytes: 256,
      ...fields,
    },
  ));
  assert.equal((await choose(createBrowserHostCalls(base), "device/no-authority", {
    authorized: false,
  })).disposition, "authority-missing");
  assert.equal((await choose(createBrowserHostCalls(base), "device/no-activation", {
    userActivation: false,
  })).disposition, "user-activation-required");
  assert.equal((await choose(createBrowserHostCalls({ ...base, deviceAdapters: {} }),
    "device/unavailable")).disposition, "unavailable-api");

  for (const [name, disposition] of [
    ["AbortError", "cancelled"],
    ["NotAllowedError", "denied"],
    ["SecurityError", "policy-prerequisite-absent"],
  ]) {
    const calls = createBrowserHostCalls({
      ...base,
      deviceAdapters: { "browser/web-usb@1": async () => { throw new DOMException("choice", name); } },
    });
    assert.equal((await choose(calls, `device/${disposition}`)).disposition, disposition);
  }
  const noMatch = createBrowserHostCalls({
    ...base, deviceAdapters: { "browser/web-usb@1": async () => null },
  });
  assert.equal((await choose(noMatch, "device/no-match")).disposition, "no-matching-device");
  const identityPressure = createBrowserHostCalls({
    ...base,
    deviceAdapters: { "browser/web-usb@1": async () => ({ serialNumber: "x".repeat(300) }) },
  });
  assert.equal((await choose(identityPressure, "device/identity-pressure"))
    .disposition, "resource-identity-pressure");
});

test("call slots are finite and duplicate correlation refuses", async () => {
  const completions = [];
  const calls = createBrowserHostCalls({
    ...context,
    artifactAdapter: { handoff: () => new Promise((resolve) => completions.push(resolve)) },
  });
  const artifact = (callId) => calls.handoffArtifact(request("artifact-handoff", callId, {
    userActivation: true,
    artifactId: `artifact/${callId}`,
    bytes: Uint8Array.of(1),
    maximumBytes: 1,
    filename: `${callId.replace("/", "-")}.bin`,
    mediaType: "application/octet-stream",
  }));
  const pending = Array.from({ length: browserHostCallLimits.slots }, (_, index) => artifact(`slot/${index}`));
  await assert.rejects(artifact("slot/0"), (error) => error.code === "duplicate-call");
  await assert.rejects(artifact("slot/overflow"), (error) => error.code === "call-pressure");
  for (const complete of completions) complete({ disposition: "completed" });
  assert.equal((await Promise.all(pending)).every((value) => value.disposition === "completed"), true);
  assert.equal(calls.activeCalls(), 0);
});
