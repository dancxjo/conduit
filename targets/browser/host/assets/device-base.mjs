const INPUT_CAPACITY = 4096;
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const READ = 1;
const WRITE = 2;

function requireStatus(status, operation) {
  if (status < 0) throw new Error(`browser serial ${operation} refused ${status}`);
}

function writeInput(api, bytes) {
  if (bytes.length > api.conduit_browser_serial_input_capacity()) {
    throw new Error("browser serial input exceeds admitted capacity");
  }
  new Uint8Array(
    api.memory.buffer,
    api.conduit_browser_serial_input_ptr(),
    bytes.length,
  ).set(bytes);
}

function evidence(api) {
  const bytes = new Uint8Array(
    api.memory.buffer,
    api.conduit_browser_serial_evidence_ptr(),
    api.conduit_browser_serial_evidence_len(),
  );
  return JSON.parse(decoder.decode(bytes));
}

function parityCode(value) {
  if (value === "none") return 0;
  if (value === "even") return 1;
  if (value === "odd") return 2;
  throw new TypeError("unsupported serial parity");
}

function acquisitionOutcome(error, opening) {
  if (error?.name === "SecurityError") return 1;
  if (error?.name === "NotFoundError") return 2;
  if (error?.name === "NotSupportedError") return 3;
  if (opening && ["InvalidStateError", "NetworkError"].includes(error?.name)) return 4;
  if (error?.name === "AbortError") return 5;
  return 6;
}

function bytesOf(value) {
  if (value instanceof Uint8Array) return value;
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) return new Uint8Array(value);
  throw new TypeError("serial transfer must be bytes");
}

export function createBrowserDeviceBase({
  api,
  hostId,
  bootId,
  serial = navigator.serial,
  status = document.querySelector("#device-status"),
  output = document.querySelector("#device-evidence"),
}) {
  const required = [
    "conduit_browser_serial_input_ptr",
    "conduit_browser_serial_input_capacity",
    "conduit_browser_serial_start_acquisition",
    "conduit_browser_serial_complete_acquisition",
    "conduit_browser_serial_start_use",
    "conduit_browser_serial_begin_transfer",
    "conduit_browser_serial_complete_transfer",
    "conduit_browser_serial_release_transfer",
    "conduit_browser_serial_transfer_failed",
    "conduit_browser_serial_device_lost",
    "conduit_browser_serial_close",
    "conduit_browser_serial_cancel",
    "conduit_browser_serial_evidence_ptr",
    "conduit_browser_serial_evidence_len",
  ];
  if (
    required.some((name) => !(name in api)) ||
    api.conduit_browser_serial_input_capacity() !== INPUT_CAPACITY
  ) {
    throw new Error("browser serial ABI is incomplete");
  }

  let port = null;
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

  async function acquireSerial({
    baudRate = 115200,
    dataBits = 8,
    stopBits = 1,
    parity = "none",
    bufferSize = 4096,
    maximumTransferBytes = 4096,
    maximumReads = 8,
    maximumWrites = 8,
  } = {}) {
    if (port || terminal) throw new Error("one bounded browser device Base session is already owned");
    const identity = encoder.encode(hostId + bootId);
    writeInput(api, identity);
    requireStatus(
      api.conduit_browser_serial_start_acquisition(
        hostId.length,
        bootId.length,
        1,
        1,
        baudRate,
        dataBits,
        stopBits,
        parityCode(parity),
        bufferSize,
        maximumTransferBytes,
        maximumReads,
        maximumWrites,
      ),
      "acquisition admission",
    );
    publish();
    if (!serial?.requestPort) {
      requireStatus(
        api.conduit_browser_serial_complete_acquisition(3, 0, 0, 0, 0, 64),
        "unsupported completion",
      );
      terminal = true;
      publish();
      throw new DOMException("Web Serial is unavailable", "NotSupportedError");
    }

    let opening = false;
    try {
      port = await serial.requestPort();
      opening = true;
      await port.open({
        baudRate,
        dataBits,
        stopBits,
        parity,
        bufferSize,
        flowControl: "none",
      });
      const info = port.getInfo?.() ?? {};
      const handle = `serial/${crypto.randomUUID()}`;
      const baseInstance = `serial-base/${crypto.randomUUID()}`;
      const encodedHandle = encoder.encode(handle);
      const encodedBase = encoder.encode(baseInstance);
      const result = new Uint8Array(encodedHandle.length + encodedBase.length);
      result.set(encodedHandle);
      result.set(encodedBase, encodedHandle.length);
      writeInput(api, result);
      requireStatus(
        api.conduit_browser_serial_complete_acquisition(
          0,
          encodedHandle.length,
          encodedBase.length,
          info.usbVendorId ?? 0,
          info.usbProductId ?? 0,
          256,
        ),
        "resource completion",
      );
      const acquiredPort = port;
      const onDisconnect = () => {
        if (port !== acquiredPort || terminal) return;
        terminal = true;
        requireStatus(api.conduit_browser_serial_device_lost(), "device loss");
        port = null;
        publish();
      };
      acquiredPort.addEventListener?.("disconnect", onDisconnect, { once: true });
      const resourceTruth = publish();
      let useStarted = false;

      function startUse(planId) {
        if (useStarted || terminal || port !== acquiredPort) {
          throw new Error("browser serial resource is not current for a new use Plan");
        }
        const plan = encoder.encode(planId);
        writeInput(api, plan);
        requireStatus(api.conduit_browser_serial_start_use(plan.length, 1), "use Plan");
        useStarted = true;
        return publish();
      }

      async function transfer(direction, effect) {
        if (!useStarted || terminal || port !== acquiredPort) {
          throw new Error("browser serial use Plan is not active");
        }
        requireStatus(api.conduit_browser_serial_begin_transfer(direction), "transfer admission");
        try {
          const bytes = bytesOf(await effect());
          if (bytes.byteLength > maximumTransferBytes) {
            throw new RangeError("serial transfer exceeds admitted bound");
          }
          writeInput(api, bytes);
          requireStatus(
            api.conduit_browser_serial_complete_transfer(direction, bytes.length),
            "transfer completion",
          );
          const receipt = publish();
          requireStatus(api.conduit_browser_serial_release_transfer(), "transfer release");
          publish();
          return { bytes, receipt };
        } catch (error) {
          terminal = true;
          const failure = error instanceof RangeError ? 1 : error?.name === "AbortError" ? 3 : 2;
          requireStatus(api.conduit_browser_serial_transfer_failed(failure), "transfer failure");
          await acquiredPort.close?.().catch(() => {});
          port = null;
          publish();
          throw error;
        }
      }

      async function write(bytes) {
        const value = bytesOf(bytes);
        if (value.byteLength > maximumTransferBytes) {
          terminal = true;
          requireStatus(api.conduit_browser_serial_transfer_failed(1), "oversized write");
          await acquiredPort.close?.().catch(() => {});
          port = null;
          publish();
          throw new RangeError("serial write exceeds admitted bound");
        }
        return transfer(WRITE, async () => {
          if (!acquiredPort.writable) throw new DOMException("serial port is not writable", "NetworkError");
          const writer = acquiredPort.writable.getWriter();
          try {
            await writer.write(value);
            return value;
          } finally {
            writer.releaseLock();
          }
        });
      }

      async function read() {
        return transfer(READ, async () => {
          if (!acquiredPort.readable) throw new DOMException("serial port is not readable", "NetworkError");
          const reader = acquiredPort.readable.getReader();
          try {
            const { value, done } = await reader.read();
            if (done || !value?.byteLength) throw new DOMException("serial port closed before a value", "AbortError");
            return value;
          } finally {
            reader.releaseLock();
          }
        });
      }

      async function close() {
        if (terminal || port !== acquiredPort) return publish();
        await acquiredPort.close();
        terminal = true;
        port = null;
        requireStatus(api.conduit_browser_serial_close(), "close");
        return publish();
      }

      return Object.freeze({
        resourceTruth,
        evidence: () => evidence(api),
        startUse,
        write,
        read,
        close,
      });
    } catch (error) {
      if (!terminal) {
        const outcome = acquisitionOutcome(error, opening);
        requireStatus(
          api.conduit_browser_serial_complete_acquisition(outcome, 0, 0, 0, 0, 64),
          "acquisition refusal",
        );
        terminal = true;
        if (port) await port.close?.().catch(() => {});
        port = null;
        publish();
      }
      throw error;
    }
  }

  function terminate() {
    if (terminal || !port) return;
    terminal = true;
    requireStatus(api.conduit_browser_serial_cancel(), "page cancellation");
    void port.close?.().catch(() => {});
    port = null;
    publish();
  }

  globalThis.addEventListener("pagehide", terminate, { once: true });
  return Object.freeze({ acquireSerial, evidence: () => evidence(api), terminate });
}
