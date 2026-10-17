import assert from "node:assert/strict";
import test from "node:test";
import {
  ConduitSdkError,
  IncompatibleRuntimeAbiError,
  InvalidLifecycleError,
  PermissionDeniedError,
  PlanRefusalError,
  ResourceLossError,
} from "./browser-sdk.mjs";

test("typed lifecycle refusals preserve their category, operation, evidence, and exact identities", () => {
  const evidence = Object.freeze({ schema: "conduit.body/wake-refusal@1", reason: "resource pressure" });
  const identities = Object.freeze({ hostId: "host/one", bootId: "boot/two", bodyId: "body/three", planId: "plan/four" });
  const types = [
    [PlanRefusalError, "PlanRefusal"],
    [ResourceLossError, "ResourceLoss"],
    [InvalidLifecycleError, "InvalidLifecycle"],
    [PermissionDeniedError, "PermissionDenied"],
    [IncompatibleRuntimeAbiError, "IncompatibleRuntimeAbi"],
  ];
  for (const [Type, category] of types) {
    const error = new Type({ code: "exact-runtime-code", message: "refused", operation: "Body.wake", evidence, identities });
    assert(error instanceof ConduitSdkError);
    assert.equal(error.category, category);
    assert.equal(error.operation, "Body.wake");
    assert.equal(error.evidence, evidence);
    assert.deepEqual(error.identities, identities);
    assert(Object.isFrozen(error.identities));
  }
});

test("BrowserHost cannot be made outside the admitted Conduit entrance", async () => {
  const { BrowserHost, BrowserPlay } = await import("./browser-sdk.mjs");
  assert.throws(() => new BrowserHost(), /come from Conduit\.browser/);
  assert.throws(() => new BrowserPlay(), /admitted runtime start/);
});

test("external Body executor accepts saved choice, continuous PCM and Thermostat within its exact finite envelope", async () => {
  const { checkedBrowserExecutionCapabilities } = await import("./browser-sdk.mjs");
  const installed = Array.from({ length: 116 }, (_, index) => `capability/installed-${index}`);
  installed.push("browser-thermostat-combine-v1", "browser/thermostat-scan@1",
    "browser/kernel-saved-choice@1", "browser/webaudio-continuous-pcm@1");
  const disclosure = { schema: "conduit.browser/body-capabilities@1", capability_ids: installed };
  const accepted = checkedBrowserExecutionCapabilities(disclosure);
  assert.deepEqual(accepted, installed);
  assert(Object.isFrozen(accepted));
  assert.notEqual(accepted, installed);
  assert.equal(checkedBrowserExecutionCapabilities({ ...disclosure,
    capability_ids: [...installed, "capability/final-admitted-slot"] }).length, 121);
  for (const invalid of [
    { ...disclosure, capability_ids: [...installed, "capability/121", "capability/122"] },
    { ...disclosure, schema: "wrong" },
    { ...disclosure, capability_ids: {} },
    ...[null, "", "x".repeat(257)].map(identity => ({ ...disclosure,
      capability_ids: [...installed.slice(0, -1), identity] })),
  ]) {
    assert.throws(() => checkedBrowserExecutionCapabilities(invalid),
      /invalid external Body execution capabilities/);
  }
});

test("Browser SDK refuses URL-changing asset paths before requesting them", async () => {
  const { Conduit } = await import("./browser-sdk.mjs");
  const saved = Object.fromEntries(["document", "location", "fetch"].map(key => [key, Object.getOwnPropertyDescriptor(globalThis, key)]));
  const encoder = new TextEncoder();
  const hash = async value => `sha256:${Buffer.from(await crypto.subtle.digest("SHA-256", encoder.encode(value))).toString("hex")}`;
  try {
    globalThis.document = { baseURI: "https://fixture.invalid/app/" };
    globalThis.location = { origin: "https://fixture.invalid" };
    for (const path of ["https:other.mjs", "other.mjs?query", "other.mjs#fragment", "other\nmodule.mjs", "other\tmodule.mjs"]) {
      const file = { path, bytes: 1, sha256: `sha256:${"a".repeat(64)}`, media_type: "text/javascript" };
      const distributionDigest = await hash(`conduit.release/host-bundle-content@1\0${path}\0${file.sha256}\n`);
      const imageFile = { ...file, path: "conduit-browser-image.json" };
      const files = [file, imageFile];
      const bundleDigest = await hash(`conduit.release/host-bundle-content@1\0${files.map(item => `${item.path}\0${item.sha256}\n`).join("")}`);
      const pkg = { schema: "conduit.browser/sdk-package@1", runtime_abi: "conduit.browser/runtime-abi@1",
        package_id: "@conduit/browser", package_version: "0.1.0", target_id: "browser/wasm32/page",
        image_id: `image:sha256:${"b".repeat(64)}`, profile_id: `sha256:${"c".repeat(64)}`, bundle_sha256: bundleDigest, files };
      const distribution = { schema: "conduit.release/host-bundle@1", target_id: pkg.target_id,
        make_package_id: "browser-wasm@1", output: "browser-bundle", files: [file], bundle_sha256: distributionDigest,
        reviewed_distribution: { schema: "conduit.browser/reviewed-distribution@1", runtime_abi: pkg.runtime_abi, distribution_id: "fixture" } };
      const bundle = { ...distribution, files, bundle_sha256: bundleDigest, distribution_id: "fixture",
        distribution_sha256: distributionDigest, browser_image_id: pkg.image_id, browser_profile_id: pkg.profile_id };
      const image = { schema: "conduit.browser/bundle-image@1", image_id: pkg.image_id, profile_id: pkg.profile_id,
        reviewed_distribution: { runtime_abi: pkg.runtime_abi, distribution_digest: distributionDigest }, files: [file] };
      const assets = new Map([["browser-sdk-package.json", pkg], ["browser-page.json", distribution],
        ["browser-bundle-release.json", bundle], ["conduit-browser-image.json", image]]);
      const requested = [];
      globalThis.fetch = async url => {
        requested.push(url.href);
        const name = url.pathname.split("/").at(-1);
        assert(assets.has(name), `unexpected asset request ${url.href}`);
        return new Response(JSON.stringify(assets.get(name)));
      };
      await assert.rejects(Conduit.browser({ bundleRoot: "./bundle/" }), { code: "ArtifactBound" });
      assert(requested.every(url => !url.includes("other")));
    }
  } finally {
    for (const [key, descriptor] of Object.entries(saved)) {
      if (descriptor) Object.defineProperty(globalThis, key, descriptor);
      else delete globalThis[key];
    }
  }
});

test("Browser SDK cancels oversized manifest streams at the byte bound", async () => {
  const { Conduit } = await import("./browser-sdk.mjs");
  const saved = Object.fromEntries(["document", "location", "fetch"].map(key => [key, Object.getOwnPropertyDescriptor(globalThis, key)]));
  try {
    globalThis.document = { baseURI: "https://fixture.invalid/app/" };
    globalThis.location = { origin: "https://fixture.invalid" };
    for (const oversized of [true, false]) {
      const maximum = 128 * 1024;
      const bytes = new TextEncoder().encode('{}' + ' '.repeat(maximum - 2));
      let pulls = 0;
      let cancelled = false;
      const stream = new ReadableStream({
        pull(controller) {
          pulls++;
          if (pulls <= 2) controller.enqueue(bytes.slice((pulls - 1) * maximum / 2, pulls * maximum / 2));
          else if (oversized && pulls <= 4) controller.enqueue(new Uint8Array([32]));
          else controller.close();
        },
        cancel() { cancelled = true; },
      }, { highWaterMark: 0 });
      globalThis.fetch = async () => new Response(stream);
      await assert.rejects(Conduit.browser({ bundleRoot: './bundle/' }),
        { code: oversized ? 'ArtifactBound' : 'PackageManifestInvalid' });
      assert.equal(cancelled, oversized);
      assert.equal(pulls, 3);
      assert.equal(stream.locked, false);
    }
  } finally {
    for (const [key, descriptor] of Object.entries(saved)) {
      if (descriptor) Object.defineProperty(globalThis, key, descriptor);
      else delete globalThis[key];
    }
  }
});

test("Browser SDK verifies the exact IMAGE bytes it interpreted without refetching", async () => {
  const { Conduit } = await import("./browser-sdk.mjs");
  const saved = Object.fromEntries(["document", "location", "fetch"].map(key => [key, Object.getOwnPropertyDescriptor(globalThis, key)]));
  const encoder = new TextEncoder();
  const hash = async value => `sha256:${Buffer.from(await crypto.subtle.digest("SHA-256", encoder.encode(value))).toString("hex")}`;
  try {
    globalThis.document = { baseURI: "https://fixture.invalid/app/" };
    globalThis.location = { origin: "https://fixture.invalid" };
    for (const corrupt of [false, true]) {
      const distributionDigest = await hash('conduit.release/host-bundle-content@1\0');
      const image = { schema: "conduit.browser/bundle-image@1", image_id: `image:sha256:${"b".repeat(64)}`,
        profile_id: `sha256:${"c".repeat(64)}`, files: [],
        reviewed_distribution: { runtime_abi: "conduit.browser/runtime-abi@1", distribution_digest: distributionDigest } };
      const imageText = JSON.stringify(image);
      const file = { path: "conduit-browser-image.json", bytes: encoder.encode(imageText).length,
        sha256: corrupt ? `sha256:${"a".repeat(64)}` : await hash(imageText), media_type: "application/json" };
      const bundleDigest = await hash(`conduit.release/host-bundle-content@1\0${file.path}\0${file.sha256}\n`);
      const pkg = { schema: "conduit.browser/sdk-package@1", runtime_abi: image.reviewed_distribution.runtime_abi,
        package_id: "@conduit/browser", package_version: "0.1.0", target_id: "browser/wasm32/page",
        image_id: image.image_id, profile_id: image.profile_id, bundle_sha256: bundleDigest, files: [file] };
      const distribution = { schema: "conduit.release/host-bundle@1", target_id: pkg.target_id,
        make_package_id: "browser-wasm@1", output: "browser-bundle", files: [], bundle_sha256: distributionDigest,
        reviewed_distribution: { schema: "conduit.browser/reviewed-distribution@1", runtime_abi: pkg.runtime_abi, distribution_id: "fixture" } };
      const bundle = { ...distribution, files: [file], bundle_sha256: bundleDigest, distribution_id: "fixture",
        distribution_sha256: distributionDigest, browser_image_id: pkg.image_id, browser_profile_id: pkg.profile_id };
      const assets = new Map([["browser-sdk-package.json", pkg], ["browser-page.json", distribution],
        ["browser-bundle-release.json", bundle], [file.path, image]]);
      let imageRequests = 0;
      globalThis.fetch = async url => {
        const name = url.pathname.split('/').at(-1);
        if (name === file.path && ++imageRequests > 1) return new Response('changed after first read');
        return new Response(JSON.stringify(assets.get(name)));
      };
      // Valid IMAGE bytes advance to the intentionally absent runtime; bad
      // declared digests refuse those same retained bytes before that point.
      await assert.rejects(Conduit.browser({ bundleRoot: './bundle/' }),
        { code: corrupt ? 'ArtifactDigestMismatch' : 'ArtifactMissing' });
      assert.equal(imageRequests, 1);
    }
  } finally {
    for (const [key, descriptor] of Object.entries(saved)) {
      if (descriptor) Object.defineProperty(globalThis, key, descriptor);
      else delete globalThis[key];
    }
  }
});
