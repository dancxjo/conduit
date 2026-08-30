const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function createBodyBirthRunner({ source, sourceKey, listingId, host, draft, onDraft, nextSequence, onBodyChanged }) {
  const runner = document.createElement("section");
  runner.className = "runner body-birth-runner";
  runner.dataset.sourceKey = sourceKey;
  runner.innerHTML = `
    <div class="editor birth-editor">
      <label class="editor-label" for="body-program">Initial program</label>
      <select id="body-program" aria-label="Initial program"><option value="morse-network@1">Morse Network</option></select>
      <label class="editor-label" for="body-friendly-name">Friendly name · editable later</label>
      <input id="body-friendly-name" maxlength="64" aria-label="Friendly Body name">
      <details class="seed-source"><summary>Reviewed program source</summary>
        <label class="editor-label" for="${listingId}">Conduit Seed</label>
        <textarea id="${listingId}" spellcheck="false" aria-label="Conduit Seed source"></textarea>
      </details>
      <div class="actions"><button class="birth" type="button">Birth Body</button></div>
    </div>
    <div class="result body-birth-result">
      <div class="body-chain" aria-label="Seed to Body lifecycle">
        <article><span>checked Seed</span><code class="seed-id">not born</code></article>
        <b aria-hidden="true">BIRTH →</b>
        <article><span>durable Body</span><strong class="body-state">not born</strong><code class="body-id"></code></article>
      </div>
      <p class="birth-status" role="status">Edit the Seed, then explicitly birth one Body.</p>
      <dl class="body-identities"></dl>
      <details class="body-raw"><summary>Raw Body and membership evidence</summary><pre><code></code></pre></details>
    </div>`;
  const textarea = runner.querySelector("textarea");
  const friendlyName = runner.querySelector("#body-friendly-name");
  const button = runner.querySelector(".birth");
  textarea.value = draft ?? source;
  friendlyName.value = generatedFriendlyName();
  textarea.addEventListener("input", () => onDraft(textarea.value));
  button.addEventListener("click", () => birth(
    runner,
    host,
    friendlyName.value,
    runner.querySelector("#body-program").value,
    textarea.value,
    nextSequence(),
    onBodyChanged,
  ));

  const current = readCurrent(host.runtime);
  if (current) renderReceipt(runner, current, true);
  return runner;
}

function birth(runner, host, friendlyName, initialProgram, source, sequence, onBodyChanged) {
  const api = host.runtime;
  const sourceBytes = encoder.encode(source);
  const hostBytes = encoder.encode(host.hostId);
  const bootBytes = encoder.encode(host.bootId);
  const nameBytes = encoder.encode(friendlyName.trim());
  const programBytes = encoder.encode(initialProgram);
  const total = hostBytes.length + bootBytes.length + nameBytes.length + programBytes.length + sourceBytes.length;
  const status = runner.querySelector(".birth-status");
  status.classList.remove("error");
  if (total > api.conduit_book_body_input_capacity()) {
    status.textContent = "The Seed and exact Host identities exceed the admitted BIRTH input bound.";
    status.classList.add("error");
    return;
  }
  const input = new Uint8Array(api.memory.buffer, api.conduit_book_body_input_ptr(), total);
  input.set(sourceBytes);
  const admitted = api.conduit_book_body_admit_source_interaction(sourceBytes.length, BigInt(sequence));
  if (admitted < 0) {
    renderRefusal(runner, api, admitted);
    return;
  }
  input.set(hostBytes);
  input.set(bootBytes, hostBytes.length);
  input.set(nameBytes, hostBytes.length + bootBytes.length);
  input.set(programBytes, hostBytes.length + bootBytes.length + nameBytes.length);
  input.set(sourceBytes, hostBytes.length + bootBytes.length + nameBytes.length + programBytes.length);
  const code = api.conduit_book_body_birth(
    hostBytes.length,
    bootBytes.length,
    nameBytes.length,
    programBytes.length,
    sourceBytes.length,
    BigInt(sequence),
  );
  if (code < 0) {
    renderRefusal(runner, api, code);
    return;
  }
  renderReceipt(runner, readOutput(api), false);
  onBodyChanged?.();
}

function readCurrent(api) {
  const code = api.conduit_book_body_current();
  if (code === 1) return null;
  if (code < 0) throw new Error(`current Body projection refused (${code})`);
  return readOutput(api);
}

export function readBodyProjection(api) {
  return readCurrent(api);
}

function renderRefusal(runner, api, code) {
  const refusal = api.conduit_book_body_output_len() > 0 ? readOutput(api) : null;
  const status = runner.querySelector(".birth-status");
  status.textContent = refusal?.message
    ? `BIRTH refused · ${refusal.category}: ${refusal.message}`
    : `BIRTH refused (${code}).`;
  status.classList.add("error");
}

function renderReceipt(runner, receipt, retained) {
  runner.dataset.bodyId = receipt.body_id;
  runner.dataset.birthSignId = receipt.birth_sign_id;
  runner.querySelector("textarea").disabled = true;
  runner.querySelector("#body-friendly-name").disabled = true;
  runner.querySelector("#body-program").disabled = true;
  runner.querySelector(".birth").disabled = true;
  runner.querySelector(".seed-id").textContent = receipt.seed_id;
  runner.querySelector(".body-id").textContent = receipt.body_id;
  runner.querySelector(".body-state").textContent = receipt.state;
  runner.querySelector(".birth-status").textContent = retained
    ? "Same LULLED Body retained — Crèche presentation controls did not recreate it."
    : "Born — one checked Seed now has one LULLED Body; no Wake, Plan, or Play exists.";
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

export function createFirstHostRunner({ host, nextSequence, onBodyChanged }) {
  const runner = document.createElement("section");
  runner.className = "runner first-host-runner";
  runner.innerHTML = `
    <div class="editor">
      <p>This browser is available, but availability is not membership.</p>
      <div class="actions"><button class="attach-host" type="button">Give this Body its first Host</button></div>
    </div>
    <div class="result">
      <p class="host-admission-status" role="status">The Body is still LULLED with no admitted Host.</p>
      <dl class="host-identities"></dl>
    </div>`;
  const current = readCurrent(host.runtime);
  if (!current) {
    runner.querySelector(".attach-host").disabled = true;
    runner.querySelector(".host-admission-status").textContent = "Birth the Body on page zero first.";
    return runner;
  }
  if (current.here_part_id) renderAttachedHost(runner, current);
  runner.querySelector(".attach-host").addEventListener("click", () => {
    const api = host.runtime;
    const hostBytes = encoder.encode(host.hostId);
    const bootBytes = encoder.encode(host.bootId);
    const input = new Uint8Array(api.memory.buffer, api.conduit_book_body_input_ptr(), hostBytes.length + bootBytes.length);
    input.set(hostBytes);
    input.set(bootBytes, hostBytes.length);
    const code = api.conduit_book_body_attach_here(hostBytes.length, bootBytes.length, BigInt(nextSequence()));
    if (code < 0) {
      const refusal = readOutput(api);
      const status = runner.querySelector(".host-admission-status");
      status.textContent = `Host admission refused: ${refusal.message ?? code}`;
      status.classList.add("error");
      return;
    }
    renderAttachedHost(runner, readOutput(api));
    onBodyChanged?.();
  });
  return runner;
}

function renderAttachedHost(runner, receipt) {
  runner.querySelector(".attach-host").disabled = true;
  runner.querySelector(".host-admission-status").textContent = `${receipt.friendly_name} now has one admitted browser Host and remains LULLED.`;
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
    api.conduit_book_body_output_ptr(),
    api.conduit_book_body_output_len(),
  );
  return JSON.parse(decoder.decode(bytes));
}
