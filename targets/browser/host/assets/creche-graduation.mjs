import { createApplicationPresentationHost } from "./application-presentation.mjs";

const decoder = new TextDecoder();

export function createGraduationRunner({ host, nextSequence, onBodyChanged, onEnd }) {
  const runner = document.createElement("section");
  runner.className = "graduation-runner";
  runner.innerHTML = `
    <div data-application-slot="graduation-controls"></div>
    <dl class="graduation-evidence"></dl>
    <section class="body-biography" aria-label="Body biography" hidden>
      <h3>Body biography · durable evidence</h3>
      <ol></ol>
    </section>
    <details><summary>Raw graduation evidence</summary><pre><code></code></pre></details>`;
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
  const present = () => presentGraduationControls(runner, state, {
    onChoice(choice) {
      const receipt = call(host.runtime, "conduit_creche_graduate", choice, BigInt(nextSequence()));
      renderGraduation(runner, receipt, host.runtime, state, present);
      onBodyChanged?.();
    },
    onEnd() { onEnd(call(host.runtime, "conduit_creche_current"), call(host.runtime, "conduit_creche_biography")); },
  });
  const current = currentBody(host.runtime);
  if (current?.graduation) renderGraduation(runner, current, host.runtime, state, present);
  else present();
  return runner;
}

function presentGraduationControls(runner, state, { onChoice, onEnd }) {
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
  const presentation = createApplicationPresentationHost(runner);
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
      { parent: 0, component: "action-group", action: null, key: "graduation-actions", text: "" },
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

function renderGraduation(runner, receipt, api, state = null, present = null) {
  const evidence = receipt.graduation;
  runner.dataset.bodyId = receipt.body_id;
  if (state) {
    state.graduated = true;
    state.outcome = "success-status";
    state.status = evidence.choice === "host-patchbay"
    ? "Graduated: an ordinary immutable Plan places Patchbay on the current browser Host."
    : "Graduated: no Patchbay was hosted; a compatible reader may project this Body later.";
    present();
  }
  const values = [
    ["Body", receipt.body_id], ["Choice", evidence.choice], ["Graduation Sign", evidence.sign_id],
    ["Patchbay Plan", evidence.patchbay_plan_id ?? "not hosted"],
    ["Patchbay implementation", evidence.patchbay_implementation_id ?? "not hosted"],
    ["Crèche required", String(evidence.creche_required)],
  ];
  const list = runner.querySelector(".graduation-evidence");
  list.replaceChildren();
  for (const [label, value] of values) {
    const dt = document.createElement("dt"); dt.textContent = label;
    const dd = document.createElement("dd"); dd.textContent = value;
    list.append(dt, dd);
  }
  runner.querySelector("details code").textContent = JSON.stringify(evidence, null, 2);
  renderBiography(runner.querySelector(".body-biography"), call(api, "conduit_creche_biography"));
}

export function renderBiography(container, biography) {
  if (!biography) return;
  const list = container.querySelector("ol");
  list.replaceChildren();
  for (const record of biography.records) {
    const [kind, facts] = Object.entries(record.kind)[0];
    const item = document.createElement("li");
    const heading = document.createElement("strong");
    heading.textContent = biographyHeading(kind);
    const explanation = document.createElement("p");
    explanation.textContent = biographyExplanation(kind, facts, biography);
    const proof = document.createElement("code");
    proof.textContent = `sequence ${record.sequence} · Sign ${record.sign_id}`;
    item.append(heading, explanation, proof);
    list.append(item);
  }
  container.dataset.bodyId = biography.body_id;
  container.hidden = false;
}

function biographyHeading(kind) {
  return ({ Born: "Born", PartAdmitted: "Part admitted", HostJoined: "Host joined", Graduated: "Graduated from the Crèche" })[kind] ?? kind;
}

function biographyExplanation(kind, facts, biography) {
  if (kind === "Born") return `${biography.friendly_name} began as Body ${biography.body_id} with ${biography.initial_program}.`;
  if (kind === "PartAdmitted") return `Part ${facts.part_id} entered this Body's admitted membership.`;
  if (kind === "HostJoined") return `Part ${facts.part_id} was observed on Host ${facts.host_id}, Boot ${facts.boot_id}.`;
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
