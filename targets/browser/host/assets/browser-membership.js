import { BodyWebRtcSessions } from "./body-webrtc-sessions.mjs";
import { openBrowserHostIdentity } from "./browser-host-identity.mjs";

const INPUT_CAPACITY = 4096;
const MEDIA_PLAN_TIMEOUT_MILLIS = 10_000;

const MAXIMUM_WEB_RTC_GRANTS = 16;
const OWNER_FACE_REQUEST_SCHEMA = "conduit.presentation/owner-face-request@1";
const OWNER_FACE_RESPONSE_SCHEMA = "conduit.presentation/owner-face-response@1";
const MAX_OWNER_FACE_RESPONSE_BYTES = 32768;
const MAX_OWNER_WARDROBE_REPORT_BYTES = 64 * 1024;
const MAX_OWNER_WARDROBE_RESPONSE_BYTES = MAX_OWNER_WARDROBE_REPORT_BYTES + 1024;
const MAX_U64 = 18_446_744_073_709_551_615n;
const wardrobeEncoder = new TextEncoder();

export function checkedOwnerWardrobeResponse(frame, pending, frameLength) {
  if (!pending || frame?.kind !== "face-wardrobe-response" || frame.protocol !== 1 ||
      frame.request_id !== pending.requestId || !Number.isSafeInteger(frameLength) ||
      frameLength < 1 || frameLength > MAX_OWNER_WARDROBE_RESPONSE_BYTES) {
    throw new Error("unsolicited or mismatched owner wardrobe response");
  }
  if (frame.accepted === false && typeof frame.code === "string" && frame.code.length > 0 &&
      frame.code.length <= 128 && frame.report === null) {
    const error = new Error(`Body owner refused wardrobe: ${frame.code}`);
    error.code = frame.code;
    throw error;
  }
  if (frame.accepted !== true || frame.code !== "" ||
      frame.report?.schema !== "conduit.body/owner-mask-wardrobe@1" ||
      wardrobeEncoder.encode(JSON.stringify(frame.report)).length > MAX_OWNER_WARDROBE_REPORT_BYTES) {
    throw new Error("invalid owner wardrobe response");
  }
  return frame.report;
}

function checkedWardrobeRevision(value) {
  if (typeof value !== "string" || !/^(0|[1-9][0-9]{0,19})$/.test(value) ||
      BigInt(value) > MAX_U64) throw new Error("wardrobe revision is not one exact u64");
  return value;
}

export function checkedOwnerWardrobeRequestFrame({ requestId, request, ownerPlanId, basisRevision, action }) {
  const revision = checkedWardrobeRevision(basisRevision);
  const serialized = JSON.stringify({ kind: "face-wardrobe-request", protocol: 1,
    request_id: requestId, request, owner_plan_id: ownerPlanId,
    basis_revision: null, action });
  const marker = '"basis_revision":null';
  const pieces = serialized.split(marker);
  if (pieces.length !== 2) throw new Error("owner wardrobe request has no unique revision slot");
  return wardrobeEncoder.encode(`${pieces[0]}"basis_revision":${revision}${pieces[1]}`);
}

export function checkedSelectedSpeechResponse(frame, pending, frameLength) {
  if (!pending || frame?.kind !== "selected-speech-response" || frame.protocol !== 1 ||
      frame.request_id !== pending.requestId) throw new Error("unsolicited or mismatched selected speech response");
  const expected = { start: "started", status: "status", stop: "stop-requested" }[pending.kind];
  const status = frame.status;
  const statusValid = pending.kind !== "status" ||
    (status?.operation_id === pending.operationId &&
      ((status.schema === "conduit.body/selected-speech-status@1" && status.state === "running") ||
       (status.schema === "conduit.body/selected-speech-terminal@1" &&
         ["completed", "cancelled", "refused", "failed", "play-refused-unclassified"].includes(status.outcome))));
  if (!Number.isSafeInteger(frameLength) || frameLength < 1 || frameLength > 64 * 1024 ||
      ![expected, "refused"].includes(frame.outcome) ||
      (frame.outcome === "refused" && (typeof frame.code !== "string" || !frame.code || frame.code.length > 128)) ||
      (frame.outcome !== "refused" && frame.code !== null) ||
      (frame.operation_id !== null && (typeof frame.operation_id !== "string" || !frame.operation_id || frame.operation_id.length > 256)) ||
      (frame.outcome !== "refused" &&
        (!frame.operation_id || (pending.operationId !== null && frame.operation_id !== pending.operationId) ||
         (pending.kind !== "status" && status !== null) || !statusValid))) {
    throw new Error("invalid selected speech response");
  }
  if (frame.outcome === "refused") throw new Error(`Owner refused selected speech: ${frame.code}`);
  return Object.freeze(frame);
}

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

function requireCredential(candidate, { expectedBodyId, hostId, bootId, prior = null }) {
  const fields = ["credential_id", "body_id", "part_id", "host_id", "boot_id"];
  if (typeof candidate !== "object" || candidate === null ||
      fields.some(field => typeof candidate[field] !== "string" || candidate[field].length === 0 || candidate[field].length > 256) ||
      !Number.isSafeInteger(candidate.issued_at_millis) || candidate.issued_at_millis < 0 ||
      (expectedBodyId !== null && candidate.body_id !== expectedBodyId) ||
      candidate.host_id !== hostId || candidate.boot_id !== bootId ||
      (prior !== null && (candidate.part_id !== prior.part_id || candidate.credential_id === prior.credential_id))) {
    throw new Error("invalid browser membership credential identity");
  }
  return Object.freeze({ ...candidate });
}

export async function joinBrowserBody({ bodyUrl, wasmBytes, admittedHost = null, expectedBodyId = null, retainedCredential = null, onCredential, onState, onBiographyEvidence, onOfferEvidence, onWebRtcGrant, onWebRtcSignal, onWebRtcState, configureHost, renewPresence = true, reconnectPresence = true }) {
  if (expectedBodyId !== null && (typeof expectedBodyId !== "string" || expectedBodyId.length === 0)) {
    throw new Error("invalid expected Body identity");
  }
  const admitted = admittedHost !== null;
  const instance = admitted ? null : (await WebAssembly.instantiate(wasmBytes, {})).instance;
  const api = admitted ? admittedHost.api : instance.exports;
  const required = [
    "memory",
    "conduit_browser_membership_input_ptr",
    "conduit_browser_membership_input_capacity",
    "conduit_browser_membership_output_ptr",
    "conduit_browser_membership_output_capacity",
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
  const outputCapacity = api.conduit_browser_membership_output_capacity();
  const readOutput = () => {
    const length = api.conduit_browser_membership_output_len();
    if (length < 0 || length > outputCapacity) {
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
  let hostId;
  let bootId;
  let verifyingKey;
  let advertisement;
  if (admitted) {
    const membership = admittedHost.membership;
    if (membership?.schema !== "conduit.browser/body-membership-client@1"
        || membership.hostId !== admittedHost.hostId || membership.bootId !== admittedHost.bootId
        || !Array.isArray(membership.verifyingKey) || membership.verifyingKey.length !== 32) {
      throw new Error("admitted browser Host has an invalid membership boundary");
    }
    hostId = admittedHost.hostId;
    bootId = admittedHost.bootId;
    verifyingKey = Uint8Array.from(membership.verifyingKey);
    advertisement = membership.advertisement();
  } else {
    const identity = await openBrowserHostIdentity();
    hostId = identity.hostId;
    bootId = `browser-boot/${crypto.randomUUID()}`;
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
    verifyingKey = readOutput();
    if (verifyingKey.length !== 32) throw new Error("invalid browser verifying key");
    requireSuccess(api.conduit_browser_membership_advertisement(), "browser advertisement");
    advertisement = JSON.parse(decoder.decode(readOutput()));
  }
  configureHost?.(Object.freeze({ api, hostId, bootId }));
  let state = "connecting";
  let presenceState = "unavailable";
  const priorCredential = retainedCredential === null ? null : requireCredential(retainedCredential, {
    expectedBodyId,
    hostId,
    bootId: retainedCredential?.boot_id,
  });
  if (priorCredential !== null && priorCredential.boot_id === bootId) {
    throw new Error("retained membership credential belongs to the current Boot");
  }
  let credential = priorCredential;
  let credentialPersistence = Promise.resolve();
  let biographyEvidence = null;
  let offerEvidence = null;
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
  let pendingFaceSnapshot = null;
  let pendingFaceShow = null;
  let pendingFaceInteraction = null;
  let pendingWardrobe = null;
  let wardrobeSequence = 0;
  let pendingSelectedSpeech = null;
  let selectedSpeechSequence = 0;
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
  const openSocket = (returning = false, freshBoot = false) => {
    socket = new WebSocket(bodyUrl);
    socket.binaryType = "arraybuffer";
    socket.addEventListener("open", () => {
      if (returning) {
        setState(freshBoot ? "returning-fresh-boot" : "returning");
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
    socket.addEventListener("message", async (event) => {
    const frameBytes = typeof event.data === "string"
      ? encoder.encode(event.data)
      : new Uint8Array(event.data);
    const frame = JSON.parse(decoder.decode(frameBytes));
    if (frame.kind === "face-snapshot-response" && frame.protocol === 1) {
      const pending = pendingFaceSnapshot;
      if (!pending) throw new Error("unsolicited owner Face response");
      clearTimeout(pending.timeout);
      pendingFaceSnapshot = null;
      if (frameBytes.length < 1 || frameBytes.length > MAX_OWNER_FACE_RESPONSE_BYTES + 256 ||
          frame.response?.schema !== OWNER_FACE_RESPONSE_SCHEMA ||
          !["snapshot", "unchanged", "refused"].includes(frame.response?.outcome)) {
        pending.reject(new Error("invalid bounded owner Face response"));
      } else {
        // Keep the exact u64 revision bytes. JSON.parse rounds them in JS;
        // the production WASM Mask decodes and validates this original frame.
        pending.resolve(frameBytes.slice());
      }
    } else if (frame.kind === "face-show-response" && frame.protocol === 1) {
      const pending = pendingFaceShow;
      if (!pending) throw new Error("unsolicited owner Show response");
      clearTimeout(pending.timeout);
      pendingFaceShow = null;
      if (frame.accepted === true && frame.code === "") {
        pending.resolve(Object.freeze({ accepted: true }));
      } else if (frame.accepted === false && typeof frame.code === "string" &&
          frame.code.length > 0 && frame.code.length <= 128) {
        pending.reject(new Error(`Body owner refused Show: ${frame.code}`));
      } else {
        pending.reject(new Error("invalid owner Show response"));
      }
    } else if (frame.kind === "face-interaction-response" && frame.protocol === 1) {
      const pending = pendingFaceInteraction;
      if (!pending) throw new Error("unsolicited owner interaction response");
      clearTimeout(pending.timeout);
      pendingFaceInteraction = null;
      if (frame.accepted === true && frame.code === "") {
        pending.resolve(Object.freeze({ accepted: true }));
      } else if (frame.accepted === false && typeof frame.code === "string" &&
          frame.code.length > 0 && frame.code.length <= 128) {
        const uncertain = frame.code === "control-outcome-unknown";
        const error = new Error(uncertain
          ? "Body owner control outcome unknown; do not retry this interaction"
          : `Body owner refused interaction: ${frame.code}`);
        error.code = frame.code;
        pending.reject(error);
      } else {
        pending.reject(new Error("invalid owner interaction response"));
      }
    } else if (frame.kind === "face-wardrobe-response" && frame.protocol === 1) {
      const pending = pendingWardrobe;
      if (!pending || frame.request_id !== pending.requestId) {
        throw new Error("unsolicited owner wardrobe response");
      }
      clearTimeout(pending.timeout);
      pendingWardrobe = null;
      try { pending.resolve(checkedOwnerWardrobeResponse(frame, pending, frameBytes.length)); }
      catch (error) { pending.reject(error); }
    } else if (frame.kind === "selected-speech-response" && frame.protocol === 1) {
      const pending = pendingSelectedSpeech;
      if (!pending || frame.request_id !== pending.requestId) throw new Error("unsolicited or mismatched selected speech response");
      clearTimeout(pending.timeout);
      pendingSelectedSpeech = null;
      try { pending.resolve(checkedSelectedSpeechResponse(frame, pending, frameBytes.length)); }
      catch (error) { pending.reject(error); }
    } else if (frame.kind === "media-use-plan" && frame.protocol === 1) {
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
      let signature;
      if (admitted) {
        signature = Uint8Array.from(admittedHost.membership.proveAdmission(frame.challenge).signature);
      } else {
        writeInput(bytes);
        requireSuccess(api.conduit_browser_membership_prove(bytes.length), "browser admission proof");
        signature = readOutput();
      }
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
      let signature;
      if (admitted) {
        signature = Uint8Array.from(admittedHost.membership.proveReturn(frame.challenge).signature);
      } else {
        writeInput(bytes);
        requireSuccess(
          api.conduit_browser_membership_prove_return(bytes.length),
          "browser Part return proof",
        );
        signature = readOutput();
      }
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
      credential = requireCredential(frame.credential, {
        expectedBodyId,
        hostId,
        bootId,
        prior: returning ? credential : null,
      });
      offerEvidence = null;
      credentialPersistence = Promise.resolve(onCredential?.(credential));
      await credentialPersistence;
      setState("admitted");
    } else if (frame.kind === "biography-evidence" && frame.protocol === 1) {
      const evidence = frame.evidence;
      const encodedEvidence = encoder.encode(JSON.stringify(evidence));
      const part = evidence?.membership?.parts?.find(part => part.part_id === credential?.part_id);
      const current = part?.current;
      const latestRecord = evidence?.records?.at(-1);
      const detached = part?.state === "Admitted" && current === null &&
        latestRecord?.kind?.HostLeft?.part_id === credential?.part_id &&
        latestRecord.kind.HostLeft.prior_boot_id === bootId;
      if (!credential || encodedEvidence.length === 0 || encodedEvidence.length > 65_536 ||
          evidence?.schema !== "conduit.body/biography-evidence@2" ||
          evidence.body_id !== credential.body_id || evidence.membership?.body_id !== credential.body_id ||
          (!detached && (current?.host_id !== hostId || current?.boot_id !== bootId)) ||
          !Array.isArray(evidence.records)) {
        throw new Error("invalid admission biography evidence");
      }
      const freeze = value => { if (value && typeof value === "object" && !Object.isFrozen(value)) { Object.values(value).forEach(freeze);Object.freeze(value); } return value; };
      biographyEvidence = freeze(evidence);
      onBiographyEvidence?.(biographyEvidence);
      if (detached) socket.close(1000, "Body recorded browser Host leave");
    } else if (frame.kind === "offer-evidence" && frame.protocol === 1) {
      const evidence = frame.evidence;
      const summary = evidence?.capability_summary;
      const admittedSummary = evidence?.stage === "AdmittedMembership";
      const planningDetail = evidence?.stage === "Planning";
      if (!credential || (!admittedSummary && !planningDetail) || evidence.protocol_version !== 1 ||
          evidence.host_id !== credential.host_id || evidence.boot_id !== credential.boot_id ||
          evidence.offer_generation !== advertisement.offer_generation ||
          typeof evidence.observation_sign_id !== "string" || evidence.observation_sign_id.length === 0 ||
          !Number.isSafeInteger(evidence.freshness_sequence) || evidence.freshness_sequence < 1 ||
          typeof evidence.profile !== "string" || evidence.profile !== advertisement.profile ||
          !Array.isArray(summary) || summary.length > 16 || !Array.isArray(evidence.capabilities) || evidence.capabilities.length > 16 || !Array.isArray(evidence.resources) || evidence.resources.length > 16 ||
          (admittedSummary && (evidence.capabilities.length !== 0 || evidence.resources.length !== 0)) ||
          (planningDetail && (summary.length !== 0 || evidence.capabilities.length + evidence.resources.length === 0)) ||
          summary.some((item, index) => typeof item?.capability_id !== "string" || item.capability_id.length === 0 ||
            typeof item.implementation_id !== "string" || item.implementation_id.length === 0 ||
            (index > 0 && summary[index - 1].capability_id >= item.capability_id))) {
        throw new Error("invalid admitted browser offer evidence");
      }
      const advertised = new Map(advertisement.capabilities.map(offer => [offer.capability_id, offer.implementation_id]));
      if (summary.some(item => advertised.get(item.capability_id) !== item.implementation_id) ||
          evidence.capabilities.some(item => !advertisement.capabilities.some(offer => offer.capability_id === item.capability_id && JSON.stringify(offer) === JSON.stringify(item))) ||
          evidence.resources.some(item => !advertisement.resources.some(offer => offer.pool_id === item.pool_id && JSON.stringify(offer) === JSON.stringify(item)))) {
        throw new Error("browser offer evidence does not match the current advertisement");
      }
      const freeze = value => { if (value && typeof value === "object" && !Object.isFrozen(value)) { Object.values(value).forEach(freeze);Object.freeze(value); } return value; };
      offerEvidence = freeze(evidence);
      onOfferEvidence?.(offerEvidence);
    } else if (frame.kind === "presence-accepted" && frame.protocol === 1) {
      await credentialPersistence;
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
      if (pendingFaceSnapshot) {
        clearTimeout(pendingFaceSnapshot.timeout);
        pendingFaceSnapshot.reject(new Error("owner Face Line closed"));
        pendingFaceSnapshot = null;
      }
      if (pendingFaceShow) {
        clearTimeout(pendingFaceShow.timeout);
        pendingFaceShow.reject(new Error("owner Show Line closed"));
        pendingFaceShow = null;
      }
      if (pendingFaceInteraction) {
        clearTimeout(pendingFaceInteraction.timeout);
        pendingFaceInteraction.reject(new Error("owner interaction Line closed"));
        pendingFaceInteraction = null;
      }
      if (pendingWardrobe) {
        clearTimeout(pendingWardrobe.timeout);
        pendingWardrobe.reject(new Error("owner wardrobe Line closed; action outcome unknown"));
        pendingWardrobe = null;
      }
      if (pendingSelectedSpeech) {
        clearTimeout(pendingSelectedSpeech.timeout);
        pendingSelectedSpeech.reject(new Error("selected speech owner Line closed"));
        pendingSelectedSpeech = null;
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
  openSocket(priorCredential !== null, priorCredential !== null);
  function publishMediaResource(mediaEvidence) {
    if (!credential || presenceState !== "available" || socket?.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error("current body membership is required for media resource truth"));
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
  function requestOfferEvidence({ capabilityIds = [], resourcePoolIds = [] }) {
    if (!credential || presenceState !== "available" || socket?.readyState !== WebSocket.OPEN) throw new Error("current body membership is required for offer disclosure");
    const canonical = values => Array.isArray(values) && values.length > 0 && values.length <= 16 && values.every((value, index) => typeof value === "string" && value.length > 0 && (index === 0 || values[index - 1] < value));
    if ((!canonical(capabilityIds) && capabilityIds.length !== 0) || (!canonical(resourcePoolIds) && resourcePoolIds.length !== 0) || capabilityIds.length + resourcePoolIds.length === 0) throw new Error("offer disclosure selection must be finite and canonical");
    socket.send(encoder.encode(JSON.stringify({kind:"offer-disclosure-request",protocol:1,credential_id:credential.credential_id,body_id:credential.body_id,part_id:credential.part_id,host_id:hostId,boot_id:bootId,request:{stage:"Planning",capability_ids:capabilityIds,resource_pool_ids:resourcePoolIds}})));
  }
  function requestFaceSnapshot({ lastSeenRevision = null, lastSeenIdentity = null } = {}) {
    if (!credential || presenceState !== "available" || socket?.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error("current browser presence is required for the owner Face"));
    }
    if (pendingFaceSnapshot) return Promise.reject(new Error("one owner Face request is already pending"));
    if ((lastSeenRevision === null) !== (lastSeenIdentity === null) ||
        (lastSeenRevision !== null && (typeof lastSeenRevision !== "string" ||
          !/^(0|[1-9][0-9]{0,19})$/.test(lastSeenRevision) ||
          BigInt(lastSeenRevision) > 18_446_744_073_709_551_615n ||
          typeof lastSeenIdentity !== "string" || lastSeenIdentity.length < 1 || lastSeenIdentity.length > 256))) {
      return Promise.reject(new Error("owner Face prior basis must be one exact revision and identity"));
    }
    const request = {
      schema: OWNER_FACE_REQUEST_SCHEMA,
      credential_id: credential.credential_id,
      body_id: credential.body_id,
      part_id: credential.part_id,
      host_id: credential.host_id,
      boot_id: credential.boot_id,
      last_seen_revision: null,
      last_seen_identity: lastSeenIdentity,
    };
    // Preserve the exact Rust u64 while serializing from JavaScript; JSON
    // numbers above 2^53 cannot make a round trip through Number.
    const serialized = JSON.stringify({ kind: "face-snapshot-request", protocol: 1, request });
    const bytes = encoder.encode(lastSeenRevision === null ? serialized
      : serialized.replace('"last_seen_revision":null', `"last_seen_revision":${lastSeenRevision}`));
    if (bytes.length > INPUT_CAPACITY) return Promise.reject(new Error("owner Face request exceeds its finite bound"));
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        if (pendingFaceSnapshot?.resolve === resolve) {
          pendingFaceSnapshot = null;
          reject(new Error("owner Face response deadline"));
        }
      }, 5_000);
      pendingFaceSnapshot = { resolve, reject, timeout };
      try { socket.send(bytes); }
      catch (error) { clearTimeout(timeout); pendingFaceSnapshot = null; reject(error); }
    });
  }
  function submitFaceInteraction(submissionBytes) {
    if (!credential || presenceState !== "available" || socket?.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error("current browser presence is required for owner interaction"));
    }
    if (pendingFaceInteraction) return Promise.reject(new Error("one owner interaction is already pending"));
    if (!(submissionBytes instanceof Uint8Array) || submissionBytes.length < 1 || submissionBytes.length > 64 * 1024) {
      return Promise.reject(new Error("owner Mask interaction emission violates its finite bound"));
    }
    // The WASM Mask serialized Show and FaceInteraction with exact u64 fields.
    // Embed those bytes without a JavaScript JSON number round trip.
    const request = {
      schema: OWNER_FACE_REQUEST_SCHEMA,
      credential_id: credential.credential_id,
      body_id: credential.body_id,
      part_id: credential.part_id,
      host_id: credential.host_id,
      boot_id: credential.boot_id,
      last_seen_revision: null,
      last_seen_identity: null,
    };
    const emission = decoder.decode(submissionBytes);
    if (!emission.startsWith('{"show":') || !emission.endsWith('}')) {
      return Promise.reject(new Error("invalid owner Mask interaction emission"));
    }
    const bytes = encoder.encode(`{"kind":"face-interaction-request","protocol":1,"request":${JSON.stringify(request)},${emission.slice(1)}`);
    if (bytes.length > 193 * 1024) return Promise.reject(new Error("owner interaction frame exceeds its admitted bound"));
    socket.send(bytes);
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        if (pendingFaceInteraction?.resolve === resolve) {
          pendingFaceInteraction = null;
          reject(new Error("owner interaction timed out"));
        }
      }, MEDIA_PLAN_TIMEOUT_MILLIS);
      pendingFaceInteraction = { resolve, reject, timeout };
    });
  }
  function ownerWardrobe({ action = null, ownerPlanId = null, basisRevision = "0" } = {}) {
    if (!credential || presenceState !== "available" || socket?.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error("current browser presence is required for owner wardrobe"));
    }
    if (pendingWardrobe) return Promise.reject(new Error("one owner wardrobe request is already pending"));
    if ((action === null) !== (ownerPlanId === null) ||
        (ownerPlanId !== null && (typeof ownerPlanId !== "string" || !ownerPlanId || ownerPlanId.length > 256))) {
      return Promise.reject(new Error("wardrobe action needs its exact owner Plan"));
    }
    if (action !== null && (typeof action !== "object" || Array.isArray(action) ||
        Object.keys(action).length !== 1 || !["Wear", "Doff", "Prefer"].includes(Object.keys(action)[0]))) {
      return Promise.reject(new Error("wardrobe action is not a declared Wear, Doff, or Prefer"));
    }
    const requestId = `browser-wardrobe:${++wardrobeSequence}`;
    const request = { schema: OWNER_FACE_REQUEST_SCHEMA, credential_id: credential.credential_id,
      body_id: credential.body_id, part_id: credential.part_id, host_id: credential.host_id,
      boot_id: credential.boot_id, last_seen_revision: null, last_seen_identity: null };
    let bytes;
    try { bytes = checkedOwnerWardrobeRequestFrame({ requestId, request, ownerPlanId, basisRevision, action }); }
    catch (error) { return Promise.reject(error); }
    if (bytes.length > INPUT_CAPACITY) return Promise.reject(new Error("owner wardrobe request exceeds its finite bound"));
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        if (pendingWardrobe?.requestId === requestId) {
          pendingWardrobe = null;
          if (action !== null) socket.close(1000, "Wardrobe action outcome unknown");
          reject(new Error("owner wardrobe response deadline; action outcome unknown"));
        }
      }, MEDIA_PLAN_TIMEOUT_MILLIS);
      pendingWardrobe = { requestId, resolve, reject, timeout };
      try { socket.send(bytes); }
      catch (error) { clearTimeout(timeout); pendingWardrobe = null; reject(error); }
    });
  }
  function acknowledgeFaceShow(showBytes) {
    if (!credential || presenceState !== "available" || socket?.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error("current browser presence is required for owner Show"));
    }
    if (pendingFaceShow) return Promise.reject(new Error("one owner Show acknowledgement is already pending"));
    if (!(showBytes instanceof Uint8Array) || showBytes.length < 1 || showBytes.length > 64 * 1024) {
      return Promise.reject(new Error("owner Mask Show receipt violates its finite bound"));
    }
    const show = decoder.decode(showBytes);
    if (!show.startsWith('{') || !show.endsWith('}')) {
      return Promise.reject(new Error("invalid owner Mask Show receipt"));
    }
    const request = {
      schema: OWNER_FACE_REQUEST_SCHEMA,
      credential_id: credential.credential_id,
      body_id: credential.body_id,
      part_id: credential.part_id,
      host_id: credential.host_id,
      boot_id: credential.boot_id,
      last_seen_revision: null,
      last_seen_identity: null,
    };
    const bytes = encoder.encode(`{"kind":"face-show-acknowledgement","protocol":1,"request":${JSON.stringify(request)},"show":${show}}`);
    if (bytes.length > 193 * 1024) return Promise.reject(new Error("owner Show frame exceeds its admitted bound"));
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        if (pendingFaceShow?.resolve === resolve) {
          pendingFaceShow = null;
          reject(new Error("owner Show acknowledgement timed out"));
        }
      }, MEDIA_PLAN_TIMEOUT_MILLIS);
      pendingFaceShow = { resolve, reject, timeout };
      try { socket.send(bytes); }
      catch (error) { clearTimeout(timeout); pendingFaceShow = null; reject(error); }
    });
  }
  function selectedSpeech(kind, { showBytes = null, operationId = null } = {}) {
    if (!credential || presenceState !== "available" || socket?.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error("current browser presence is required for selected speech"));
    }
    if (pendingSelectedSpeech) return Promise.reject(new Error("one selected speech request is already pending"));
    if (!["start", "status", "stop"].includes(kind)) return Promise.reject(new Error("unknown selected speech request"));
    const requestId = `browser-speech/${++selectedSpeechSequence}`;
    let serialized;
    if (kind === "start") {
      if (!(showBytes instanceof Uint8Array) || showBytes.length < 1 || showBytes.length > 64 * 1024) {
        return Promise.reject(new Error("current bounded owner Show receipt is required for speech"));
      }
      const show = decoder.decode(showBytes);
      if (!show.startsWith('{') || !show.endsWith('}')) return Promise.reject(new Error("invalid owner Show receipt"));
      const request = { schema: OWNER_FACE_REQUEST_SCHEMA, credential_id: credential.credential_id,
        body_id: credential.body_id, part_id: credential.part_id, host_id: credential.host_id,
        boot_id: credential.boot_id, last_seen_revision: null, last_seen_identity: null };
      serialized = `{"kind":"selected-speech-start","protocol":1,"request_id":${JSON.stringify(requestId)},"request":${JSON.stringify(request)},"show":${show}}`;
    } else {
      if (typeof operationId !== "string" || !operationId || operationId.length > 256) {
        return Promise.reject(new Error("one exact selected speech operation is required"));
      }
      serialized = JSON.stringify({ kind: `selected-speech-${kind}`, protocol: 1,
        request_id: requestId, operation_id: operationId });
    }
    const bytes = encoder.encode(serialized);
    if (bytes.length > 193 * 1024) return Promise.reject(new Error("selected speech frame exceeds its admitted bound"));
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        if (pendingSelectedSpeech?.requestId === requestId) {
          pendingSelectedSpeech = null;
          if (kind === "start") socket.close(1000, "Selected speech start outcome unknown");
          reject(new Error("selected speech owner response deadline; outcome unknown"));
        }
      }, MEDIA_PLAN_TIMEOUT_MILLIS);
      pendingSelectedSpeech = { requestId, kind, operationId, resolve, reject, timeout };
      try { socket.send(bytes); }
      catch (error) { clearTimeout(timeout); pendingSelectedSpeech = null; reject(error); }
    });
  }
  return Object.freeze({
    hostId,
    bootId,
    membershipCredential: () => credential === undefined ? null : Object.freeze({ ...credential }),
    biographyEvidence: () => biographyEvidence,
    offerEvidence: () => offerEvidence,
    requestOfferEvidence,
    requestFaceSnapshot,
    ownerWardrobe,
    acknowledgeFaceShow,
    selectedSpeech,
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
    submitFaceInteraction,
    publishMediaResource,
    close: () => {
      clearTimeout(renewalTimer);
      webRtcSessions.reset("presence-closed");
      if (!credential || presenceState !== "available" || socket?.readyState !== WebSocket.OPEN) {
        deliberateClose = true;
        socket?.close(1000, "Browser Host leaving");
        return renewalSequence;
      }
      deliberateClose = true;
      renewalSequence += 1;
      setState("leaving");
      socket.send(encoder.encode(JSON.stringify({
        kind: "presence-leave",
        protocol: 1,
        credential_id: credential.credential_id,
        body_id: credential.body_id,
        part_id: credential.part_id,
        host_id: credential.host_id,
        boot_id: credential.boot_id,
        sequence: renewalSequence,
      })));
      return renewalSequence;
    },
  });
}
