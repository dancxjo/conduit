import { nameFor, NAMING_SYSTEM_OPTIONS } from "./creche-names.mjs";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function createBodyBirthRunner({ source, sourceKey, listingId, host, presentationFor, draft, onDraft, nextSequence, onBodyChanged }) {
  const runner = document.createElement("section");
  runner.className = "runner body-birth-runner";
  runner.dataset.sourceKey = sourceKey;
  runner.innerHTML = `
    <div class="birth-presentation">
      <div data-application-slot="birth-fields"></div>
      <div data-application-slot="birth-source"></div>
    </div>
    <div class="result body-birth-result">
      <div class="body-chain" aria-label="Seed to Body lifecycle">
        <article><span>checked Seed</span><code class="seed-id">not born</code></article>
        <b aria-hidden="true">BIRTH →</b>
        <article><span>durable Body</span><strong class="body-state">not born</strong><code class="body-id"></code></article>
      </div>
      <div class="body-evidence" data-application-slot="birth-evidence"></div>
    </div>`;
  const presentation = presentationFor(runner);
  const state = {
    revision: 0,
    friendlyName: "Choosing a persona…",
    namingSystem: "surprise",
    namingLabel: "Surprise me",
    personaUuid: crypto.randomUUID(),
    variation: 0,
    namingRequest: 0,
    pending: true,
    initialProgram: "morse-network@1",
    source: draft ?? source,
    status: "Edit the Seed, then explicitly birth one Body.",
    outcome: "status",
    terminal: false,
  };
  const current = readCurrent(host.runtime);
  const controls = { presentation, listingId, onDraft, onBirth() {
    birth(runner, host, state, nextSequence(), onBodyChanged, controls);
  } };
  if (current) {
    state.pending = false;
    renderReceipt(runner, current, true, state, controls);
  } else {
    void suggestName(runner, state, controls);
  }
  return runner;
}

function presentBirthControls(runner, state, { presentation, listingId, onDraft, onBirth = () => {} }) {
  const interactive = !state.terminal && !state.pending;
  const fieldActions = interactive ? [
    { id: "program.change", event: "change" },
    { id: "name.input", event: "input" },
    { id: "name-system.change", event: "change" },
    { id: "name.refresh", event: "activate" },
  ] : [];
  const sourceActions = interactive ? [
    { id: "source.input", event: "input" },
    { id: "birth.activate", event: "activate" },
  ] : [];
  const onEvent = (slot) => (event) => {
    presentation.nextEvent(slot);
    const value = decoder.decode(event.value);
    if (event.action === "program.change") state.initialProgram = value;
    if (event.action === "name.input") state.friendlyName = value;
    if (event.action === "name-system.change") { state.namingSystem = value; void suggestName(runner, state, { presentation, listingId, onDraft, onBirth }); }
    if (event.action === "name.refresh") { state.variation += 1; void suggestName(runner, state, { presentation, listingId, onDraft, onBirth }); }
    if (event.action === "source.input") { state.source = value; onDraft(value); }
    if (event.action === "birth.activate") onBirth();
  };
  presentation.present("birth-fields", {
    revision: ++state.revision,
    actions: fieldActions,
    nodes: birthFieldNodes(state),
  }, { onEvent: onEvent("birth-fields") });
  presentation.present("birth-source", {
    revision: ++state.revision,
    actions: sourceActions,
    nodes: birthSourceNodes(state, listingId),
  }, { onEvent: onEvent("birth-source") });
}

function birthFieldNodes(state) {
  const interactive = !state.terminal && !state.pending;
  const nodes = [
    { parent: null, component: "stack", action: null, key: "birth-fields", text: "" },
    { parent: 0, component: "form-field", action: null, key: "program-field", text: "" },
    { parent: 1, component: "field-label", action: null, key: "program-label", text: "Initial program" },
    { parent: 1, component: "select", action: interactive ? 0 : null, key: "body-program", text: "Initial program", value: state.initialProgram, valueCapacity: 64 },
    { parent: 1, component: "field-help", action: null, key: "program-help", text: "Choose the checked Form that the new Body will retain." },
    { parent: 3, component: "option", action: null, key: "morse-program", text: "Morse Network", value: "morse-network@1", valueCapacity: 64 },
    { parent: 0, component: "form-field", action: null, key: "friendly-name-field", text: "" },
    { parent: 6, component: "field-label", action: null, key: "friendly-name-label", text: "Friendly Body name" },
    { parent: 6, component: "text-input", action: interactive ? 1 : null, key: "body-friendly-name", text: "Friendly Body name", value: state.friendlyName, valueCapacity: 64 },
    { parent: 6, component: "field-help", action: null, key: "friendly-name-help", text: "Editable metadata; the durable Body identity remains distinct." },
    { parent: 0, component: "paragraph", action: null, key: "name-origin", text: nameOriginText(state) },
    { parent: 0, component: "form-field", action: null, key: "name-system-field", text: "" },
    { parent: 11, component: "field-label", action: null, key: "name-system-label", text: "Naming tradition" },
    { parent: 11, component: "select", action: interactive ? 2 : null, key: "name-system", text: "Naming tradition", value: state.namingSystem, valueCapacity: 32 },
    { parent: 11, component: "field-help", action: null, key: "name-system-help", text: "Select one bounded naming system for the next suggestion." },
  ];
  for (const option of NAMING_SYSTEM_OPTIONS) {
    nodes.push({ parent: 13, component: "option", action: null, key: `name-${option.id}`, text: option.label, value: option.id, valueCapacity: 32 });
  }
  nodes.push({ parent: 0, component: "button", action: interactive ? 3 : null, key: "another-name", text: "Suggest another name" });
  return nodes;
}

function birthSourceNodes(state, listingId) {
  const interactive = !state.terminal && !state.pending;
  return [
    { parent: null, component: "stack", action: null, key: "birth-source", text: "" },
    { parent: 0, component: "disclosure", action: null, key: "seed-source", text: "" },
    { parent: 1, component: "summary", action: null, key: "seed-summary", text: "Reviewed program source" },
    { parent: 1, component: "form-field", action: null, key: "seed-source-field", text: "" },
    { parent: 3, component: "field-label", action: null, key: "seed-source-label", text: "Conduit Seed source" },
    { parent: 3, component: "textarea", action: interactive ? 0 : null, key: listingId, text: "Conduit Seed source", value: state.source, valueCapacity: 65_536 },
    { parent: 3, component: "field-help", action: null, key: "seed-source-help", text: "This checked meaning contains no Host, Boot, device, or transport facts." },
    { parent: 0, component: "action-group", action: null, key: "birth-actions", text: "" },
    { parent: 7, component: "button", action: interactive ? 1 : null, key: "birth", text: "Birth Body" },
    { parent: 0, component: state.outcome, action: null, key: "birth-status", text: state.status },
  ];
}

async function suggestName(runner, state, controls) {
  const request = ++state.namingRequest;
  state.pending = true;
  state.status = "Deriving a stable persona suggestion from the persona seed UUID…";
  state.outcome = "status";
  presentBirthControls(runner, state, controls);
  try {
    const suggestion = await nameFor(state.personaUuid, state.namingSystem, state.variation);
    if (request !== state.namingRequest || state.terminal) return;
    state.friendlyName = suggestion.name;
    state.namingLabel = suggestion.system_label;
    state.pending = false;
    state.status = "Edit the suggestion or the Seed, then explicitly birth one Body.";
    presentBirthControls(runner, state, controls);
  } catch (error) {
    if (request !== state.namingRequest || state.terminal) return;
    state.pending = false;
    state.outcome = "failure-status";
    state.status = `Persona suggestion refused: ${error instanceof Error ? error.message : String(error)}`;
    presentBirthControls(runner, state, controls);
  }
}

function nameOriginText(state) {
  if (state.terminal) return "This persisted friendly name is metadata; the Body ID remains distinct.";
  return `${state.namingLabel} · persona seed ${state.personaUuid.slice(0, 8)} · variation ${state.variation}. The seed chooses a suggestion; it is not the Body ID.`;
}

function birth(runner, host, state, sequence, onBodyChanged, presentationOptions) {
  const api = host.runtime;
  const sourceBytes = encoder.encode(state.source);
  const hostBytes = encoder.encode(host.hostId);
  const bootBytes = encoder.encode(host.bootId);
  const nameBytes = encoder.encode(state.friendlyName.trim());
  const programBytes = encoder.encode(state.initialProgram);
  const total = hostBytes.length + bootBytes.length + nameBytes.length + programBytes.length + sourceBytes.length;
  if (total > api.conduit_creche_input_capacity()) {
    state.status = "The Seed and exact Host identities exceed the admitted BIRTH input bound.";
    state.outcome = "failure-status";
    presentBirthControls(runner, state, presentationOptions);
    return;
  }
  const input = new Uint8Array(api.memory.buffer, api.conduit_creche_input_ptr(), total);
  input.set(sourceBytes);
  const admitted = api.conduit_creche_admit_source_interaction(sourceBytes.length, BigInt(sequence));
  if (admitted < 0) {
    renderRefusal(runner, api, admitted, state, presentationOptions);
    return;
  }
  input.set(hostBytes);
  input.set(bootBytes, hostBytes.length);
  input.set(nameBytes, hostBytes.length + bootBytes.length);
  input.set(programBytes, hostBytes.length + bootBytes.length + nameBytes.length);
  input.set(sourceBytes, hostBytes.length + bootBytes.length + nameBytes.length + programBytes.length);
  const code = api.conduit_creche_birth(
    hostBytes.length,
    bootBytes.length,
    nameBytes.length,
    programBytes.length,
    sourceBytes.length,
    BigInt(sequence),
  );
  if (code < 0) {
    renderRefusal(runner, api, code, state, presentationOptions);
    return;
  }
  renderReceipt(runner, readOutput(api), false, state, presentationOptions);
  onBodyChanged?.();
}

function readCurrent(api) {
  const code = api.conduit_creche_current();
  if (code === 1) return null;
  if (code < 0) throw new Error(`current Body projection refused (${code})`);
  return readOutput(api);
}

export function readBodyProjection(api) {
  return readCurrent(api);
}

function renderRefusal(runner, api, code, state, presentationOptions) {
  const refusal = api.conduit_creche_output_len() > 0 ? readOutput(api) : null;
  state.status = refusal?.message
    ? `BIRTH refused · ${refusal.category}: ${refusal.message}`
    : `BIRTH refused (${code}).`;
  state.outcome = "failure-status";
  presentBirthControls(runner, state, presentationOptions);
}

function renderReceipt(runner, receipt, retained, state, presentationOptions) {
  runner.dataset.bodyId = receipt.body_id;
  runner.dataset.birthSignId = receipt.birth_sign_id;
  state.terminal = true;
  state.friendlyName = receipt.friendly_name;
  state.initialProgram = receipt.initial_program;
  runner.querySelector(".seed-id").textContent = receipt.seed_id;
  runner.querySelector(".body-id").textContent = receipt.body_id;
  runner.querySelector(".body-state").textContent = receipt.state;
  state.status = retained
    ? "Same LULLED Body retained — Crèche presentation controls did not recreate it."
    : "Born — one checked Seed now has one LULLED Body; no Wake, Plan, or Play exists.";
  state.outcome = "success-status";
  presentBirthControls(runner, state, presentationOptions);
  const identities = [
    ["Friendly name", receipt.friendly_name],
    ["Initial program", receipt.initial_program],
    ["Source document", receipt.source_document_id],
    ["Checked Form", receipt.checked_form_id],
    ["Seed", receipt.seed_id],
    ["BIRTH Sign", receipt.birth_sign_id],
    ["Body", receipt.body_id],
    ["Here Part", receipt.here_part_id ?? "none yet"],
    ["Current Host", receipt.host_id ?? "none yet"],
    ["Current Boot", receipt.boot_id ?? "none yet"],
    ["Membership revision", String(receipt.membership_revision)],
    ["Wake", receipt.wake_id ?? "none"],
    ["Plan", receipt.plan_id ?? "none"],
    ["Active Play", receipt.active_play_id ?? "none"],
  ];
  const rawEvidence = JSON.stringify({
    body: receipt.raw_body,
    membership: receipt.raw_membership,
    source_interaction: receipt.source_interaction,
  }, null, 2);
  presentationOptions.presentation.present("birth-evidence", {
    revision: ++state.revision,
    actions: [],
    nodes: [
      { parent: null, component: "successful-evidence", action: null, key: "body-evidence", text: "Body and membership evidence" },
      { parent: 0, component: "definition-table", action: null, key: "body-identities", text: "Exact Body identities" },
      ...identities.map(([label, value]) => ({ parent: 1, component: "definition", action: null, key: identityKey(label), text: label, value, valueCapacity: 65_536 })),
      { parent: 0, component: "disclosure", action: null, key: "body-raw", text: "Raw Body and membership evidence" },
      { parent: identities.length + 2, component: "code-block", action: null, key: "body-raw-json", text: "json", value: rawEvidence, valueCapacity: 65_536 },
    ],
  });
}

export function createFirstHostRunner({ host, presentationFor, nextSequence, onBodyChanged }) {
  const runner = document.createElement("section");
  runner.className = "runner first-host-runner";
  runner.innerHTML = `
    <div data-application-slot="first-host-controls"></div>
    <div class="result" data-application-slot="first-host-evidence"></div>`;
  const presentation = presentationFor(runner);
  const state = {
    revision: 0,
    status: "The Body is still LULLED with no admitted Host.",
    outcome: "status",
    terminal: false,
  };
  const current = readCurrent(host.runtime);
  if (!current) {
    state.terminal = true;
    state.status = "Birth the Body on page zero first.";
    presentFirstHostControls(runner, presentation, state, () => {});
    return runner;
  }
  if (current.here_part_id) {
    renderAttachedHost(runner, presentation, current, state);
    return runner;
  }
  presentFirstHostControls(runner, presentation, state, () => {
    const api = host.runtime;
    const hostBytes = encoder.encode(host.hostId);
    const bootBytes = encoder.encode(host.bootId);
    const input = new Uint8Array(api.memory.buffer, api.conduit_creche_input_ptr(), hostBytes.length + bootBytes.length);
    input.set(hostBytes);
    input.set(bootBytes, hostBytes.length);
    const code = api.conduit_creche_attach_here(hostBytes.length, bootBytes.length, BigInt(nextSequence()));
    if (code < 0) {
      const refusal = readOutput(api);
      state.status = `Host admission refused: ${refusal.message ?? code}`;
      state.outcome = "failure-status";
      presentFirstHostControls(runner, presentation, state, () => {});
      return;
    }
    renderAttachedHost(runner, presentation, readOutput(api), state);
    onBodyChanged?.();
  });
  return runner;
}

function presentFirstHostControls(runner, presentation, state, onAttach) {
  presentation.present("first-host-controls", {
    revision: ++state.revision,
    actions: state.terminal ? [] : [{ id: "host.attach", event: "activate" }],
    nodes: [
      { parent: null, component: "stack", action: null, key: "first-host", text: "" },
      { parent: 0, component: "paragraph", action: null, key: "availability", text: "This browser is available, but availability is not membership." },
      { parent: 0, component: "action-group", action: null, key: "host-actions", text: "" },
      { parent: 2, component: "button", action: state.terminal ? null : 0, key: "attach-host", text: "Give this Body its first Host" },
      { parent: 0, component: state.outcome, action: null, key: "host-status", text: state.status },
    ],
  }, { onEvent() {
    presentation.nextEvent("first-host-controls");
    onAttach();
  } });
}

function renderAttachedHost(runner, presentation, receipt, state) {
  state.terminal = true;
  state.outcome = "success-status";
  state.status = `${receipt.friendly_name} now has one admitted browser Host and remains LULLED.`;
  presentFirstHostControls(runner, presentation, state, () => {});
  const values = [["Body", receipt.body_id], ["Part", receipt.here_part_id], ["Host", receipt.host_id], ["Boot", receipt.boot_id]];
  presentation.present("first-host-evidence", {
    revision: ++state.revision,
    actions: [],
    nodes: [
      { parent: null, component: "successful-evidence", action: null, key: "host-evidence", text: "Current browser Host membership" },
      { parent: 0, component: "definition-table", action: null, key: "host-identities", text: "Exact Host identities" },
      ...values.map(([label, value]) => ({ parent: 1, component: "definition", action: null, key: identityKey(label), text: label, value, valueCapacity: 256 })),
    ],
  });
}

function identityKey(label) {
  return label.toLowerCase().replaceAll(/[^a-z0-9]+/g, "-").replaceAll(/^-|-$/g, "").slice(0, 32);
}

function readOutput(api) {
  const bytes = new Uint8Array(
    api.memory.buffer,
    api.conduit_creche_output_ptr(),
    api.conduit_creche_output_len(),
  );
  return JSON.parse(decoder.decode(bytes));
}
