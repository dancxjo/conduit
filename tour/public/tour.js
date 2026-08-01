import "./patchbay-components.js";
import init, {
  explain_panel,
  panel_language_metadata,
  panel_source_metadata,
  parse_panel,
  patchbay_apply_transaction,
  patchbay_open_session,
} from "./conduit_web.js";
import { PatchbayReactFlowRenderer } from "./patchbay-renderer.js";
import { patchbayFeatures } from "./patchbay-features.js";
import { autoArrangeOperations } from "./patchbay-layout.js";
import { PatchbayWorkspaceController } from "./patchbay-workspace.js";
import {
  attachPanelSourceHighlighting,
  configurePanelLanguage,
  configurePanelSourceMetadata,
} from "./panel-highlighter.js";

const source = document.querySelector("#source");
const syncSourceHighlight = attachPanelSourceHighlighting(source);
const result = document.querySelector("#result");
const runButton = document.querySelector("#run");
const stopButton = document.querySelector("#stop");
const undoResetButton = document.querySelector("#undo-reset");
const arrangeButton = document.querySelector("#arrange");
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
const watchValue = document.querySelector("#watch-value");
const watchAccounting = document.querySelector("#watch-accounting");
const watchToggle = document.querySelector("#watch-toggle");
const freezeDisplay = document.querySelector("#freeze-display");
const displayFreezeStatus = document.querySelector("#display-freeze-status");
const watchObservationLead = document.querySelector("#watch-observation-lead");
const liveFlowStatus = document.querySelector("#live-flow-status");
const liveFlowTableBody = document.querySelector("#live-flow-table tbody");
const workspace = document.querySelector("#workspace");
const workspaceKicker = document.querySelector("#workspace-kicker");
const bookChapter = document.querySelector("#book-chapter");
const bookKicker = document.querySelector("#book-kicker");
const bookChapterTitle = document.querySelector("#book-chapter-title");
const bookDeck = document.querySelector("#book-deck");
const bookAnchor = document.querySelector("#book-anchor");
const bookSections = document.querySelector("#book-sections");
const originScattered = document.querySelector("#origin-scattered");
const originConduit = document.querySelector("#origin-conduit");
const originComparison = document.querySelector("#origin-comparison");
const bookRepresentativeNote = document.querySelector("#book-representative-note");
const bookReferences = document.querySelector("#book-references");
const originContinue = document.querySelector("#origin-continue");

const lessons = await (await fetch("../lessons/current.json", { cache: "no-store" })).json();
const browserPlan = await (await fetch("./browser-plan.json", { cache: "no-store" })).json();
const referenceManifest = await (
  await fetch("../reference-panels/current.json", { cache: "no-store" })
).json();
if (referenceManifest.schema !== "conduit.tour-reference-panels") {
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

if (browserPlan.schema !== "conduit.tour-browser-plan") {
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
configurePanelLanguage(JSON.parse(panel_language_metadata()));
configurePanelSourceMetadata(panel_source_metadata);
syncSourceHighlight();

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
    passportIdentity: "conduit/tour-browser-passport",
    statusObservation: {
      realmId: "conduit/tour-static-realm",
      entityId: "conduit/tour-browser-workload",
      passportIdentity: "conduit/tour-browser-passport",
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

let current = lessons.lessons.find((lesson) => lesson.id === "book.origin-hidden-program")
  || lessons.lessons[0];
let acceptedSource = "";
let selectedNode = null;
let selectedCord = null;
let positions = {};
let patchbaySessionId = "";
let patchbaySourceRevision = 0;
let patchbayPresentationRevision = 0;
let patchbayView = null;
let activeAdapter = null;
let activeWorkerSessionId = null;
let activeWorkerRunIdentity = null;
let runEpoch = 0;
let liveWakeTimer = null;
let activeWatchControl = null;
let activeRunProjection = null;
let displayIsFrozen = false;
let deferredLivePresentation = null;
let deferredLiveDeltaCount = 0;
let liveEvidenceSequence = -1;
let liveFlowRows = [];
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
const LIVE_WATCH_PRESENTATION_INTERVAL_MS = 750;
const MAXIMUM_LIVE_FLOW_ROWS = 12;

function clearLiveWakeTimer() {
  if (liveWakeTimer !== null) {
    clearTimeout(liveWakeTimer);
    liveWakeTimer = null;
  }
}

function resetWatchPresentation(message = "No Watch is attached.") {
  disableWatchControl();
  watchValue.textContent = message;
  watchAccounting.textContent = "No Watch accounting yet.";
  activeRunProjection = null;
  displayIsFrozen = false;
  deferredLivePresentation = null;
  deferredLiveDeltaCount = 0;
  liveEvidenceSequence = -1;
  liveFlowRows = [];
  freezeDisplay.disabled = true;
  freezeDisplay.setAttribute("aria-pressed", "false");
  freezeDisplay.textContent = "Freeze Display (F)";
  displayFreezeStatus.textContent = "Display follows authoritative live deltas.";
  liveFlowStatus.textContent = "No authoritative live-flow delta yet.";
  liveFlowTableBody.replaceChildren();
}

function disableWatchControl() {
  activeWatchControl = null;
  watchToggle.disabled = true;
  watchToggle.textContent = "Attach Watch (W)";
  watchToggle.setAttribute("aria-pressed", "false");
  watchObservationLead.dataset.attached = "false";
  watchObservationLead.textContent =
    "Observation lead detached. This dashed lead is presentation only and is never a graph cord.";
  renderStructuredTopology();
}

function setWatchControl(control) {
  activeWatchControl = control;
  watchToggle.disabled = false;
  watchToggle.textContent = control.attached ? "Remove Watch (W)" : "Attach Watch (W)";
  watchToggle.setAttribute("aria-pressed", String(control.attached));
  watchObservationLead.dataset.attached = String(control.attached);
  watchObservationLead.textContent = control.attached
    ? `Presentation-only observation lead attached to ${control.subjectLabel || control.watchId}; it cannot carry demand or pressure.`
    : `Observation lead detached from ${control.subjectLabel || control.watchId}; the exact dataflow continues.`;
  renderStructuredTopology();
}

function setLivePresentationActive() {
  freezeDisplay.disabled = false;
  displayFreezeStatus.textContent = displayIsFrozen
    ? "Display is frozen; the exact executor and bounded observation cursor continue."
    : "Display follows authoritative live deltas.";
}

function authoritativeEvidenceDelta(records) {
  const delta = (records || []).filter((record) =>
    Number.isSafeInteger(record.sequence) && record.sequence > liveEvidenceSequence
  );
  for (const record of delta) {
    liveEvidenceSequence = Math.max(liveEvidenceSequence, record.sequence);
  }
  return delta;
}

function renderLiveFlowRows(projection, delta, watchRecord, runtime = projection) {
  if (delta.length > 0) {
    liveFlowRows.push(...delta);
    if (liveFlowRows.length > MAXIMUM_LIVE_FLOW_ROWS) {
      liveFlowRows.splice(0, liveFlowRows.length - MAXIMUM_LIVE_FLOW_ROWS);
    }
  }
  liveFlowTableBody.replaceChildren();
  for (const record of liveFlowRows) {
    const row = document.createElement("tr");
    row.dataset.sequence = String(record.sequence);
    const pressure = record.pressure
      ? `${record.pressure}; ${record.occupancy_items} items / ${record.occupancy_bytes} bytes`
      : "—";
    for (const value of [
      record.sequence,
      record.tick,
      `${record.subject_kind}: ${record.subject_id}`,
      record.event_detail ? `${record.event_kind}: ${record.event_detail}` : record.event_kind,
      pressure,
    ]) {
      const cell = document.createElement("td");
      cell.textContent = String(value);
      row.append(cell);
    }
    liveFlowTableBody.append(row);
  }
  const cordDeltas = delta.filter((record) => record.subject_kind === "cord");
  const latest = cordDeltas.at(-1) || delta.at(-1);
  const dropped = runtime.evidence_store?.dropped_events ?? 0;
  liveFlowStatus.textContent = latest
    ? `Batch rate: ${delta.length} authoritative event${delta.length === 1 ? "" : "s"} per bounded presentation update; ` +
      `${cordDeltas.length} cord event${cordDeltas.length === 1 ? "" : "s"}. ` +
      `Latest: ${latest.subject_kind} ${latest.subject_id}, ${latest.event_kind}, ` +
      `${latest.occupancy_items} items / ${latest.occupancy_bytes} bytes. ` +
      `Rolling evidence gaps: ${dropped}. Watch cursor: ${watchRecord?.cursor ?? "detached"}.`
    : `No new authoritative event in this update. Rolling evidence gaps: ${dropped}.`;
  patchbayRenderer?.presentLiveEvidence(patchbayView || projection, delta, watchRecord);
}

function applyLiveRunProjection(projection) {
  if (!patchbayView) return;
  patchbayView = {
    ...patchbayView,
    run: projection.run,
    evidence: projection.evidence,
  };
  evidence.splice(0, evidence.length, ...projection.evidence);
  document.querySelector("#evidence").textContent = JSON.stringify(evidence, null, 2);
  patchbayRenderer?.updateRunPresentation(patchbayView);
}

function presentContinuousUpdate(presentation) {
  activeRunProjection = presentation.projection;
  if (displayIsFrozen) {
    deferredLivePresentation = presentation;
    deferredLiveDeltaCount += presentation.delta.length;
    displayFreezeStatus.textContent =
      `Display frozen; ${deferredLiveDeltaCount} authoritative delta${deferredLiveDeltaCount === 1 ? "" : "s"} deferred while the exact executor remains live.`;
    return;
  }
  applyLiveRunProjection(presentation.projection);
  if (presentation.watched) {
    renderLatestWatch(presentation.watched, presentation.result);
  } else if (presentation.control) {
    renderDetachedWatch(presentation.result, presentation.control);
  }
  renderExactResultTimeline(presentation.result);
  renderLiveFlowRows(
    presentation.projection,
    presentation.delta,
    presentation.watched?.records?.at(-1),
    presentation.result,
  );
  result.textContent = presentation.message;
}

function toggleDisplayFreeze() {
  if (freezeDisplay.disabled) return;
  displayIsFrozen = !displayIsFrozen;
  freezeDisplay.setAttribute("aria-pressed", String(displayIsFrozen));
  freezeDisplay.textContent = displayIsFrozen ? "Resume Display (F)" : "Freeze Display (F)";
  if (displayIsFrozen) {
    displayFreezeStatus.textContent =
      "Display frozen; the exact executor and bounded observation cursor continue.";
    return;
  }
  const deferred = deferredLivePresentation;
  deferredLivePresentation = null;
  deferredLiveDeltaCount = 0;
  displayFreezeStatus.textContent = "Display resumed at the latest bounded authoritative state.";
  if (deferred) presentContinuousUpdate(deferred);
}

async function toggleWatch() {
  const control = activeWatchControl;
  if (!control || watchToggle.disabled) return;
  const wasAttached = control.attached;
  const nextAttached = !wasAttached;
  control.pending = true;
  // Stop issuing observation reads before the detach request reaches the
  // worker. The ticker pump remains independent and continues below.
  if (wasAttached) control.attached = false;
  watchToggle.disabled = true;
  watchToggle.setAttribute("aria-pressed", String(control.attached));
  const operation = wasAttached
    ? "patchbay-detach-exact-watch"
    : "patchbay-attach-exact-watch";
  const response = await control.adapter.request(operation, {
    sessionId: control.sessionId,
    ...control.runIdentity,
    watchId: control.watchId,
  });
  if (activeWatchControl !== control) return;
  if (!response.ok || !response.value?.ok) {
    control.attached = wasAttached;
    control.pending = false;
    setWatchControl(control);
    result.textContent = response.value?.diagnostic || response.code || "Watch control failed";
    return;
  }
  control.attached = nextAttached;
  control.pending = false;
  setWatchControl(control);
  if (!control.attached) {
    watchValue.textContent = "Watch detached; the exact ticker continues without observation pressure.";
  }
  recordEvidence({
    kind: control.attached ? "watch-attached" : "watch-detached",
    lesson: current.id,
    watch_id: control.watchId,
  });
}

function renderLatestWatch(batch, run) {
  const record = batch?.records?.at(-1);
  const text = record?.material?.kind === "preview"
    ? record.material.text
    : null;
  watchValue.textContent = text ?? (record?.material?.kind || "No value observed yet.");
  watchAccounting.textContent = JSON.stringify({
    run_id: run.run_id,
    plan_identity: run.plan_identity,
    source_semantic_hash: run.source_semantic_hash,
    state: run.state,
    next_timer_deadline: run.next_timer_deadline,
    watch_id: batch.watch_id,
    retention: "latest",
    cursor: batch.next_cursor,
    representation: record?.representation,
    sensitivity: record?.sensitivity,
    timestamp: {
      tick: record?.tick,
      time_basis: record?.time_basis,
      uncertainty_ticks: record?.clock_uncertainty_ticks,
      value_timestamps: record?.value_timestamps || [],
    },
    renderer: record?.material?.renderer || { status: record?.material?.kind || "absent" },
    material_kind: record?.material?.kind || "absent",
    truncated: Boolean(record?.truncated),
    recent_history: batch.records.slice(-1).map((item) => ({
      cursor: item.cursor,
      tick: item.tick,
      material_kind: item.material?.kind,
      truncated: Boolean(item.truncated),
    })),
    gap_before: record?.gap_before ?? 0,
    value_storage: run.value_storage,
    evidence_store: run.evidence_store,
  }, null, 2);
}

function renderDetachedWatch(run, control) {
  watchAccounting.textContent = JSON.stringify({
    run_id: run.run_id,
    plan_identity: run.plan_identity,
    source_semantic_hash: run.source_semantic_hash,
    state: run.state,
    next_timer_deadline: run.next_timer_deadline,
    watch_id: control.watchId,
    attached: false,
    retention: "latest",
    cursor: control.cursor,
    representation: { id: "std/text" },
    sensitivity: "public",
    value_storage: run.value_storage,
    evidence_store: run.evidence_store,
  }, null, 2);
}

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
let workspaceController = null;
const cyContainer = document.getElementById("cy");
document.querySelector(".node-controls").hidden =
  !patchbayFeatures.legacyLinePlacement;
if (cyContainer) {
  patchbayRenderer = new PatchbayReactFlowRenderer(cyContainer, {
    onTransaction: (operation, options) => applyPatchbayOperations(
      Array.isArray(operation) ? operation : [operation],
      options,
    ),
    onNodeSelect: (nodeId) => {
      selectNode(nodeId);
    },
    onCordSelect: (cordId) => {
      selectCord(cordId);
    },
    onPortSelect: (nodeId, port) => {
      selectPort(nodeId, port);
    },
    onCordWatch: (cordId) => toggleWatchForSubject({ cordId }),
    onPortWatch: (nodeId, port) => toggleWatchForSubject({ nodeId, port }),
    onSelectionClear: () => {
      clearTopologySelection();
    },
    onNotification: (msg) => {
      result.textContent = msg;
    }
  });
  patchbayRenderer.init();
  workspaceController = new PatchbayWorkspaceController({
    canvasCard: document.querySelector(".canvas-card"),
    sourceCard: document.querySelector(".source-card"),
    actionBar: document.querySelector(".primary-actions"),
    consoleCard: document.querySelector(".console-card"),
    source,
    renderer: patchbayRenderer,
    getContext: () => ({
      documentName: current?.id || "untitled.panel",
      revision: patchbayView?.source?.revision || 0,
      identity: patchbayView?.source?.identity,
      dirty: Boolean(current && source.value !== current.source),
      diagnostics: patchbayView?.diagnostics?.length || 0,
    }),
  });
  workspaceController.init();
}

function updateCytoscapeGraph() {
  syncDiagnosticSourceHighlights();
  if (patchbayRenderer) {
    patchbayRenderer.setViewModel(patchbayView, current.id, topologyView);
  }
  renderStructuredTopology();
  renderDiagnosticConsole();
  workspaceController?.updateStatus();
}

function syncDiagnosticSourceHighlights() {
  const projectionIsCurrent = patchbayView &&
    patchbayView.source.revision === patchbaySourceRevision &&
    patchbayView.source.source === source.value;
  const ranges = projectionIsCurrent
    ? (patchbayView.diagnostics || [])
      .map((diagnostic) => diagnostic.primary_range)
      .filter((range) => range?.source_revision === patchbaySourceRevision)
    : [];
  source.setSourceDiagnosticRanges?.(ranges);
}

function renderDiagnosticConsole() {
  const consoleList = document.querySelector("#diagnostic-console");
  if (!consoleList) return;
  consoleList.replaceChildren();
  for (const diagnostic of patchbayView?.diagnostics || []) {
    const item = document.createElement("li");
    item.dataset.diagnosticId = diagnostic.id;
    item.dataset.diagnosticState = diagnostic.state;
    const button = document.createElement("button");
    button.type = "button";
    button.className = `diagnostic-console-button diagnostic-${diagnostic.state}`;
    button.textContent = `${diagnostic.code}: ${diagnostic.explanation}`;
    button.setAttribute(
      "aria-label",
      `${diagnostic.severity} ${diagnostic.code}; ${diagnostic.explanation}`,
    );
    button.onclick = () => selectDiagnostic(diagnostic);
    item.append(button);
    consoleList.append(item);
  }
}

function watchAdmissionForSubject({ cordId = null, nodeId = null, port = null }) {
  const admissions = activeRunProjection?.plan?.watch_admissions ||
    patchbayView?.plan?.watch_admissions || [];
  const direct = admissions.find((admission) =>
    (cordId && admission.subject_kind === "cord" && admission.cord === cordId) ||
    (nodeId && port && admission.subject_kind === "node-port" &&
      admission.node === nodeId && admission.port === port.id)
  );
  if (direct || !nodeId || !port) return direct;
  const connectedCord = (patchbayView?.topology?.cords || []).find((cord) =>
    (cord.from_node === nodeId && cord.from_port === port.id) ||
    (cord.to_node === nodeId && cord.to_port === port.id)
  );
  return connectedCord
    ? admissions.find((admission) =>
        admission.subject_kind === "cord" && admission.cord === connectedCord.id
      )
    : null;
}

function toggleWatchForSubject(subject) {
  const admission = watchAdmissionForSubject(subject);
  if (!admission) {
    result.textContent = "No exact-plan Watch admission exists for this selected subject.";
    return;
  }
  if (activeWatchControl?.watchId !== admission.id) {
    result.textContent =
      `Watch ${admission.id} is admitted but is not the active bounded instrument control.`;
    return;
  }
  void toggleWatch();
}

function watchSubjectButton(admission, label) {
  if (!admission) return null;
  const button = document.createElement("button");
  button.type = "button";
  button.className = "structured-watch-button";
  const attached = activeWatchControl?.watchId === admission.id && activeWatchControl.attached;
  button.textContent = attached ? `Remove Watch from ${label}` : `Watch ${label}`;
  button.setAttribute("aria-label", button.textContent);
  button.onclick = () => toggleWatchForSubject({
    cordId: admission.cord,
    nodeId: admission.node,
    port: admission.port ? { id: admission.port } : null,
  });
  return button;
}

function selectDiagnostic(diagnostic) {
  const cord = diagnostic.targets.find((target) => target.kind === "cord");
  const node = diagnostic.targets.find((target) => target.kind === "node");
  if (cord) {
    selectCord(cord.id);
  } else if (node) {
    selectNode(node.id);
  } else {
    selectSourceRange(
      "diagnostic",
      diagnostic.id,
      diagnostic.primary_range,
    );
  }
  result.textContent =
    `${diagnostic.code}: ${diagnostic.message}\n${diagnostic.explanation}`;
}

function renderStructuredTopology() {
  const portList = document.querySelector("#panel-port-list");
  const connectionList = document.querySelector("#panel-connection-list");
  if (!portList || !connectionList) return;
  portList.replaceChildren();
  connectionList.replaceChildren();
  const nodes = topologyView === "logical"
    ? patchbayView?.topology?.logical_nodes || []
    : patchbayView?.topology?.expanded_nodes || [];
  for (const node of nodes) {
    for (const port of [...node.inputs, ...node.outputs]) {
      const item = document.createElement("li");
      item.dataset.semanticPath = port.semantic_path;
      item.dataset.portDirection =
        port.direction === "input" ? "receiving" : "outgoing";
      const button = document.createElement("button");
      button.type = "button";
      button.className = "structured-topology-button";
      button.textContent =
        `${node.id}: ${port.display_label} — ${port.type_id}; ` +
        `${port.delivery}; ${port.connections}`;
      button.setAttribute(
        "aria-label",
        `${node.id}, ${port.accessible_label}, type ${port.type_id}, ` +
        `${port.delivery}, ${port.connections}`,
      );
      button.onclick = () => selectPort(node.id, port);
      item.append(button);
      const admission = watchAdmissionForSubject({ nodeId: node.id, port });
      const watchButton = watchSubjectButton(admission, `${node.id}.${port.id}`);
      if (watchButton) item.append(watchButton);
      portList.append(item);
    }
  }
  for (const cord of patchbayView?.topology?.cords || []) {
    const diagnostic = (patchbayView?.diagnostics || []).find((candidate) =>
      candidate.targets.some(
        (target) => target.kind === "cord" && target.id === cord.id,
      )
    );
    const item = document.createElement("li");
    item.dataset.fromPortPath = cord.from_port_path;
    item.dataset.toPortPath = cord.to_port_path;
    const button = document.createElement("button");
    button.type = "button";
    button.className = "structured-topology-button";
    const from = `${cord.from_node || "unfinished"}.${cord.from_port || "…"}`;
    const to = `${cord.to_node || "unfinished"}.${cord.to_port || "…"}`;
    button.textContent =
      `${from} > → > ${to} — ${diagnostic?.code || "valid"}; ` +
      `${diagnostic?.explanation || `${cord.value_type}; ${cord.pressure}`}`;
    button.setAttribute(
      "aria-label",
      `${from}, authored source endpoint, to ${to}, authored destination endpoint; ` +
      `${diagnostic?.code || "valid"}; ` +
      `${diagnostic?.explanation || `${cord.value_type}; ${cord.pressure}`}`,
    );
    button.onclick = () => selectCord(cord.id);
    item.append(button);
    const admission = watchAdmissionForSubject({ cordId: cord.id });
    const watchButton = watchSubjectButton(admission, `cord ${cord.id}`);
    if (watchButton) item.append(watchButton);
    connectionList.append(item);
  }
}

function openPatchbaySession() {
  patchbaySessionId = `tour/${current.id}`;
  const opened = JSON.parse(patchbay_open_session(patchbaySessionId, acceptedSource));
  if (!opened.ok) {
    patchbayView = null;
    updateCytoscapeGraph();
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
  autoArrangePatchbay();
  return true;
}

function autoArrangePatchbay() {
  if (!patchbayView) return false;
  const operations = autoArrangeOperations(patchbayView, topologyView);
  for (let offset = 0; offset < operations.length;
    offset += MAXIMUM_LAYOUT_OPERATIONS_PER_TRANSACTION) {
    const arranged = applyPatchbayOperations(
      operations.slice(
        offset,
        offset + MAXIMUM_LAYOUT_OPERATIONS_PER_TRANSACTION,
      ),
      { skipRender: true },
    );
    if (!arranged.ok) return false;
  }
  updateCytoscapeGraph();
  if (cyContainer) cyContainer.dataset.layoutAlgorithm = "layered";
  result.textContent = operations.length === 0
    ? "No topology items were available to arrange."
    : `Arranged ${operations.length} topology item${operations.length === 1 ? "" : "s"} by dataflow.`;
  return true;
}

function applyPatchbayOperations(operations, options = {}) {
  const request = {
    protocol_version: 0,
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
  patchbayView = activeRunProjection?.run
    ? {
        ...transaction.view,
        run: activeRunProjection.run,
        evidence: activeRunProjection.evidence,
      }
    : transaction.view;
  patchbaySourceRevision = transaction.result.source.revision;
  patchbayPresentationRevision = transaction.result.presentation.revision;
  acceptedSource = transaction.result.source.source;
  if (options.preserveFaceplateFocus || options.syncSource) {
    source.value = acceptedSource;
    syncSourceHighlight();
  }
  positions = transaction.result.presentation.node_positions;
  syncDiagnosticSourceHighlights();
  runButton.disabled =
    current?.runnability?.state !== "runnable" || !patchbayView.plan || Boolean(activeAdapter);
  rememberLayout(current.id, positions, patchbayView);
  if (!options.preserveFaceplateFocus && !options.skipRender) {
    updateCytoscapeGraph();
  }
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
  if (!scenarioSelect) return null;
  const libraryScenario = current.library?.scenarios?.find(
    (scenario) => scenario.id === scenarioSelect.value,
  );
  if (libraryScenario) return libraryScenario;
  const platformProfile = current.platform?.profiles?.find(
    (profile) => profile.id === scenarioSelect.value,
  );
  if (!platformProfile) return null;
  const outcome = platformProfile.admission === "accepted"
    ? "admitted by the checked contract"
    : `rejected before execution with ${platformProfile.code}`;
  return {
    ...platformProfile,
    source: current.source,
    explanation: `${platformProfile.id}: ${outcome}. The editable representative panel remains real source and reruns independently.`,
  };
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
  if (record.node_id) {
    selectNode(record.node_id);
  } else if (record.cord_id) {
    selectCord(record.cord_id);
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
  if (!executionStory) return;
  const values = document.querySelector("#timeline-values");
  if (value.ok) {
    values.textContent =
      `Exact terminal state: ${value.terminal}\n` +
      `Exact stdout: ${JSON.stringify(value.stdout || "")}\n` +
      `Exact display: ${JSON.stringify(value.display || "")}\n` +
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
  if (!executionStory) return;
  renderTimeline([]);
  document.querySelector("#timeline-values").textContent =
    "No exact run values yet.";
  const story = current.library || current.platform;
  executionStory.hidden = !story;
  if (!story) return;
  const platform = Boolean(current.platform);
  document.querySelector("#story-kind").textContent =
    platform ? "Platform contract lesson" : "Library lesson";
  document.querySelector("#story-selectable-title").textContent =
    platform ? "Checked plan profiles" : "Selectable contracts";
  document.querySelector("#library-summary").textContent =
    story.summary || current.objective;
  document.querySelector("#library-what").textContent = story.what;
  document.querySelector("#library-when").textContent = story.when;
  document.querySelector("#library-wrong").textContent = story.wrong;
  const contractList = document.querySelector("#library-contracts");
  contractList.replaceChildren();
  for (const contract of story.contracts || story.profiles) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "btn small";
    button.textContent = contract.id;
    button.onclick = () => {
      if (platform) {
        scenarioSelect.value = contract.id;
        scenarioSelect.dispatchEvent(new Event("change"));
      } else {
        selectNode(contract.instance);
        result.textContent =
          `${contract.id}: selected ${contract.instance} in the authoritative Patchbay projection.`;
      }
    };
    contractList.append(button);
  }
  const docs = document.querySelector("#library-docs");
  docs.replaceChildren();
  const references = [
    ...(story.docs || []),
    ...(platform ? [story.fixture, story.panel].filter(Boolean) : []),
  ];
  for (const path of references) {
    const item = document.createElement("li");
    const link = document.createElement("a");
    link.href = path;
    link.textContent = path.split("/").at(-1);
    item.append(link);
    docs.append(item);
  }
  scenarioSelect.replaceChildren();
  for (const scenario of story.scenarios || story.profiles) {
    const option = document.createElement("option");
    option.value = scenario.id;
    option.textContent = scenario.title || scenario.id;
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
  if (!patchbayView) {
    throw new Error("CND-PBY-009: run projection has no Patchbay workspace");
  }
  projection = {
    ...projection,
    run: {
      ...projection.run,
      source_revision: activeWorkerRunIdentity?.sourceRevision ?? null,
    },
  };
  activeRunProjection = projection;
  if (projection.source.semantic_hash !== patchbayView.source.semantic_hash) {
    applyLiveRunProjection(projection);
    return;
  }
  const sourceRevision = patchbayView.source.revision;
  const rebaseRange = (item) => ({
    ...item,
    source_range: item.source_range
      ? { ...item.source_range, source_revision: sourceRevision }
      : null,
  });
  const topology = {
    ...projection.topology,
    logical_nodes: projection.topology.logical_nodes.map(rebaseRange),
    expanded_nodes: projection.topology.expanded_nodes.map(rebaseRange),
    cords: projection.topology.cords.map(rebaseRange),
  };
  const survivingNodes = new Set(
    topology.expanded_nodes.map((node) => node.id),
  );
  const retainedPositions = Object.fromEntries(
    Object.entries(patchbayView.presentation.node_positions)
      .filter(([nodeId]) => survivingNodes.has(nodeId)),
  );
  renderPlan(projection);
  patchbayView = {
    ...projection,
    source: patchbayView.source,
    semantic: patchbayView.semantic,
    presentation: {
      ...patchbayView.presentation,
      node_positions: retainedPositions,
    },
    topology,
  };
  positions = retainedPositions;
  updateCytoscapeGraph();
  evidence.splice(0, evidence.length, ...projection.evidence);
  document.querySelector("#evidence").textContent = JSON.stringify(evidence, null, 2);
}

function renderLiveRunProjection(projection) {
  if (!projection?.source || !projection.run || !Array.isArray(projection.evidence) ||
      !patchbayView) {
    throw new Error("CND-PBY-009: incomplete live run projection");
  }
  projection = {
    ...projection,
    run: {
      ...projection.run,
      source_revision: activeWorkerRunIdentity?.sourceRevision ?? null,
    },
  };
  activeRunProjection = projection;
  return authoritativeEvidenceDelta(projection.evidence);
}

async function stopExactSession(cause, message) {
  runEpoch += 1;
  clearLiveWakeTimer();
  stopTimelinePlayback();
  const adapter = activeAdapter;
  const sessionId = activeWorkerSessionId;
  const runIdentity = activeWorkerRunIdentity;
  activeAdapter = null;
  activeWorkerSessionId = null;
  activeWorkerRunIdentity = null;
  disableWatchControl();
  if (adapter && sessionId) {
    try {
      const cancelled = await adapter.request("patchbay-cancel-exact-run", {
        sessionId,
        ...runIdentity,
        disposition: "abort",
      });
      if (cancelled.ok && cancelled.value?.ok) {
        renderRustProjection(cancelled.value.view);
        renderExactResultTimeline(cancelled.value);
        recordEvidence({ kind: "run-cancelled", state: cancelled.value.state });
        await adapter.request("patchbay-dispose-exact-run", {
          sessionId,
          ...runIdentity,
        });
      } else {
        patchbayRenderer?.presentPlacementLoss(
          cancelled.value?.diagnostic || cancelled.code || "exact cancellation failed",
        );
      }
    } finally {
      adapter.terminate(cause);
    }
  } else if (adapter) {
    adapter.terminate(cause);
  }
  activeRunProjection = null;
  deferredLivePresentation = null;
  deferredLiveDeltaCount = 0;
  displayIsFrozen = false;
  freezeDisplay.disabled = true;
  freezeDisplay.setAttribute("aria-pressed", "false");
  freezeDisplay.textContent = "Freeze Display (F)";
  displayFreezeStatus.textContent = "Display follows authoritative live deltas.";
  runButton.disabled = current?.runnability?.state !== "runnable";
  stopButton.disabled = true;
  consoleBadge.textContent = "Ready";
  consoleBadge.className = "badge status-badge idle";
  if (message) result.textContent = message;
}

function renderOriginComparison(origin, mode) {
  const view = origin.comparison?.[mode];
  if (!view) return;

  originScattered.setAttribute("aria-pressed", String(mode === "scattered"));
  originConduit.setAttribute("aria-pressed", String(mode === "conduit"));
  originComparison.replaceChildren();

  const title = document.createElement("h4");
  title.textContent = view.title;
  const summary = document.createElement("p");
  summary.textContent = view.summary;
  const list = document.createElement("ul");
  for (const item of view.items || []) {
    const entry = document.createElement("li");
    entry.textContent = item;
    list.append(entry);
  }
  originComparison.append(title, summary, list);
}

function renderOriginStory(lesson) {
  const origin = lesson.origin;
  const isOrigin = Boolean(origin);
  workspace.classList.toggle("book-origin-active", isOrigin);
  if (!bookChapter || !workspaceKicker) return;
  bookChapter.hidden = !isOrigin;
  workspaceKicker.textContent = isOrigin ? "Conduit, chapter zero" : "Interactive Workspace";
  if (!isOrigin) return;

  bookKicker.textContent = origin.kicker;
  bookChapterTitle.textContent = origin.title;
  bookDeck.textContent = origin.deck;
  bookAnchor.textContent = origin.anchor;
  bookSections.replaceChildren();

  for (const [index, section] of (origin.sections || []).entries()) {
    const article = document.createElement("article");
    article.className = "book-section";
    const marker = document.createElement("p");
    marker.className = "book-section-number";
    marker.textContent = String(index + 1).padStart(2, "0");
    const title = document.createElement("h3");
    title.textContent = section.title;
    const body = document.createElement("p");
    body.textContent = section.body;
    article.append(marker, title, body);
    bookSections.append(article);
  }

  originScattered.textContent = origin.comparison.scattered.label;
  originConduit.textContent = origin.comparison.conduit.label;
  originScattered.onclick = () => renderOriginComparison(origin, "scattered");
  originConduit.onclick = () => renderOriginComparison(origin, "conduit");
  renderOriginComparison(origin, "scattered");

  bookRepresentativeNote.textContent = origin.representative_note;
  bookReferences.replaceChildren();
  for (const reference of origin.references || []) {
    const item = document.createElement("li");
    const link = document.createElement("a");
    link.href = reference.href;
    link.textContent = reference.label;
    item.append(link);
    bookReferences.append(item);
  }

  originContinue.textContent = origin.next_label;
  originContinue.onclick = () => {
    const next = lessons.lessons.find((candidate) => candidate.id === origin.next_lesson);
    if (!next) throw new Error(`missing origin continuation ${origin.next_lesson}`);
    show(next);
  };

  document.querySelectorAll("#workspace details").forEach((details) => {
    details.open = false;
  });
}

function show(lesson) {
  void stopExactSession("lesson-changed");
  resetWatchPresentation();
  current = lesson;
  patchbayView = null;
  patchbaySourceRevision = 0;
  patchbayPresentationRevision = 0;
  positions = {};
  updateCytoscapeGraph();
  workspaceController?.setDocument(lesson.id);
  document.querySelector("#title").textContent = lesson.title;
  document.querySelector("#goal").textContent = lesson.objective || lesson.title;
  document.querySelector("#prose").textContent = lesson.prose || "";
  renderOriginStory(lesson);
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
  source.setSourceDiagnosticRanges?.([]);
  syncSourceHighlight();
  const parsedDraft = JSON.parse(parse_panel(source.value));
  acceptedSource = parsedDraft.ok ? source.value : lesson.source;
  selectedNode = null;
  selectedCord = null;
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
  const projection = [
    ...(patchbayView?.topology?.logical_nodes || []),
    ...(patchbayView?.topology?.expanded_nodes || []),
  ].find((candidate) => candidate.id === node);
  if (!selectSourceRange("node", node, projection?.source_range)) return;
  selectedNode = node;
  selectedCord = null;
  selectedNodeLabel.textContent = `Selected node: ${node}`;
  moveLeftBtn.disabled = false;
  moveRightBtn.disabled = false;
  if (patchbayRenderer) {
    patchbayRenderer.selectNode(node);
  }
}

function selectCord(cordId) {
  const projection = (patchbayView?.topology?.cords || [])
    .find((candidate) => candidate.id === cordId);
  if (!selectSourceRange("cord", cordId, projection?.source_range)) return;
  source.setSourceRelatedRanges?.([
    projection.from_port_range,
    projection.to_port_range,
  ]);
  selectedNode = null;
  selectedCord = cordId;
  moveLeftBtn.disabled = true;
  moveRightBtn.disabled = true;
  const provenance = projection.source_range?.provenance === "authored"
    ? ""
    : " (derived edge; revealing authored owner)";
  selectedNodeLabel.textContent = `Selected cord: ${cordId}${provenance}`;
  patchbayRenderer?.selectCord(cordId);
}

function selectPort(nodeId, port) {
  const cord = (patchbayView?.topology?.cords || []).find((candidate) =>
    port.direction === "input"
      ? candidate.to_port_path === port.semantic_path
      : candidate.from_port_path === port.semantic_path
  );
  if (cord) {
    const range = port.direction === "input"
      ? cord.to_port_range
      : cord.from_port_range;
    const projectedPath = port.direction === "input"
      ? cord.to_port_path
      : cord.from_port_path;
    if (projectedPath !== port.semantic_path ||
        !selectSourceRange("port", port.semantic_path, range)) {
      result.textContent =
        `CND-PBY-STALE: port selection path ${port.semantic_path} ` +
        "does not match its authoritative cord projection.";
      return;
    }
    selectedNode = null;
    selectedCord = cord.id;
    patchbayRenderer?.selectCord(cord.id);
    selectedNodeLabel.textContent =
      `Selected ${port.accessible_label}: ${port.semantic_path}`;
    return;
  }
  const projection = [
    ...(patchbayView?.topology?.logical_nodes || []),
    ...(patchbayView?.topology?.expanded_nodes || []),
  ].find((candidate) => candidate.id === nodeId);
  if (!selectSourceRange("port owner", port.semantic_path, projection?.source_range)) {
    return;
  }
  selectedNode = nodeId;
  selectedCord = null;
  selectedNodeLabel.textContent =
    `Selected ${port.accessible_label}: ${port.semantic_path}`;
}

function selectSourceRange(kind, id, range) {
  if (!range) {
    result.textContent =
      `Selected ${kind} ${id} has no direct authored source range.`;
    return false;
  }
  if (!patchbayView || range.source_revision !== patchbaySourceRevision ||
      patchbayView.source.revision !== patchbaySourceRevision ||
      patchbayView.source.source !== source.value) {
    result.textContent =
      `CND-PBY-STALE: ${kind} selection was rejected because the source projection is stale.`;
    return false;
  }
  source.setSourceRelatedRanges?.([]);
  source.setSourceHighlightRange?.(range.start_utf16, range.end_utf16);
  source.setSelectionRange(range.start_utf16, range.end_utf16);
  const line = source.value.slice(0, range.start_utf16).split("\n").length - 1;
  const computedLineHeight = Number.parseFloat(getComputedStyle(source).lineHeight);
  const lineHeight = Number.isFinite(computedLineHeight) ? computedLineHeight : 20;
  source.scrollTop = Math.max(0, line * lineHeight - source.clientHeight / 3);
  source.syncHighlight?.();
  return true;
}

function clearTopologySelection() {
  selectedNode = null;
  selectedCord = null;
  selectedNodeLabel.textContent = "No topology item selected";
  moveLeftBtn.disabled = true;
  moveRightBtn.disabled = true;
  source.setSourceHighlightRange?.(null, null);
  source.setSourceRelatedRanges?.([]);
  patchbayRenderer?.highlightCordEndpoints(null);
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
  runButton.disabled =
    current?.runnability?.state !== "runnable" || !patchbayView?.plan;
}

function renderTopology() {
  const explanation = JSON.parse(explain_panel(source.value));
  const logicalButton = document.querySelector("#logical-view");
  const expandedButton = document.querySelector("#expanded-view");
  logicalButton.classList.toggle("active", topologyView === "logical");
  logicalButton.setAttribute("aria-pressed", String(topologyView === "logical"));
  expandedButton.classList.toggle("active", topologyView === "expanded");
  expandedButton.setAttribute("aria-pressed", String(topologyView === "expanded"));
  document.querySelector("#topology").textContent = explanation.ok
    ? explanation[topologyView]
    : explanation.diagnostic;
  renderStructuredTopology();
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

scenarioSelect?.addEventListener("change", () => {
  void stopExactSession("scenario-changed");
  const scenario = activeScenario();
  if (!scenario) return;
  patchbayView = null;
  patchbaySourceRevision = 0;
  patchbayPresentationRevision = 0;
  positions = {};
  updateCytoscapeGraph();
  source.value = scenario.source;
  source.setSourceDiagnosticRanges?.([]);
  syncSourceHighlight();
  localStorage.removeItem(draftKey(current.id));
  acceptedSource = scenario.source;
  selectedNode = null;
  selectedCord = null;
  evidence.length = 0;
  recordEvidence({ kind: "scenario-selected", scenario: scenario.id });
  renderPlan();
  renderTimeline([]);
  openPatchbaySession();
  check();
  result.textContent = `${scenario.explanation}\nReady for an exact deterministic run.`;
});

document.querySelector("#check").onclick = check;
if (arrangeButton) {
  arrangeButton.onclick = () => {
    localStorage.removeItem(layoutKey(current.id));
    autoArrangePatchbay();
  };
}
document.querySelector("#logical-view").onclick = () => {
  topologyView = "logical";
  updateCytoscapeGraph();
  renderTopology();
};
document.querySelector("#expanded-view").onclick = () => {
  topologyView = "expanded";
  updateCytoscapeGraph();
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

function scheduleContinuousWatch({
  adapter, sessionId, runIdentity, watchId, epoch, cursor, deadline,
}) {
  if (epoch !== runEpoch || activeAdapter !== adapter || !Number.isSafeInteger(deadline)) return;
  clearLiveWakeTimer();
  liveWakeTimer = setTimeout(async () => {
    liveWakeTimer = null;
    if (epoch !== runEpoch || activeAdapter !== adapter) return;
    try {
      const advanced = await adapter.request("patchbay-advance-exact-run", {
        sessionId,
        ...runIdentity,
        tick: deadline,
      });
      if (!advanced.ok || !advanced.value?.ok) {
        throw new Error(advanced.value?.diagnostic || advanced.code || "timer wake failed");
      }
      const pumped = await adapter.request("patchbay-pump-exact-run", {
        sessionId,
        ...runIdentity,
        quantum: 256,
      });
      if (!pumped.ok || !pumped.value?.ok) {
        throw new Error(pumped.value?.diagnostic || pumped.code || "ticker pump failed");
      }
      if (epoch !== runEpoch || activeAdapter !== adapter) return;
      // The exact topology is immutable for this epoch. Rebuilding React Flow
      // for every value would spend the observation budget on static work and
      // can starve Stop/Watch input on slower browser hosts.
      const liveDelta = renderLiveRunProjection(pumped.value.view);
      const control = activeWatchControl?.watchId === watchId
        ? activeWatchControl
        : null;
      let nextCursor = cursor;
      let watched = null;
      if (control?.attached && !control.pending) {
        watched = await adapter.request("patchbay-read-exact-watch", {
          sessionId,
          ...runIdentity,
          watchId,
          cursor,
          maximumRecords: 1,
        });
        if (!watched.ok || !watched.value?.ok) {
          throw new Error(watched.value?.diagnostic || watched.code || "Watch read failed");
        }
        if (epoch !== runEpoch || activeAdapter !== adapter) return;
        nextCursor = watched.value.next_cursor;
        control.cursor = nextCursor;
      }
      const message =
        `✓ Live exact run remains ${pumped.value.state}.\n` +
        (watched
          ? `Latest public text: ${JSON.stringify(watched.value.records.at(-1)?.material?.text ?? "")}\n`
          : "Watch detached; the ticker continues without observation pressure.\n") +
        `Next admitted timer deadline: ${pumped.value.next_timer_deadline}.`;
      presentContinuousUpdate({
        projection: activeRunProjection,
        result: pumped.value,
        watched: watched?.value || null,
        control,
        delta: liveDelta,
        message,
      });
      recordEvidence({
        kind: watched ? "watch-latest" : "ticker-without-watch",
        lesson: current.id,
        cursor: nextCursor,
        state: pumped.value.state,
      });
      if (!pumped.value.terminal) {
        scheduleContinuousWatch({
          adapter,
          sessionId,
          runIdentity,
          watchId,
          epoch,
          cursor: nextCursor,
          deadline: pumped.value.next_timer_deadline,
        });
      }
    } catch (error) {
      if (epoch === runEpoch) {
        patchbayRenderer?.presentPlacementLoss(String(error));
        liveFlowStatus.textContent =
          `Abrupt browser placement loss: ${error}. This is not graceful cancellation.`;
        result.textContent = `Live ticker stopped: ${error}`;
        consoleBadge.textContent = "Failed";
        consoleBadge.className = "badge status-badge failed";
      }
    }
  }, LIVE_WATCH_PRESENTATION_INTERVAL_MS);
}

async function run() {
  if (current.runnability?.state !== "runnable") {
    result.textContent =
      `${current.runnability.code}: ${current.runnability.reason}`;
    return;
  }
  if (!patchbayView?.plan) {
    result.textContent =
      "CND-PBY-NO-PLAN: no exact plan exists for the current source revision.";
    return;
  }
  await stopExactSession("superseded");
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

  let terminal = false;
  let watchId = null;
  let watchCursor = 0;
  let sessionId = null;
  let runIdentity = null;
  try {
    const configured = await adapter.request("configure", {
      wasmUrl: wasmArtifact.url.href,
      wasmSha256: wasmArtifact.artifact.sha256,
    });
    if (!configured.ok) throw new Error(configured.code);
    const opened = await adapter.request("patchbay-open-session", {
      documentId: `tour/worker-run/${epoch}`,
      source: source.value,
    });
    if (!opened.ok || !opened.value?.ok) {
      throw new Error(`${opened.code || opened.value?.code || "open-failed"}: ${opened.value?.diagnostic || ""}`);
    }
    sessionId = opened.value.session_id;
    activeWorkerSessionId = sessionId;
    const started = await adapter.request("patchbay-start-exact-run", { sessionId });
    if (!started.ok || !started.value?.ok) {
      const rejection = {
        ok: false,
        code: started.code || started.value?.code || "start-failed",
        diagnostic: started.value?.diagnostic || "exact session start was rejected",
      };
      terminal = true;
      renderExactResultTimeline(rejection);
      result.textContent = rejection.diagnostic;
      recordEvidence({ kind: "run-rejected", lesson: current.id, code: rejection.code });
      return;
    }
    runIdentity = {
      runId: started.value.run_id,
      sourceRevision: started.value.source_revision,
      planIdentity: started.value.plan_identity,
    };
    activeWorkerRunIdentity = runIdentity;
    setLivePresentationActive();
    if (current.execution === "continuous-watch") {
      const admission = started.value.view.plan.watch_admissions.find(
        (watch) => watch.representation_id === "std/text" &&
          watch.retention === "latest" &&
          watch.sensitivity_ceiling === "public",
      );
      if (!admission) throw new Error("exact plan has no public latest-value text Watch");
      watchId = admission.id;
      const attached = await adapter.request("patchbay-attach-exact-watch", {
        sessionId,
        ...runIdentity,
        watchId,
      });
      if (!attached.ok || !attached.value?.ok) {
        throw new Error(attached.value?.diagnostic || attached.code || "Watch attach failed");
      }
      setWatchControl({
        adapter,
        sessionId,
        runIdentity,
        watchId,
        subjectLabel: admission.subject_kind === "cord"
          ? `cord ${admission.cord}`
          : `${admission.node}.${admission.port}`,
        attached: true,
        cursor: 0,
      });
    }
    const operation = activeScenario()?.execution === "cancel-before-first-step"
      ? "patchbay-cancel-exact-run"
      : "patchbay-pump-exact-run";
    const executed = await adapter.request(operation, operation === "patchbay-cancel-exact-run"
      ? { sessionId, ...runIdentity, disposition: "abort" }
      : { sessionId, ...runIdentity, quantum: 256 });
    if (epoch !== runEpoch) return;
    if (!executed.ok || !executed.value?.ok) {
      const rejection = {
        ok: false,
        code: executed.code || executed.value?.code || "pump-failed",
        diagnostic: executed.value?.diagnostic || "exact session pump was rejected",
      };
      terminal = true;
      renderExactResultTimeline(rejection);
      result.textContent = rejection.diagnostic;
      recordEvidence({ kind: "run-rejected", lesson: current.id, code: rejection.code });
      return;
    }
    const value = executed.value;
    terminal = Boolean(value.terminal);
    renderRustProjection(value.view);
    renderExactResultTimeline(value);
    const initialLiveDelta = authoritativeEvidenceDelta(value.view.evidence);
    let watched = null;
    if (watchId) {
      const read = await adapter.request("patchbay-read-exact-watch", {
        sessionId,
        ...runIdentity,
        watchId,
        cursor: watchCursor,
        maximumRecords: 1,
      });
      if (!read.ok || !read.value?.ok) {
        throw new Error(read.value?.diagnostic || read.code || "Watch read failed");
      }
      watched = read.value;
      watchCursor = watched.next_cursor;
      if (activeWatchControl?.watchId === watchId) {
        activeWatchControl.cursor = watchCursor;
      }
      renderLatestWatch(watched, value);
    }
    renderLiveFlowRows(value.view, initialLiveDelta, watched?.records?.at(-1), value);
    const counts = Number.isInteger(value.completed_nodes)
      ? `\nEvidence: ${value.completed_nodes} nodes, ${value.cords_conducted} cords conducted.`
      : "";
    const validation = activeScenario()?.validation || current.validation;
    const visibleValue = validation?.kind === "display"
      ? value.display
      : validation?.kind === "stdout"
        ? value.stdout
        : value.stdout || value.display;
    const visibleResult = value.ok
      ? `${visibleValue || (value.terminal
        ? `Run completed: ${value.terminal}.`
        : `Run remains ${value.state}; awaiting an admitted wake.`)}${counts}`
      : value.diagnostic;
    const lessonComplete = validation?.kind === "stdout"
      ? value.ok && Boolean(value.terminal) && value.stdout === validation.value
      : validation?.kind === "display"
        ? value.ok && Boolean(value.terminal) && value.display === validation.value
      : validation?.kind === "terminal"
        ? value.ok && value.terminal === validation.value
      : validation?.kind === "diagnostic"
          ? (!value.ok && value.diagnostic.includes(validation.value))
            || (value.stderr || "").includes(validation.value)
          : validation?.kind === "watch"
            ? value.ok && watched?.records?.at(-1)?.material?.text === validation.value
          : value.ok;

    result.textContent = lessonComplete
      ? `✓ Lesson complete!\n${visibleResult}`
      : visibleResult;

    recordEvidence({
      kind: lessonComplete ? "lesson-completed" : (value.ok ? "run-completed" : "run-rejected"),
      lesson: current.id,
      completedNodes: value.completed_nodes,
      cordsConducted: value.cords_conducted,
      state: value.state,
    });
    if (watchId && !terminal) {
      scheduleContinuousWatch({
        adapter,
        sessionId,
        runIdentity,
        watchId,
        epoch,
        cursor: watchCursor,
        deadline: value.next_timer_deadline,
      });
    }
  } catch (error) {
    if (epoch === runEpoch) {
      patchbayRenderer?.presentPlacementLoss(String(error));
      liveFlowStatus.textContent =
        `Abrupt browser placement loss: ${error}. This is not graceful cancellation.`;
      result.textContent = `Run failed: ${error}`;
      await stopExactSession("run-failed");
    }
  } finally {
    if (epoch === runEpoch && terminal) {
      if (sessionId && runIdentity) {
        await adapter.request("patchbay-dispose-exact-run", {
          sessionId,
          ...runIdentity,
        });
      }
      adapter.terminate("completed");
      activeAdapter = null;
      activeWorkerSessionId = null;
      activeWorkerRunIdentity = null;
      activeRunProjection = null;
      displayIsFrozen = false;
      deferredLivePresentation = null;
      freezeDisplay.disabled = true;
      freezeDisplay.setAttribute("aria-pressed", "false");
      freezeDisplay.textContent = "Freeze Display (F)";
      disableWatchControl();
      runButton.disabled = current.runnability?.state !== "runnable";
      stopButton.disabled = true;
      consoleBadge.textContent = "Idle";
      consoleBadge.className = "badge status-badge idle";
    } else if (epoch === runEpoch) {
      consoleBadge.textContent = "Live";
      consoleBadge.className = "badge status-badge running";
    }
  }
}

runButton.onclick = run;
watchToggle.onclick = () => void toggleWatch();
freezeDisplay.onclick = toggleDisplayFreeze;
stopButton.onclick = () => void stopExactSession(
  "learner-cancelled",
  "Run cancelled; exact worker placement is terminal.",
);

document.addEventListener("keydown", (event) => {
  const target = event.target;
  const isEditing = target instanceof HTMLElement && (
    ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName) ||
    target.isContentEditable
  );
  const isWatchShortcut = event.key.toLowerCase() === "w" &&
    !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
  if (isWatchShortcut && !isEditing && !event.repeat && !event.isComposing) {
    event.preventDefault();
    if (!watchToggle.disabled) void toggleWatch();
    return;
  }
  const isFreezeShortcut = event.key.toLowerCase() === "f" &&
    !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
  if (isFreezeShortcut && !isEditing && !event.repeat && !event.isComposing) {
    event.preventDefault();
    toggleDisplayFreeze();
    return;
  }
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
  void stopExactSession("reset");
  resetWatchPresentation();
  localStorage.setItem(recoveryKey(current.id), source.value);
  localStorage.removeItem(layoutKey(current.id));
  source.value = authoredSource();
  syncSourceHighlight();
  acceptedSource = source.value;
  selectedNode = null;
  selectedCord = null;
  positions = {};
  selectedNodeLabel.textContent = "No topology item selected";
  moveLeftBtn.disabled = true;
  moveRightBtn.disabled = true;
  localStorage.removeItem(draftKey(current.id));
  undoResetButton.disabled = false;
  openPatchbaySession();
  check();
};

if (executionStory) {
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
}

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
if (requestedScenario && scenarioSelect &&
    [...(current.library?.scenarios || []), ...(current.platform?.profiles || [])]
      .some((scenario) => scenario.id === requestedScenario)) {
  scenarioSelect.value = requestedScenario;
  scenarioSelect.dispatchEvent(new Event("change"));
}
if (pageParameters.has("autorun")) await run();
