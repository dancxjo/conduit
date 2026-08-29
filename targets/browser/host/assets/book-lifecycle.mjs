const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function createBodyBirthRunner({ source, sourceKey, listingId, host, draft, onDraft, nextSequence }) {
  const runner = document.createElement("section");
  runner.className = "runner body-birth-runner";
  runner.dataset.sourceKey = sourceKey;
  runner.innerHTML = `
    <div class="editor">
      <label class="editor-label" for="${listingId}">Conduit Seed · editable before BIRTH</label>
      <textarea id="${listingId}" spellcheck="false" aria-label="Editable Conduit Seed"></textarea>
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
  const button = runner.querySelector(".birth");
  textarea.value = draft ?? source;
  textarea.addEventListener("input", () => onDraft(textarea.value));
  button.addEventListener("click", () => birth(runner, host, textarea.value, nextSequence()));

  const current = readCurrent(host.runtime);
  if (current) renderReceipt(runner, current, true);
  return runner;
}

function birth(runner, host, source, sequence) {
  const api = host.runtime;
  const sourceBytes = encoder.encode(source);
  const hostBytes = encoder.encode(host.hostId);
  const bootBytes = encoder.encode(host.bootId);
  const total = hostBytes.length + bootBytes.length + sourceBytes.length;
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
  input.set(sourceBytes, hostBytes.length + bootBytes.length);
  const code = api.conduit_book_body_birth(
    hostBytes.length,
    bootBytes.length,
    sourceBytes.length,
    BigInt(sequence),
  );
  if (code < 0) {
    renderRefusal(runner, api, code);
    return;
  }
  renderReceipt(runner, readOutput(api), false);
}

function readCurrent(api) {
  const code = api.conduit_book_body_current();
  if (code === 1) return null;
  if (code < 0) throw new Error(`current Body projection refused (${code})`);
  return readOutput(api);
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
  runner.querySelector(".birth").disabled = true;
  runner.querySelector(".seed-id").textContent = receipt.seed_id;
  runner.querySelector(".body-id").textContent = receipt.body_id;
  runner.querySelector(".body-state").textContent = receipt.state;
  runner.querySelector(".birth-status").textContent = retained
    ? "Same LULLED Body retained — Tour presentation controls did not recreate it."
    : "Born — one checked Seed now has one LULLED Body; no Wake, Plan, or Play exists.";
  const identities = [
    ["Source document", receipt.source_document_id],
    ["Checked Form", receipt.checked_form_id],
    ["Seed", receipt.seed_id],
    ["BIRTH Sign", receipt.birth_sign_id],
    ["Body", receipt.body_id],
    ["Here Part", receipt.here_part_id],
    ["Current Host", receipt.host_id],
    ["Current Boot", receipt.boot_id],
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

function readOutput(api) {
  const bytes = new Uint8Array(
    api.memory.buffer,
    api.conduit_book_body_output_ptr(),
    api.conduit_book_body_output_len(),
  );
  return JSON.parse(decoder.decode(bytes));
}
