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

async function waitForBrowserHost(page) {
  await page.waitForFunction(() => globalThis.__conduitBrowserHost?.devices && globalThis.__conduitBrowserHost?.usbDevices);
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

async function installFragmentedSpawnSerial(page) {
  await page.addInitScript(() => {
    const encode = (value) => {
      const payload = new TextEncoder().encode(JSON.stringify(value));
      const bytes = new Uint8Array(payload.length + 2);
      new DataView(bytes.buffer).setUint16(0, payload.length, false);
      bytes.set(payload, 2);
      return bytes;
    };
    const split = (bytes, lengths) => {
      const chunks = [];
      let offset = 0;
      for (const length of lengths) {
        chunks.push(bytes.slice(offset, offset + length));
        offset += length;
      }
      chunks.push(bytes.slice(offset));
      return chunks;
    };
    const advertisement = {
      host_id: "pico/tour",
      boot_id: "pico-boot/fresh",
      offer_generation: 4,
      capabilities: [{ implementation_id: "pico/signal-source@1" }],
    };
    const nonce = Array(32).fill(7);
    const chunks = [
      ...split(encode({
        protocol: 1,
        advertisement,
        friendly_label: "Pico W",
        verifying_key: Array(32).fill(1),
        freshness_sequence: 1,
      }), [1, 3]),
      ...split(encode({
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
      }), [2, 1, 7]),
    ];
    const port = new EventTarget();
    port.reads = 0;
    port.writes = [];
    port.signals = [];
    port.open = async () => {};
    port.close = async () => {};
    port.getInfo = () => ({ usbVendorId: 0x2e8a, usbProductId: 0x000a });
    port.setSignals = async (signals) => port.signals.push({ ...signals });
    port.writable = {
      getWriter: () => ({
        write: async (bytes) => port.writes.push(Array.from(bytes)),
        releaseLock() {},
      }),
    };
    port.readable = {
      getReader: () => ({
        read: async () => {
          port.reads += 1;
          const value = chunks.shift();
          return { value, done: !value };
        },
        releaseLock() {},
      }),
    };
    Object.defineProperty(navigator, "serial", {
      configurable: true,
      value: { requestPort: async () => port },
    });
    globalThis.__fragmentedSpawnPort = port;
  });
}

test.afterEach(() => {
  while (entrances.length > 0) entrances.pop().kill();
});

test("target-owned fabrication returns exact attributable bytes through two local strategies", async ({ page }) => {
  await page.goto(`${await startEntrance()}creche/`);
  const result = await page.evaluate(async () => {
    const { createRp2040BrowserFabricationAdapter } = await import(
      "/targets/rp2040/deployment/browser/index.mjs"
    );
    const adapter = createRp2040BrowserFabricationAdapter();
    const selection = {
      targetId: "conduit-target/rp2040-pico-w@1",
      profileId: "pico-local",
      buildId: "conduit-pico-w-signal:4ccd179a7ddf32c17ba8b7f948a1f528e6cf8d78:thumbv6m-none-eabi:release:pico-local",
      imageId: "conduit-image/pico-w-signal-b7@1",
      manifestPath: "/creche/artifacts/pico-w-signal-pico-local.json",
    };
    const packaged = await adapter.fabricate({ strategy: "packaged-exact", selection, configuration: {} });
    const specialized = await adapter.fabricate({
      strategy: "template-specialized",
      selection,
      configuration: { body_label: "field-kit" },
    });
    let unsupported;
    try { await adapter.fabricate({ strategy: "browser-built", selection, configuration: {} }); }
    catch (error) { unsupported = { code: error.code, message: error.message }; }
    let oversized;
    try {
      await adapter.fabricate({
        strategy: "template-specialized", selection,
        configuration: { value: "x".repeat(193) },
      });
    } catch (error) { oversized = { code: error.code, message: error.message }; }
    return {
      packaged: { ...packaged, bytes: packaged.bytes.length },
      specialized: { ...specialized, bytes: specialized.bytes.length },
      unsupported,
      oversized,
    };
  });
  expect(result.packaged).toMatchObject({
    schema: "conduit.rp2040/browser-fabrication-result@1",
    strategy: "packaged-exact",
    bytes: 775168,
    content_id: "sha256:11e92a00aa1e1144faacfd25540426e57dd862b172595ef9197da02daf17ef8e",
    maximum_artifact_bytes: 2097152,
    provenance: {
      mechanism: "packaged-exact",
      artifact_id: "conduit-pico-w-signal/pico-local-b8@1",
      remote_builder: null,
      uploaded_artifact: null,
      cache_fallback: null,
    },
  });
  expect(result.specialized.strategy).toBe("template-specialized");
  expect(result.specialized.bytes).toBe(result.packaged.bytes + 512);
  expect(result.specialized.content_id).not.toBe(result.packaged.content_id);
  expect(result.specialized.selection_binding).not.toBe(result.packaged.selection_binding);
  expect(result.specialized.provenance.template_content_id).toBe(result.packaged.content_id);
  expect(result.unsupported.code).toBe("UnsupportedStrategy");
  expect(result.oversized.code).toBe("ConfigurationBound");
});

test("one reviewed IMAGE yields distinct directly plantable Body-bound UF2 spores", async ({ page }) => {
  await page.goto(`${await startEntrance()}creche/`);
  const result = await page.evaluate(async () => {
    const {
      bindRp2040BodySpore,
      createRp2040BrowserFabricationAdapter,
      readRp2040BodySpore,
    } = await import("/targets/rp2040/deployment/browser/index.mjs");
    const image = await createRp2040BrowserFabricationAdapter().fabricate({
      strategy: "packaged-exact",
      selection: {
        targetId: "conduit-target/rp2040-pico-w@1",
        profileId: "pico-local",
        buildId: "conduit-pico-w-signal:4ccd179a7ddf32c17ba8b7f948a1f528e6cf8d78:thumbv6m-none-eabi:release:pico-local",
        imageId: "conduit-image/pico-w-signal-b7@1",
        manifestPath: "/creche/artifacts/pico-w-signal-pico-local.json",
      },
      configuration: {},
    });
    const prepared = (suffix) => ({
      output: "uf2",
      target_id: "conduitos/thumbv6m/pico-w",
      spore_id: `spore:${suffix}`,
      image_id: "image:reviewed",
      image_content_digest: image.content_id,
      invitation_id: `invitation:${suffix}`,
      body_id: `body:${suffix}`,
      invitation_nonce: Array(32).fill(suffix.charCodeAt(0)),
      invitation_expires_at_millis: Date.now() + 60_000,
      invitation_secret: Array(32).fill(suffix.charCodeAt(0) + 1),
    });
    const first = await bindRp2040BodySpore(image.bytes, prepared("a"));
    const second = await bindRp2040BodySpore(image.bytes, prepared("b"));
    const recovered = readRp2040BodySpore(first.bytes);
    let missing;
    try { readRp2040BodySpore(image.bytes); } catch (error) { missing = error.code; }
    return {
      imageBytes: image.bytes.byteLength,
      first: { ...first, bytes: first.bytes.byteLength },
      second: { ...second, bytes: second.bytes.byteLength },
      recovered,
      missing,
    };
  });
  expect(result.first).toMatchObject({
    schema: "conduit.rp2040/native-body-spore@1",
    format: "uf2",
    image_content_id: "sha256:11e92a00aa1e1144faacfd25540426e57dd862b172595ef9197da02daf17ef8e",
    spore_id: "spore:a",
    bootstrap_flash_address: 0x101ff000,
  });
  expect(result.first.bytes).toBe(result.imageBytes + 16 * 512);
  expect(result.first.content_id).not.toBe(result.second.content_id);
  expect(result.recovered).toMatchObject({
    protocol: 2,
    spore_id: "spore:a",
    image_id: "image:reviewed",
    invitation_id: "invitation:a",
    body_id: "body:a",
  });
  expect(result.missing).toBe("SporeMissing");
});

test("browser serial observes a distinct fresh Boot and invitation-bound Pico join", async ({ page }) => {
  await installFragmentedSpawnSerial(page);
  await page.goto(await startEntrance());
  await waitForBrowserHost(page);
  const result = await page.evaluate(async () => {
    const { PHYSICAL_SPAWN_STREAM_BOUNDS, requestRp2040SpawnJoin } = await import(
      "/targets/rp2040/deployment/browser/index.mjs"
    );
    const nonce = Array(32).fill(7);
    const base = await globalThis.__conduitBrowserHost.devices.acquireSerial({
      maximumTransferBytes: PHYSICAL_SPAWN_STREAM_BOUNDS.maximumTransferBytes,
      maximumReads: PHYSICAL_SPAWN_STREAM_BOUNDS.maximumReads,
      maximumWrites: PHYSICAL_SPAWN_STREAM_BOUNDS.maximumWrites,
      maximumSignalOperations: PHYSICAL_SPAWN_STREAM_BOUNDS.maximumSignalOperations,
    });
    const observation = await requestRp2040SpawnJoin({
      base,
      prepared: {
        spore_id: "spore:one",
        image_id: "image:one",
        invitation_id: "invitation:one",
        body_id: "body:one",
        invitation_nonce: nonce,
        invitation_expires_at_millis: Date.now() + 60_000,
      },
    });
    return {
      observation,
      base: base.evidence(),
      browserReads: __fragmentedSpawnPort.reads,
      signals: __fragmentedSpawnPort.signals,
      write: __fragmentedSpawnPort.writes[0],
    };
  });
  expect(result.observation).toMatchObject({
    schema: "conduit.rp2040/browser-spawn-observation@1",
    spore_id: "spore:one",
    image_id: "image:one",
    host_id: "pico/tour",
    boot_id: "pico-boot/fresh",
  });
  expect(result.observation.advertisement.capabilities).toHaveLength(1);
  expect(result.observation.serial_use_plan_id).toBe("pico-spawn/spore:one");
  expect(result.observation.serial_stream).toEqual({
    response_frames: 2,
    browser_chunks: 7,
    admitted_reads: 2,
    maximum_read_bytes: 4096,
    maximum_total_response_bytes: 8192,
    maximum_chunks_per_read: 4096,
    maximum_read_millis: 10000,
  });
  expect(result.base).toMatchObject({
    phase: "serial-use-playing",
    admitted_reads: 2,
    admitted_writes: 1,
    admitted_signal_operations: 1,
    retained_bytes: 0,
    transfer_bounds: {
      maximum_transfer_bytes: 4096,
      maximum_reads: 2,
      maximum_writes: 1,
      maximum_signal_operations: 1,
    },
  });
  expect(result.browserReads).toBe(7);
  expect(result.signals).toEqual([{ dataTerminalReady: true }]);
  expect(result.write.length).toBeLessThanOrEqual(4098);
  const requestLength = (result.write[0] << 8) | result.write[1];
  expect(new TextDecoder().decode(Uint8Array.from(result.write.slice(2, 2 + requestLength))))
    .toBe("CONDUIT_SPORE_JOIN@1");
});

test("expired invitation and join-to-advertisement mismatch refuse before admission", async ({ page }) => {
  await page.goto(await startEntrance());
  await waitForBrowserHost(page);
  const result = await page.evaluate(async () => {
    const { requestRp2040SpawnJoin } = await import(
      "/targets/rp2040/deployment/browser/index.mjs"
    );
    const prepared = (expiry) => ({
      spore_id: "spore:one", image_id: "image:one", invitation_id: "invitation:one",
      body_id: "body:one", invitation_nonce: Array(32).fill(7),
      invitation_expires_at_millis: expiry,
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
      async setSignals() {},
      async write() { this.writes += 1; },
      async readStream({ complete }) {
        const bytes = responses.shift();
        if (!complete(bytes)) throw new Error("fixture response did not complete one frame");
        return { bytes, chunks: 1 };
      },
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
    return { expiredCode, expiredWrites: expiredBase.writes, mismatchCode, mismatchWrites: mismatchBase.writes };
  });
  expect(result).toEqual({
    expiredCode: "ExpiredInvitation",
    expiredWrites: 0,
    mismatchCode: "WrongBoot",
    mismatchWrites: 1,
  });
});

test("exact RP2040 UF2 deploys through one finite WebUSB Base without runtime promotion", async ({ page }) => {
  await installPicoboot(page);
  await page.goto(await startEntrance());
  await waitForBrowserHost(page);
  const result = await page.evaluate(async () => {
    const { createRp2040BrowserDeploymentAdapter, RP2040_BROWSER_DEPLOYMENT } = await import(
      "/targets/rp2040/deployment/browser/index.mjs"
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
      sporeContentId: contentId,
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
      acknowledgementEndpoint: __fakePicoboot.ackEndpoint,
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
  expect(result.commands.every(({ endpoint }) => endpoint === 3)).toBe(true);
  expect(result.dataWrites).toHaveLength(1);
  expect(result.dataWrites[0].endpoint).toBe(3);
  expect(result.dataWrites[0].bytes).toHaveLength(512);
  expect(result.acknowledgementEndpoint).toBe(4);
  expect(result.controlOut[0].setup).toMatchObject({ request: 0x41, recipient: "interface", index: 1 });
  expect(result.controlIn).toHaveLength(6);
  expect(result.activeBase).toMatchObject({
    phase: "usb-use-playing",
    admitted_in_transfers: 12,
    admitted_out_transfers: 8,
    use_plan_id: "rp2040-deployment-plan/one",
    configuration: { interface_number: 1, in_endpoint: 4, out_endpoint: 3 },
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
  await waitForBrowserHost(page);
  const result = await page.evaluate(async () => {
    const { requestRunningFirmwareBootsel } = await import(
      "/targets/rp2040/deployment/browser/index.mjs"
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
  await waitForBrowserHost(page);
  const result = await page.evaluate(async () => {
    const { createRp2040BrowserDeploymentAdapter, RP2040_BROWSER_DEPLOYMENT } = await import(
      "/targets/rp2040/deployment/browser/index.mjs"
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
      configurationValue: RP2040_BROWSER_DEPLOYMENT.configurationValue,
      interfaceNumber: RP2040_BROWSER_DEPLOYMENT.interfaceNumber,
      alternateSetting: RP2040_BROWSER_DEPLOYMENT.alternateSetting,
      inEndpoint: RP2040_BROWSER_DEPLOYMENT.inEndpoint,
      outEndpoint: RP2040_BROWSER_DEPLOYMENT.outEndpoint,
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
        sporeContentId: contentId(wrongDigest),
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
      sporeContentId: contentId(goodDigest),
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
