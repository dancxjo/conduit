const EFFECT_WAIT = 1;
const EFFECT_PRESENT = 2;
const STATUS_RUNNING = 0;
const STATUS_COMPLETE = 1;
const FRAME_CAPACITY = 4_096;
const decoder = new TextDecoder("utf-8", { fatal: true });
const encoder = new TextEncoder();

class FrameReader {
  constructor(bytes) {
    this.buffer = bytes;
    this.offset = 0;
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }

  take(length) {
    if (!Number.isSafeInteger(length) || length < 0 || this.offset + length > this.buffer.length) {
      throw new Error("CND-BRW-S4-005 malformed runtime effect frame");
    }
    const value = this.buffer.slice(this.offset, this.offset + length);
    this.offset += length;
    return value;
  }

  byte() {
    return this.take(1)[0];
  }

  u16() {
    const value = this.view.getUint16(this.offset, true);
    this.take(2);
    return value;
  }

  u32() {
    const value = this.view.getUint32(this.offset, true);
    this.take(4);
    return value;
  }

  u64() {
    const value = this.view.getBigUint64(this.offset, true);
    this.take(8);
    return value;
  }

  bytes() {
    return this.take(this.u16());
  }

  text() {
    return decoder.decode(this.bytes());
  }

  finish() {
    if (this.offset !== this.buffer.length) {
      throw new Error("CND-BRW-S4-005 trailing runtime effect bytes");
    }
  }
}

class FrameWriter {
  constructor() {
    this.bytes = new Uint8Array(FRAME_CAPACITY);
    this.offset = 0;
  }

  write(value) {
    if (this.offset + value.length > this.bytes.length) {
      throw new Error("CND-BRW-S4-006 completion frame exceeds fixed capacity");
    }
    this.bytes.set(value, this.offset);
    this.offset += value.length;
  }

  byte(value) {
    this.write(Uint8Array.of(value));
  }

  bytesField(value) {
    if (!(value instanceof Uint8Array) || value.length > 0xffff) {
      throw new Error("CND-BRW-S4-006 invalid completion byte field");
    }
    const length = new Uint8Array(2);
    new DataView(length.buffer).setUint16(0, value.length, true);
    this.write(length);
    this.write(value);
  }

  text(value) {
    this.bytesField(encoder.encode(value));
  }

  finish() {
    return this.bytes.slice(0, this.offset);
  }
}

function decodeEffect(bytes) {
  const frame = new FrameReader(bytes);
  const kind = frame.byte();
  const common = {
    sourceDocumentId: frame.text(),
    checkedFormId: frame.text(),
    expandedFormId: frame.text(),
    planId: frame.text(),
    fragmentId: frame.text(),
    hostId: frame.text(),
    bootId: frame.text(),
    activePlayId: frame.text(),
    requestNode: frame.u16(),
    requestId: frame.u32(),
    operationId: frame.u16(),
    hostOperationContractId: frame.text(),
    placementId: frame.text(),
  };
  let effect;
  if (kind === EFFECT_WAIT) {
    effect = Object.freeze({
      kind,
      ...common,
      durationMs: frame.u64(),
    });
  } else if (kind === EFFECT_PRESENT) {
    effect = Object.freeze({
      kind,
      ...common,
      presentationId: frame.text(),
      evidenceId: frame.text(),
      presentationKind: frame.text(),
      value: Object.freeze({
        valueKind: frame.text(),
        encoded: Object.freeze([...frame.bytes()]),
      }),
    });
  } else {
    throw new Error(`CND-BRW-S4-005 unsupported runtime effect ${kind}`);
  }
  frame.finish();
  return effect;
}

function encodeCompletion(effect, completion) {
  const frame = new FrameWriter();
  frame.byte(effect.kind);
  frame.text(completion.sourceDocumentId);
  frame.text(completion.checkedFormId);
  frame.text(completion.expandedFormId);
  frame.text(completion.planId);
  frame.text(completion.fragmentId);
  frame.text(completion.hostId);
  frame.text(completion.bootId);
  frame.text(completion.activePlayId);
  const numeric = new Uint8Array(8);
  const view = new DataView(numeric.buffer);
  view.setUint16(0, completion.requestNode, true);
  view.setUint32(2, completion.requestId, true);
  view.setUint16(6, completion.operationId, true);
  frame.write(numeric);
  frame.text(completion.hostOperationContractId);
  frame.text(completion.placementId);
  if (effect.kind === EFFECT_PRESENT) {
    frame.text(completion.presentationId);
    frame.text(completion.evidenceId);
    frame.text(completion.value.valueKind);
    frame.bytesField(Uint8Array.from(completion.value.encoded));
  }
  frame.byte(completion.success ? 1 : 0);
  return frame.finish();
}

function commonCompletion(effect, success = true) {
  return {
    sourceDocumentId: effect.sourceDocumentId,
    checkedFormId: effect.checkedFormId,
    expandedFormId: effect.expandedFormId,
    planId: effect.planId,
    fragmentId: effect.fragmentId,
    hostId: effect.hostId,
    bootId: effect.bootId,
    activePlayId: effect.activePlayId,
    requestNode: effect.requestNode,
    requestId: effect.requestId,
    operationId: effect.operationId,
    hostOperationContractId: effect.hostOperationContractId,
    placementId: effect.placementId,
    success,
  };
}

export async function instantiateBrowserRuntime(wasmBytes, hostIndex) {
  const { instance } = await WebAssembly.instantiate(wasmBytes, {});
  const api = instance.exports;
  const required = [
    "memory",
    "conduit_browser_start",
    "conduit_browser_status",
    "conduit_browser_effect_kind",
    "conduit_browser_output_ptr",
    "conduit_browser_output_len",
    "conduit_browser_input_ptr",
    "conduit_browser_input_capacity",
    "conduit_browser_complete",
    "conduit_browser_cancel",
    "conduit_browser_receipt_count",
    "conduit_browser_capacity_stable",
    "conduit_browser_terminal_failure",
  ];
  if (required.some((name) => !(name in api))) {
    throw new Error("CND-BRW-S4-007 incomplete browser runtime ABI");
  }
  const started = api.conduit_browser_start(hostIndex);
  if (started !== STATUS_RUNNING) {
    throw new Error(`CND-BRW-S4-008 runtime start failed ${started}`);
  }
  return Object.freeze({ api, hostIndex });
}

export function currentEffect(runtime) {
  const { api } = runtime;
  const kind = api.conduit_browser_effect_kind();
  if (kind <= 0) return null;
  const pointer = api.conduit_browser_output_ptr();
  const length = api.conduit_browser_output_len();
  if (length <= 0 || length > FRAME_CAPACITY) {
    throw new Error(`CND-BRW-S4-005 invalid runtime effect length ${length}`);
  }
  const bytes = new Uint8Array(api.memory.buffer, pointer, length).slice();
  const effect = decodeEffect(bytes);
  if (effect.kind !== kind) {
    throw new Error("CND-BRW-S4-005 effect kind disagrees with runtime status");
  }
  return effect;
}

export function submitCompletion(runtime, effect, completion) {
  return submitRawCompletion(runtime, encodeCompletion(effect, completion));
}

export function submitRawCompletion(runtime, bytes) {
  const { api } = runtime;
  if (!(bytes instanceof Uint8Array)) {
    throw new TypeError("CND-BRW-S4-006 completion must be bytes");
  }
  const capacity = api.conduit_browser_input_capacity();
  if (bytes.length > capacity) {
    throw new Error("CND-BRW-S4-006 completion exceeds runtime input capacity");
  }
  new Uint8Array(api.memory.buffer, api.conduit_browser_input_ptr(), bytes.length).set(bytes);
  return api.conduit_browser_complete(bytes.length);
}

export async function runBrowserRuntime(runtime, domHost) {
  let timerCount = 0;
  let requestedTimerMs = 0;
  let completionCount = 0;
  const presentations = [];
  while (runtime.api.conduit_browser_status() === STATUS_RUNNING) {
    const effect = currentEffect(runtime);
    if (!effect) throw new Error("CND-BRW-S4-009 running runtime has no effect");
    let completion;
    if (effect.kind === EFFECT_WAIT) {
      const durationMs = Number(effect.durationMs);
      if (!Number.isSafeInteger(durationMs) || durationMs < 0) {
        throw new Error("CND-BRW-S4-005 invalid timer duration");
      }
      await new Promise((resolve) => setTimeout(resolve, durationMs));
      timerCount += 1;
      requestedTimerMs += durationMs;
      completion = Object.freeze({
        ...commonCompletion(effect),
      });
    } else {
      const result = domHost.completePresentation(effect);
      if (!result.ok) throw new Error(`${result.code} ${result.detail}`);
      completion = result.completion;
      presentations.push(effect);
      completionCount += 1;
    }
    const status = submitCompletion(runtime, effect, completion);
    if (status < 0) throw new Error(`CND-BRW-S4-010 runtime rejected completion ${status}`);
  }
  const status = runtime.api.conduit_browser_status();
  if (status !== STATUS_COMPLETE) {
    throw new Error(`CND-BRW-S4-011 runtime did not complete ${status}`);
  }
  return Object.freeze({
    status,
    timerCount,
    requestedTimerMs,
    completionCount,
    receiptCount: runtime.api.conduit_browser_receipt_count(),
    capacityStable: runtime.api.conduit_browser_capacity_stable() === 1,
    terminalFailure: runtime.api.conduit_browser_terminal_failure() === 1,
    presentations: Object.freeze(presentations),
  });
}

export function successfulCompletion(effect) {
  const completion = commonCompletion(effect);
  if (effect.kind === EFFECT_PRESENT) {
    completion.presentationId = effect.presentationId;
    completion.evidenceId = effect.evidenceId;
    completion.value = effect.value;
  }
  return Object.freeze(completion);
}

export function failedCompletion(effect) {
  const completion = { ...successfulCompletion(effect), success: false };
  return Object.freeze(completion);
}

export function completionBytes(effect, completion) {
  return encodeCompletion(effect, completion);
}
