import {
  completionBytes,
  decodeEffect,
  successfulCompletion,
} from "./signal-wasm-runtime.mjs";

const OUTPUT_SESSION = 1;
const OUTPUT_PRESENT = 3;
const STATUS_COMPLETE = 1;
const SESSION_ACCEPTED = 4;
const SESSION_DELIVERED = 5;
const FRAME_CAPACITY = 4096;

function writeInput(runtime, bytes) {
  if (!(bytes instanceof Uint8Array) || bytes.length > FRAME_CAPACITY) {
    throw new Error("CND-DST-S4-001 invalid distributed input frame");
  }
  new Uint8Array(
    runtime.api.memory.buffer,
    runtime.api.conduit_browser_distributed_input_ptr(),
    bytes.length,
  ).set(bytes);
}

export async function instantiateDistributedBrowserRuntime(
  wasmBytes,
  { triple = false, sourceIdentity = null } = {},
) {
  const { instance } = await WebAssembly.instantiate(wasmBytes, {});
  const api = instance.exports;
  const required = [
    "memory",
    "conduit_browser_distributed_start",
    "conduit_browser_distributed_configure_source",
    "conduit_browser_distributed_status",
    "conduit_browser_distributed_output_kind",
    "conduit_browser_distributed_output_ptr",
    "conduit_browser_distributed_output_len",
    "conduit_browser_distributed_input_ptr",
    "conduit_browser_distributed_input_capacity",
    "conduit_browser_distributed_ingest",
    "conduit_browser_distributed_advance",
    "conduit_browser_distributed_clear_output",
    "conduit_browser_distributed_complete",
    "conduit_browser_distributed_cancel",
    "conduit_browser_distributed_receipt_count",
    "conduit_browser_distributed_pressure_retries",
    "conduit_browser_distributed_capacity_stable",
    "conduit_browser_distributed_retained_values",
    "conduit_browser_distributed_in_flight_items",
  ];
  if (required.some((name) => !(name in api)) ||
      (triple && !("conduit_browser_triple_start" in api)) ||
      api.conduit_browser_distributed_input_capacity() !== FRAME_CAPACITY) {
    throw new Error("CND-DST-S4-002 incomplete distributed WASM ABI");
  }
  if (sourceIdentity !== null) {
    const host = new TextEncoder().encode(sourceIdentity.hostId);
    const boot = new TextEncoder().encode(sourceIdentity.bootId);
    if (host.length === 0 || boot.length === 0 || host.length + boot.length > FRAME_CAPACITY) {
      throw new Error("CND-DST-S4-008 invalid distributed source identity");
    }
    const input = new Uint8Array(
      api.memory.buffer,
      api.conduit_browser_distributed_input_ptr(),
      host.length + boot.length,
    );
    input.set(host, 0);
    input.set(boot, host.length);
    if (api.conduit_browser_distributed_configure_source(host.length, boot.length) !== 0) {
      throw new Error("CND-DST-S4-009 distributed source identity rejected");
    }
  }
  const status = triple
    ? api.conduit_browser_triple_start()
    : api.conduit_browser_distributed_start();
  if (status !== 0) throw new Error(`CND-DST-S4-003 browser prepare failed ${status}`);
  return Object.freeze({ api });
}

function output(runtime) {
  const kind = runtime.api.conduit_browser_distributed_output_kind();
  if (kind === 0) return null;
  const length = runtime.api.conduit_browser_distributed_output_len();
  if (length <= 0 || length > FRAME_CAPACITY) {
    throw new Error(`CND-DST-S4-004 invalid distributed output ${length}`);
  }
  const bytes = new Uint8Array(
    runtime.api.memory.buffer,
    runtime.api.conduit_browser_distributed_output_ptr(),
    length,
  ).slice();
  return Object.freeze({ kind, bytes });
}

function requireStatus(status, operation) {
  if (status < 0) throw new Error(`CND-DST-S4-005 ${operation} failed ${status}`);
}

export async function runDistributedBrowserRuntime(runtime, line, domHost) {
  const presentations = [];
  let sessionFramesSent = 0;
  let sessionFramesReceived = 0;
  for (;;) {
    const pending = output(runtime);
    if (pending?.kind === OUTPUT_SESSION) {
      const messageKind = pending.bytes[5];
      const sent = line.sendBinary(pending.bytes);
      if (!sent.ok) throw new Error(`${sent.code} ${sent.detail}`);
      sessionFramesSent += 1;
      if (messageKind === SESSION_ACCEPTED || messageKind === SESSION_DELIVERED) {
        requireStatus(runtime.api.conduit_browser_distributed_advance(), "advance");
      } else {
        requireStatus(
          runtime.api.conduit_browser_distributed_clear_output(),
          "clear session output",
        );
      }
      continue;
    }
    if (pending?.kind === OUTPUT_PRESENT) {
      const effect = decodeEffect(pending.bytes);
      const result = domHost.completePresentation(effect);
      if (!result.ok) throw new Error(`${result.code} ${result.detail}`);
      const completion = completionBytes(effect, successfulCompletion(effect));
      writeInput(runtime, completion);
      requireStatus(
        runtime.api.conduit_browser_distributed_complete(completion.length),
        "presentation completion",
      );
      presentations.push(effect);
      continue;
    }
    if (runtime.api.conduit_browser_distributed_status() === STATUS_COMPLETE) break;
    const inbound = await line.receiveBinary();
    writeInput(runtime, inbound);
    requireStatus(
      runtime.api.conduit_browser_distributed_ingest(inbound.length),
      "session ingest",
    );
    sessionFramesReceived += 1;
  }
  return Object.freeze({
    status: runtime.api.conduit_browser_distributed_status(),
    receiptCount: runtime.api.conduit_browser_distributed_receipt_count(),
    pressureRetries: runtime.api.conduit_browser_distributed_pressure_retries(),
    capacityStable: runtime.api.conduit_browser_distributed_capacity_stable() === 1,
    retainedValues: runtime.api.conduit_browser_distributed_retained_values(),
    inFlightItems: runtime.api.conduit_browser_distributed_in_flight_items(),
    sessionFramesSent,
    sessionFramesReceived,
    presentations: Object.freeze(presentations),
  });
}
