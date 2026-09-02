const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function createBodyBirthRunner({ source, sourceKey, listingId, host, presentationFor, draft, onDraft, nextSequence, onBodyChanged }) {
  const runner = document.createElement("section");
  runner.className = "runner body-birth-runner";
  runner.dataset.sourceKey = sourceKey;
  runner.innerHTML = `
    <div class="birth-presentation" data-application-slot="birth-controls"></div>
    <div class="result body-birth-result">
      <div class="body-chain" aria-label="Seed to Body lifecycle">
        <article><span>checked Seed</span><code class="seed-id">not born</code></article>
        <b aria-hidden="true">BIRTH →</b>
        <article><span>durable Body</span><strong class="body-state">not born</strong><code class="body-id"></code></article>
      </div>
      <dl class="body-identities"></dl>
      <details class="body-raw"><summary>Raw Body and membership evidence</summary><pre><code></code></pre></details>
    </div>`;
  const presentation = presentationFor(runner);
  const state = {
    revision: 0,
    friendlyName: generatedFriendlyName(),
    initialProgram: "morse-network@1",
    source: draft ?? source,
    status: "Edit the Seed, then explicitly birth one Body.",
    outcome: "status",
    terminal: false,
  };
  presentBirthControls(runner, state, { presentation, listingId, onDraft, onBirth() {
    birth(runner, host, state, nextSequence(), onBodyChanged, { presentation, listingId, onDraft });
  } });

  const current = readCurrent(host.runtime);
  if (current) renderReceipt(runner, current, true, state, { presentation, listingId, onDraft });
  return runner;
}

function presentBirthControls(runner, state, { presentation, listingId, onDraft, onBirth = () => {} }) {
  const actions = state.terminal ? [] : [
    { id: "program.change", event: "change" },
    { id: "name.input", event: "input" },
    { id: "source.input", event: "input" },
    { id: "birth.activate", event: "activate" },
  ];
  presentation.present("birth-controls", {
    revision: ++state.revision,
    actions,
    nodes: [
      { parent: null, component: "stack", action: null, key: "birth-editor", text: "" },
      { parent: 0, component: "select", action: state.terminal ? null : 0, key: "body-program", text: "Initial program", value: state.initialProgram, valueCapacity: 64 },
      { parent: 1, component: "option", action: null, key: "morse-program", text: "Morse Network", value: "morse-network@1", valueCapacity: 64 },
      { parent: 0, component: "text-input", action: state.terminal ? null : 1, key: "body-friendly-name", text: "Friendly Body name", value: state.friendlyName, valueCapacity: 64 },
      { parent: 0, component: "disclosure", action: null, key: "seed-source", text: "" },
      { parent: 4, component: "summary", action: null, key: "seed-summary", text: "Reviewed program source" },
      { parent: 4, component: "textarea", action: state.terminal ? null : 2, key: listingId, text: "Conduit Seed source", value: state.source, valueCapacity: 65_536 },
      { parent: 0, component: "action-group", action: null, key: "birth-actions", text: "" },
      { parent: 7, component: "button", action: state.terminal ? null : 3, key: "birth", text: "Birth Body" },
      { parent: 0, component: state.outcome, action: null, key: "birth-status", text: state.status },
    ],
  }, { onEvent(event) {
    presentation.nextEvent("birth-controls");
    const value = decoder.decode(event.value);
    if (event.action === "program.change") state.initialProgram = value;
    if (event.action === "name.input") state.friendlyName = value;
    if (event.action === "source.input") { state.source = value; onDraft(value); }
    if (event.action === "birth.activate") onBirth();
  } });
}

function birth(runner, host, state, sequence, onBodyChanged, presentationOptions) {
  const api = host.runtime;
  const sourceBytes = encoder.encode(state.source);
  const hostBytes = encoder.encode(host.hostId);
  const bootBytes = encoder.encode(host.bootId);
  const nameBytes = encoder.encode(state.friendlyName.trim());
  const programBytes = encoder.encode(state.initialProgram);
  const total = hostBytes.length + bootBytes.length + nameBytes.length + programBytes.length + sourceBytes.length;
  if (total > api.conduit_creche_input_capacity()) {
    state.status = "The Seed and exact Host identities exceed the admitted BIRTH input bound.";
    state.outcome = "failure-status";
    presentBirthControls(runner, state, presentationOptions);
    return;
  }
  const input = new Uint8Array(api.memory.buffer, api.conduit_creche_input_ptr(), total);
  input.set(sourceBytes);
  const admitted = api.conduit_creche_admit_source_interaction(sourceBytes.length, BigInt(sequence));
  if (admitted < 0) {
    renderRefusal(runner, api, admitted, state, presentationOptions);
    return;
  }
  input.set(hostBytes);
  input.set(bootBytes, hostBytes.length);
  input.set(nameBytes, hostBytes.length + bootBytes.length);
  input.set(programBytes, hostBytes.length + bootBytes.length + nameBytes.length);
  input.set(sourceBytes, hostBytes.length + bootBytes.length + nameBytes.length + programBytes.length);
  const code = api.conduit_creche_birth(
    hostBytes.length,
    bootBytes.length,
    nameBytes.length,
    programBytes.length,
    sourceBytes.length,
    BigInt(sequence),
  );
  if (code < 0) {
    renderRefusal(runner, api, code, state, presentationOptions);
    return;
  }
  renderReceipt(runner, readOutput(api), false, state, presentationOptions);
  onBodyChanged?.();
}

function readCurrent(api) {
  const code = api.conduit_creche_current();
  if (code === 1) return null;
  if (code < 0) throw new Error(`current Body projection refused (${code})`);
  return readOutput(api);
}

export function readBodyProjection(api) {
  return readCurrent(api);
}

function renderRefusal(runner, api, code, state, presentationOptions) {
  const refusal = api.conduit_creche_output_len() > 0 ? readOutput(api) : null;
  state.status = refusal?.message
    ? `BIRTH refused · ${refusal.category}: ${refusal.message}`
    : `BIRTH refused (${code}).`;
  state.outcome = "failure-status";
  presentBirthControls(runner, state, presentationOptions);
}

function renderReceipt(runner, receipt, retained, state, presentationOptions) {
  runner.dataset.bodyId = receipt.body_id;
  runner.dataset.birthSignId = receipt.birth_sign_id;
  state.terminal = true;
  state.friendlyName = receipt.friendly_name;
  state.initialProgram = receipt.initial_program;
  runner.querySelector(".seed-id").textContent = receipt.seed_id;
  runner.querySelector(".body-id").textContent = receipt.body_id;
  runner.querySelector(".body-state").textContent = receipt.state;
  state.status = retained
    ? "Same LULLED Body retained — Crèche presentation controls did not recreate it."
    : "Born — one checked Seed now has one LULLED Body; no Wake, Plan, or Play exists.";
  state.outcome = "success-status";
  presentBirthControls(runner, state, presentationOptions);
  const identities = [
    ["Friendly name", receipt.friendly_name],
    ["Initial program", receipt.initial_program],
    ["Source document", receipt.source_document_id],
    ["Checked Form", receipt.checked_form_id],
    ["Seed", receipt.seed_id],
    ["BIRTH Sign", receipt.birth_sign_id],
    ["Body", receipt.body_id],
    ["Here Part", receipt.here_part_id ?? "none yet"],
    ["Current Host", receipt.host_id ?? "none yet"],
    ["Current Boot", receipt.boot_id ?? "none yet"],
    ["Membership revision", String(receipt.membership_revision)],
    ["Wake", receipt.wake_id ?? "none"],
    ["Plan", receipt.plan_id ?? "none"],
    ["Active Play", receipt.active_play_id ?? "none"],
  ];
  const list = runner.querySelector(".body-identities");
  list.replaceChildren();
  for (const [label, value] of identities) {
    const term = document.createElement("dt");
    const description = document.createElement("dd");
    term.textContent = label;
    description.textContent = value;
    list.append(term, description);
  }
  runner.querySelector(".body-raw code").textContent = JSON.stringify({
    body: receipt.raw_body,
    membership: receipt.raw_membership,
    source_interaction: receipt.source_interaction,
  }, null, 2);
}

export function createFirstHostRunner({ host, presentationFor, nextSequence, onBodyChanged }) {
  const runner = document.createElement("section");
  runner.className = "runner first-host-runner";
  runner.innerHTML = `
    <div data-application-slot="first-host-controls"></div>
    <div class="result">
      <dl class="host-identities"></dl>
    </div>`;
  const presentation = presentationFor(runner);
  const state = {
    revision: 0,
    status: "The Body is still LULLED with no admitted Host.",
    outcome: "status",
    terminal: false,
  };
  const current = readCurrent(host.runtime);
  if (!current) {
    state.terminal = true;
    state.status = "Birth the Body on page zero first.";
    presentFirstHostControls(runner, presentation, state, () => {});
    return runner;
  }
  if (current.here_part_id) {
    renderAttachedHost(runner, presentation, current, state);
    return runner;
  }
  presentFirstHostControls(runner, presentation, state, () => {
    const api = host.runtime;
    const hostBytes = encoder.encode(host.hostId);
    const bootBytes = encoder.encode(host.bootId);
    const input = new Uint8Array(api.memory.buffer, api.conduit_creche_input_ptr(), hostBytes.length + bootBytes.length);
    input.set(hostBytes);
    input.set(bootBytes, hostBytes.length);
    const code = api.conduit_creche_attach_here(hostBytes.length, bootBytes.length, BigInt(nextSequence()));
    if (code < 0) {
      const refusal = readOutput(api);
      state.status = `Host admission refused: ${refusal.message ?? code}`;
      state.outcome = "failure-status";
      presentFirstHostControls(runner, presentation, state, () => {});
      return;
    }
    renderAttachedHost(runner, presentation, readOutput(api), state);
    onBodyChanged?.();
  });
  return runner;
}

function presentFirstHostControls(runner, presentation, state, onAttach) {
  presentation.present("first-host-controls", {
    revision: ++state.revision,
    actions: state.terminal ? [] : [{ id: "host.attach", event: "activate" }],
    nodes: [
      { parent: null, component: "stack", action: null, key: "first-host", text: "" },
      { parent: 0, component: "paragraph", action: null, key: "availability", text: "This browser is available, but availability is not membership." },
      { parent: 0, component: "action-group", action: null, key: "host-actions", text: "" },
      { parent: 2, component: "button", action: state.terminal ? null : 0, key: "attach-host", text: "Give this Body its first Host" },
      { parent: 0, component: state.outcome, action: null, key: "host-status", text: state.status },
    ],
  }, { onEvent() {
    presentation.nextEvent("first-host-controls");
    onAttach();
  } });
}

function renderAttachedHost(runner, presentation, receipt, state) {
  state.terminal = true;
  state.outcome = "success-status";
  state.status = `${receipt.friendly_name} now has one admitted browser Host and remains LULLED.`;
  presentFirstHostControls(runner, presentation, state, () => {});
  const values = [["Body", receipt.body_id], ["Part", receipt.here_part_id], ["Host", receipt.host_id], ["Boot", receipt.boot_id]];
  const list = runner.querySelector(".host-identities");
  list.replaceChildren();
  for (const [label, value] of values) {
    const term = document.createElement("dt");
    const description = document.createElement("dd");
    term.textContent = label;
    description.textContent = value;
    list.append(term, description);
  }
}

function generatedFriendlyName() {
  const adjectives = ["brisk", "calm", "clever", "gentle", "lively", "steady"];
  const nouns = ["beacon", "finch", "lantern", "otter", "sparrow", "willow"];
  const bytes = new Uint8Array(2);
  crypto.getRandomValues(bytes);
  return `${adjectives[bytes[0] % adjectives.length]} ${nouns[bytes[1] % nouns.length]}`;
}

function readOutput(api) {
  const bytes = new Uint8Array(
    api.memory.buffer,
    api.conduit_creche_output_ptr(),
    api.conduit_creche_output_len(),
  );
  return JSON.parse(decoder.decode(bytes));
}
