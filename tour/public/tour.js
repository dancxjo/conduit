import init, {
  explain_panel,
  parse_panel,
  patchbay_move_node,
  patchbay_replace_source,
} from "./conduit_web.js";

const source = document.querySelector("#source");
const result = document.querySelector("#result");
const runButton = document.querySelector("#run");
const stopButton = document.querySelector("#stop");
const undoResetButton = document.querySelector("#undo-reset");
const lessons = await (await fetch("../lessons/v1.json", { cache: "no-store" })).json();
const browserPlan = await (await fetch("./browser-plan.json", { cache: "no-store" })).json();

async function fetchArtifact(artifact) {
  const url = new URL(artifact.path, import.meta.url);
  const response = await fetch(url, { cache: "no-store" });
  if (!response.ok) throw new Error(`artifact-fetch:${artifact.id}:${response.status}`);
  const bytes = await response.arrayBuffer();
  if (bytes.byteLength !== artifact.bytes) {
    throw new Error(`artifact-size:${artifact.id}`);
  }
  return { artifact, bytes, url };
}

async function sha256Hex(bytes) {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)]
    .map((value) => value.toString(16).padStart(2, "0"))
    .join("");
}

if (browserPlan.schema !== "conduit.tour-browser-plan/v1") {
  throw new Error("unsupported Tour browser plan");
}
const adapterArtifact = browserPlan.artifacts.find(
  (artifact) => artifact.id === "browser-host-adapter",
);
const loadedAdapter = await fetchArtifact(adapterArtifact);
if (await sha256Hex(loadedAdapter.bytes) !== adapterArtifact.sha256) {
  throw new Error("browser host adapter integrity mismatch");
}
const {
  BrowserHostReason,
  DedicatedWorkerExecutionAdapter,
  Placement,
  observeBrowserHost,
  resolveBrowserPlacement,
  verifyExactArtifact,
} = await import(loadedAdapter.url);

const loadedArtifacts = new Map([[adapterArtifact.id, loadedAdapter]]);
for (const artifact of browserPlan.artifacts) {
  if (loadedArtifacts.has(artifact.id)) continue;
  const loaded = await fetchArtifact(artifact);
  const verified = await verifyExactArtifact(loaded.bytes, artifact.sha256);
  if (!verified.ok) {
    throw new Error(`${BrowserHostReason.ArtifactIntegrity}:${artifact.id}`);
  }
  loadedArtifacts.set(artifact.id, loaded);
}
await init({
  module_or_path: loadedArtifacts.get("conduit-web-wasm").bytes,
});

const placementFact = (id, available, lifetime, scheduling, transfer, terminalRisks) => ({
  id,
  available,
  lifetime,
  scheduling,
  transfer,
  limits: { queueBytes: browserPlan.bounds.maximum_message_bytes },
  terminalRisks,
});
const hostReport = observeBrowserHost({
  hostId: "conduit/tour-browser-host",
  observationId: browserPlan.observation_id,
  reporter: {
    realmId: "conduit/tour-static-realm",
    entityId: "conduit/tour-browser-workload",
    passportIdentity: "conduit/tour-browser-passport-v1",
    statusObservation: {
      realmId: "conduit/tour-static-realm",
      entityId: "conduit/tour-browser-workload",
      passportIdentity: "conduit/tour-browser-passport-v1",
      reporterIdentity: "conduit/tour-static-status-reporter",
      timeBasis: "conduit/tour-fixture-clock",
      observedAtTick: 9,
      validUntilTick: 100,
      status: "active",
    },
  },
  tick: 10,
  validUntilTick: 100,
  context: {
    secureContext: globalThis.isSecureContext,
    origin: globalThis.location.origin,
    crossOriginIsolated: globalThis.crossOriginIsolated,
  },
  placements: [
    placementFact(
      Placement.DedicatedWorker,
      typeof Worker === "function",
      "worker",
      "event-loop",
      "structured-clone",
      ["worker-death", "page-close"],
    ),
    placementFact(
      Placement.Wasm,
      typeof WebAssembly === "object",
      "worker",
      "placement-owned",
      "linear-memory",
      ["trap", "worker-death"],
    ),
  ],
  permissions: [],
  activation: false,
  resources: {
    queueBytes: browserPlan.bounds.maximum_message_bytes,
    pendingMessages: browserPlan.bounds.maximum_pending,
  },
});
if (hostReport.ok === false) {
  throw new Error(`${hostReport.code}:${hostReport.detail}`);
}

let current = lessons.lessons[0];
let acceptedSource = "";
let selectedNode = null;
let positions = {};
let activeAdapter = null;
let runEpoch = 0;
let topologyView = "logical";
const evidence = [];
const draftKey = (id) => `conduit-tour-draft/${id}`;
const recoveryKey = (id) => `conduit-tour-reset-recovery/${id}`;

function recordEvidence(event) {
  evidence.push(event);
  const maximum = Math.min(
    browserPlan.bounds.maximum_evidence_events,
    current.budgets.evidence_events || browserPlan.bounds.maximum_evidence_events,
  );
  if (evidence.length > maximum) evidence.splice(0, evidence.length - maximum);
  document.querySelector("#evidence").textContent =
    evidence.length === 0 ? "No run evidence yet." : JSON.stringify(evidence, null, 2);
}

function renderPlan(binding = null) {
  document.querySelector("#plan").textContent = JSON.stringify({
    source_profile: current.profile,
    semantic_contract: browserPlan.semantic_contract,
    implementation_id: browserPlan.implementation_id,
    plan_identity: browserPlan.plan_identity,
    host_observation: hostReport.observationId,
    placement: binding?.placement ?? browserPlan.placement,
    bounds: browserPlan.bounds,
    artifacts: browserPlan.artifacts.map(({ id, sha256, bytes }) => ({ id, sha256, bytes })),
  }, null, 2);
}

function stopActive(cause, message) {
  runEpoch += 1;
  if (activeAdapter) {
    activeAdapter.terminate(cause);
    activeAdapter = null;
  }
  runButton.disabled = false;
  stopButton.disabled = true;
  if (message) result.textContent = message;
}

function show(lesson) {
  stopActive("lesson-changed");
  current = lesson;
  document.querySelector("#title").textContent = lesson.title;
  document.querySelector("#goal").textContent = lesson.objective;
  document.querySelector("#prose").textContent = lesson.prose;
  document.querySelector("#execution-note").textContent =
    `${lesson.profile}: exact ${browserPlan.placement} placement on ${hostReport.hostId}.`;
  document.querySelector("#command").textContent =
    (lesson.commands ?? [lesson.command]).join("  ·  ");
  const draft = localStorage.getItem(draftKey(lesson.id));
  source.value = draft ?? lesson.source;
  const parsedDraft = JSON.parse(parse_panel(source.value));
  acceptedSource = parsedDraft.ok ? source.value : lesson.source;
  selectedNode = null;
  positions = {};
  topologyView = "logical";
  undoResetButton.disabled = localStorage.getItem(recoveryKey(lesson.id)) === null;
  document.querySelector("#topology-inspector").open =
    lesson.presentation.layout === "logical-expanded";
  evidence.length = 0;
  recordEvidence({ kind: "lesson-selected", lesson: lesson.id });
  renderPlan();
  check();
}

function selectNode(node) {
  selectedNode = node;
  const start = source.value.indexOf(`node ${node} `);
  if (start >= 0) {
    source.focus();
    source.setSelectionRange(start, start + node.length + 5);
  }
  renderPanel(JSON.parse(parse_panel(source.value)));
}

function renderPanel(value) {
  const panel = document.querySelector("#panel");
  panel.replaceChildren(...(value.node_labels ?? []).map((label) => {
    const node = label.split(" : ")[0];
    const button = document.createElement("button");
    const item = document.createElement("li");
    button.textContent = label;
    button.setAttribute("aria-pressed", String(node === selectedNode));
    button.style.transform = `translateX(${positions[node]?.x ?? 0}px)`;
    button.onclick = () => selectNode(node);
    item.append(button);
    return item;
  }));
  document.querySelector("#move-left").disabled = !selectedNode;
  document.querySelector("#move-right").disabled = !selectedNode;
}

function check() {
  const value = JSON.parse(parse_panel(source.value));
  result.textContent = value.ok
    ? `Valid panel: ${value.nodes} nodes, ${value.cords} cords.`
    : value.diagnostic;
  renderPanel(value);
  renderTopology();
}

function renderTopology() {
  const explanation = JSON.parse(explain_panel(source.value));
  document.querySelector("#logical-view").setAttribute(
    "aria-pressed",
    String(topologyView === "logical"),
  );
  document.querySelector("#expanded-view").setAttribute(
    "aria-pressed",
    String(topologyView === "expanded"),
  );
  document.querySelector("#topology").textContent = explanation.ok
    ? explanation[topologyView]
    : explanation.diagnostic;
}

for (const lesson of lessons.lessons) {
  const button = document.createElement("button");
  button.textContent = lesson.title;
  button.onclick = () => show(lesson);
  const item = document.createElement("li");
  item.append(button);
  document.querySelector("#lessons").append(item);
}

source.addEventListener("input", () => {
  localStorage.setItem(draftKey(current.id), source.value);
  const transaction = JSON.parse(patchbay_replace_source(acceptedSource, source.value));
  if (transaction.ok) acceptedSource = source.value;
  check();
});
source.addEventListener("select", () => {
  const before = source.value.slice(0, source.selectionStart);
  const match = before.match(/node\s+([A-Za-z][A-Za-z0-9_-]*)\s*$/);
  if (match) {
    selectedNode = match[1];
    renderPanel(JSON.parse(parse_panel(source.value)));
  }
});
document.querySelector("#check").onclick = check;
document.querySelector("#logical-view").onclick = () => {
  topologyView = "logical";
  renderTopology();
};
document.querySelector("#expanded-view").onclick = () => {
  topologyView = "expanded";
  renderTopology();
};

function moveSelected(delta) {
  if (!selectedNode) return;
  const position = positions[selectedNode] ?? { x: 0, y: 0 };
  const transaction = JSON.parse(
    patchbay_move_node(source.value, selectedNode, position.x + delta, position.y),
  );
  if (!transaction.ok) {
    result.textContent = transaction.diagnostic;
    return;
  }
  positions = transaction.positions;
  result.textContent =
    `Presentation moved; semantic hash remains ${transaction.semantic_hash}.`;
  check();
}
document.querySelector("#move-left").onclick = () => moveSelected(-16);
document.querySelector("#move-right").onclick = () => moveSelected(16);

async function run() {
  stopActive("superseded");
  const epoch = ++runEpoch;
  const binding = resolveBrowserPlacement(hostReport, {
    tick: 11,
    placement: Placement.DedicatedWorker,
    minimumResources: {
      queueBytes: browserPlan.bounds.maximum_message_bytes,
      pendingMessages: browserPlan.bounds.maximum_pending,
    },
  });
  if (!binding.ok) {
    result.textContent = `${binding.code}: ${binding.detail}`;
    recordEvidence({ kind: "placement-rejected", code: binding.code });
    return;
  }
  renderPlan(binding);
  const workerArtifact = loadedArtifacts.get("tour-worker");
  const wasmArtifact = loadedArtifacts.get("conduit-web-wasm");
  const adapter = new DedicatedWorkerExecutionAdapter({
    ...binding,
    planIdentity: browserPlan.plan_identity,
    artifactUrl: workerArtifact.url,
    maximumPending: browserPlan.bounds.maximum_pending,
    maximumMessageBytes: browserPlan.bounds.maximum_message_bytes,
    responseTimeoutMs: browserPlan.bounds.response_timeout_ms,
  }, recordEvidence);
  activeAdapter = adapter;
  const started = adapter.start();
  if (!started.ok) {
    activeAdapter = null;
    result.textContent = `${started.code}: ${started.detail}`;
    return;
  }
  runButton.disabled = true;
  stopButton.disabled = false;
  result.textContent = "Running in the resolved browser worker…";
  try {
    const configured = await adapter.request("configure", {
      wasmUrl: wasmArtifact.url.href,
      wasmSha256: wasmArtifact.artifact.sha256,
    });
    if (!configured.ok) throw new Error(configured.code);
    const executed = await adapter.request("run", { source: source.value });
    if (epoch !== runEpoch) return;
    if (!executed.ok) throw new Error(executed.code);
    const value = executed.value;
    result.textContent = value.ok
      ? `${value.stdout || "Run completed."}\nEvidence: ${value.completed_nodes} nodes, ${value.cords_conducted} cords conducted.`
      : value.diagnostic;
    recordEvidence({
      kind: value.ok ? "run-completed" : "run-rejected",
      lesson: current.id,
      completedNodes: value.completed_nodes,
      cordsConducted: value.cords_conducted,
    });
  } catch (error) {
    if (epoch === runEpoch) result.textContent = `Run failed: ${error}`;
  } finally {
    if (epoch === runEpoch) {
      adapter.terminate("completed");
      activeAdapter = null;
      runButton.disabled = false;
      stopButton.disabled = true;
    }
  }
}

runButton.onclick = run;
stopButton.onclick = () => stopActive(
  "learner-cancelled",
  "Run cancelled; the exact worker placement is terminal.",
);
source.addEventListener("keydown", (event) => {
  if (event.shiftKey && event.key === "Enter") {
    event.preventDefault();
    run();
  }
});
document.querySelector("#reset").onclick = () => {
  stopActive("reset");
  localStorage.setItem(recoveryKey(current.id), source.value);
  source.value = current.source;
  acceptedSource = source.value;
  selectedNode = null;
  positions = {};
  localStorage.removeItem(draftKey(current.id));
  undoResetButton.disabled = false;
  check();
};
undoResetButton.onclick = () => {
  const recovered = localStorage.getItem(recoveryKey(current.id));
  if (recovered === null) return;
  source.value = recovered;
  localStorage.setItem(draftKey(current.id), recovered);
  localStorage.removeItem(recoveryKey(current.id));
  undoResetButton.disabled = true;
  const parsed = JSON.parse(parse_panel(recovered));
  acceptedSource = parsed.ok ? recovered : current.source;
  check();
};
document.querySelector("#download").onclick = () => {
  const link = document.createElement("a");
  link.href = URL.createObjectURL(new Blob([source.value], { type: "text/plain" }));
  link.download = `${current.id}.panel`;
  link.click();
  URL.revokeObjectURL(link.href);
};

show(current);
if (new URLSearchParams(location.search).has("autorun")) await run();
