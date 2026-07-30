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

const e = window.React.createElement;

export class PatchbayReactFlowRenderer {
  constructor(containerElement, options = {}) {
    this.container = containerElement;
    this.options = options;
    this.savedPositions = {};
    this.nodeTypes = {
      conduitFaceplate: FaceplateNodeComponent
    };
    this.reactRoot = null;
    this.currentSource = "";
    this.runtimeState = {};
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

    const cordId = `${source}.${sourceHandle || "out"} -> ${target}.${targetHandle || "in"}`;
    const newCordSource = `\ncord ${source}.${sourceHandle || "out"} -> ${target}.${targetHandle || "in"} {\n    capacity = 4\n    max_value_bytes = 1024\n    max_queued_bytes = 4096\n    low_watermark = 1\n    high_watermark = 4\n    pressure = block\n}\n`;

    if (this.currentSource.includes(`${source}.${sourceHandle || "out"}`) && this.currentSource.includes(`${target}.${targetHandle || "in"}`)) {
      const updatedSource = this.currentSource + newCordSource;
      if (this.onSourceMutation) {
        this.onSourceMutation(updatedSource);
      }
    }
  }

  handleNodeDragStop(evt, node) {
    if (node && node.id && node.position) {
      this.savePosition(node.id, node.position);
    }
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
      data: {
        ...n,
        onConfigChange: (nodeId, key, val) => {
          // Inline config edit
        },
        onOpenNested: (kind) => {
          if (this.onOpenNested) this.onOpenNested(kind);
        }
      }
    }));

    const flowEdges = viewModel.edges.map((e) => ({
      id: e.id,
      source: e.sourceNodeId,
      sourceHandle: e.sourcePortId,
      target: e.targetNodeId,
      targetHandle: e.targetPortId,
      animated: e.connectionState === "active" || e.pressure === "block",
      style: { stroke: "#38bdf8", strokeWidth: 3 },
      label: `${e.capacity} cap`
    }));

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
