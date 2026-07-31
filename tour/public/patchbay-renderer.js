/**
 * Conduit Patchbay React Flow renderer.
 *
 * Conduit supplies semantic and projected graph data. React Flow owns the canvas
 * interactions, selection, and presentation while Rust remains authoritative.
 */

import { FaceplateNodeComponent } from "./patchbay-faceplate.js";
import { patchbayFeatures } from "./patchbay-features.js";

const e = window.React.createElement;

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
  if (normalized.includes("text") || normalized.includes("utf")) {
    return { color: "#34d399", family: "text" };
  }
  if (normalized.includes("bytes") || normalized.includes("binary") ||
      normalized.includes("audio")) {
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

function endpointForView(edge, side, view, anchors, projectedNodeIds) {
  const anchorId = edge[`${side}_anchor`];
  if (anchorId) {
    const anchor = anchors.find((candidate) => candidate.id === anchorId);
    if (anchor?.owner_node && projectedNodeIds.has(anchor.owner_node)) {
      return { node: anchor.owner_node, port: anchor.id };
    }
    return { node: anchorId, port: "diagnostic-anchor" };
  }
  if (view !== "logical") {
    return {
      node: edge[`expanded_${side}_node`] || edge[`${side}_node`],
      port: edge[`expanded_${side}_port`] || edge[`${side}_port`],
    };
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
    this.onSelectionClear = options.onSelectionClear || null;
    this.onOpenNested = options.onOpenNested || null;
    this.legacySmartEdge = null;
  }

  init() {
    this.container.innerHTML = "";
    this.flowWrapper = document.createElement("div");
    this.flowWrapper.id = "patchbay-flow-root";
    this.flowWrapper.className = "patchbay-flow-container";
    this.container.appendChild(this.flowWrapper);

    if (patchbayFeatures.legacyLinePlacement) {
      import("./patchbay-smart-edge.js").then((legacy) => {
        this.legacySmartEdge = legacy.PatchbaySmartEdge || null;
        this.renderFlow();
      });
      return;
    }

    this.renderFlow();
  }

  setViewModel(viewModel, lessonId = "default", topologyView = "logical") {
    this.viewModel = viewModel;
    this.lessonId = lessonId;
    this.topologyView = topologyView;
    const renderIdentity = JSON.stringify([
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

  selectNode(nodeId) {
    this.selectedNodeId = nodeId;
    this.selectedCordId = null;
    this.highlightCordEndpoints(null);
    this.flowWrapper?.querySelectorAll(".conduit-faceplate-card").forEach((card) => {
      const shell = card.closest(".react-flow__node");
      card.classList.toggle("selected-faceplate", shell?.dataset.id === nodeId);
    });
  }

  selectCord(cordId) {
    this.selectedNodeId = null;
    this.selectedCordId = cordId;
    const projectedEdges = this.viewModel?.topology?.cords || [];
    this.flowWrapper?.querySelectorAll(".react-flow__edge").forEach((edge, index) => {
      edge.classList.toggle("selected", this.renderedCordIds?.[index] === cordId);
    });
    this.highlightCordEndpoints(
      projectedEdges.find((edge) => edge.id === cordId) || null,
    );
  }

  highlightCordEndpoints(cord) {
    const selectedPaths = new Set(cord
      ? this.topologyView === "logical"
        ? [cord.from_port_path, cord.to_port_path]
        : [
            `root/${cord.expanded_from_node || cord.from_node}/port/outgoing/` +
              `${cord.expanded_from_port || cord.from_port}`,
            `root/${cord.expanded_to_node || cord.to_node}/port/receiving/` +
              `${cord.expanded_to_port || cord.to_port}`,
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
    if (patchbayFeatures.legacyLinePlacement && !this.legacySmartEdge) {
      return;
    }

    const viewModel = this.viewModel;
    if (!viewModel) {
      this.flowWrapper.dataset.projection = "unavailable";
      return;
    }

    const projectedNodes = this.topologyView === "logical"
      ? viewModel.topology?.logical_nodes || []
      : viewModel.topology?.expanded_nodes || [];
    const projectedNodeIds = new Set(projectedNodes.map((node) => node.id));
    const diagnosticAnchors = viewModel.topology?.diagnostic_anchors || [];
    const projectedEdges = (viewModel.topology?.cords || [])
      .map((edge) => ({
        edge,
        source: endpointForView(
          edge, "from", this.topologyView, diagnosticAnchors, projectedNodeIds,
        ),
        target: endpointForView(
          edge, "to", this.topologyView, diagnosticAnchors, projectedNodeIds,
        ),
      }))
      .filter(({ source, target }) =>
        source && target &&
        (projectedNodeIds.has(source.node) ||
          diagnosticAnchors.some((anchor) => anchor.id === source.node)) &&
        (projectedNodeIds.has(target.node) ||
          diagnosticAnchors.some((anchor) => anchor.id === target.node))
      );
    const nodePositions = viewModel.presentation?.node_positions || {};
    const positionForNode = (nodeId, index) =>
      nodePositions[nodeId] || {
        x: 32 + (index % 2) * 640,
        y: 40 + Math.floor(index / 2) * 280,
      };

    const compositeIds = new Set(
      (viewModel.topology?.composites || []).map((composite) => composite.id),
    );
    const nodes = projectedNodes.map((node, index) => {
      const configRows = Object.keys(node.config || {}).length;
      const portRows = (node.inputs?.length || 0) + (node.outputs?.length || 0);
      const statusRows = [
        node.availability,
        node.placement,
        node.activity,
      ].filter(Boolean).length;
      const nodeHeight = Math.max(
        118,
        76 + configRows * 38 + portRows * 38 + (statusRows > 0 ? 46 : 0),
      );
      return {
        id: node.id,
        type: "faceplate",
        position: positionForNode(node.id, index),
        className: "react-flow-node-shell",
        width: 350,
        height: nodeHeight,
        style: { width: 350, height: nodeHeight },
        data: {
        ...node,
        title: node.id,
        kind: node.contract_id,
        config: node.config || {},
        inputs: node.inputs || [],
        outputs: node.outputs || [],
        status: node.activity || "idle",
        activity: node.activity,
        validity: node.validity,
        diagnosticIds: node.diagnostic_ids || [],
        diagnosticAnchors: diagnosticAnchors.filter(
          (anchor) => anchor.owner_node === node.id,
        ),
        isComposite: compositeIds.has(node.id),
        isSelected: node.id === this.selectedNodeId,
        onConfigChange: (nodeId, key, value, kind) =>
          this.updateConfig(nodeId, key, value, kind),
        onOpenNested: (kind) => {
          if (this.onOpenNested) {
            this.onOpenNested(kind);
          }
        },
        onPortSelect: (nodeId, port) => {
          if (this.onPortSelect) {
            this.onPortSelect(nodeId, port);
          }
        },
        },
        draggable: true,
        selectable: true,
      };
    });
    const standaloneAnchors = diagnosticAnchors
      .filter((anchor) => !anchor.owner_node || !projectedNodeIds.has(anchor.owner_node))
      .map((anchor, index) => ({
        id: anchor.id,
        type: "diagnosticAnchor",
        position: {
          x: 450 + (index % 2) * 300,
          y: 130 + Math.floor(index / 2) * 130,
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
      const edgeType = patchbayFeatures.legacyLinePlacement && this.legacySmartEdge
        ? "patchbaySmartEdge"
        : "smoothstep";
      return {
        id: edge.id,
        source: source.node,
        sourceHandle: source.port,
        target: target.node,
        targetHandle: target.port,
        type: edgeType,
        label: presentation.label,
        markerEnd: {
          type: window.ReactFlow.MarkerType.ArrowClosed,
          color: presentation.color,
          width: 18,
          height: 18,
        },
        className: [
          "patchbay-smart-cord",
          presentation.className,
          edge.id === emphasizedDiagnosticCord ? "diagnostic-emphasized" : "",
        ].join(" "),
        data: {
          presentationClass: `patchbay-smart-cord ${presentation.className}`,
        },
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

    const edgeTypes = {};
    if (patchbayFeatures.legacyLinePlacement && this.legacySmartEdge) {
      edgeTypes.patchbaySmartEdge = (props) =>
        e(this.legacySmartEdge, {
          ...props,
          className: `${props.className || ""} ${props.data?.presentationClass || ""}`,
          markerEnd: props.markerEnd,
          markerStart: props.markerStart,
        });
    }

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
        nodesDraggable: true,
        nodesConnectable: false,
        elementsSelectable: true,
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
          } else if ((this.selectedNodeId || this.selectedCordId) &&
              this.onSelectionClear) {
            this.selectedNodeId = null;
            this.selectedCordId = null;
            this.onSelectionClear();
          }
        },
        onPaneClick: () => {
          this.selectedNodeId = null;
          this.selectedCordId = null;
          if (this.onSelectionClear) this.onSelectionClear();
        },
        onNodeDragStop: (_event, node) => {
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
        onInit: () => {
          this.flowWrapper.dataset.layout = "ready";
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
