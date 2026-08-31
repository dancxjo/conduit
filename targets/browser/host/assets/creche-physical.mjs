import { PHYSICAL_HOST_INTENTIONS } from "./creche-target-catalog.mjs";

const WORKFLOW_SCHEMA = "conduit.creche/physical-host-workflow-evidence@1";
const FAILURE_SCHEMA = "conduit.creche/physical-host-workflow-failure@1";
const SELECTION_FAILURE_SCHEMA = "conduit.creche/physical-host-target-selection-failure@1";
const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function createPhysicalHostRunner({ host, targetCatalog }) {
  const catalog = requireTargetCatalog(targetCatalog);
  const runner = document.createElement("section");
  runner.className = "physical-host-runner";
  runner.innerHTML = `
    <ol class="physical-stages" aria-label="Add one physical Host">
      <li data-stage="obtain"><strong>1 · Obtain exact machinery</strong><span>waiting</span></li>
      <li data-stage="bind"><strong>2 · Bind invitation</strong><span>waiting</span></li>
      <li data-stage="realize"><strong>3 · Deploy, install, or attach</strong><span>waiting</span></li>
      <li data-stage="observe"><strong>4 · Observe Boot + join</strong><span>waiting</span></li>
      <li data-stage="admit"><strong>5 · Admit Part + offers</strong><span>waiting</span></li>
    </ol>
    <div class="physical-selection">
      <label>Intention
        <span class="select-field"><select class="physical-mode"></select></span>
      </label>
      <label>Target
        <span class="select-field"><select class="physical-target"></select></span>
      </label>
      <div class="target-options"></div>
      <div class="spore-download" aria-live="polite"></div>
    </div>
    <div class="physical-actions">
      <button class="bind" type="button" disabled>Bind Body invitation</button>
      <button class="realize" type="button" disabled>Realize selected Host</button>
      <button class="observe" type="button" disabled>Observe Boot and join</button>
      <button class="admit" type="button" disabled>Admit Part and offers</button>
      <button class="cancel" type="button" disabled>Cancel current operation</button>
    </div>
    <p class="physical-status" role="status">Preparing the selected target without requesting machine access.</p>
    <details><summary>Exact physical-Host evidence</summary><pre><code></code></pre></details>`;

  const modeControl = runner.querySelector(".physical-mode");
  const targetControl = runner.querySelector(".physical-target");
  for (const family of catalog.families) {
    const heading = document.createElement("optgroup");
    heading.label = family.label;
    heading.dataset.familyId = family.id;
    for (const entry of family.entries) {
      const option = document.createElement("option");
      option.value = entry.target.id;
      option.textContent = entry.target.label;
      heading.append(option);
    }
    targetControl.append(heading);
  }

  const state = {
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
    downloadUrl: null,
  };

  modeControl.addEventListener("change", () => selectMode(runner, host, state, modeControl.value));
  targetControl.addEventListener("change", () => selectTarget(runner, host, state, targetControl.value));
  runner.querySelector(".bind").addEventListener("click", () => bindInvitation(runner, host, state));
  runner.querySelector(".realize").addEventListener("click", () => realizeHost(runner, host, state));
  runner.querySelector(".observe").addEventListener("click", () => observeJoin(runner, host, state));
  runner.querySelector(".admit").addEventListener("click", () => admitPart(runner, host, state));
  runner.querySelector(".cancel").addEventListener("click", () => cancelActive(runner, state, true));
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
  runner.querySelector(".physical-target").value = targetId;
  const modeControl = runner.querySelector(".physical-mode");
  modeControl.replaceChildren();
  for (const intention of PHYSICAL_HOST_INTENTIONS) {
    const support = entry.intentions.find((mode) => mode.id === intention.id);
    const option = document.createElement("option");
    option.value = intention.id;
    option.textContent = `${intention.label}${support.supported ? "" : " · unavailable for this target"}`;
    modeControl.append(option);
  }
  const currentSupport = entry.intentions.find((mode) => mode.id === state.mode);
  if (!currentSupport?.supported) {
    state.mode = entry.intentions.find((mode) => mode.supported)?.id ?? state.mode;
  }
  modeControl.value = state.mode;
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
  resetStages(runner);
  setButtons(runner, null);
  runner.querySelector(".physical-mode").disabled = false;
  runner.querySelector(".physical-target").disabled = false;
  const options = runner.querySelector(".target-options");
  options.replaceChildren();
  const support = state.entry.intentions.find((candidate) => candidate.id === state.mode);
  if (!support.supported) {
    fail(runner, state, "obtain", selectionFailure(state, "UnsupportedCombination", "selected target does not offer the selected physical Host intention"));
    return;
  }
  if (!state.adapter) {
    state.adapter = state.catalog.createAdapter({ targetId: state.entry.target.id, host });
  }
  const targetOptions = state.adapter.createOptions({
    mode: state.mode,
    onChange: () => selectMode(runner, host, state, state.mode),
  });
  if (targetOptions) options.append(targetOptions);
  renderEvidence(runner, state);
  void operate(runner, state, "obtain", (signal) => state.adapter.obtain({
    mode: state.mode,
    body: currentBody(host.runtime),
    signal,
  }), (result) => {
    state.obtainment = result;
    state.phase = "obtained";
    completeStage(runner, "obtain", `${result.resultKind} · exact`);
    setButtons(runner, "bind");
    status(runner, "Exact machinery result retained. Invitation, realization, Boot, join, membership, offers, Plan, and Play remain absent.");
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
    completeStage(runner, "bind", short(result.prepared.spore_id));
    runner.querySelector(".physical-mode").disabled = true;
    runner.querySelector(".physical-target").disabled = true;
    setOptionsDisabled(runner, true);
    setButtons(runner, "realize");
    renderDownload(runner, state, result.download);
    status(runner, "Invitation bound. Realization, Boot, join, membership, offers, Plan, and Play remain absent.");
  });
}

function renderDownload(runner, state, download) {
  if (!download || download.schema !== "conduit.spore/browser-download@1"
    || !(download.blob instanceof Blob) || typeof download.filename !== "string") {
    throw new TypeError("physical Host adapter omitted its downloadable spore bundle");
  }
  clearDownload(runner, state);
  state.downloadUrl = URL.createObjectURL(download.blob);
  const link = document.createElement("a");
  link.className = "download-spore";
  link.href = state.downloadUrl;
  link.download = download.filename;
  link.textContent = `Download spore · ${download.bytes} bytes`;
  runner.querySelector(".spore-download").append(link);
}

function clearDownload(runner, state) {
  if (state.downloadUrl) URL.revokeObjectURL(state.downloadUrl);
  state.downloadUrl = null;
  runner.querySelector(".spore-download")?.replaceChildren();
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
    completeStage(runner, "realize", result.terminal);
    setButtons(runner, "observe");
    status(runner, "Carrier realization completed. No Boot or join has been observed, and no membership, offers, readiness, Plan, or Play has been admitted.");
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
    completeStage(runner, "observe", short(result.join.boot_id));
    setButtons(runner, "admit");
    status(runner, "Fresh Boot advertisement and invitation-bound join observed. Admission remains an explicit action.");
  });
}

function admitPart(runner, host, state) {
  void operate(runner, state, "admit", () => Promise.resolve(admitObservation(host.runtime, state.observation.join)), (result) => {
    state.admission = { evidence: result };
    state.phase = "admitted";
    completeStage(runner, "admit", `revision ${result.membership_revision}`);
    setButtons(runner, null);
    status(runner, `Physical Part admitted; ${result.offer_count} current offers are ready. No Plan or Play was created.`);
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
  setButtons(runner, null);
  runner.querySelector(".cancel").disabled = false;
  renderEvidence(runner, state);
  try {
    const result = await work(controller.signal);
    if (!currentOperation(state, generation) || controller.signal.aborted) return;
    requireOperationEvidence(state, result?.evidence ?? result, operation);
    accept(result);
    state.active = null;
    runner.querySelector(".cancel").disabled = true;
    renderEvidence(runner, state);
  } catch (error) {
    if (!currentOperation(state, generation)) return;
    state.active = null;
    runner.querySelector(".cancel").disabled = true;
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
  runner.querySelector(".cancel").disabled = true;
  if (terminal) {
    state.cancellations += 1;
    fail(runner, state, active.operation, workflowFailure(state, active.operation, "Cancelled", "operator cancelled the active physical Host operation"));
  }
}

function fail(runner, state, operation, evidence) {
  state.phase = "terminal";
  state.terminal = evidence;
  clearOperationResult(state, operation);
  setButtons(runner, null);
  status(runner, `${operation} refused: ${evidence.terminal}: ${evidence.message}`, true);
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
  runner.querySelector("details code").textContent = JSON.stringify(evidence, null, 2);
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

function resetStages(runner) {
  for (const stage of runner.querySelectorAll(".physical-stages li")) {
    stage.classList.remove("complete");
    stage.querySelector("span").textContent = "waiting";
  }
}

function completeStage(runner, name, value) {
  const stage = runner.querySelector(`[data-stage="${name}"]`);
  stage.classList.add("complete");
  stage.querySelector("span").textContent = value;
}

function setButtons(runner, enabled) {
  for (const name of ["bind", "realize", "observe", "admit"]) {
    runner.querySelector(`.${name}`).disabled = name !== enabled;
  }
}

function setOptionsDisabled(runner, disabled) {
  for (const control of runner.querySelectorAll(".target-options button, .target-options input, .target-options select")) {
    control.disabled = disabled;
  }
}

function status(runner, message, error = false) {
  const element = runner.querySelector(".physical-status");
  element.classList.toggle("error", error);
  element.textContent = message;
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
