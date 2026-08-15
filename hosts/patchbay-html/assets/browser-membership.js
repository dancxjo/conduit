const INPUT_CAPACITY = 4096;
const MAXIMUM_OUTPUT_BYTES = 9216;

export async function joinBrowserBody({ bodyUrl, wasmBytes, onState, onWebRtcSignal, renewPresence = true, reconnectPresence = true }) {
  const { instance } = await WebAssembly.instantiate(wasmBytes, {});
  const api = instance.exports;
  const required = [
    "memory",
    "conduit_browser_membership_input_ptr",
    "conduit_browser_membership_input_capacity",
    "conduit_browser_membership_output_ptr",
    "conduit_browser_membership_output_len",
    "conduit_browser_membership_initialize",
    "conduit_browser_membership_prove",
    "conduit_browser_membership_prove_return",
    "conduit_browser_membership_advertisement",
  ];
  if (required.some((name) => !(name in api)) ||
      api.conduit_browser_membership_input_capacity() !== INPUT_CAPACITY) {
    throw new Error("incomplete browser membership ABI");
  }
  const encoder = new TextEncoder();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  const readOutput = () => {
    const length = api.conduit_browser_membership_output_len();
    if (length < 0 || length > MAXIMUM_OUTPUT_BYTES) {
      throw new Error("invalid browser membership output length");
    }
    return new Uint8Array(
      api.memory.buffer,
      api.conduit_browser_membership_output_ptr(),
      length,
    ).slice();
  };
  const writeInput = (bytes) => {
    if (!(bytes instanceof Uint8Array) || bytes.length === 0 || bytes.length > INPUT_CAPACITY) {
      throw new Error("invalid browser membership input");
    }
    new Uint8Array(
      api.memory.buffer,
      api.conduit_browser_membership_input_ptr(),
      bytes.length,
    ).set(bytes);
  };
  const requireSuccess = (status, action) => {
    if (status < 0) throw new Error(`${action} failed ${status}`);
  };
  const hostId = `browser/${crypto.randomUUID()}`;
  const bootId = `browser-boot/${crypto.randomUUID()}`;
  const seed = crypto.getRandomValues(new Uint8Array(32));
  const host = encoder.encode(hostId);
  const boot = encoder.encode(bootId);
  const initialization = new Uint8Array(host.length + boot.length + seed.length);
  initialization.set(host);
  initialization.set(boot, host.length);
  initialization.set(seed, host.length + boot.length);
  writeInput(initialization);
  requireSuccess(
    api.conduit_browser_membership_initialize(host.length, boot.length),
    "browser membership initialization",
  );
  seed.fill(0);
  initialization.fill(0);
  const verifyingKey = readOutput();
  if (verifyingKey.length !== 32) throw new Error("invalid browser verifying key");
  requireSuccess(api.conduit_browser_membership_advertisement(), "browser advertisement");
  const advertisement = JSON.parse(decoder.decode(readOutput()));
  let state = "connecting";
  let presenceState = "unavailable";
  let credential;
  let renewalTimer;
  let renewalSequence = 1;
  let socket;
  let deliberateClose = false;
  let reconnectAttempts = 0;
  let presenceEstablished = false;
  let pageLifecycle = document.visibilityState === "hidden" ? "hidden" : "visible";
  let freshnessProfile = Object.freeze({
    scheduling: "best-effort-browser-event-loop",
    availabilityAuthority: "server-session-or-lease",
    backgroundRealtimeGuarantee: false,
    maximumReconnectAttempts: 1,
    sequence: 0,
    renewAfterMillis: null,
    serverExpiresAtMillis: null,
  });
  document.addEventListener("visibilitychange", (event) => {
    if (event.isTrusted) pageLifecycle = document.visibilityState === "hidden" ? "hidden" : "visible";
  });
  window.addEventListener("pagehide", (event) => {
    if (event.isTrusted) pageLifecycle = "page-hidden";
  });
  window.addEventListener("pageshow", (event) => {
    if (event.isTrusted) pageLifecycle = document.visibilityState === "hidden" ? "hidden" : "visible";
  });
  document.addEventListener("freeze", (event) => {
    if (event.isTrusted) pageLifecycle = "frozen";
  });
  document.addEventListener("resume", (event) => {
    if (event.isTrusted) pageLifecycle = document.visibilityState === "hidden" ? "hidden" : "visible";
  });
  const setState = (next) => {
    state = next;
    onState?.(next);
  };
  const openSocket = (returning = false) => {
    socket = new WebSocket(bodyUrl);
    socket.binaryType = "arraybuffer";
    socket.addEventListener("open", () => {
      if (returning) {
        setState("returning");
        socket.send(encoder.encode(JSON.stringify({
          kind: "return-advertise",
          protocol: 1,
          credential,
          advertisement,
        })));
        return;
      }
      setState("wants-to-join");
      socket.send(encoder.encode(JSON.stringify({
        kind: "advertise",
        protocol: 1,
        advertisement,
        friendly_label: "This browser",
        verifying_key: Array.from(verifyingKey),
        freshness_sequence: 1,
      })));
    });
    socket.addEventListener("message", (event) => {
    const frame = JSON.parse(typeof event.data === "string"
      ? event.data
      : decoder.decode(new Uint8Array(event.data)));
    if (frame.kind === "challenge" && frame.protocol === 1) {
      const bytes = encoder.encode(JSON.stringify(frame.challenge));
      writeInput(bytes);
      requireSuccess(api.conduit_browser_membership_prove(bytes.length), "browser admission proof");
      const signature = readOutput();
      if (signature.length !== 64) throw new Error("invalid browser admission signature");
      setState("proof-sent");
      socket.send(encoder.encode(JSON.stringify({
        kind: "ambient-proof",
        protocol: 1,
        admission_id: frame.challenge.admission_id,
        body_id: frame.challenge.body_id,
        host_id: hostId,
        boot_id: bootId,
        nonce: frame.challenge.nonce,
        signature: Array.from(signature),
      })));
    } else if (frame.kind === "return-challenge" && frame.protocol === 1) {
      const bytes = encoder.encode(JSON.stringify(frame.challenge));
      writeInput(bytes);
      requireSuccess(
        api.conduit_browser_membership_prove_return(bytes.length),
        "browser Part return proof",
      );
      const signature = readOutput();
      if (signature.length !== 64) throw new Error("invalid browser return signature");
      socket.send(encoder.encode(JSON.stringify({
        kind: "return-proof",
        protocol: 1,
        admission_id: frame.challenge.admission_id,
        body_id: frame.challenge.body_id,
        part_id: frame.challenge.part_id,
        host_id: hostId,
        boot_id: bootId,
        nonce: frame.challenge.nonce,
        signature: Array.from(signature),
      })));
    } else if (frame.kind === "admitted" && frame.protocol === 1) {
      credential = frame.credential;
      setState("admitted");
    } else if (frame.kind === "presence-accepted" && frame.protocol === 1) {
      if (!credential || (!returning && frame.sequence !== renewalSequence)) {
        throw new Error("presence acceptance did not match the current credential sequence");
      }
      renewalSequence = frame.sequence;
      freshnessProfile = Object.freeze({
        ...freshnessProfile,
        sequence: frame.sequence,
        renewAfterMillis: frame.renew_after_millis,
        serverExpiresAtMillis: frame.expires_at_millis,
      });
      presenceState = "available";
      presenceEstablished = true;
      if (returning) setState("admitted");
      clearTimeout(renewalTimer);
      if (renewPresence) {
        renewalTimer = setTimeout(() => {
          renewalSequence += 1;
          socket.send(encoder.encode(JSON.stringify({
            kind: "presence-renewal",
            protocol: 1,
            credential_id: credential.credential_id,
            body_id: credential.body_id,
            part_id: credential.part_id,
            host_id: credential.host_id,
            boot_id: credential.boot_id,
            sequence: renewalSequence,
          })));
        }, frame.renew_after_millis);
      }
    } else if (frame.kind === "web-rtc-signal" && frame.protocol === 1) {
      if (!credential || presenceState !== "available") {
        throw new Error("WebRTC signal arrived without current browser presence");
      }
      onWebRtcSignal?.(Object.freeze(frame));
    } else if (frame.kind === "refused" && frame.protocol === 1) {
      presenceState = "unavailable";
      clearTimeout(renewalTimer);
      setState(`refused:${frame.code}`);
    }
    });
    socket.addEventListener("close", () => {
      clearTimeout(renewalTimer);
      presenceState = "unavailable";
      if (state.startsWith("refused:")) return;
      if (!deliberateClose && presenceEstablished && reconnectPresence && reconnectAttempts === 0) {
        reconnectAttempts += 1;
        setState("reconnecting");
        queueMicrotask(() => openSocket(true));
      } else {
        setState("offline");
      }
    });
  };
  openSocket();
  return Object.freeze({
    hostId,
    bootId,
    state: () => state,
    presenceState: () => presenceState,
    pageLifecycle: () => pageLifecycle,
    freshnessProfile: () => freshnessProfile,
    signalWebRtc: ({ targetHostId, targetBootId, signal }) => {
      if (!credential || presenceState !== "available" || socket?.readyState !== WebSocket.OPEN) {
        throw new Error("current browser presence is required for WebRTC signaling");
      }
      if (typeof targetHostId !== "string" || targetHostId.length === 0 ||
          typeof targetBootId !== "string" || targetBootId.length === 0 ||
          typeof signal !== "object" || signal === null) {
        throw new Error("invalid WebRTC signaling target or payload");
      }
      socket.send(encoder.encode(JSON.stringify({
        kind: "web-rtc-signal",
        protocol: 1,
        credential_id: credential.credential_id,
        body_id: credential.body_id,
        part_id: credential.part_id,
        host_id: credential.host_id,
        boot_id: credential.boot_id,
        target_host_id: targetHostId,
        target_boot_id: targetBootId,
        signal,
      })));
    },
    close: () => {
      deliberateClose = true;
      clearTimeout(renewalTimer);
      const finalSequence = renewalSequence;
      socket.close(1000, "Patchbay browser Host leaving");
      return finalSequence;
    },
  });
}
