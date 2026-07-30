/**
 * Conduit Patchbay React Flow Renderer (#90, #99, #91)
 *
 * Implements clean React Flow integration boundary:
 * - DERIVES React Flow nodes and edges deterministically from Conduit view adapter.
 * - OWNS presentation layout, viewport pan/zoom, selection, and drag states.
 * - MUTATES authoritative Conduit graph via explicit protocol operations.
 */

import { parsePanelToViewModel } from "./patchbay-view-adapter.js";
import { FaceplateNodeComponent } from "./patchbay-faceplate.js";
import { PatchbaySmartEdge } from "./patchbay-smart-edge.js";

const e = window.React.createElement;

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

export class PatchbayReactFlowRenderer {
  constructor(containerElement, options = {}) {
    this.container = containerElement;
    this.options = options;
    this.savedPositions = {};
    this.nodeTypes = {
      conduitFaceplate: FaceplateNodeComponent
    };
    this.edgeTypes = {
      smartCord: PatchbaySmartEdge
    };
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
    const flowWrapper = document.createElement("div");
    flowWrapper.id = "patchbay-flow-root";
    flowWrapper.className = "patchbay-flow-container";
    flowWrapper.style.width = "100%";
    flowWrapper.style.height = "100%";
    flowWrapper.style.position = "relative";
    this.container.appendChild(flowWrapper);

    this.flowWrapper = flowWrapper;
    this.renderFlow();
  }

  setSource(sourceText, runtimeState = {}, lessonId = "default") {
    this.currentSource = sourceText;
    this.runtimeState = runtimeState;
    this.lessonId = lessonId;

    // Load saved presentation positions
    try {
      const stored = localStorage.getItem(`conduit-tour-layout/${lessonId}`);
      if (stored) {
        this.savedPositions = JSON.parse(stored);
      }
    } catch (_) {}

    this.renderFlow();
  }

  savePosition(nodeId, pos) {
    this.savedPositions[nodeId] = pos;
    try {
      localStorage.setItem(`conduit-tour-layout/${this.lessonId}`, JSON.stringify(this.savedPositions));
    } catch (_) {}
  }

  handleConnect(params) {
    const { source, sourceHandle, target, targetHandle } = params;
    if (!source || !target || source === target) {
      if (this.options.onNotification) {
        this.options.onNotification("Invalid cord connection: cannot connect node to itself.");
      }
      return;
    }

    const srcPort = sourceHandle || "out";
    const tgtPort = targetHandle || "in";
    const cordDeclaration = `cord ${source}.${srcPort} -> ${target}.${tgtPort}`;

    // Prevent duplicate cords
    if (this.currentSource.includes(cordDeclaration)) {
      if (this.options.onNotification) {
        this.options.onNotification(`Cord ${source}.${srcPort} → ${target}.${tgtPort} already exists.`);
      }
      return;
    }

    const newCordSource = `\ncord ${source}.${srcPort} -> ${target}.${tgtPort} {\n    capacity = 4\n    max_value_bytes = 1024\n    max_queued_bytes = 4096\n    low_watermark = 1\n    high_watermark = 4\n    pressure = block\n}\n`;

    const updatedSource = this.currentSource + newCordSource;
    if (this.onSourceMutation) {
      this.onSourceMutation(updatedSource);
    }
  }

  handleNodeDragStop(evt, node) {
    if (node && node.id && node.position) {
      this.savePosition(node.id, node.position);
    }
  }

  selectNode(nodeId) {
    this.selectedNodeId = nodeId;
    this.renderFlow();
  }

  moveNode(nodeId, position) {
    this.savePosition(nodeId, position);
    this.renderFlow();
  }

  renderFlow() {
    if (!window.React || !window.ReactDOM || !window.ReactFlow) {
      this.container.innerHTML = `<div class="card-subtitle error-text">React Flow libraries unavailable.</div>`;
      return;
    }

    const viewModel = parsePanelToViewModel(this.currentSource, this.runtimeState, this.savedPositions);

    const flowNodes = viewModel.nodes.map((n) => ({
      id: n.id,
      type: "conduitFaceplate",
      position: n.position,
      selected: n.id === this.selectedNodeId,
      data: {
        ...n,
        isSelected: n.id === this.selectedNodeId,
        onConfigChange: (nodeId, key, val) => {
          if (this.onSourceMutation && this.currentSource) {
            // Replace the config value in the .panel source text
            const escaped = val.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
            const pattern = new RegExp(
              `(node\\s+${nodeId}\\s*:[^{]*\\{[^}]*?)${key}\\s*=\\s*"[^"]*"`,
              "s"
            );
            const updated = this.currentSource.replace(pattern, `$1${key} = "${escaped}"`);
            if (updated !== this.currentSource) {
              this.onSourceMutation(updated);
            }
          }
        },
        onOpenNested: (kind) => {
          if (this.onOpenNested) this.onOpenNested(kind);
        }
      }
    }));

    const flowEdges = viewModel.edges.map((edge) => {
      const presentation = edgePresentation(edge);
      return {
        id: edge.id,
        type: "smartCord",
        source: edge.sourceNodeId,
        sourceHandle: edge.sourcePortId,
        target: edge.targetNodeId,
        targetHandle: edge.targetPortId,
        animated: false,
        className: presentation.className,
        data: { presentationClass: presentation.className },
        style: { stroke: presentation.color, strokeWidth: 2 },
        labelStyle: { fill: "#cbd5e1", fontSize: 10 },
        labelBgStyle: { fill: "#0f172a", fillOpacity: 0.92 },
        labelBgPadding: [6, 3],
        label: presentation.label
      };
    });

    const FlowApp = () => {
      const [nodes, setNodes, onNodesChange] = window.ReactFlow.useNodesState(flowNodes);
      const [edges, setEdges, onEdgesChange] = window.ReactFlow.useEdgesState(flowEdges);

      window.React.useEffect(() => {
        setNodes(flowNodes);
        setEdges(flowEdges);
      }, [this.currentSource, this.runtimeState]);

      return e(window.ReactFlow.default || window.ReactFlow, {
        nodes,
        edges,
        nodeTypes: this.nodeTypes,
        edgeTypes: this.edgeTypes,
        onNodesChange,
        onEdgesChange,
        onConnect: (params) => this.handleConnect(params),
        onNodeDragStop: (evt, node) => this.handleNodeDragStop(evt, node),
        onNodeClick: (evt, node) => {
          if (this.onNodeSelect) this.onNodeSelect(node.id);
        },
        fitView: true,
        attributionPosition: "bottom-left"
      },
        e(window.ReactFlow.Controls, { className: "flow-controls" }),
        e(window.ReactFlow.Background, { color: "#1e293b", gap: 16 })
      );
    };

    if (!this.reactRoot) {
      if (window.ReactDOM.createRoot) {
        this.reactRoot = window.ReactDOM.createRoot(this.flowWrapper);
        this.reactRoot.render(e(FlowApp));
      } else {
        window.ReactDOM.render(e(FlowApp), this.flowWrapper);
      }
    } else {
      if (this.reactRoot.render) {
        this.reactRoot.render(e(FlowApp));
      } else {
        window.ReactDOM.render(e(FlowApp), this.flowWrapper);
      }
    }
  }
}
