const INPUT_CAPACITY = 4096;
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const IN = 1;
const OUT = 2;

function requireStatus(status, operation) {
  if (status < 0) throw new Error(`browser USB ${operation} refused ${status}`);
}

function writeInput(api, bytes) {
  if (bytes.length > api.conduit_browser_usb_input_capacity()) {
    throw new Error("browser USB input exceeds admitted capacity");
  }
  new Uint8Array(api.memory.buffer, api.conduit_browser_usb_input_ptr(), bytes.length).set(bytes);
}

function evidence(api) {
  const bytes = new Uint8Array(
    api.memory.buffer,
    api.conduit_browser_usb_evidence_ptr(),
    api.conduit_browser_usb_evidence_len(),
  );
  return JSON.parse(decoder.decode(bytes));
}

function bytesOf(value) {
  if (value instanceof Uint8Array) return value;
  if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  if (value instanceof ArrayBuffer) return new Uint8Array(value);
  throw new TypeError("USB transfer must be bytes");
}

function acquisitionOutcome(error, stage) {
  if (error?.name === "SecurityError") return 1;
  if (stage === "request" && error?.name === "NotFoundError") return 2;
  if (error?.name === "NotSupportedError") return 3;
  if (error?.name === "AbortError") return 8;
  if (stage === "open") return 4;
  if (stage === "configuration") return 5;
  if (stage === "interface") return 6;
  if (stage === "alternate") return 7;
  return 9;
}

function transferFailure(status) {
  if (status === "stall") return 2;
  if (status === "babble") return 3;
  return 4;
}

function requireInteger(value, minimum, maximum, name) {
  if (!Number.isInteger(value) || value < minimum || value > maximum) {
    throw new RangeError(`${name} is outside its admitted bound`);
  }
}

export function createBrowserUsbDeviceBase({
  api,
  hostId,
  bootId,
  usb = navigator.usb,
  status = document.querySelector("#usb-device-status"),
  output = document.querySelector("#usb-device-evidence"),
}) {
  const required = [
    "conduit_browser_usb_input_ptr",
    "conduit_browser_usb_input_capacity",
    "conduit_browser_usb_start_acquisition",
    "conduit_browser_usb_complete_acquisition",
    "conduit_browser_usb_start_use",
    "conduit_browser_usb_begin_transfer",
    "conduit_browser_usb_complete_transfer",
    "conduit_browser_usb_release_transfer",
    "conduit_browser_usb_transfer_failed",
    "conduit_browser_usb_device_lost",
    "conduit_browser_usb_close_failed",
    "conduit_browser_usb_close",
    "conduit_browser_usb_cancel",
    "conduit_browser_usb_evidence_ptr",
    "conduit_browser_usb_evidence_len",
  ];
  if (required.some((name) => !(name in api)) || api.conduit_browser_usb_input_capacity() !== INPUT_CAPACITY) {
    throw new Error("browser USB ABI is incomplete");
  }

  let device = null;
  let terminal = false;
  const publish = () => {
    const value = evidence(api);
    if (output) {
      output.textContent = JSON.stringify(value, null, 2);
      output.closest("details")?.setAttribute("open", "");
    }
    if (status) status.textContent = `${value.phase}${value.terminal ? `: ${value.terminal}` : ""}`;
    return value;
  };

  async function acquireUsb({
    configurationValue = 1,
    interfaceNumber = 0,
    alternateSetting = 0,
    inEndpoint = 1,
    outEndpoint = 1,
    maximumTransferBytes = 4096,
    maximumInTransfers = 8,
    maximumOutTransfers = 8,
  } = {}) {
    if (device || terminal) throw new Error("one bounded WebUSB Base session is already owned");
    requireInteger(configurationValue, 1, 255, "USB configuration");
    requireInteger(interfaceNumber, 0, 255, "USB interface");
    requireInteger(alternateSetting, 0, 255, "USB alternate setting");
    requireInteger(inEndpoint, 1, 15, "USB IN endpoint");
    requireInteger(outEndpoint, 1, 15, "USB OUT endpoint");
    requireInteger(maximumTransferBytes, 1, 4096, "USB transfer bytes");
    requireInteger(maximumInTransfers, 1, 8, "USB IN transfer count");
    requireInteger(maximumOutTransfers, 1, 8, "USB OUT transfer count");
    const identity = encoder.encode(hostId + bootId);
    writeInput(api, identity);
    requireStatus(api.conduit_browser_usb_start_acquisition(
      hostId.length,
      bootId.length,
      1,
      1,
      configurationValue,
      interfaceNumber,
      alternateSetting,
      inEndpoint,
      outEndpoint,
      maximumTransferBytes,
      maximumInTransfers,
      maximumOutTransfers,
    ), "acquisition admission");
    publish();
    if (!usb?.requestDevice) {
      requireStatus(api.conduit_browser_usb_complete_acquisition(3, 0, 0, 0, 0, 64), "unsupported completion");
      terminal = true;
      publish();
      throw new DOMException("WebUSB is unavailable", "NotSupportedError");
    }

    let stage = "request";
    try {
      device = await usb.requestDevice({ filters: [] });
      stage = "open";
      await device.open();
      stage = "configuration";
      if (device.configuration?.configurationValue !== configurationValue) {
        await device.selectConfiguration(configurationValue);
      }
      stage = "interface";
      await device.claimInterface(interfaceNumber);
      stage = "alternate";
      if (alternateSetting !== 0) {
        await device.selectAlternateInterface(interfaceNumber, alternateSetting);
      }

      const handle = `usb/${crypto.randomUUID()}`;
      const baseInstance = `usb-base/${crypto.randomUUID()}`;
      const encodedHandle = encoder.encode(handle);
      const encodedBase = encoder.encode(baseInstance);
      const result = new Uint8Array(encodedHandle.length + encodedBase.length);
      result.set(encodedHandle);
      result.set(encodedBase, encodedHandle.length);
      writeInput(api, result);
      requireStatus(api.conduit_browser_usb_complete_acquisition(
        0,
        encodedHandle.length,
        encodedBase.length,
        device.vendorId,
        device.productId,
        256,
      ), "resource completion");

      const acquiredDevice = device;
      const onDisconnect = (event) => {
        if (event.device !== acquiredDevice || device !== acquiredDevice || terminal) return;
        terminal = true;
        requireStatus(api.conduit_browser_usb_device_lost(), "device loss");
        device = null;
        publish();
      };
      usb.addEventListener?.("disconnect", onDisconnect);
      const resourceTruth = publish();
      let useStarted = false;

      function startUse(planId) {
        if (useStarted || terminal || device !== acquiredDevice) {
          throw new Error("WebUSB resource is not current for a new use Plan");
        }
        const plan = encoder.encode(planId);
        writeInput(api, plan);
        requireStatus(api.conduit_browser_usb_start_use(plan.length, 1), "use Plan");
        useStarted = true;
        return publish();
      }

      async function transfer(direction, effect) {
        if (!useStarted || terminal || device !== acquiredDevice) {
          throw new Error("WebUSB use Plan is not active");
        }
        requireStatus(api.conduit_browser_usb_begin_transfer(direction), "transfer admission");
        try {
          const { bytes, transferStatus } = await effect();
          if (transferStatus !== "ok") {
            terminal = true;
            requireStatus(api.conduit_browser_usb_transfer_failed(transferFailure(transferStatus)), "transfer status");
            await closePlatform(acquiredDevice, interfaceNumber);
            device = null;
            publish();
            throw new DOMException(`WebUSB transfer ended ${transferStatus}`, "NetworkError");
          }
          if (bytes.byteLength === 0 || bytes.byteLength > maximumTransferBytes) {
            throw new RangeError("WebUSB transfer exceeds admitted bound");
          }
          writeInput(api, bytes);
          requireStatus(api.conduit_browser_usb_complete_transfer(direction, bytes.length), "transfer completion");
          const receipt = publish();
          requireStatus(api.conduit_browser_usb_release_transfer(), "transfer release");
          publish();
          return { bytes, receipt };
        } catch (error) {
          if (!terminal) {
            terminal = true;
            const failure = error instanceof RangeError ? 1 : 4;
            requireStatus(api.conduit_browser_usb_transfer_failed(failure), "transfer failure");
            await closePlatform(acquiredDevice, interfaceNumber);
            device = null;
            publish();
          }
          throw error;
        }
      }

      async function transferOut(bytes) {
        const value = bytesOf(bytes);
        if (value.byteLength === 0 || value.byteLength > maximumTransferBytes) {
          terminal = true;
          requireStatus(api.conduit_browser_usb_transfer_failed(1), "oversized OUT transfer");
          await closePlatform(acquiredDevice, interfaceNumber);
          device = null;
          publish();
          throw new RangeError("WebUSB OUT transfer exceeds admitted bound");
        }
        return transfer(OUT, async () => {
          const result = await acquiredDevice.transferOut(outEndpoint, value);
          if (result.status === "ok" && (
            !Number.isInteger(result.bytesWritten)
            || result.bytesWritten <= 0
            || result.bytesWritten > value.byteLength
          )) {
            throw new DOMException("WebUSB OUT completion had invalid byte truth", "NetworkError");
          }
          return { bytes: value.subarray(0, result.bytesWritten), transferStatus: result.status };
        });
      }

      async function transferIn(length = maximumTransferBytes) {
        if (!Number.isInteger(length) || length <= 0 || length > maximumTransferBytes) {
          terminal = true;
          requireStatus(api.conduit_browser_usb_transfer_failed(1), "oversized IN transfer");
          await closePlatform(acquiredDevice, interfaceNumber);
          device = null;
          publish();
          throw new RangeError("WebUSB IN transfer exceeds admitted bound");
        }
        return transfer(IN, async () => {
          const result = await acquiredDevice.transferIn(inEndpoint, length);
          if (result.status === "ok" && !result.data?.byteLength) {
            throw new DOMException("WebUSB IN completion had no byte truth", "NetworkError");
          }
          return {
            bytes: result.data ? bytesOf(result.data) : new Uint8Array(),
            transferStatus: result.status,
          };
        });
      }

      async function close() {
        if (terminal || device !== acquiredDevice) return publish();
        try {
          await acquiredDevice.releaseInterface(interfaceNumber);
          await acquiredDevice.close();
          terminal = true;
          device = null;
          requireStatus(api.conduit_browser_usb_close(), "close");
          return publish();
        } catch (error) {
          terminal = true;
          device = null;
          requireStatus(api.conduit_browser_usb_close_failed(), "close failure");
          publish();
          throw error;
        }
      }

      return Object.freeze({ resourceTruth, evidence: () => evidence(api), startUse, transferIn, transferOut, close });
    } catch (error) {
      if (!terminal) {
        const outcome = acquisitionOutcome(error, stage);
        requireStatus(api.conduit_browser_usb_complete_acquisition(outcome, 0, 0, 0, 0, 64), "acquisition refusal");
        terminal = true;
        if (device) await closePlatform(device, interfaceNumber);
        device = null;
        publish();
      }
      throw error;
    }
  }

  function terminate() {
    if (terminal || !device) return;
    const current = device;
    terminal = true;
    requireStatus(api.conduit_browser_usb_cancel(), "page cancellation");
    device = null;
    void closePlatform(current, evidence(api).configuration.interface_number);
    publish();
  }

  globalThis.addEventListener("pagehide", terminate, { once: true });
  return Object.freeze({ acquireUsb, evidence: () => evidence(api), terminate });
}

async function closePlatform(device, interfaceNumber) {
  await device.releaseInterface?.(interfaceNumber).catch(() => {});
  await device.close?.().catch(() => {});
}
