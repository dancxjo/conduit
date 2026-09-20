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
  const raw = String(value ?? "").trim();
  if (raw.startsWith("{")) return decodeRemoteDescriptor(raw);
  const normalized = raw.toUpperCase();
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

function decodeRemoteDescriptor(raw) {
  let descriptor;
  try { descriptor = JSON.parse(raw); }
  catch (error) { refuse("InvalidDescriptor", "remote rendezvous descriptor is not valid JSON", error); }
  const keys = Object.keys(descriptor ?? {}).sort().join(",");
  const expectedKeys = ["candidates", "schema", "session_secret"].sort().join(",");
  if (keys !== expectedKeys
    || descriptor.schema !== "conduit.host/rendezvous-descriptor@1"
    || !Array.isArray(descriptor.candidates) || descriptor.candidates.length < 1
    || descriptor.candidates.length > 4
    || !byteSequence(descriptor.session_secret, 32)
    || descriptor.session_secret.every((byte) => byte === 0)) {
    refuse("InvalidDescriptor", "remote rendezvous descriptor is stale, insecure, or outside its finite policy");
  }
  const seen = new Set();
  const candidates = descriptor.candidates.map((candidate) => {
    const candidateKeys = Object.keys(candidate ?? {}).sort().join(",");
    const expectedCandidateKeys = ["attempt_timeout_millis", "authentication", "candidate_id",
      "expires_at_millis", "line_family", "maximum_attempts", "reachability"].sort().join(",");
    const authenticationKeys = Object.keys(candidate?.authentication ?? {}).sort().join(",");
    if (candidateKeys !== expectedCandidateKeys
      || authenticationKeys !== "server_identity,transport_binding_sha256"
      || typeof candidate.candidate_id !== "string" || candidate.candidate_id.length < 1
      || candidate.candidate_id.length > 256 || seen.has(candidate.candidate_id)
      || !Number.isSafeInteger(candidate.expires_at_millis) || candidate.expires_at_millis <= Date.now()
      || !Number.isSafeInteger(candidate.maximum_attempts) || candidate.maximum_attempts < 1
      || candidate.maximum_attempts > 3
      || !Number.isSafeInteger(candidate.attempt_timeout_millis)
      || candidate.attempt_timeout_millis < 1 || candidate.attempt_timeout_millis > 30_000
      || typeof candidate.reachability !== "string" || candidate.reachability.length < 1
      || candidate.reachability.length > 256
      || typeof candidate.authentication.server_identity !== "string"
      || candidate.authentication.server_identity.length < 1
      || candidate.authentication.server_identity.length > 256
      || !byteSequence(candidate.authentication.transport_binding_sha256, 32)
      || candidate.authentication.transport_binding_sha256.every((byte) => byte === 0)) {
      refuse("InvalidDescriptor", "remote rendezvous candidate is stale, duplicated, or outside its finite policy");
    }
    seen.add(candidate.candidate_id);
    if (candidate.line_family !== "authenticated-tls-stream") {
      return Object.freeze({ supported: false, candidate_id: candidate.candidate_id });
    }
    let endpoint;
    try { endpoint = new URL(candidate.reachability); }
    catch (error) { refuse("InvalidDescriptor", "remote rendezvous endpoint is malformed", error); }
    if (endpoint.protocol !== "wss:" || endpoint.hostname !== candidate.authentication.server_identity) {
      refuse("InvalidDescriptor", "remote rendezvous server identity does not match its secure endpoint");
    }
    return Object.freeze({
      supported: true,
      candidate_id: candidate.candidate_id,
      carrier: "websocket",
      line_id: "conduit-line/authenticated-tls-stream@1",
      url: candidate.reachability,
      transport_binding_sha256: hexBytes(candidate.authentication.transport_binding_sha256),
      expires_at_millis: candidate.expires_at_millis,
      maximum_attempts: candidate.maximum_attempts,
      attempt_timeout_millis: candidate.attempt_timeout_millis,
    });
  });
  const selected = candidates.find((candidate) => candidate.supported);
  if (!selected) {
    refuse("LineUnavailable", "this browser offers none of the descriptor's bounded Line candidates");
  }
  return Object.freeze({
    ...selected,
    schema: descriptor.schema,
    session_secret: new Uint8Array(descriptor.session_secret),
    candidates: Object.freeze(candidates),
  });
}

function hexBytes(bytes) {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
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
        let intentional = false, membershipRetained = false, bodyContextInstalled = false, remotePrepared = false;
        const joinedLine = Object.freeze({
          schema: "conduit.creche/joined-host-line@1",
          line_id: decoded.line_id,
          onClosed(callback) { void line.closed.then(() => callback(Object.freeze({ intentional }))); },
          async retainMembership(credential) {
            if (intentional) refuse("LineClosed", "joined Host Line is already closed");
            if (membershipRetained) refuse("Replay", "joined Host membership is already retained");
            requireMembershipCredential(credential, prepared.body_id, descriptor.advertisement);
            await send(line, { kind: "admitted", protocol: PROTOCOL, credential });
            const retained = await receive(line, signal);
            if (retained?.kind !== "admission-retained" || retained.protocol !== PROTOCOL
              || retained.body_id !== credential.body_id || retained.part_id !== credential.part_id) {
              refuse("MembershipRetention", "joined Host did not retain the exact admitted membership");
            }
            membershipRetained = true;
            return Object.freeze(retained);
          },
          async installBodyContext(context) {
            if (intentional) refuse("LineClosed", "joined Host Line is already closed");
            if (!membershipRetained) refuse("MembershipNotRetained", "joined Host has not retained its admitted Body membership");
            requireBodyConversationContext(context, prepared.body_id);
            await send(line, { kind: "body-context", protocol: PROTOCOL, context });
            const installed = await receive(line, signal);
            if (installed?.kind !== "body-context-installed" || installed.protocol !== PROTOCOL
              || installed.body_id !== context.body_id
              || installed.basis_revision !== context.basis.revision) {
              refuse("BodyContext", "joined Host did not retain the exact current Body context");
            }
            bodyContextInstalled = true;
            return Object.freeze(installed);
          },
          async observeLocalModelPool(realization) {
            if (intentional) refuse("LineClosed", "joined Host Line is already closed");
            if (!membershipRetained) refuse("MembershipNotRetained", "joined Host has not retained its admitted Body membership");
            requirePoolRealization(realization, descriptor.advertisement);
            await send(line, { kind: "observe-local-model-pool", protocol: PROTOCOL, realization });
            const observed = await receive(line, signal);
            requirePoolObservation(observed, realization);
            return Object.freeze(observed.observation);
          },
          async prepareRemote(plan) {
            if (intentional) refuse("LineClosed", "joined Host Line is already closed");
            if (!membershipRetained) refuse("MembershipNotRetained", "joined Host has not retained its admitted Body membership");
            if (!bodyContextInstalled) refuse("BodyContextAbsent", "joined Host has no current Body conversation context");
            if (remotePrepared) refuse("RemotePlayActive", "joined Host Line already owns a remote Play");
            await send(line, { kind: "prepare-remote", protocol: PROTOCOL, plan });
            const prepared = await receive(line, signal);
            requireRemotePrepared(prepared, descriptor.advertisement);
            remotePrepared = true;
            return Object.freeze(prepared);
          },
          async preparePoolMember({ plan, selection, consumerPlacementId }) {
            if (intentional) refuse("LineClosed", "joined Host Line is already closed");
            if (!membershipRetained) refuse("MembershipNotRetained", "joined Host has not retained its admitted Body membership");
            if (!bodyContextInstalled) refuse("BodyContextAbsent", "joined Host has no current Body conversation context");
            if (remotePrepared) refuse("RemotePlayActive", "joined Host Line already owns a remote Play");
            if (!plan || selection?.plan_id !== plan.plan_id
              || selection?.disposition !== "Selected"
              || !Number.isSafeInteger(selection?.selected_realization)
              || selection.selected_realization < 0
              || !boundedIdentity(selection?.pool_id)
              || !boundedIdentity(selection?.operation_id)
              || !boundedIdentity(selection?.sign_id)
              || !Array.isArray(selection?.observation_sign_ids)
              || selection.observation_sign_ids.length < 1
              || selection.observation_sign_ids.some((identity) => !boundedIdentity(identity))
              || !boundedIdentity(consumerPlacementId)) {
              refuse("PoolMemberSelection", "pool member preparation lost exact bounded selection truth");
            }
            await send(line, {
              kind: "prepare-pool-member", protocol: PROTOCOL, plan, selection,
              consumer_placement_id: consumerPlacementId,
            });
            const prepared = await receive(line, signal);
            requireRemotePrepared(prepared, descriptor.advertisement);
            remotePrepared = true;
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
          async releaseRemote() {
            if (intentional) refuse("LineClosed", "joined Host Line is already closed");
            if (!remotePrepared) refuse("RemotePlayAbsent", "joined Host Line has no remote Play");
            await send(line, { kind: "release-remote", protocol: PROTOCOL });
            const released = await receive(line, signal);
            if (released?.kind !== "remote-released" || released.protocol !== PROTOCOL) {
              refuse("RemoteRelease", "joined Host did not release the exact remote Play");
            }
            remotePrepared = false;
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

function requireBodyConversationContext(value, bodyId) {
  if (!value || value.schema !== "conduit.body/conversation-context-value@2"
    || value.body_id !== bodyId || typeof value.display_name !== "string" || value.display_name.length < 1
    || typeof value.wake_id !== "string" || value.wake_id.length < 1
    || !Number.isSafeInteger(value.wake_sequence) || value.wake_sequence < 0
    || value.basis?.body_id !== value.body_id || value.basis?.wake_id !== value.wake_id
    || value.basis?.wake_sequence !== value.wake_sequence
    || !Number.isSafeInteger(value.basis?.revision) || value.basis.revision < 0
    || !Array.isArray(value.hosts) || !Array.isArray(value.active_forms)
    || !Array.isArray(value.lines) || !Array.isArray(value.recent_sign_ids)) {
    refuse("BodyContext", "Body conversation context is malformed or belongs to another Body");
  }
}

function requireMembershipCredential(value, bodyId, advertisement) {
  if (!value || !boundedIdentity(value.credential_id)
    || value.body_id !== bodyId || !boundedIdentity(value.part_id)
    || value.host_id !== advertisement.host_id || value.boot_id !== advertisement.boot_id
    || !Number.isSafeInteger(value.issued_at_millis) || value.issued_at_millis < 0) {
    refuse("MembershipCredential", "admitted membership credential lost the exact Body, Host, or Boot identity");
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

function requirePoolRealization(realization, advertisement) {
  const generation = advertisement.offer_generation?.[0] ?? advertisement.offer_generation;
  if (!realization || realization.host_id !== advertisement.host_id
    || realization.boot_id !== advertisement.boot_id
    || realization.offer_generation !== generation
    || !boundedIdentity(realization.capability_id)
    || !boundedIdentity(realization.implementation_id)
    || !boundedIdentity(realization.artifact_id)
    || !Number.isSafeInteger(realization.member_capacity) || realization.member_capacity < 1
    || !Array.isArray(realization.resources) || realization.resources.length > 32
    || realization.resources.some((binding) => !boundedIdentity(binding?.pool_id)
      || !boundedIdentity(binding?.class_id) || !Number.isSafeInteger(binding?.units)
      || binding.units < 1)) {
    refuse("PoolRealization", "model-pool realization is malformed or stale for this Host");
  }
}

function requirePoolObservation(value, realization) {
  const observation = value?.observation;
  if (value?.kind !== "local-model-pool-observed" || value.protocol !== PROTOCOL
    || !observation || observation.host_id !== realization.host_id
    || observation.boot_id !== realization.boot_id
    || observation.offer_generation !== realization.offer_generation
    || observation.capability_id !== realization.capability_id
    || observation.implementation_id !== realization.implementation_id
    || observation.artifact_id !== realization.artifact_id
    || !["Ready", "Unavailable"].includes(observation.health)
    || !boundedIdentity(observation.sign_id)
    || !Array.isArray(observation.resources)
    || observation.resources.length !== realization.resources.length
    || observation.resources.some((resource, index) => {
      const binding = realization.resources[index];
      return resource?.host_id !== realization.host_id || resource.boot_id !== realization.boot_id
        || resource.offer_generation !== realization.offer_generation
        || resource.pool_id !== binding.pool_id || resource.class_id !== binding.class_id
        || !["Ready", "Unavailable"].includes(resource.health)
        || !Number.isSafeInteger(resource.unreserved_units) || resource.unreserved_units < 0
        || !Number.isSafeInteger(resource.utilized_units) || resource.utilized_units < 0
        || !boundedIdentity(resource.sign_id);
    })) {
    refuse("PoolObservation", "joined Host returned malformed or stale model-pool truth");
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
