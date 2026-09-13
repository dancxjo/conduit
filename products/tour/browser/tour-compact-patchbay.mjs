export const COMPACT_PATCHBAY_CONTRACT = Object.freeze({
  schema: "conduit.tour/compact-patchbay-contract@1",
  truthSource: "checked-form-projection",
  operationMode: "inspection-only",
  operationVocabulary: "patchbay-control@1",
  supportedOperations: Object.freeze(["open-back"]),
});

export function compactPatchbaySnapshot(projection, options = {}) {
  const subjects = [];
  const relationships = [];
  const properties = [];
  const gears = options.realizationTopology ? projection.realization_gears : projection.gears;
  const cords = options.realizationTopology ? projection.realization_cords : projection.cords;
  const addProperty = (subject, name, value) => properties.push({ subject, name, value: { Text: value } });
  const portIdentity = (gearId, direction, portId) => `${gearId}.${direction}:${portId}`;
  const diagnosticSubjects = new Set(projection.diagnostics.flatMap((diagnostic) => diagnostic.subjects));
  for (const gear of gears) {
    subjects.push({ identity: gear.gear_id, role: "Gear", label: gear.gear_id, accessibility_name: `Gear ${gear.gear_id}` });
    addProperty(gear.gear_id, "kind-id", gear.kind_id);
    if (diagnosticSubjects.has(gear.gear_id)) addProperty(gear.gear_id, "diagnostic-state", "error");
    if (options.reviewedBack && gear.kind_id === "text/morse") {
      addProperty(gear.gear_id, "reviewed-back", "available");
      addProperty(gear.gear_id, "back-expanded", String(options.backExpanded === true));
    }
    for (const [direction, ports] of [["receiving", gear.inputs], ["emitting", gear.outputs]]) {
      for (const port of ports) {
        const identity = portIdentity(gear.gear_id, direction, port.port_id);
        subjects.push({ identity, role: "Port", label: port.port_id, accessibility_name: `${direction} Port ${identity}` });
        relationships.push({ source: gear.gear_id, target: identity, kind: "Contains" });
        addProperty(identity, "semantic-id", identity);
        addProperty(identity, "direction", direction);
        addProperty(identity, "value-kind", port.info_kind);
        addProperty(identity, "temporal", port.temporal);
        if (diagnosticSubjects.has(identity)) addProperty(identity, "diagnostic-state", "error");
      }
    }
  }
  for (const [index, cord] of cords.entries()) {
    const identity = `cord:${index}:${cord.source_gear_id}.${cord.source_port_id}->${cord.sink_gear_id}.${cord.sink_port_id}`;
    subjects.push({ identity, role: "Cord", label: `Cord ${index + 1}`, accessibility_name: `Cord from ${cord.source_gear_id}.${cord.source_port_id} to ${cord.sink_gear_id}.${cord.sink_port_id}` });
    addProperty(identity, "source-port", portIdentity(cord.source_gear_id, "emitting", cord.source_port_id));
    addProperty(identity, "sink-port", portIdentity(cord.sink_gear_id, "receiving", cord.sink_port_id));
    addProperty(identity, "value-kind", cord.info_kind);
    addProperty(identity, "flow-animation", "directional");
    addProperty(identity, "flow-label", "");
    if (cord.invalid || diagnosticSubjects.has(identity)) addProperty(identity, "diagnostic-state", "error");
  }
  return {
    contract: COMPACT_PATCHBAY_CONTRACT,
    presentation: {
      identity: projection.visible_expanded_form_id || projection.source_proposal_id,
      revision: projection.sequence,
      basis: { source_document_id: projection.source_document_id, checked_form_id: projection.checked_form_id },
      subjects, relationships, properties, text: [], actions: [], disclosures: [],
    },
    interaction: { revision: projection.sequence, selected_subject: null },
  };
}
