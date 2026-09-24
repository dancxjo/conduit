const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const ABI = Object.freeze({
  identity: "conduit.browser/runtime-abi",
  revision: 1,
});

function exported(api, name, context) {
  const value = api?.[name];
  if (typeof value !== "function") throw new Error(`${context} missing export ${name}`);
  return value;
}

function boundedLength(length, { minimum, maximum, label }) {
  if (!Number.isSafeInteger(length) || length < minimum || length > maximum) {
    throw new Error(`${label} exceeds its bound`);
  }
}

function boundedView(buffer, pointer, length, label) {
  if (!Number.isSafeInteger(pointer) || pointer < 0 || pointer + length > buffer.byteLength) {
    throw new Error(`${label} exceeds its bound`);
  }
}

function writeInput(api, bytes, { pointerExport, capacityExport, minimum = 0, label }) {
  if (!(bytes instanceof Uint8Array)) throw new TypeError(`${label} must be encoded bytes`);
  const capacity = Number(exported(api, capacityExport, label).call(api));
  boundedLength(bytes.length, { minimum, maximum: capacity, label });
  const pointer = Number(exported(api, pointerExport, label).call(api));
  boundedView(api.memory.buffer, pointer, bytes.length, label);
  const view = new Uint8Array(api.memory.buffer, pointer, bytes.length);
  view.set(bytes);
  return view;
}

function readOutputBytes(api, { pointerExport, lengthExport, capacityExport = null, minimum = 1, maximum, label }) {
  const length = Number(exported(api, lengthExport, label).call(api));
  const bound = capacityExport
    ? Number(exported(api, capacityExport, label).call(api))
    : maximum;
  boundedLength(length, { minimum, maximum: Math.min(maximum, bound), label });
  const pointer = Number(exported(api, pointerExport, label).call(api));
  boundedView(api.memory.buffer, pointer, length, label);
  return new Uint8Array(api.memory.buffer, pointer, length).slice();
}

export function bindBrowserRuntimeBridge(api, { context, requiredExports = [] }) {
  if (!(api?.memory instanceof WebAssembly.Memory)) {
    throw new Error(`${context} requires WebAssembly linear memory`);
  }
  const abiRevision = exported(api, "conduit_browser_runtime_abi_revision", context).call(api);
  if (abiRevision !== ABI.revision) {
    throw Object.assign(new Error(`${context} requires ${ABI.identity}@${ABI.revision}; runtime reported @${abiRevision}`), {
      code: "IncompatibleRuntimeAbi",
      required_runtime_abi: `${ABI.identity}@${ABI.revision}`,
      runtime_abi_revision: abiRevision,
    });
  }
  if (requiredExports.some((name) => typeof api[name] !== "function")) {
    throw new Error(`${context} has an incomplete runtime ABI surface`);
  }
  let lifecycleStarted = false;
  return Object.freeze({
    abi: ABI,
    encodeJson: (value) => encoder.encode(JSON.stringify(value)),
    decodeJson: (bytes) => JSON.parse(decoder.decode(bytes)),
    writeInput: (bytes, fields) => writeInput(api, bytes, fields),
    readOutputBytes: (fields) => readOutputBytes(api, fields),
    transact({ inputBytes, input, output = null, invoke, retireInput = false }) {
      const written = writeInput(api, inputBytes, input);
      try {
        const status = invoke(inputBytes.length);
        const outputBytes = output ? readOutputBytes(api, output) : null;
        return { status, outputBytes };
      } finally {
        if (retireInput) written.fill(0);
      }
    },
    start({ inputBytes, input, output = null, invoke, retireInput = false, label = "runtime" }) {
      if (lifecycleStarted) throw new Error(`${label} duplicate start refused`);
      const written = writeInput(api, inputBytes, input);
      let status;
      let outputBytes = null;
      try {
        status = invoke(inputBytes.length);
        outputBytes = output ? readOutputBytes(api, output) : null;
      } finally {
        if (retireInput) written.fill(0);
      }
      const result = { status, outputBytes };
      if (result.status >= 0) lifecycleStarted = true;
      return result;
    },
  });
}
