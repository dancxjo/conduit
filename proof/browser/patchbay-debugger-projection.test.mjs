import assert from "node:assert/strict";
import test from "node:test";

import { projectFlowScene } from "../../apps/patchbay/html/assets/flow-scene.js";

function snapshot(reducedMotion = false) {
  const gear = "gear/source";
  const output = "port/source/out";
  const sink = "gear/sink";
  const input = "port/sink/in";
  const cord = "cord/source-sink";
  const subjects = [
    { identity: gear, role: "Gear", label: "Source", accessibility_name: "Source Gear" },
    { identity: output, role: "Port", label: "out", accessibility_name: "Source output" },
    { identity: sink, role: "Gear", label: "Sink", accessibility_name: "Sink Gear" },
    { identity: input, role: "Port", label: "in", accessibility_name: "Sink input" },
    { identity: cord, role: "Cord", label: "Cord", accessibility_name: "Source to sink Cord" },
  ];
  const property = (subject, name, value) => ({ subject, name, value: { Text: value } });
  return {
    presentation: {
      identity: "presentation/debugger",
      revision: 1,
      basis: { source_document_id: "source/debugger", checked_form_id: "form/debugger" },
      subjects,
      relationships: [
        { source: gear, target: output, kind: "Contains" },
        { source: sink, target: input, kind: "Contains" },
      ],
      properties: [
        property(output, "semantic-id", output),
        property(input, "semantic-id", input),
        property(output, "direction", "emitting"),
        property(input, "direction", "receiving"),
        property(cord, "source-port", output),
        property(cord, "sink-port", input),
      ],
      text: [], actions: [], disclosures: [],
    },
    interaction: { revision: 1, selected_subject: null },
    debugger: {
      revision: 12,
      reduced_motion: reducedMotion,
      gap: { dropped_records: 9_999, first_retained_sequence: 10_000 },
      activities: [{
        subject: cord,
        phase: "active",
        latest_kind: "value-sent",
        observed_count: 10_000,
        latest_value: { summary: "42" },
      }],
    },
  };
}

test("live debugger activity stays on its exact Cord and remains meaningful without motion", () => {
  const moving = projectFlowScene(snapshot());
  assert.equal(moving.edges.length, 1);
  assert.equal(moving.edges[0].id, "cord/source-sink");
  assert.equal(moving.edges[0].animated, true);
  assert.equal(moving.edges[0].label, "42 · 10000 observed");
  assert.match(moving.edges[0].className, /debugger-active/);
  assert.match(moving.edges[0].className, /debugger-gap/);

  const reduced = projectFlowScene(snapshot(true));
  assert.equal(reduced.edges[0].animated, false);
  assert.equal(reduced.edges[0].label, "42 · 10000 observed");
  assert.equal(reduced.edges[0].data.debugger.subject, "cord/source-sink");
});
