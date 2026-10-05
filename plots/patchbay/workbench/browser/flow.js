import {
  MAX_FLOW_WORKSPACES,
  projectFlowScene,
  reconcileFlowScene,
} from "./flow-scene.js";
import { FaceplateNode } from "./flow-frontplate.js";
import {
  applyWorkspace, captureWorkspace, decodeWorkspace,
  validateWorkspace, workspaceCorrelation, workspaceLayout,
} from "./flow-workspace.js";
import { WorkspaceDecoration, WorkspaceManualEdge, workspaceDecorations } from "./flow-workspace-view.js";

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
const nodeTypes = { faceplate: FaceplateNode, workspaceDecoration: WorkspaceDecoration };
const edgeTypes = { workspaceManual: WorkspaceManualEdge };
const retainedScenes = new Map();
const loadedWorkspaces = new Set();
let admittedStorage = null;
let storageWrites = Promise.resolve();
const workspaceDocuments = new Map();
const workspaceRefusals = new Map();
let workspaceValidator = async (document) => validateWorkspace(document);
let applyCurrent = null;

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
  if (!projection.basis) return null;
  try {
    const encoded = retainedScenes.get(storageKey(projection.workspaceIdentity));
    const document = workspaceDocuments.get(projection.workspaceIdentity)
      || (encoded ? decodeWorkspace(encoded, projection) : null);
    if (!document) return null;
    workspaceDocuments.set(projection.workspaceIdentity, document);
    return applyWorkspace(document, reconcileFlowScene(projection));
  } catch (error) {
    workspaceRefusals.set(projection.workspaceIdentity, error.code || error.message);
    return null;
  }
}

async function retainWorkspace(workspaceIdentity) {
  const parsed = await admittedStorage.readJson(workspaceIndexKey);
  let identities = Array.isArray(parsed)
    ? parsed.filter((identity) => typeof identity === "string").slice(0, MAX_FLOW_WORKSPACES)
    : [];
  identities = [workspaceIdentity, ...identities.filter((identity) => identity !== workspaceIdentity)];
  for (const evicted of identities.slice(MAX_FLOW_WORKSPACES)) {
    retainedScenes.delete(storageKey(evicted));
    workspaceDocuments.delete(evicted);
    workspaceRefusals.delete(evicted);
    loadedWorkspaces.delete(evicted);
    await admittedStorage.deleteJson(storageKey(evicted));
  }
  await admittedStorage.writeJson(workspaceIndexKey, identities.slice(0, MAX_FLOW_WORKSPACES));
}

function persist(scene, viewport = instance?.getViewport() || scene.viewport) {
  currentScene = { ...scene, viewport };
  if (!scene.basis) return;
  if (admittedStorage && !loadedWorkspaces.has(scene.workspaceIdentity)) return;
  if (workspaceRefusals.has(scene.workspaceIdentity)) return;
  const key = storageKey(scene.workspaceIdentity);
  let document;
  try {
    document = captureWorkspace(currentScene, workspaceDocuments.get(scene.workspaceIdentity));
    workspaceDocuments.set(scene.workspaceIdentity, document);
    currentScene = { ...currentScene, storageRefusal: null, workspace: document, workspaceCorrelation: workspaceCorrelation(document, scene) };
  } catch (error) {
    currentScene = { ...currentScene, storageRefusal: error.code || error.message };
    return;
  }
  const encoded = JSON.stringify(document);
  retainedScenes.set(key, encoded);
  if (admittedStorage) {
    storageWrites = storageWrites.then(async () => {
      await workspaceValidator(document);
      await retainWorkspace(scene.workspaceIdentity);
      await admittedStorage.writeJson(key, encoded);
    }).catch((error) => { currentScene = { ...currentScene, storageRefusal: error.code ?? "StorageFailure" }; });
  }

}

export function configureFlowWorkspaceValidator(validate) {
  if (typeof validate !== "function") throw new Error("Workspace validator must be callable");
  workspaceValidator = validate;
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

function Workspace({ snapshot, onSelect, onConnect, onConnectStart, onClear, onOpenBack, onWorkspaceChange, lens, selectionGroup }) {
  const [openedBacks, setOpenedBacks] = React.useState(() => new Set());
  React.useEffect(() => setOpenedBacks(new Set()), [
    snapshot.presentation.basis.checked_plot_id,
    snapshot.presentation.basis.expanded_plot_id,
    snapshot.authoring?.checked_plot_id,
    snapshot.authoring?.expanded_plot_id,
  ]);
  const projected = projectFlowScene(snapshot, lens, openedBacks);
  const initial = React.useMemo(() => {
    const restored = restore(projected);
    return restored || reconcileFlowScene(projected);
  }, [projected.workspaceIdentity]);
  const [nodes, setNodes] = React.useState(initial.nodes);
  const [edges, setEdges] = React.useState(presentEdges(initial.edges));
  const workspace = React.useRef(projected.workspaceIdentity);
  const mounted = React.useRef(false);
  React.useEffect(() => {
    let active = true;
    if (projected.basis && admittedStorage && !retainedScenes.has(storageKey(projected.workspaceIdentity))) {
      admittedStorage.readJson(storageKey(projected.workspaceIdentity)).then(async (document) => {
        if (!active) return;
        if (typeof document !== "string") {
          loadedWorkspaces.add(projected.workspaceIdentity);
          if (currentScene?.workspaceIdentity === projected.workspaceIdentity) persist(currentScene);
          return;
        }
        const decoded = decodeWorkspace(document, projected);
        await workspaceValidator(decoded);
        if (!active) return;
        loadedWorkspaces.add(projected.workspaceIdentity);
        retainedScenes.set(storageKey(projected.workspaceIdentity), document);
        workspaceDocuments.set(projected.workspaceIdentity, decoded);
        const next = applyWorkspace(decoded, reconcileFlowScene(projected));
        currentScene = next;
        setNodes(next.nodes);
        setEdges(presentEdges(next.edges));
        instance?.setViewport(next.viewport, { duration: 0 });
        onWorkspaceChange?.(flowWorkspaceStatus());
      }).catch((error) => {
        if (!active) return;
        workspaceRefusals.set(projected.workspaceIdentity, error.code || error.message);
        currentScene = { ...currentScene, storageRefusal: error.code || error.message };
        onWorkspaceChange?.(flowWorkspaceStatus());
      });
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
      const document = workspaceDocuments.get(projected.workspaceIdentity);
      const decorated = document ? applyWorkspace(document, next) : next;
      if (!sameWorkspace && currentScene?.basis
        && currentScene.basis.checked_plot_id !== projected.basis?.checked_plot_id) {
        decorated.workspaceNotice = "ChangedBasis: previous layouts retained separately; no remapping";
      }
      if (!sameWorkspace) instance?.setViewport(decorated.viewport, { duration: 0 });
      workspace.current = projected.workspaceIdentity;
      persist(decorated);
      onWorkspaceChange?.(flowWorkspaceStatus());
      return decorated.nodes;
    });
    const document = workspaceDocuments.get(projected.workspaceIdentity);
    setEdges(presentEdges(document ? applyWorkspace(document, projected).edges : projected.edges));
  }, [projected.workspaceIdentity, projected.lens, openedBacks, snapshot.presentation.identity, snapshot.presentation.revision, snapshot.interaction.revision, snapshot.debugger?.revision, snapshot.timeline?.revision, snapshot.authoring?.connection_candidates]);
  arrangeCurrent = () => {
    const next = reconcileFlowScene(projected);
    setNodes(next.nodes);
    persist(next);
    requestAnimationFrame(() => instance?.fitView({ duration: 0, maxZoom: 1.1, padding: 0.18 }));
  };
  applyCurrent = (document) => {
    const next = applyWorkspace(document, reconcileFlowScene(projected));
    workspaceDocuments.set(projected.workspaceIdentity, document);
    setNodes(next.nodes);
    setEdges(presentEdges(next.edges));
    instance?.setViewport(next.viewport, { duration: 0 });
    persist({ ...next, lens: workspaceLayout(document).lens }, next.viewport);
    onWorkspaceChange?.(flowWorkspaceStatus());
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
      nodes: [...presentedNodes, ...workspaceDecorations(workspaceDocuments.get(projected.workspaceIdentity)
        ? workspaceLayout(workspaceDocuments.get(projected.workspaceIdentity)) : null)],
      edges,
      nodeTypes,
      edgeTypes,
      onNodesChange: (changes) => setNodes((current) => {
        const nextNodes = Flow.applyNodeChanges(changes.filter((change) => !change.id?.startsWith("workspace-")), current);
        persist({
          ...projected,
          nodes: nextNodes,
          edges,
          viewport: instance?.getViewport() || initial.viewport,
        });
        return nextNodes;
      }),
      onPaneClick: onClear,
      onConnectStart: (_event, params) => {
        if (params.handleType === "source" && params.handleId) onConnectStart?.(params.handleId);
      },
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
        onWorkspaceChange?.(flowWorkspaceStatus());
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

function currentDocument() {
  if (!currentScene?.basis) throw new Error("WorkspaceUnavailable");
  return captureWorkspace(currentScene, workspaceDocuments.get(currentScene.workspaceIdentity));
}

async function updateWorkspace(edit) {
  const document = currentDocument();
  edit(document, workspaceLayout(document));
  validateWorkspace(document);
  await workspaceValidator(document);
  workspaceRefusals.delete(currentScene.workspaceIdentity);
  loadedWorkspaces.add(currentScene.workspaceIdentity);
  applyCurrent(document);
  await flowStorageSettled();
  return flowWorkspaceStatus();
}

export function flowWorkspaceStatus() {
  if (!currentScene?.basis) return { status: "WorkspaceUnavailable", available: false,
    layouts: [], active_layout: null, basis: null, lens: currentScene?.lens,
    correlation: { basis_matches: false, orphaned_subjects: [] } };
  const document = workspaceDocuments.get(currentScene.workspaceIdentity);
  return {
    status: workspaceRefusals.get(currentScene.workspaceIdentity) || currentScene.storageRefusal || "Ready",
    available: true,
    layouts: document?.layouts.map((layout) => layout.name) || ["Default"],
    active_layout: document?.active_layout || "Default",
    lens: document ? workspaceLayout(document).lens : currentScene.lens,
    basis: currentScene.basis,
    notice: currentScene.workspaceNotice || null,
    correlation: document ? workspaceCorrelation(document, currentScene) : { basis_matches: true, orphaned_subjects: [] },
    retained_workspaces: [...workspaceDocuments.entries()].filter(([identity]) => identity !== currentScene.workspaceIdentity)
      .map(([identity, prior]) => ({ identity, basis: prior.basis, correlation: workspaceCorrelation(prior, currentScene) })),
  };
}

export async function saveFlowLayout(name) {
  return updateWorkspace((document, layout) => {
    const saved = { ...structuredClone(layout), name };
    const index = document.layouts.findIndex((item) => item.name === name);
    if (index < 0) document.layouts.push(saved); else document.layouts[index] = saved;
    document.active_layout = name;
  });
}

export async function selectFlowLayout(name) {
  return updateWorkspace((document) => { document.active_layout = name; });
}

export function exportFlowWorkspace(identity = null) {
  if (identity !== null) {
    const document = workspaceDocuments.get(identity);
    if (!document) throw new Error("WorkspaceNotRetained");
    return JSON.stringify(document);
  }
  return JSON.stringify(currentDocument());
}

export async function retainedFlowWorkspaces() {
  if (!currentScene) return [];
  if (admittedStorage) {
    const identities = await admittedStorage.readJson(workspaceIndexKey);
    if (Array.isArray(identities)) for (const identity of identities.slice(0, MAX_FLOW_WORKSPACES)) {
      if (typeof identity !== "string" || workspaceDocuments.has(identity)) continue;
      const encoded = await admittedStorage.readJson(storageKey(identity));
      if (typeof encoded !== "string") continue;
      const document = decodeWorkspace(encoded, currentScene);
      workspaceDocuments.set(identity, document);
    }
  }
  return [...workspaceDocuments.entries()].map(([identity, document]) => ({
    identity, basis: document.basis, layouts: document.layouts.map((layout) => layout.name),
    correlation: workspaceCorrelation(document, currentScene),
  }));
}

export async function importFlowWorkspace(encoded) {
  if (!currentScene?.basis) throw new Error("WorkspaceUnavailable");
  const document = decodeWorkspace(encoded, currentScene);
  const correlation = workspaceCorrelation(document, currentScene);
  if (!correlation.basis_matches) {
    throw Object.assign(new Error("ChangedBasis — workspace retained by caller; no layout applied"), { code: "ChangedBasis", correlation });
  }
  await workspaceValidator(document);
  workspaceRefusals.delete(currentScene.workspaceIdentity);
  loadedWorkspaces.add(currentScene.workspaceIdentity);
  applyCurrent(document);
  await flowStorageSettled();
  return flowWorkspaceStatus();
}

function upsert(items, item, key) {
  const index = items.findIndex((candidate) => candidate[key] === item[key]);
  if (index < 0) items.push(item); else items[index] = item;
}

export async function annotateFlow(note) {
  return updateWorkspace((_document, layout) => upsert(layout.notes, structuredClone(note), "id"));
}

export async function frameFlow(frame) {
  return updateWorkspace((_document, layout) => upsert(layout.frames, structuredClone(frame), "id"));
}

export async function routeFlow(subject, style, points = []) {
  return updateWorkspace((_document, layout) => {
    const edge = currentScene.edges.find((item) => item.id === subject || item.data?.workspaceSubject === subject);
    if (!edge?.data?.workspaceSubject) throw new Error("UnknownCord");
    upsert(layout.routes, { subject: edge.data.workspaceSubject, style, points }, "subject");
  });
}

export async function collapseFlow(subject, collapsed) {
  return updateWorkspace((_document, layout) => {
    const frame = layout.frames.find((item) => item.id === subject);
    if (frame) { frame.collapsed = collapsed; return; }
    if (typeof collapsed !== "boolean") throw new Error("InvalidGeometry");
    const node = currentScene.nodes.find((item) => item.id === subject || item.data?.workspaceSubject === subject);
    if (!node?.data?.workspaceSubject) throw new Error("UnknownSubject");
    layout.collapsed = layout.collapsed.filter((item) => item !== node.data.workspaceSubject);
    if (collapsed) layout.collapsed.push(node.data.workspaceSubject);
  });
}
