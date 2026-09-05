const decoder = new TextDecoder();
const MAX_BODY_EVIDENCE_EXPORT_BYTES = 65_536;

export function createGraduationRunner({ host, presentationFor, nextSequence, onBodyChanged, onEnd }) {
  const runner = document.createElement("section");
  runner.className = "graduation-runner";
  runner.innerHTML = `
    <div data-application-slot="graduation-controls"></div>
    <div data-application-slot="graduation-evidence"></div>
    <section class="body-biography" data-application-slot="graduation-biography"></section>`;
  const presentation = presentationFor(runner);
  const state = { revision: 0, readiness: null, graduated: false, status: "", outcome: "status" };
  try {
    state.readiness = call(host.runtime, "conduit_creche_graduation_readiness");
    state.status = state.readiness.ready
      ? "Ready: choose where this Body's ongoing history can be read."
      : "Not ready: birth the Body and admit its first current Host before graduating.";
  } catch (error) {
    state.status = error.message;
    state.outcome = "failure-status";
  }
  const present = () => presentGraduationControls(runner, presentation, state, {
    onChoice(choice) {
      const receipt = call(host.runtime, "conduit_creche_graduate", choice, BigInt(nextSequence()));
      renderGraduation(runner, receipt, host.runtime, presentation, state, present);
      onBodyChanged?.();
    },
    onEnd() { onEnd(call(host.runtime, "conduit_creche_current"), call(host.runtime, "conduit_creche_biography")); },
  });
  const current = currentBody(host.runtime);
  if (current?.graduation) renderGraduation(runner, current, host.runtime, presentation, state, present);
  else present();
  return runner;
}

function presentGraduationControls(runner, presentation, state, { onChoice, onEnd }) {
  const ready = state.readiness?.ready === true;
  const choiceActions = ready && !state.graduated;
  const actions = [
    ...(choiceActions ? [{ id: "graduate.host-patchbay", event: "activate" }, { id: "graduate.without-patchbay", event: "activate" }] : []),
    ...(state.graduated ? [{ id: "graduate.end", event: "activate" }] : []),
  ];
  const choiceOne = choiceActions ? 0 : null;
  const choiceTwo = choiceActions ? 1 : null;
  const endAction = state.graduated ? actions.length - 1 : null;
  const readiness = state.readiness ?? {};
  presentation.present("graduation-controls", {
    revision: ++state.revision,
    actions,
    nodes: [
      { parent: null, component: "stack", action: null, key: "graduation", text: "" },
      { parent: 0, component: "grid", action: null, key: "graduation-criteria", text: "" },
      { parent: 1, component: "panel", action: null, key: "durable-identity", text: `Durable Body identity · ${readiness.durable_identity ? "ready" : "waiting"}` },
      { parent: 1, component: "panel", action: null, key: "birth-evidence", text: `Bound BIRTH evidence · ${readiness.birth_evidence ? "ready" : "waiting"}` },
      { parent: 1, component: "panel", action: null, key: "current-part", text: `Current admitted Part · ${readiness.current_admitted_part ? "ready" : "waiting"}` },
      { parent: 0, component: state.outcome, action: null, key: "graduation-status", text: state.status },
      { parent: 0, component: "action-group", action: null, key: "graduation-actions", text: "Graduation actions" },
      { parent: 6, component: "button", action: choiceOne, key: "host-patchbay", text: "Host Patchbay on this Body" },
      { parent: 6, component: "button", action: choiceTwo, key: "without-patchbay", text: "Finish without hosted Patchbay" },
      { parent: 6, component: "button", action: endAction, key: "end-creche", text: "End the Crèche" },
    ],
  }, { onEvent(event) {
    presentation.nextEvent("graduation-controls");
    if (event.action === "graduate.host-patchbay") onChoice(1);
    if (event.action === "graduate.without-patchbay") onChoice(2);
    if (event.action === "graduate.end") onEnd();
  } });
}

function renderGraduation(runner, receipt, api, presentation, state, present) {
  const evidence = receipt.graduation;
  runner.dataset.bodyId = receipt.body_id;
  state.graduated = true;
  state.outcome = "success-status";
  state.status = evidence.choice === "host-patchbay"
    ? "Graduated: an ordinary immutable Plan places Patchbay on the current browser Host."
    : "Graduated: no Patchbay was hosted; a compatible reader may project this Body later.";
  present();
  const values = [
    ["Body", receipt.body_id], ["Choice", evidence.choice], ["Graduation Sign", evidence.sign_id],
    ["Patchbay Plan", evidence.patchbay_plan_id ?? "not hosted"],
    ["Patchbay implementation", evidence.patchbay_implementation_id ?? "not hosted"],
    ["Crèche required", String(evidence.creche_required)],
  ];
  const rawEvidence = JSON.stringify(evidence, null, 2);
  presentation.present("graduation-evidence", {
    revision: ++state.revision,
    actions: [],
    nodes: [
      { parent: null, component: "successful-evidence", action: null, key: "graduation-evidence", text: "Graduation evidence" },
      { parent: 0, component: "definition-table", action: null, key: "graduation-identities", text: "Exact graduation identities" },
      ...values.map(([label, value], index) => ({ parent: 1, component: "definition", action: null, key: `graduation-${index}`, text: label, value, valueCapacity: 256 })),
      { parent: 0, component: "disclosure", action: null, key: "graduation-raw", text: "Raw graduation evidence" },
      { parent: values.length + 2, component: "code-block", action: null, key: "graduation-raw-json", text: "json", value: rawEvidence, valueCapacity: 65_536 },
    ],
  });
  renderBiography(presentation, "graduation-biography", call(api, "conduit_creche_biography"), ++state.revision);
}

export function renderBiography(presentation, slot, biography, revision) {
  if (!biography) return;
  presentation.present(slot, {
    revision,
    actions: [],
    nodes: [
      { parent: null, component: "successful-evidence", action: null, key: "biography", text: "Body biography · durable evidence" },
      { parent: 0, component: "definition-table", action: null, key: "biography-records", text: `Body ${biography.body_id}` },
      ...biography.records.map((record, index) => {
        const [kind, facts] = Object.entries(record.kind)[0];
        return {
          parent: 1,
          component: "definition",
          action: null,
          key: `biography-record-${index}`,
          text: biographyHeading(kind),
          value: `${biographyExplanation(kind, facts, biography)} sequence ${record.sequence} · Sign ${record.sign_id}`,
          valueCapacity: 1024,
        };
      }),
    ],
  });
}

export function exportBodyEvidence(biography) {
  if (!biography || typeof biography.body_id !== "string" || !Array.isArray(biography.records)) {
    throw new Error("Body biography evidence is unavailable");
  }
  const encoded = new TextEncoder().encode(`${JSON.stringify(biography, null, 2)}\n`);
  if (encoded.length === 0 || encoded.length > MAX_BODY_EVIDENCE_EXPORT_BYTES) {
    throw new Error("Body biography evidence exceeds the export bound");
  }
  const url = URL.createObjectURL(new Blob([encoded], { type: "application/json" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = `conduit-body-${biography.body_id}.json`;
  link.click();
  URL.revokeObjectURL(url);
}

function biographyHeading(kind) {
  return ({ Born: "Born", PartAdmitted: "Part admitted", HostJoined: "Host joined", HostLeft: "Host left", PartRevoked: "Part revoked", Graduated: "Graduated from the Crèche" })[kind] ?? kind;
}

function biographyExplanation(kind, facts, biography) {
  if (kind === "Born") {
    const count = facts.initial_workset?.forms?.length ?? 0;
    return `${biography.friendly_name} began as Body ${biography.body_id} with ${count} initial active Form(s).`;
  }
  if (kind === "PartAdmitted") return `Part ${facts.part_id} entered this Body's admitted membership.`;
  if (kind === "HostJoined") return `Part ${facts.part_id} was observed on Host ${facts.host_id}, Boot ${facts.boot_id}.`;
  if (kind === "HostLeft") return `Part ${facts.part_id} left prior Boot ${facts.prior_boot_id} and remains admitted.`;
  if (kind === "PartRevoked") return `Part ${facts.part_id} was removed from Body membership.`;
  if (facts.choice === "HostedPatchbay") return `Patchbay was placed by Plan ${facts.patchbay_plan_id} using ${facts.patchbay_implementation_id}.`;
  return "No Patchbay was hosted. A compatible reader can project this same evidence later.";
}

function currentBody(api) {
  const code = api.conduit_creche_current();
  if (code === 1) return null;
  if (code < 0) throw new Error(`Body projection refused (${code})`);
  return readOutput(api);
}

function call(api, name, ...args) {
  const code = api[name](...args);
  if (code < 0) {
    const refusal = readOutput(api);
    throw new Error(refusal.message ?? `${name} refused (${code})`);
  }
  return readOutput(api);
}

function readOutput(api) {
  const bytes = new Uint8Array(api.memory.buffer, api.conduit_creche_output_ptr(), api.conduit_creche_output_len());
  return JSON.parse(decoder.decode(bytes));
}
