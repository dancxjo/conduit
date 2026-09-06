import { createHash } from "node:crypto";
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

async function installRomLoader(page, {
  chipMagic = 0x6921506f,
  expectedMd5,
  mismatchedMd5 = false,
  failSignals = false,
  failOperation = null,
} = {}) {
  await page.addInitScript(({ chipMagic, expectedMd5, mismatchedMd5, failSignals, failOperation }) => {
    const slipDecode = (encoded) => {
      const decoded = [];
      for (let index = 1; index < encoded.length - 1; index += 1) {
        if (encoded[index] === 0xdb) {
          index += 1;
          decoded.push(encoded[index] === 0xdc ? 0xc0 : 0xdb);
        } else decoded.push(encoded[index]);
      }
      return new Uint8Array(decoded);
    };
    const slipEncode = (packet) => {
      const encoded = [0xc0];
      for (const byte of packet) {
        if (byte === 0xc0) encoded.push(0xdb, 0xdc);
        else if (byte === 0xdb) encoded.push(0xdb, 0xdd);
        else encoded.push(byte);
      }
      encoded.push(0xc0);
      return new Uint8Array(encoded);
    };
    class FakeEsp32Port extends EventTarget {
      constructor() {
        super();
        this.responses = [];
        this.commands = [];
        this.signals = [];
        this.closed = false;
        this.readable = {
          getReader: () => ({
            read: async () => ({ value: this.responses.shift(), done: false }),
            releaseLock() {},
          }),
        };
        this.writable = {
          getWriter: () => ({
            write: async (encoded) => this.accept(encoded),
            releaseLock() {},
          }),
        };
      }
      async open(options) { this.opened = options; }
      getInfo() { return { usbVendorId: 0x303a, usbProductId: 0x1001 }; }
      async setSignals(signals) {
        if (failSignals) throw new DOMException("reset lines failed", "NetworkError");
        this.signals.push({ ...signals });
      }
      async close() { this.closed = true; }
      accept(encoded) {
        const packet = slipDecode(new Uint8Array(encoded));
        const view = new DataView(packet.buffer, packet.byteOffset, packet.byteLength);
        const operation = view.getUint8(1);
        if (operation === failOperation) throw new DOMException("serial transfer failed", "NetworkError");
        const size = view.getUint16(2, true);
        this.commands.push({ operation, size, checksum: view.getUint32(4, true), data: Array.from(packet.subarray(8)) });
        let value = 1;
        let data = new Uint8Array();
        if (operation === 0x0a) value = chipMagic;
        if (operation === 0x13) {
          data = new TextEncoder().encode(mismatchedMd5 ? "00000000000000000000000000000000" : expectedMd5);
        }
        const response = new Uint8Array(8 + data.length + 4);
        const responseView = new DataView(response.buffer);
        responseView.setUint8(0, 1);
        responseView.setUint8(1, operation);
        responseView.setUint16(2, data.length + 4, true);
        responseView.setUint32(4, value, true);
        response.set(data, 8);
        const encodedResponse = slipEncode(response);
        const count = operation === 0x08 ? 8 : 1;
        for (let index = 0; index < count; index += 1) this.responses.push(encodedResponse);
      }
    }
    const port = new FakeEsp32Port();
    Object.defineProperty(navigator, "serial", {
      configurable: true,
      value: { requestPort: async () => port },
    });
    globalThis.__fakeEsp32 = port;
  }, { chipMagic, expectedMd5, mismatchedMd5, failSignals, failOperation });
}

test.afterEach(() => {
  while (entrances.length > 0) entrances.pop().kill();
});

async function runDeployment(page, overrides = {}) {
  await page.waitForFunction(() => globalThis.__conduitBrowserHost?.devices);
  return page.evaluate(async (overrides) => {
    const { createEsp32BrowserDeploymentAdapter, ESP32_BROWSER_DEPLOYMENT, sha256ContentId, sha256Bytes } = await import(
      "/targets/esp32/browser-deployment/index.mjs"
    );
    const targetId = overrides.targetId ?? "esp32/riscv32imc/usb-dcf8355d-esp32-c3";
    const payload = new Uint8Array(1500);
    payload.forEach((_, index) => { payload[index] = index & 0xff; });
    payload[0] = 0xe9;
    new DataView(payload.buffer).setUint16(12, overrides.imageChipId ?? 5, true);
    const headerOffset = overrides.imageOffset ?? 0;
    const segmentBytes = headerOffset === 0 ? payload : new Uint8Array(headerOffset + payload.byteLength).fill(0xff);
    if (headerOffset !== 0) segmentBytes.set(payload, headerOffset);
    const segments = [{ offset: 0, bytes: segmentBytes }];
    const imageContentId = await sha256ContentId(targetId, segments);
    const artifactContentId = await sha256Bytes(segmentBytes);
    const base = await __conduitBrowserHost.devices.acquireSerial({
      baudRate: ESP32_BROWSER_DEPLOYMENT.baudRate,
      maximumTransferBytes: ESP32_BROWSER_DEPLOYMENT.maximumTransferBytes,
      maximumReads: overrides.maximumReads ?? 128,
      maximumWrites: 64,
      maximumSignalOperations: 8,
    });
    const adapter = createEsp32BrowserDeploymentAdapter({ base, wait: async () => {} });
    try {
      const plan = await adapter.sealDeployment({
        deploymentPlanId: "esp32-deployment-plan/one",
        deploymentOperationId: "esp32-deployment/one",
        targetId,
        imageId: "image/esp32-c3-signal/one",
        imageContentId,
        sporeId: "spore/esp32/one",
        artifactContentId: overrides.artifactContentId ?? artifactContentId,
        segments,
        resetStrategy: overrides.resetStrategy ?? "usb-jtag",
        explicitAction: true,
      });
      const receipt = await adapter.deploy(plan);
      await adapter.close();
      return {
        plan,
        receipt,
        evidence: adapter.evidence(),
        base: base.evidence(),
        commands: __fakeEsp32.commands,
        signals: __fakeEsp32.signals,
      };
    } catch (error) {
      return { code: error.code, evidence: adapter.evidence(), base: base.evidence() };
    }
  }, overrides);
}

test("exact ESP32 IMAGE deploys through one finite Web Serial Base without runtime promotion", async ({ page }) => {
  const bytes = Uint8Array.from({ length: 1500 }, (_, index) => index & 0xff);
  bytes[0] = 0xe9;
  new DataView(bytes.buffer).setUint16(12, 5, true);
  await installRomLoader(page, { expectedMd5: createHash("md5").update(bytes).digest("hex") });
  await page.goto(await startEntrance());
  const result = await runDeployment(page);

  expect(result.plan).toMatchObject({
    schema: "conduit.esp32/browser-deployment-plan@1",
    targetId: "esp32/riscv32imc/usb-dcf8355d-esp32-c3",
    sporeId: "spore/esp32/one",
    resetStrategy: "usb-jtag",
    requiredReads: 128,
    requiredWrites: 9,
    requiredSignalOperations: 5,
    imageBytes: 1500,
    segmentCount: 1,
  });
  expect(result.receipt).toMatchObject({
    schema: "conduit.esp32/browser-deployment-receipt@1",
    terminal: "RebootRequested",
    chip: "esp32c3",
    chip_magic: 0x6921506f,
    completed_signal_operations: 5,
    admitted_commands: 9,
    completed_commands: 9,
    admitted_image_bytes: 1500,
    completed_image_bytes: 1500,
    verification: "matched",
    reboot_requested: true,
    runtime_truth_created: false,
  });
  expect(result.commands.map(({ operation }) => operation)).toEqual([0x08, 0x0a, 0x0d, 0x0b, 0x02, 0x03, 0x03, 0x13, 0x04]);
  expect(result.signals).toHaveLength(5);
  expect(result.base).toMatchObject({
    phase: "terminal",
    terminal: "Closed",
    admitted_reads: 16,
    admitted_writes: 9,
    admitted_signal_operations: 5,
  });
  expect(result.evidence.resource_terminal).toBe("Closed");
});

test("two Bodies produce distinct native ESP32 images with recoverable provisioning", async ({ page }) => {
  await page.goto(await startEntrance());
  const result = await page.evaluate(async () => {
    const { bindEsp32BodySpore, readEsp32BodySpore } = await import("/targets/esp32/browser-deployment/index.mjs");
    const targetId = "esp32/riscv32imc/usb-dcf8355d-esp32-c3";
    const generic = new Uint8Array(2048);
    generic[0] = 0xe9;
    new DataView(generic.buffer).setUint16(12, 5, true);
    const prepared = (suffix) => ({
      output: "esp32-image",
      target_id: targetId,
      spore_id: `spore/${suffix}`,
      image_id: "image/shared",
      invitation_id: `invitation/${suffix}`,
      body_id: `body/${suffix}`,
      invitation_nonce: Array(32).fill(suffix.charCodeAt(0)),
      invitation_secret: Array(32).fill(suffix.charCodeAt(0) + 1),
      invitation_expires_at_millis: 2_000_000_000_000,
      image_content_digest: `sha256:${"ab".repeat(32)}`,
    });
    const first = await bindEsp32BodySpore({ targetId, segments: [{ offset: 0, bytes: generic }], prepared: prepared("a") });
    const second = await bindEsp32BodySpore({ targetId, segments: [{ offset: 0, bytes: generic }], prepared: prepared("b") });
    const downloaded = new Uint8Array(await new Blob([first.bytes]).arrayBuffer());
    return {
      first: {
        bytes: first.bytes.byteLength,
        contentId: first.content_id,
        deploymentContentId: first.deployment_content_id,
        provision: readEsp32BodySpore(downloaded),
        deploymentUsesDownloadBytes: first.segments.length === 1
          && first.segments[0].offset === 0
          && first.segments[0].bytes.every((byte, index) => byte === downloaded[index]),
        legacyEnvelope: new TextDecoder().decode(downloaded.subarray(0, 8)) === "CNDSPOR1",
      },
      secondContentId: second.content_id,
    };
  });

  expect(result.first).toMatchObject({
    bytes: 4 * 1024 * 1024,
    provision: {
      spore_id: "spore/a",
      image_id: "image/shared",
      invitation_id: "invitation/a",
      body_id: "body/a",
    },
    deploymentUsesDownloadBytes: true,
    legacyEnvelope: false,
  });
  expect(result.first.contentId).toMatch(/^sha256:[0-9a-f]{64}$/);
  expect(result.first.deploymentContentId).toMatch(/^sha256:[0-9a-f]{64}$/);
  expect(result.first.contentId).not.toBe(result.secondContentId);
});

test("wrong chip and flash verification mismatch remain distinct terminal refusals", async ({ page }) => {
  const bytes = Uint8Array.from({ length: 1500 }, (_, index) => index & 0xff);
  bytes[0] = 0xe9;
  new DataView(bytes.buffer).setUint16(12, 5, true);
  const digest = createHash("md5").update(bytes).digest("hex");
  await installRomLoader(page, { chipMagic: 0x9, expectedMd5: digest });
  await page.goto(await startEntrance());
  const wrongChip = await runDeployment(page);
  expect(wrongChip).toMatchObject({ code: "WrongChip", evidence: { terminal: "WrongChip", runtime_truth_created: false } });

  const mismatchPage = await page.context().newPage();
  await installRomLoader(mismatchPage, { expectedMd5: digest, mismatchedMd5: true });
  await mismatchPage.goto(await startEntrance());
  const mismatch = await runDeployment(mismatchPage);
  expect(mismatch).toMatchObject({
    code: "VerificationFailed",
    evidence: { terminal: "VerificationFailed", verification: "mismatch", reboot_requested: false },
  });
});

test("insufficient serial operation budget refuses before use or reset", async ({ page }) => {
  const bytes = Uint8Array.from({ length: 1500 }, (_, index) => index & 0xff);
  bytes[0] = 0xe9;
  new DataView(bytes.buffer).setUint16(12, 5, true);
  await installRomLoader(page, { expectedMd5: createHash("md5").update(bytes).digest("hex") });
  await page.goto(await startEntrance());
  const result = await runDeployment(page, { maximumReads: 8 });
  expect(result).toMatchObject({ code: "OperationBudget" });
  expect(result.base).toMatchObject({ phase: "resource-truth", admitted_reads: 0, admitted_writes: 0, admitted_signal_operations: 0 });
  expect(result.evidence).toMatchObject({ phase: "available", terminal: null });
});

test("stale Body-bound artifact content refuses before serial use", async ({ page }) => {
  const bytes = Uint8Array.from({ length: 1500 }, (_, index) => index & 0xff);
  bytes[0] = 0xe9;
  new DataView(bytes.buffer).setUint16(12, 5, true);
  await installRomLoader(page, { expectedMd5: createHash("md5").update(bytes).digest("hex") });
  await page.goto(await startEntrance());
  const result = await runDeployment(page, { artifactContentId: `sha256:${"00".repeat(32)}` });
  expect(result).toMatchObject({
    code: "ArtifactContentIdentity",
    evidence: { phase: "available", terminal: null },
    base: { phase: "resource-truth", admitted_reads: 0, admitted_writes: 0, admitted_signal_operations: 0 },
  });
});

test("IMAGE chip incompatibility refuses before serial use", async ({ page }) => {
  const bytes = Uint8Array.from({ length: 1500 }, (_, index) => index & 0xff);
  bytes[0] = 0xe9;
  new DataView(bytes.buffer).setUint16(12, 5, true);
  await installRomLoader(page, { expectedMd5: createHash("md5").update(bytes).digest("hex") });
  await page.goto(await startEntrance());
  const result = await runDeployment(page, { imageChipId: 9 });
  expect(result).toMatchObject({
    code: "IncompatibleImage",
    evidence: { phase: "available", terminal: null },
    base: { phase: "resource-truth", admitted_reads: 0, admitted_writes: 0, admitted_signal_operations: 0 },
  });
});

test("reset-line and serial-transfer failures remain distinct terminals", async ({ page }) => {
  const bytes = Uint8Array.from({ length: 1500 }, (_, index) => index & 0xff);
  bytes[0] = 0xe9;
  new DataView(bytes.buffer).setUint16(12, 5, true);
  const expectedMd5 = createHash("md5").update(bytes).digest("hex");
  await installRomLoader(page, { expectedMd5, failSignals: true });
  await page.goto(await startEntrance());
  const reset = await runDeployment(page);
  expect(reset).toMatchObject({ code: "ResetFailed", evidence: { terminal: "ResetFailed" } });

  const transferPage = await page.context().newPage();
  await installRomLoader(transferPage, { expectedMd5, failOperation: 0x0a });
  await transferPage.goto(await startEntrance());
  const transfer = await runDeployment(transferPage);
  expect(transfer).toMatchObject({ code: "DeploymentFailed", evidence: { terminal: "DeploymentFailed" } });
  expect(transfer.code).not.toBe(reset.code);
});

test("WROOM and S3 retain their distinct chip observations and bootloader offsets", async ({ page }) => {
  const cases = [
    {
      targetId: "esp32/xtensa-lx6/hw-463-esp-wroom-32",
      chip: "esp32",
      chipMagic: 0x00f01d83,
      imageChipId: 0,
      imageOffset: 0x1000,
      resetStrategy: "classic",
      extendedBeginBytes: 16,
      maximumReads: 256,
    },
    {
      targetId: "esp32/xtensa-lx7/usb-54e2006398-esp32-s3",
      chip: "esp32s3",
      chipMagic: 0x9,
      imageChipId: 9,
      imageOffset: 0,
      resetStrategy: "usb-jtag",
      extendedBeginBytes: 20,
    },
  ];
  for (const value of cases) {
    const isolated = await page.context().newPage();
    const payload = Uint8Array.from({ length: 1500 }, (_, index) => index & 0xff);
    payload[0] = 0xe9;
    new DataView(payload.buffer).setUint16(12, value.imageChipId, true);
    const bytes = value.imageOffset
      ? new Uint8Array(value.imageOffset + payload.byteLength).fill(0xff)
      : payload;
    if (value.imageOffset) bytes.set(payload, value.imageOffset);
    await installRomLoader(isolated, {
      chipMagic: value.chipMagic,
      expectedMd5: createHash("md5").update(bytes).digest("hex"),
    });
    await isolated.goto(await startEntrance());
    const result = await runDeployment(isolated, value);
    expect(result, JSON.stringify(result)).toHaveProperty("plan");
    expect(result.plan).toMatchObject({ targetId: value.targetId, resetStrategy: value.resetStrategy });
    expect(result.receipt).toMatchObject({ terminal: "RebootRequested", chip: value.chip, chip_magic: value.chipMagic });
    expect(result.commands.find(({ operation }) => operation === 0x02).size).toBe(value.extendedBeginBytes);
    await isolated.close();
  }
});
