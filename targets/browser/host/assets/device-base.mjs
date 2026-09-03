const INPUT_CAPACITY = 4096;
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const READ = 1;
const WRITE = 2;
const SIGNALS = 3;

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
  selectedImplementations,
  serial = navigator.serial,
  status = document.querySelector("#device-status"),
  output = document.querySelector("#device-evidence"),
}) {
  const selected = new Set(selectedImplementations ?? []);
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
    maximumSignalOperations = 0,
  } = {}) {
    if (!selected.has("browser/webserial@1")) {
      throw new Error("browser WebSerial implementation is absent from the selected PROFILE");
    }
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
        maximumSignalOperations,
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

      async function readStream({ maximumBytes, maximumChunks, timeoutMillis, complete } = {}) {
        if (!Number.isSafeInteger(maximumBytes) || maximumBytes < 1 || maximumBytes > maximumTransferBytes) {
          throw new RangeError("serial stream byte bound is outside the admitted transfer bound");
        }
        if (!Number.isSafeInteger(maximumChunks) || maximumChunks < 1 || maximumChunks > maximumBytes) {
          throw new RangeError("serial stream chunk bound must fit its finite byte bound");
        }
        if (!Number.isSafeInteger(timeoutMillis) || timeoutMillis < 1 || timeoutMillis > 60_000) {
          throw new RangeError("serial stream time bound must be between 1 and 60000 milliseconds");
        }
        if (typeof complete !== "function") {
          throw new TypeError("serial stream completion predicate is required");
        }
        let chunks = 0;
        const result = await transfer(READ, async () => {
          if (!acquiredPort.readable) throw new DOMException("serial port is not readable", "NetworkError");
          const reader = acquiredPort.readable.getReader();
          const retained = new Uint8Array(maximumBytes);
          let retainedBytes = 0;
          const deadline = Date.now() + timeoutMillis;
          try {
            while (chunks < maximumChunks) {
              const remainingMillis = deadline - Date.now();
              if (remainingMillis <= 0) throw streamRefusal("StreamTimeout", "serial stream exceeded its admitted time bound");
              const { value, done } = await readBefore(reader, remainingMillis);
              if (done || !value?.byteLength) throw new DOMException("serial port closed before a value", "AbortError");
              if (retainedBytes + value.byteLength > retained.byteLength) {
                throw streamRefusal("StreamByteBound", "serial stream exceeds its admitted byte bound");
              }
              retained.set(value, retainedBytes);
              retainedBytes += value.byteLength;
              chunks += 1;
              if (complete(retained.subarray(0, retainedBytes)) === true) {
                return retained.slice(0, retainedBytes);
              }
            }
            throw streamRefusal("StreamChunkBound", "serial stream exceeded its admitted browser-chunk bound");
          } finally {
            reader.releaseLock();
          }
        });
        return Object.freeze({ ...result, chunks });
      }

      async function setSignals({ dataTerminalReady, requestToSend, break: breakSignal } = {}) {
        if (!useStarted || terminal || port !== acquiredPort) {
          throw new Error("browser serial use Plan is not active");
        }
        const values = [dataTerminalReady, requestToSend, breakSignal].filter((value) => value !== undefined);
        if (values.length === 0 || values.some((value) => typeof value !== "boolean")) {
          throw new TypeError("serial modem-control operation requires one or more exact boolean signals");
        }
        requireStatus(api.conduit_browser_serial_begin_transfer(SIGNALS), "signal admission");
        try {
          if (!acquiredPort.setSignals) {
            throw new DOMException("serial modem-control signals are unavailable", "NotSupportedError");
          }
          const signals = {};
          if (dataTerminalReady !== undefined) signals.dataTerminalReady = dataTerminalReady;
          if (requestToSend !== undefined) signals.requestToSend = requestToSend;
          if (breakSignal !== undefined) signals.break = breakSignal;
          await acquiredPort.setSignals(signals);
          requireStatus(api.conduit_browser_serial_complete_transfer(SIGNALS, 0), "signal completion");
          const receipt = publish();
          requireStatus(api.conduit_browser_serial_release_transfer(), "signal release");
          publish();
          return { signals: Object.freeze({ ...signals }), receipt };
        } catch (error) {
          terminal = true;
          requireStatus(api.conduit_browser_serial_transfer_failed(2), "signal failure");
          await acquiredPort.close?.().catch(() => {});
          port = null;
          publish();
          throw error;
        }
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
        readStream,
        setSignals,
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

  function readBefore(reader, timeoutMillis) {
    return new Promise((resolve, reject) => {
      let settled = false;
      const timeout = setTimeout(async () => {
        if (settled) return;
        settled = true;
        try {
          await reader.cancel?.();
        } catch {}
        reject(streamRefusal("StreamTimeout", "serial stream exceeded its admitted time bound"));
      }, timeoutMillis);
      reader.read().then((result) => {
        if (settled) return;
        settled = true;
        clearTimeout(timeout);
        resolve(result);
      }, (error) => {
        if (settled) return;
        settled = true;
        clearTimeout(timeout);
        reject(error);
      });
    });
  }

  function streamRefusal(code, message) {
    const error = new Error(message);
    error.name = "SerialStreamRefusal";
    error.code = code;
    return error;
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
