import "./patchbay-components.js";
import init, {
  explain_panel,
  parse_panel,
  patchbay_apply_transaction,
  patchbay_open_session,
} from "./conduit_web.js";
import { PatchbayReactFlowRenderer } from "./patchbay-renderer.js";
import { patchbayFeatures } from "./patchbay-features.js";
import { attachPanelSourceHighlighting } from "./panel-highlighter.js";

const source = document.querySelector("#source");
const syncSourceHighlight = attachPanelSourceHighlighting(source);
const result = document.querySelector("#result");
const runButton = document.querySelector("#run");
const stopButton = document.querySelector("#stop");
const undoResetButton = document.querySelector("#undo-reset");
const consoleBadge = document.querySelector("#console-status-badge");
const selectedNodeLabel = document.querySelector("#selected-node-label");
const moveLeftBtn = document.querySelector("#move-left");
const moveRightBtn = document.querySelector("#move-right");
const runnabilityState = document.querySelector("#runnability-state");
const executionStory = document.querySelector("#execution-story");
const scenarioSelect = document.querySelector("#scenario");
const timelinePosition = document.querySelector("#timeline-position");
const timelinePositionLabel = document.querySelector("#timeline-position-label");
const timelineLanes = document.querySelector("#timeline-lanes");
const timelineExplanation = document.querySelector("#timeline-explanation");
const timelineTableBody = document.querySelector("#timeline-table tbody");

const lessons = await (await fetch("../lessons/v1.json", { cache: "no-store" })).json();
const browserPlan = await (await fetch("./browser-plan.json", { cache: "no-store" })).json();
const referenceManifest = await (
  await fetch("../reference-panels/v1.json", { cache: "no-store" })
).json();
if (referenceManifest.schema !== "conduit.tour-reference-panels/v1") {
  throw new Error("unsupported Tour reference-panel manifest");
}
const referencePanels = await Promise.all(referenceManifest.panels.map(async (panel) => {
  const response = await fetch(new URL(panel.source_path, import.meta.url), {
    cache: "no-store",
  });
  if (!response.ok) {
    throw new Error(`reference-panel-fetch:${panel.id}:${response.status}`);
  }
  return { ...panel, source: await response.text() };
}));

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
let patchbaySessionId = "";
let patchbaySourceRevision = 0;
let patchbayPresentationRevision = 0;
let patchbayView = null;
let activeAdapter = null;
let runEpoch = 0;
let topologyView = "logical";
const evidence = [];
let timelineRecords = [];
let timelineCursor = -1;
let timelineTimer = null;
const draftKey = (id) => `conduit-tour-draft/${id}`;
const recoveryKey = (id) => `conduit-tour-reset-recovery/${id}`;
const layoutKey = (id) => `conduit-tour-layout/${id}`;
const MIN_I32 = -2_147_483_648;
const MAX_I32 = 2_147_483_647;
const MAXIMUM_LAYOUT_OPERATIONS_PER_TRANSACTION = 32;

function validPosition(position) {
  return position &&
    Number.isInteger(position.x) &&
    Number.isInteger(position.y) &&
    position.x >= MIN_I32 &&
    position.x <= MAX_I32 &&
    position.y >= MIN_I32 &&
    position.y <= MAX_I32;
}

function rememberLayout(lessonId, nodePositions, view) {
  const movableNodeIds = new Set(
    (view.topology?.logical_nodes || []).map((node) => node.id),
  );
  const boundedPositions = {};
  for (const nodeId of movableNodeIds) {
    const position = nodePositions[nodeId];
    if (validPosition(position)) boundedPositions[nodeId] = position;
  }
  localStorage.setItem(layoutKey(lessonId), JSON.stringify(boundedPositions));
}

function rememberedLayoutOperations(lessonId, view) {
  let storedPositions;
  try {
    storedPositions = JSON.parse(localStorage.getItem(layoutKey(lessonId)) || "{}");
  } catch {
    localStorage.removeItem(layoutKey(lessonId));
    return [];
  }
  if (!storedPositions || typeof storedPositions !== "object" ||
      Array.isArray(storedPositions)) {
    localStorage.removeItem(layoutKey(lessonId));
    return [];
  }

  const movableNodeIds = new Set(
    (view.topology?.logical_nodes || []).map((node) => node.id),
  );
  const maximumNodes = Math.min(
    view.bounds?.maximum_nodes || 0,
    movableNodeIds.size,
  );
  return Object.entries(storedPositions)
    .filter(([nodeId, position]) =>
      movableNodeIds.has(nodeId) && validPosition(position)
    )
    .slice(0, maximumNodes)
    .map(([nodeId, position]) => ({
      MoveNode: {
        node_id: nodeId,
        position,
      },
    }));
}

// Initialize React Flow Patchbay Renderer
let patchbayRenderer = null;
const cyContainer = document.getElementById("cy");
document.querySelector(".node-controls").hidden =
  !patchbayFeatures.legacyLinePlacement;
if (cyContainer) {
  patchbayRenderer = new PatchbayReactFlowRenderer(cyContainer, {
    onTransaction: (operation) => applyPatchbayOperations([operation]),
    onNodeSelect: (nodeId) => {
      selectNode(nodeId);
    },
    onNotification: (msg) => {
      result.textContent = msg;
    }
  });
  patchbayRenderer.init();
}

function updateCytoscapeGraph() {
  if (patchbayRenderer && patchbayView) {
    patchbayRenderer.setViewModel(patchbayView, current.id);
  }
}

function openPatchbaySession() {
  patchbaySessionId = `tour/${current.id}`;
  const opened = JSON.parse(patchbay_open_session(patchbaySessionId, acceptedSource));
  if (!opened.ok) {
    patchbayView = null;
    result.textContent = opened.diagnostic;
    return false;
  }
  patchbayView = opened.view;
  patchbaySourceRevision = opened.view.source.revision;
  patchbayPresentationRevision = opened.view.presentation.revision;
  positions = opened.view.presentation.node_positions;
  const rememberedOperations = rememberedLayoutOperations(current.id, opened.view);
  if (rememberedOperations.length > 0) {
    for (let offset = 0; offset < rememberedOperations.length;
      offset += MAXIMUM_LAYOUT_OPERATIONS_PER_TRANSACTION) {
      const restored = applyPatchbayOperations(
        rememberedOperations.slice(
          offset,
          offset + MAXIMUM_LAYOUT_OPERATIONS_PER_TRANSACTION,
        ),
      );
      if (!restored.ok) {
        localStorage.removeItem(layoutKey(current.id));
        updateCytoscapeGraph();
        break;
      }
    }
    return true;
  }
  updateCytoscapeGraph();
  return true;
}

function applyPatchbayOperations(operations) {
  const request = {
    protocol_version: 1,
    document_id: patchbaySessionId,
    expected_source_revision: patchbaySourceRevision,
    expected_presentation_revision: patchbayPresentationRevision,
    operations
  };
  const transaction = JSON.parse(
    patchbay_apply_transaction(patchbaySessionId, JSON.stringify(request)),
  );
  if (!transaction.ok) {
    result.textContent = transaction.diagnostic;
    return transaction;
  }
  patchbayView = transaction.view;
  patchbaySourceRevision = transaction.result.source.revision;
  patchbayPresentationRevision = transaction.result.presentation.revision;
  acceptedSource = transaction.result.source.source;
  positions = transaction.result.presentation.node_positions;
  rememberLayout(current.id, positions, patchbayView);
  updateCytoscapeGraph();
  return transaction;
}

function recordEvidence(event) {
  evidence.push(event);
  const maximum = Math.min(
    browserPlan.bounds.maximum_evidence_events,
    current.budgets?.evidence_events || browserPlan.bounds.maximum_evidence_events,
  );
  if (evidence.length > maximum) evidence.splice(0, evidence.length - maximum);
  document.querySelector("#evidence").textContent =
    evidence.length === 0 ? "No run evidence yet." : JSON.stringify(evidence, null, 2);
}

function activeScenario() {
  return current.library?.scenarios?.find(
    (scenario) => scenario.id === scenarioSelect.value,
  ) || null;
}

function authoredSource() {
  return activeScenario()?.source || current.source;
}

function stopTimelinePlayback() {
  if (timelineTimer !== null) {
    clearInterval(timelineTimer);
    timelineTimer = null;
  }
}

function explainTimelineRecord(record) {
  if (record.source === "exact-run-result") {
    return `The exact browser run was rejected before evidence could be emitted: ${record.event_detail}`;
  }
  const time = `At deterministic tick ${record.tick}, event ${record.sequence}`;
  const subject = `${record.subject_kind} ${record.subject_id}`;
  const action = record.event_detail
    ? `${record.event_kind} (${record.event_detail})`
    : record.event_kind;
  const pressure = record.subject_kind === "cord"
    ? ` The cord used ${record.pressure} pressure with occupancy ${record.occupancy_items} items / ${record.occupancy_bytes} bytes.`
    : "";
  const terminal = record.terminal_cause
    ? ` The run became terminal: ${record.terminal_cause}.`
    : "";
  return `${time} records ${action} for ${subject}.${pressure}${terminal}`;
}

function highlightTimelineSubject(record) {
  document.querySelectorAll(".timeline-linked").forEach(
    (element) => element.classList.remove("timeline-linked"),
  );
  const targetId = record?.node_id || record?.cord_id;
  if (!targetId) return;
  if (record.node_id && patchbayRenderer) {
    patchbayRenderer.selectNode(record.node_id);
  }
  requestAnimationFrame(() => {
    document.querySelectorAll("[data-id]").forEach((element) => {
      if (element.dataset.id === targetId) element.classList.add("timeline-linked");
    });
  });
}

function selectTimelineRecord(index) {
  if (timelineRecords.length === 0) {
    timelineCursor = -1;
    timelinePosition.disabled = true;
    timelinePositionLabel.textContent = "No exact run evidence yet.";
    timelineExplanation.textContent =
      "Run a scenario to inspect its exact ordered evidence.";
    highlightTimelineSubject(null);
    return;
  }
  timelineCursor = Math.max(0, Math.min(index, timelineRecords.length - 1));
  timelinePosition.disabled = false;
  timelinePosition.max = String(timelineRecords.length - 1);
  timelinePosition.value = String(timelineCursor);
  const record = timelineRecords[timelineCursor];
  timelinePositionLabel.textContent =
    `${timelineCursor + 1} of ${timelineRecords.length}: ${record.event_kind}`;
  timelineExplanation.textContent = explainTimelineRecord(record);
  timelineLanes.querySelectorAll(".timeline-event").forEach((marker) => {
    const markerIndex = Number(marker.dataset.index);
    marker.classList.toggle("current", markerIndex === timelineCursor);
    marker.classList.toggle("future", markerIndex > timelineCursor);
    marker.setAttribute("aria-current", markerIndex === timelineCursor ? "true" : "false");
  });
  timelineTableBody.querySelectorAll("tr").forEach((row) => {
    row.classList.toggle("selected", Number(row.dataset.index) === timelineCursor);
  });
  highlightTimelineSubject(record);
}

function renderTimeline(records) {
  stopTimelinePlayback();
  timelineRecords = records;
  timelineLanes.replaceChildren();
  timelineTableBody.replaceChildren();
  const lanes = [...new Set(records.map((record) => record.subject_id))];
  for (const subjectId of lanes) {
    const lane = document.createElement("div");
    lane.className = "timeline-lane";
    const label = document.createElement("span");
    label.className = "timeline-lane-label";
    label.textContent = subjectId;
    label.title = subjectId;
    const track = document.createElement("div");
    track.className = "timeline-track";
    records.forEach((record, index) => {
      const slot = document.createElement("span");
      if (record.subject_id === subjectId) {
        const marker = document.createElement("button");
        marker.type = "button";
        marker.className = "timeline-event";
        marker.dataset.index = String(index);
        marker.dataset.subjectKind = record.subject_kind;
        marker.dataset.terminal = String(Boolean(record.terminal_cause));
        marker.textContent = String(record.sequence ?? index);
        marker.title = explainTimelineRecord(record);
        marker.setAttribute(
          "aria-label",
          `Select event ${record.sequence ?? index}: ${record.event_kind} for ${record.subject_id}`,
        );
        marker.onclick = () => selectTimelineRecord(index);
        slot.append(marker);
      }
      track.append(slot);
    });
    lane.append(label, track);
    timelineLanes.append(lane);
  }
  records.forEach((record, index) => {
    const row = document.createElement("tr");
    row.dataset.index = String(index);
    const pressure = record.pressure
      ? `${record.pressure}; ${record.occupancy_items} items / ${record.occupancy_bytes} bytes`
      : "—";
    for (const value of [
      record.sequence ?? "—",
      record.tick ?? "before execution",
      `${record.subject_kind}: ${record.subject_id}`,
      record.event_detail
        ? `${record.event_kind}: ${record.event_detail}`
        : record.event_kind,
      pressure,
      record.terminal_cause || "—",
    ]) {
      const cell = document.createElement("td");
      cell.textContent = String(value);
      row.append(cell);
    }
    timelineTableBody.append(row);
  });
  selectTimelineRecord(records.length - 1);
}

function renderExactResultTimeline(value) {
  const values = document.querySelector("#timeline-values");
  if (value.ok) {
    values.textContent =
      `Exact terminal state: ${value.terminal}\n` +
      `Exact stdout: ${JSON.stringify(value.stdout || "")}\n` +
      `Exact stderr: ${JSON.stringify(value.stderr || "")}`;
  } else {
    values.textContent =
      `Exact run rejection: ${value.code || "unknown"}\n${value.diagnostic || ""}`;
  }
  if (Array.isArray(value.evidence) && value.evidence.length > 0) {
    renderTimeline(value.evidence);
    return;
  }
  if (!value.ok) {
    renderTimeline([{
      source: "exact-run-result",
      sequence: "—",
      tick: null,
      subject_kind: "run-result",
      subject_id: value.code || "rejected",
      event_kind: "run-rejected",
      event_detail: value.diagnostic || value.code || "unknown rejection",
      terminal_cause: "rejected",
    }]);
    return;
  }
  renderTimeline([]);
}

function configureExecutionStory() {
  stopTimelinePlayback();
  renderTimeline([]);
  document.querySelector("#timeline-values").textContent =
    "No exact run values yet.";
  const library = current.library;
  executionStory.hidden = !library;
  if (!library) return;
  document.querySelector("#library-summary").textContent = library.summary;
  document.querySelector("#library-what").textContent = library.what;
  document.querySelector("#library-when").textContent = library.when;
  document.querySelector("#library-wrong").textContent = library.wrong;
  const contractList = document.querySelector("#library-contracts");
  contractList.replaceChildren();
  for (const contract of library.contracts) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "btn small";
    button.textContent = contract.id;
    button.onclick = () => {
      selectNode(contract.instance);
      result.textContent =
        `${contract.id}: selected ${contract.instance} in the authoritative Patchbay projection.`;
    };
    contractList.append(button);
  }
  const docs = document.querySelector("#library-docs");
  docs.replaceChildren();
  for (const path of library.docs) {
    const item = document.createElement("li");
    const link = document.createElement("a");
    link.href = path;
    link.textContent = path.split("/").at(-1);
    item.append(link);
    docs.append(item);
  }
  scenarioSelect.replaceChildren();
  for (const scenario of library.scenarios) {
    const option = document.createElement("option");
    option.value = scenario.id;
    option.textContent = scenario.title;
    scenarioSelect.append(option);
  }
}

function renderPlan(projection = null) {
  document.querySelector("#plan").textContent = projection
    ? JSON.stringify(projection, null, 2)
    : "No Rust-resolved plan for this source yet.";
}

function renderRustProjection(projection) {
  if (!projection?.source || !projection.semantic || !projection.plan ||
      !projection.presentation || !projection.run || !Array.isArray(projection.evidence)) {
    throw new Error("CND-PBY-009: incomplete Rust Patchbay projection");
  }
  renderPlan(projection);
  patchbayView = projection;
  updateCytoscapeGraph();
  evidence.splice(0, evidence.length, ...projection.evidence);
  document.querySelector("#evidence").textContent = JSON.stringify(evidence, null, 2);
}

function stopActive(cause, message) {
  runEpoch += 1;
  stopTimelinePlayback();
  if (activeAdapter) {
    activeAdapter.terminate(cause);
    activeAdapter = null;
  }
  runButton.disabled = current?.runnability?.state !== "runnable";
  stopButton.disabled = true;
  consoleBadge.textContent = "Ready";
  consoleBadge.className = "badge status-badge idle";
  if (message) result.textContent = message;
}

function show(lesson) {
  stopActive("lesson-changed");
  current = lesson;
  document.querySelector("#title").textContent = lesson.title;
  document.querySelector("#goal").textContent = lesson.objective || lesson.title;
  document.querySelector("#prose").textContent = lesson.prose || "";
  const availability = lesson.runnability;
  if (!availability) {
    throw new Error(`missing runnability declaration for ${lesson.id}`);
  }
  runnabilityState.textContent =
    `${availability.state} · ${availability.profile}`;
  runnabilityState.dataset.state = availability.state;
  document.querySelector("#execution-note").textContent = availability.state === "runnable"
    ? `${lesson.profile || availability.profile}: exact ${browserPlan.placement} placement on ${hostReport.hostId}.`
    : `${availability.code}: ${availability.reason}`;
  document.querySelector("#command").textContent =
    (lesson.commands ?? [lesson.command || "conduct inspect"]).join("  ·  ");

  // Active state highlighting in nav
  document.querySelectorAll(".nav-list button").forEach((btn) => {
    btn.classList.toggle("active", btn.dataset.id === lesson.id);
  });

  const draft = localStorage.getItem(draftKey(lesson.id));
  source.value = draft ?? lesson.source;
  syncSourceHighlight();
  const parsedDraft = JSON.parse(parse_panel(source.value));
  acceptedSource = parsedDraft.ok ? source.value : lesson.source;
  selectedNode = null;
  positions = {};
  topologyView = "logical";
  undoResetButton.disabled = localStorage.getItem(recoveryKey(lesson.id)) === null;
  document.querySelector("#topology-inspector").open =
    lesson.presentation?.layout === "logical-expanded";
  evidence.length = 0;
  recordEvidence({ kind: "lesson-selected", lesson: lesson.id });
  configureExecutionStory();
  renderPlan();
  openPatchbaySession();
  check();
  runButton.disabled = availability.state !== "runnable";
}

function selectNode(node) {
  selectedNode = node;
  selectedNodeLabel.textContent = `Selected: ${node}`;
  moveLeftBtn.disabled = false;
  moveRightBtn.disabled = false;

  const start = source.value.indexOf(`node ${node} `);
  if (start >= 0) {
    source.focus();
    source.setSelectionRange(start, start + node.length + 5);
  }
  if (patchbayRenderer) {
    patchbayRenderer.selectNode(node);
  }
}

function check() {
  const parsed = JSON.parse(parse_panel(source.value));
  const availability = current.runnability;
  if (!parsed.ok) {
    result.textContent = parsed.diagnostic;
  } else if (availability.state === "runnable") {
    result.textContent =
      `Valid runnable panel: ${parsed.nodes} nodes, ${parsed.cords} cords.`;
  } else {
    const resolved = JSON.parse(explain_panel(source.value));
    const diagnostic = resolved.ok
      ? `${availability.code}: declared ${availability.state}; execution remains disabled`
      : resolved.diagnostic;
    const checkComplete = current.validation?.kind === "diagnostic"
      && diagnostic.includes(current.validation.value);
    result.textContent = checkComplete
      ? `✓ Lesson check complete (not execution evidence).\n${diagnostic}`
      : `${availability.code}: ${availability.reason}\n${diagnostic}`;
    if (checkComplete) {
      recordEvidence({
        kind: "lesson-check-completed",
        lesson: current.id,
        executionEvidence: false,
      });
    }
  }
  updateCytoscapeGraph();
  renderTopology();
}

function renderTopology() {
  const explanation = JSON.parse(explain_panel(source.value));
  document.querySelector("#logical-view").classList.toggle("active", topologyView === "logical");
  document.querySelector("#expanded-view").classList.toggle("active", topologyView === "expanded");
  document.querySelector("#topology").textContent = explanation.ok
    ? explanation[topologyView]
    : explanation.diagnostic;
}

// Populate Lessons Nav
for (const lesson of lessons.lessons) {
  const button = document.createElement("button");
  button.textContent = lesson.title;
  button.dataset.id = lesson.id;
  button.onclick = () => show(lesson);
  const item = document.createElement("li");
  item.append(button);
  document.querySelector("#lessons").append(item);
}

// Populate Reference Panels Nav
for (const refPanel of referencePanels) {
  const button = document.createElement("button");
  button.textContent = refPanel.title;
  button.dataset.id = refPanel.id;
  button.onclick = () => show(refPanel);
  const item = document.createElement("li");
  item.append(button);
  document.querySelector("#reference-panels").append(item);
}

source.addEventListener("input", () => {
  localStorage.setItem(draftKey(current.id), source.value);
  applyPatchbayOperations([{ ReplaceSource: { source: source.value } }]);
  check();
});

scenarioSelect.addEventListener("change", () => {
  stopActive("scenario-changed");
  const scenario = activeScenario();
  if (!scenario) return;
  source.value = scenario.source;
  syncSourceHighlight();
  localStorage.removeItem(draftKey(current.id));
  acceptedSource = scenario.source;
  selectedNode = null;
  positions = {};
  evidence.length = 0;
  recordEvidence({ kind: "scenario-selected", scenario: scenario.id });
  renderPlan();
  renderTimeline([]);
  openPatchbaySession();
  check();
  result.textContent = `${scenario.explanation}\nReady for an exact deterministic run.`;
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
  const currentPos = positions[selectedNode]
    || { x: 100, y: 80 };
  const newX = currentPos.x + delta;
  const newY = currentPos.y;
  const transaction = applyPatchbayOperations([{
    MoveNode: {
      node_id: selectedNode,
      position: { x: newX, y: newY }
    }
  }]);
  if (!transaction.ok) {
    return;
  }
  result.textContent =
    `Presentation moved; semantic hash remains ${transaction.result.semantic.source_semantic_hash}.`;
}

moveLeftBtn.onclick = () => moveSelected(-20);
moveRightBtn.onclick = () => moveSelected(20);

async function run() {
  if (current.runnability?.state !== "runnable") {
    result.textContent =
      `${current.runnability.code}: ${current.runnability.reason}`;
    return;
  }
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
  consoleBadge.textContent = "Running";
  consoleBadge.className = "badge status-badge running";
  result.textContent = "Executing graph in browser placement worker…";

  try {
    const configured = await adapter.request("configure", {
      wasmUrl: wasmArtifact.url.href,
      wasmSha256: wasmArtifact.artifact.sha256,
    });
    if (!configured.ok) throw new Error(configured.code);
    const operation = activeScenario()?.execution === "cancel-before-first-step"
      ? "cancel"
      : "run";
    const executed = await adapter.request(operation, { source: source.value });
    if (epoch !== runEpoch) return;
    if (!executed.ok) throw new Error(executed.code);
    const value = executed.value;
    if (value.ok) renderRustProjection(value.patchbay);
    renderExactResultTimeline(value);
    const counts = Number.isInteger(value.completed_nodes)
      ? `\nEvidence: ${value.completed_nodes} nodes, ${value.cords_conducted} cords conducted.`
      : "";
    const visibleResult = value.ok
      ? `${value.stdout || `Run completed: ${value.terminal}.`}${counts}`
      : value.diagnostic;

    const validation = activeScenario()?.validation || current.validation;
    const lessonComplete = validation?.kind === "stdout"
      ? value.ok && value.stdout === validation.value
      : validation?.kind === "terminal"
        ? value.ok && value.terminal === validation.value
        : validation?.kind === "diagnostic"
          ? (!value.ok && value.diagnostic.includes(validation.value))
            || (value.stderr || "").includes(validation.value)
          : value.ok;

    result.textContent = lessonComplete
      ? `✓ Lesson complete!\n${visibleResult}`
      : visibleResult;

    recordEvidence({
      kind: lessonComplete ? "lesson-completed" : (value.ok ? "run-completed" : "run-rejected"),
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
      runButton.disabled = current.runnability?.state !== "runnable";
      stopButton.disabled = true;
      consoleBadge.textContent = "Idle";
      consoleBadge.className = "badge status-badge idle";
    }
  }
}

runButton.onclick = run;
stopButton.onclick = () => stopActive(
  "learner-cancelled",
  "Run cancelled; exact worker placement is terminal.",
);

document.addEventListener("keydown", (event) => {
  const isRunShortcut = event.shiftKey
    && event.key === "Enter"
    && !event.altKey
    && !event.ctrlKey
    && !event.metaKey;
  if (!isRunShortcut || event.repeat || event.isComposing) return;

  event.preventDefault();
  if (!runButton.disabled) void run();
});

document.querySelector("#reset").onclick = () => {
  stopActive("reset");
  localStorage.setItem(recoveryKey(current.id), source.value);
  source.value = authoredSource();
  syncSourceHighlight();
  acceptedSource = source.value;
  selectedNode = null;
  positions = {};
  selectedNodeLabel.textContent = "No node selected";
  moveLeftBtn.disabled = true;
  moveRightBtn.disabled = true;
  localStorage.removeItem(draftKey(current.id));
  undoResetButton.disabled = false;
  openPatchbaySession();
  check();
};

timelinePosition.addEventListener("input", () => {
  stopTimelinePlayback();
  selectTimelineRecord(Number(timelinePosition.value));
});

document.querySelector("#timeline-play").onclick = () => {
  if (timelineRecords.length === 0 || timelineTimer !== null) return;
  if (timelineCursor >= timelineRecords.length - 1) selectTimelineRecord(0);
  timelineTimer = setInterval(() => {
    if (timelineCursor >= timelineRecords.length - 1) {
      stopTimelinePlayback();
      return;
    }
    selectTimelineRecord(timelineCursor + 1);
  }, 650);
};
document.querySelector("#timeline-pause").onclick = stopTimelinePlayback;
document.querySelector("#timeline-step").onclick = () => {
  stopTimelinePlayback();
  if (timelineRecords.length > 0) selectTimelineRecord(timelineCursor + 1);
};
document.querySelector("#timeline-reset").onclick = () => {
  stopTimelinePlayback();
  if (timelineRecords.length > 0) selectTimelineRecord(0);
};
document.querySelector("#timeline-replay").onclick = () => {
  stopTimelinePlayback();
  if (timelineRecords.length === 0) return;
  selectTimelineRecord(0);
  document.querySelector("#timeline-play").click();
};
executionStory.addEventListener("keydown", (event) => {
  if (["INPUT", "SELECT", "BUTTON"].includes(event.target.tagName)) return;
  if (event.key === " ") {
    event.preventDefault();
    document.querySelector("#timeline-play").click();
  } else if (event.key === "ArrowRight") {
    event.preventDefault();
    document.querySelector("#timeline-step").click();
  }
});

undoResetButton.onclick = () => {
  const recovered = localStorage.getItem(recoveryKey(current.id));
  if (recovered === null) return;
  source.value = recovered;
  syncSourceHighlight();
  localStorage.setItem(draftKey(current.id), recovered);
  localStorage.removeItem(recoveryKey(current.id));
  undoResetButton.disabled = true;
  const parsed = JSON.parse(parse_panel(recovered));
  acceptedSource = parsed.ok ? recovered : current.source;
  openPatchbaySession();
  check();
};

document.querySelector("#download").onclick = () => {
  const link = document.createElement("a");
  link.href = URL.createObjectURL(new Blob([source.value], { type: "text/plain" }));
  link.download = `${current.id}.panel`;
  link.click();
  URL.revokeObjectURL(link.href);
};

const pageParameters = new URLSearchParams(location.search);
const requestedLesson = pageParameters.get("lesson");
if (requestedLesson) {
  current = lessons.lessons.find((lesson) => lesson.id === requestedLesson)
    || referencePanels.find((panel) => panel.id === requestedLesson)
    || current;
}
show(current);
const requestedScenario = pageParameters.get("scenario");
if (requestedScenario && current.library &&
    current.library.scenarios.some((scenario) => scenario.id === requestedScenario)) {
  scenarioSelect.value = requestedScenario;
  scenarioSelect.dispatchEvent(new Event("change"));
}
if (pageParameters.has("autorun")) await run();
