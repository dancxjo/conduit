const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const ABI = Object.freeze({
  identity: "conduit.browser/runtime-abi",
  revision: 1,
});

const ABI_IDENTITY_BYTES = encoder.encode(ABI.identity);
const OPERATION_OUTPUT_MAX = 256 * 1024;

function exported(api, name, context) {
  const value = api?.[name];
  if (typeof value !== "function") throw new Error(`${context} missing export ${name}`);
  return value;
}

function nonnegativeInteger(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) throw new Error(`${label} exceeds its bound`);
  return value;
}

function boundedLength(length, { minimum = 0, maximum, label }) {
  if (!Number.isSafeInteger(length) || length < minimum || length > maximum) {
    throw new Error(`${label} exceeds its bound`);
  }
}

function boundedPointer(buffer, pointer, length, label) {
  const start = nonnegativeInteger(pointer, label);
  const end = start + length;
  if (!Number.isSafeInteger(end) || end > buffer.byteLength) {
    throw new Error(`${label} exceeds its bound`);
  }
  return start;
}

function readLength(api, lengthExport, label) {
  return nonnegativeInteger(Number(exported(api, lengthExport, label).call(api)), label);
}

function readCapacity(api, capacityExport, label) {
  return nonnegativeInteger(Number(exported(api, capacityExport, label).call(api)), label);
}

function readPointer(api, pointerExport, label) {
  return nonnegativeInteger(Number(exported(api, pointerExport, label).call(api)), label);
}

function readBytes(api, { pointerExport, lengthExport, capacityExport = null, minimum = 0, maximum = OPERATION_OUTPUT_MAX, label }) {
  const length = readLength(api, lengthExport, label);
  const capacity = capacityExport ? readCapacity(api, capacityExport, label) : maximum;
  boundedLength(length, { minimum, maximum: Math.min(maximum, capacity), label });
  const pointer = readPointer(api, pointerExport, label);
  const start = boundedPointer(api.memory.buffer, pointer, length, label);
  return new Uint8Array(api.memory.buffer, start, length).slice();
}

function withInput(api, bytes, { pointerExport, capacityExport, minimum = 0, label }, invoke, { retireInput = false } = {}) {
  if (!(bytes instanceof Uint8Array)) throw new TypeError(`${label} must be encoded bytes`);
  const capacity = readCapacity(api, capacityExport, label);
  boundedLength(bytes.length, { minimum, maximum: capacity, label });
  const pointer = readPointer(api, pointerExport, label);
  const start = boundedPointer(api.memory.buffer, pointer, bytes.length, label);
  new Uint8Array(api.memory.buffer, start, bytes.length).set(bytes);
  try {
    return invoke(bytes.length);
  } finally {
    if (retireInput) {
      const activeStart = boundedPointer(api.memory.buffer, pointer, bytes.length, label);
      new Uint8Array(api.memory.buffer, activeStart, bytes.length).fill(0);
    }
  }
}

function readJson(api, fields) {
  try {
    return decodeJson(readBytes(api, fields), fields.label);
  } catch (error) {
    if (error instanceof Error && error.message.startsWith(`${fields.label} `)) throw error;
    throw new Error(`${fields.label} is not valid bounded UTF-8 JSON: ${error instanceof Error ? error.message : String(error)}`);
  }
}

function decodeJson(bytes, label) {
  let source;
  try { source = decoder.decode(bytes); }
  catch (error) { throw new Error(`${label} is not valid UTF-8: ${error instanceof Error ? error.message : String(error)}`); }
  try { return JSON.parse(source); }
  catch (error) { throw new Error(`${label} is not valid JSON: ${error instanceof Error ? error.message : String(error)}`); }
}

function maybeReadFormOutputJson(api, minimum = 0) {
  const length = readLength(api, "conduit_browser_form_output_len", "browser Body output");
  if (length < minimum || length > OPERATION_OUTPUT_MAX) throw new Error("browser Body output exceeds its bound");
  if (length === 0) return null;
  return readJson(api, {
    pointerExport: "conduit_browser_form_output_ptr",
    lengthExport: "conduit_browser_form_output_len",
    minimum,
    maximum: OPERATION_OUTPUT_MAX,
    label: "browser Body output",
  });
}

function requireAbiIdentity(api, context) {
  const pointer = readPointer(api, "conduit_browser_runtime_abi_identity_ptr", context);
  const length = readLength(api, "conduit_browser_runtime_abi_identity_len", context);
  boundedLength(length, { minimum: 1, maximum: 128, label: `${context} runtime ABI identity` });
  const start = boundedPointer(api.memory.buffer, pointer, length, `${context} runtime ABI identity`);
  const runtimeIdentity = new Uint8Array(api.memory.buffer, start, length);
  if (runtimeIdentity.length !== ABI_IDENTITY_BYTES.length
      || runtimeIdentity.some((byte, index) => byte !== ABI_IDENTITY_BYTES[index])) {
    throw Object.assign(new Error(`${context} requires ${ABI.identity}@${ABI.revision}`), {
      code: "IncompatibleRuntimeAbi",
      required_runtime_abi: `${ABI.identity}@${ABI.revision}`,
      runtime_abi_revision: null,
      supported_runtime_abi_revisions: [ABI.revision],
    });
  }
}

function requireAbiRevision(api, context) {
  const abiExport = api?.conduit_browser_runtime_abi_revision;
  const abiRevision = typeof abiExport === "function" ? Number(abiExport.call(api)) : 0;
  if (!Number.isSafeInteger(abiRevision) || abiRevision < 0) {
    throw new Error(`${context} has a malformed runtime ABI revision export`);
  }
  if (abiRevision !== ABI.revision) {
    throw Object.assign(new Error(`${context} requires ${ABI.identity}@${ABI.revision}; runtime reported @${abiRevision}`), {
      code: "IncompatibleRuntimeAbi",
      required_runtime_abi: `${ABI.identity}@${ABI.revision}`,
      runtime_abi_revision: abiRevision,
      supported_runtime_abi_revisions: [ABI.revision],
    });
  }
}

export function bindBrowserRuntimeBridge(api, { context }) {
  if (!(api?.memory instanceof WebAssembly.Memory)) {
    throw new Error(`${context} requires WebAssembly linear memory`);
  }
  requireAbiIdentity(api, context);
  requireAbiRevision(api, context);
  const call = (name, ...args) => exported(api, name, context).call(api, ...args);
  return Object.freeze({
    abi: ABI,
    encodeJson: (value) => encoder.encode(JSON.stringify(value)),
    decodeJson: (bytes) => JSON.parse(decoder.decode(bytes)),
    workspaceRequest(message, { binary = false } = {}) {
      const input = encoder.encode(JSON.stringify(message));
      return withInput(api, input, {
        pointerExport: "conduit_workspace_input_ptr",
        capacityExport: "conduit_workspace_input_capacity",
        label: "Workspace input",
      }, (length) => {
        const status = call("conduit_workspace_request", length);
        const outputLength = readLength(api, "conduit_workspace_output_len", "Workspace output");
        if (status >= 0 || outputLength > 0) {
          const outputBytes = readBytes(api, {
            pointerExport: "conduit_workspace_output_ptr",
            lengthExport: "conduit_workspace_output_len",
            capacityExport: "conduit_workspace_output_capacity",
            minimum: 0,
            maximum: OPERATION_OUTPUT_MAX,
            label: "Workspace output",
          });
          return { status, outputBytes, outputJson: !binary && outputBytes.length > 0 ? decodeJson(outputBytes, "Workspace output") : null };
        }
        return { status, outputBytes: null, outputJson: null };
      }, { retireInput: true });
    },
    crecheCurrent() {
      const status = call("conduit_creche_current");
      return { status, outputJson: status === 1 ? null : readJson(api, {
        pointerExport: "conduit_creche_output_ptr",
        lengthExport: "conduit_creche_output_len",
        minimum: 1,
        maximum: 65536,
        label: "Body output",
      }) };
    },
    crecheReviewedInventory(source) {
      const input = encoder.encode(source);
      return withInput(api, input, {
        pointerExport: "conduit_creche_input_ptr",
        capacityExport: "conduit_creche_input_capacity",
        label: "Form source",
      }, (length) => {
        const status = call("conduit_creche_reviewed_inventory", length);
        return { status, outputJson: readJson(api, {
          pointerExport: "conduit_creche_output_ptr",
          lengthExport: "conduit_creche_output_len",
          minimum: 1,
          maximum: 32 * 1024,
          label: "Form check output",
        }) };
      }, { retireInput: true });
    },
    crecheReviewInitialWorkload({ host, boot, initialForms, source }) {
      const parts = [host, boot, JSON.stringify(initialForms), source].map((value) => encoder.encode(value));
      const input = new Uint8Array(parts.reduce((total, part) => total + part.length, 0));
      let offset = 0;
      for (const part of parts) { input.set(part, offset); offset += part.length; }
      return withInput(api, input, {
        pointerExport: "conduit_creche_input_ptr",
        capacityExport: "conduit_creche_input_capacity",
        minimum: 1,
        label: "Form realization review input",
      }, () => {
        const status = call("conduit_creche_review_initial_workload", parts[0].length, parts[1].length, parts[2].length, parts[3].length);
        return { status, outputJson: readJson(api, {
          pointerExport: "conduit_creche_output_ptr",
          lengthExport: "conduit_creche_output_len",
          minimum: 1,
          maximum: 32 * 1024,
          label: "Form realization review output",
        }) };
      }, { retireInput: true });
    },
    crecheAdmitSourceInteraction(source, sequence) {
      const input = encoder.encode(source);
      return withInput(api, input, {
        pointerExport: "conduit_creche_input_ptr",
        capacityExport: "conduit_creche_input_capacity",
        label: "Form source interaction",
      }, (length) => {
        const status = call("conduit_creche_admit_source_interaction", length, sequence);
        return { status, outputJson: readJson(api, {
          pointerExport: "conduit_creche_output_ptr",
          lengthExport: "conduit_creche_output_len",
          minimum: 1,
          maximum: 32 * 1024,
          label: "Form source interaction output",
        }) };
      }, { retireInput: true });
    },
    crecheBirth({ host, boot, friendlyName, initialForms, source, sequence }) {
      const parts = [host, boot, friendlyName, JSON.stringify(initialForms), source].map((value) => encoder.encode(value));
      const input = new Uint8Array(parts.reduce((total, part) => total + part.length, 0));
      let offset = 0;
      for (const part of parts) { input.set(part, offset); offset += part.length; }
      return withInput(api, input, {
        pointerExport: "conduit_creche_input_ptr",
        capacityExport: "conduit_creche_input_capacity",
        minimum: 1,
        label: "Body birth input",
      }, () => {
        const status = call("conduit_creche_birth", parts[0].length, parts[1].length, parts[2].length, parts[3].length, parts[4].length, sequence);
        return { status, outputJson: readJson(api, {
          pointerExport: "conduit_creche_output_ptr",
          lengthExport: "conduit_creche_output_len",
          minimum: 1,
          maximum: 32 * 1024,
          label: "Body birth output",
        }) };
      }, { retireInput: true });
    },
    crecheDurableSnapshot() {
      const status = call("conduit_creche_durable_snapshot");
      return { status, outputJson: status === 1 ? null : readJson(api, {
        pointerExport: "conduit_creche_output_ptr",
        lengthExport: "conduit_creche_output_len",
        minimum: 1,
        maximum: 65536,
        label: "Body output",
      }) };
    },
    crecheAttachHere(hostBytes, bootBytes, sequence) {
      const input = new Uint8Array(hostBytes.length + bootBytes.length);
      input.set(hostBytes);
      input.set(bootBytes, hostBytes.length);
      return withInput(api, input, {
        pointerExport: "conduit_creche_input_ptr",
        capacityExport: "conduit_creche_input_capacity",
        minimum: 1,
        label: "Body input",
      }, () => {
        const status = call("conduit_creche_attach_here", hostBytes.length, bootBytes.length, sequence);
        return { status, outputJson: status === 1 ? null : readJson(api, {
        pointerExport: "conduit_creche_output_ptr",
        lengthExport: "conduit_creche_output_len",
        minimum: 1,
        maximum: 65536,
        label: "Body output",
        }) };
      }, { retireInput: true });
    },
    crecheRestoreDurable(snapshotBytes) {
      return withInput(api, snapshotBytes, {
        pointerExport: "conduit_creche_input_ptr",
        capacityExport: "conduit_creche_input_capacity",
        minimum: 1,
        label: "Body input",
      }, (length) => {
        const status = call("conduit_creche_restore_durable", length);
        return { status, outputJson: status === 1 ? null : readJson(api, {
        pointerExport: "conduit_creche_output_ptr",
        lengthExport: "conduit_creche_output_len",
        minimum: 1,
        maximum: 65536,
        label: "Body output",
        }) };
      }, { retireInput: true });
    },
    browserFormReadOutputJson() {
      return readJson(api, {
        pointerExport: "conduit_browser_form_output_ptr",
        lengthExport: "conduit_browser_form_output_len",
        minimum: 1,
        maximum: OPERATION_OUTPUT_MAX,
        label: "browser Body output",
      });
    },
    browserFormReadOutputBytes() {
      return readBytes(api, {
        pointerExport: "conduit_browser_form_output_ptr",
        lengthExport: "conduit_browser_form_output_len",
        minimum: 1,
        maximum: OPERATION_OUTPUT_MAX,
        label: "browser Body output",
      });
    },
    browserBodyStart(startRequest) {
      return withInput(api, encoder.encode(JSON.stringify(startRequest)), {
        pointerExport: "conduit_browser_body_input_ptr",
        capacityExport: "conduit_browser_body_input_capacity",
        minimum: 1,
        label: "browser Body input",
      }, (length) => {
        const status = call("conduit_browser_body_start", length);
        return { status, outputJson: maybeReadFormOutputJson(api, status >= 0 ? 1 : 0) };
      }, { retireInput: true });
    },
    browserFormAcknowledgeCancellation(activePlayId, placementId, sequence) {
      const play = encoder.encode(activePlayId);
      const placement = encoder.encode(placementId);
      const bytes = new Uint8Array(play.length + placement.length);
      bytes.set(play);
      bytes.set(placement, play.length);
      return withInput(api, bytes, {
        pointerExport: "conduit_browser_form_input_ptr",
        capacityExport: "conduit_browser_form_input_capacity",
        label: "cancellation acknowledgement input",
      }, () => {
        const status = call("conduit_browser_form_acknowledge_cancellation", play.length, placement.length, sequence);
        return { status, outputJson: maybeReadFormOutputJson(api, status >= 0 ? 1 : 0) };
      }, { retireInput: true });
    },
    browserFormCompleteEffect(activePlayId, placementId, sequence, effectOutputBytes) {
      const play = encoder.encode(activePlayId);
      const placement = encoder.encode(placementId);
      const bytes = new Uint8Array(play.length + placement.length + effectOutputBytes.length);
      bytes.set(play);
      bytes.set(placement, play.length);
      bytes.set(effectOutputBytes, play.length + placement.length);
      return withInput(api, bytes, {
        pointerExport: "conduit_browser_form_input_ptr",
        capacityExport: "conduit_browser_form_input_capacity",
        label: "effect completion input",
      }, () => {
        const status = call("conduit_browser_form_complete_effect", play.length, placement.length, sequence, effectOutputBytes.length);
        return { status, outputJson: maybeReadFormOutputJson(api, status >= 0 ? 1 : 0) };
      }, { retireInput: true });
    },
    browserFormRefuseEffect(activePlayId, placementId, sequence, disposition, detail) {
      const play = encoder.encode(activePlayId);
      const placement = encoder.encode(placementId);
      const bytes = new Uint8Array(play.length + placement.length);
      bytes.set(play);
      bytes.set(placement, play.length);
      return withInput(api, bytes, {
        pointerExport: "conduit_browser_form_input_ptr",
        capacityExport: "conduit_browser_form_input_capacity",
        label: "effect refusal input",
      }, () => {
        const status = call("conduit_browser_form_refuse_effect", play.length, placement.length, sequence, disposition, detail);
        return { status, outputJson: maybeReadFormOutputJson(api, status >= 0 ? 1 : 0) };
      }, { retireInput: true });
    },
  });
}
