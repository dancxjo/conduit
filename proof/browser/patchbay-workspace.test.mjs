import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import {
  applyWorkspace, captureWorkspace, decodeWorkspace, migrateFlowPresentation,
  validateWorkspace, workspaceCorrelation,
} from "../../plots/patchbay/workbench/browser/flow-workspace.js";
import { projectFlowScene, workspaceBasis, workspaceIdentity } from "../../plots/patchbay/workbench/browser/flow-scene.js";

const fixture = () => JSON.parse(readFileSync(new URL("./fixtures/patchbay-workspace.json", import.meta.url), "utf8"));
function scene() {
  const document = fixture();
  const subjects = document.layouts[0].positions.map((item) => item.subject);
  return {
    basis: document.basis, workspaceIdentity: "source/exact/checked/exact/canonical",
    lens: "plan", viewport: { x: 0, y: 0, zoom: 1 },
    semanticSubjects: [...subjects, ...document.layouts[0].routes.map((item) => item.subject)],
    nodes: subjects.map((subject, i) => ({
      id: `renderer-${i}`, position: { x: 0, y: 0 },
      data: { workspaceSubject: subject, debugger: { phase: "active" } },
    })),
    edges: document.layouts[0].routes.map((route, i) => ({
      id: `renderer-cord-${i}`, source: `renderer-${i}`, target: `renderer-${i + 1}`,
      data: { workspaceSubject: route.subject, lineIdentity: "live/line" }, type: "simplebezier",
    })),
  };
}

test("shared native/browser schema keeps radically different arrangements on one checked basis", () => {
  const document = fixture();
  validateWorkspace(document);
  const projected = scene();
  const before = structuredClone(projected);
  const first = applyWorkspace(document, projected);
  document.active_layout = "Vertical";
  const second = applyWorkspace(document, projected);
  assert.notDeepEqual(first.nodes.map((node) => node.position), second.nodes.map((node) => node.position));
  assert.deepEqual(first.basis, second.basis);
  assert.deepEqual(projected, before);
  assert.equal(first.nodes[4].data.workspaceCollapsed, true);
  assert.deepEqual(first.edges.map((edge) => edge.type), ["straight", "simplebezier", "step", "workspaceManual"]);
  assert.equal(first.edges[3].data.lineIdentity, "live/line");
  const saved = JSON.stringify(captureWorkspace(first, fixture()));
  assert.equal(saved.includes("renderer-"), false);
  assert.equal(saved.includes("debugger"), false);
  assert.equal(saved.includes("live/line"), false);
});

test("renderer reorder/reidentification never changes semantic correlation", () => {
  const projected = scene();
  projected.nodes.reverse();
  projected.nodes.forEach((node, i) => { node.id = `new-wrapper-${i}`; });
  const applied = applyWorkspace(fixture(), projected);
  assert.equal(applied.nodes.find((node) => node.data.workspaceSubject === "gear/source").position.x, 80);
});

test("missing and renamed subjects remain explicit orphans and changed basis refuses applying", () => {
  const document = fixture();
  const projected = scene();
  projected.semanticSubjects = projected.semanticSubjects.filter((subject) => subject !== "gear/source");
  projected.nodes.shift();
  assert.deepEqual(workspaceCorrelation(document, projected).orphaned_subjects, ["gear/source"]);
  assert.ok(captureWorkspace(projected, document).layouts[0].positions.some((item) => item.subject === "gear/source"));
  projected.basis = { ...projected.basis, checked_plot_id: "checked/renamed" };
  assert.throws(() => applyWorkspace(document, projected), /ChangedBasis/);
});

test("migration is deterministic and requires exact legacy wrapper-to-semantic mapping", () => {
  const projected = scene();
  const legacy = {
    schema: "conduit.patchbay.flow-presentation/v1", workspaceIdentity: projected.workspaceIdentity,
    nodes: [{ id: "renderer-0", position: { x: -1.9, y: 2.8 }, selected: true }],
    viewport: { x: 0, y: 0, zoom: 1 },
  };
  const migrated = migrateFlowPresentation(legacy, projected);
  assert.deepEqual(migrated, migrateFlowPresentation(legacy, projected));
  assert.deepEqual(migrated.layouts[0].positions, [{ subject: "gear/source", x: -1, y: 2 }]);
  legacy.nodes[0].id = "deleted-wrapper";
  assert.throws(() => migrateFlowPresentation(legacy, projected), /UnmappedLegacySubject/);
});

test("unknown fields, future versions, counts, UTF-8 bytes and nonfinite geometry refuse", () => {
  const check = (edit, error) => {
    const document = fixture(); edit(document);
    assert.throws(() => validateWorkspace(document), error);
  };
  check((doc) => { doc.schema = "conduit.patchbay.workspace/v9"; }, /UnsupportedSchema/);
  check((doc) => { doc.plan_id = "live"; }, /UnknownOrMissingField/);
  check((doc) => { doc.layouts[0].positions[0].renderer = "wrapper"; }, /UnknownOrMissingField/);
  check((doc) => { doc.layouts[0].notes[0].text = "é".repeat(1025); }, /BoundExceeded/);
  check((doc) => { doc.layouts[0].viewport.zoom = NaN; }, /InvalidGeometry/);
  check((doc) => { doc.layouts[0].positions[0].x = 32768; }, /InvalidGeometry/);
  check((doc) => { doc.layouts[0].routes[0].points = [{ x: 1, y: 2 }]; }, /InvalidRoute/);
  check((doc) => { doc.layouts.push(...doc.layouts); doc.layouts[2].name = "Third"; doc.layouts.push(structuredClone(doc.layouts[0])); }, /BoundExceeded/);
  assert.throws(() => decodeWorkspace("x".repeat(65537), scene()), /BoundExceeded/);
});

test("authoring workspace uses exact checked source rather than enclosing Body presentation", () => {
  const snapshot = { authoring: { source_document_id: "source/authored", checked_plot_id: "checked/authored" },
    presentation: { basis: { source_document_id: "source/body", checked_plot_id: "checked/body" } } };
  assert.deepEqual(workspaceBasis(snapshot), snapshot.authoring);
  assert.equal(workspaceIdentity(snapshot), "source/authored/checked/authored/canonical");
  snapshot.authoring.checked_plot_id = undefined;
  assert.throws(() => workspaceBasis(snapshot), /WorkspaceBasisUnavailable/);
});

test("fresh Entrance and Body scenes have no portable Plot workspace and still project", () => {
  const snapshot = { presentation: { identity: "entrance", revision: 1,
    basis: { source_document_id: null, checked_plot_id: null },
    subjects: [{ identity: "body/present", role: "Body", name: "Present Body" }],
    properties: [], relationships: [] }, interaction: { selected_subject: null } };
  assert.equal(workspaceBasis(snapshot), null);
  assert.equal(workspaceIdentity(snapshot), null);
  const projected = projectFlowScene(snapshot);
  assert.equal(projected.basis, null);
  assert.equal(projected.workspaceIdentity, null);
  assert.equal(projected.nodes.length, 1);
});

test("port compatibility is copied from canonical candidates by semantic identity, never label", () => {
  const candidate = { sink_identity: "port/exact", compatible: false, diagnostic: "Canonical temporal mismatch", adapters: [] };
  const snapshot = {
    authoring: { ...fixture().basis, connection_candidates: [candidate] },
    presentation: { identity: "presentation", revision: 1, basis: fixture().basis,
      subjects: [{ identity: "gear-wrapper", role: "Gear", name: "Gear" },
        { identity: "port-wrapper", role: "Port", name: "Misleading label", label: "Same-looking Text" }],
      relationships: [{ source: "gear-wrapper", target: "port-wrapper", kind: "Contains" }],
      properties: [{ subject: "port-wrapper", name: "semantic-id", value: { Identity: "port/exact" } },
        { subject: "port-wrapper", name: "direction", value: { Text: "receiving" } }],
    }, interaction: { selected_subject: null },
  };
  assert.deepEqual(projectFlowScene(snapshot).nodes[0].data.ports[0].compatibility, candidate);
  snapshot.authoring.connection_candidates = [{ ...candidate, sink_identity: "port-wrapper", compatible: true }];
  assert.equal(projectFlowScene(snapshot).nodes[0].data.ports[0].compatibility, null);
  snapshot.authoring.connection_candidates = Array(513).fill(candidate);
  assert.throws(() => projectFlowScene(snapshot), /Connection candidate bound exceeded/);
});

test("frames collapse only visual members and leave exact graph connections intact", () => {
  const document = fixture();
  document.layouts[0].frames[0].collapsed = true;
  const projected = scene();
  const result = applyWorkspace(document, projected);
  assert.equal(result.nodes.filter((node) => node.hidden).length, 2);
  assert.equal(result.edges.length, projected.edges.length);
  assert.deepEqual(result.edges.map((edge) => [edge.source, edge.target]), projected.edges.map((edge) => [edge.source, edge.target]));
});
