/**
 * Conduit Patchbay Reaflow renderer.
 *
 * Conduit supplies semantic view data. Reaflow exclusively owns automatic
 * layout, edge geometry, viewport interaction, selection, and node dragging.
 */

import {
  Canvas,
  Edge,
  MarkerArrow,
  Node
} from "./reaflow.bundle.mjs";
import { patchbayFeatures } from "./patchbay-features.js";
import { FaceplateNodeComponent } from "./patchbay-faceplate.js";
import { parsePanelToViewModel } from "./patchbay-view-adapter.js";

const e = window.React.createElement;
const NODE_WIDTH = 300;
const NODE_HEIGHT = 240;

function edgePresentation(edge) {
  const pressure = edge.pressure.replaceAll("_", "-");
  const type = edge.valueType.replace(/^conduit\//, "").replace(/[^a-z0-9-]/gi, "-").toLowerCase();
  const colors = {
    any: "#38bdf8",
    bytes: "#22d3ee",
    text: "#34d399",
    utf8: "#34d399",
    json: "#c084fc",
    "http-req": "#f59e0b",
    "http-res": "#fb7185"
  };
  return {
    color: colors[type] || "#60a5fa",
    className: `pressure-${pressure} value-type-${type}`,
    label: `${edge.valueType} · ${edge.capacity} cap · ${edge.pressure}`
  };
}

export class PatchbayReaflowRenderer {
  constructor(containerElement, options = {}) {
    this.container = containerElement;
    this.options = options;
    this.reactRoot = null;
    this.currentSource = "";
    this.runtimeState = {};
    this.selectedNodeId = null;
    this.onSourceMutation = options.onSourceMutation || null;
    this.onNodeSelect = options.onNodeSelect || null;
    this.onOpenNested = options.onOpenNested || null;
  }

  init() {
    this.container.innerHTML = "";
    this.flowWrapper = document.createElement("div");
    this.flowWrapper.id = "patchbay-flow-root";
    this.flowWrapper.className = "patchbay-flow-container";
    this.container.appendChild(this.flowWrapper);

    if (patchbayFeatures.legacyLinePlacement) {
      import("./patchbay-smart-edge.js").then((legacyLinePlacement) => {
        this.legacyLinePlacement = legacyLinePlacement;
      });
    }
  }

  setSource(sourceText, runtimeState = {}, lessonId = "default") {
    this.currentSource = sourceText;
    this.runtimeState = runtimeState;
    this.lessonId = lessonId;
    this.renderFlow();
  }

  selectNode(nodeId) {
    this.selectedNodeId = nodeId;
    this.renderFlow();
  }

  moveNode() {
    // Reaflow owns presentation positions. The toolbar remains a selection
    // affordance until its controls are redesigned around Reaflow's viewport.
  }

  nodeData(node) {
    return {
      id: node.id,
      text: node.title,
      width: NODE_WIDTH,
      height: NODE_HEIGHT,
      className: "reaflow-node-shell",
      data: { id: node.id }
    };
  }

  updateConfig(nodeId, key, value) {
    if (!this.onSourceMutation || !this.currentSource) return;
    const escaped = value.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
    const pattern = new RegExp(
      `(node\\s+${nodeId}\\s*:[^{]*\\{[^}]*?)${key}\\s*=\\s*"[^"]*"`,
      "s"
    );
    const updated = this.currentSource.replace(pattern, `$1${key} = "${escaped}"`);
    if (updated !== this.currentSource) this.onSourceMutation(updated);
  }

  renderFlow() {
    if (!window.React || !window.ReactDOM) {
      this.container.innerHTML = '<div class="card-subtitle error-text">Reaflow libraries unavailable.</div>';
      return;
    }

    const viewModel = parsePanelToViewModel(this.currentSource, this.runtimeState);
    const nodesById = new Map(viewModel.nodes.map((node) => [node.id, node]));
    const nodes = viewModel.nodes.map((node) => this.nodeData(node));
    const edgePresentations = new Map();
    const edges = viewModel.edges.map((edge) => {
      const presentation = edgePresentation(edge);
      edgePresentations.set(edge.id, presentation);
      return {
        id: edge.id,
        from: edge.sourceNodeId,
        to: edge.targetNodeId,
        text: presentation.label,
        className: `reaflow-cord patchbay-smart-cord ${presentation.className}`,
        data: { id: edge.id }
      };
    });

    const nodeRenderer = (props) => e(Node, {
      ...props,
      style: { fill: "transparent", stroke: "transparent" },
      onClick: (_event, node) => {
        this.selectedNodeId = node.id;
        if (this.onNodeSelect) this.onNodeSelect(node.id);
      }
    }, e("foreignObject", {
      x: 0,
      y: 0,
      width: props.width,
      height: props.height,
      className: "reaflow-node-shell"
    }, e(FaceplateNodeComponent, {
      id: props.properties.id,
      data: {
        ...nodesById.get(props.properties.id),
        isSelected: props.properties.id === this.selectedNodeId,
        onConfigChange: (nodeId, key, value) =>
          this.updateConfig(nodeId, key, value),
        onOpenNested: (kind) => {
          if (this.onOpenNested) this.onOpenNested(kind);
        }
      }
    })));

    const edgeRenderer = (props) => {
      const presentation = edgePresentations.get(props.properties.id);
      return e(Edge, {
        ...props,
        className: `reaflow-cord patchbay-smart-cord ${presentation.className}`,
        style: { stroke: presentation.color, strokeWidth: 2 }
      });
    };

    const FlowApp = () => e(Canvas, {
      nodes,
      edges,
      selections: this.selectedNodeId ? [this.selectedNodeId] : [],
      node: nodeRenderer,
      edge: edgeRenderer,
      arrow: e(MarkerArrow, { style: { fill: "#64748b" } }),
      direction: "RIGHT",
      fit: true,
      pannable: true,
      panType: "drag",
      zoomable: true,
      readonly: true,
      onLayoutChange: () => {
        this.flowWrapper.dataset.layout = "ready";
      },
      maxWidth: this.flowWrapper.clientWidth || 1200,
      maxHeight: this.flowWrapper.clientHeight || 640
    });

    this.flowWrapper.dataset.renderer = "reaflow";
    this.flowWrapper.dataset.legacyLinePlacement =
      String(patchbayFeatures.legacyLinePlacement);
    this.flowWrapper.dataset.nodeCount = String(nodes.length);
    this.flowWrapper.dataset.edgeCount = String(edges.length);
    if (!this.reactRoot) {
      this.reactRoot = window.ReactDOM.createRoot
        ? window.ReactDOM.createRoot(this.flowWrapper)
        : { render: (tree) => window.ReactDOM.render(tree, this.flowWrapper) };
    }
    this.reactRoot.render(e(FlowApp));
  }
}
