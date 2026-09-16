const PROTOCOL = 1;
const MAXIMUM_FRAME_BYTES = 96 * 1024;
const MAXIMUM_WAIT_MILLIS = 10_000;
const WEBSOCKET_CODE_PATTERN = /^C1-WS-([0-9A-F]{4})-([0-9A-F]{64})$/;
const SERIAL_CODE_PATTERN = /^C1-SERIAL-([0-9A-F]{64})$/;
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

export class CrecheRendezvousRefusal extends Error {
  constructor(code, message, cause = undefined) {
    super(message, cause ? { cause } : undefined);
    this.name = "CrecheRendezvousRefusal";
    this.code = code;
  }
}

export function decodeRendezvousCode(value) {
  const normalized = String(value ?? "").trim().toUpperCase();
  const websocket = WEBSOCKET_CODE_PATTERN.exec(normalized);
  const serial = SERIAL_CODE_PATTERN.exec(normalized);
  if (!websocket && !serial) refuse("InvalidCode", "rendezvous code is not a supported finite Line code");
  const secretHex = websocket?.[2] ?? serial[1];
  const sessionSecret = new Uint8Array(32);
  for (let index = 0; index < sessionSecret.length; index += 1) {
    sessionSecret[index] = Number.parseInt(secretHex.slice(index * 2, index * 2 + 2), 16);
  }
  if (sessionSecret.every((byte) => byte === 0)) refuse("InvalidCode", "rendezvous code contains a weak session secret");
  return Object.freeze({
    schema: "conduit.creche/rendezvous-code@1",
    carrier: websocket ? "websocket" : "serial",
    line_id: websocket ? "conduit-line/loopback-websocket@1" : "conduit-line/serial-text@1",
    url: websocket ? websocketUrl(websocket[1]) : null,
    session_secret: sessionSecret,
  });
}

export async function connectRendezvousHost(code, {
  signal,
  retainLine = false,
  WebSocketClass = globalThis.WebSocket,
  serial = globalThis.navigator?.serial,
} = {}) {
  const decoded = decodeRendezvousCode(code);
  requireCurrent(signal);
  const line = decoded.carrier === "websocket"
    ? await openWebSocketLine(decoded, WebSocketClass, signal)
    : await openSerialLine(serial, signal);
  try {
    await send(line, {
      kind: "hello",
      protocol: PROTOCOL,
      session_secret: Array.from(decoded.session_secret),
    });
    const descriptor = await receive(line, signal);
    requireHostDescriptor(descriptor, decoded.line_id);
    let used = false;
    return Object.freeze({
      schema: "conduit.creche/running-host-rendezvous@1",
      code_schema: decoded.schema,
      line_id: decoded.line_id,
      descriptor: Object.freeze(descriptor),
      async invite(prepared) {
        if (used) refuse("Replay", "this one-use rendezvous session has already carried an invitation");
        used = true;
        requirePrepared(prepared);
        requireCurrent(signal);
        const invitationSecret = new Uint8Array(prepared.invitation_secret);
        try {
          await send(line, {
            kind: "invite",
            protocol: PROTOCOL,
            session_secret: Array.from(decoded.session_secret),
            spore_id: prepared.spore_id,
            image_id: prepared.image_id,
            claim: {
              invitation_id: prepared.invitation_id,
              body_id: prepared.body_id,
              nonce: prepared.invitation_nonce,
              expires_at_millis: prepared.invitation_expires_at_millis,
            },
            secret: Array.from(invitationSecret),
          });
        } finally {
          invitationSecret.fill(0);
          prepared.invitation_secret.fill(0);
          decoded.session_secret.fill(0);
        }
        const join = await receive(line, signal);
        requireJoin(join, prepared, descriptor.advertisement);
        if (!retainLine) {
          await send(line, { kind: "close", protocol: PROTOCOL });
          await line.close("conduit-terminal");
          return Object.freeze(join);
        }
        let intentional = false;
        const joinedLine = Object.freeze({
          schema: "conduit.creche/joined-host-line@1",
          line_id: decoded.line_id,
          onClosed(callback) { void line.closed.then(() => callback(Object.freeze({ intentional }))); },
          async prepareRemote(plan) {
            if (intentional) refuse("LineClosed", "joined Host Line is already closed");
            await send(line, { kind: "prepare-remote", protocol: PROTOCOL, plan });
            const prepared = await receive(line, signal);
            requireRemotePrepared(prepared, descriptor.advertisement);
            return Object.freeze(prepared);
          },
          async sendSessionFrame(frame) {
            if (intentional) refuse("LineClosed", "joined Host Line is already closed");
            const bytes = requireSessionFrame(frame);
            await line.sendBytes(bytes);
          },
          async receiveSessionFrame() {
            if (intentional) refuse("LineClosed", "joined Host Line is already closed");
            return requireSessionFrame(await line.receiveBytes(signal));
          },
          async close() {
            if (intentional) return;
            intentional = true;
            try { await send(line, { kind: "close", protocol: PROTOCOL }); } catch {}
            await line.close("conduit-terminal");
          },
        });
        return Object.freeze({ ...join, line: joinedLine });
      },
      cancel() {
        decoded.session_secret.fill(0);
        void line.close("conduit-cancelled");
      },
    });
  } catch (error) {
    decoded.session_secret.fill(0);
    await line.close("conduit-refused");
    if (error instanceof CrecheRendezvousRefusal) throw error;
    refuse("LineFailed", "running Host rendezvous Line failed", error);
  }
}

function requireSessionFrame(value) {
  const bytes = bytesOf(value);
  if (bytes.byteLength < 5 || bytes.byteLength > MAXIMUM_FRAME_BYTES
    || bytes[0] !== 0x43 || bytes[1] !== 0x4e || bytes[2] !== 0x44 || bytes[3] !== 0x53) {
    refuse("SessionFrame", "joined Host Line frame is malformed or outside its finite bound");
  }
  return bytes;
}

function requireRemotePrepared(prepared, advertisement) {
  const identity = prepared?.identity;
  const frames = prepared?.hello_frames;
  if (prepared?.kind !== "remote-prepared" || prepared.protocol !== PROTOCOL
    || identity?.host_id !== advertisement.host_id || identity?.boot_id !== advertisement.boot_id
    || typeof identity?.plan_id !== "string" || identity.plan_id.length === 0
    || typeof identity?.active_play_id !== "string" || identity.active_play_id.length === 0
    || !Number.isSafeInteger(identity?.play_sequence) || identity.play_sequence < 0
    || !Array.isArray(frames) || frames.length === 0 || frames.length > 32
    || frames.some(frame => !Array.isArray(frame) || frame.length < 5 || frame.length > MAXIMUM_FRAME_BYTES
      || frame[0] !== 0x43 || frame[1] !== 0x4e || frame[2] !== 0x44 || frame[3] !== 0x53
      || frame.some(byte => !Number.isInteger(byte) || byte < 0 || byte > 255))) {
    refuse("RemotePreparation", "joined Host returned malformed or stale remote Play truth");
  }
}

function websocketUrl(portHex) {
  const port = Number.parseInt(portHex, 16);
  if (!Number.isSafeInteger(port) || port < 1) refuse("InvalidCode", "rendezvous code names an invalid Line endpoint");
  return `ws://127.0.0.1:${port}/conduit`;
}

async function openWebSocketLine(decoded, WebSocketClass, signal) {
  if (typeof WebSocketClass !== "function") refuse("LineUnavailable", "this browser does not offer the WebSocket Line carrier");
  const socket = new WebSocketClass(decoded.url);
  socket.binaryType = "arraybuffer";
  await waitForSocket(socket, signal, "open");
  const queued = [];
  const waiting = [];
  socket.addEventListener("message", (event) => {
    const accepted = waiting.shift();
    if (accepted) accepted(event.data);
    else queued.push(event.data);
  });
  const closed = new Promise((resolve) => socket.addEventListener("close", resolve, { once: true }));
  return Object.freeze({
    sendBytes: (bytes) => socket.send(bytes),
    receiveBytes: async (currentSignal) => {
      if (queued.length > 0) return bytesOf(queued.shift());
      return bytesOf(await waitForPromise(new Promise((resolve) => waiting.push(resolve)), currentSignal));
    },
    close: async (reason) => { if (socket.readyState < 2) socket.close(1000, reason); },
    closed,
  });
}

async function openSerialLine(serial, signal) {
  if (!serial || typeof serial.requestPort !== "function") refuse("LineUnavailable", "this browser does not offer the Web Serial Line carrier");
  requireCurrent(signal);
  const port = await serial.requestPort();
  await port.open({ baudRate: 115200, dataBits: 8, stopBits: 1, parity: "none", flowControl: "none", bufferSize: 4096 });
  const reader = port.readable?.getReader();
  if (!reader || !port.writable) {
    await port.close();
    refuse("LineUnavailable", "selected serial port omitted readable or writable streams");
  }
  let pending = new Uint8Array(0);
  let closed = false;
  let resolveClosed;
  const closedPromise = new Promise(resolve => { resolveClosed = resolve; });
  return Object.freeze({
    async sendBytes(bytes) {
      const writer = port.writable.getWriter();
      const framed = new Uint8Array(bytes.byteLength + 1);
      framed.set(bytes); framed[bytes.byteLength] = 10;
      try { await writer.write(framed); }
      finally { framed.fill(0); writer.releaseLock(); }
    },
    async receiveBytes(currentSignal) {
      while (true) {
        const newline = pending.indexOf(10);
        if (newline >= 0) {
          let frame = pending.slice(0, newline);
          pending = pending.slice(newline + 1);
          if (frame.at(-1) === 13) frame = frame.slice(0, -1);
          return frame;
        }
        const { value, done } = await waitForPromise(reader.read(), currentSignal);
        if (done) refuse("LineClosed", "serial rendezvous Line closed before completion");
        const chunk = bytesOf(value);
        if (pending.byteLength + chunk.byteLength > MAXIMUM_FRAME_BYTES + 1) refuse("FrameBound", "serial rendezvous response exceeds its finite frame bound");
        const combined = new Uint8Array(pending.byteLength + chunk.byteLength);
        combined.set(pending); combined.set(chunk, pending.byteLength); pending = combined;
      }
    },
    async close() {
      if (closed) return;
      closed = true;
      try { await reader.cancel(); } catch {}
      reader.releaseLock();
      await port.close();
      pending.fill(0);
      resolveClosed();
    },
    closed: closedPromise,
  });
}

async function receive(line, signal) {
  const bytes = await line.receiveBytes(signal);
  if (bytes.byteLength < 1 || bytes.byteLength > MAXIMUM_FRAME_BYTES) refuse("FrameBound", "rendezvous response exceeds its finite frame bound");
  try {
    const value = JSON.parse(decoder.decode(bytes));
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new TypeError();
    if (value.kind === "refused") refuse(value.code ?? "HostRefused", "running Host refused rendezvous");
    return value;
  } catch (error) {
    if (error instanceof CrecheRendezvousRefusal) throw error;
    refuse("MalformedFrame", "running Host returned malformed rendezvous evidence", error);
  }
}

function waitForSocket(socket, signal, wanted) {
  return new Promise((resolve, reject) => {
    let settled = false;
    const finish = (callback, value) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      signal?.removeEventListener("abort", aborted);
      socket.removeEventListener(wanted, accepted);
      socket.removeEventListener("error", failed);
      socket.removeEventListener("close", closed);
      callback(value);
    };
    const accepted = (event) => finish(resolve, event);
    const failed = () => finish(reject, new CrecheRendezvousRefusal("LineFailed", "rendezvous Line reported an error"));
    const closed = () => finish(reject, new CrecheRendezvousRefusal("LineClosed", "rendezvous Line closed before completion"));
    const aborted = () => finish(reject, signal.reason ?? new DOMException("Aborted", "AbortError"));
    const timer = setTimeout(() => finish(reject, new CrecheRendezvousRefusal("LineTimeout", "rendezvous Line did not answer within its admitted time")), MAXIMUM_WAIT_MILLIS);
    socket.addEventListener(wanted, accepted, { once: true });
    socket.addEventListener("error", failed, { once: true });
    socket.addEventListener("close", closed, { once: true });
    signal?.addEventListener("abort", aborted, { once: true });
    if (signal?.aborted) aborted();
  });
}

function waitForPromise(promise, signal) {
  return new Promise((resolve, reject) => {
    let settled = false;
    const aborted = () => finish(reject, signal.reason ?? new DOMException("Aborted", "AbortError"));
    const timer = setTimeout(() => finish(reject, new CrecheRendezvousRefusal("LineTimeout", "rendezvous Line did not answer within its admitted time")), MAXIMUM_WAIT_MILLIS);
    const finish = (callback, value) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      signal?.removeEventListener("abort", aborted);
      callback(value);
    };
    promise.then((value) => finish(resolve, value), (error) => finish(reject, error));
    signal?.addEventListener("abort", aborted, { once: true });
    if (signal?.aborted) aborted();
  });
}

async function send(line, value) {
  const bytes = encoder.encode(JSON.stringify(value));
  if (bytes.byteLength < 1 || bytes.byteLength > MAXIMUM_FRAME_BYTES) refuse("FrameBound", "rendezvous request exceeds its finite frame bound");
  try { await line.sendBytes(bytes); }
  finally { bytes.fill(0); }
}

function requireHostDescriptor(value, lineId) {
  if (value.kind !== "host" || value.protocol !== PROTOCOL
    || value.friendly_label !== "This running computer"
    || !boundedIdentity(value.target_id)
    || !/^sha256:[0-9a-f]{64}$/.test(value.image_content_digest)
    || !Array.isArray(value.lines) || !value.lines.includes(lineId)) {
    refuse("WrongProtocol", "rendezvous response is not a compatible running Host descriptor");
  }
  requireAdvertisement(value.advertisement);
}

function requireAdvertisement(value) {
  if (!value || typeof value !== "object"
    || !boundedIdentity(value.host_id) || !boundedIdentity(value.boot_id)
    || !Number.isSafeInteger(value.offer_generation) && !Number.isSafeInteger(value.offer_generation?.[0])) {
    refuse("WrongAdvertisement", "running Host advertisement is missing exact Host, Boot, or generation identity");
  }
}

function requirePrepared(value) {
  if (!value || !boundedIdentity(value.spore_id) || !boundedIdentity(value.image_id)
    || !boundedIdentity(value.invitation_id) || !boundedIdentity(value.body_id)
    || !Array.isArray(value.invitation_nonce) || value.invitation_nonce.length !== 32
    || !byteSequence(value.invitation_secret, 32)
    || !Number.isSafeInteger(value.invitation_expires_at_millis)) {
    refuse("InvalidInvitation", "Crèche prepared invitation is incomplete or outside its finite bounds");
  }
}

function byteSequence(value, length) {
  return (Array.isArray(value) || ArrayBuffer.isView(value)) && value.length === length
    && Array.from(value).every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255);
}

function requireJoin(value, prepared, advertisement) {
  if (value.kind !== "join" || value.protocol !== PROTOCOL
    || value.spore_id !== prepared.spore_id || value.image_id !== prepared.image_id
    || value.invitation_id !== prepared.invitation_id || value.body_id !== prepared.body_id
    || value.host_id !== advertisement.host_id || value.boot_id !== advertisement.boot_id
    || JSON.stringify(value.advertisement) !== JSON.stringify(advertisement)
    || !Array.isArray(value.nonce) || value.nonce.length !== 32
    || value.nonce.some((byte, index) => byte !== prepared.invitation_nonce[index])
    || !Array.isArray(value.signature) || value.signature.length !== 64
    || !Number.isSafeInteger(value.observed_at_millis)) {
    refuse("WrongJoin", "running Host join proof lost the exact invitation or Boot identity");
  }
}

function boundedIdentity(value) {
  return typeof value === "string" && value.length > 0 && value.length <= 192 && !/\s/.test(value);
}

function bytesOf(value) {
  if (value instanceof ArrayBuffer) return new Uint8Array(value);
  if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  refuse("MalformedFrame", "rendezvous Line must carry binary frames");
}

function requireCurrent(signal) {
  if (signal?.aborted) throw signal.reason ?? new DOMException("Aborted", "AbortError");
}

function refuse(code, message, cause) {
  throw new CrecheRendezvousRefusal(code, message, cause);
}

export const CRECHE_RENDEZVOUS_BOUNDS = Object.freeze({
  maximumFrameBytes: MAXIMUM_FRAME_BYTES,
  maximumWaitMillis: MAXIMUM_WAIT_MILLIS,
  maximumSessions: 1,
});
