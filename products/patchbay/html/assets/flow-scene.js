import { layoutFlowScene } from "./flow-layout.js";
import { projectCurrent } from "./portable-navigation.js";

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
  const cursor = snapshot.navigation?.cursor;
  return `${basis.source_document_id}/${basis.checked_form_id}/${cursor?.place??"canonical"}`;
}

function propertiesBySubject(presentation) {
  const result = new Map();
  for (const property of presentation.properties) {
    if (!result.has(property.subject)) result.set(property.subject, new Map());
    result.get(property.subject).set(property.name, value(property));
  }
  return result;
}

function debuggerBySubject(snapshot) {
  const activities = snapshot.debugger?.activities;
  if (!Array.isArray(activities) || activities.length > MAX_FLOW_SUBJECTS) return new Map();
  return new Map(activities.map((activity) => [activity.subject, activity]));
}

function debuggerValue(activity) {
  const value = activity?.latest_value;
  return typeof value?.summary === "string" ? value.summary.slice(0, 96) : null;
}

function debuggerLabel(activity) {
  if (!activity) return null;
  const route = activity.line_subject ? ` · Host ${activity.host} via ${activity.line_subject}` : "";
  if (activity.phase === "faulted") return `Fault ${activity.retained_fault_code ?? "unknown"}${route}`;
  const latest = debuggerValue(activity);
  return `${latest ? `${latest} · ` : `${activity.latest_kind} · `}${activity.observed_count} observed${route}`;
}

function compactClue(role, properties, lens) {
  if (lens === "form") return properties.get("kind-id") || [...properties.entries()].find(([name]) => name.startsWith("authored-control-"))?.[1] || "checked intent";
  if (lens === "plan") return properties.get("host-id") || properties.get("realization-layer") || "not realized";
  if (lens === "play") return [properties.get("play-state"), properties.get("pressure")].filter(Boolean).join(" · ") || "not playing";
  if (lens === "signs") return `${[...properties.keys()].filter((name) => name.startsWith("sign-")).length} causal Signs`;
  return properties.get("operational-state") || properties.get("availability") || (role === "Gear" ? "semantic Gear" : "current world subject");
}

function cordCapacity(properties) {
  const admitted = properties.get("admitted-capacity");
  const match = typeof admitted === "string"
    ? /^items=(\d+) bytes=(\d+)$/.exec(admitted)
    : null;
  if (!match) return { items: null, bytes: null, label: "capacity not admitted", strokeWidth: 2 };
  const items = Number(match[1]);
  const bytes = Number(match[2]);
  return {
    items,
    bytes,
    label: `${items} ${items === 1 ? "item" : "items"} · ${bytes} B`,
    // Keep large admitted queues legible without allowing their presentation
    // to obscure the graph. The exact capacity remains in the label.
    strokeWidth: Math.min(7, 2 + Math.log2(Math.max(1, items) + 1)),
  };
}

function cordVisual(properties, lens) {
  const capacity = cordCapacity(properties);
  const valueKind = properties.get("value-kind") || "typed Info";
  const playState = properties.get("play-state");
  const pressure = properties.get("pressure");
  const hasLine = Boolean(properties.get("line-id"));
  if (lens === "play") {
    const pressureKnown = pressure && pressure !== "not exposed by this Play";
    const pressureLabel = pressureKnown ? pressure : "pressure unavailable";
    const terminal = /^(Completed|Failed|Cancelled)/.test(playState || "");
    return {
      label: [playState || "not playing", pressureLabel, capacity.label].join(" · "),
      className: `cord-play ${pressureKnown ? "cord-pressure-known" : "cord-pressure-unknown"}`,
      // Motion means a non-terminal Play, never inferred Info delivery. The
      // pressure label remains visible so status animation cannot pose as a
      // measurement the Play did not expose.
      animated: Boolean(properties.get("active-play-id")) && !terminal,
      strokeWidth: capacity.strokeWidth,
    };
  }
  if (lens === "plan") return {
    label: `${capacity.label} · ${hasLine ? "external Line" : "local"}`,
    className: hasLine ? "cord-line" : "cord-local",
    animated: false,
    strokeWidth: capacity.strokeWidth,
  };
  return {
    label: properties.has("flow-label") ? properties.get("flow-label") : valueKind,
    className: properties.get("diagnostic-state") === "error" ? "cord-semantic diagnostic-error" : "cord-semantic",
    animated: properties.get("flow-animation") === "directional",
    strokeWidth: capacity.items === null ? 3 : capacity.strokeWidth,
  };
}

export function projectFlowScene(snapshot, lens = "world", openedBacks = new Set()) {
  const presentation = projectCurrent(snapshot);
  // Projection owns which subjects and relationships exist in this scene. The
  // canonical Presentation still owns the exact typed facts needed to draw
  // those admitted subjects (for example Port direction and Cord endpoints).
  const subjectProperties = propertiesBySubject(snapshot.presentation);
  const debuggerActivity = debuggerBySubject(snapshot);
  const causalTrace = new Set((snapshot.timeline?.trace?.steps || []).map((step) => step.subject));
  const tracing = causalTrace.size > 0;
  const reducedMotion = snapshot.debugger?.reduced_motion === true;
  const allSubjects = new Map(presentation.subjects.map((subject) => [subject.identity, subject]));
  const children = new Map();
  for (const relation of presentation.relationships) {
    if (relation.kind !== "Contains") continue;
    if (!children.has(relation.source)) children.set(relation.source, []);
    children.get(relation.source).push(relation.target);
  }
  const visibleThroughBacks = (subject) => {
    let parent = subjectProperties.get(subject.identity)?.get("recursive-parent");
    const visited = new Set();
    while (parent) {
      if (visited.size === MAX_FLOW_SUBJECTS || visited.has(parent)) {
        throw new Error("Recursive Back presentation is cyclic or exceeds its finite bound");
      }
      visited.add(parent);
      if (!openedBacks.has(parent)) return false;
      parent = subjectProperties.get(parent)?.get("recursive-parent");
    }
    return true;
  };
  const subjects = presentation.subjects.filter((subject) =>
    ["Form", "Body", "Host", "Part", "Gear"].includes(subject.role)
      && visibleThroughBacks(subject));
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
      reviewedBack: subjectProperties.get(subject.identity)?.get("reviewed-back") === "available",
      backExpanded: openedBacks.has(subject.identity),
      diagnosticError: subjectProperties.get(subject.identity)?.get("diagnostic-state") === "error",
      debugger: debuggerActivity.get(subject.identity) || null,
      causalTrace: causalTrace.has(subject.identity),
      causalUnrelated: tracing && !causalTrace.has(subject.identity),
      reducedMotion,
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
          diagnosticError: properties.get("diagnostic-state") === "error",
          debugger: debuggerActivity.get(port.identity) || null,
          causalTrace: causalTrace.has(port.identity),
        };
      }).sort((left, right) => left.direction.localeCompare(right.direction) || left.id.localeCompare(right.id)),
      semanticSelected: (snapshot.navigation?.cursor.focus ?? snapshot.interaction.selected_subject) === subject.identity,
    },
    className: `flow-subject flow-${subject.role.toLowerCase()}${subjectProperties.get(subject.identity)?.get("diagnostic-state") === "error" ? " diagnostic-error" : ""}${causalTrace.has(subject.identity) ? " causal-trace-exact" : tracing ? " causal-trace-unrelated" : ""}`,
    ariaLabel: subject.accessibility_name,
  })).sort(compareIdentity);
  const nodeIds = new Set(nodes.map((node) => node.id));
  const semanticSubjects = new Map();
  for (const property of snapshot.presentation.properties) {
    if (property.name === "semantic-id") semanticSubjects.set(value(property), property.subject);
  }
  const owner = new Map();
  for (const relation of presentation.relationships) {
    if (relation.kind === "Contains" && nodeIds.has(relation.source)) owner.set(relation.target, relation.source);
  }
  const edges = [];
  for (const cord of presentation.subjects.filter((subject) => subject.role === "Cord")) {
    const properties = snapshot.presentation.properties.filter((property) => property.subject === cord.identity);
    const propertiesMap = subjectProperties.get(cord.identity) || new Map();
    let sourcePort = semanticSubjects.get(value(properties.find((property) => property.name === "source-port")));
    let sinkPort = semanticSubjects.get(value(properties.find((property) => property.name === "sink-port")));
    const collapsedSource = propertiesMap.get("collapsed-source-port");
    const collapsedSink = propertiesMap.get("collapsed-sink-port");
    if (collapsedSource) sourcePort = semanticSubjects.get(collapsedSource);
    if (collapsedSink) sinkPort = semanticSubjects.get(collapsedSink);
    const source = owner.get(sourcePort);
    const target = owner.get(sinkPort);
    const visual = cordVisual(propertiesMap, lens);
    const activity = debuggerActivity.get(cord.identity);
    const activityLabel = debuggerLabel(activity);
    if (source && target) edges.push({
      id: cord.identity,
      source,
      target,
      sourceHandle: sourcePort,
      targetHandle: sinkPort,
      type: "simplebezier",
      label: activityLabel || visual.label,
      className: `${visual.className}${activity ? ` debugger-${activity.phase}` : ""}${snapshot.debugger?.gap ? " debugger-gap" : ""}${causalTrace.has(cord.identity) ? " causal-trace-exact" : tracing ? " causal-trace-unrelated" : ""}`,
      animated: activity ? activity.phase === "active" && !reducedMotion : visual.animated,
      style: { strokeWidth: visual.strokeWidth },
      data: {
        semanticIdentity: cord.identity,
        sourcePort,
        sinkPort,
        lineIdentity: subjectProperties.get(cord.identity)?.get("line-id") || null,
        lens,
        debugger: activity || null,
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
