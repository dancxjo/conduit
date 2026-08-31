const ADAPTER_SCHEMA = "conduit.creche/physical-host-target-adapter@1";
const WORKFLOW_SCHEMA = "conduit.creche/physical-host-workflow-evidence@1";
const FAILURE_SCHEMA = "conduit.creche/physical-host-workflow-failure@1";
const MAXIMUM_OPERATIONS = 16;
const MAXIMUM_OPERATION_EVIDENCE_BYTES = 32 * 1024;
const MAXIMUM_RETAINED_EVIDENCE_BYTES = 128 * 1024;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

const INTENTIONS = Object.freeze([
  Object.freeze({ id: "fabricate-new", label: "Fabricate new machinery", resultKind: "artifact" }),
  Object.freeze({ id: "install-existing", label: "Install on an existing computer", resultKind: "installation" }),
  Object.freeze({ id: "attach-running", label: "Attach an already running Host", resultKind: "attachment" }),
]);

export function createPhysicalHostRunner({ host, targetAdapter }) {
  const adapter = requireTargetAdapter(targetAdapter);
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
      <label>Target <output class="physical-target"></output></label>
      <div class="target-options"></div>
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
  for (const intention of INTENTIONS) {
    const support = adapter.modes.find((mode) => mode.id === intention.id);
    const option = document.createElement("option");
    option.value = intention.id;
    option.textContent = `${intention.label}${support.supported ? "" : " · unavailable for this target"}`;
    modeControl.append(option);
  }
  runner.querySelector(".physical-target").textContent = adapter.target.label;

  const state = {
    adapter,
    mode: INTENTIONS[0].id,
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
  };

  modeControl.addEventListener("change", () => selectMode(runner, host, state, modeControl.value));
  runner.querySelector(".bind").addEventListener("click", () => bindInvitation(runner, host, state));
  runner.querySelector(".realize").addEventListener("click", () => realizeHost(runner, host, state));
  runner.querySelector(".observe").addEventListener("click", () => observeJoin(runner, host, state));
  runner.querySelector(".admit").addEventListener("click", () => admitPart(runner, host, state));
  runner.querySelector(".cancel").addEventListener("click", () => cancelActive(runner, state, true));
  selectMode(runner, host, state, state.mode);
  return runner;
}

function requireTargetAdapter(adapter) {
  const methods = ["createOptions", "obtain", "bind", "realize", "observe", "cancel"];
  if (!adapter || adapter.schema !== ADAPTER_SCHEMA || methods.some((name) => typeof adapter[name] !== "function")) {
    throw new TypeError("physical Host target adapter contract is incomplete");
  }
  if (!adapter.target || !boundedText(adapter.target.id, 256) || !boundedText(adapter.target.label, 128)) {
    throw new TypeError("physical Host target identity is missing or outside its finite bound");
  }
  if (!Array.isArray(adapter.modes) || adapter.modes.length !== INTENTIONS.length) {
    throw new TypeError("physical Host target adapter must classify all three intentions");
  }
  for (const intention of INTENTIONS) {
    const mode = adapter.modes.find((candidate) => candidate.id === intention.id);
    if (!mode || typeof mode.supported !== "boolean" || mode.resultKind !== intention.resultKind) {
      throw new TypeError(`physical Host target adapter misclassified ${intention.id}`);
    }
  }
  const bounds = adapter.bounds;
  if (!boundedInteger(bounds?.maximumOperations, 1, MAXIMUM_OPERATIONS)
    || !boundedInteger(bounds?.maximumOperationEvidenceBytes, 256, MAXIMUM_OPERATION_EVIDENCE_BYTES)
    || !boundedInteger(bounds?.maximumRetainedEvidenceBytes, 1024, MAXIMUM_RETAINED_EVIDENCE_BYTES)) {
    throw new TypeError("physical Host target adapter bounds are missing or exceed the workflow maxima");
  }
  return adapter;
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
  resetStages(runner);
  setButtons(runner, null);
  runner.querySelector(".physical-mode").disabled = false;
  const options = runner.querySelector(".target-options");
  options.replaceChildren();
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
    setOptionsDisabled(runner, true);
    setButtons(runner, "realize");
    status(runner, "Invitation bound. Realization, Boot, join, membership, offers, Plan, and Play remain absent.");
  });
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
    status(runner, "Realization completed. That proves no Boot, join, membership, offers, readiness, Plan, or Play.");
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
    target: state.adapter.target,
    intention: {
      schema: "conduit.creche/physical-host-intention@1",
      mode: state.mode,
      result_kind: requireIntention(state.mode).resultKind,
      supported: state.adapter.modes.find((mode) => mode.id === state.mode).supported,
    },
    bounds: state.adapter.bounds,
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
  if (encoded.length > state.adapter.bounds.maximumRetainedEvidenceBytes) {
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
  const intention = INTENTIONS.find((candidate) => candidate.id === mode);
  if (!intention) throw new TypeError("physical Host intention is unknown");
  return intention;
}

function boundedText(value, maximum) {
  return typeof value === "string" && value.length > 0 && value.length <= maximum;
}

function boundedInteger(value, minimum, maximum) {
  return Number.isSafeInteger(value) && value >= minimum && value <= maximum;
}

function short(value) {
  return value.length > 24 ? `${value.slice(0, 21)}…` : value;
}
