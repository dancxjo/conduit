const React = window.React;
const ReactDOM = window.ReactDOM;
const Flow = window.ReactFlow;

if (!React || !ReactDOM || !Flow) {
  throw new Error("preserved React Flow presentation assets are unavailable");
}

const e = React.createElement;
const ReactFlow = Flow.default || Flow.ReactFlow || Flow;
const positions = new Map();
let instance = null;
let root = null;

function value(property) {
  const item = property?.value || {};
  return item.Identity ?? item.Text ?? item.Count ?? item.Flag ?? null;
}

function initialPosition(identity, index) {
  return positions.get(identity) || {
    x: 80 + (index % 4) * 260,
    y: 100 + Math.floor(index / 4) * 180,
  };
}

function scene(snapshot) {
  const presentation = snapshot.presentation;
  const spatial = presentation.subjects.filter((subject) =>
    ["Seed", "Body", "Host", "Part", "Gear"].includes(subject.role));
  const nodes = spatial.map((subject, index) => ({
    id: subject.identity,
    position: initialPosition(subject.identity, index),
    data: { label: `${subject.role} · ${subject.label}` },
    className: `flow-subject flow-${subject.role.toLowerCase()}`,
    ariaLabel: subject.accessibility_name,
  }));
  const nodeIds = new Set(nodes.map((node) => node.id));
  const semanticSubjects = new Map();
  for (const property of presentation.properties) {
    if (property.name === "semantic-id") semanticSubjects.set(value(property), property.subject);
  }
  const owner = new Map();
  for (const relation of presentation.relationships) {
    if (relation.kind === "Contains" && nodeIds.has(relation.source)) owner.set(relation.target, relation.source);
  }
  const edges = [];
  for (const cord of presentation.subjects.filter((subject) => subject.role === "Cord")) {
    const properties = presentation.properties.filter((property) => property.subject === cord.identity);
    const sourcePort = semanticSubjects.get(value(properties.find((property) => property.name === "source-port")));
    const sinkPort = semanticSubjects.get(value(properties.find((property) => property.name === "sink-port")));
    const source = owner.get(sourcePort);
    const target = owner.get(sinkPort);
    if (!source || !target) continue;
    edges.push({
      id: cord.identity,
      source,
      target,
      type: "smoothstep",
      className: "flow-cord",
      markerEnd: { type: Flow.MarkerType.ArrowClosed },
      ariaLabel: cord.accessibility_name,
    });
  }
  return { nodes, edges };
}

function Workspace({ snapshot, onSelect, onClear }) {
  const projected = scene(snapshot);
  const [nodes, setNodes] = React.useState(projected.nodes);
  const [edges, setEdges] = React.useState(projected.edges);
  React.useEffect(() => {
    setNodes(projected.nodes.map((node, index) => ({
      ...node,
      position: initialPosition(node.id, index),
    })));
    setEdges(projected.edges);
  }, [snapshot.presentation.identity, snapshot.presentation.revision]);
  return e(
    ReactFlow,
    {
      nodes,
      edges,
      onNodesChange: (changes) => setNodes((current) => Flow.applyNodeChanges(changes, current)),
      onNodeClick: (_event, node) => onSelect(node.id),
      onPaneClick: onClear,
      onNodeDragStop: (_event, node) => positions.set(node.id, { ...node.position }),
      onInit: (next) => { instance = next; },
      nodesDraggable: true,
      nodesConnectable: false,
      elementsSelectable: true,
      panOnDrag: true,
      zoomOnScroll: true,
      zoomOnPinch: true,
      minZoom: 0.2,
      maxZoom: 3,
      fitView: true,
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
