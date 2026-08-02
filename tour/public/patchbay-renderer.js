/**
 * Conduit Patchbay React Flow renderer.
 *
 * Conduit supplies semantic and projected graph data. React Flow owns the canvas
 * interactions, selection, and presentation while Rust remains authoritative.
 */

import { FaceplateNodeComponent } from "./patchbay-faceplate.js";
import { patchbayFeatures } from "./patchbay-features.js";
import { PatchbayCordEdge } from "./patchbay-cord-edge.js";
import { projectedNodeHeight } from "./patchbay-layout.js";

const e = window.React.createElement;

const DEFAULT_CORD_BOUNDS = Object.freeze({
  capacity_items: 1,
  max_value_bytes: 1024,
  max_queued_bytes: 1024,
  low_watermark_items: 0,
  high_watermark_items: 1,
  pressure: "block",
});

function connectOperation(connection, bounds = DEFAULT_CORD_BOUNDS) {
  if (!connection?.source || !connection?.sourceHandle ||
      !connection?.target || !connection?.targetHandle) {
    return null;
  }
  return {
    Connect: {
      from_node: connection.source,
      from_port: connection.sourceHandle,
      to_node: connection.target,
      to_port: connection.targetHandle,
      bounds: { ...bounds },
    },
  };
}

function normalizeConnection(connection, nodes) {
  const portDirection = (nodeId, portId) => {
    const node = nodes.find((candidate) => candidate.id === nodeId);
    if (node?.outputs?.some((port) => port.id === portId)) return "output";
    if (node?.inputs?.some((port) => port.id === portId)) return "input";
    return null;
  };
  const sourceDirection = portDirection(connection?.source, connection?.sourceHandle);
  const targetDirection = portDirection(connection?.target, connection?.targetHandle);
  if (sourceDirection === "output" && targetDirection === "input") {
    return connection;
  }
  if (sourceDirection === "input" && targetDirection === "output") {
    return {
      source: connection.target,
      sourceHandle: connection.targetHandle,
      target: connection.source,
      targetHandle: connection.sourceHandle,
    };
  }
  return null;
}

function projectedCordBounds(cord) {
  const bounds = {
    capacity_items: cord?.capacity_items,
    max_value_bytes: cord?.max_value_bytes,
    max_queued_bytes: cord?.max_queued_bytes,
    low_watermark_items: cord?.low_watermark_items,
    high_watermark_items: cord?.high_watermark_items,
    pressure: cord?.pressure === "block(fifo)" ? "block" : cord?.pressure,
  };
  return Object.values(bounds).every((value) => value !== null && value !== undefined)
    ? bounds
    : null;
}

function classToken(value) {
  return value
    .replace(/[^a-z0-9]+/gi, "-")
    .replace(/^-|-$/g, "")
    .toLowerCase();
}

function pressureFamily(pressure) {
  if (typeof pressure !== "string") return "unknown";
  const normalized = pressure.toLowerCase();
  for (const policy of [
    "drop-disposable",
    "disconnect",
    "coalesce",
    "reject",
    "sample",
    "block",
    "fail",
  ]) {
    if (normalized.startsWith(policy)) return policy;
  }
  return "unknown";
}

function typePresentation(valueType) {
  if (typeof valueType !== "string") {
    return { color: "#94a3b8", family: "unknown" };
  }
  const normalized = valueType.toLowerCase();
  if (normalized.startsWith("conduit.net/")) {
    if (normalized.includes("link-observation")) {
      return { color: "#2dd4bf", family: "network-link" };
    }
    if (normalized.endsWith("/frame")) {
      return { color: "#38bdf8", family: "network-frame" };
    }
    if (normalized.endsWith("/packet")) {
      return { color: "#818cf8", family: "network-packet" };
    }
    if (normalized.endsWith("/datagram")) {
      return { color: "#a78bfa", family: "network-datagram" };
    }
    if (normalized.includes("byte-stream")) {
      return { color: "#f472b6", family: "network-stream" };
    }
    if (normalized.endsWith("/session")) {
      return { color: "#f59e0b", family: "network-session" };
    }
    if (normalized.includes("control-event")) {
      return { color: "#facc15", family: "network-control" };
    }
    return { color: "#e879f9", family: "network-state" };
  }
  if (normalized.includes("retained-state") || normalized.includes("retained_state")) {
    return { color: "#e879f9", family: "state" };
  }
  if (normalized.includes("/event") || normalized.endsWith("event")) {
    return { color: "#fb923c", family: "event" };
  }
  if (normalized.includes("/gate") || normalized.endsWith("gate")) {
    return { color: "#fb7185", family: "gate" };
  }
  if (normalized.includes("/control") || normalized.endsWith("control")) {
    return { color: "#facc15", family: "control" };
  }
  if (normalized.includes("audio")) {
    return { color: "#22d3ee", family: "audio" };
  }
  if (normalized.includes("text") || normalized.includes("utf")) {
    return { color: "#34d399", family: "text" };
  }
  if (normalized.includes("bytes") || normalized.includes("binary")) {
    return { color: "#22d3ee", family: "bytes" };
  }
  if (normalized.includes("json") || normalized.includes("record") ||
      normalized.includes("data")) {
    return { color: "#c084fc", family: "structured" };
  }
  if (normalized.includes("http") || normalized.includes("request") ||
      normalized.includes("response") || normalized.includes("network")) {
    return { color: "#f59e0b", family: "network" };
  }
  if (normalized.includes("bool") || normalized.includes("number") ||
      normalized.includes("integer") || normalized.includes("float")) {
    return { color: "#facc15", family: "numeric" };
  }
  return { color: "#60a5fa", family: "other" };
}

function edgePresentation(edge) {
  const policy = pressureFamily(edge.pressure);
  const valueType = typePresentation(edge.value_type);
  const compatible = edge.compatibility?.compatible === true;
  const capacity = Number.isFinite(edge.capacity_items) ? edge.capacity_items : 0;
  const capacityTier = capacity <= 1
    ? "single"
    : edge.capacity_items <= 4
      ? "small"
      : edge.capacity_items <= 16
        ? "medium"
        : "large";
  const lossClass = ["coalesce", "sample", "drop-disposable"].includes(policy)
    ? "lossy"
    : "lossless";
  const thresholdRatio = capacity > 0 ? edge.high_watermark_items / capacity : 1;
  const thresholdClass = thresholdRatio <= 0.5
    ? "early"
    : thresholdRatio < 1
      ? "graduated"
      : "full";
  const strokeWidth = edge.validity === "valid"
    ? 2 + Math.min(2.5, Math.log2(capacity + 1) * 0.45)
    : 4;
  const invalid = edge.validity !== "valid";
  const color = invalid ? "#ff1744" : compatible ? valueType.color : "#fb7185";
  const stateLabel = invalid
    ? `× ${edge.validity.replaceAll("-", " ")} ×`
    : `${edge.value_type} · ${edge.capacity_items} slots · ` +
      `${edge.low_watermark_items}↗${edge.high_watermark_items} · ${edge.pressure}`;

  return {
    color,
    strokeWidth,
    className: [
      `pressure-${policy}`,
      `pressure-${lossClass}`,
      `value-type-${classToken(edge.value_type || "unknown")}`,
      `type-family-${valueType.family}`,
      `capacity-${capacityTier}`,
      `threshold-${thresholdClass}`,
      compatible ? "compatibility-compatible" : "compatibility-incompatible",
      invalid ? "cord-diagnostic-error" : "",
      `cord-validity-${classToken(edge.validity || "unresolved")}`,
    ].join(" "),
    label: stateLabel,
  };
}

function endpointForLogicalView(edge, side, anchors, projectedNodeIds) {
  const anchorId = edge[`${side}_anchor`];
  if (anchorId) {
    const anchor = anchors.find((candidate) => candidate.id === anchorId);
    if (anchor?.owner_node && projectedNodeIds.has(anchor.owner_node)) {
      return { node: anchor.owner_node, port: anchor.id };
    }
    return { node: anchorId, port: "diagnostic-anchor" };
  }
  const path = edge[`${side}_port_path`];
  const members = typeof path === "string" ? path.split("/") : [];
  const portIndex = members.lastIndexOf("port");
  if (members[0] !== "root" || portIndex < 2 ||
      portIndex + 2 >= members.length) {
    return null;
  }
  return {
    node: members.slice(1, portIndex).join("/"),
    port: members.slice(portIndex + 2).join("/"),
  };
}

export class PatchbayReactFlowRenderer {
  constructor(containerElement, options = {}) {
    this.container = containerElement;
    this.options = options;
    this.reactRoot = null;
    this.viewModel = null;
    this.selectedNodeId = null;
    this.selectedCordId = null;
    this.onTransaction = options.onTransaction || null;
    this.onNodeSelect = options.onNodeSelect || null;
    this.onCordSelect = options.onCordSelect || null;
    this.onPortSelect = options.onPortSelect || null;
    this.onCordWatch = options.onCordWatch || null;
    this.onPortWatch = options.onPortWatch || null;
    this.onSelectionClear = options.onSelectionClear || null;
    this.onOpenNested = options.onOpenNested || null;
    this.livePulseTimers = new Map();
  }

  init() {
    this.container.innerHTML = "";
    this.flowWrapper = document.createElement("div");
    this.flowWrapper.id = "patchbay-flow-root";
    this.flowWrapper.className = "patchbay-flow-container";
    this.container.appendChild(this.flowWrapper);
    this.liveRunStatus = document.createElement("p");
    this.liveRunStatus.className = "patchbay-live-run-status";
    this.liveRunStatus.setAttribute("role", "status");
    this.liveRunStatus.setAttribute("aria-live", "polite");
    this.container.appendChild(this.liveRunStatus);
    this.renderFlow();
  }

  setViewModel(viewModel, lessonId = "default", topologyView = "logical") {
    this.viewModel = viewModel;
    this.lessonId = lessonId;
    this.topologyView = topologyView;
    if (!viewModel) {
      this.selectedNodeId = null;
      this.selectedCordId = null;
      this.renderedCordIds = [];
      this.flowInstance = null;
    }
    this.updateRunPresentation(viewModel);
    const renderIdentity = JSON.stringify([
      lessonId,
      viewModel?.source?.identity,
      viewModel?.source?.revision,
      viewModel?.presentation?.identity,
      viewModel?.plan?.identity,
      viewModel?.run?.run_id,
      topologyView,
      (viewModel?.diagnostics || []).map((diagnostic) => diagnostic.id),
      (viewModel?.topology?.cords || []).map((cord) => [
        cord.id,
        cord.validity,
        cord.from_anchor,
        cord.to_anchor,
      ]),
    ]);
    if (renderIdentity === this.lastRenderIdentity) return;
    this.lastRenderIdentity = renderIdentity;
    this.renderFlow();
  }

  updateRunPresentation(viewModel) {
    if (!this.flowWrapper) return;
    const run = viewModel?.run;
    const state = run?.state || "Prepared";
    const sourceRevision = viewModel?.source?.revision;
    const activeSource = run?.source_semantic_hash;
    const candidateSource = viewModel?.semantic?.source_semantic_hash;
    const candidateChanged = Boolean(
      (activeSource && candidateSource && activeSource !== candidateSource) ||
      (Number.isSafeInteger(run?.source_revision) &&
        Number.isSafeInteger(sourceRevision) && run.source_revision !== sourceRevision),
    );
    this.flowWrapper.dataset.runState = state.toLowerCase();
    this.flowWrapper.dataset.activeEpoch = run?.plan_identity || "";
    this.flowWrapper.dataset.candidateRevision = String(sourceRevision ?? "");
    this.flowWrapper.dataset.candidateChanged = String(candidateChanged);
    if (!this.liveRunStatus) return;
    if (!run) {
      this.liveRunStatus.textContent = "No exact run started.";
      return;
    }
    const lifecycle = state.toLowerCase();
    const activeRevision = run.source_revision ?? "pinned at Start";
    const candidate = candidateChanged
      ? ` Candidate revision ${sourceRevision} is separate from this active epoch.`
      : "";
    this.liveRunStatus.textContent =
      `Exact run ${run.run_id} is ${lifecycle}; active plan ${run.plan_identity}; ` +
      `active source revision ${activeRevision}; edited draft revision ${sourceRevision}.${candidate}`;
  }

  presentLiveEvidence(viewModel, records, watchRecord) {
    this.updateRunPresentation(viewModel);
    this.flowWrapper.querySelectorAll(".react-flow__edge[data-watch-observed]")
      .forEach((edge) => delete edge.dataset.watchObserved);
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    for (const record of records.slice(-32)) {
      if (record.subject_kind !== "cord" || !record.cord_id) continue;
      const edge = [...this.flowWrapper.querySelectorAll(".react-flow__edge")]
        .find((candidate) =>
          candidate.dataset.id === record.cord_id ||
          candidate.querySelector(".react-flow__edge-path")?.id === record.cord_id
        );
      if (!edge) continue;
      edge.dataset.liveUpdate = "true";
      edge.dataset.liveSequence = String(record.sequence);
      edge.dataset.liveTick = String(record.tick);
      edge.dataset.occupancyItems = String(record.occupancy_items);
      edge.dataset.occupancyBytes = String(record.occupancy_bytes);
      edge.setAttribute(
        "aria-label",
        `${record.cord_id}: ${record.event_kind}; ${record.pressure}; ` +
        `${record.occupancy_items} items, ${record.occupancy_bytes} bytes`,
      );
      if (reducedMotion) continue;
      edge.classList.remove("live-flow-pulse");
      void edge.getBoundingClientRect();
      edge.classList.add("live-flow-pulse");
      clearTimeout(this.livePulseTimers.get(record.cord_id));
      this.livePulseTimers.set(record.cord_id, setTimeout(() => {
        edge.classList.remove("live-flow-pulse");
        this.livePulseTimers.delete(record.cord_id);
      }, 450));
    }
    if (watchRecord?.subject?.kind === "cord") {
      const watched = [...this.flowWrapper.querySelectorAll(".react-flow__edge")]
        .find((candidate) =>
          candidate.dataset.id === watchRecord.subject.cord ||
          candidate.querySelector(".react-flow__edge-path")?.id === watchRecord.subject.cord
        );
      if (watched) watched.dataset.watchObserved = "true";
    }
  }

  presentPlacementLoss(detail) {
    if (!this.flowWrapper || !this.liveRunStatus) return;
    this.flowWrapper.dataset.runState = "placement-loss";
    this.liveRunStatus.textContent =
      `Abrupt browser placement loss: ${detail}. No graceful cancellation is claimed.`;
  }

  selectNode(nodeId) {
    this.selectedNodeId = nodeId;
    this.selectedCordId = null;
    this.highlightCordEndpoints(null);
    const projectedEdges = this.topologyView === "logical"
      ? this.viewModel?.topology?.cords || []
      : this.viewModel?.topology?.planned_realization?.cords || [];
    const neighboringNodes = new Set([nodeId]);
    const neighboringCords = new Set();
    for (const cord of projectedEdges) {
      if (cord.from_node === nodeId || cord.to_node === nodeId) {
        neighboringCords.add(cord.id);
        if (cord.from_node) neighboringNodes.add(cord.from_node);
        if (cord.to_node) neighboringNodes.add(cord.to_node);
      }
    }
    this.flowWrapper.dataset.selection = "node";
    this.flowWrapper?.querySelectorAll(".conduit-faceplate-card").forEach((card) => {
      const shell = card.closest(".react-flow__node");
      const candidateId = shell?.dataset.id;
      card.classList.toggle("selected-faceplate", candidateId === nodeId);
      card.classList.toggle("selection-neighbor", neighboringNodes.has(candidateId));
      card.classList.toggle("selection-muted", !neighboringNodes.has(candidateId));
    });
    this.flowWrapper?.querySelectorAll(".react-flow__edge").forEach((edge, index) => {
      const candidateId = this.renderedCordIds?.[index];
      edge.classList.toggle("selection-neighbor", neighboringCords.has(candidateId));
      edge.classList.toggle("selection-muted", !neighboringCords.has(candidateId));
    });
  }

  selectCord(cordId) {
    this.selectedNodeId = null;
    this.selectedCordId = cordId;
    const projectedEdges = this.topologyView === "logical"
      ? this.viewModel?.topology?.cords || []
      : this.viewModel?.topology?.planned_realization?.cords || [];
    const selectedCord = projectedEdges.find((edge) => edge.id === cordId) || null;
    const endpointNodes = new Set(
      selectedCord ? [selectedCord.from_node, selectedCord.to_node].filter(Boolean) : [],
    );
    this.flowWrapper.dataset.selection = "cord";
    this.flowWrapper?.querySelectorAll(".conduit-faceplate-card").forEach((card) => {
      const candidateId = card.closest(".react-flow__node")?.dataset.id;
      card.classList.remove("selected-faceplate");
      card.classList.toggle("selection-neighbor", endpointNodes.has(candidateId));
      card.classList.toggle("selection-muted", !endpointNodes.has(candidateId));
    });
    this.flowWrapper?.querySelectorAll(".react-flow__edge").forEach((edge, index) => {
      const selected = this.renderedCordIds?.[index] === cordId;
      edge.classList.toggle("selected", selected);
      edge.classList.toggle("selection-neighbor", selected);
      edge.classList.toggle("selection-muted", !selected);
    });
    this.highlightCordEndpoints(
      selectedCord,
    );
  }

  clearSelection() {
    this.selectedNodeId = null;
    this.selectedCordId = null;
    if (this.flowWrapper) delete this.flowWrapper.dataset.selection;
    this.flowWrapper?.querySelectorAll(
      ".selected-faceplate, .selection-neighbor, .selection-muted",
    ).forEach((element) => element.classList.remove(
      "selected-faceplate",
      "selection-neighbor",
      "selection-muted",
    ));
    this.flowWrapper?.querySelectorAll(".react-flow__edge.selected").forEach((edge) => {
      edge.classList.remove("selected");
    });
    this.highlightCordEndpoints(null);
  }

  highlightCordEndpoints(cord) {
    const plannedPortPath = (node, direction, port) =>
      `${node.startsWith("root/") ? node : `root/${node}`}` +
      `/port/${direction}/${port}`;
    const selectedPaths = new Set(cord
      ? this.topologyView === "logical"
        ? [cord.from_port_path, cord.to_port_path]
        : [
            plannedPortPath(cord.from_node, "outgoing", cord.from_port),
            plannedPortPath(cord.to_node, "receiving", cord.to_port),
          ]
      : []
    );
    this.flowWrapper?.querySelectorAll(".faceplate-port-row").forEach((row) => {
      const selected = selectedPaths.has(row.dataset.semanticPath);
      row.classList.toggle("selected-cord-endpoint", selected);
      row.setAttribute("aria-current", selected ? "true" : "false");
    });
  }

  moveNode() {
    // React Flow owns interaction. Position updates come from drag callbacks.
  }

  getViewport() {
    return this.flowInstance?.getViewport?.() || null;
  }

  setViewport(viewport) {
    if (!viewport || !this.flowInstance?.setViewport) return;
    void this.flowInstance.setViewport(viewport, { duration: 0 });
  }

  fitViewport(instance = this.flowInstance) {
    if (!instance?.fitView) return;
    requestAnimationFrame(() => requestAnimationFrame(() => {
      if (this.flowInstance !== instance) return;
      void instance.fitView({ maxZoom: 1.2, duration: 0 });
    }));
  }

  notifyResize() {
    window.dispatchEvent(new Event("resize"));
  }

  updateConfig(nodeId, key, value, kind) {
    if (!this.onTransaction) return;
    this.onTransaction({
      SetConfig: {
        node_id: nodeId,
        key,
        value: { kind, value },
      },
    }, { preserveFaceplateFocus: true });
  }

  renderFlow() {
    if (window.__CONDUIT_DISABLE_PATCHBAY_RENDERER__) {
      const unavailable = e(
        "div",
        { className: "card-subtitle error-text" },
        "React Flow renderer unavailable.",
      );
      if (this.reactRoot) {
        this.reactRoot.render(unavailable);
      } else {
        this.flowWrapper.replaceChildren(
          Object.assign(document.createElement("div"), {
            className: "card-subtitle error-text",
            textContent: "React Flow renderer unavailable.",
          }),
        );
      }
      return;
    }
    if (!window.React || !window.ReactDOM || !window.ReactFlow) {
      this.container.innerHTML =
        '<div class="card-subtitle error-text">React Flow libraries unavailable.</div>';
      return;
    }
    const viewModel = this.viewModel;
    if (!viewModel) {
      this.flowWrapper.dataset.projection = "unavailable";
      this.flowWrapper.dataset.nodeCount = "0";
      this.flowWrapper.dataset.edgeCount = "0";
      delete this.flowWrapper.dataset.layout;
      if (this.reactRoot) {
        this.reactRoot.render(null);
      } else {
        this.flowWrapper.replaceChildren();
      }
      return;
    }

    const realization = viewModel.topology?.planned_realization;
    const presentationCanEdit = viewModel.presentation?.mode === "build" &&
      ["face", "context", "configure"].includes(viewModel.presentation?.lens);
    const projectedNodes = this.topologyView === "logical"
      ? viewModel.topology?.logical_nodes || []
      : (realization?.nodes || []).map((node) => ({
          id: node.instance,
          semantic_id: node.binding.contract_identity,
          contract_id: node.binding.contract_id,
          contract_identity: node.binding.contract_identity,
          semantic_effects: [],
          source_range: node.source_origin_range,
          inputs: node.inputs || [],
          outputs: node.outputs || [],
          config: {},
          availability: null,
          validity: "valid",
          diagnostic_ids: [],
          placement: null,
          activity: null,
          plannedBinding: node.binding,
          logicalOrigin: node.logical_origin,
          compositeProvenance: node.composite_provenance || [],
        }));
    const projectedNodeIds = new Set(projectedNodes.map((node) => node.id));
    const diagnosticAnchors = this.topologyView === "logical"
      ? viewModel.topology?.diagnostic_anchors || []
      : [];
    const sourceEdges = this.topologyView === "logical"
      ? viewModel.topology?.cords || []
      : realization?.cords || [];
    const projectedEdges = sourceEdges
      .map((edge) => ({
        edge: this.topologyView === "logical"
          ? edge
          : {
              ...edge,
              validity: "valid",
              compatibility: { compatible: true },
            },
        source: this.topologyView === "logical"
          ? endpointForLogicalView(edge, "from", diagnosticAnchors, projectedNodeIds)
          : { node: edge.from_node, port: edge.from_port },
        target: this.topologyView === "logical"
          ? endpointForLogicalView(edge, "to", diagnosticAnchors, projectedNodeIds)
          : { node: edge.to_node, port: edge.to_port },
      }))
      .filter(({ source, target }) =>
        source && target &&
        (projectedNodeIds.has(source.node) ||
          diagnosticAnchors.some((anchor) => anchor.id === source.node)) &&
        (projectedNodeIds.has(target.node) ||
          diagnosticAnchors.some((anchor) => anchor.id === target.node))
      );
    const nodePositions = viewModel.presentation?.node_positions || {};
    const projectedNodeHeights = new Map(projectedNodes.map((node) => [
      node.id,
      projectedNodeHeight({
        ...node,
        diagnosticAnchors: diagnosticAnchors.filter(
          (anchor) => anchor.owner_node === node.id,
        ),
      }),
    ]));
    const defaultNodePositions = new Map();
    const nextDefaultY = [40, 40];
    projectedNodes.forEach((node, index) => {
      const column = index % 2;
      defaultNodePositions.set(node.id, {
        x: 32 + column * 640,
        y: nextDefaultY[column],
      });
      nextDefaultY[column] += projectedNodeHeights.get(node.id) + 80;
    });
    const positionForNode = (nodeId, index) => {
      if (this.topologyView === "expanded") {
        // Expanded keeps compact recognizable symbols; complete exact values
        // live in the selected-subject inspector rather than widening a strip
        // of per-node property sheets.
        return defaultNodePositions.get(nodeId) || {
          x: 32 + (index % 2) * 640,
          y: 40 + Math.floor(index / 2) * 320,
        };
      }
      return nodePositions[nodeId] || defaultNodePositions.get(nodeId);
    };

    const compositeIds = new Set(
      (viewModel.topology?.composites || []).map((composite) => composite.id),
    );
    const nodes = projectedNodes.map((node, index) => {
      return {
        id: node.id,
        type: "faceplate",
        position: positionForNode(node.id, index),
        className: "react-flow-node-shell",
        width: 350,
        style: { width: 350 },
        data: {
        ...node,
        title: node.id,
        kind: node.contract_id,
        inputs: node.inputs || [],
        outputs: node.outputs || [],
        status: node.activity || "idle",
        activity: node.activity,
        validity: node.validity,
        diagnosticIds: node.diagnostic_ids || [],
        diagnosticAnchors: diagnosticAnchors.filter(
          (anchor) => anchor.owner_node === node.id,
        ),
        isConnectable: this.topologyView === "logical" && presentationCanEdit,
        plannedBinding: node.plannedBinding,
        isComposite: compositeIds.has(node.id),
        isSelected: node.id === this.selectedNodeId,
        onOpenNested: (nodeId, kind) => {
          if (this.onOpenNested) {
            this.onOpenNested(nodeId, kind);
          }
        },
        onPortSelect: (nodeId, port) => {
          if (this.onPortSelect) {
            this.onPortSelect(nodeId, port);
          }
        },
        onPortWatch: (nodeId, port) => {
          if (this.onPortWatch) this.onPortWatch(nodeId, port);
        },
        },
        draggable: this.topologyView === "logical" && presentationCanEdit,
        selectable: true,
      };
    });
    const diagnosticAnchorTop = nodes.reduce(
      (bottom, node) => Math.max(
        bottom,
        node.position.y + projectedNodeHeights.get(node.id),
      ),
      40,
    ) + 80;
    const standaloneAnchors = diagnosticAnchors
      .filter((anchor) => !anchor.owner_node || !projectedNodeIds.has(anchor.owner_node))
      .map((anchor, index) => ({
        id: anchor.id,
        type: "diagnosticAnchor",
        position: {
          x: 32 + (index % 2) * 400,
          y: diagnosticAnchorTop + Math.floor(index / 2) * 130,
        },
        data: anchor,
        draggable: false,
        selectable: true,
      }));
    nodes.push(...standaloneAnchors);

    const emphasizedDiagnosticCord = projectedEdges.find(
      ({ edge }) => edge.validity !== "valid",
    )?.edge.id;
    const edges = projectedEdges.map(({ edge, source, target }) => {
      const presentation = edgePresentation(edge);
      return {
        id: edge.id,
        source: source.node,
        sourceHandle: source.port,
        target: target.node,
        targetHandle: target.port,
        type: "patchbayCord",
        label: presentation.label,
        markerEnd: {
          type: window.ReactFlow.MarkerType.ArrowClosed,
          color: presentation.color,
          width: 18,
          height: 18,
        },
        className: [
          "patchbay-cord",
          presentation.className,
          edge.id === emphasizedDiagnosticCord ? "diagnostic-emphasized" : "",
        ].join(" "),
        style: {
          stroke: presentation.color,
          strokeWidth: presentation.strokeWidth,
          "--cord-color": presentation.color,
          "--cord-width": `${presentation.strokeWidth}px`,
        },
        labelStyle: {
          fill: "#f8fafc",
          fontFamily: "var(--font-mono)",
          fontSize: "10px",
          fontWeight: 700,
        },
        labelBgStyle: {
          fill: "#111827",
          stroke: presentation.color,
          strokeWidth: "1.5px",
        },
        labelBgPadding: [5, 7],
        labelBgBorderRadius: 4,
        animated: false,
        selected: edge.id === this.selectedCordId,
        ariaLabel: `${edge.id}: ${presentation.label}`,
      };
    });
    this.renderedCordIds = edges.map((edge) => edge.id);

    const ReactFlowRenderer =
      window.ReactFlow.default || window.ReactFlow.ReactFlow || window.ReactFlow;
    const FaceplateNode = ({ data }) => e(
      "div",
      { className: "react-flow-node-shell" },
      e(FaceplateNodeComponent, { id: data.id, data }),
    );
    const DiagnosticAnchorNode = ({ data }) => e(
      "div",
      {
        className: "patchbay-diagnostic-anchor-card",
        role: "note",
        "aria-label": `Unresolved authored endpoint ${data.label}`,
      },
      e(window.ReactFlow.Handle, {
        id: "diagnostic-anchor",
        type: data.side === "from" ? "source" : "target",
        position: data.side === "from"
          ? window.ReactFlow.Position.Right
          : window.ReactFlow.Position.Left,
        isConnectable: false,
        className: "patchbay-diagnostic-anchor-handle",
      }),
      e("strong", { "aria-hidden": "true" }, "×"),
      e("span", null, data.label),
    );
    const nodeTypes = {
      faceplate: FaceplateNode,
      diagnosticAnchor: DiagnosticAnchorNode,
    };

    const edgeTypes = { patchbayCord: PatchbayCordEdge };

    const topologyIdentity = [
      ...nodes.map((node) =>
        `node:${node.id}:${node.position.x}:${node.position.y}`),
      ...edges.map((edge) => `cord:${edge.id}`),
    ].join("\0");
    const flow = e(
      ReactFlowRenderer,
      {
        key: topologyIdentity,
        defaultNodes: nodes,
        edges,
        nodeTypes,
        edgeTypes,
        edgesSelectable: true,
        elevateEdgesOnSelect: true,
        nodesDraggable: this.topologyView === "logical" && presentationCanEdit,
        nodesConnectable: this.topologyView === "logical" && presentationCanEdit,
        elementsSelectable: true,
        connectionMode: window.ReactFlow.ConnectionMode.Loose,
        connectionLineType: window.ReactFlow.ConnectionLineType.Straight,
        onConnect: (connection) => {
          if (!presentationCanEdit) return;
          const normalizedConnection = normalizeConnection(connection, projectedNodes);
          const operation = connectOperation(normalizedConnection);
          if (!operation || !this.onTransaction) return;
          this.onTransaction(operation, { syncSource: true });
        },
        onEdgeUpdate: (oldEdge, connection) => {
          if (!presentationCanEdit || !this.onTransaction) return;
          const cord = (viewModel.topology?.cords || [])
            .find((candidate) => candidate.id === oldEdge.id);
          const bounds = projectedCordBounds(cord);
          const replacement = connectOperation(
            normalizeConnection(connection, projectedNodes),
            bounds,
          );
          if (!cord || !bounds || !replacement) return;
          this.onTransaction([
            { Disconnect: { cord_id: cord.id } },
            replacement,
          ], { syncSource: true });
        },
        onEdgesDelete: (deletedEdges) => {
          if (!presentationCanEdit || !this.onTransaction || deletedEdges.length === 0) return;
          this.onTransaction(
            deletedEdges.map((edge) => ({ Disconnect: { cord_id: edge.id } })),
            { syncSource: true },
          );
        },
        onNodeClick: (_event, node) => {
          this.selectedNodeId = node.id;
          this.selectedCordId = null;
          if (this.onNodeSelect) {
            this.onNodeSelect(node.id);
          }
        },
        onEdgeClick: (_event, edge) => {
          this.selectedNodeId = null;
          this.selectedCordId = edge.id;
          if (this.onCordSelect) {
            this.onCordSelect(edge.id);
          }
        },
        onEdgeDoubleClick: (event, edge) => {
          event.preventDefault();
          if (this.onCordWatch) this.onCordWatch(edge.id);
        },
        onSelectionChange: ({ nodes: selectedNodes, edges: selectedEdges }) => {
          if (selectedEdges.length > 0) {
            const cordId = selectedEdges[0].id;
            if (cordId !== this.selectedCordId && this.onCordSelect) {
              this.selectedNodeId = null;
              this.selectedCordId = cordId;
              this.onCordSelect(cordId);
            }
          } else if (selectedNodes.length > 0) {
            const nodeId = selectedNodes[0].id;
            if (nodeId !== this.selectedNodeId && this.onNodeSelect) {
              this.selectedNodeId = nodeId;
              this.selectedCordId = null;
              this.onNodeSelect(nodeId);
            }
          }
        },
        onPaneClick: () => {
          this.selectedNodeId = null;
          this.selectedCordId = null;
          if (this.onSelectionClear) this.onSelectionClear();
        },
        onNodeDragStop: (_event, node) => {
          if (this.topologyView !== "logical" || !presentationCanEdit) return;
          if (!this.onTransaction) return;
          if (!node?.position) return;
          this.onTransaction({
            MoveNode: {
              node_id: node.id,
              position: {
                x: Math.round(node.position.x),
                y: Math.round(node.position.y),
              },
            },
          });
        },
        snapToGrid: false,
        defaultViewport: { x: 0, y: 0, zoom: 1 },
        minZoom: 0.2,
        maxZoom: 3,
        fitView: true,
        fitViewOptions: { maxZoom: 1.2 },
        onInit: (instance) => {
          this.flowInstance = instance;
          this.flowWrapper.dataset.layout = "ready";
          // React Flow's initial fit can run before WebKit has reported the
          // intrinsic height of the semantic-promise faceplates. Refit once
          // those ResizeObserver measurements have crossed two paint frames;
          // this is still the initial topology fit, not a mutation of the
          // presentation positions or a later reset of the user's viewport.
          this.fitViewport(instance);
        },
      },
    );

    this.flowWrapper.dataset.renderer = "react-flow";
    this.flowWrapper.dataset.projection = "rust-authoritative";
    this.flowWrapper.dataset.legacyLinePlacement = String(
      patchbayFeatures.legacyLinePlacement,
    );
    this.flowWrapper.dataset.nodeCount = String(projectedNodes.length);
    this.flowWrapper.dataset.edgeCount = String(edges.length);

    if (!this.reactRoot) {
      this.reactRoot = window.ReactDOM.createRoot
        ? window.ReactDOM.createRoot(this.flowWrapper)
        : {
            render: (tree) => window.ReactDOM.render(tree, this.flowWrapper),
          };
    }
    this.reactRoot.render(flow);
  }
}
