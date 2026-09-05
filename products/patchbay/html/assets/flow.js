import {
  decodeFlowPresentation,
  encodeFlowPresentation,
  MAX_FLOW_WORKSPACES,
  projectFlowScene,
  reconcileFlowScene,
} from "./flow-scene.js";
import { FaceplateNode } from "./flow-faceplate.js";

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
const roots = new WeakMap();
let currentScene = null;
let arrangeCurrent = null;
const workspaceIndexKey = "conduit.patchbay.flow/workspaces";
const nodeTypes = { faceplate: FaceplateNode };
const retainedScenes = new Map();
const loadedWorkspaces = new Set();
let admittedStorage = null;
let storageWrites = Promise.resolve();

function rootFor(target) {
  let mounted = roots.get(target);
  if (!mounted) {
    mounted = ReactDOM.createRoot
      ? ReactDOM.createRoot(target)
      : { render: (tree) => ReactDOM.render(tree, target) };
    roots.set(target, mounted);
  }
  return mounted;
}

function storageKey(workspaceIdentity) {
  return `conduit.patchbay.flow/${encodeURIComponent(workspaceIdentity)}`;
}

function restore(projection) {
  const document = retainedScenes.get(storageKey(projection.workspaceIdentity)) ?? null;
  return decodeFlowPresentation(document, projection);
}

async function retainWorkspace(workspaceIdentity) {
  const parsed = await admittedStorage.readJson(workspaceIndexKey);
  let identities = Array.isArray(parsed)
    ? parsed.filter((identity) => typeof identity === "string").slice(0, MAX_FLOW_WORKSPACES)
    : [];
  identities = [workspaceIdentity, ...identities.filter((identity) => identity !== workspaceIdentity)];
  for (const evicted of identities.slice(MAX_FLOW_WORKSPACES)) {
    retainedScenes.delete(storageKey(evicted));
    await admittedStorage.deleteJson(storageKey(evicted));
  }
  await admittedStorage.writeJson(workspaceIndexKey, identities.slice(0, MAX_FLOW_WORKSPACES));
}

function persist(scene, viewport = instance?.getViewport() || scene.viewport) {
  currentScene = { ...scene, viewport };
  if (admittedStorage && !loadedWorkspaces.has(scene.workspaceIdentity)) return;
  const key = storageKey(scene.workspaceIdentity);
  const document = encodeFlowPresentation(currentScene);
  retainedScenes.set(key, document);
  if (admittedStorage) {
    storageWrites = storageWrites.then(async () => {
      await retainWorkspace(scene.workspaceIdentity);
      await admittedStorage.writeJson(key, document);
    }).catch((error) => { currentScene = { ...currentScene, storageRefusal: error.code ?? "StorageFailure" }; });
  }
}

export function configureFlowStorage(storage) {
  if (storage?.schema !== "conduit.browser/application-storage@1") {
    throw new Error("Patchbay requires admitted application storage");
  }
  admittedStorage = storage;
}

function presentEdges(edges) {
  return edges.map((edge) => ({
    ...edge,
    className: `flow-cord ${edge.className || ""}`.trim(),
    markerEnd: { type: Flow.MarkerType.ArrowClosed },
  }));
}

function Workspace({ snapshot, onSelect, onConnect, onClear, onOpenBack, lens, selectionGroup }) {
  const [openedBacks, setOpenedBacks] = React.useState(() => new Set());
  React.useEffect(() => setOpenedBacks(new Set()), [
    snapshot.presentation.basis.checked_form_id,
    snapshot.presentation.basis.expanded_form_id,
  ]);
  const projected = projectFlowScene(snapshot, lens, openedBacks);
  const initial = React.useMemo(() => {
    const restored = restore(projected);
    return reconcileFlowScene(projected, restored);
  }, [projected.workspaceIdentity]);
  const [nodes, setNodes] = React.useState(initial.nodes);
  const [edges, setEdges] = React.useState(presentEdges(initial.edges));
  const workspace = React.useRef(projected.workspaceIdentity);
  const mounted = React.useRef(false);
  React.useEffect(() => {
    let active = true;
    if (admittedStorage && !retainedScenes.has(storageKey(projected.workspaceIdentity))) {
      admittedStorage.readJson(storageKey(projected.workspaceIdentity)).then((document) => {
        loadedWorkspaces.add(projected.workspaceIdentity);
        if (!active) return;
        if (typeof document !== "string") {
          if (currentScene?.workspaceIdentity === projected.workspaceIdentity) persist(currentScene);
          return;
        }
        retainedScenes.set(storageKey(projected.workspaceIdentity), document);
        const next = reconcileFlowScene(projected, decodeFlowPresentation(document, projected));
        currentScene = next;
        setNodes(next.nodes);
        instance?.setViewport(next.viewport, { duration: 0 });
      }).catch((error) => { currentScene = { ...currentScene, storageRefusal: error.code ?? "StorageFailure" }; });
    }
    return () => { active = false; };
  }, [projected.workspaceIdentity]);
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
  }, [projected.workspaceIdentity, projected.lens, openedBacks, snapshot.presentation.identity, snapshot.presentation.revision, snapshot.interaction.revision, snapshot.debugger?.revision, snapshot.timeline?.revision]);
  arrangeCurrent = () => {
    const next = reconcileFlowScene(projected);
    setNodes(next.nodes);
    persist(next);
    requestAnimationFrame(() => instance?.fitView({ duration: 0, maxZoom: 1.1, padding: 0.18 }));
  };
  const presentedNodes = nodes.map((node) => ({
    ...node,
    data: {
      ...node.data,
      onActivate: onSelect,
      onOpenBack: (identity) => {
        setOpenedBacks((current) => {
          const next = new Set(current);
          if (next.has(identity)) next.delete(identity); else next.add(identity);
          return next;
        });
        onOpenBack?.(identity);
      },
      selectionGroup,
    },
  }));
  const liveSummary = (snapshot.debugger?.activities || [])
    .slice(-8)
    .map((activity) => `${activity.subject}: ${activity.latest_kind}${activity.latest_value ? ` ${activity.latest_value.summary}` : ""}`)
    .join("; ");
  return e(
    React.Fragment,
    null,
    e(ReactFlow, {
      nodes: presentedNodes,
      edges,
      nodeTypes,
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
      onPaneClick: onClear,
      onConnect: (connection) => {
        if (connection.sourceHandle && connection.targetHandle) onConnect(connection.sourceHandle, connection.targetHandle);
      },
      onNodeDragStop: (_event, node) => {
        const next = { ...projected, nodes: nodes.map((current) => current.id === node.id ? { ...current, position: { ...node.position } } : current), edges, viewport: instance?.getViewport() || initial.viewport };
        persist(next);
      },
      onMoveEnd: (_event, viewport) => persist({ ...projected, nodes, edges, viewport }, viewport),
      onInit: (next) => {
        instance = next;
        const viewport = currentScene?.workspaceIdentity === projected.workspaceIdentity
          ? currentScene.viewport
          : initial.viewport;
        const initializedNodes = currentScene?.workspaceIdentity === projected.workspaceIdentity
          ? currentScene.nodes
          : nodes;
        next.setViewport(viewport, { duration: 0 });
        currentScene = { ...projected, nodes: initializedNodes, edges, viewport };
      },
      nodesDraggable: true,
      nodesConnectable: Boolean(snapshot.authoring),
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
    ),
    e("p", { className: "debugger-live-region", "aria-live": "polite", "aria-atomic": "true" }, liveSummary),
  );
}

export function renderFlow(snapshot, handlers) {
  const target = handlers.target || document.querySelector("#flow-root");
  if (!target) throw new Error("Patchbay flow target is unavailable");
  root = rootFor(target);
  const { target: _target, ...workspaceHandlers } = handlers;
  root.render(e(Workspace, {
    snapshot,
    ...workspaceHandlers,
    selectionGroup: `patchbay-flow-${target.id || snapshot.presentation.identity}`,
  }));
  target.dataset.renderer = "react-flow";
  target.dataset.presentationId = snapshot.presentation.identity;
  target.dataset.presentationRevision = String(snapshot.presentation.revision);
}

export function renderFlowRefusal(target, message) {
  rootFor(target).render(e("p", { className: "compact-patchbay-refusal", role: "status" }, message));
  delete target.dataset.renderer;
}

export function fitFlow() {
  return instance?.fitView({ duration: 0, maxZoom: 1.1, padding: 0.18 });
}

export function focusFlow(subjectIdentity) {
  const node = instance?.getNode(subjectIdentity);
  if (!node) return false;
  const position = node.positionAbsolute || node.position;
  instance.setCenter(
    position.x + (node.width || 240) / 2,
    position.y + (node.height || 96) / 2,
    { duration: 0, zoom: 0.85 },
  );
  return true;
}

export function arrangeFlow() {
  arrangeCurrent?.();
}

export function flowViewport() {
  return instance?.getViewport() || null;
}

export async function flowStorageSettled() {
  await storageWrites;
  return currentScene?.storageRefusal ?? "Stored";
}

export function zoomFlow(factor) {
  const viewport = instance?.getViewport();
  if (!viewport) return;
  instance.setViewport({ ...viewport, zoom: Math.max(0.2, Math.min(3, viewport.zoom * factor)) }, { duration: 0 });
}

export function panFlow(x, y) {
  const viewport = instance?.getViewport();
  if (!viewport) return;
  instance.setViewport({ ...viewport, x: viewport.x + x, y: viewport.y + y }, { duration: 0 });
}

export function flowSceneSnapshot() {
  return currentScene;
}
