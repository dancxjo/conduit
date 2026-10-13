const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const MAXIMUM_CANDIDATE_BYTES = 16 * 1024;
const MAXIMUM_PENDING_MESSAGES = 2;
const RELAY_IMPLEMENTATION = "conduit.relay/opaque-two-endpoint@1";

const requiredApi = [
  "conduit_browser_protected_line_input_ptr",
  "conduit_browser_protected_line_input_capacity",
  "conduit_browser_protected_line_output_ptr",
  "conduit_browser_protected_line_output_len",
  "conduit_browser_relay_candidate_validate",
  "conduit_browser_protected_line_initialize",
  "conduit_browser_protected_line_write_handshake",
  "conduit_browser_protected_line_read_handshake",
  "conduit_browser_protected_line_seal",
  "conduit_browser_protected_line_open",
  "conduit_browser_protected_line_close",
];

export class BrowserRelayError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "BrowserRelayError";
    this.code = code;
  }
}

export async function openBrowserRelayLine({
  api,
  candidate,
  ephemeralPrivateKey,
  WebSocketType = globalThis.WebSocket,
  nowMillis = Date.now(),
}) {
  requireApi(api);
  if (typeof WebSocketType !== "function" || !Number.isSafeInteger(nowMillis) || nowMillis < 0) {
    throw refusal("invalid-candidate", "browser relay inputs are invalid");
  }
  const relayCapability = exactSecret(candidate?.relay_capability, "relay capability");
  const protectedPsk = exactSecret(candidate?.protected_session_psk, "protected-session PSK");
  const ephemeral = exactSecret(ephemeralPrivateKey, "ephemeral private key");
  const normalized = normalizeCandidate(candidate, relayCapability, protectedPsk);
  const candidateBytes = encoder.encode(JSON.stringify(normalized));
  if (candidateBytes.byteLength === 0 || candidateBytes.byteLength > MAXIMUM_CANDIDATE_BYTES) {
    erase(relayCapability, protectedPsk, ephemeral);
    throw refusal("invalid-candidate", "relay candidate exceeds its serialized bound");
  }
  writeInput(api, candidateBytes);
  const candidateStatus = api.conduit_browser_relay_candidate_validate(
    candidateBytes.byteLength,
    low(nowMillis),
    high(nowMillis),
  );
  candidateBytes.fill(0);
  if (candidateStatus !== 0) {
    erase(relayCapability, protectedPsk, ephemeral);
    throw refusal("invalid-candidate", `portable relay candidate refused (${candidateStatus})`);
  }

  let socket;
  try {
    socket = new WebSocketType(normalized.relay_locator);
  } catch (error) {
    erase(relayCapability, protectedPsk, ephemeral);
    throw refusal("relay-unreachable", `cannot open relay WebSocket: ${error}`);
  }
  socket.binaryType = "arraybuffer";
  const messages = boundedMessages(socket);
  try {
    await socketOpened(socket, normalized.bounds.attempt_timeout_millis);
    const attachment = encoder.encode(JSON.stringify({
      schema: "conduit.relay/attach@1",
      route_id: normalized.route_id,
      role: normalized.role === "initiator" ? "first" : "second",
      endpoint_binding: normalized.endpoint_binding,
      capability: [...relayCapability],
    }));
    socket.send(attachment);
    attachment.fill(0);
    relayCapability.fill(0);
    eraseCandidateSecret(candidate?.relay_capability);
    await awaitPair(messages, normalized);

    const binding = encoder.encode(JSON.stringify(normalized.session_binding));
    const material = new Uint8Array(binding.byteLength + 64);
    material.set(binding);
    material.set(protectedPsk, binding.byteLength);
    material.set(ephemeral, binding.byteLength + 32);
    writeInput(api, material);
    const role = normalized.role === "initiator" ? 0 : 1;
    const limits = normalized.bounds;
    const initialized = api.conduit_browser_protected_line_initialize(
      role,
      binding.byteLength,
      limits.maximum_payload_bytes,
      low(limits.maximum_frames_per_direction),
      high(limits.maximum_frames_per_direction),
      low(limits.maximum_bytes_per_direction),
      high(limits.maximum_bytes_per_direction),
    );
    material.fill(0);
    binding.fill(0);
    erase(protectedPsk, ephemeral);
    eraseCandidateSecret(candidate?.protected_session_psk);
    eraseCandidateSecret(ephemeralPrivateKey);
    if (initialized !== 0) {
      throw refusal("end-to-end-authentication", `protected relay session refused (${initialized})`);
    }
    await handshake(api, socket, messages, normalized, role);
  } catch (error) {
    erase(relayCapability, protectedPsk, ephemeral);
    messages.close();
    try { socket.close(); } catch {}
    throw error;
  }

  let closed = false;
  return Object.freeze({
    schema: "conduit.creche/joined-host-line@1",
    implementationId: "conduit-line/user-operated-protected-relay@1",
    candidateId: normalized.route_id,
    async sendSessionFrame(frame) {
      current();
      const plaintext = boundedFrame(frame, normalized.bounds.maximum_payload_bytes, "session frame");
      writeInput(api, plaintext);
      const status = api.conduit_browser_protected_line_seal(plaintext.byteLength);
      if (status !== 0) throw refusal("protected-frame", `seal refused (${status})`);
      socket.send(encodeEnvelope(normalized.route_id, readOutput(api)));
    },
    async receiveSessionFrame() {
      current();
      const frame = await receiveProtected(messages, normalized);
      writeInput(api, frame);
      const status = api.conduit_browser_protected_line_open(frame.byteLength);
      if (status !== 0) throw refusal("protected-frame", `open refused (${status})`);
      return readOutput(api);
    },
    close() {
      if (closed) return;
      closed = true;
      try {
        socket.send(encoder.encode(JSON.stringify({ schema: "conduit.relay/control@1", kind: "close" })));
        api.conduit_browser_protected_line_close();
      } finally {
        messages.close();
        socket.close();
      }
    },
  });

  function current() {
    if (closed) throw refusal("closed", "browser relay Line is closed");
  }
}

function requireApi(api) {
  if (!api?.memory || requiredApi.some((name) => typeof api[name] !== "function")) {
    throw refusal("runtime-unavailable", "browser protected-Line runtime is incomplete");
  }
}

function normalizeCandidate(candidate, relayCapability, protectedPsk) {
  if (!candidate || !candidate.bounds || !candidate.session_binding ||
      !["initiator", "responder"].includes(candidate.role)) {
    throw refusal("invalid-candidate", "browser relay candidate is incomplete");
  }
  return {
    ...candidate,
    certificate_binding_sha256: [...boundedBytes(candidate.certificate_binding_sha256, 32, "certificate binding")],
    relay_capability: [...relayCapability],
    protected_session_psk: [...protectedPsk],
    bounds: { ...candidate.bounds },
    session_binding: {
      ...candidate.session_binding,
      initiator: { ...candidate.session_binding.initiator },
      responder: { ...candidate.session_binding.responder },
    },
  };
}

function exactSecret(value, label) {
  const bytes = boundedBytes(value, 32, label);
  if (bytes.byteLength !== 32) throw refusal("invalid-candidate", `${label} must contain exactly 32 bytes`);
  return bytes.slice();
}

function boundedBytes(value, maximum, label) {
  const bytes = value instanceof Uint8Array ? value : Uint8Array.from(value ?? []);
  if (bytes.byteLength === 0 || bytes.byteLength > maximum) {
    throw refusal("invalid-candidate", `${label} violates its byte bound`);
  }
  return bytes;
}

function boundedFrame(value, maximum, label) {
  return boundedBytes(value, maximum, label);
}

function writeInput(api, bytes) {
  if (bytes.byteLength > api.conduit_browser_protected_line_input_capacity()) {
    throw refusal("pressure", "browser protected-Line input exceeds its admitted arena");
  }
  new Uint8Array(api.memory.buffer, api.conduit_browser_protected_line_input_ptr(), bytes.byteLength).set(bytes);
}

function readOutput(api) {
  const length = api.conduit_browser_protected_line_output_len();
  if (!Number.isSafeInteger(length) || length < 0 || length > 65_600) {
    throw refusal("runtime", "browser protected-Line output violates its bound");
  }
  return new Uint8Array(
    api.memory.buffer,
    api.conduit_browser_protected_line_output_ptr(),
    length,
  ).slice();
}

async function handshake(api, socket, messages, candidate, role) {
  if (role === 0) {
    const status = api.conduit_browser_protected_line_write_handshake();
    if (status !== 0) throw refusal("end-to-end-authentication", `initiator handshake write refused (${status})`);
    socket.send(encodeEnvelope(candidate.route_id, readOutput(api)));
    const response = await receiveProtected(messages, candidate);
    writeInput(api, response);
    const read = api.conduit_browser_protected_line_read_handshake(response.byteLength);
    if (read !== 0) throw refusal("end-to-end-authentication", `initiator handshake read refused (${read})`);
  } else {
    const request = await receiveProtected(messages, candidate);
    writeInput(api, request);
    const read = api.conduit_browser_protected_line_read_handshake(request.byteLength);
    if (read !== 0) throw refusal("end-to-end-authentication", `responder handshake read refused (${read})`);
    const status = api.conduit_browser_protected_line_write_handshake();
    if (status !== 0) throw refusal("end-to-end-authentication", `responder handshake write refused (${status})`);
    socket.send(encodeEnvelope(candidate.route_id, readOutput(api)));
  }
}

async function awaitPair(messages, candidate) {
  for (let count = 0; count < 2; count += 1) {
    const bytes = await messages.receive(candidate.bounds.attempt_timeout_millis);
    const outcome = decodeOutcome(bytes, candidate.route_id);
    if (outcome.status === "paired") return;
    if (outcome.status !== "waiting-for-peer") throw outcomeRefusal(outcome);
  }
  throw refusal("protocol", "relay did not pair within its bounded control exchange");
}

async function receiveProtected(messages, candidate) {
  const bytes = await messages.receive(candidate.bounds.idle_timeout_millis);
  if (!magic(bytes)) throw outcomeRefusal(decodeOutcome(bytes, candidate.route_id));
  return decodeEnvelope(bytes, candidate.route_id, candidate.bounds.maximum_protected_frame_bytes);
}

function boundedMessages(socket) {
  const queue = [];
  let waiter = null;
  let terminal = null;
  const deliver = async (event) => {
    try {
      const bytes = await messageBytes(event.data);
      if (waiter) {
        const current = waiter;
        waiter = null;
        current.resolve(bytes);
      } else if (queue.length < MAXIMUM_PENDING_MESSAGES) {
        queue.push(bytes);
      } else {
        terminal = refusal("pressure", "browser relay message queue is full");
        socket.close();
      }
    } catch (error) {
      terminal = error;
      waiter?.reject(error);
      waiter = null;
    }
  };
  socket.addEventListener("message", deliver);
  socket.addEventListener("error", () => fail("relay-transport", "browser relay transport failed"));
  socket.addEventListener("close", () => fail("relay-lost", "browser relay connection closed"));
  function fail(code, detail) {
    if (!terminal) terminal = refusal(code, detail);
    waiter?.reject(terminal);
    waiter = null;
  }
  return {
    receive(timeoutMillis) {
      if (queue.length) return Promise.resolve(queue.shift());
      if (terminal) return Promise.reject(terminal);
      if (waiter) return Promise.reject(refusal("pressure", "more than one relay receive is pending"));
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          if (waiter?.resolve === resolve) waiter = null;
          reject(refusal("relay-timeout", "browser relay receive reached its deadline"));
        }, timeoutMillis);
        waiter = {
          resolve: (bytes) => { clearTimeout(timeout); resolve(bytes); },
          reject: (error) => { clearTimeout(timeout); reject(error); },
        };
      });
    },
    close() { fail("closed", "browser relay Line closed"); },
  };
}

function socketOpened(socket, timeoutMillis) {
  if (socket.readyState === 1) return Promise.resolve();
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(refusal("relay-timeout", "relay connection deadline elapsed")), timeoutMillis);
    socket.addEventListener("open", () => { clearTimeout(timeout); resolve(); }, { once: true });
    socket.addEventListener("error", () => { clearTimeout(timeout); reject(refusal("relay-transport-authentication", "relay WSS authentication failed")); }, { once: true });
  });
}

async function messageBytes(value) {
  if (value instanceof ArrayBuffer) return new Uint8Array(value);
  if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer, value.byteOffset, value.byteLength).slice();
  if (typeof Blob === "function" && value instanceof Blob) return new Uint8Array(await value.arrayBuffer());
  if (typeof value === "string") return encoder.encode(value);
  throw refusal("protocol", "relay delivered an unsupported message representation");
}

function encodeEnvelope(route, frame) {
  const routeBytes = encoder.encode(route);
  if (routeBytes.byteLength < 1 || routeBytes.byteLength > 128 || frame.byteLength > 65_553) {
    throw refusal("pressure", "relay envelope violates its bound");
  }
  const output = new Uint8Array(11 + routeBytes.byteLength + frame.byteLength);
  output.set([0x43, 0x4e, 0x44, 0x52, 1]);
  const view = new DataView(output.buffer);
  view.setUint16(5, routeBytes.byteLength, true);
  view.setUint32(7, frame.byteLength, true);
  output.set(routeBytes, 11);
  output.set(frame, 11 + routeBytes.byteLength);
  return output;
}

function decodeEnvelope(bytes, route, maximumFrameBytes) {
  if (!magic(bytes) || bytes.byteLength < 11) throw refusal("protocol", "relay envelope is malformed");
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const routeLength = view.getUint16(5, true);
  const frameLength = view.getUint32(7, true);
  if (routeLength < 1 || routeLength > 128 || frameLength > maximumFrameBytes ||
      bytes.byteLength !== 11 + routeLength + frameLength ||
      decoder.decode(bytes.subarray(11, 11 + routeLength)) !== route) {
    throw refusal("protocol", "relay envelope differs from the exact candidate");
  }
  return bytes.slice(11 + routeLength);
}

function decodeOutcome(bytes, route) {
  let outcome;
  try { outcome = JSON.parse(decoder.decode(bytes)); } catch { throw refusal("protocol", "relay outcome is malformed"); }
  if (outcome?.schema !== "conduit.relay/outcome@1" ||
      outcome.implementation_id !== RELAY_IMPLEMENTATION || outcome.route_id !== route) {
    throw refusal("protocol", "relay outcome differs from the exact candidate");
  }
  return outcome;
}

function outcomeRefusal(outcome) {
  const code = outcome?.status === "pressure" ? "pressure" :
    outcome?.status === "closed" ? "closed" : "relay-lost";
  return refusal(code, `relay ended with ${outcome?.code ?? outcome?.status ?? "unknown"}`);
}

function magic(bytes) {
  return bytes.byteLength >= 5 && bytes[0] === 0x43 && bytes[1] === 0x4e && bytes[2] === 0x44 && bytes[3] === 0x52 && bytes[4] === 1;
}

function erase(...values) { for (const value of values) value?.fill?.(0); }
function eraseCandidateSecret(value) { if (value instanceof Uint8Array) value.fill(0); else if (Array.isArray(value)) value.fill(0); }
function low(value) { return Number(BigInt(value) & 0xffff_ffffn); }
function high(value) { return Number((BigInt(value) >> 32n) & 0xffff_ffffn); }
function refusal(code, message) { return new BrowserRelayError(code, message); }
