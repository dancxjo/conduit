import { PHYSICAL_HOST_INTENTIONS } from "./creche-target-catalog.mjs";
import { presentPhysicalActions, presentPhysicalArtifact, presentPhysicalEvidence, presentPhysicalProgress, presentPhysicalSelection, presentPhysicalStatus } from "./creche-physical-presentation.mjs";
const WORKFLOW_SCHEMA = "conduit.creche/physical-host-workflow-evidence@1";
const FAILURE_SCHEMA = "conduit.creche/physical-host-workflow-failure@1";
const SELECTION_FAILURE_SCHEMA = "conduit.creche/physical-host-target-selection-failure@1";
const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function createPhysicalHostRunner({ host, hostOperations, presentationFor, targetCatalog, onBodyChanged }) {
  const catalog = requireTargetCatalog(targetCatalog);
  const runner = document.createElement("section");
  runner.className = "physical-host-runner";
  runner.innerHTML = `
    <div data-application-slot="physical-progress"></div>
    <div class="physical-selection">
      <div class="select-field" data-application-slot="physical-mode-control"></div>
      <div class="select-field" data-application-slot="physical-target-control"></div>
      <div class="target-options"></div>
      <div class="spore-download" data-application-slot="physical-artifact"></div>
    </div>
    <div class="physical-actions" data-application-slot="physical-actions"></div>
    <div class="physical-status" data-application-slot="physical-status"></div>
    <div class="physical-evidence" data-application-slot="physical-evidence"></div>`;

  const presentation = presentationFor(runner);
  const state = {
    presentation,
    presentationFor,
    presentationRevision: 0,
    catalog,
    entry: null,
    adapter: null,
    mode: PHYSICAL_HOST_INTENTIONS[0].id,
    generation: 0,
    admittedOperations: 0,
    cancellations: 0,
    active: null,
    phase: "choosing",
    terminal: null,
    obtainment: null,
    binding: null,
    realization: null,
    observation: null,
    admission: null,
    download: null,
    hostOperations,
    intentions: PHYSICAL_HOST_INTENTIONS,
    selectionDisabled: false,
    actionEnabled: null,
    cancelEnabled: false,
    stages: { obtain: "waiting", bind: "waiting", realize: "waiting", observe: "waiting", admit: "waiting" },
  };

  state.changeMode = (mode) => selectMode(runner, host, state, mode);
  state.changeTarget = (target) => selectTarget(runner, host, state, target);
  state.runAction = (action) => ({
    bind: () => bindInvitation(runner, host, state),
    realize: () => realizeHost(runner, host, state),
    observe: () => observeJoin(runner, host, state),
    admit: () => admitPart(runner, host, state, onBodyChanged),
    cancel: () => cancelActive(runner, state, true),
  })[action]?.();
  presentPhysicalActions(state);
  presentPhysicalProgress(state);
  selectTarget(runner, host, state, catalog.entries[0].target.id);
  return runner;
}

function requireTargetCatalog(catalog) {
  if (!catalog || catalog.schema !== "conduit.creche/physical-host-target-catalog@1"
    || !Number.isSafeInteger(catalog.generation) || catalog.generation <= 0
    || !Array.isArray(catalog.entries) || catalog.entries.length === 0
    || !Array.isArray(catalog.families) || catalog.families.length === 0
    || typeof catalog.createAdapter !== "function") {
    throw new TypeError("physical Host target catalog contract is incomplete");
  }
  return catalog;
}

function selectTarget(runner, host, state, targetId) {
  cancelActive(runner, state, false);
  const entry = state.catalog.entries.find((candidate) => candidate.target.id === targetId);
  if (!entry) throw new TypeError("physical Host target is absent from the current catalog generation");
  state.entry = entry;
  state.adapter = null;
  const currentSupport = entry.intentions.find((mode) => mode.id === state.mode);
  if (!currentSupport?.supported) {
    state.mode = entry.intentions.find((mode) => mode.supported)?.id ?? state.mode;
  }
  selectMode(runner, host, state, state.mode);
}

function selectMode(runner, host, state, mode) {
  cancelActive(runner, state, false);
  state.mode = requireIntention(mode).id;
  state.phase = "obtaining";
  state.terminal = null;
  state.obtainment = null;
  state.binding = null;
  state.realization = null;
  state.observation = null;
  state.admission = null;
  clearDownload(runner, state);
  resetStages(state);
  setButtons(state, null);
  state.selectionDisabled = false;
  presentPhysicalSelection(state);
  const options = runner.querySelector(".target-options");
  options.replaceChildren();
  const support = state.entry.intentions.find((candidate) => candidate.id === state.mode);
  if (!support.supported) {
    fail(runner, state, "obtain", selectionFailure(state, "UnsupportedCombination", "selected target does not offer the selected physical Host intention"));
    return;
  }
  if (!state.adapter) {
    state.adapter = state.catalog.createAdapter({ targetId: state.entry.target.id, host, presentationFor: state.presentationFor });
  }
  const targetOptions = state.adapter.createOptions({
    mode: state.mode,
    onChange: () => selectMode(runner, host, state, state.mode),
  });
  if (targetOptions) options.append(targetOptions);
  renderEvidence(runner, state);
  const configuration = state.adapter.configuration?.();
  if (configuration?.required && !configuration.checked) {
    state.phase = "configuring";
    status(runner, state, "Choose and review the finite Host machinery. No Host, Boot, permission, resource, offer, Plan, or Play exists yet.");
    return;
  }
  void operate(runner, state, "obtain", (signal) => state.adapter.obtain({
    mode: state.mode,
    body: currentBody(host.runtime),
    signal,
  }), (result) => {
    state.obtainment = result;
    state.phase = "obtained";
    completeStage(state, "obtain", `${result.resultKind} · exact`);
    setButtons(state, "bind");
    status(runner, state, "Exact machinery result retained. Invitation, realization, Boot, join, membership, offers, Plan, and Play remain absent.");
  });
}

function bindInvitation(runner, host, state) {
  void operate(runner, state, "bind", (signal) => state.adapter.bind({
    mode: state.mode,
    body: currentBody(host.runtime),
    obtainment: state.obtainment,
    nowMillis: Date.now(),
    signal,
  }), (result) => {
    state.binding = result;
    state.phase = "bound";
    completeStage(state, "bind", short(result.prepared.spore_id));
    state.selectionDisabled = true;
    presentPhysicalSelection(state);
    setOptionsDisabled(runner, true);
    setButtons(state, "realize");
    renderDownload(runner, state, result.download);
    status(runner, state, "Invitation bound. Realization, Boot, join, membership, offers, Plan, and Play remain absent.");
  });
}

function renderDownload(runner, state, download) {
  if (!download || download.schema !== "conduit.spore/browser-artifact@1"
    || !(download.payload instanceof Uint8Array) || download.payload.byteLength !== download.bytes
    || typeof download.filename !== "string") {
    throw new TypeError("physical Host adapter omitted its downloadable spore artifact");
  }
  clearDownload(runner, state);
  state.download = download;
  presentPhysicalArtifact(state, download, async (artifact) => {
    try {
      const outcome = await state.hostOperations.handoffArtifact(artifact);
      if (!["completed", "handoff-offered"].includes(outcome.disposition)) {
        status(runner, state, `Artifact handoff refused: ${outcome.disposition}`, true);
      }
    } catch (error) {
      status(runner, state, `Artifact handoff refused: ${error.code ?? error.message}`, true);
    }
  });
}

function clearDownload(runner, state) {
  state.download = null;
  presentPhysicalArtifact(state, null, () => {});
}

function realizeHost(runner, host, state) {
  void operate(runner, state, "realize", (signal) => state.adapter.realize({
    mode: state.mode,
    host,
    obtainment: state.obtainment,
    binding: state.binding,
    signal,
  }), (result) => {
    state.realization = result;
    state.phase = "realized";
    completeStage(state, "realize", result.terminal);
    setButtons(state, "observe");
    status(runner, state, "Carrier realization completed. No Boot or join has been observed, and no membership, offers, readiness, Plan, or Play has been admitted.");
  });
}

function observeJoin(runner, host, state) {
  void operate(runner, state, "observe", (signal) => state.adapter.observe({
    mode: state.mode,
    host,
    obtainment: state.obtainment,
    binding: state.binding,
    realization: state.realization,
    signal,
  }), (result) => {
    state.observation = result;
    state.phase = "observed";
    completeStage(state, "observe", short(result.join.boot_id));
    setButtons(state, "admit");
    status(runner, state, "Fresh Boot advertisement and invitation-bound join observed. Admission remains an explicit action.");
  });
}

function admitPart(runner, host, state, onBodyChanged) {
  void operate(runner, state, "admit", () => Promise.resolve(admitObservation(host.runtime, state.observation.join)), (result) => {
    state.admission = { evidence: result };
    state.phase = "admitted";
    completeStage(state, "admit", `revision ${result.membership_revision}`);
    setButtons(state, null);
    status(runner, state, `Physical Part admitted; ${result.offer_count} current offers are ready. No Plan or Play was created.`);
    onBodyChanged?.();
  });
}

async function operate(runner, state, operation, work, accept) {
  if (state.active) return;
  if (state.admittedOperations >= state.adapter.bounds.maximumOperations) {
    fail(runner, state, operation, workflowFailure(state, operation, "OperationBound", "physical Host workflow exhausted its admitted operation bound"));
    return;
  }
  state.admittedOperations += 1;
  const generation = state.generation + 1;
  state.generation = generation;
  const controller = new AbortController();
  state.active = { operation, generation, controller };
  state.phase = operation;
  state.terminal = null;
  setButtons(state, null);
  state.cancelEnabled = true;
  presentPhysicalActions(state);
  renderEvidence(runner, state);
  try {
    const result = await work(controller.signal);
    if (!currentOperation(state, generation) || controller.signal.aborted) return;
    requireOperationEvidence(state, result?.evidence ?? result, operation);
    accept(result);
    state.active = null;
    state.cancelEnabled = false;
    presentPhysicalActions(state);
    renderEvidence(runner, state);
  } catch (error) {
    if (!currentOperation(state, generation)) return;
    state.active = null;
    state.cancelEnabled = false;
    presentPhysicalActions(state);
    let evidence = targetFailure(state, operation, error);
    try {
      requireOperationEvidence(state, evidence, operation);
    } catch (boundError) {
      evidence = workflowFailure(state, operation, boundError.code, boundError.message);
    }
    fail(runner, state, operation, evidence);
  }
}

function cancelActive(runner, state, terminal) {
  const active = state.active;
  if (!active) return;
  state.generation += 1;
  active.controller.abort();
  state.active = null;
  void Promise.resolve(state.adapter.cancel({ mode: state.mode, operation: active.operation })).catch(() => {});
  state.cancelEnabled = false;
  presentPhysicalActions(state);
  if (terminal) {
    state.cancellations += 1;
    fail(runner, state, active.operation, workflowFailure(state, active.operation, "Cancelled", "operator cancelled the active physical Host operation"));
  }
}

function fail(runner, state, operation, evidence) {
  state.phase = "terminal";
  state.terminal = evidence;
  clearOperationResult(state, operation);
  setButtons(state, null);
  status(runner, state, `${operation} refused: ${evidence.terminal}: ${evidence.message}`, true);
  renderEvidence(runner, state);
}

function targetFailure(state, operation, error) {
  if (typeof error?.evidence?.terminal === "string") return error.evidence;
  const evidence = workflowFailure(
    state,
    operation,
    boundedText(error?.code, 64) ? error.code : "OperationFailed",
    error instanceof Error ? error.message : String(error),
  );
  return error?.evidence ? Object.freeze({ ...evidence, target_evidence: error.evidence }) : evidence;
}

function workflowFailure(state, operation, code, message) {
  return Object.freeze({
    schema: FAILURE_SCHEMA,
    target_id: state.adapter.target.id,
    mode: state.mode,
    operation,
    terminal: code,
    message,
  });
}

function selectionFailure(state, code, message) {
  return Object.freeze({
    schema: SELECTION_FAILURE_SCHEMA,
    catalog_generation: state.catalog.generation,
    target_id: state.entry.target.id,
    family_id: state.entry.family.id,
    model_id: state.entry.target.model_id,
    profile_id: state.entry.target.profile_id,
    mode: state.mode,
    result_kind: requireIntention(state.mode).resultKind,
    operation: "obtain",
    terminal: code,
    message,
    authority_requested: false,
    artifact_work_started: false,
  });
}

function requireOperationEvidence(state, evidence, operation) {
  let bytes;
  try {
    bytes = encoder.encode(JSON.stringify(evidence));
  } catch {
    throw Object.assign(new Error(`${operation} evidence is not finite JSON`), { code: "MalformedEvidence" });
  }
  if (bytes.length > state.adapter.bounds.maximumOperationEvidenceBytes) {
    throw Object.assign(new Error(`${operation} evidence exceeds its admitted byte bound`), { code: "EvidenceBound" });
  }
}

function renderEvidence(runner, state) {
  const evidence = {
    schema: WORKFLOW_SCHEMA,
    catalog: {
      schema: state.catalog.schema,
      generation: state.catalog.generation,
      bounds: state.catalog.bounds,
    },
    target_entry: state.entry,
    target: state.entry.target,
    intention: {
      schema: "conduit.creche/physical-host-intention@1",
      mode: state.mode,
      result_kind: requireIntention(state.mode).resultKind,
      supported: state.entry.intentions.find((mode) => mode.id === state.mode).supported,
    },
    bounds: state.entry.bounds,
    admitted_operations: state.admittedOperations,
    cancellations: state.cancellations,
    active_operation: state.active?.operation ?? null,
    phase: state.phase,
    terminal: state.terminal,
    obtainment: state.obtainment?.evidence ?? null,
    binding: state.binding?.evidence ?? null,
    realization: state.realization?.evidence ?? null,
    observation: state.observation?.evidence ?? null,
    admission: state.admission?.evidence ?? null,
  };
  const encoded = encoder.encode(JSON.stringify(evidence));
  if (encoded.length > state.entry.bounds.maximumRetainedEvidenceBytes) {
    throw new RangeError("physical Host retained evidence exceeds its admitted byte bound");
  }
  const disposition = state.terminal
    ? state.terminal.terminal === "Cancelled" || state.terminal.schema === SELECTION_FAILURE_SCHEMA
      ? "refused-evidence"
      : "failed-evidence"
    : state.phase === "admitted" ? "successful-evidence" : "artifact";
  presentPhysicalEvidence(state, evidence, disposition);
}

function admitObservation(api, join) {
  const encoded = encoder.encode(JSON.stringify(join));
  if (encoded.length === 0 || encoded.length > api.conduit_creche_input_capacity()) {
    throw new RangeError("join observation exceeds the admitted Body input bound");
  }
  new Uint8Array(api.memory.buffer, api.conduit_creche_input_ptr(), encoded.length).set(encoded);
  const code = api.conduit_creche_admit_physical_spore(encoded.length);
  if (code < 0) throw outputError(api, "Part admission", code);
  return readOutput(api);
}

function currentBody(api) {
  const code = api.conduit_creche_current();
  if (code === 1) return null;
  if (code < 0) throw outputError(api, "Body projection", code);
  return readOutput(api);
}

function readOutput(api) {
  return JSON.parse(decoder.decode(new Uint8Array(
    api.memory.buffer,
    api.conduit_creche_output_ptr(),
    api.conduit_creche_output_len(),
  )));
}

function outputError(api, operation, code) {
  const evidence = api.conduit_creche_output_len() > 0 ? readOutput(api) : null;
  return Object.assign(new Error(evidence?.message ?? `${operation} refused (${code})`), { code: "BodyRefusal", evidence });
}

function resetStages(state) {
  for (const name of Object.keys(state.stages)) state.stages[name] = "waiting";
  presentPhysicalProgress(state);
}

function completeStage(state, name, value) {
  state.stages[name] = value;
  presentPhysicalProgress(state);
}

function setButtons(state, enabled) {
  state.actionEnabled = enabled;
  presentPhysicalActions(state);
}

function setOptionsDisabled(runner, disabled) {
  for (const control of runner.querySelectorAll(".target-options button, .target-options input, .target-options select")) {
    control.disabled = disabled;
  }
}

function status(runner, state, message, error = false) {
  presentPhysicalStatus(state, message, error);
}

function clearOperationResult(state, operation) {
  const field = { obtain: "obtainment", bind: "binding", realize: "realization", observe: "observation", admit: "admission" }[operation];
  if (field) state[field] = null;
}

function currentOperation(state, generation) {
  return state.active?.generation === generation && state.generation === generation;
}

function requireIntention(mode) {
  const intention = PHYSICAL_HOST_INTENTIONS.find((candidate) => candidate.id === mode);
  if (!intention) throw new TypeError("physical Host intention is unknown");
  return intention;
}

function boundedText(value, maximum) {
  return typeof value === "string" && value.length > 0 && value.length <= maximum;
}

function short(value) {
  return value.length > 24 ? `${value.slice(0, 21)}…` : value;
}
