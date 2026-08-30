import { spawn } from "node:child_process";
import { expect, test } from "@playwright/test";

const entrances = [];

async function startEntrance() {
  const child = spawn("target/debug/conduit-browser-host", ["--no-open"], {
    cwd: new URL("../..", import.meta.url).pathname,
    stdio: ["ignore", "pipe", "pipe"],
  });
  entrances.push(child);
  let output = "";
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`browser Host was not ready\n${output}`)), 10_000);
    const inspect = (chunk) => {
      output += chunk.toString();
      const match = output.match(/CONDUIT_BROWSER_HOST_URL=(http:\/\/127\.0\.0\.1:\d+\/)/);
      if (match) {
        clearTimeout(timeout);
        resolve(match[1]);
      }
    };
    child.stdout.on("data", inspect);
    child.stderr.on("data", inspect);
    child.once("exit", (code) => reject(new Error(`browser Host exited ${code}\n${output}`)));
  });
}

async function installPicoboot(page, staleStatus = false) {
  await page.addInitScript(({ staleStatus }) => {
    class FakePicobootDevice {
      constructor() {
        this.vendorId = 0x2e8a;
        this.productId = 0x0003;
        this.configuration = null;
        this.commands = [];
        this.dataWrites = [];
        this.controlIn = [];
        this.controlOut = [];
        this.pending = null;
      }
      async open() {}
      async selectConfiguration(value) { this.configuration = { configurationValue: value }; }
      async claimInterface(number) { this.claimedInterface = number; }
      async releaseInterface() {}
      async close() {}
      async transferOut(endpoint, bytes) {
        const value = new Uint8Array(bytes);
        const view = new DataView(value.buffer, value.byteOffset, value.byteLength);
        if (value.byteLength === 32 && view.getUint32(0, true) === 0x431fd10b) {
          this.pending = { token: view.getUint32(4, true), command: view.getUint8(8) };
          this.commands.push({
            endpoint,
            token: this.pending.token,
            command: this.pending.command,
            transferBytes: view.getUint32(12, true),
            arguments: Array.from(value.subarray(16)),
          });
        } else {
          this.dataWrites.push({ endpoint, bytes: Array.from(value) });
        }
        return { status: "ok", bytesWritten: value.byteLength };
      }
      async transferIn(endpoint) {
        this.ackEndpoint = endpoint;
        return { status: "ok", data: new DataView(new ArrayBuffer(0)) };
      }
      async controlTransferOut(setup, bytes) {
        this.controlOut.push({ setup, bytes: Array.from(bytes) });
        return { status: "ok", bytesWritten: bytes.byteLength };
      }
      async controlTransferIn(setup, length) {
        this.controlIn.push({ setup, length });
        const bytes = new Uint8Array(16);
        const view = new DataView(bytes.buffer);
        view.setUint32(0, this.pending.token + (staleStatus ? 1 : 0), true);
        view.setUint32(4, 0, true);
        view.setUint8(8, this.pending.command);
        return { status: "ok", data: new DataView(bytes.buffer) };
      }
    }
    const device = new FakePicobootDevice();
    const usb = new EventTarget();
    usb.requestDevice = async () => device;
    Object.defineProperty(navigator, "usb", { configurable: true, value: usb });
    globalThis.__fakePicoboot = device;
  }, { staleStatus });
}

async function installRunningPico(page, buildId = "pico-build/accepted") {
  await page.addInitScript(({ buildId }) => {
    const encodeFrame = (text) => {
      const payload = new TextEncoder().encode(text);
      const bytes = new Uint8Array(payload.length + 2);
      new DataView(bytes.buffer).setUint16(0, payload.length, false);
      bytes.set(payload, 2);
      return bytes;
    };
    const responses = [
      encodeFrame(`CONDUIT_BOOTSEL_CHALLENGE@1:${buildId}`),
      encodeFrame("CONDUIT_REBOOT_BOOTSEL_ACK@1"),
    ];
    const port = new EventTarget();
    port.writes = [];
    port.open = async () => {};
    port.close = async () => {};
    port.getInfo = () => ({ usbVendorId: 0x2e8a, usbProductId: 0x000a });
    port.writable = {
      getWriter: () => ({
        write: async (bytes) => port.writes.push(Array.from(bytes)),
        releaseLock() {},
      }),
    };
    port.readable = {
      getReader: () => ({
        read: async () => ({ value: responses.shift(), done: false }),
        releaseLock() {},
      }),
    };
    Object.defineProperty(navigator, "serial", {
      configurable: true,
      value: { requestPort: async () => port },
    });
    globalThis.__fakeRunningPico = port;
  }, { buildId });
}

test.afterEach(() => {
  while (entrances.length > 0) entrances.pop().kill();
});

test("browser serial observes a distinct fresh Boot and invitation-bound Pico join", async ({ page }) => {
  await page.goto(await startEntrance());
  const result = await page.evaluate(async () => {
    const { requestRp2040SpawnJoin } = await import(
      "/targets/rp2040/browser-deployment/index.mjs"
    );
    const encode = (value) => {
      const payload = new TextEncoder().encode(JSON.stringify(value));
      const bytes = new Uint8Array(payload.length + 2);
      new DataView(bytes.buffer).setUint16(0, payload.length, false);
      bytes.set(payload, 2);
      return bytes;
    };
    const advertisement = {
      host_id: "pico/tour",
      boot_id: "pico-boot/fresh",
      offer_generation: 4,
      capabilities: [{ implementation_id: "pico/signal-source@1" }],
    };
    const nonce = Array(32).fill(7);
    const responses = [
      encode({ protocol: 1, advertisement, friendly_label: "Pico W", verifying_key: Array(32).fill(1), freshness_sequence: 1 }),
      encode({
        protocol: 2,
        spore_id: "spore:one",
        image_id: "image:one",
        invitation_id: "invitation:one",
        body_id: "body:one",
        host_id: advertisement.host_id,
        boot_id: advertisement.boot_id,
        offer_generation: advertisement.offer_generation,
        nonce,
        signature: Array(64).fill(9),
      }),
    ];
    const evidence = { schema: "conduit.browser/serial-base-evidence@1", phase: "resource-truth" };
    const base = {
      writes: [],
      usePlanId: null,
      evidence: () => evidence,
      startUse(planId) { this.usePlanId = planId; },
      async write(bytes) { this.writes.push(Array.from(bytes)); return { bytes }; },
      async read() { return { bytes: responses.shift() }; },
    };
    const secret = Array(32).fill(8);
    const observation = await requestRp2040SpawnJoin({
      base,
      prepared: {
        spore_id: "spore:one",
        image_id: "image:one",
        invitation_id: "invitation:one",
        body_id: "body:one",
        invitation_nonce: nonce,
        invitation_secret: secret,
        invitation_expires_at_millis: Date.now() + 60_000,
      },
    });
    return { observation, usePlanId: base.usePlanId, write: base.writes[0], secret };
  });
  expect(result.observation).toMatchObject({
    schema: "conduit.rp2040/browser-spawn-observation@1",
    spore_id: "spore:one",
    image_id: "image:one",
    host_id: "pico/tour",
    boot_id: "pico-boot/fresh",
  });
  expect(result.observation.advertisement.capabilities).toHaveLength(1);
  expect(result.usePlanId).toBe("pico-spawn/spore:one");
  expect(result.write.length).toBeLessThanOrEqual(4098);
  expect(result.secret).toEqual(Array(32).fill(0));
});

test("expired invitation and join-to-advertisement mismatch refuse before admission", async ({ page }) => {
  await page.goto(await startEntrance());
  const result = await page.evaluate(async () => {
    const { requestRp2040SpawnJoin } = await import(
      "/targets/rp2040/browser-deployment/index.mjs"
    );
    const prepared = (expiry) => ({
      spore_id: "spore:one", image_id: "image:one", invitation_id: "invitation:one",
      body_id: "body:one", invitation_nonce: Array(32).fill(7),
      invitation_secret: Array(32).fill(8), invitation_expires_at_millis: expiry,
    });
    const encode = (value) => {
      const payload = new TextEncoder().encode(JSON.stringify(value));
      const bytes = new Uint8Array(payload.length + 2);
      new DataView(bytes.buffer).setUint16(0, payload.length, false);
      bytes.set(payload, 2);
      return bytes;
    };
    const base = (responses = []) => ({
      writes: 0, evidence: () => ({}), startUse() {},
      async write() { this.writes += 1; },
      async read() { return { bytes: responses.shift() }; },
    });
    const expired = prepared(Date.now() - 1);
    const expiredBase = base();
    let expiredCode;
    try { await requestRp2040SpawnJoin({ base: expiredBase, prepared: expired }); }
    catch (error) { expiredCode = error.code; }

    const advertisement = { host_id: "pico/one", boot_id: "boot/fresh", offer_generation: 1 };
    const responses = [
      encode({ protocol: 1, advertisement }),
      encode({ protocol: 2, spore_id: "spore:one", image_id: "image:one",
        invitation_id: "invitation:one", body_id: "body:one", host_id: "pico/one",
        boot_id: "boot/stale", offer_generation: 1, nonce: Array(32).fill(7), signature: Array(64).fill(9) }),
    ];
    const mismatchBase = base(responses);
    let mismatchCode;
    try {
      await requestRp2040SpawnJoin({ base: mismatchBase, prepared: prepared(Date.now() + 60_000) });
    } catch (error) { mismatchCode = error.code; }
    return { expiredCode, expiredWrites: expiredBase.writes, expiredSecret: expired.invitation_secret,
      mismatchCode, mismatchWrites: mismatchBase.writes };
  });
  expect(result).toEqual({
    expiredCode: "ExpiredInvitation",
    expiredWrites: 0,
    expiredSecret: Array(32).fill(0),
    mismatchCode: "WrongBoot",
    mismatchWrites: 1,
  });
});

test("exact RP2040 UF2 deploys through one finite WebUSB Base without runtime promotion", async ({ page }) => {
  await installPicoboot(page);
  await page.goto(await startEntrance());
  const result = await page.evaluate(async () => {
    const { createRp2040BrowserDeploymentAdapter, RP2040_BROWSER_DEPLOYMENT } = await import(
      "/targets/rp2040/browser-deployment/index.mjs"
    );
    const blockCount = 2;
    const uf2 = new Uint8Array(blockCount * 512);
    for (let block = 0; block < blockCount; block += 1) {
      const offset = block * 512;
      const view = new DataView(uf2.buffer, offset, 512);
      view.setUint32(0, 0x0a324655, true);
      view.setUint32(4, 0x9e5d5157, true);
      view.setUint32(8, 0x2000, true);
      view.setUint32(12, 0x10000000 + block * 256, true);
      view.setUint32(16, 256, true);
      view.setUint32(20, block, true);
      view.setUint32(24, blockCount, true);
      view.setUint32(28, 0xe48bff56, true);
      uf2.fill(block + 1, offset + 32, offset + 288);
      view.setUint32(508, 0x0ab16f30, true);
    }
    const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", uf2));
    const contentId = `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
    const base = await __conduitBrowserHost.usbDevices.acquireUsb({
      configurationValue: RP2040_BROWSER_DEPLOYMENT.configurationValue,
      interfaceNumber: RP2040_BROWSER_DEPLOYMENT.interfaceNumber,
      alternateSetting: RP2040_BROWSER_DEPLOYMENT.alternateSetting,
      inEndpoint: RP2040_BROWSER_DEPLOYMENT.inEndpoint,
      outEndpoint: RP2040_BROWSER_DEPLOYMENT.outEndpoint,
      maximumTransferBytes: RP2040_BROWSER_DEPLOYMENT.maximumTransferBytes,
      maximumInTransfers: 32,
      maximumOutTransfers: 32,
    });
    const adapter = createRp2040BrowserDeploymentAdapter({ base });
    const plan = await adapter.sealDeployment({
      deploymentPlanId: "rp2040-deployment-plan/one",
      deploymentOperationId: "rp2040-deployment/one",
      targetId: RP2040_BROWSER_DEPLOYMENT.targetId,
      sporeId: "spore/pico-w/one",
      imageId: "image/pico-w-signal/one",
      imageContentId: contentId,
      imageBytes: uf2,
      explicitAction: true,
    });
    const receipt = await adapter.deploy(plan);
    const activeBase = base.evidence();
    const disconnect = new Event("disconnect");
    Object.defineProperty(disconnect, "device", { value: __fakePicoboot });
    navigator.usb.dispatchEvent(disconnect);
    const resourceEvidence = adapter.observeResourceTerminal();
    return {
      plan,
      receipt,
      activeBase,
      terminalBase: base.evidence(),
      resourceEvidence,
      commands: __fakePicoboot.commands,
      dataWrites: __fakePicoboot.dataWrites,
      controlIn: __fakePicoboot.controlIn,
      controlOut: __fakePicoboot.controlOut,
    };
  });

  expect(result.plan).toMatchObject({
    schema: "conduit.rp2040/browser-deployment-plan@1",
    targetId: "conduitos/thumbv6m/pico-w",
    sporeId: "spore/pico-w/one",
    imageBytes: 512,
    chunkCount: 1,
    requiredInTransfers: 12,
    requiredOutTransfers: 8,
  });
  expect(result.receipt).toMatchObject({
    schema: "conduit.rp2040/browser-deployment-receipt@1",
    terminal: "RebootRequested",
    admitted_commands: 6,
    admitted_image_bytes: 512,
    completed_image_bytes: 512,
    reboot_requested: true,
    runtime_truth_created: false,
  });
  expect(result.commands.map(({ command }) => command)).toEqual([1, 6, 3, 5, 7, 2]);
  expect(result.dataWrites).toHaveLength(1);
  expect(result.dataWrites[0].bytes).toHaveLength(512);
  expect(result.controlOut[0].setup).toMatchObject({ request: 0x41, recipient: "interface", index: 1 });
  expect(result.controlIn).toHaveLength(6);
  expect(result.activeBase).toMatchObject({
    phase: "usb-use-playing",
    admitted_in_transfers: 12,
    admitted_out_transfers: 8,
    use_plan_id: "rp2040-deployment-plan/one",
  });
  expect(result.terminalBase).toMatchObject({ phase: "terminal", terminal: "DeviceLost" });
  expect(result.resourceEvidence).toMatchObject({
    terminal: "RebootRequested",
    resource_terminal: "DeviceLost",
  });
});

test("running accepted firmware acknowledges a build-bound BOOTSEL reboot through Web Serial", async ({ page }) => {
  await installRunningPico(page);
  await page.goto(await startEntrance());
  const result = await page.evaluate(async () => {
    const { requestRunningFirmwareBootsel } = await import(
      "/targets/rp2040/browser-deployment/index.mjs"
    );
    const base = await __conduitBrowserHost.devices.acquireSerial();
    const receipt = await requestRunningFirmwareBootsel({
      base,
      usePlanId: "rp2040-bootsel-use-plan/one",
      operationId: "rp2040-bootsel-reboot/one",
      expectedBuildId: "pico-build/accepted",
      explicitAction: true,
    });
    return { receipt, base: base.evidence(), writes: __fakeRunningPico.writes };
  });
  expect(result.receipt).toMatchObject({
    schema: "conduit.rp2040/browser-bootsel-reboot-receipt@1",
    running_build_id: "pico-build/accepted",
    terminal: "RebootRequested",
    bootsel_resource_observed: false,
    runtime_truth_created: false,
  });
  expect(result.writes).toHaveLength(2);
  expect(result.base).toMatchObject({
    phase: "serial-use-playing",
    admitted_reads: 2,
    admitted_writes: 2,
  });
});

test("wrong IMAGE family and stale command status refuse without deployment success", async ({ page }) => {
  await installPicoboot(page, true);
  await page.goto(await startEntrance());
  const result = await page.evaluate(async () => {
    const { createRp2040BrowserDeploymentAdapter, RP2040_BROWSER_DEPLOYMENT } = await import(
      "/targets/rp2040/browser-deployment/index.mjs"
    );
    const makeUf2 = (family) => {
      const bytes = new Uint8Array(512);
      const view = new DataView(bytes.buffer);
      for (const [offset, word] of [[0, 0x0a324655], [4, 0x9e5d5157], [8, 0x2000],
        [12, 0x10000000], [16, 256], [20, 0], [24, 1], [28, family], [508, 0x0ab16f30]]) {
        view.setUint32(offset, word, true);
      }
      return bytes;
    };
    const base = await __conduitBrowserHost.usbDevices.acquireUsb({
      interfaceNumber: 1,
      maximumInTransfers: 32,
      maximumOutTransfers: 32,
    });
    const wrong = makeUf2(0x12345678);
    const wrongDigest = new Uint8Array(await crypto.subtle.digest("SHA-256", wrong));
    const contentId = (digest) => `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
    const first = createRp2040BrowserDeploymentAdapter({ base });
    let familyCode = null;
    try {
      await first.sealDeployment({
        deploymentPlanId: "p/family",
        deploymentOperationId: "o/family",
        targetId: RP2040_BROWSER_DEPLOYMENT.targetId,
        sporeId: "spore/wrong",
        imageId: "image/wrong",
        imageContentId: contentId(wrongDigest),
        imageBytes: wrong,
        explicitAction: true,
      });
    } catch (error) { familyCode = error.code; }

    const good = makeUf2(0xe48bff56);
    const goodDigest = new Uint8Array(await crypto.subtle.digest("SHA-256", good));
    const second = createRp2040BrowserDeploymentAdapter({ base });
    const plan = await second.sealDeployment({
      deploymentPlanId: "p/stale",
      deploymentOperationId: "o/stale",
      targetId: RP2040_BROWSER_DEPLOYMENT.targetId,
      sporeId: "spore/stale-status",
      imageId: "image/good",
      imageContentId: contentId(goodDigest),
      imageBytes: good,
      explicitAction: true,
    });
    let statusCode = null;
    try { await second.deploy(plan); } catch (error) { statusCode = error.code; }
    return {
      familyCode,
      statusCode,
      firstEvidence: first.evidence(),
      secondEvidence: second.evidence(),
      commands: __fakePicoboot.commands,
    };
  });
  expect(result.familyCode).toBe("ImageCompatibility");
  expect(result.firstEvidence.phase).toBe("available");
  expect(result.statusCode).toBe("StaleStatus");
  expect(result.secondEvidence).toMatchObject({ phase: "terminal", terminal: "StaleStatus", reboot_requested: false });
  expect(result.commands).toHaveLength(1);
});
