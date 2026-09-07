const EFFECT_OPEN = 1;
const EFFECT_RECEIVE = 2;
const EFFECT_SEND = 3;
const EFFECT_CLOSE = 4;
const EFFECT_PRESENT = 5;
const STATUS_COMPLETE = 1;
const INPUT_CAPACITY = 4096;
const MAXIMUM_MEMBERSHIP_OUTPUT_BYTES = 72 * 1024;

function requireApi(api) {
  const names = [
    "memory",
    "conduit_browser_webchat_input_ptr",
    "conduit_browser_webchat_input_capacity",
    "conduit_browser_webchat_start",
    "conduit_browser_webchat_status",
    "conduit_browser_webchat_effect_kind",
    "conduit_browser_webchat_effect_ptr",
    "conduit_browser_webchat_effect_len",
    "conduit_browser_webchat_complete_effect",
    "conduit_browser_webchat_receive",
    "conduit_browser_webchat_submit",
    "conduit_browser_webchat_disconnect",
    "conduit_browser_webchat_identity_ptr",
    "conduit_browser_webchat_identity_len",
    "conduit_browser_webchat_disconnected",
    "conduit_browser_webchat_capacity_stable",
    "conduit_browser_webchat_request_count",
    "conduit_browser_webchat_interaction_ptr",
    "conduit_browser_webchat_interaction_len",
    "conduit_browser_webchat_evidence_ptr",
    "conduit_browser_webchat_evidence_len",
    "conduit_browser_membership_input_ptr",
    "conduit_browser_membership_input_capacity",
    "conduit_browser_membership_output_ptr",
    "conduit_browser_membership_output_len",
    "conduit_browser_membership_initialize",
    "conduit_browser_membership_prove",
    "conduit_browser_membership_prove_spawn",
    "conduit_browser_membership_advertisement",
  ];
  if (names.some((name) => !(name in api)) ||
      api.conduit_browser_webchat_input_capacity() !== INPUT_CAPACITY ||
      api.conduit_browser_membership_input_capacity() !== INPUT_CAPACITY) {
    throw new Error("CND-CHAT-001 incomplete browser webchat ABI");
  }
}

function readBytes(api, pointer, length) {
  if (length < 0 || length > MAXIMUM_MEMBERSHIP_OUTPUT_BYTES) {
    throw new Error("CND-CHAT-002 invalid WASM frame length");
  }
  return new Uint8Array(api.memory.buffer, pointer, length).slice();
}

function writeInput(api, bytes) {
  if (!(bytes instanceof Uint8Array) || bytes.length > INPUT_CAPACITY) {
    throw new Error("CND-CHAT-003 invalid host input");
  }
  new Uint8Array(
    api.memory.buffer,
    api.conduit_browser_webchat_input_ptr(),
    bytes.length,
  ).set(bytes);
}

function requireStatus(status, action) {
  if (status < 0) throw new Error(`CND-CHAT-004 ${action} failed ${status}`);
}

export async function createWebchatRuntime({ wasmBytes, url, form = "webchat-browser-demo", bodyUrl = null, spawn = null, root }) {
  const { instance } = await WebAssembly.instantiate(wasmBytes, {});
  const api = instance.exports;
  requireApi(api);
  const encoder = new TextEncoder();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  const hostId = `browser/${crypto.randomUUID()}`;
  const bootId = `browser-boot/${crypto.randomUUID()}`;
  const seed = crypto.getRandomValues(new Uint8Array(32));
  const hostBytes = encoder.encode(hostId);
  const bootBytes = encoder.encode(bootId);
  const membershipFrame = new Uint8Array(hostBytes.length + bootBytes.length + seed.length);
  membershipFrame.set(hostBytes);
  membershipFrame.set(bootBytes, hostBytes.length);
  membershipFrame.set(seed, hostBytes.length + bootBytes.length);
  new Uint8Array(
    api.memory.buffer,
    api.conduit_browser_membership_input_ptr(),
    membershipFrame.length,
  ).set(membershipFrame);
  requireStatus(
    api.conduit_browser_membership_initialize(hostBytes.length, bootBytes.length),
    "membership initialization",
  );
  seed.fill(0);
  membershipFrame.fill(0);
  const verifyingKey = readBytes(
    api,
    api.conduit_browser_membership_output_ptr(),
    api.conduit_browser_membership_output_len(),
  );
  if (verifyingKey.length !== 32) {
    throw new Error("CND-CHAT-010 invalid membership verifying key");
  }
  requireStatus(
    api.conduit_browser_membership_advertisement(),
    "membership advertisement",
  );
  const advertisement = JSON.parse(decoder.decode(readBytes(
    api,
    api.conduit_browser_membership_output_ptr(),
    api.conduit_browser_membership_output_len(),
  )));
  const startFrame = encoder.encode(`${url}\n${hostId}\n${bootId}\n${form}`);
  writeInput(api, startFrame);
  requireStatus(api.conduit_browser_webchat_start(startFrame.length), "start");

  let socket = null;
  let bodySocket = null;
  let bodyState = bodyUrl ? "connecting" : "not-configured";
  let closed = false;
  let currentPresentation = null;
  let currentManifestation = null;
  let interactionSequence = 0;
  let chain = Promise.resolve();
  const enqueue = (action) => {
    chain = chain.then(action).catch((error) => {
      root.replaceChildren(document.createTextNode(`error:${error.stack ?? error}`));
      throw error;
    });
    return chain;
  };
  const effectBytes = () => readBytes(
    api,
    api.conduit_browser_webchat_effect_ptr(),
    api.conduit_browser_webchat_effect_len(),
  );

  const contained = (presentation, source) => presentation.relationships
    .filter((relationship) => relationship.kind === "Contains" && relationship.source === source)
    .map((relationship) => relationship.target);
  const subjectById = (presentation, identity) => presentation.subjects.find((subject) => subject.identity === identity);
  const textById = (presentation, identity) => presentation.text.find((entry) => entry.subject === identity)?.text ?? "";

  function renderPresentation(presentation) {
    const documentSubject = presentation.subjects.find((subject) => subject.role === "Document");
    if (!documentSubject) throw new Error("CND-CHAT-015 Presentation has no document subject");
    const fragment = document.createDocumentFragment();
    const heading = document.createElement("h1");
    heading.textContent = documentSubject.label;
    fragment.append(heading);
    const renderSubject = (identity) => {
      const subject = subjectById(presentation, identity);
      if (!subject) return null;
      if (subject.role === "Collection") {
        const list = document.createElement("ol");
        list.setAttribute("aria-label", subject.accessibility_name);
        for (const child of contained(presentation, identity)) {
          const rendered = renderSubject(child);
          if (rendered) list.append(rendered);
        }
        return list;
      }
      if (subject.role === "Item") {
        const item = document.createElement("li"); item.textContent = textById(presentation, identity); return item;
      }
      if (subject.role === "Status") {
        const status = document.createElement("p"); status.setAttribute("role", "status"); status.textContent = textById(presentation, identity).toLowerCase(); return status;
      }
      if (subject.role === "TextEntry") {
        const contract = presentation.inputs.find((input) => input.target === identity);
        if (!contract) return null;
        const label = document.createElement("label"); label.textContent = contract.label;
        const input = document.createElement("input");
        input.setAttribute("aria-label", contract.accessibility_name);
        input.maxLength = contract.maximum_bytes;
        input.dataset.inputId = contract.identity;
        input.addEventListener("keydown", (event) => {
          if (event.key === "Enter") enqueue(() => submitInteraction(input, contract));
        });
        label.append(input);
        const action = presentation.actions.find((candidate) => candidate.identity === contract.submit_action);
        if (action) {
          const button = document.createElement("button"); button.type = "button"; button.textContent = action.label;
          button.disabled = action.availability !== "Available";
          button.addEventListener("click", () => enqueue(() => submitInteraction(input, contract)));
          label.append(button);
        }
        return label;
      }
      return null;
    };
    for (const child of contained(presentation, documentSubject.identity)) {
      const rendered = renderSubject(child); if (rendered) fragment.append(rendered);
    }
    root.replaceChildren(fragment);
  }

  async function pump() {
    for (;;) {
      const effect = api.conduit_browser_webchat_effect_kind();
      if (effect === EFFECT_RECEIVE || api.conduit_browser_webchat_status() === STATUS_COMPLETE) {
        return;
      }
      if (effect === EFFECT_OPEN) {
        const requestedUrl = decoder.decode(effectBytes());
        socket = new WebSocket(requestedUrl);
        socket.binaryType = "arraybuffer";
        await new Promise((resolve, reject) => {
          socket.addEventListener("open", resolve, { once: true });
          socket.addEventListener("error", () => reject(new Error("CND-CHAT-005 WebSocket open failed")), { once: true });
        });
        socket.addEventListener("message", (event) => enqueue(async () => {
          const bytes = new Uint8Array(event.data);
          writeInput(api, bytes);
          requireStatus(api.conduit_browser_webchat_receive(bytes.length), "receive");
          await pump();
        }));
        socket.addEventListener("close", () => enqueue(async () => {
          if (!closed && api.conduit_browser_webchat_effect_kind() === EFFECT_RECEIVE) {
            closed = true;
            requireStatus(api.conduit_browser_webchat_disconnect(), "disconnect");
            await pump();
          }
        }));
        requireStatus(api.conduit_browser_webchat_complete_effect(), "open completion");
        continue;
      }
      if (effect === EFFECT_SEND) {
        const bytes = effectBytes();
        if (socket?.readyState !== WebSocket.OPEN) {
          throw new Error("CND-CHAT-007 send without an open socket");
        }
        socket.send(bytes);
        requireStatus(api.conduit_browser_webchat_complete_effect(), "send completion");
        continue;
      }
      if (effect === EFFECT_PRESENT) {
        currentPresentation = JSON.parse(decoder.decode(effectBytes()));
        renderPresentation(currentPresentation);
        requireStatus(api.conduit_browser_webchat_complete_effect(), "presentation completion");
        currentManifestation = JSON.parse(decoder.decode(readBytes(
          api, api.conduit_browser_webchat_interaction_ptr(), api.conduit_browser_webchat_interaction_len(),
        )));
        continue;
      }
      if (effect === EFFECT_CLOSE) {
        socket?.close(1000, "Conduit close");
        requireStatus(api.conduit_browser_webchat_complete_effect(), "close completion");
        continue;
      }
      throw new Error(`CND-CHAT-008 unsupported effect ${effect}`);
    }
  }

  async function submitInteraction(input, contract) {
    const frame = encoder.encode(JSON.stringify({
      presentation_id: currentPresentation.identity,
      presentation_revision: currentPresentation.revision,
      manifestation_id: currentManifestation.manifestation_id,
      input_id: contract.identity,
      action_id: contract.submit_action,
      target: contract.target,
      value_kind: contract.value_kind,
      sequence: interactionSequence++,
      value: input.value,
    }));
    writeInput(api, frame);
    requireStatus(api.conduit_browser_webchat_submit(frame.length), "interaction");
    input.value = "";
    await pump();
  }

  async function disconnect() {
    if (!closed && api.conduit_browser_webchat_effect_kind() === EFFECT_RECEIVE) {
      closed = true;
      requireStatus(api.conduit_browser_webchat_disconnect(), "disconnect");
      socket?.close(1000, "Conduit disconnect");
      await pump();
    }
  }

  await pump();
  const identity = decoder.decode(readBytes(
    api,
    api.conduit_browser_webchat_identity_ptr(),
    api.conduit_browser_webchat_identity_len(),
  ));
  function proveAdmission(challenge) {
    const bytes = encoder.encode(JSON.stringify(challenge));
    if (bytes.length === 0 || bytes.length > api.conduit_browser_membership_input_capacity()) {
      throw new Error("CND-CHAT-011 invalid admission challenge length");
    }
    new Uint8Array(
      api.memory.buffer,
      api.conduit_browser_membership_input_ptr(),
      bytes.length,
    ).set(bytes);
    requireStatus(api.conduit_browser_membership_prove(bytes.length), "admission proof");
    const signature = readBytes(
      api,
      api.conduit_browser_membership_output_ptr(),
      api.conduit_browser_membership_output_len(),
    );
    if (signature.length !== 64) {
      throw new Error("CND-CHAT-012 invalid admission signature");
    }
    return signature;
  }
  function proveSpawn(invitation) {
    const bytes = encoder.encode(JSON.stringify(invitation));
    if (bytes.length === 0 || bytes.length > api.conduit_browser_membership_input_capacity()) {
      throw new Error("CND-CHAT-013 invalid spawn invitation length");
    }
    new Uint8Array(
      api.memory.buffer,
      api.conduit_browser_membership_input_ptr(),
      bytes.length,
    ).set(bytes);
    requireStatus(api.conduit_browser_membership_prove_spawn(bytes.length), "spawn proof");
    const signature = readBytes(
      api,
      api.conduit_browser_membership_output_ptr(),
      api.conduit_browser_membership_output_len(),
    );
    if (signature.length !== 64) throw new Error("CND-CHAT-014 invalid spawn signature");
    return signature;
  }
  if (bodyUrl) {
    bodySocket = new WebSocket(bodyUrl);
    bodySocket.binaryType = "arraybuffer";
    bodySocket.addEventListener("open", () => {
      bodyState = "wants-to-join";
      bodySocket.send(encoder.encode(JSON.stringify({
        kind: "advertise",
        protocol: 1,
        advertisement,
        friendly_label: "Browser",
        verifying_key: Array.from(verifyingKey),
        freshness_sequence: 1,
      })));
      if (spawn) {
        const signature = proveSpawn(spawn);
        bodyState = "spawn-proof-sent";
        bodySocket.send(encoder.encode(JSON.stringify({
          kind: "spawn-proof",
          protocol: 1,
          invitation_id: spawn.claim.invitation_id,
          body_id: spawn.claim.body_id,
          host_id: hostId,
          boot_id: bootId,
          nonce: spawn.claim.nonce,
          signature: Array.from(signature),
        })));
      }
    });
    bodySocket.addEventListener("message", (event) => {
      const frame = JSON.parse(typeof event.data === "string"
        ? event.data
        : decoder.decode(new Uint8Array(event.data)));
      if (frame.kind === "challenge" && frame.protocol === 1) {
        const signature = proveAdmission(frame.challenge);
        bodyState = "proof-sent";
        bodySocket.send(encoder.encode(JSON.stringify({
          kind: "ambient-proof",
          protocol: 1,
          admission_id: frame.challenge.admission_id,
          body_id: frame.challenge.body_id,
          host_id: hostId,
          boot_id: bootId,
          nonce: frame.challenge.nonce,
          signature: Array.from(signature),
        })));
      } else if (frame.kind === "admitted" && frame.protocol === 1) {
        bodyState = "admitted";
      } else if (frame.kind === "refused" && frame.protocol === 1) {
        bodyState = `refused:${frame.code}`;
      }
    });
    bodySocket.addEventListener("close", () => {
      if (bodyState !== "admitted" && !bodyState.startsWith("refused:")) {
        bodyState = "offline";
      }
    });
  }
  return Object.freeze({
    submit: (text) => enqueue(async () => {
      const contract = currentPresentation.inputs[0];
      const input = root.querySelector(`[data-input-id="${contract.identity}"]`);
      input.value = text;
      await submitInteraction(input, contract);
    }),
    disconnect: () => enqueue(disconnect),
    refusal: (overrides = {}) => {
      const contract = currentPresentation.inputs[0];
      const frame = encoder.encode(JSON.stringify({
        presentation_id: currentPresentation.identity,
        presentation_revision: currentPresentation.revision,
        manifestation_id: currentManifestation.manifestation_id,
        input_id: contract.identity,
        action_id: contract.submit_action,
        target: contract.target,
        value_kind: contract.value_kind,
        sequence: interactionSequence,
        value: "refusal-probe",
        ...overrides,
      }));
      writeInput(api, frame);
      return api.conduit_browser_webchat_submit(frame.length);
    },
    admissionCandidate: Object.freeze({
      hostId,
      bootId,
      verifyingKey,
      advertisement: Object.freeze(advertisement),
      prove: proveAdmission,
    }),
    bodyAdmission: Object.freeze({
      state: () => bodyState,
    }),
    proof: () => Object.freeze({
      identity,
      history: Object.freeze([...root.querySelectorAll("li")].map((item) => item.textContent)),
      presentationId: currentPresentation?.identity,
      presentationRevision: currentPresentation?.revision,
      manifestationId: currentManifestation?.manifestation_id,
      interactionEvidence: api.conduit_browser_webchat_evidence_len() === 0 ? null : JSON.parse(decoder.decode(readBytes(
        api, api.conduit_browser_webchat_evidence_ptr(), api.conduit_browser_webchat_evidence_len(),
      ))),
      requestCount: api.conduit_browser_webchat_request_count(),
      capacityStable: api.conduit_browser_webchat_capacity_stable() === 1,
      disconnected: api.conduit_browser_webchat_disconnected() === 1,
      status: api.conduit_browser_webchat_status(),
    }),
  });
}
