/**
 * Conduit Patchbay React Flow renderer.
 *
 * Conduit supplies semantic and projected graph data. React Flow owns the canvas
 * interactions, selection, and presentation while Rust remains authoritative.
 */

import { FaceplateNodeComponent } from "./patchbay-faceplate.js";
import { patchbayFeatures } from "./patchbay-features.js";

const e = window.React.createElement;

function edgePresentation(edge) {
  const pressure = edge.pressure
    .replace(/[^a-z0-9]+/gi, "-")
    .replace(/^-|-$/g, "")
    .toLowerCase();
  const type = edge.value_type
    .replace(/^conduit\//, "")
    .replace(/[^a-z0-9-]/gi, "-")
    .toLowerCase();
  const colors = {
    any: "#38bdf8",
    bytes: "#22d3ee",
    text: "#34d399",
    utf8: "#34d399",
    json: "#c084fc",
    "http-req": "#f59e0b",
    "http-res": "#fb7185",
  };

  return {
    color: colors[type] || "#60a5fa",
    className: `pressure-${pressure} value-type-${type}`,
    label: `${edge.value_type} · ${edge.capacity_items} cap · ${edge.pressure}`,
  };
}

export class PatchbayReactFlowRenderer {
  constructor(containerElement, options = {}) {
    this.container = containerElement;
    this.options = options;
    this.reactRoot = null;
    this.viewModel = null;
    this.selectedNodeId = null;
    this.onTransaction = options.onTransaction || null;
    this.onNodeSelect = options.onNodeSelect || null;
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
    }
  }

  setViewModel(viewModel, lessonId = "default") {
    this.viewModel = viewModel;
    this.lessonId = lessonId;
    this.renderFlow();
  }

  selectNode(nodeId) {
    this.selectedNodeId = nodeId;
    this.renderFlow();
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
    });
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
      return;
    }

    const projectedNodes = viewModel.topology?.expanded_nodes || [];
    const projectedEdges = viewModel.topology?.cords || [];
    const nodePositions = viewModel.presentation?.node_positions || {};
    const positionForNode = (nodeId, index) =>
      nodePositions[nodeId] || {
        x: 32 + (index % 2) * 320,
        y: 40 + Math.floor(index / 2) * 220,
      };

    const compositeIds = new Set(
      (viewModel.topology?.composites || []).map((composite) => composite.id),
    );
    const nodes = projectedNodes.map((node, index) => {
      const configRows = Object.keys(node.config || {}).length;
      const portRows = Math.max(node.inputs?.length || 0, node.outputs?.length || 0);
      const nodeHeight = Math.max(160, 132 + Math.max(configRows * 34, portRows * 36));
      return {
        id: node.id,
        type: "faceplate",
        position: positionForNode(node.id, index),
        className: "react-flow-node-shell",
        width: 280,
        height: nodeHeight,
        style: { width: 280, height: nodeHeight },
        data: {
        ...node,
        title: node.id,
        kind: node.contract_id,
        config: node.config || {},
        inputs: node.inputs || [],
        outputs: node.outputs || [],
        status: node.activity || "idle",
        activity: node.activity,
        isComposite: compositeIds.has(node.id),
        isSelected: node.id === this.selectedNodeId,
        onConfigChange: (nodeId, key, value, kind) =>
          this.updateConfig(nodeId, key, value, kind),
        onOpenNested: (kind) => {
          if (this.onOpenNested) {
            this.onOpenNested(kind);
          }
        },
        },
        draggable: true,
        selectable: true,
      };
    });

    const edges = projectedEdges.map((edge) => {
      const presentation = edgePresentation(edge);
      const edgeType = patchbayFeatures.legacyLinePlacement && this.legacySmartEdge
        ? "patchbaySmartEdge"
        : "smoothstep";
      return {
        id: edge.id,
        source: edge.from_node,
        sourceHandle: edge.from_port,
        target: edge.to_node,
        targetHandle: edge.to_port,
        type: edgeType,
        label: presentation.label,
        className: `patchbay-smart-cord ${presentation.className}`,
        data: {
          presentationClass: `patchbay-smart-cord ${presentation.className}`,
        },
        style: {
          stroke: presentation.color,
          strokeWidth: 2,
        },
        animated: false,
      };
    });

    const ReactFlowRenderer =
      window.ReactFlow.default || window.ReactFlow.ReactFlow || window.ReactFlow;
    const FaceplateNode = ({ data }) => e(
      "div",
      { className: "react-flow-node-shell" },
      e(FaceplateNodeComponent, { id: data.id, data }),
    );

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

    const FlowApp = () => e(
      ReactFlowRenderer,
      {
        nodes,
        edges,
        nodeTypes: { faceplate: FaceplateNode },
        edgeTypes,
        edgesSelectable: true,
        nodesDraggable: true,
        nodesConnectable: false,
        elementsSelectable: true,
        onNodeClick: (_event, node) => {
          this.selectedNodeId = node.id;
          if (this.onNodeSelect) {
            this.onNodeSelect(node.id);
          }
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
    this.flowWrapper.dataset.projection = "rust-authoritative-v1";
    this.flowWrapper.dataset.legacyLinePlacement = String(
      patchbayFeatures.legacyLinePlacement,
    );
    this.flowWrapper.dataset.nodeCount = String(projectedNodes.length);
    this.flowWrapper.dataset.edgeCount = String(projectedEdges.length);

    if (!this.reactRoot) {
      this.reactRoot = window.ReactDOM.createRoot
        ? window.ReactDOM.createRoot(this.flowWrapper)
        : {
            render: (tree) => window.ReactDOM.render(tree, this.flowWrapper),
          };
    }
    this.reactRoot.render(e(FlowApp));
  }
}
