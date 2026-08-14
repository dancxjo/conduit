import { layoutFlowScene } from "/assets/flow-layout.js";

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

function propertiesBySubject(presentation) {
  const result = new Map();
  for (const property of presentation.properties) {
    if (!result.has(property.subject)) result.set(property.subject, new Map());
    result.get(property.subject).set(property.name, value(property));
  }
  return result;
}

function compactClue(role, properties, lens) {
  if (lens === "form") return properties.get("kind-id") || [...properties.entries()].find(([name]) => name.startsWith("authored-control-"))?.[1] || "checked intent";
  if (lens === "plan") return properties.get("host-id") || properties.get("realization-layer") || "not realized";
  if (lens === "play") return [properties.get("play-state"), properties.get("pressure")].filter(Boolean).join(" · ") || "not playing";
  if (lens === "signs") return `${[...properties.keys()].filter((name) => name.startsWith("sign-")).length} causal Signs`;
  return properties.get("operational-state") || properties.get("availability") || (role === "Gear" ? "semantic Gear" : "current world subject");
}

export function projectFlowScene(snapshot, lens = "world") {
  const presentation = snapshot.presentation;
  const subjectProperties = propertiesBySubject(presentation);
  const allSubjects = new Map(presentation.subjects.map((subject) => [subject.identity, subject]));
  const children = new Map();
  for (const relation of presentation.relationships) {
    if (relation.kind !== "Contains") continue;
    if (!children.has(relation.source)) children.set(relation.source, []);
    children.get(relation.source).push(relation.target);
  }
  const subjects = presentation.subjects.filter((subject) =>
    ["Seed", "Body", "Host", "Part", "Gear"].includes(subject.role));
  if (subjects.length > MAX_FLOW_SUBJECTS) throw new Error("Flow subject bound exceeded");
  const nodes = subjects.map((subject) => ({
    id: subject.identity,
    type: "faceplate",
    data: {
      subjectIdentity: subject.identity,
      label: subject.label,
      role: subject.role,
      accessibilityName: subject.accessibility_name,
      icon: subjectProperties.get(subject.identity)?.get("icon-token") || (subject.role === "Gear" ? "◆" : "◇"),
      iconName: subjectProperties.get(subject.identity)?.get("icon-name") || subject.role,
      clue: compactClue(subject.role, subjectProperties.get(subject.identity) || new Map(), lens),
      lens,
      ports: (children.get(subject.identity) || []).map((identity) => allSubjects.get(identity)).filter((item) => item?.role === "Port").map((port) => {
        const properties = subjectProperties.get(port.identity) || new Map();
        return {
          id: port.identity,
          label: port.label,
          accessibilityName: port.accessibility_name,
          direction: properties.get("direction"),
          valueKind: properties.get("value-kind") || "typed value",
          temporal: properties.get("temporal") || "",
        };
      }).sort((left, right) => left.direction.localeCompare(right.direction) || left.id.localeCompare(right.id)),
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
      sourceHandle: sourcePort,
      targetHandle: sinkPort,
      type: "smoothstep",
      label: lens === "plan" ? `Cord · ${subjectProperties.get(cord.identity)?.get("line-id") ? `Line ${subjectProperties.get(cord.identity).get("line-id")}` : "local; no Line"}` : lens === "play" ? `Cord · ${subjectProperties.get(cord.identity)?.get("play-state") || "not playing"}` : "Cord",
      data: {
        semanticIdentity: cord.identity,
        sourcePort,
        sinkPort,
        lineIdentity: subjectProperties.get(cord.identity)?.get("line-id") || null,
        lens,
      },
      ariaLabel: cord.accessibility_name,
    });
  }
  edges.sort(compareIdentity);
  return {
    workspaceIdentity: workspaceIdentity(snapshot),
    presentationIdentity: presentation.identity,
    presentationRevision: presentation.revision,
    lens,
    nodes,
    edges,
  };
}

export function reconcileFlowScene(projection, prior = null) {
  const priorNodes = new Map((prior?.nodes || []).map((node) => [node.id, node]));
  const arranged = layoutFlowScene(projection.nodes, projection.edges);
  const nodes = projection.nodes.map((node, index) => ({
    ...node,
    selected: priorNodes.get(node.id)?.selected === true,
    position: priorNodes.get(node.id)?.position || arranged.get(node.id) || deterministicPosition(index),
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
