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
    const timeout = setTimeout(
      () => reject(new Error(`browser Host was not ready\n${output}`)),
      10_000,
    );
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
      reject(new Error(`browser Host exited ${code}\n${output}`));
    });
  });
}

async function installSuccessfulSerial(page) {
  await page.addInitScript(() => {
    class FakeSerialPort extends EventTarget {
      constructor() {
        super();
        this.opened = null;
        this.closed = false;
        this.writes = [];
        this.signals = [];
        this.readable = {
          getReader: () => ({
            read: async () => ({ value: new Uint8Array([9, 8, 7]), done: false }),
            releaseLock() {},
          }),
        };
        this.writable = {
          getWriter: () => ({
            write: async (bytes) => this.writes.push(Array.from(bytes)),
            releaseLock() {},
          }),
        };
      }
      async open(options) { this.opened = options; }
      getInfo() { return { usbVendorId: 0x2e8a, usbProductId: 0x000a }; }
      async setSignals(signals) { this.signals.push(signals); }
      async close() { this.closed = true; }
    }
    const port = new FakeSerialPort();
    Object.defineProperty(navigator, "serial", {
      configurable: true,
      value: { requestPort: async () => port },
    });
    globalThis.__fakeSerialPort = port;
  });
}

async function installSerialFailure(page, name, duringOpen = false) {
  await page.addInitScript(({ name, duringOpen }) => {
    const error = () => { throw new DOMException("bounded fixture refusal", name); };
    const port = {
      open: duringOpen ? async () => error() : async () => {},
      close: async () => {},
      getInfo: () => ({}),
      addEventListener() {},
    };
    Object.defineProperty(navigator, "serial", {
      configurable: true,
      value: { requestPort: duringOpen ? async () => port : async () => error() },
    });
  }, { name, duringOpen });
}

test.afterEach(() => {
  while (entrances.length > 0) entrances.pop().kill();
});

test("explicit Web Serial acquisition creates one exact finite Base then bounded use", async ({ page }) => {
  await installSuccessfulSerial(page);
  await page.goto(await startEntrance());
  await page.locator("#serial").click();
  await expect(page.locator("#device-status")).toHaveText("resource-truth");

  const truth = await page.evaluate(() => __conduitBrowserHost.devices.evidence());
  expect(truth.schema).toBe("conduit.browser/web-serial-base-evidence@1");
  expect(truth.host_id).toBe(__nonEmpty(truth.host_id));
  expect(truth.boot_id).toBe(__nonEmpty(truth.boot_id));
  expect(truth.permission).toBe("explicit-user-action-required");
  expect(truth.base_implementation_id).toBe("browser/web-serial@1");
  expect(truth.base_instance_id).toMatch(/^serial-base\//);
  expect(truth.resource_handle).toMatch(/^serial\//);
  expect(truth.usb_vendor_id).toBe(0x2e8a);
  expect(truth.usb_product_id).toBe(0x000a);
  expect(truth.transfer_bounds).toEqual({
    maximum_transfer_bytes: 4096,
    maximum_reads: 8,
    maximum_writes: 8,
    maximum_signal_operations: 0,
    maximum_in_flight: 1,
  });
  await expect(page.locator("#serial-close")).toBeEnabled();
  await expect(page.locator("#serial-evidence-details")).toHaveAttribute("open", "");
  expect(JSON.parse(await page.locator("#device-evidence").textContent())).toEqual(truth);

  const beforeDuplicate = JSON.stringify(truth);
  const duplicate = await page.evaluate(async () => {
    try {
      await __conduitBrowserHost.devices.acquireSerial();
      return null;
    } catch (error) {
      return error.message;
    }
  });
  expect(duplicate).toContain("already owned");
  expect(JSON.stringify(await page.evaluate(() => __conduitBrowserHost.devices.evidence())))
    .toBe(beforeDuplicate);

  const transfer = await page.evaluate(async () => {
    const resource = globalThis.__conduitSerialResource;
    const started = resource.startUse("browser/serial-use-plan/one");
    const written = await resource.write(new Uint8Array([1, 2, 3, 4]));
    const read = await resource.read();
    return {
      started,
      written: Array.from(written.bytes),
      read: Array.from(read.bytes),
      openOptions: __fakeSerialPort.opened,
      writes: __fakeSerialPort.writes,
    };
  });
  expect(transfer.started.phase).toBe("serial-use-playing");
  expect(transfer.written).toEqual([1, 2, 3, 4]);
  expect(transfer.read).toEqual([9, 8, 7]);
  expect(transfer.openOptions).toEqual({
    baudRate: 115200,
    dataBits: 8,
    stopBits: 1,
    parity: "none",
    bufferSize: 4096,
    flowControl: "none",
  });
  expect(transfer.writes).toEqual([[1, 2, 3, 4]]);
  await page.locator("#serial-close").click();
  await expect(page.locator("#device-status")).toHaveText("terminal: Closed");
  await expect(page.locator("#serial-close")).toBeDisabled();
  expect(JSON.parse(await page.locator("#device-evidence").textContent())).toMatchObject({
    phase: "terminal",
    terminal: "Closed",
    admitted_reads: 1,
    admitted_writes: 1,
    retained_bytes: 0,
  });
});

test("permission, selection, API, and open failures remain distinct", async ({ page }) => {
  const cases = [
    ["SecurityError", false, "PermissionDenied"],
    ["NotFoundError", false, "NoPortSelected"],
    ["NetworkError", true, "OpenFailed"],
  ];
  for (const [name, duringOpen, terminal] of cases) {
    const isolated = await page.context().newPage();
    await installSerialFailure(isolated, name, duringOpen);
    await isolated.goto(await startEntrance());
    await isolated.locator("#serial").click();
    await expect(isolated.locator("#device-status")).toHaveText(`terminal: ${terminal}`);
    await expect(isolated.locator("#serial-close")).toBeDisabled();
    expect(await isolated.evaluate(() => __conduitBrowserHost.devices.evidence()))
      .toMatchObject({ phase: "terminal", terminal, resource_handle: null });
    await isolated.close();
  }

  const unsupported = await page.context().newPage();
  await unsupported.addInitScript(() => {
    Object.defineProperty(navigator, "serial", { configurable: true, value: undefined });
  });
  await unsupported.goto(await startEntrance());
  await unsupported.locator("#serial").click();
  await expect(unsupported.locator("#device-status")).toHaveText("terminal: Unsupported");
  await expect(unsupported.locator("#serial-close")).toBeDisabled();
});

test("disconnect and page cancellation terminate without retry or replacement", async ({ page }) => {
  await installSuccessfulSerial(page);
  await page.goto(await startEntrance());
  await page.locator("#serial").click();
  await expect(page.locator("#device-status")).toHaveText("resource-truth");
  await page.evaluate(() => __fakeSerialPort.dispatchEvent(new Event("disconnect")));
  await expect(page.locator("#device-status")).toHaveText("terminal: DeviceLost");

  const cancelled = await page.context().newPage();
  await installSuccessfulSerial(cancelled);
  await cancelled.goto(await startEntrance());
  await cancelled.locator("#serial").click();
  await expect(cancelled.locator("#device-status")).toHaveText("resource-truth");
  const receipt = await cancelled.evaluate(() => {
    __conduitBrowserHost.devices.terminate();
    return __conduitBrowserHost.devices.evidence();
  });
  expect(receipt).toMatchObject({ phase: "terminal", terminal: "ResourceCancelled" });

  const oversized = await page.context().newPage();
  await installSuccessfulSerial(oversized);
  await oversized.goto(await startEntrance());
  await oversized.locator("#serial").click();
  await expect(oversized.locator("#device-status")).toHaveText("resource-truth");
  const overflow = await oversized.evaluate(async () => {
    const resource = globalThis.__conduitSerialResource;
    resource.startUse("browser/serial-overflow-plan/one");
    try {
      await resource.write(new Uint8Array(4097));
      return null;
    } catch (error) {
      return { message: error.message, evidence: resource.evidence(), writes: __fakeSerialPort.writes };
    }
  });
  expect(overflow.message).toContain("exceeds admitted bound");
  expect(overflow.evidence).toMatchObject({ phase: "terminal", terminal: "TransferTooLarge" });
  expect(overflow.writes).toEqual([]);
});

function __nonEmpty(value) {
  expect(value).toEqual(expect.any(String));
  expect(value.length).toBeGreaterThan(0);
  return value;
}
