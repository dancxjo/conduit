const decoder = new TextDecoder();
const encoder = new TextEncoder();
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
  const present = () => presentGraduationControls(runner, host.runtime, presentation, state, {
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

function presentGraduationControls(runner, runtime, presentation, state, { onChoice, onEnd }) {
  const ready = state.readiness?.ready === true;
  const readiness = state.readiness ?? {};
  presentGraduationView(runtime, presentation, "graduation-controls", {
    mode: "controls",
    revision: ++state.revision,
    durable_identity: readiness.durable_identity === true,
    birth_evidence: readiness.birth_evidence === true,
    current_admitted_part: readiness.current_admitted_part === true,
    ready,
    graduated: state.graduated,
    status: state.status,
    status_kind: state.outcome === "failure-status" ? "failure" : state.outcome === "success-status" ? "success" : "ordinary",
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
  const rawEvidence = JSON.stringify(evidence, null, 2);
  presentGraduationView(api, presentation, "graduation-evidence", {
    mode: "evidence",
    revision: ++state.revision,
    body_id: receipt.body_id,
    choice: evidence.choice,
    sign_id: evidence.sign_id,
    patchbay_plan_id: evidence.patchbay_plan_id ?? null,
    patchbay_implementation_id: evidence.patchbay_implementation_id ?? null,
    creche_required: evidence.creche_required,
    canonical_json: rawEvidence,
  });
  renderBiography(presentation, "graduation-biography", call(api, "conduit_creche_biography"), ++state.revision);
}

function presentGraduationView(runtime, presentation, slot, request, options = {}) {
  const encoded = encoder.encode(JSON.stringify(request));
  if (encoded.length > runtime.conduit_creche_input_capacity()) {
    throw new Error("Crèche graduation presentation request exceeds its bound");
  }
  new Uint8Array(runtime.memory.buffer, runtime.conduit_creche_input_ptr(), encoded.length).set(encoded);
  const code = runtime.conduit_creche_graduation_view(encoded.length);
  if (code < 0) {
    let message = `Crèche graduation presentation refused (${code})`;
    try { message = readOutput(runtime).message ?? message; } catch {}
    throw new Error(message);
  }
  const view = new Uint8Array(runtime.memory.buffer, runtime.conduit_creche_output_ptr(), runtime.conduit_creche_output_len()).slice();
  presentation.present(slot, view, options);
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
