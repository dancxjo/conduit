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

export function presentPhysicalArtifact(state, artifact, onHandoff) {
  const ready = artifact !== null;
  state.presentation.present("physical-artifact", {
    revision: ++state.presentationRevision,
    actions: ready ? [{ id: "artifact.handoff", event: "activate" }] : [],
    nodes: ready ? [
      { parent: null, component: "action-group", action: null, key: "physical-artifact", text: "" },
      { parent: 0, component: "button", action: 0, key: "download-spore", text: `Download ${artifact.format.toUpperCase()} · ${artifact.bytes} bytes` },
    ] : [{ parent: null, component: "paragraph", action: null, key: "artifact-not-ready", text: "Artifact handoff not ready" }],
  }, { onEvent(event) {
    state.presentation.nextEvent("physical-artifact");
    if (event.action === "artifact.handoff") void onHandoff(artifact);
  } });
}
