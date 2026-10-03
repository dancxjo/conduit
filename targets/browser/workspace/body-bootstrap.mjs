import { attachConduitSyntaxEditor } from "../../../targets/browser/host/assets/application-syntax-presentation.mjs";
import {
  encodedPlotSelection,
  reviewInitialWorkload,
  selectedReviewedSource,
} from "./reviewed-plot-selection.mjs";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function createBodyBirthRunner({ source, sourceKey, listingId, host, presentationFor, inventory, initialSelection, onSelection, nextSequence, onBodyChanged, onContinue }) {
  const runner = document.createElement("section");
  runner.className = "runner body-birth-runner";
  runner.dataset.sourceKey = sourceKey;
  runner.innerHTML = `
    <div class="birth-presentation">
      <div data-application-slot="birth-fields"></div><div data-application-slot="birth-feedback"></div>
      <details class="birth-details"><summary>Details and source</summary><div data-application-slot="birth-source"></div></details>
    </div>
    <div class="result body-birth-result" hidden>
      <div data-application-slot="birth-next"></div>
      <details class="birth-details"><summary>Body details and evidence</summary>
      <div class="body-chain" aria-label="Plots to Body lifecycle">
        <article><span>initial active plots</span><code class="initial-plots">not born</code></article>
        <b aria-hidden="true">BIRTH →</b>
        <article><span>durable body</span><strong class="body-state">not born</strong><code class="body-id"></code></article>
      </div>
      <div class="body-evidence" data-application-slot="birth-evidence"></div>
      </details>
    </div>`;
  const presentation = presentationFor(runner);
  const selectionNotice = initialPlotSelectionNotice(initialSelection);
  const state = {
    revision: 0,
    friendlyName: "Choosing a persona…",
    namingSystem: "surprise",
    personaUuid: crypto.randomUUID(),
    pending: true,
    initialPlots: [...initialSelection.selected],
    inventorySource: source,
    review: null,
    status: selectionNotice ?? "Browse the reviewed plots, compose a bounded workload, then review it before birth.",
    outcome: initialSelection.refusals.length === 0 ? "status" : "warning-status",
    terminal: false,
    selectionNotice,
  };
  const current = readCurrent(host.runtime);
  const controls = { presentation, runtime: host.runtime, listingId, inventory, onContinue, onSelection: onSelection ?? (() => {}), onReview() {
    review(runner, host, state, controls);
  }, onBirth() {
    review(runner, host, state, controls);
    if (state.review) birth(runner, host, state, nextSequence(), onBodyChanged, controls);
  } };
  if (current) {
    state.pending = false;
    renderReceipt(runner, current, true, state, controls);
  } else {
    try {
      applyDraftSnapshot(state, controls, draftCall(host.runtime, "conduit_creche_birth_draft_open", {
        persona_uuid: state.personaUuid,
        choices: inventory.plots.map((plot) => ({
          title: plot.title,
          search_text: `${plot.name} ${plot.required_kinds.join(" ")}`,
          plot: { source_document_id: plot.source_document_id, checked_plot_id: plot.checked_plot_id },
          selected: state.initialPlots.some((selected) => selected.checked_plot_id === plot.checked_plot_id),
        })),
      }));
      state.pending = false;
      state.status = selectionNotice ?? "Give it a name. Choose what it wakes with.";
      presentBirthControls(runner, state, controls);
    } catch (error) {
      state.pending = false;
      state.status = error instanceof Error ? error.message : String(error);
      state.outcome = "failure-status";
      presentBirthControls(runner, state, controls);
    }
  }
  return runner;
}

export function initialPlotSelectionNotice(initialSelection) {
  if (initialSelection.refusals.some((refusal) => refusal.origin === "gallery-handoff")) {
    return "The Gallery Plot handoff was stale or substituted and was not selected.";
  }
  const notices = [];
  if (initialSelection.refusals.length > 0) {
    notices.push(`${initialSelection.refusals.length} stale or over-capacity restored Plot selection(s) were refused.`);
  }
  if (initialSelection.acceptedHandoff) {
    notices.push(`${initialSelection.acceptedHandoff.title} was revalidated and preselected from Gallery. Add more ordinary plots or review this workload; no Body has been born.`);
  }
  return notices.length === 0 ? null : notices.join(" ");
}

function draftCall(runtime, entrance, request) {
  const bytes = encoder.encode(JSON.stringify(request));
  if (bytes.length > runtime.conduit_creche_input_capacity()) throw new Error("Crèche draft request exceeds its bound");
  const pointer = runtime.conduit_creche_input_ptr();
  new Uint8Array(runtime.memory.buffer, pointer, bytes.length).set(bytes);
  const code = runtime[entrance](bytes.length);
  const response = readOutput(runtime);
  if (code < 0) throw new Error(response.message ?? `Crèche draft refused (${code})`);
  return response;
}

function applyDraftSnapshot(state, controls, snapshot) {
  state.draft = snapshot;
  state.friendlyName = snapshot.friendly_name;
  state.namingSystem = snapshot.naming_system;
  state.initialPlots = snapshot.selected.map((identity) => {
    const plot = controls.inventory.plots.find((candidate) => candidate.source_document_id === identity.source_document_id
      && candidate.checked_plot_id === identity.checked_plot_id);
    if (!plot) throw new Error("Crèche draft returned an unknown Plot identity");
    return plot;
  });
}

function presentBirthControls(runner, state, controls) {
  const { presentation, runtime, listingId, onSelection, onReview, onBirth } = controls;
  runner.querySelector(".birth-presentation").hidden = state.terminal;
  if (state.terminal) return;
  if (state.draft && state.renderedDraftRevision !== state.draft.revision) {
    const code = runtime.conduit_creche_birth_draft_view(state.draft.generation);
    if (code < 0) throw new Error(readOutput(runtime).message ?? `Crèche view refused (${code})`);
    const view = new Uint8Array(runtime.memory.buffer, runtime.conduit_creche_output_ptr(), runtime.conduit_creche_output_len()).slice();
    presentation.present("birth-fields", view, {
      onEvent(event) {
        presentation.nextEvent("birth-fields");
        try {
          const snapshot = draftCall(runtime, "conduit_creche_birth_draft_event", {
            generation: state.draft.generation, revision: event.revision,
            action: event.action, event: ({ 1: "activate", 2: "change", 3: "input" })[event.kind],
            value: decoder.decode(event.value),
          });
          applyDraftSnapshot(state, controls, snapshot);
          if (event.action.startsWith("creche.plot.")) {
            state.review = null;
            onSelection(state.initialPlots);
          }
          if (snapshot.birth_requested) { onBirth(); return; }
          state.status = "Give it a name. Choose what it wakes with.";
          state.outcome = "status";
          presentBirthControls(runner, state, controls);
        } catch (error) {
          state.status = error instanceof Error ? error.message : String(error);
          state.outcome = "failure-status";
          presentBirthControls(runner, state, controls);
        }
      },
    });
    state.renderedDraftRevision = state.draft.revision;
  }
  presentation.present("birth-feedback", {
    revision: ++state.revision, actions: [],
    nodes: birthFeedbackNodes(state.status, state.outcome),
  });
  presentation.present("birth-source", {
    revision: ++state.revision,
    actions: [{ id: "workload.review", event: "activate" }],
    nodes: [
      { parent: null, component: "stack", action: null, key: "birth-source", text: "" },
      { parent: 0, component: "form-field", action: null, key: "plot-source-field", text: "" },
      { parent: 1, component: "field-label", action: null, key: "plot-source-label", text: "Selected Conduit Plot source" },
      { parent: 1, component: "textarea", action: null, key: listingId, text: "Selected Conduit Plot source", value: selectedCanonicalSource(state.initialPlots), valueCapacity: 65_536 },
      { parent: 1, component: "field-help", action: null, key: "plot-source-help", text: "The exact source of the selected plots." },
      { parent: 0, component: "definition-table", action: null, key: "combined-requirements", text: "Combined requirements" },
      { parent: 5, component: "definition", action: null, key: "required-kinds", text: "Checked kinds", value: combinedKinds(state), valueCapacity: 4096 },
      { parent: 5, component: "definition", action: null, key: "review-basis", text: "Review basis", value: state.review
        ? "Reviewed against current host OFFER(s); no permission or resource acquired; no Body Plan or Play created."
        : "Selection not reviewed; Birth will review it before creating the body.", valueCapacity: 4096 },
      { parent: 0, component: "button", action: 0, key: "review", text: "Review workload" },
    ],
  }, { onEvent() { presentation.nextEvent("birth-source"); onReview(); } });
  attachConduitSyntaxEditor(runner.querySelector(`[data-application-key="${listingId}"]`), runtime);
}

// Status labels have a 256-byte bound; detailed evidence uses the existing value channel.
export function birthFeedbackNodes(message, outcome) {
  const bytes = encoder.encode(message);
  if (bytes.length <= 256) return [{ parent: null, component: outcome, action: null, key: "birth-status", text: message }];
  const suffix = "\n[Details exceed the display bound.]";
  let detail = message;
  if (bytes.length > 65_536) {
    let end = 65_536 - encoder.encode(suffix).length;
    while ((bytes[end] & 0xc0) === 0x80) end--;
    detail = decoder.decode(bytes.subarray(0, end)) + suffix;
  }
  return [
    { parent: null, component: "stack", action: null, key: "birth-feedback", text: "" },
    { parent: 0, component: outcome, action: null, key: "birth-status", text: "The workload could not be accepted. Details are shown below." },
    { parent: 0, component: "definition-table", action: null, key: "birth-details", text: "Workload details" },
    { parent: 2, component: "definition", action: null, key: "birth-detail", text: "Reason", value: detail, valueCapacity: 65_536 },
  ];
}

export function selectedCanonicalSource(plots) {
  return plots.map((plot) => plot.source.trimEnd()).join("\n\n");
}

function combinedKinds(state) {
  const kinds = [...new Set(state.initialPlots.flatMap((plot) => plot.required_kinds))].sort();
  return kinds.join(", ") || "none (idle Body)";
}

function review(runner, host, state, controls) {
  try {
    state.review = reviewInitialWorkload(
      host.runtime,
      host.hostId,
      host.bootId,
      selectedReviewedSource(state.inventorySource, state.initialPlots),
      state.initialPlots,
    );
    const plotCount = state.initialPlots.length;
    state.status = `Ready to birth with ${plotCount} ${plotCount === 1 ? 'plot' : 'plots'}.`;
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
  const sourceBytes = encoder.encode(selectedReviewedSource(state.inventorySource, state.initialPlots));
  const hostBytes = encoder.encode(host.hostId);
  const bootBytes = encoder.encode(host.bootId);
  const nameBytes = encoder.encode(state.friendlyName.trim());
  const formsBytes = encoder.encode(encodedPlotSelection(state.initialPlots));
  const total = hostBytes.length + bootBytes.length + nameBytes.length + formsBytes.length + sourceBytes.length;
  if (total > api.conduit_creche_input_capacity()) {
    state.status = "The plot selection and exact host identities exceed the admitted BIRTH input bound.";
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
  if (code < 0) throw new Error(`current body projection refused (${code})`);
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
  runner.querySelector(".body-birth-result").hidden = false;
  runner.dataset.bodyId = receipt.body_id;
  runner.dataset.birthSignId = receipt.birth_sign_id;
  state.terminal = true;
  state.friendlyName = receipt.friendly_name;
  state.initialPlots = receipt.initial_plots.map((plot) => {
    const current = presentationOptions.inventory.plots.find((candidate) => candidate.name === plot.name
      && candidate.source_document_id === plot.source_document_id
      && candidate.checked_plot_id === plot.checked_plot_id);
    if (!current) throw new Error(`Body receipt carries stale initial Plot identity ${JSON.stringify(plot.name)}`);
    return current;
  });
  runner.querySelector(".initial-plots").textContent = `${receipt.initial_plots.length} active`;
  runner.querySelector(".body-id").textContent = receipt.body_id;
  runner.querySelector(".body-state").textContent = receipt.state;
  state.status = retained
    ? "Same LULLED Body retained — Crèche presentation controls did not recreate it."
    : `Born — ${receipt.initial_plots.length} checked plot(s) now have one LULLED Body; no Wake, Plan, or Play exists.`;
  state.outcome = "success-status";
  if (!retained) presentationOptions.onSelection(null);
  presentBirthControls(runner, state, presentationOptions);
  presentationOptions.presentation.present("birth-next", {
    revision: ++state.revision,
    actions: [{ id: "creche.continue", event: "activate" }],
    nodes: [
      { parent: null, component: "stack", action: null, key: "born", text: "" },
      { parent: 0, component: "heading", action: null, key: "born-heading", text: `${receipt.friendly_name} is born` },
      { parent: 0, component: "paragraph", action: null, key: "born-plots", text: state.initialPlots.length
        ? `${state.initialPlots.length} Plot${state.initialPlots.length === 1 ? "" : "s"} included.`
        : "Your body is ready for Plots whenever you are." },
      ...state.initialPlots.map((plot, index) => ({ parent: 0, component: "paragraph", action: null, key: `born-plot-${index}`, text: plot.title })),
      { parent: 0, component: "paragraph", action: null, key: "born-state", text: receipt.here_part_id
        ? "This body has a host. Continue to its host options." : "Give it a host to continue." },
      { parent: 0, component: "button", action: 0, key: "born-continue", text: "Continue on this host" },
    ],
  }, { onEvent() {
    presentationOptions.presentation.nextEvent("birth-next");
    presentationOptions.onContinue();
  } });
  if (!retained) {
    const heading = runner.querySelector('[data-application-key="born-heading"]');
    heading.tabIndex = -1;
    heading.focus();
  }
  const identities = [
    ["Friendly name", receipt.friendly_name],
    ["Initial Plots", receipt.initial_plots.map((plot) => plot.name).join(", ") || "none"],
    ["BIRTH Sign", receipt.birth_sign_id],
    ["Body", receipt.body_id],
    ["Here Part", receipt.here_part_id ?? "none yet"],
    ["Current host", receipt.host_id ?? "none yet"],
    ["Current Boot", receipt.boot_id ?? "none yet"],
    ["Membership revision", String(receipt.membership_revision)],
    ["Workload revision", String(receipt.workload_revision)],
    ["Wake", receipt.wake_id ?? "none"],
    ["Plan", receipt.plan_id ?? "none"],
    ["Active play", receipt.active_play_id ?? "none"],
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
      { parent: 0, component: "definition-table", action: null, key: "body-identities", text: "Exact body identities" },
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
    status: "The body is still LULLED with no admitted host.",
    outcome: "status",
    terminal: false,
  };
  const current = readCurrent(host.runtime);
  if (!current) {
    state.terminal = true;
    state.status = "Birth the body on page zero first.";
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
      { parent: 0, component: "action-group", action: null, key: "host-actions", text: "First host actions" },
      { parent: 2, component: "button", action: state.terminal ? null : 0, key: "attach-host", text: "Give this body its first host" },
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
      { parent: 0, component: "definition-table", action: null, key: "host-identities", text: "Exact host identities" },
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
