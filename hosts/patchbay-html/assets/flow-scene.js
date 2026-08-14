export const FLOW_PRESENTATION_SCHEMA = "conduit.patchbay.flow-presentation/v1";
export const MAX_FLOW_SUBJECTS = 512;
export const MAX_FLOW_STATE_BYTES = 64 * 1024;
export const MAX_FLOW_WORKSPACES = 4;

function value(property) {
  const item = property?.value || {};
  return item.Identity ?? item.Text ?? item.Count ?? item.Flag ?? null;
}

function compareIdentity(left, right) {
  return left.id.localeCompare(right.id);
}

function deterministicPosition(index) {
  return {
    x: 80 + (index % 4) * 260,
    y: 100 + Math.floor(index / 4) * 180,
  };
}

export function workspaceIdentity(snapshot) {
  const basis = snapshot.presentation.basis;
  return `${basis.source_document_id}/${basis.checked_form_id}`;
}

export function projectFlowScene(snapshot) {
  const presentation = snapshot.presentation;
  const subjects = presentation.subjects.filter((subject) =>
    ["Seed", "Body", "Host", "Part", "Gear"].includes(subject.role));
  if (subjects.length > MAX_FLOW_SUBJECTS) throw new Error("Flow subject bound exceeded");
  const nodes = subjects.map((subject) => ({
    id: subject.identity,
    data: {
      label: `${subject.role} · ${subject.label}`,
      semanticSelected: snapshot.interaction.selected_subject === subject.identity,
    },
    className: `flow-subject flow-${subject.role.toLowerCase()}`,
    ariaLabel: subject.accessibility_name,
  })).sort(compareIdentity);
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
    if (source && target) edges.push({
      id: cord.identity,
      source,
      target,
      type: "smoothstep",
      ariaLabel: cord.accessibility_name,
    });
  }
  edges.sort(compareIdentity);
  return {
    workspaceIdentity: workspaceIdentity(snapshot),
    presentationIdentity: presentation.identity,
    presentationRevision: presentation.revision,
    nodes,
    edges,
  };
}

export function reconcileFlowScene(projection, prior = null) {
  const priorNodes = new Map((prior?.nodes || []).map((node) => [node.id, node]));
  const nodes = projection.nodes.map((node, index) => ({
    ...node,
    selected: priorNodes.get(node.id)?.selected === true,
    position: priorNodes.get(node.id)?.position || deterministicPosition(index),
  }));
  return {
    ...projection,
    nodes,
    viewport: prior?.viewport || { x: 0, y: 0, zoom: 1 },
  };
}

export function encodeFlowPresentation(scene) {
  const document = JSON.stringify({
    schema: FLOW_PRESENTATION_SCHEMA,
    workspaceIdentity: scene.workspaceIdentity,
    nodes: scene.nodes.slice(0, MAX_FLOW_SUBJECTS).map(({ id, position, selected }) => ({
      id,
      position,
      selected: selected === true,
    })),
    viewport: scene.viewport,
  });
  if (document.length > MAX_FLOW_STATE_BYTES) throw new Error("Flow presentation byte bound exceeded");
  return document;
}

export function decodeFlowPresentation(document, projection) {
  if (typeof document !== "string" || document.length > MAX_FLOW_STATE_BYTES) return null;
  let parsed;
  try { parsed = JSON.parse(document); } catch { return null; }
  if (parsed?.schema !== FLOW_PRESENTATION_SCHEMA || parsed.workspaceIdentity !== projection.workspaceIdentity || !Array.isArray(parsed.nodes) || parsed.nodes.length > MAX_FLOW_SUBJECTS) return null;
  const current = new Set(projection.nodes.map((node) => node.id));
  const nodes = [];
  const seen = new Set();
  for (const node of parsed.nodes) {
    if (typeof node?.id !== "string" || seen.has(node.id) || !current.has(node.id) || !Number.isFinite(node.position?.x) || !Number.isFinite(node.position?.y)) return null;
    seen.add(node.id);
    if (node.selected !== undefined && typeof node.selected !== "boolean") return null;
    nodes.push({
      id: node.id,
      position: { x: node.position.x, y: node.position.y },
      selected: node.selected === true,
    });
  }
  const viewport = parsed.viewport;
  if (![viewport?.x, viewport?.y, viewport?.zoom].every(Number.isFinite) || viewport.zoom < 0.2 || viewport.zoom > 3) return null;
  return { nodes, viewport };
}
