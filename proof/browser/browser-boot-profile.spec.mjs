import { expect, test } from "@playwright/test";

const IDS = [
  "browser/dom@1",
  "browser/media-devices-camera@1",
  "browser/websocket@1",
];

test.beforeEach(async ({ page }) => page.goto("/proof/browser/signal-dom-host.test.html"));

test("durable storage is offered only when its exact implementation is selected and available", async ({ page }) => {
  const fixture = await makeImage(["browser/dom@1"]);
  const selectedFixture = await makeImage(["browser/indexeddb@1"]);
  const result = await page.evaluate(async ({ fixture, selectedFixture, artifactDigest }) => {
    const boot = await import(new URL("../../targets/browser/host/assets/browser-boot-profile.mjs", location.href).href);
    const image = { ...fixture, bytes: new Uint8Array(fixture.bytes) };
    const common = {
      imageBytes: image.bytes, expectedImageId: image.id, expectedProfileId: image.profileId,
      runtimeBytes: new Uint8Array(fixture.runtimeBytes), bootModuleDigest: fixture.bootModuleDigest,
      artifactContentDigest: artifactDigest, bootId: "boot/storage",
      availableImplementations: ["browser/dom@1", "browser/indexeddb@1"].map((id) => ({ id, revision: 1 })),
    };
    const omitted = await boot.admitBrowserBoot({ ...common, observations: { "browser/dom@1": { api_supported: true } } });
    const selectedImage = { ...selectedFixture, bytes: new Uint8Array(selectedFixture.bytes) };
    const selected = await boot.admitBrowserBoot({
      ...common, imageBytes: selectedImage.bytes, expectedImageId: selectedImage.id,
      expectedProfileId: selectedImage.profileId,
      observations: { "browser/indexeddb@1": { api_supported: true, secure_context: true, resource_ready: true } },
    });
    return { omitted, selected };
  }, {
    fixture: { ...fixture, bytes: Array.from(fixture.bytes) },
    selectedFixture: { ...selectedFixture, bytes: Array.from(selectedFixture.bytes) },
    artifactDigest: digest("6"),
  });
  expect(result.omitted.inspection.some(({ implementation_id }) => implementation_id === "browser/indexeddb@1")).toBe(false);
  expect(result.selected.inspection[0]).toMatchObject({ implementation_id: "browser/indexeddb@1", configured: true, initialized: true, offered: true });
});

test("selected durable storage reopens one maximum bounded binary value exactly", async ({ page }) => {
  const result = await page.evaluate(async () => {
    const module = await import(new URL("../../targets/browser/host/assets/browser-application-storage.mjs", location.href).href);
    const selected = { implementationRegistry: ["browser/indexeddb@1"] };
    const digest = `sha256:${"a".repeat(64)}`;
    const first = await module.openBrowserApplicationStorage("proof/history-snapshot", 1, digest, selected);
    const source = new Uint8Array(first.bounds.maximumValueBytes);
    source[0] = 0x43;
    source[1] = 0x48;
    source[source.length - 1] = 0x54;
    await first.writeBytes("timeline", source);
    source[0] = 0;
    first.close();

    const incompatible = await module.openBrowserApplicationStorage("proof/history-snapshot", 2, digest, selected);
    let versionMismatch;
    try { await incompatible.readBytes("timeline"); } catch (error) { versionMismatch = error.code; }
    incompatible.close();

    const reopened = await module.openBrowserApplicationStorage("proof/history-snapshot", 1, digest, selected);
    const restored = await reopened.readBytes("timeline");
    let kindMismatch;
    try { await reopened.readJson("timeline"); } catch (error) { kindMismatch = error.code; }
    await reopened.writeJson("metadata", { entries: 2 });
    let jsonMismatch;
    try { await reopened.readBytes("metadata"); } catch (error) { jsonMismatch = error.code; }
    let oversize;
    try { await reopened.writeBytes("oversize", new Uint8Array(reopened.bounds.maximumValueBytes + 1)); }
    catch (error) { oversize = error.code; }
    let wrongValue;
    try { await reopened.writeBytes("wrong-value", [1, 2, 3]); }
    catch (error) { wrongValue = error.code; }
    await reopened.clearApplication();
    const afterClear = await reopened.readBytes("timeline");
    reopened.close();
    return {
      byteLength: restored.byteLength,
      first: restored[0],
      second: restored[1],
      last: restored.at(-1),
      versionMismatch,
      kindMismatch,
      jsonMismatch,
      oversize,
      wrongValue,
      afterClear,
    };
  });
  expect(result).toEqual({
    byteLength: 64 * 1024,
    first: 0x43,
    second: 0x48,
    last: 0x54,
    versionMismatch: "VersionMismatch",
    kindMismatch: "ValueKindMismatch",
    jsonMismatch: "ValueKindMismatch",
    oversize: "ValueBound",
    wrongValue: "ValueEncoding",
    afterClear: null,
  });
});

test("device PROFILE selection offers only acquisition without permission or resource claims", async ({ page }) => {
  const ids = ["browser/webserial@1", "browser/webusb@1"];
  const fixture = await makeImage(ids);
  const result = await page.evaluate(async ({ ids, fixture, artifactDigest }) => {
    const boot = await import(new URL("../../targets/browser/host/assets/browser-boot-profile.mjs", location.href).href);
    const image = { ...fixture, bytes: new Uint8Array(fixture.bytes) };
    return boot.admitBrowserBoot({
      imageBytes: image.bytes,
      expectedImageId: image.id,
      expectedProfileId: image.profileId,
      runtimeBytes: new Uint8Array(fixture.runtimeBytes),
      bootModuleDigest: fixture.bootModuleDigest,
      artifactContentDigest: artifactDigest,
      bootId: "boot/devices",
      availableImplementations: ids.map((id) => ({ id, revision: 1 })),
      observations: Object.fromEntries(ids.map((id) => [id, {
        api_supported: true, secure_context: true, permission: "prompt", resource_ready: false,
      }])),
    });
  }, { ids, fixture: { ...fixture, bytes: Array.from(fixture.bytes) }, artifactDigest: digest("8") });
  expect(result.offers).toEqual([
    expect.objectContaining({ offer_id: "device/acquire-webserial@1", resource_identity: null }),
    expect.objectContaining({ offer_id: "device/acquire-webusb@1", resource_identity: null }),
  ]);
});

test("exact IMAGE gates a superset runtime into only selected implementations and current offers", async ({ page }) => {
  const fixture = await makeImage(IDS.slice(0, 2));
  const result = await page.evaluate(async ({ ids, fixture, artifactDigest }) => {
    const boot = await import(new URL("../../targets/browser/host/assets/browser-boot-profile.mjs", location.href).href);
    const image = { ...fixture, bytes: new Uint8Array(fixture.bytes) };
    const snapshot = await boot.admitBrowserBoot({
      imageBytes: image.bytes,
      expectedImageId: image.id,
      expectedProfileId: image.profileId,
      runtimeBytes: new Uint8Array(fixture.runtimeBytes),
      bootModuleDigest: fixture.bootModuleDigest,
      artifactContentDigest: artifactDigest,
      bootId: "boot/one",
      availableImplementations: ids.map((id) => ({ id, revision: 1 })),
      observations: {
        "browser/dom@1": { api_supported: true },
        "browser/media-devices-camera@1": { api_supported: true, secure_context: true, permission: "denied", resource_ready: false },
      },
      bundleVariant: "superset",
    });
    return snapshot;
  }, { ids: IDS, fixture: { ...fixture, bytes: Array.from(fixture.bytes) }, artifactDigest: digest("a") });

  expect(result.implementation_registry.map(({ id }) => id)).toEqual([
    "browser/dom@1", "browser/media-devices-camera@1",
  ]);
  expect(result.inspection).toEqual([
    expect.objectContaining({ implementation_id: "browser/dom@1", configured: true, admitted: true, initialized: true, resource_ready: true, offered: true }),
    expect.objectContaining({ implementation_id: "browser/media-devices-camera@1", offer_id: "media/acquire-camera@1", configured: true, admitted: true, initialized: true, resource_ready: true, offered: true, refusal: null }),
  ]);
  expect(result.inspection.some(({ implementation_id }) => implementation_id === "browser/websocket@1")).toBe(false);
});

test("unsupported, initialization, permission, provider, endpoint authority, and successful offer states remain distinct", async ({ page }) => {
  const fixture = await makeImage(IDS);
  const states = await page.evaluate(async ({ ids, fixture, artifactDigest }) => {
    const boot = await import(new URL("../../targets/browser/host/assets/browser-boot-profile.mjs", location.href).href);
    const image = { ...fixture, bytes: new Uint8Array(fixture.bytes) };
    const availableImplementations = ids.map((id) => ({ id, revision: 1 }));
    async function inspect(observations) {
      const snapshot = await boot.admitBrowserBoot({
        imageBytes: image.bytes, expectedImageId: image.id, expectedProfileId: image.profileId,
        runtimeBytes: new Uint8Array(fixture.runtimeBytes), bootModuleDigest: fixture.bootModuleDigest,
        artifactContentDigest: artifactDigest, bootId: "boot/matrix", availableImplementations, observations,
      });
      return Object.fromEntries(snapshot.inspection.map((item) => [item.implementation_id, item]));
    }
    return {
      unsupported: await inspect({
        "browser/dom@1": { api_supported: false },
        "browser/media-devices-camera@1": { api_supported: true, secure_context: true, permission: "granted", resource_ready: true },
        "browser/websocket@1": { api_supported: true, secure_context: true, provider_ready: true },
      }),
      initialization: await inspect({
        "browser/dom@1": { api_supported: true, initialization_failure: true },
        "browser/media-devices-camera@1": { api_supported: true, secure_context: true, permission: "denied" },
        "browser/websocket@1": { api_supported: true, secure_context: true, provider_ready: false },
      }),
      endpoint: await inspect({
        "browser/dom@1": { api_supported: true },
        "browser/media-devices-camera@1": { api_supported: true, secure_context: true, permission: "denied" },
        "browser/websocket@1": { api_supported: true, secure_context: true, provider_ready: true, endpoint_ready: false, authority_ready: true },
      }),
      authority: await inspect({
        "browser/dom@1": { api_supported: true },
        "browser/media-devices-camera@1": { api_supported: true, secure_context: true, permission: "denied" },
        "browser/websocket@1": { api_supported: true, secure_context: true, provider_ready: true, endpoint_ready: true, authority_ready: false },
      }),
      successful: await inspect({
        "browser/dom@1": { api_supported: true },
        "browser/media-devices-camera@1": { api_supported: true, secure_context: true, permission: "granted", resource_ready: true, resource_identity: "camera/front" },
        "browser/websocket@1": { api_supported: true, secure_context: true, provider_ready: true, endpoint_ready: true, authority_ready: true },
      }),
    };
  }, { ids: IDS, fixture: { ...fixture, bytes: Array.from(fixture.bytes) }, artifactDigest: digest("b") });

  expect(states.unsupported["browser/dom@1"].refusal).toBe("UnsupportedApi");
  expect(states.initialization["browser/dom@1"].refusal).toBe("InitializationFailed");
  expect(states.initialization["browser/media-devices-camera@1"]).toMatchObject({
    offer_id: "media/acquire-camera@1", offered: true, refusal: null,
  });
  expect(states.initialization["browser/websocket@1"].refusal).toBe("ProviderUnavailable");
  expect(states.endpoint["browser/websocket@1"].refusal).toBe("EndpointUnavailable");
  expect(states.authority["browser/websocket@1"].refusal).toBe("EndpointAuthorityAbsent");
  expect(states.successful["browser/websocket@1"]).toMatchObject({ offered: true, resource_ready: true });
  expect(states.successful["browser/media-devices-camera@1"]).toMatchObject({ offered: true, resource_ready: true, resource_identity: "camera/front" });
});

test("permission and acquired-resource observations do not collapse the media acquisition offer", async ({ page }) => {
  const fixture = await makeImage([IDS[1]]);
  const result = await page.evaluate(async ({ ids, fixture, artifactDigest }) => {
    const boot = await import(new URL("../../targets/browser/host/assets/browser-boot-profile.mjs", location.href).href);
    const image = { ...fixture, bytes: new Uint8Array(fixture.bytes) };
    const common = {
      imageBytes: image.bytes, expectedImageId: image.id, expectedProfileId: image.profileId,
      runtimeBytes: new Uint8Array(fixture.runtimeBytes), bootModuleDigest: fixture.bootModuleDigest,
      artifactContentDigest: artifactDigest, bootId: "boot/loss",
      availableImplementations: ids.map((id) => ({ id, revision: 1 })),
      observations: { [ids[1]]: { api_supported: true, secure_context: true, permission: "granted", resource_ready: true, resource_identity: "camera/7" } },
    };
    const before = await boot.admitBrowserBoot(common);
    const realization = boot.bindBrowserOfferRealization(before, {
      realizationId: "realization/7", offerId: "media/acquire-camera@1", formId: "form/unchanged", planId: "plan/7",
    });
    const after = boot.refreshBrowserBootTruth(before, {
      [ids[1]]: { api_supported: true, secure_context: true, permission: "granted", resource_ready: false, resource_lost: true },
    });
    return { before, after, loss: boot.reconcileBrowserOfferRealizations(before, after, [realization]) };
  }, { ids: IDS, fixture: { ...fixture, bytes: Array.from(fixture.bytes) }, artifactDigest: digest("c") });

  expect(result.after.image_id).toBe(result.before.image_id);
  expect(result.after.profile_id).toBe(result.before.profile_id);
  expect(result.before.inspection[0]).toMatchObject({ offered: true, offer_id: "media/acquire-camera@1" });
  expect(result.after.inspection[0]).toMatchObject({ offered: true, offer_id: "media/acquire-camera@1" });
  expect(result.loss).toEqual([]);
  expect(result.after.inspection[0].resource_identity).toBeNull();
});

test("WebRTC signaling bootstrap and exact session grant remain separate from DataChannel offer truth", async ({ page }) => {
  const implementation = "browser/webrtc-datachannel@1";
  const fixture = await makeImage([implementation]);
  const result = await page.evaluate(async ({ implementation, fixture, artifactDigest }) => {
    const boot = await import(new URL("../../targets/browser/host/assets/browser-boot-profile.mjs", location.href).href);
    const image = { ...fixture, bytes: new Uint8Array(fixture.bytes) };
    const common = {
      imageBytes: image.bytes, expectedImageId: image.id, expectedProfileId: image.profileId,
      runtimeBytes: new Uint8Array(fixture.runtimeBytes), bootModuleDigest: fixture.bootModuleDigest,
      artifactContentDigest: artifactDigest, bootId: "boot/webrtc",
      availableImplementations: [{ id: implementation, revision: 1 }],
    };
    const observation = {
      api_supported: true, secure_context: true, provider_ready: true,
      endpoint_ready: true, authority_ready: true,
    };
    const noSignaling = await boot.admitBrowserBoot({ ...common, observations: { [implementation]: observation } });
    const noGrant = await boot.admitBrowserBoot({
      ...common, observations: { [implementation]: { ...observation, signaling_ready: true } },
    });
    const ready = await boot.admitBrowserBoot({
      ...common, observations: { [implementation]: { ...observation, signaling_ready: true, session_grant_ready: true } },
    });
    const realization = boot.bindBrowserOfferRealization(ready, {
      realizationId: "line-realization/webrtc/1", offerId: "line/webrtc-datachannel@1",
      formId: "form/portable-camera", planId: "plan/webrtc/1",
    });
    return { noSignaling, noGrant, ready, realization };
  }, { implementation, fixture: { ...fixture, bytes: Array.from(fixture.bytes) }, artifactDigest: digest("7") });

  expect(result.noSignaling.inspection[0]).toMatchObject({ offered: false, refusal: "SignalingBootstrapAbsent" });
  expect(result.noGrant.inspection[0]).toMatchObject({ offered: false, refusal: "SessionGrantAbsent" });
  expect(result.ready.inspection[0]).toMatchObject({ offered: true, resource_ready: true });
  expect(result.realization).toMatchObject({
    implementation_id: implementation,
    form_id: "form/portable-camera",
    plan_id: "plan/webrtc/1",
    admitted_offer_generation: 1,
  });
  expect(result.realization).not.toHaveProperty("body_id");
  expect(result.realization).not.toHaveProperty("signaling_url");
});

test("superset and reduced bundles yield equivalent semantic truth while stale IMAGE and missing selected code refuse", async ({ page }) => {
  const fixture = await makeImage([IDS[0], IDS[2]]);
  const result = await page.evaluate(async ({ ids, fixture, artifactDigest }) => {
    const boot = await import(new URL("../../targets/browser/host/assets/browser-boot-profile.mjs", location.href).href);
    const image = { ...fixture, bytes: new Uint8Array(fixture.bytes) };
    const observations = {
      [ids[0]]: { api_supported: true },
      [ids[2]]: { api_supported: true, secure_context: true, provider_ready: true, endpoint_ready: true, authority_ready: true },
    };
    const input = {
      imageBytes: image.bytes, expectedImageId: image.id, expectedProfileId: image.profileId,
      runtimeBytes: new Uint8Array(fixture.runtimeBytes), bootModuleDigest: fixture.bootModuleDigest,
      artifactContentDigest: artifactDigest, bootId: "boot/parity", observations,
    };
    const superset = await boot.admitBrowserBoot({ ...input, availableImplementations: ids.map((id) => ({ id, revision: 1 })), bundleVariant: "superset" });
    const reduced = await boot.admitBrowserBoot({ ...input, availableImplementations: [ids[0], ids[2]].map((id) => ({ id, revision: 1 })), bundleVariant: "reduced-modules" });
    const capture = async (operation) => { try { await operation(); return "accepted"; } catch (error) { return error.code; } };
    const changedValue = JSON.parse(new TextDecoder().decode(image.bytes));
    changedValue.source_configuration_id = artifactDigest;
    const changed = new TextEncoder().encode(JSON.stringify(changedValue));
    return {
      superset: { registry: superset.implementation_registry, offers: superset.offers, inspection: superset.inspection },
      reduced: { registry: reduced.implementation_registry, offers: reduced.offers, inspection: reduced.inspection },
      stale: await capture(() => boot.admitBrowserBoot({ ...input, imageBytes: changed, availableImplementations: ids.map((id) => ({ id, revision: 1 })) })),
      absent: await boot.admitBrowserBoot({ ...input, availableImplementations: [{ id: ids[0], revision: 1 }] }),
    };
  }, { ids: IDS, fixture: { ...fixture, bytes: Array.from(fixture.bytes) }, artifactDigest: digest("d") });

  expect(result.reduced).toEqual(result.superset);
  expect(result.stale).toBe("ImageDigestMismatch");
  expect(result.absent.inspection.find(({ implementation_id }) => implementation_id === "browser/websocket@1")).toMatchObject({ admitted: false, offered: false, refusal: "ImplementationCodeAbsent" });
  expect(result.reduced.inspection.every(({ admitted }) => admitted)).toBe(true);
});

async function makeImage(implementations) {
  const runtimeBytes = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]);
  const runtimeDigest = await hash(runtimeBytes);
  const bootModuleDigest = digest("5");
  const payload = {
    schema: "conduit.browser/bundle-image@1",
    build_id: `build:${digest("1")}`,
    target_id: "browser/wasm32/page",
    profile_id: digest("2"),
    source_configuration_id: digest("3"),
    reviewed_distribution: { distribution_id: "fixture", distribution_digest: digest("4"), runtime_abi: "conduit.browser/runtime-abi@1", toolchain_identity: "fixture", source_commit: "fixture" },
    implementations: implementations.map((id) => ({ id, revision: 1, artifact: "browser-runtime-superset.wasm" })),
    boot_module: { role: "profile-gated-boot", path: "browser-boot-profile.mjs", sha256: bootModuleDigest },
    files: [
      { path: "runtime.wasm", bytes: runtimeBytes.byteLength, sha256: runtimeDigest, media_type: "application/wasm" },
      { path: "browser-boot-profile.mjs", bytes: 1, sha256: bootModuleDigest, media_type: "text/javascript" },
    ],
  };
  const imageHash = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(JSON.stringify(payload))));
  const id = `image:sha256:${Array.from(imageHash, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
  return { id, profileId: payload.profile_id, bytes: new TextEncoder().encode(JSON.stringify({ ...payload, image_id: id })), runtimeBytes, bootModuleDigest };
}

function digest(character) { return `sha256:${character.repeat(64)}`; }
async function hash(bytes) {
  const value = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(value, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}
