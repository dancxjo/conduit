// Opt-in CPU conformance oracle; no DOM, browser input or pixel claim.
import assert from "node:assert/strict";
import { projectCurrent, observeCurrent } from "../../plots/patchbay/workbench/browser/portable-navigation.js";
import { projectFlowScene, reconcileFlowScene } from "../../plots/patchbay/workbench/browser/flow-scene.js";
let encoded = "";
for await (const chunk of process.stdin) encoded += chunk;
const snapshot = JSON.parse(encoded);
const projected = projectCurrent(snapshot);
const flow = projectFlowScene(snapshot);
const arranged = reconcileFlowScene(flow);
assert.deepEqual(arranged.nodes.map(node => node.id), flow.nodes.map(node => node.id));
assert.deepEqual(arranged.edges, flow.edges);
const stale = structuredClone(snapshot);
stale.navigation.cursor.revision += 1;
assert.throws(() => projectCurrent(stale), /stale portable navigation identity/);
const unknown = structuredClone(snapshot);
unknown.navigation.cursor.focus = "absent/graph-subject";
assert.throws(() => projectCurrent(unknown), /invalid portable navigation cursor/);
process.stdout.write(JSON.stringify({
  subjects: projected.subjects,
  relationships: projected.relationships,
  properties: projected.properties,
  observation: observeCurrent(snapshot),
  edges: flow.edges.map(edge => ({id: edge.id, source: edge.source, target: edge.target,
    source_port: edge.sourceHandle, sink_port: edge.targetHandle})),
  nodes: flow.nodes.map(node => ({id: node.id, role: node.data.role,
    label: node.data.label, ports: node.data.ports})),
  cord_count: flow.cordCount,
  stale_rejected: true, unknown_rejected: true,
}));
