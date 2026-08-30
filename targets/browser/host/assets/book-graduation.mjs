const decoder = new TextDecoder();

export function createGraduationRunner({ host, nextSequence, onBodyChanged, onEnd }) {
  const runner = document.createElement("section");
  runner.className = "graduation-runner";
  runner.innerHTML = `
    <ul class="graduation-criteria" aria-label="Graduation readiness">
      <li data-criterion="durable_identity">Durable Body identity</li>
      <li data-criterion="birth_evidence">Bound BIRTH evidence</li>
      <li data-criterion="current_admitted_part">Current admitted Part</li>
    </ul>
    <p class="graduation-status" role="status"></p>
    <div class="graduation-actions">
      <button type="button" data-choice="1">Host Patchbay on this Body</button>
      <button type="button" data-choice="2">Finish without hosted Patchbay</button>
      <button type="button" class="end-creche" hidden>End the Crèche</button>
    </div>
    <dl class="graduation-evidence"></dl>
    <details><summary>Raw graduation evidence</summary><pre><code></code></pre></details>`;
  let readiness;
  try {
    readiness = call(host.runtime, "conduit_book_body_graduation_readiness");
    for (const key of ["durable_identity", "birth_evidence", "current_admitted_part"]) {
      runner.querySelector(`[data-criterion="${key}"]`).classList.toggle("ready", readiness[key]);
    }
    runner.querySelector(".graduation-status").textContent = readiness.ready
      ? "Ready: choose where this Body's ongoing history can be read."
      : "Not ready: birth the Body and admit its first current Host before graduating.";
  } catch (error) {
    runner.querySelector(".graduation-status").textContent = error.message;
  }
  for (const button of runner.querySelectorAll("[data-choice]")) {
    button.disabled = !readiness?.ready;
    button.addEventListener("click", () => {
      const receipt = call(host.runtime, "conduit_book_body_graduate", Number(button.dataset.choice), BigInt(nextSequence()));
      renderGraduation(runner, receipt);
      onBodyChanged?.();
    });
  }
  runner.querySelector(".end-creche").addEventListener("click", () => onEnd(call(host.runtime, "conduit_book_body_current")));
  const current = currentBody(host.runtime);
  if (current?.graduation) renderGraduation(runner, current);
  return runner;
}

function renderGraduation(runner, receipt) {
  for (const button of runner.querySelectorAll("[data-choice]")) button.disabled = true;
  const evidence = receipt.graduation;
  runner.dataset.bodyId = receipt.body_id;
  runner.querySelector(".graduation-status").textContent = evidence.choice === "host-patchbay"
    ? "Graduated: an ordinary immutable Plan places Patchbay on the current browser Host."
    : "Graduated: no Patchbay was hosted; a compatible reader may project this Body later.";
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
  runner.querySelector(".end-creche").hidden = false;
}

function currentBody(api) {
  const code = api.conduit_book_body_current();
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
  const bytes = new Uint8Array(api.memory.buffer, api.conduit_book_body_output_ptr(), api.conduit_book_body_output_len());
  return JSON.parse(decoder.decode(bytes));
}
