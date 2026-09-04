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
      { parent: null, component: "action-group", action: null, key: "physical-artifact", text: "Prepared spore artifact" },
      { parent: 0, component: "button", action: 0, key: "download-spore", text: `Download ${artifact.format.toUpperCase()} · ${artifact.bytes} bytes` },
    ] : [{ parent: null, component: "paragraph", action: null, key: "artifact-not-ready", text: "Artifact handoff not ready" }],
  }, { onEvent(event) {
    state.presentation.nextEvent("physical-artifact");
    if (event.action === "artifact.handoff") void onHandoff(artifact);
  } });
}

export function presentPhysicalActions(state) {
  const actions = ["bind", "realize", "observe", "admit", "cancel"];
  const labels = {
    bind: "Bind Body invitation",
    realize: "Realize selected Host",
    observe: "Observe Boot and join",
    admit: "Admit Part and offers",
    cancel: "Cancel current operation",
  };
  state.presentation.present("physical-actions", {
    revision: ++state.presentationRevision,
    actions: actions.map((id) => ({ id: `physical.${id}`, event: "activate" })),
    nodes: [
      { parent: null, component: "action-group", action: null, key: "physical-actions", text: "Physical Host actions" },
      ...actions.map((id, index) => ({
        parent: 0,
        component: "button",
        state: (id === "cancel" ? state.cancelEnabled : state.actionEnabled === id) ? "ready" : "unavailable",
        action: (id === "cancel" ? state.cancelEnabled : state.actionEnabled === id) ? index : null,
        key: `physical-${id}`,
        text: labels[id],
      })),
    ],
  }, { onEvent(event) {
    state.presentation.nextEvent("physical-actions");
    state.runAction(event.action.slice("physical.".length));
  } });
}

export function presentPhysicalProgress(state) {
  const stages = [
    ["obtain", "1 · Obtain exact machinery"],
    ["bind", "2 · Bind invitation"],
    ["realize", "3 · Deploy, install, or attach"],
    ["observe", "4 · Observe Boot + join"],
    ["admit", "5 · Admit Part + offers"],
  ];
  const completed = stages.filter(([id]) => state.stages[id] !== "waiting").length;
  state.presentation.present("physical-progress", {
    revision: ++state.presentationRevision,
    actions: [],
    nodes: [
      { parent: null, component: "stack", action: null, key: "physical-progress-stack", text: "" },
      { parent: 0, component: "progress", action: null, key: "physical-progress", text: "Add one physical Host", value: `${completed}/${stages.length}`, valueCapacity: 11 },
      { parent: 0, component: "definition-table", action: null, key: "physical-stages", text: "Physical Host stages" },
      ...stages.map(([id, label]) => ({ parent: 2, component: "definition", action: null, key: `physical-stage-${id}`, text: label, value: state.stages[id], valueCapacity: 256 })),
    ],
  });
}

export function presentPhysicalSelection(state) {
  const controlState = state.selectionDisabled ? "unavailable" : "ready";
  const support = state.entry.intentions;
  state.presentation.present("physical-mode-control", {
    revision: ++state.presentationRevision,
    actions: state.selectionDisabled ? [] : [{ id: "physical.mode", event: "change" }],
    nodes: [
      { parent: null, component: "form-field", action: null, key: "physical-mode-field", text: "" },
      { parent: 0, component: "field-label", action: null, key: "physical-mode-label", text: "Intention" },
      { parent: 0, component: "select", state: controlState, action: state.selectionDisabled ? null : 0, key: "physical-mode", text: "Intention", value: state.mode, valueCapacity: 64 },
      { parent: 0, component: "field-help", action: null, key: "physical-mode-help", text: "Choose fabrication, installation, or attachment without implying lifecycle progress." },
      ...state.intentions.map((intention, index) => {
        const available = support.find((candidate) => candidate.id === intention.id).supported;
        return { parent: 2, component: "option", action: null, key: `physical-mode-${index}`, text: `${intention.label}${available ? "" : " · unavailable for this target"}`, value: intention.id, valueCapacity: 64 };
      }),
    ],
  }, { onEvent(event) {
    state.presentation.nextEvent("physical-mode-control");
    state.changeMode(new TextDecoder().decode(event.value));
  } });
  state.presentation.present("physical-target-control", {
    revision: ++state.presentationRevision,
    actions: state.selectionDisabled ? [] : [{ id: "physical.target", event: "change" }],
    nodes: [
      { parent: null, component: "form-field", action: null, key: "physical-target-field", text: "" },
      { parent: 0, component: "field-label", action: null, key: "physical-target-label", text: "Target" },
      { parent: 0, component: "select", state: controlState, action: state.selectionDisabled ? null : 0, key: "physical-target", text: "Target", value: state.entry.target.id, valueCapacity: 256 },
      { parent: 0, component: "field-help", action: null, key: "physical-target-help", text: "Each option retains its authoritative catalog family, model, and profile identity." },
      ...state.catalog.entries.map((entry, index) => ({ parent: 2, component: "option", action: null, key: `physical-target-${index}`, text: `${entry.family.label} · ${entry.target.label}`, value: entry.target.id, valueCapacity: 256 })),
    ],
  }, { onEvent(event) {
    state.presentation.nextEvent("physical-target-control");
    state.changeTarget(new TextDecoder().decode(event.value));
  } });
}
