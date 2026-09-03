export function presentPhysicalStatus(state, message, error = false) {
  state.presentation.present("physical-status", {
    revision: ++state.presentationRevision,
    actions: [],
    nodes: [{
      parent: null,
      component: error ? "failure-status" : "status",
      action: null,
      key: "physical-status",
      text: message,
    }],
  });
}

export function presentPhysicalEvidence(state, evidence, disposition) {
  state.presentation.present("physical-evidence", {
    revision: ++state.presentationRevision,
    actions: [],
    nodes: [
      { parent: null, component: disposition, action: null, key: "physical-evidence", text: "Physical Host workflow evidence" },
      { parent: 0, component: "disclosure", action: null, key: "physical-evidence-raw", text: "Exact physical-Host evidence" },
      { parent: 1, component: "code-block", action: null, key: "physical-evidence-json", text: "json", value: JSON.stringify(evidence, null, 2), valueCapacity: 65_536 },
    ],
  });
}
