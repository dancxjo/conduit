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
    child.once("exit", (code) => {
      clearTimeout(timeout);
      reject(new Error(`executable browser Host exited ${code}\n${output}`));
    });
  });
}

async function installSuccessfulUsb(page, transferStatus = "ok") {
  await page.addInitScript(({ transferStatus }) => {
    class FakeUsbDevice {
      constructor() {
        this.vendorId = 0x2e8a;
        this.productId = 0x000a;
        this.configuration = null;
        this.opened = false;
        this.claimed = [];
        this.released = [];
        this.alternates = [];
        this.outTransfers = [];
        this.closed = false;
      }
      async open() { this.opened = true; }
      async selectConfiguration(value) { this.configuration = { configurationValue: value }; }
      async claimInterface(number) { this.claimed.push(number); }
      async selectAlternateInterface(number, alternate) { this.alternates.push([number, alternate]); }
      async releaseInterface(number) { this.released.push(number); }
      async close() { this.closed = true; }
      async transferOut(endpoint, bytes) {
        this.outTransfers.push([endpoint, Array.from(bytes)]);
        return { status: transferStatus, bytesWritten: bytes.byteLength };
      }
      async transferIn(endpoint) {
        this.inEndpoint = endpoint;
        const bytes = new Uint8Array([9, 8, 7]);
        return { status: transferStatus, data: new DataView(bytes.buffer) };
      }
    }
    const device = new FakeUsbDevice();
    const usb = new EventTarget();
    usb.requests = [];
    usb.requestDevice = async (options) => {
      usb.requests.push(options);
      return device;
    };
    Object.defineProperty(navigator, "usb", { configurable: true, value: usb });
    globalThis.__fakeUsb = usb;
    globalThis.__fakeUsbDevice = device;
  }, { transferStatus });
}

async function installUsbFailure(page, name, failingStage) {
  await page.addInitScript(({ name, failingStage }) => {
    const fail = () => { throw new DOMException("bounded fixture refusal", name); };
    const device = {
      vendorId: 0x2e8a,
      productId: 0x000a,
      configuration: null,
      open: async () => failingStage === "open" ? fail() : undefined,
      selectConfiguration: async () => failingStage === "configuration" ? fail() : undefined,
      claimInterface: async () => failingStage === "interface" ? fail() : undefined,
      selectAlternateInterface: async () => failingStage === "alternate" ? fail() : undefined,
      releaseInterface: async () => {},
      close: async () => {},
    };
    const usb = new EventTarget();
    usb.requestDevice = async () => failingStage === "request" ? fail() : device;
    Object.defineProperty(navigator, "usb", { configurable: true, value: usb });
  }, { name, failingStage });
}

test.afterEach(() => {
  while (entrances.length > 0) entrances.pop().kill();
});

test("explicit WebUSB acquisition creates one exact finite Base then bounded use", async ({ page }) => {
  await installSuccessfulUsb(page);
  await page.goto(await startEntrance());
  await page.locator("#usb").click();
  await expect(page.locator("#usb-device-status")).toHaveText("resource-truth");

  const truth = await page.evaluate(() => __conduitBrowserHost.usbDevices.evidence());
  expect(truth).toMatchObject({
    schema: "conduit.browser/web-usb-base-evidence@1",
    phase: "resource-truth",
    permission: "explicit-user-action-required",
    base_implementation_id: "browser/web-usb@1",
    vendor_id: 0x2e8a,
    product_id: 0x000a,
    configuration: {
      configuration_value: 1,
      interface_number: 0,
      alternate_setting: 0,
      in_endpoint: 1,
      out_endpoint: 1,
    },
    transfer_bounds: {
      maximum_transfer_bytes: 4096,
      maximum_in_transfers: 8,
      maximum_out_transfers: 8,
      maximum_in_flight: 1,
    },
  });
  expect(truth.resource_handle).toMatch(/^usb\//);
  expect(truth.base_instance_id).toMatch(/^usb-base\//);
  await expect(page.locator("#usb-close")).toBeEnabled();
  await expect(page.locator("#usb-evidence-details")).toHaveAttribute("open", "");
  expect(JSON.parse(await page.locator("#usb-device-evidence").textContent())).toEqual(truth);

  const beforeDuplicate = JSON.stringify(truth);
  const duplicate = await page.evaluate(async () => {
    try { await __conduitBrowserHost.usbDevices.acquireUsb(); return null; }
    catch (error) { return error.message; }
  });
  expect(duplicate).toContain("already owned");
  expect(JSON.stringify(await page.evaluate(() => __conduitBrowserHost.usbDevices.evidence()))).toBe(beforeDuplicate);

  const transfer = await page.evaluate(async () => {
    const resource = globalThis.__conduitUsbResource;
    const started = resource.startUse("browser/usb-use-plan/one");
    const written = await resource.transferOut(new Uint8Array([1, 2, 3, 4]));
    const read = await resource.transferIn(8);
    return {
      started,
      written: Array.from(written.bytes),
      read: Array.from(read.bytes),
      selected: __fakeUsbDevice.configuration.configurationValue,
      claimed: __fakeUsbDevice.claimed,
      outTransfers: __fakeUsbDevice.outTransfers,
      requests: __fakeUsb.requests,
    };
  });
  expect(transfer.started.phase).toBe("usb-use-playing");
  expect(transfer.written).toEqual([1, 2, 3, 4]);
  expect(transfer.read).toEqual([9, 8, 7]);
  expect(transfer.selected).toBe(1);
  expect(transfer.claimed).toEqual([0]);
  expect(transfer.outTransfers).toEqual([[1, [1, 2, 3, 4]]]);
  expect(transfer.requests).toEqual([{ filters: [] }]);
  await page.locator("#usb-close").click();
  await expect(page.locator("#usb-device-status")).toHaveText("terminal: Closed");
  await expect(page.locator("#usb-close")).toBeDisabled();
  expect(await page.evaluate(() => __fakeUsbDevice.released)).toEqual([0]);
  expect(await page.evaluate(() => __fakeUsbDevice.closed)).toBe(true);
  expect(JSON.parse(await page.locator("#usb-device-evidence").textContent())).toMatchObject({
    phase: "terminal",
    terminal: "Closed",
    admitted_in_transfers: 1,
    admitted_out_transfers: 1,
    retained_bytes: 0,
  });
});

test("selection, API, open, configuration, interface, and alternate failures remain distinct", async ({ page }) => {
  const cases = [
    ["SecurityError", "request", "PermissionDenied", {}],
    ["NotFoundError", "request", "NoDeviceSelected", {}],
    ["NetworkError", "open", "OpenFailed", {}],
    ["NotFoundError", "configuration", "ConfigurationFailed", {}],
    ["NetworkError", "interface", "InterfaceClaimFailed", {}],
    ["NetworkError", "alternate", "AlternateFailed", { alternateSetting: 1 }],
  ];
  for (const [name, stage, terminal, options] of cases) {
    const isolated = await page.context().newPage();
    await installUsbFailure(isolated, name, stage);
    await isolated.goto(await startEntrance());
    await expect(isolated.locator("#identity")).toBeVisible();
    await isolated.evaluate(async (options) => {
      try { await __conduitBrowserHost.usbDevices.acquireUsb(options); } catch {}
    }, options);
    await expect(isolated.locator("#usb-device-status")).toHaveText(`terminal: ${terminal}`);
    await expect(isolated.locator("#usb-close")).toBeDisabled();
    expect(await isolated.evaluate(() => __conduitBrowserHost.usbDevices.evidence()))
      .toMatchObject({ phase: "terminal", terminal, resource_handle: null });
    await isolated.close();
  }

  const unsupported = await page.context().newPage();
  await unsupported.addInitScript(() => {
    Object.defineProperty(navigator, "usb", { configurable: true, value: undefined });
  });
  await unsupported.goto(await startEntrance());
  await unsupported.locator("#usb").click();
  await expect(unsupported.locator("#usb-device-status")).toHaveText("terminal: Unsupported");
  await expect(unsupported.locator("#usb-close")).toBeDisabled();

  const invalid = await page.context().newPage();
  await installSuccessfulUsb(invalid);
  await invalid.goto(await startEntrance());
  await expect(invalid.locator("#identity")).toBeVisible();
  const invalidResult = await invalid.evaluate(async () => {
    try {
      await __conduitBrowserHost.usbDevices.acquireUsb({ interfaceNumber: 256 });
      return null;
    } catch (error) {
      return { message: error.message, requests: __fakeUsb.requests };
    }
  });
  expect(invalidResult.message).toContain("outside its admitted bound");
  expect(invalidResult.requests).toEqual([]);
  await expect(invalid.locator("#usb-device-status")).toHaveText("WebUSB acquisition offer available");
});

test("disconnect, cancellation, overflow, and stalled transfer terminate without replacement", async ({ page }) => {
  await installSuccessfulUsb(page);
  await page.goto(await startEntrance());
  await page.locator("#usb").click();
  const lost = await page.evaluate(() => {
    const event = new Event("disconnect");
    Object.defineProperty(event, "device", { value: __fakeUsbDevice });
    __fakeUsb.dispatchEvent(event);
    return __conduitBrowserHost.usbDevices.evidence();
  });
  expect(lost).toMatchObject({ phase: "terminal", terminal: "DeviceLost" });

  const cancelled = await page.context().newPage();
  await installSuccessfulUsb(cancelled);
  await cancelled.goto(await startEntrance());
  await cancelled.locator("#usb").click();
  const cancellation = await cancelled.evaluate(() => {
    __conduitBrowserHost.usbDevices.terminate();
    return __conduitBrowserHost.usbDevices.evidence();
  });
  expect(cancellation).toMatchObject({ phase: "terminal", terminal: "ResourceCancelled" });

  const oversized = await page.context().newPage();
  await installSuccessfulUsb(oversized);
  await oversized.goto(await startEntrance());
  await oversized.locator("#usb").click();
  const overflow = await oversized.evaluate(async () => {
    const resource = globalThis.__conduitUsbResource;
    resource.startUse("browser/usb-overflow-plan/one");
    try { await resource.transferOut(new Uint8Array(4097)); return null; }
    catch (error) { return { message: error.message, evidence: resource.evidence(), transfers: __fakeUsbDevice.outTransfers }; }
  });
  expect(overflow.message).toContain("exceeds admitted bound");
  expect(overflow.evidence).toMatchObject({ phase: "terminal", terminal: "TransferTooLarge" });
  expect(overflow.transfers).toEqual([]);

  const stalled = await page.context().newPage();
  await installSuccessfulUsb(stalled, "stall");
  await stalled.goto(await startEntrance());
  await stalled.locator("#usb").click();
  const stall = await stalled.evaluate(async () => {
    const resource = globalThis.__conduitUsbResource;
    resource.startUse("browser/usb-stall-plan/one");
    try { await resource.transferIn(8); return null; }
    catch { return resource.evidence(); }
  });
  expect(stall).toMatchObject({ phase: "terminal", terminal: "TransferStalled" });
});
