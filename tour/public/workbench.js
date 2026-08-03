import { attachPanelSourceHighlighting } from "./panel-highlighter.js";
import { PatchbayReactFlowRenderer } from "./patchbay-renderer.js";

const SESSION_ID = "workbench/current";
const STORAGE_KEY = "conduit-workbench/documents/0";
const PANEL_STORAGE_KEY = "conduit-workbench/panels/0";
const EMPTY_SOURCE = "panel 0\n";
const MAXIMUM_PUMP_TURNS = 512;

const elements = Object.fromEntries([
  "new-document", "saved-documents", "open-document", "save-document", "undo", "redo",
  "run", "stop", "workbench-status", "palette-search", "palette-category",
  "palette-support", "palette-count", "palette-results", "workbench-canvas", "cy",
  "delete-node", "source", "selection-summary", "readiness", "run-result",
  "diagnostics", "evidence", "physical-execution", "connection-builder", "connection-from", "connection-to",
  "node-config", "cord-actions",
].map((id) => [id.replaceAll("-", "_"), document.getElementById(id)]));

class WorkerBridge {
  constructor(url) {
    this.worker = new Worker(url, { type: "module" });
    this.nextId = 1;
    this.pending = new Map();
    this.worker.onmessage = ({ data }) => {
      const pending = this.pending.get(data.id);
      if (!pending) return;
      this.pending.delete(data.id);
      clearTimeout(pending.timer);
      data.ok ? pending.resolve(data.value) : pending.reject(new Error(data.code));
    };
  }

  request(operation, value = {}) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`${operation} timed out`));
      }, 20_000);
      this.pending.set(id, { resolve, reject, timer });
      this.worker.postMessage({ id, operation, value });
    });
  }
}

let bridge;
let view;
let palette = [];
let history = { can_undo: false, can_redo: false };
let selectedNodeId = null;
let sourceTimer = null;
let pendingSourceEdit = null;
let running = false;
let operationQueue = Promise.resolve();
let panelZ = 30;

attachPanelSourceHighlighting(elements.source);

function readPanelStates() {
  try {
    const value = JSON.parse(localStorage.getItem(PANEL_STORAGE_KEY) || "{}");
    return value && typeof value === "object" ? value : {};
  } catch {
    return {};
  }
}

function documentZoom() {
  const zoom = Number.parseFloat(getComputedStyle(document.documentElement).zoom);
  return Number.isFinite(zoom) && zoom > 0 ? zoom : 1;
}

function savePanelState(panel) {
  const states = readPanelStates();
  states[panel.dataset.panel] = {
    schema: "conduit.workbench-panel-state",
    schema_version: 0,
    mode: panel.dataset.panelMode,
    dock: panel.dataset.panelDock,
    collapsed: panel.dataset.panelCollapsed === "true",
    left: panel.style.left,
    top: panel.style.top,
    width: panel.style.width,
    height: panel.style.height,
  };
  localStorage.setItem(PANEL_STORAGE_KEY, JSON.stringify(states));
}

function clampPanel(panel) {
  if (panel.dataset.panelMode !== "floating") return;
  const zoom = documentZoom();
  const rect = panel.getBoundingClientRect();
  const left = Math.max(8, Math.min(rect.left, window.innerWidth - Math.min(rect.width, 280) - 8));
  const top = Math.max(8, Math.min(rect.top, window.innerHeight - 56));
  panel.style.left = `${Math.round(left / zoom)}px`;
  panel.style.top = `${Math.round(top / zoom)}px`;
}

function setPanelMode(panel, mode, rect = panel.getBoundingClientRect()) {
  const zoom = documentZoom();
  panel.dataset.panelMode = mode;
  const modeButton = panel.querySelector("[data-panel-mode-control]");
  if (mode === "floating") {
    panel.style.right = "auto";
    panel.style.bottom = "auto";
    panel.style.left = `${Math.round(rect.left / zoom)}px`;
    panel.style.top = `${Math.round(rect.top / zoom)}px`;
    panel.style.width = `${Math.round(Math.max(288, rect.width) / zoom)}px`;
    panel.style.height = `${Math.round(Math.max(180, rect.height) / zoom)}px`;
    modeButton.textContent = "Dock";
    modeButton.setAttribute("aria-label", `Dock ${panel.dataset.panel} panel`);
  } else {
    for (const property of ["left", "right", "top", "bottom", "width", "height"]) {
      panel.style[property] = "";
    }
    modeButton.textContent = "Detach";
    modeButton.setAttribute("aria-label", `Detach ${panel.dataset.panel} panel`);
  }
  clampPanel(panel);
  savePanelState(panel);
}

function enhanceWorkbenchPanel(panel, saved) {
  const header = panel.querySelector(":scope > .card-header");
  if (!header) return;
  const controls = document.createElement("div");
  controls.className = "workbench-panel-controls";
  const collapse = document.createElement("button");
  collapse.type = "button";
  collapse.className = "btn small secondary";
  collapse.dataset.panelCollapseControl = "true";
  const mode = document.createElement("button");
  mode.type = "button";
  mode.className = "btn small secondary";
  mode.dataset.panelModeControl = "true";
  controls.append(collapse, mode);
  header.append(controls);

  const setCollapsed = (collapsed) => {
    panel.dataset.panelCollapsed = String(collapsed);
    collapse.textContent = collapsed ? "Expand" : "Collapse";
    collapse.setAttribute("aria-expanded", String(!collapsed));
    collapse.setAttribute("aria-label", `${collapsed ? "Expand" : "Collapse"} ${panel.dataset.panel} panel`);
    savePanelState(panel);
  };
  collapse.onclick = () => setCollapsed(panel.dataset.panelCollapsed !== "true");
  mode.onclick = () => setPanelMode(
    panel,
    panel.dataset.panelMode === "floating" ? "docked" : "floating",
  );

  if (saved?.schema === "conduit.workbench-panel-state" && saved.schema_version === 0) {
    panel.dataset.panelDock = saved.dock || panel.dataset.panelDock;
    panel.dataset.panelMode = saved.mode === "floating" ? "floating" : "docked";
    if (panel.dataset.panelMode === "floating") {
      panel.style.left = saved.left || "24px";
      panel.style.top = saved.top || "120px";
      panel.style.width = saved.width || "360px";
      panel.style.height = saved.height || "480px";
    }
    setCollapsed(Boolean(saved.collapsed));
  } else {
    setCollapsed(panel.dataset.panel !== "palette");
  }
  mode.textContent = panel.dataset.panelMode === "floating" ? "Dock" : "Detach";
  mode.setAttribute(
    "aria-label",
    `${panel.dataset.panelMode === "floating" ? "Dock" : "Detach"} ${panel.dataset.panel} panel`,
  );
  clampPanel(panel);

  let drag = null;
  header.addEventListener("pointerdown", (event) => {
    if (event.button !== 0 || event.target.closest("button")) return;
    const rect = panel.getBoundingClientRect();
    if (panel.dataset.panelMode !== "floating") setPanelMode(panel, "floating", rect);
    panel.style.zIndex = String(++panelZ);
    const zoom = documentZoom();
    drag = {
      pointerId: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      left: rect.left / zoom,
      top: rect.top / zoom,
      zoom,
    };
    header.setPointerCapture(event.pointerId);
    event.preventDefault();
  });
  header.addEventListener("pointermove", (event) => {
    if (!drag || event.pointerId !== drag.pointerId) return;
    panel.style.left = `${Math.round(drag.left + (event.clientX - drag.x) / drag.zoom)}px`;
    panel.style.top = `${Math.round(drag.top + (event.clientY - drag.y) / drag.zoom)}px`;
    clampPanel(panel);
  });
  const finishDrag = (event) => {
    if (!drag || event.pointerId !== drag.pointerId) return;
    drag = null;
    savePanelState(panel);
  };
  header.addEventListener("pointerup", finishDrag);
  header.addEventListener("pointercancel", finishDrag);
  panel.addEventListener("pointerdown", () => { panel.style.zIndex = String(++panelZ); });
}

const savedPanelStates = readPanelStates();
document.querySelectorAll(".workbench-panel").forEach((panel) =>
  enhanceWorkbenchPanel(panel, savedPanelStates[panel.dataset.panel])
);
window.addEventListener("resize", () =>
  document.querySelectorAll('.workbench-panel[data-panel-mode="floating"]').forEach(clampPanel)
);

const renderer = new PatchbayReactFlowRenderer(elements.cy, {
  preserveViewportOnTopologyChange: true,
  onTransaction: (operation, options) =>
    void applyOperations(Array.isArray(operation) ? operation : [operation], options),
  onNodeSelect: (nodeId) => selectNode(nodeId),
  onCordSelect: (cordId) => {
    selectedNodeId = null;
    elements.delete_node.disabled = true;
    elements.selection_summary.textContent = `Selected cord ${cordId}.`;
  },
  onSelectionClear: () => selectNode(null),
  onNotification: (message) => setStatus(message),
});
renderer.init();

function setStatus(message) {
  elements.workbench_status.textContent = message;
}

function parseDocuments() {
  try {
    const value = JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}");
    return value && typeof value === "object" ? value : {};
  } catch {
    return {};
  }
}

function writeDocuments(documents) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(documents));
}

function refreshSavedDocuments(selected = "") {
  const documents = parseDocuments();
  elements.saved_documents.replaceChildren(new Option("Open saved work…", ""));
  Object.keys(documents).sort().forEach((name) =>
    elements.saved_documents.add(new Option(name, name))
  );
  elements.saved_documents.value = selected;
}

function updateHistory() {
  elements.undo.disabled = !history.can_undo || running;
  elements.redo.disabled = !history.can_redo || running;
}

function renderView(nextView, { syncSource = true } = {}) {
  view = nextView;
  if (syncSource && elements.source.value !== view.source.source) {
    elements.source.value = view.source.source;
    elements.source.syncHighlight?.();
  }
  renderer.setViewModel(view, SESSION_ID, "logical");
  const planReady = Boolean(view.plan);
  elements.run.disabled = !planReady || running;
  elements.stop.disabled = !running;
  elements.readiness.textContent = planReady ? "Runnable" : "Not runnable";
  elements.readiness.className = `badge ${planReady ? "available" : "unavailable"}`;
  elements.diagnostics.replaceChildren();
  for (const diagnostic of view.diagnostics || []) {
    const item = document.createElement("li");
    item.textContent = `${diagnostic.code}: ${diagnostic.message}`;
    elements.diagnostics.append(item);
  }
  if (!elements.diagnostics.children.length && !planReady && view.topology?.logical_nodes?.length) {
    const item = document.createElement("li");
    item.textContent = "No exact plan is available. Inspect each node's support state and source configuration.";
    elements.diagnostics.append(item);
  }
  elements.evidence.textContent = view.evidence?.length
    ? JSON.stringify(view.evidence, null, 2)
    : "No evidence yet.";
  elements.physical_execution.textContent = view.physical_execution
    ? JSON.stringify(view.physical_execution, null, 2)
    : "No hosted lane activity yet.";
  renderConnectionBuilder();
  renderSelectedNode();
  renderCordActions();
  updateHistory();
}

function renderConnectionBuilder() {
  const priorFrom = elements.connection_from.value;
  const priorTo = elements.connection_to.value;
  elements.connection_from.replaceChildren();
  elements.connection_to.replaceChildren();
  for (const node of view?.topology?.logical_nodes || []) {
    for (const port of node.outputs || []) {
      elements.connection_from.add(new Option(
        `${node.id}.${port.id} — ${port.type_id}`,
        `${node.id}::${port.id}`,
      ));
    }
    for (const port of node.inputs || []) {
      elements.connection_to.add(new Option(
        `${node.id}.${port.id} — ${port.type_id}`,
        `${node.id}::${port.id}`,
      ));
    }
  }
  if ([...elements.connection_from.options].some((option) => option.value === priorFrom)) {
    elements.connection_from.value = priorFrom;
  }
  if ([...elements.connection_to.options].some((option) => option.value === priorTo)) {
    elements.connection_to.value = priorTo;
  }
}

async function openSession(source = EMPTY_SOURCE, positions = {}) {
  const opened = await bridge.request("patchbay-open-session", {
    documentId: SESSION_ID,
    source,
    taskFront: null,
  });
  if (!opened.ok) throw new Error(`${opened.code}: ${opened.diagnostic}`);
  history = opened.history || { can_undo: false, can_redo: false };
  renderView(opened.view);
  const operations = Object.entries(positions).map(([node_id, position]) => ({
    MoveNode: { node_id, position },
  }));
  for (let offset = 0; offset < operations.length; offset += 32) {
    await applyOperations(operations.slice(offset, offset + 32), { skipStatus: true });
  }
  setStatus(source === EMPTY_SOURCE ? "New blank Workbench ready." : "Saved work reopened.");
}

function applyOperations(operations, options = {}) {
  if (pendingSourceEdit !== null && !options.sourceEdit) {
    return flushSourceEdit().then(() => enqueueOperations(operations, options));
  }
  return enqueueOperations(operations, options);
}

function enqueueOperations(operations, options = {}) {
  const execute = () => applyOperationsNow(operations, options);
  operationQueue = operationQueue.then(execute, execute);
  return operationQueue;
}

function flushSourceEdit() {
  if (pendingSourceEdit === null) return operationQueue;
  clearTimeout(sourceTimer);
  sourceTimer = null;
  const candidate = pendingSourceEdit;
  pendingSourceEdit = null;
  return enqueueOperations([
    { ReplaceSource: { source: candidate } },
  ], { syncSource: false, sourceEdit: true });
}

async function applyOperationsNow(operations, options = {}) {
  if (!view || running) return { ok: false };
  const transaction = await bridge.request("patchbay-apply-transaction", {
    sessionId: SESSION_ID,
    requestJson: JSON.stringify({
      protocol_version: 0,
      document_id: SESSION_ID,
      expected_source_revision: view.source.revision,
      expected_presentation_revision: view.presentation.revision,
      operations,
    }),
  });
  if (!transaction.ok) {
    setStatus(`${transaction.code}: ${transaction.diagnostic}`);
    if (transaction.diagnostics?.length) {
      elements.run_result.textContent = transaction.diagnostics.join("\n");
    }
    return transaction;
  }
  history = transaction.history;
  renderView(transaction.view, { syncSource: options?.syncSource !== false });
  if (!options?.skipStatus) {
    setStatus(transaction.result.compatibility.compatible
      ? "Authoritative workspace revision committed."
      : `Committed with ${transaction.result.compatibility.code} diagnostics.`);
  }
  return transaction;
}

function defaultEditValue(valueType) {
  if (valueType === "std/boolean") return { kind: "boolean", value: false };
  if (/\b(?:u|i)(?:8|16|32|64|128)\b/.test(valueType) || valueType.includes("integer")) {
    return { kind: "integer", value: 1 };
  }
  if (valueType.includes("decimal")) return { kind: "exact-decimal", value: "0" };
  return { kind: "text", value: "" };
}

function initialConfig(entry) {
  return entry.catalog.config
    .filter((field) => field.requirement === "required")
    .map((field) => ({ key: field.key, value: defaultEditValue(field.value_type) }));
}

function nextNodeId(entry) {
  const base = entry.catalog.public_source_spelling.split("/").at(-1)
    .replace(/[^a-zA-Z0-9_]/g, "_") || "node";
  const ids = new Set((view?.topology?.logical_nodes || []).map((node) => node.id));
  if (!ids.has(base)) return base;
  let suffix = 2;
  while (ids.has(`${base}_${suffix}`)) suffix += 1;
  return `${base}_${suffix}`;
}

function collisionFreePosition(preferred) {
  const positions = Object.values(view?.presentation?.node_positions || {});
  let position = { x: Math.round(preferred.x), y: Math.round(preferred.y) };
  while (positions.some((item) => Math.abs(item.x - position.x) < 180 && Math.abs(item.y - position.y) < 120)) {
    position = { x: position.x + 48, y: position.y + 48 };
  }
  return position;
}

async function addEntry(entry, preferred = { x: 80, y: 80 }) {
  const node_id = nextNodeId(entry);
  await applyOperations([{ AddNode: {
    node_id,
    kind: entry.catalog.public_source_spelling,
    config: initialConfig(entry),
    position: collisionFreePosition(preferred),
  } }]);
  selectNode(node_id);
}

function selectNode(nodeId) {
  selectedNodeId = nodeId;
  elements.delete_node.disabled = !nodeId || running;
  const node = view?.topology?.logical_nodes?.find((candidate) => candidate.id === nodeId);
  elements.selection_summary.textContent = node
    ? `${node.id}: ${node.kind || node.contract_id || "semantic node"}`
    : "No node selected.";
  renderSelectedNode();
}

function editValue(kind, input) {
  if (kind === "boolean") return { kind: "boolean", value: input.checked };
  if (kind === "integer") return { kind: "integer", value: Number(input.value) };
  if (kind === "exact-decimal") return { kind: "exact-decimal", value: input.value };
  if (kind === "reference") return { kind: "reference", value: input.value };
  if (kind === "contract-reference") return { kind: "contract-reference", value: input.value };
  return { kind: "text", value: input.value };
}

function configInputValue(field) {
  if (field.kind === "text") {
    try { return JSON.parse(field.display_value); } catch { return field.display_value; }
  }
  return field.display_value;
}

function renderSelectedNode() {
  elements.node_config.replaceChildren();
  const node = view?.topology?.logical_nodes?.find((candidate) => candidate.id === selectedNodeId);
  if (!node || !Object.keys(node.config || {}).length) return;
  const title = document.createElement("h3");
  title.textContent = "Semantic configuration";
  elements.node_config.append(title);
  for (const [key, field] of Object.entries(node.config)) {
    const form = document.createElement("form");
    const label = document.createElement("label");
    label.textContent = key;
    const input = document.createElement("input");
    input.type = field.kind === "boolean" ? "checkbox" : field.kind === "integer" ? "number" : "text";
    if (input.type === "checkbox") input.checked = field.display_value === "true";
    else input.value = configInputValue(field);
    input.disabled = !field.editable || running;
    const save = document.createElement("button");
    save.type = "submit";
    save.className = "btn small secondary";
    save.textContent = `Set ${key}`;
    save.disabled = input.disabled;
    label.append(input);
    form.append(label, save);
    form.onsubmit = (event) => {
      event.preventDefault();
      void applyOperations([{ SetConfig: {
        node_id: node.id,
        key,
        value: editValue(field.kind, input),
      } }]);
    };
    elements.node_config.append(form);
  }
}

function renderCordActions() {
  elements.cord_actions.replaceChildren();
  const cords = view?.topology?.cords || [];
  if (!cords.length) return;
  const title = document.createElement("h3");
  title.textContent = "Authored cords";
  elements.cord_actions.append(title);
  for (const cord of cords) {
    const row = document.createElement("div");
    row.className = "cord-action";
    const label = document.createElement("span");
    label.textContent = `${cord.from_node || cord.from_anchor} → ${cord.to_node || cord.to_anchor}`;
    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "btn small danger";
    remove.textContent = "Disconnect";
    remove.onclick = () => void applyOperations([{ Disconnect: { cord_id: cord.id } }]);
    row.append(label, remove);
    elements.cord_actions.append(row);
  }
}

function isSupported(entry) {
  return ["provider-available", "resolvable-on-this-host"].includes(
    entry.host_observation.availability_state,
  );
}

function renderPalette() {
  const query = elements.palette_search.value.trim().toLowerCase();
  const category = elements.palette_category.value;
  const support = elements.palette_support.value;
  const filtered = palette.filter((entry) => {
    const catalog = entry.catalog;
    const haystack = [
      catalog.public_source_spelling, catalog.semantic_identity, catalog.classification,
      catalog.package_owner, ...catalog.ports.flatMap((port) => [port.id, port.value_type]),
    ].join(" ").toLowerCase();
    return (!query || haystack.includes(query)) &&
      (category === "all" || catalog.classification === category) &&
      (support === "all" || (support === "available") === isSupported(entry));
  });
  elements.palette_count.textContent = `${filtered.length}`;
  elements.palette_results.replaceChildren();
  for (const entry of filtered) {
    const item = document.createElement("article");
    item.className = "palette-item";
    item.draggable = true;
    item.dataset.contract = entry.catalog.semantic_identity;
    item.setAttribute("role", "listitem");
    item.addEventListener("dragstart", (event) => {
      event.dataTransfer.effectAllowed = "copy";
      event.dataTransfer.setData("application/x-conduit-contract", entry.catalog.semantic_identity);
    });
    const title = document.createElement("h3");
    title.textContent = entry.catalog.public_source_spelling;
    const purpose = document.createElement("p");
    purpose.textContent = entry.catalog.purpose;
    const ports = document.createElement("p");
    ports.className = "palette-ports";
    ports.textContent = entry.catalog.ports.length
      ? entry.catalog.ports.map((port) => `${port.direction === "input" ? "←" : "→"}${port.id}: ${port.value_type}`).join(" · ")
      : "No ports";
    const row = document.createElement("div");
    row.className = "button-row";
    const status = document.createElement("span");
    status.className = "palette-status";
    status.dataset.supported = String(isSupported(entry));
    status.textContent = isSupported(entry)
      ? `Available · ${entry.host_observation.implementation_id || "provider observed"}`
      : `Unavailable here · ${entry.host_observation.reason_code}`;
    const add = document.createElement("button");
    add.type = "button";
    add.className = "btn small secondary";
    add.textContent = "Add to canvas";
    add.setAttribute("aria-label", `Add ${entry.catalog.public_source_spelling} to canvas`);
    add.onclick = () => void addEntry(entry);
    row.append(status, add);
    const details = document.createElement("details");
    const summary = document.createElement("summary");
    summary.textContent = "Contract details";
    const config = document.createElement("p");
    config.textContent = entry.catalog.config.length
      ? `Configuration: ${entry.catalog.config.map((field) => `${field.key} (${field.requirement}, ${field.value_type})`).join(", ")}`
      : "No configuration required.";
    details.append(summary, config);
    item.append(title, purpose, ports, row, details);
    elements.palette_results.append(item);
  }
}

async function runPanel() {
  await flushSourceEdit();
  if (!view?.plan || running) return;
  running = true;
  renderView(view, { syncSource: false });
  setStatus("Executing the exact planned revision in the production worker…");
  try {
    let result = await bridge.request("patchbay-start-exact-run", { sessionId: SESSION_ID });
    if (!result.ok) throw new Error(`${result.code}: ${result.diagnostic}`);
    for (let turn = 0; turn < MAXIMUM_PUMP_TURNS && !result.terminal; turn += 1) {
      result = await bridge.request("patchbay-pump-exact-run", {
        sessionId: SESSION_ID,
        runId: result.run_id,
        sourceRevision: result.source_revision,
        planIdentity: result.plan_identity,
        quantum: 64,
      });
      if (!result.ok) throw new Error(`${result.code}: ${result.diagnostic}`);
      if (result.state === "waiting" && turn > 1) break;
    }
    renderView(result.view);
    elements.run_result.textContent = result.display || result.stdout ||
      `${result.state}${result.terminal ? ` (${result.terminal})` : ""}`;
    setStatus(`Exact run ${result.run_id}: ${result.state}.`);
  } catch (error) {
    elements.run_result.textContent = String(error);
    setStatus("Exact run failed; the authored revision was not changed.");
  } finally {
    running = false;
    renderView(view, { syncSource: false });
  }
}

async function stopPanel() {
  const run = view?.run;
  if (!run) return;
  const result = await bridge.request("patchbay-cancel-exact-run", {
    sessionId: SESSION_ID,
    runId: run.run_id,
    sourceRevision: view.source.revision,
    planIdentity: run.plan_identity,
    disposition: "cancelled",
  });
  if (result.ok) renderView(result.view);
  running = false;
  setStatus(result.ok ? "Run stopped through the authoritative runtime." : result.diagnostic);
}

elements.palette_search.addEventListener("input", renderPalette);
elements.palette_category.addEventListener("change", renderPalette);
elements.palette_support.addEventListener("change", renderPalette);
elements.workbench_canvas.addEventListener("dragover", (event) => {
  if (event.dataTransfer.types.includes("application/x-conduit-contract")) {
    event.preventDefault();
    elements.workbench_canvas.dataset.dragActive = "true";
  }
});
elements.workbench_canvas.addEventListener("dragleave", () => {
  delete elements.workbench_canvas.dataset.dragActive;
});
elements.workbench_canvas.addEventListener("drop", (event) => {
  event.preventDefault();
  delete elements.workbench_canvas.dataset.dragActive;
  const id = event.dataTransfer.getData("application/x-conduit-contract");
  const entry = palette.find((candidate) => candidate.catalog.semantic_identity === id);
  if (!entry) return;
  const bounds = elements.cy.getBoundingClientRect();
  void addEntry(entry, { x: event.clientX - bounds.left, y: event.clientY - bounds.top });
});
elements.source.addEventListener("input", () => {
  clearTimeout(sourceTimer);
  pendingSourceEdit = elements.source.value;
  sourceTimer = setTimeout(() => void flushSourceEdit(), 220);
});
elements.new_document.onclick = () => void openSession();
elements.save_document.onclick = async () => {
  await flushSourceEdit();
  const suggested = elements.saved_documents.value || "untitled";
  const name = window.prompt("Save this Workbench document as:", suggested)?.trim();
  if (!name || !view) return;
  const documents = parseDocuments();
  documents[name] = {
    schema: "conduit.workbench-document",
    schema_version: 0,
    source: view.source.source,
    source_identity: view.source.identity,
    semantic_identity: view.semantic.source_semantic_hash || null,
    presentation: { node_positions: view.presentation.node_positions },
  };
  writeDocuments(documents);
  refreshSavedDocuments(name);
  setStatus(`Saved “${name}” with separate semantic and presentation state.`);
};
elements.open_document.onclick = () => {
  const name = elements.saved_documents.value;
  const saved = parseDocuments()[name];
  if (!saved || saved.schema !== "conduit.workbench-document" || saved.schema_version !== 0) {
    setStatus("Choose one current Workbench document to open.");
    return;
  }
  void openSession(saved.source, saved.presentation?.node_positions || {});
};
elements.undo.onclick = () => void applyOperations(["Undo"]);
elements.redo.onclick = () => void applyOperations(["Redo"]);
elements.delete_node.onclick = () => {
  if (selectedNodeId) void applyOperations([{ DeleteNode: { node_id: selectedNodeId } }]);
};
elements.run.onclick = () => void runPanel();
elements.stop.onclick = () => void stopPanel();
elements.connection_builder.onsubmit = (event) => {
  event.preventDefault();
  const [from_node, from_port] = elements.connection_from.value.split("::");
  const [to_node, to_port] = elements.connection_to.value.split("::");
  if (!from_node || !from_port || !to_node || !to_port) return;
  void applyOperations([{ Connect: {
    from_node,
    from_port,
    to_node,
    to_port,
    bounds: {
      capacity_items: 8,
      max_value_bytes: 65536,
      max_queued_bytes: 524288,
      low_watermark_items: 4,
      high_watermark_items: 8,
      pressure: "block",
    },
  } }]);
};
document.addEventListener("keydown", (event) => {
  const editing = event.target instanceof HTMLTextAreaElement || event.target instanceof HTMLInputElement;
  if (editing || !(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== "z") return;
  event.preventDefault();
  void applyOperations([event.shiftKey ? "Redo" : "Undo"]);
});

async function boot() {
  try {
    const plan = await fetch("./browser-plan.json", { cache: "no-store" }).then((response) => {
      if (!response.ok) throw new Error(`browser plan ${response.status}`);
      return response.json();
    });
    const wasm = plan.artifacts.find((artifact) => artifact.id === "conduit-web-wasm");
    bridge = new WorkerBridge(new URL("./tour-worker.mjs", import.meta.url));
    await bridge.request("configure", {
      wasmUrl: new URL(wasm.path, import.meta.url).href,
      wasmSha256: wasm.sha256,
    });
    const projected = await bridge.request("patchbay-workbench-palette");
    if (!projected.ok) throw new Error(`${projected.code}: ${projected.diagnostic}`);
    palette = projected.entries;
    [...new Set(palette.map((entry) => entry.catalog.classification))].sort().forEach((category) =>
      elements.palette_category.add(new Option(category, category))
    );
    renderPalette();
    refreshSavedDocuments();
    await openSession();
    document.documentElement.dataset.workbenchReady = "true";
  } catch (error) {
    setStatus(`Workbench could not start: ${error}. Run “just workbench” to rebuild required artifacts.`);
    elements.run_result.textContent = String(error);
  }
}

void boot();
