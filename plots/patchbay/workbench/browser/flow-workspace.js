// Browser consumer of patchbay_application::PatchbayWorkspace. Server validation
// is the portable authority; these checks also fence offline storage and imports.
export const WORKSPACE_SCHEMA = "conduit.patchbay.workspace/v1";
export const WORKSPACE_BYTES = 64 * 1024;
const bytes = (value) => new TextEncoder().encode(value).length;
const fail = (code) => { throw Object.assign(new Error(code), { code }); };
function fields(value, names) {
  if (!value || typeof value !== "object" || Array.isArray(value)
    || Object.keys(value).some((key) => !names.includes(key))
    || names.some((key) => !(key in value))) fail("UnknownOrMissingField");
}
function text(value, max = 512) {
  if (typeof value !== "string" || !value.length || /[\u0000-\u001f\u007f-\u009f]/u.test(value)) fail("InvalidIdentity");
  if (bytes(value) > max) fail("BoundExceeded");
}
function list(values, max, key = (value) => value) {
  if (!Array.isArray(values) || values.length > max) fail("BoundExceeded");
  const keys = values.map(key);
  keys.forEach((value) => text(value));
  if (new Set(keys).size !== keys.length) fail("DuplicateIdentity");
}
function point(value) {
  if (![value.x, value.y].every((v) => Number.isInteger(v) && Math.abs(v) <= 32767)) fail("InvalidGeometry");
}
function viewport(value) {
  fields(value, ["x", "y", "zoom"]);
  if (![value.x, value.y, value.zoom].every(Number.isFinite)
    || Math.abs(value.x) > 32767 || Math.abs(value.y) > 32767
    || value.zoom < 0.2 || value.zoom > 3) fail("InvalidGeometry");
}
export function validateWorkspace(document) {
  if (document?.schema !== WORKSPACE_SCHEMA) fail("UnsupportedSchema");
  fields(document, ["schema", "basis", "active_layout", "layouts"]);
  fields(document.basis, ["source_document_id", "checked_plot_id"]);
  text(document.basis.source_document_id); text(document.basis.checked_plot_id);
  text(document.active_layout, 64);
  list(document.layouts, 4, (layout) => layout?.name);
  if (!document.layouts.length) fail("BoundExceeded");
  for (const layout of document.layouts) {
    fields(layout, ["name", "positions", "routes", "frames", "notes", "collapsed", "viewport", "lens"]);
    text(layout.name, 64);
    list(layout.positions, 512, (item) => item?.subject);
    list(layout.routes, 512, (item) => item?.subject);
    list(layout.frames, 32, (item) => item?.id);
    list(layout.notes, 64, (item) => item?.id);
    list(layout.collapsed, 512);
    for (const item of layout.positions) { fields(item, ["subject", "x", "y"]); point(item); }
    for (const item of layout.routes) {
      fields(item, ["subject", "style", "points"]);
      if (!["straight", "curved", "orthogonal", "manual"].includes(item.style)) fail("InvalidRoute");
      if (!Array.isArray(item.points) || item.points.length > 32) fail("BoundExceeded");
      if ((item.style === "manual") !== (item.points.length > 0)) fail("InvalidRoute");
      for (const p of item.points) { fields(p, ["x", "y"]); point(p); }
    }
    for (const item of layout.frames) {
      fields(item, ["id", "title", "members", "x", "y", "width", "height", "collapsed"]);
      text(item.title, 256); list(item.members, 512); point(item);
      point({ x: item.width, y: item.height });
      if (item.width < 1 || item.height < 1 || typeof item.collapsed !== "boolean") fail("InvalidGeometry");
    }
    for (const item of layout.notes) {
      fields(item, ["id", "text", "subject", "x", "y"]);
      text(item.text, 2048); point(item);
      if (item.subject !== null) text(item.subject);
    }
    viewport(layout.viewport);
    if (!["world", "plot", "plan", "play", "signs"].includes(layout.lens)) fail("InvalidLens");
  }
  if (!document.layouts.some((layout) => layout.name === document.active_layout)) fail("MissingActiveLayout");
  if (bytes(JSON.stringify(document)) > WORKSPACE_BYTES) fail("BoundExceeded");
  return document;
}

export function sameBasis(left, right) {
  return left?.source_document_id === right?.source_document_id && left?.checked_plot_id === right?.checked_plot_id;
}
export function workspaceLayout(document) {
  return document.layouts.find((layout) => layout.name === document.active_layout);
}
export function createWorkspace(scene) {
  return validateWorkspace({
    schema: WORKSPACE_SCHEMA, basis: { ...scene.basis }, active_layout: "Default",
    layouts: [{ name: "Default", positions: [], routes: [], frames: [], notes: [], collapsed: [],
      viewport: { x: 0, y: 0, zoom: 1 }, lens: "world" }],
  });
}
export function captureWorkspace(scene, previous = createWorkspace(scene)) {
  const document = structuredClone(previous);
  if (!sameBasis(document.basis, scene.basis)) fail("ChangedBasis");
  const layout = workspaceLayout(document);
  // Preserve unprojected and orphaned subjects; a lens is not a semantic deletion.
  const positions = new Map(layout.positions.map((item) => [item.subject, item]));
  for (const node of scene.nodes) {
    const subject = node.data?.workspaceSubject;
    if (subject) positions.set(subject, { subject, x: Math.trunc(node.position.x), y: Math.trunc(node.position.y) });
  }
  layout.positions = [...positions.values()].sort((a, b) => a.subject < b.subject ? -1 : a.subject > b.subject ? 1 : 0);
  layout.viewport = { ...scene.viewport };
  layout.lens = scene.lens;
  return validateWorkspace(document);
}
export function workspaceCorrelation(document, scene) {
  const known = new Set(scene.semanticSubjects || []);
  const refs = document.layouts.flatMap((layout) => [
    ...layout.positions.map((item) => item.subject), ...layout.routes.map((item) => item.subject),
    ...layout.frames.flatMap((item) => item.members), ...layout.notes.map((item) => item.subject).filter(Boolean),
    ...layout.collapsed,
  ]);
  return { basis_matches: sameBasis(document.basis, scene.basis),
    orphaned_subjects: [...new Set(refs.filter((subject) => !known.has(subject)))].sort() };
}
export function applyWorkspace(document, scene) {
  validateWorkspace(document);
  const correlation = workspaceCorrelation(document, scene);
  if (!correlation.basis_matches) fail("ChangedBasis");
  const layout = workspaceLayout(document);
  const positions = new Map(layout.positions.map((item) => [item.subject, item]));
  const routes = new Map(layout.routes.map((item) => [item.subject, item]));
  const collapsed = new Set(layout.collapsed);
  const hidden = new Set(layout.frames.filter((frame) => frame.collapsed).flatMap((frame) => frame.members));
  const nodes = scene.nodes.map((node) => {
    const subject = node.data?.workspaceSubject;
    const position = positions.get(subject);
    return { ...node, ...(position ? { position: { x: position.x, y: position.y } } : {}),
      hidden: hidden.has(subject), data: { ...node.data, workspaceCollapsed: collapsed.has(subject) } };
  });
  const hiddenIds = new Set(nodes.filter((node) => node.hidden).map((node) => node.id));
  return { ...scene, nodes, viewport: { ...layout.viewport }, workspace: document, workspaceCorrelation: correlation,
    edges: scene.edges.map((edge) => {
      const route = routes.get(edge.data?.workspaceSubject);
      const type = { straight: "straight", curved: "simplebezier", orthogonal: "step", manual: "workspaceManual" }[route?.style];
      return { ...edge, ...(type ? { type } : {}), hidden: hiddenIds.has(edge.source) || hiddenIds.has(edge.target),
        data: { ...edge.data, workspaceRoute: route || null } };
    }) };
}
export function decodeWorkspace(encoded, scene) {
  if (typeof encoded !== "string" || bytes(encoded) > WORKSPACE_BYTES) fail("BoundExceeded");
  const parsed = JSON.parse(encoded);
  if (parsed.schema === "conduit.patchbay.flow-presentation/v1") return migrateFlowPresentation(parsed, scene);
  return validateWorkspace(parsed);
}
export function migrateFlowPresentation(legacy, scene) {
  fields(legacy, ["schema", "workspaceIdentity", "nodes", "viewport"]);
  if (legacy.schema !== "conduit.patchbay.flow-presentation/v1") fail("UnsupportedSchema");
  if (legacy.workspaceIdentity !== scene.workspaceIdentity) fail("ChangedBasis");
  list(legacy.nodes, 512, (node) => node?.id); viewport(legacy.viewport);
  const mapping = new Map(scene.nodes.map((node) => [node.id, node.data?.workspaceSubject]));
  const document = createWorkspace(scene);
  document.layouts[0].positions = legacy.nodes.map((node) => {
    const allowed = node.selected === undefined ? ["id", "position"] : ["id", "position", "selected"];
    fields(node, allowed); fields(node.position, ["x", "y"]);
    if (node.selected !== undefined && typeof node.selected !== "boolean") fail("InvalidIdentity");
    if (![node.position.x, node.position.y].every((v) => Number.isFinite(v) && Math.abs(v) <= 32767)) fail("InvalidGeometry");
    const subject = mapping.get(node.id);
    if (!subject) fail("UnmappedLegacySubject");
    return { subject, x: Math.trunc(node.position.x), y: Math.trunc(node.position.y) };
  }).sort((a, b) => a.subject < b.subject ? -1 : a.subject > b.subject ? 1 : 0);
  document.layouts[0].viewport = { ...legacy.viewport };
  return validateWorkspace(document);
}
