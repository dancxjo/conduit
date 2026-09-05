import { nameFor, NAMING_SYSTEM_OPTIONS } from "./creche-names.mjs";
import { attachConduitSyntaxEditor } from "../../../targets/browser/host/assets/application-syntax-presentation.mjs";
import {
  encodedFormSelection,
  reviewInitialWorkload,
  searchForms,
  setFormSelected,
} from "./creche-form-selection.mjs";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function createBodyBirthRunner({ source, sourceKey, listingId, host, presentationFor, inventory, initialSelection, onSelection, nextSequence, onBodyChanged }) {
  const runner = document.createElement("section");
  runner.className = "runner body-birth-runner";
  runner.dataset.sourceKey = sourceKey;
  runner.innerHTML = `
    <div class="birth-presentation">
      <div data-application-slot="birth-selection"></div>
      <div data-application-slot="birth-fields"></div>
      <div data-application-slot="birth-source"></div>
    </div>
    <div class="result body-birth-result">
      <div class="body-chain" aria-label="Forms to Body lifecycle">
        <article><span>initial active Forms</span><code class="initial-forms">not born</code></article>
        <b aria-hidden="true">BIRTH →</b>
        <article><span>durable Body</span><strong class="body-state">not born</strong><code class="body-id"></code></article>
      </div>
      <div class="body-evidence" data-application-slot="birth-evidence"></div>
    </div>`;
  const presentation = presentationFor(runner);
  const selectionNotice = initialFormSelectionNotice(initialSelection);
  const state = {
    revision: 0,
    friendlyName: "Choosing a persona…",
    namingSystem: "surprise",
    namingLabel: "Surprise me",
    personaUuid: crypto.randomUUID(),
    variation: 0,
    namingRequest: 0,
    pending: true,
    initialForms: [...initialSelection.selected],
    search: "",
    inventorySource: source,
    review: null,
    status: selectionNotice ?? "Browse the reviewed Forms, compose a bounded workload, then review it before birth.",
    outcome: initialSelection.refusals.length === 0 ? "status" : "warning-status",
    terminal: false,
    selectionNotice,
  };
  const current = readCurrent(host.runtime);
  const controls = { presentation, runtime: host.runtime, listingId, inventory, onSelection: onSelection ?? (() => {}), onReview() {
    review(runner, host, state, controls);
  }, onBirth() {
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

export function initialFormSelectionNotice(initialSelection) {
  if (initialSelection.refusals.some((refusal) => refusal.origin === "gallery-handoff")) {
    return "The Gallery Form handoff was stale or substituted and was not selected.";
  }
  const notices = [];
  if (initialSelection.refusals.length > 0) {
    notices.push(`${initialSelection.refusals.length} stale or over-capacity restored Form selection(s) were refused.`);
  }
  if (initialSelection.acceptedHandoff) {
    notices.push(`${initialSelection.acceptedHandoff.title} was revalidated and preselected from Gallery. Add more ordinary Forms or review this workload; no Body has been born.`);
  }
  return notices.length === 0 ? null : notices.join(" ");
}

function presentBirthControls(runner, state, controls) {
  const { presentation, runtime, listingId, inventory, onSelection, onReview = () => {}, onBirth = () => {} } = controls;
  const interactive = !state.terminal && !state.pending;
  const visible = searchForms(inventory, state.search);
  const selectionActions = interactive ? [
    { id: "forms.search", event: "input" },
    ...visible.map((form) => ({ id: `form.toggle.${form.name}`, event: "change" })),
  ] : [];
  const fieldActions = interactive ? [
    { id: "name.input", event: "input" },
    { id: "name-system.change", event: "change" },
    { id: "name.refresh", event: "activate" },
  ] : [];
  const sourceActions = interactive ? [
    { id: "workload.review", event: "activate" },
    { id: "birth.activate", event: "activate" },
  ] : [];
  const onEvent = (slot) => (event) => {
    presentation.nextEvent(slot);
    const value = decoder.decode(event.value);
    if (event.action === "forms.search") {
      state.search = value;
      presentBirthControls(runner, state, controls);
      return;
    }
    if (event.action.startsWith("form.toggle.")) {
      state.initialForms = setFormSelected(
        inventory,
        state.initialForms,
        event.action.slice("form.toggle.".length),
        value === "true",
      );
      state.review = null;
      state.status = "Selection changed; review the combined workload before birth.";
      state.outcome = "status";
      onSelection(state.initialForms);
      presentBirthControls(runner, state, controls);
      return;
    }
    if (event.action === "name.input") state.friendlyName = value;
    if (event.action === "name-system.change") { state.namingSystem = value; void suggestName(runner, state, controls); }
    if (event.action === "name.refresh") { state.variation += 1; void suggestName(runner, state, controls); }
    if (event.action === "workload.review") onReview();
    if (event.action === "birth.activate") onBirth();
  };
  presentation.present("birth-selection", {
    revision: ++state.revision,
    actions: selectionActions,
    nodes: birthSelectionNodes(state, inventory, visible, selectionActions),
  }, { onEvent: onEvent("birth-selection") });
  presentation.present("birth-fields", {
    revision: ++state.revision,
    actions: fieldActions,
    nodes: birthFieldNodes(state, inventory, visible, fieldActions),
  }, { onEvent: onEvent("birth-fields") });
  presentation.present("birth-source", {
    revision: ++state.revision,
    actions: sourceActions,
    nodes: birthSourceNodes(state, listingId),
  }, { onEvent: onEvent("birth-source") });
  attachConduitSyntaxEditor(runner.querySelector(`[data-application-key="${listingId}"]`), runtime);
}

function birthSelectionNodes(state, inventory, visible, actions) {
  const interactive = !state.terminal && !state.pending;
  const action = (id) => interactive ? actions.findIndex((candidate) => candidate.id === id) : null;
  const nodes = [
    { parent: null, component: "stack", action: null, key: "initial-forms-selection", text: "" },
    { parent: 0, component: "form-field", action: null, key: "form-search-field", text: "" },
    { parent: 1, component: "field-label", action: null, key: "form-search-label", text: "Search Forms" },
    { parent: 1, component: "text-input", action: action("forms.search"), key: "form-search", text: "Search Forms", value: state.search, valueCapacity: 128 },
    { parent: 1, component: "field-help", action: null, key: "form-search-help", text: "Filter the finite reviewed inventory by name or required kind." },
    { parent: 0, component: "paragraph", action: null, key: "selected-heading", text: `Selected (${state.initialForms.length})` },
    { parent: 0, component: "choice-group", action: null, key: "initial-forms-field", text: "active_forms" },
    { parent: 6, component: "choice-group-label", action: null, key: "initial-forms-label", text: "Initial active Forms" },
  ];
  for (const form of visible) {
    const selected = state.initialForms.some((candidate) => candidate.checked_form_id === form.checked_form_id);
    const label = nodes.length;
    nodes.push({ parent: 6, component: "choice-option-label", action: null, key: `form-${form.name}-label`, text: form.title });
    nodes.push({ parent: label, component: "independent-choice", action: action(`form.toggle.${form.name}`), key: `form-${form.name}`, text: form.name, value: String(selected), valueCapacity: 5 });
    nodes.push({ parent: 6, component: "paragraph", action: null, key: `form-${form.name}-requirements`, text: form.required_kinds.join(" · ") });
  }
  nodes.push({ parent: 6, component: "paragraph", action: null, key: "initial-forms-help", text: `${state.initialForms.length} of ${inventory.forms.length} reviewed Forms selected; maximum ${inventory.maximum_selection}.` });
  return nodes;
}

function birthFieldNodes(state, _inventory, _visible, actions) {
  const interactive = !state.terminal && !state.pending;
  const action = (id) => interactive ? actions.findIndex((candidate) => candidate.id === id) : null;
  const nodes = [
    { parent: null, component: "stack", action: null, key: "birth-fields", text: "" },
  ];
  const nameField = nodes.length;
  nodes.push(
    { parent: 0, component: "form-field", action: null, key: "friendly-name-field", text: "" },
    { parent: nameField, component: "field-label", action: null, key: "friendly-name-label", text: "Friendly Body name" },
    { parent: nameField, component: "text-input", action: action("name.input"), key: "body-friendly-name", text: "Friendly Body name", value: state.friendlyName, valueCapacity: 64 },
    { parent: nameField, component: "field-help", action: null, key: "friendly-name-help", text: "Editable metadata; the durable Body identity remains distinct." },
    { parent: 0, component: "paragraph", action: null, key: "name-origin", text: nameOriginText(state) },
  );
  const systemField = nodes.length;
  nodes.push(
    { parent: 0, component: "form-field", action: null, key: "name-system-field", text: "" },
    { parent: systemField, component: "field-label", action: null, key: "name-system-label", text: "Naming tradition" },
    { parent: systemField, component: "select", action: action("name-system.change"), key: "name-system", text: "Naming tradition", value: state.namingSystem, valueCapacity: 32 },
    { parent: systemField, component: "field-help", action: null, key: "name-system-help", text: "Select one bounded naming system for the next suggestion." },
  );
  const selectIndex = systemField + 2;
  for (const option of NAMING_SYSTEM_OPTIONS) {
    nodes.push({ parent: selectIndex, component: "option", action: null, key: `name-${option.id}`, text: option.label, value: option.id, valueCapacity: 32 });
  }
  nodes.push({ parent: 0, component: "button", action: action("name.refresh"), key: "another-name", text: "Suggest another name" });
  return nodes;
}

function birthSourceNodes(state, listingId) {
  const interactive = !state.terminal && !state.pending;
  const source = selectedCanonicalSource(state.initialForms);
  return [
    { parent: null, component: "stack", action: null, key: "birth-source", text: "" },
    { parent: 0, component: "disclosure", action: null, key: "form-source", text: "" },
    { parent: 1, component: "summary", action: null, key: "form-summary", text: "Selected canonical Form source" },
    { parent: 1, component: "form-field", action: null, key: "form-source-field", text: "" },
    { parent: 3, component: "field-label", action: null, key: "form-source-label", text: "Selected Conduit Form source" },
    { parent: 3, component: "textarea", action: null, key: listingId, text: "Selected Conduit Form source", value: source, valueCapacity: 65_536 },
    { parent: 3, component: "field-help", action: null, key: "form-source-help", text: "Read-only exact canonical source for the selected Forms. The internal reviewed package envelope is not authored meaning." },
    { parent: 0, component: "definition-table", action: null, key: "combined-requirements", text: "Combined requirements" },
    { parent: 7, component: "definition", action: null, key: "required-kinds", text: "Checked kinds", value: combinedKinds(state), valueCapacity: 4096 },
    { parent: 7, component: "definition", action: null, key: "review-basis", text: "Realization basis", value: state.review ? `${state.review.proposed_hosts.length} current Host OFFER(s); no permission or resource acquired; no Body Plan or Play created` : "not reviewed", valueCapacity: 1024 },
    { parent: 0, component: "action-group", action: null, key: "birth-actions", text: "Birth actions" },
    { parent: 10, component: "button", action: interactive ? 0 : null, key: "review", text: "Review workload" },
    { parent: 10, component: "button", action: interactive && state.review ? 1 : null, key: "birth", text: "Birth Body" },
    { parent: 0, component: state.outcome, action: null, key: "birth-status", text: state.status },
  ];
}

export function selectedCanonicalSource(forms) {
  return forms.map((form) => form.source.trimEnd()).join("\n\n");
}

function combinedKinds(state) {
  const kinds = [...new Set(state.initialForms.flatMap((form) => form.required_kinds))].sort();
  return kinds.join(", ") || "none (idle Body)";
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
    state.status = state.selectionNotice
      ?? "Edit the suggestion or Form selection, then review the combined workload.";
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

function review(runner, host, state, controls) {
  try {
    state.review = reviewInitialWorkload(
      host.runtime,
      host.hostId,
      host.bootId,
      state.inventorySource,
      state.initialForms,
    );
    state.status = `Review accepted ${state.initialForms.length} Form(s) against ${state.review.proposed_hosts.length} current Host OFFER(s). No permission or resource was acquired; no Body Plan or Play exists.`;
    state.outcome = "success-status";
  } catch (error) {
    state.review = null;
    state.status = `Workload review refused: ${error instanceof Error ? error.message : String(error)}`;
    state.outcome = "failure-status";
  }
  presentBirthControls(runner, state, controls);
}

function birth(runner, host, state, sequence, onBodyChanged, presentationOptions) {
  const api = host.runtime;
  const sourceBytes = encoder.encode(state.inventorySource);
  const hostBytes = encoder.encode(host.hostId);
  const bootBytes = encoder.encode(host.bootId);
  const nameBytes = encoder.encode(state.friendlyName.trim());
  const formsBytes = encoder.encode(encodedFormSelection(state.initialForms));
  const total = hostBytes.length + bootBytes.length + nameBytes.length + formsBytes.length + sourceBytes.length;
  if (total > api.conduit_creche_input_capacity()) {
    state.status = "The Form selection and exact Host identities exceed the admitted BIRTH input bound.";
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
  input.set(formsBytes, hostBytes.length + bootBytes.length + nameBytes.length);
  input.set(sourceBytes, hostBytes.length + bootBytes.length + nameBytes.length + formsBytes.length);
  const code = api.conduit_creche_birth(
    hostBytes.length,
    bootBytes.length,
    nameBytes.length,
    formsBytes.length,
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
  state.initialForms = receipt.initial_forms.map((form) => {
    const current = presentationOptions.inventory.forms.find((candidate) => candidate.name === form.name
      && candidate.source_document_id === form.source_document_id
      && candidate.checked_form_id === form.checked_form_id);
    if (!current) throw new Error(`Body receipt carries stale initial Form identity ${JSON.stringify(form.name)}`);
    return current;
  });
  runner.querySelector(".initial-forms").textContent = `${receipt.initial_forms.length} active`;
  runner.querySelector(".body-id").textContent = receipt.body_id;
  runner.querySelector(".body-state").textContent = receipt.state;
  state.status = retained
    ? "Same LULLED Body retained — Crèche presentation controls did not recreate it."
    : `Born — ${receipt.initial_forms.length} checked Form(s) now have one LULLED Body; no Wake, Plan, or Play exists.`;
  state.outcome = "success-status";
  if (!retained) presentationOptions.onSelection(null);
  presentBirthControls(runner, state, presentationOptions);
  const identities = [
    ["Friendly name", receipt.friendly_name],
    ["Initial Forms", receipt.initial_forms.map((form) => form.name).join(", ") || "none"],
    ["BIRTH Sign", receipt.birth_sign_id],
    ["Body", receipt.body_id],
    ["Here Part", receipt.here_part_id ?? "none yet"],
    ["Current Host", receipt.host_id ?? "none yet"],
    ["Current Boot", receipt.boot_id ?? "none yet"],
    ["Membership revision", String(receipt.membership_revision)],
    ["Workload revision", String(receipt.workload_revision)],
    ["Wake", receipt.wake_id ?? "none"],
    ["Plan", receipt.plan_id ?? "none"],
    ["Active Play", receipt.active_play_id ?? "none"],
  ];
  const rawEvidence = JSON.stringify({
    body: receipt.raw_body,
    membership: receipt.raw_membership,
    initial_workload_review: receipt.initial_review,
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
      { parent: 0, component: "action-group", action: null, key: "host-actions", text: "First Host actions" },
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
