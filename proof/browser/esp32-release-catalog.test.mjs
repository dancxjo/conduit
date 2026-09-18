import assert from "node:assert/strict";
import test from "node:test";
import { acquireEsp32Release } from "../../targets/esp32/deployment/browser/release.mjs";

test("ESP32 Crèche resolves only the selected reviewed catalog release before Body binding", async () => {
  const segment = new Uint8Array(24);
  segment[0] = 0xe9;
  new DataView(segment.buffer).setUint16(12, 5, true);
  const digest = await sha256(segment);
  const manifest = {
    schema: "conduit.release/target-artifact@1",
    target_id: "esp32/riscv32imc/usb-dcf8355d-esp32-c3",
    source_identity: "git:reviewed",
    image_id: "image/esp32-c3/reviewed",
    segments: [{ path: "firmware.bin", offset: 0, bytes: segment.byteLength, sha256: digest }],
    bytes: segment.byteLength,
    artifact_sha256: digest,
    artifact_layout: { format: "espressif-segments" },
  };
  let resolvedProfile;
  let acquired = 0;
  const profile = {
    target: { id: manifest.target_id },
    target_id: manifest.target_id,
    package_id: "conduit-host-esp32@1",
    output: "esp32-image",
    builder_adapter: "conduit-host-esp32/build-c3-image@1",
    deployment_adapter: "conduit-host-esp32/flash-c3@1",
  };
  const resolveReviewedRelease = async (selected) => {
      resolvedProfile = selected;
      return Object.freeze({
        manifest: new TextEncoder().encode(JSON.stringify(manifest)),
        async acquire(descriptor, maximumBytes) {
          acquired += 1;
          assert.equal(descriptor.path, "firmware.bin");
          assert.ok(maximumBytes >= segment.byteLength);
          return segment.slice();
        },
      });
  };
  const signal = new AbortController().signal;
  const release = await acquireEsp32Release(profile, signal, {
    resolved: await resolveReviewedRelease(profile),
  });

  assert.equal(resolvedProfile.target_id, manifest.target_id);
  assert.equal(resolvedProfile.package_id, "conduit-host-esp32@1");
  assert.equal(resolvedProfile.output, "esp32-image");
  assert.equal(resolvedProfile.builder_adapter, "conduit-host-esp32/build-c3-image@1");
  assert.equal(resolvedProfile.deployment_adapter, "conduit-host-esp32/flash-c3@1");
  assert.equal(acquired, 1);
  assert.equal(release.manifest.artifact_sha256, digest);
  assert.deepEqual(release.segments[0].bytes, segment);
});

async function sha256(bytes) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}
