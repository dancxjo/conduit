import {
  decodeFlowPresentation,
  encodeFlowPresentation,
  MAX_FLOW_WORKSPACES,
  projectFlowScene,
  reconcileFlowScene,
} from "/assets/flow-scene.js";

const React = window.React;
const ReactDOM = window.ReactDOM;
const Flow = window.ReactFlow;

if (!React || !ReactDOM || !Flow) {
  throw new Error("preserved React Flow presentation assets are unavailable");
}

const e = React.createElement;
const ReactFlow = Flow.default || Flow.ReactFlow || Flow;
let instance = null;
let root = null;
let currentScene = null;
const workspaceIndexKey = "conduit.patchbay.flow/workspaces";

function storageKey(workspaceIdentity) {
  return `conduit.patchbay.flow/${encodeURIComponent(workspaceIdentity)}`;
}

function restore(projection) {
  const document = sessionStorage.getItem(storageKey(projection.workspaceIdentity));
  return decodeFlowPresentation(document, projection);
}

function retainWorkspace(workspaceIdentity) {
  let identities = [];
  try {
    const document = sessionStorage.getItem(workspaceIndexKey) || "[]";
    const parsed = document.length <= 64 * 1024 ? JSON.parse(document) : [];
    if (Array.isArray(parsed)) {
      identities = parsed
        .filter((identity) => typeof identity === "string")
        .slice(0, MAX_FLOW_WORKSPACES);
    }
  } catch { identities = []; }
  identities = [workspaceIdentity, ...identities.filter((identity) => identity !== workspaceIdentity)];
  for (const evicted of identities.slice(MAX_FLOW_WORKSPACES)) sessionStorage.removeItem(storageKey(evicted));
  sessionStorage.setItem(workspaceIndexKey, JSON.stringify(identities.slice(0, MAX_FLOW_WORKSPACES)));
}

function persist(scene, viewport = instance?.getViewport() || scene.viewport) {
  currentScene = { ...scene, viewport };
  retainWorkspace(scene.workspaceIdentity);
  sessionStorage.setItem(storageKey(scene.workspaceIdentity), encodeFlowPresentation(currentScene));
}

function presentEdges(edges) {
  return edges.map((edge) => ({
    ...edge,
    className: "flow-cord",
    markerEnd: { type: Flow.MarkerType.ArrowClosed },
  }));
}

function Workspace({ snapshot, onSelect, onClear }) {
  const projected = projectFlowScene(snapshot);
  const initial = React.useMemo(() => {
    const restored = restore(projected);
    return reconcileFlowScene(projected, restored);
  }, [projected.workspaceIdentity]);
  const [nodes, setNodes] = React.useState(initial.nodes);
  const [edges, setEdges] = React.useState(presentEdges(initial.edges));
  const workspace = React.useRef(projected.workspaceIdentity);
  const mounted = React.useRef(false);
  React.useEffect(() => {
    if (!mounted.current) {
      mounted.current = true;
      return;
    }
    setNodes((current) => {
      const sameWorkspace = workspace.current === projected.workspaceIdentity;
      const prior = sameWorkspace
        ? { nodes: current, viewport: instance?.getViewport() || initial.viewport }
        : restore(projected);
      const next = reconcileFlowScene(projected, prior);
      if (!sameWorkspace) instance?.setViewport(next.viewport, { duration: 0 });
      workspace.current = projected.workspaceIdentity;
      persist(next);
      return next.nodes;
    });
    setEdges(presentEdges(projected.edges));
  }, [projected.workspaceIdentity, snapshot.presentation.identity, snapshot.presentation.revision, snapshot.interaction.revision]);
  return e(
    ReactFlow,
    {
      nodes,
      edges,
      onNodesChange: (changes) => setNodes((current) => {
        const nextNodes = Flow.applyNodeChanges(changes, current);
        persist({
          ...projected,
          nodes: nextNodes,
          edges,
          viewport: instance?.getViewport() || initial.viewport,
        });
        return nextNodes;
      }),
      onNodeClick: (_event, node) => onSelect(node.id),
      onPaneClick: onClear,
      onNodeDragStop: (_event, node) => {
        const next = { ...projected, nodes: nodes.map((current) => current.id === node.id ? { ...current, position: { ...node.position } } : current), edges, viewport: instance?.getViewport() || initial.viewport };
        persist(next);
      },
      onMoveEnd: (_event, viewport) => persist({ ...projected, nodes, edges, viewport }, viewport),
      onInit: (next) => {
        instance = next;
        next.setViewport(initial.viewport, { duration: 0 });
        currentScene = { ...projected, nodes, edges, viewport: initial.viewport };
      },
      nodesDraggable: true,
      nodesConnectable: false,
      elementsSelectable: true,
      panOnDrag: true,
      zoomOnScroll: true,
      zoomOnPinch: true,
      minZoom: 0.2,
      maxZoom: 3,
      defaultViewport: initial.viewport,
      fitView: restore(projected) === null,
      fitViewOptions: { maxZoom: 1.1, padding: 0.18 },
      proOptions: { hideAttribution: false },
    },
    e(Flow.Background, { gap: 24, size: 1 }),
    e(Flow.Controls, { position: "bottom-right", showInteractive: false }),
  );
}

export function renderFlow(snapshot, handlers) {
  const target = document.querySelector("#flow-root");
  if (!root) {
    root = ReactDOM.createRoot
      ? ReactDOM.createRoot(target)
      : { render: (tree) => ReactDOM.render(tree, target) };
  }
  root.render(e(Workspace, { snapshot, ...handlers }));
  target.dataset.renderer = "react-flow";
  target.dataset.presentationId = snapshot.presentation.identity;
  target.dataset.presentationRevision = String(snapshot.presentation.revision);
}

export function fitFlow() {
  return instance?.fitView({ duration: 0, maxZoom: 1.1, padding: 0.18 });
}

export function flowViewport() {
  return instance?.getViewport() || null;
}

export function flowSceneSnapshot() {
  return currentScene;
}
