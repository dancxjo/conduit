import { BodyWebRtcSessions } from "./body-webrtc-sessions.mjs";
import { openBrowserHostIdentity } from "/targets/browser/host/assets/browser-host-identity.mjs";

const INPUT_CAPACITY = 4096;
const MAXIMUM_OUTPUT_BYTES = 24 * 1024;
const MEDIA_PLAN_TIMEOUT_MILLIS = 10_000;

const MAXIMUM_WEB_RTC_GRANTS = 16;

export function immutableWebRtcGrantFrame(frame) {
  if (frame?.grant !== null && (typeof frame?.grant !== "object" ||
      !Array.isArray(frame.grant.session_hello))) {
    throw new Error("invalid WebRTC grant frame");
  }
  const immutableGrant = frame.grant === null ? null : Object.freeze({
    ...frame.grant,
    session_hello: Object.freeze([...frame.grant.session_hello]),
  });
  return Object.freeze({ ...frame, grant: immutableGrant });
}

export function immutableWebRtcSignalFrame(frame) {
  if (typeof frame?.signal !== "object" || frame.signal === null ||
      !Array.isArray(frame.signal.session_hello)) {
    throw new Error("invalid WebRTC signal frame");
  }
  const immutableSignal = Object.freeze({
    ...frame.signal,
    session_hello: Object.freeze([...frame.signal.session_hello]),
  });
  return Object.freeze({ ...frame, signal: immutableSignal });
}

export async function joinBrowserBody({ bodyUrl, wasmBytes, expectedBodyId = null, onState, onWebRtcGrant, onWebRtcSignal, onWebRtcState, configureHost, renewPresence = true, reconnectPresence = true }) {
  if (expectedBodyId !== null && (typeof expectedBodyId !== "string" || expectedBodyId.length === 0)) {
    throw new Error("invalid expected Body identity");
  }
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
  const identity = await openBrowserHostIdentity();
  const hostId = identity.hostId;
  const bootId = `browser-boot/${crypto.randomUUID()}`;
  const seed = identity.seed.slice();
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
  configureHost?.(Object.freeze({ api, hostId, bootId }));
  let state = "connecting";
  let presenceState = "unavailable";
  let credential;
  let renewalTimer;
  let renewalSequence = 1;
  let socket;
  let deliberateClose = false;
  let reconnectAttempts = 0;
  let presenceEstablished = false;
  let webRtcSessions;
  let webRtcFailure = null;
  let webRtcRefusal = null;
  let pendingMediaPlan = null;
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
  const sendWebRtcSignal = ({ targetHostId, targetBootId, signal }) => {
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
  };
  const requestWebRtcGrant = (generation, index) => {
    if (!credential || presenceState !== "available" || socket?.readyState !== WebSocket.OPEN) {
      throw new Error("current browser presence is required for WebRTC grants");
    }
    if (!Number.isInteger(generation) || generation < 0 || generation >= 2 ||
        !Number.isInteger(index) || index < 0 || index >= MAXIMUM_WEB_RTC_GRANTS) {
      throw new Error("invalid WebRTC grant generation or index");
    }
    socket.send(encoder.encode(JSON.stringify({
      kind: "web-rtc-grant-request",
      protocol: 1,
      credential_id: credential.credential_id,
      body_id: credential.body_id,
      part_id: credential.part_id,
      host_id: credential.host_id,
      boot_id: credential.boot_id,
      generation,
      index,
    })));
  };
  webRtcSessions = new BodyWebRtcSessions({
    wasmBytes,
    sendSignal: sendWebRtcSignal,
    requestGrant: requestWebRtcGrant,
    onState: (next) => onWebRtcState?.(next),
  });
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
    if (frame.kind === "media-use-plan" && frame.protocol === 1) {
      if (!pendingMediaPlan || frame.resource_handle !== pendingMediaPlan.resourceHandle) {
        throw new Error("stale or mismatched media use Plan");
      }
      clearTimeout(pendingMediaPlan.timeout);
      const resolve = pendingMediaPlan.resolve;
      pendingMediaPlan = null;
      resolve(Object.freeze({
        planId: frame.plan_id,
        resourceHandle: frame.resource_handle,
        outputPort: frame.output_port,
      }));
    } else if (frame.kind === "web-rtc-plan-ready" && frame.protocol === 1) {
      if (!credential || presenceState !== "available" || frame.generation !== 1 ||
          typeof frame.plan_id !== "string" || frame.plan_id.length === 0) {
        throw new Error("invalid WebRTC Plan-ready transition");
      }
      const generation = webRtcSessions.activatePlan();
      if (generation !== frame.generation) throw new Error("WebRTC Plan generation mismatch");
    } else if (frame.kind === "challenge" && frame.protocol === 1) {
      if (expectedBodyId !== null && frame.challenge?.body_id !== expectedBodyId) {
        deliberateClose = true;
        setState("refused:wrong-body");
        socket.close(1008, "Body invitation identity mismatch");
        return;
      }
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
      if (expectedBodyId !== null && frame.credential?.body_id !== expectedBodyId) {
        deliberateClose = true;
        setState("refused:wrong-body");
        socket.close(1008, "Body credential identity mismatch");
        return;
      }
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
      if (!returning) webRtcSessions.begin();
      setState("admitted");
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
      let immutable;
      try {
        immutable = immutableWebRtcSignalFrame(frame);
      } catch (error) {
        webRtcFailure = error.message;
        webRtcSessions.reset(`signal-refused:${error.message}`);
        onWebRtcState?.(webRtcSessions.state());
        return;
      }
      onWebRtcSignal?.(immutable);
      void webRtcSessions.acceptSignal(immutable).catch((error) => {
        webRtcFailure = error.message;
        webRtcSessions.reset(`signal-refused:${error.message}`);
        onWebRtcState?.(webRtcSessions.state());
      });
    } else if (frame.kind === "web-rtc-grant" && frame.protocol === 1) {
      if (!credential || presenceState !== "available" ||
          !Number.isInteger(frame.generation) || frame.generation < 0 || frame.generation >= 2 ||
          !Number.isInteger(frame.index) || !Number.isInteger(frame.total) ||
          frame.index < 0 || frame.index >= MAXIMUM_WEB_RTC_GRANTS ||
          frame.total < 0 || frame.total > MAXIMUM_WEB_RTC_GRANTS ||
          (frame.grant !== null && typeof frame.grant !== "object")) {
        throw new Error("invalid WebRTC grant response for current browser presence");
      }
      let immutable;
      try {
        immutable = immutableWebRtcGrantFrame(frame);
      } catch (error) {
        webRtcFailure = error.message;
        webRtcSessions.reset(`grant-refused:${error.message}`);
        onWebRtcState?.(webRtcSessions.state());
        return;
      }
      onWebRtcGrant?.(immutable);
      void webRtcSessions.acceptGrantFrame(immutable).catch((error) => {
        if (error.message === "stale WebRTC grant generation") {
          webRtcRefusal = error.message;
          onWebRtcState?.(webRtcSessions.state());
          return;
        }
        webRtcFailure = error.message;
        webRtcSessions.reset(`grant-refused:${error.message}`);
        onWebRtcState?.(webRtcSessions.state());
      });
    } else if (frame.kind === "refused" && frame.protocol === 1) {
      presenceState = "unavailable";
      clearTimeout(renewalTimer);
      webRtcSessions.reset(`body-refused:${frame.code}`);
      setState(`refused:${frame.code}`);
    }
    });
    socket.addEventListener("close", () => {
      clearTimeout(renewalTimer);
      presenceState = "unavailable";
      webRtcSessions.reset("presence-lost");
      if (pendingMediaPlan) {
        clearTimeout(pendingMediaPlan.timeout);
        pendingMediaPlan.reject(new Error("media use planning Line closed"));
        pendingMediaPlan = null;
      }
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
  function publishMediaResource(mediaEvidence) {
    if (!credential || presenceState !== "available" || socket?.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error("current Body membership is required for media resource truth"));
    }
    if (pendingMediaPlan) {
      return Promise.reject(new Error("one media use planning operation is already pending"));
    }
    if (mediaEvidence?.host_id !== hostId || mediaEvidence?.boot_id !== bootId ||
        mediaEvidence?.phase !== "resource-truth" || !mediaEvidence.resource_handle ||
        !mediaEvidence.use_authority_grant || mediaEvidence.output_port !== "frame" ||
        mediaEvidence.value_kind !== "media/camera-frame@1" ||
        mediaEvidence.resource_class !== "conduit.resource/acquired-camera@1") {
      return Promise.reject(new Error("invalid or non-current camera resource truth"));
    }
    const settings = mediaEvidence.settings;
    const bounds = mediaEvidence.flow_bounds;
    if (!settings || !bounds) return Promise.reject(new Error("camera resource truth lacks exact bounds"));
    socket.send(encoder.encode(JSON.stringify({
      kind: "media-resource-truth",
      protocol: 1,
      credential_id: credential.credential_id,
      body_id: credential.body_id,
      part_id: credential.part_id,
      host_id: hostId,
      boot_id: bootId,
      resource: {
        host_id: hostId,
        boot_id: bootId,
        handle_id: mediaEvidence.resource_handle,
        class_id: mediaEvidence.resource_class,
        value_kind: mediaEvidence.value_kind,
        settings: { Camera: {
          minimum_width: settings.width,
          maximum_width: settings.width,
          minimum_height: settings.height,
          maximum_height: settings.height,
          maximum_frames_per_second: settings.maximum_frames_per_second,
        } },
        flow_bounds: bounds,
        use_authority_contract: "conduit.authority/use-human-media@1",
        use_authority_grant: mediaEvidence.use_authority_grant,
        availability: "Available",
      },
    })));
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        if (pendingMediaPlan?.resourceHandle === mediaEvidence.resource_handle) {
          pendingMediaPlan = null;
          reject(new Error("media use planning timed out"));
        }
      }, MEDIA_PLAN_TIMEOUT_MILLIS);
      pendingMediaPlan = { resourceHandle: mediaEvidence.resource_handle, resolve, reject, timeout };
    });
  }
  return Object.freeze({
    hostId,
    bootId,
    membershipCredential: () => credential === undefined ? null : Object.freeze({ ...credential }),
    state: () => state,
    presenceState: () => presenceState,
    pageLifecycle: () => pageLifecycle,
    freshnessProfile: () => freshnessProfile,
    signalWebRtc: sendWebRtcSignal,
    requestWebRtcGrant: (index, generation = 0) => requestWebRtcGrant(generation, index),
    webRtcSessions: () => Object.freeze({
      ...webRtcSessions.state(),
      failure: webRtcFailure,
      refusal: webRtcRefusal,
    }),
    offerWebRtcValue: (negotiationId, bytes) => webRtcSessions.offerValue(
      negotiationId,
      bytes instanceof Uint8Array ? bytes : Uint8Array.from(bytes),
    ),
    receiveWebRtcValue: (negotiationId) => webRtcSessions.receiveValue(negotiationId),
    pressureNextWebRtcValue: (negotiationId) => webRtcSessions.pressureNextValue(negotiationId),
    deliverWebRtcValue: (negotiationId, sequence) => webRtcSessions.deliverValue(negotiationId, sequence),
    waitWebRtcValueDelivered: (negotiationId, sequence) => webRtcSessions.waitDelivered(negotiationId, sequence),
    closeWebRtcLine: (negotiationId) => webRtcSessions.closeLine(negotiationId),
    replanWebRtc: () => webRtcSessions.replan(),
    advertisement: Object.freeze(advertisement),
    publishMediaResource,
    close: () => {
      deliberateClose = true;
      clearTimeout(renewalTimer);
      webRtcSessions.reset("presence-closed");
      const finalSequence = renewalSequence;
      socket.close(1000, "Patchbay browser Host leaving");
      return finalSequence;
    },
  });
}
