import assert from "node:assert/strict";
import test from "node:test";
import { webcrypto } from "node:crypto";
import { mkdtemp, mkdir, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { parseRp2040Uf2, RP2040_UF2, sha256ContentId } from "../../targets/rp2040/deployment/browser/uf2.mjs";
import { requiredPicobootTransfers } from "../../targets/rp2040/deployment/browser/picoboot.mjs";
import { createRp2040BrowserDeploymentAdapter } from "../../targets/rp2040/deployment/browser/deployment.mjs";

// CPU protocol/flash-model proof, not physical flash or browser acceptance.
function image(addresses) {
  const bytes = new Uint8Array(addresses.length * 512);
  addresses.forEach((address, index) => {
    const view = new DataView(bytes.buffer, index * 512, 512);
    for (const [offset, value] of [
      [0, 0x0a324655], [4, 0x9e5d5157], [8, 0x2000], [12, address],
      [16, 256], [20, index], [24, addresses.length], [28, 0xe48bff56], [508, 0x0ab16f30],
    ]) view.setUint32(offset, value, true);
    bytes.fill(index + 1, index * 512 + 32, index * 512 + 288);
  });
  return bytes;
}
const start = RP2040_UF2.flashStart;
const spore = RP2040_UF2.sporeSectorAddress;
const sparseImage = () => image([start, ...Array.from({ length: 16 }, (_, i) => spore + i * 256)]);

function baseFixture(bounds, afterErase = () => {}) {
  const flash = new Uint8Array(RP2040_UF2.flashEnd - start).fill(0x5a);
  const commands = [];
  let current;
  let payloadAddress;
  const calls = { in: 0, out: 0, starts: 0 };
  return {
    flash, commands, calls,
    evidence: () => ({
      schema: "conduit.browser/web-usb-base-evidence@1", phase: "resource-truth",
      resource_class: "conduit.resource/web-usb-device@1",
      base_implementation_id: "browser/web-usb@1",
      use_authority_contract: "conduit.authority/use-web-usb@1",
      host_id: "fixture-host", boot_id: "fixture-boot", resource_handle: "fixture-resource",
      base_instance_id: "fixture-base", use_authority_grant: "fixture-authority",
      vendor_id: 0x2e8a, product_id: 3,
      configuration: { configuration_value: 1, interface_number: 1, alternate_setting: 0, in_endpoint: 4, out_endpoint: 3 },
      admitted_in_transfers: 0, admitted_out_transfers: 0,
      transfer_bounds: { maximum_transfer_bytes: 4096, maximum_in_flight: 1,
        maximum_in_transfers: bounds.maximumInTransfers, maximum_out_transfers: bounds.maximumOutTransfers },
    }),
    startUse: () => { calls.starts += 1; },
    async transferOut(bytes) {
      calls.out += 1;
      assert.ok(calls.out <= bounds.maximumOutTransfers);
      if (payloadAddress !== undefined) {
        flash.set(bytes, payloadAddress - start);
        payloadAddress = undefined;
        return;
      }
      const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
      current = { token: view.getUint32(4, true), command: view.getUint8(8),
        address: view.getUint32(16, true), bytes: view.getUint32(20, true) };
      commands.push(current);
      if (current.command === 3) {
        flash.fill(0xff, current.address - start, current.address - start + current.bytes);
        afterErase(current);
      }
      if (current.command === 5) payloadAddress = current.address;
    },
    async transferIn() { calls.in += 1; assert.ok(calls.in <= bounds.maximumInTransfers); return { bytes: new Uint8Array() }; },
    async controlTransferOut() { calls.out += 1; assert.ok(calls.out <= bounds.maximumOutTransfers); },
    async controlTransferIn() {
      calls.in += 1;
      assert.ok(calls.in <= bounds.maximumInTransfers);
      const bytes = new Uint8Array(16);
      const view = new DataView(bytes.buffer);
      view.setUint32(0, current.token, true);
      view.setUint8(8, current.command);
      return { bytes };
    },
    async close() {},
  };
}

async function seal(adapter, bytes) {
  const id = await sha256ContentId(bytes, webcrypto);
  return adapter.sealDeployment({ deploymentPlanId: "fixture-plan", deploymentOperationId: "fixture-operation",
    targetId: "conduitos/thumbv6m/pico-w", sporeId: "fixture-spore", imageId: "fixture-image",
    imageContentId: id, sporeContentId: id, imageBytes: bytes, explicitAction: true });
}

test("sparse Body Spore deployment preserves every omitted sector with exact transfer custody", async () => {
  const bytes = sparseImage();
  const parsed = parseRp2040Uf2(bytes, 4096);
  assert.deepEqual(parsed.eraseRanges, [{ address: start, bytes: 4096 }, { address: spore, bytes: 4096 }]);
  assert.equal(parsed.erasedBytes, 8192);
  assert.ok(Object.isFrozen(parsed.eraseRanges) && parsed.eraseRanges.every(Object.isFrozen));
  const bounds = requiredPicobootTransfers(parsed.chunks.length, parsed.eraseRanges.length);
  const base = baseFixture(bounds);
  const adapter = createRp2040BrowserDeploymentAdapter({ base, cryptoApi: webcrypto });
  const plan = await seal(adapter, bytes);
  assert.deepEqual(plan.eraseRanges, parsed.eraseRanges);
  assert.equal(plan.erasedBytes, 8192);
  const receipt = await adapter.deploy(plan);
  assert.equal(receipt.admitted_commands, 8);
  assert.equal(receipt.runtime_truth_created, false);
  assert.deepEqual(base.calls, { starts: 1, in: bounds.maximumInTransfers, out: bounds.maximumOutTransfers });
  assert.ok(base.flash.subarray(4096, spore - start).every((byte) => byte === 0x5a));
  assert.ok(base.flash.subarray(0, 256).every((byte) => byte === 1));
  assert.ok(base.flash.subarray(256, 4096).every((byte) => byte === 0xff));
  assert.ok(base.flash.subarray(spore - start, spore - start + 256).every((byte) => byte === 2));
});

test("adjacent sectors merge; partial final sectors erase only their containing sector", () => {
  const bytes = image(Array.from({ length: 17 }, (_, i) => start + i * 256));
  const parsed = parseRp2040Uf2(bytes, 4096);
  assert.deepEqual(parsed.eraseRanges, [{ address: start, bytes: 8192 }]);
  assert.deepEqual(requiredPicobootTransfers(1, 1), { maximumOutTransfers: 8, maximumInTransfers: 12 });
});

test("old single-erase budget refuses sparse deployment before any device effect", async () => {
  for (const reduced of ["maximumInTransfers", "maximumOutTransfers"]) {
    const bounds = { ...requiredPicobootTransfers(2, 2), [reduced]: requiredPicobootTransfers(2, 1)[reduced] };
    const base = baseFixture(bounds);
    const adapter = createRp2040BrowserDeploymentAdapter({ base, cryptoApi: webcrypto });
    await assert.rejects(seal(adapter, sparseImage()), { code: "TransferBudget" });
    assert.deepEqual(base.calls, { starts: 0, in: 0, out: 0 });
    assert.ok(base.flash.every((byte) => byte === 0x5a));
  }
});

test("cancellation between erase ranges stops before the Spore erase or writes", async () => {
  const abort = new AbortController();
  const base = baseFixture(requiredPicobootTransfers(2, 2), () => abort.abort());
  const adapter = createRp2040BrowserDeploymentAdapter({ base, cryptoApi: webcrypto });
  const plan = await seal(adapter, sparseImage());
  await assert.rejects(adapter.deploy(plan, { signal: abort.signal }), { code: "Cancelled" });
  assert.deepEqual(base.commands.map(({ command }) => command), [1, 6, 3]);
  assert.ok(base.flash.subarray(4096).every((byte) => byte === 0x5a));
});

test("erase ranges do not widen the accepted sparse-image or command bounds", () => {
  assert.throws(() => parseRp2040Uf2(image([start, start + 8192]), 4096), { code: "SparseImage" });
  for (const count of [0, 3, 1.5, undefined]) {
    assert.throws(() => requiredPicobootTransfers(1, count), { code: "EraseRangeBound" });
  }
});


test("credential payloads refuse even when contiguous with the firmware image", () => {
  const credentials = RP2040_UF2.credentialsAddress;
  assert.equal(RP2040_UF2.credentialsBytes, 8192);
  for (const address of [credentials, spore - 256]) {
    assert.throws(() => parseRp2040Uf2(image([address]), 4096), { code: "ReservedFlash" });
  }
  const lastSector = credentials - 4096;
  assert.throws(() => parseRp2040Uf2(image(Array.from({ length: 17 }, (_, i) => lastSector + i * 256)), 4096),
    { code: "ReservedFlash" });
});

test("Workspace seals the UF2 parser and its exact shared layout dependency", async () => {
  const template = JSON.parse(await readFile("targets/browser/workspace/workspace.application.template.json", "utf8"));
  const resources = template.resources.filter(({ role }) => ["rp2040-uf2", "rp2040-flash-layout"].includes(role));
  assert.equal(resources.length, 2);
  const scratch = await mkdtemp(path.join(tmpdir(), "conduit-pico-layout-package-"));
  try {
    for (const resource of resources) {
      const bytes = await readFile(resource.path.replace("/browser-deployment/", "/deployment/browser/"));
      const destination = path.join(scratch, resource.path);
      await mkdir(path.dirname(destination), { recursive: true });
      await writeFile(destination, bytes);
    }
    const input = path.join(scratch, "template.json");
    await writeFile(input, JSON.stringify({ ...template, resources }));
    const producer = path.resolve("targets/browser/tools/build-browser-application-package.mjs");
    const built = spawnSync(process.execPath, [producer, input, scratch, "layout.application.json"], { encoding: "utf8" });
    assert.equal(built.status, 0, built.stderr);
    const manifest = JSON.parse(await readFile(path.join(scratch, "layout.application.json"), "utf8"));
    const data = manifest.resources.find(({ role }) => role === "rp2040-flash-layout");
    const bytes = await readFile("targets/rp2040/deployment/browser/flash-layout.mjs");
    assert.equal(data.sha256, await sha256ContentId(bytes, webcrypto));
    assert.equal(data.maximum_bytes, 1024);
    const missing = resources.map((resource) => resource.role === "rp2040-uf2" ? { ...resource, dependencies: [] } : resource);
    await writeFile(input, JSON.stringify({ ...template, resources: missing }));
    const refused = spawnSync(process.execPath, [producer, input, scratch, "refused.application.json"], { encoding: "utf8" });
    assert.notEqual(refused.status, 0);
    assert.match(refused.stderr, /imports undeclared module/);
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
});
