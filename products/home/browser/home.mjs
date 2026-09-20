const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

function requireAbi(api) {
  const required = [
    "memory", "conduit_home_reset", "conduit_home_input_ptr", "conduit_home_submit",
    "conduit_home_apply_action", "conduit_home_view", "conduit_home_view_ptr", "conduit_home_view_len",
    "conduit_browser_form_input_ptr", "conduit_browser_form_output_ptr", "conduit_browser_form_output_len",
    "conduit_browser_form_admit_source_interaction", "conduit_browser_form_start", "conduit_browser_form_complete",
  ];
  if (required.some((name) => !(name in api))) throw new Error("portable Home ABI is incomplete");
}

function writeInput(api, value) {
  const encoded = encoder.encode(value);
  if (encoded.length < 1 || encoded.length > 96) throw new Error("Home input exceeds its admitted bound");
  new Uint8Array(api.memory.buffer, api.conduit_home_input_ptr(), encoded.length).set(encoded);
  return encoded.length;
}

const requestLabels = new Map([
  [1, "Tour request accepted by this Browser Host."],
  [2, "Patchbay request accepted by this Browser Host."],
  [3, "Crèche request accepted by this Browser Host."],
  [4, "Form selected on this Browser Host."],
  [5, "Form completed on this Browser Host."],
  [6, "Inspection requested on this Browser Host."],
  [7, "Wake requested for this body."],
]);

export async function startApplication(context) {
  const instantiated = await WebAssembly.instantiate(context.bytes("runtime"), {});
  const api = instantiated.instance.exports;
  requireAbi(api);
  if (api.conduit_home_reset() < 0) throw new Error("portable Home refused reset");
  const command = document.querySelector("#home-command");
  const input = document.querySelector("#command");
  const status = document.querySelector("#host-state");
  if (!command || !input || !status) throw new Error("Browser Home controls are incomplete");

  const steps = ["home.arrived"];
  const identity = Object.freeze({
    host_id: "browser/home",
    boot_id: `browser-boot/home/${crypto.randomUUID()}`,
  });
  let formExecution = null;
  document.documentElement.dataset.homeSteps = steps.join(",");
  const record = (...identities) => {
    for (const identity of identities) if (!steps.includes(identity)) steps.push(identity);
    document.documentElement.dataset.homeSteps = steps.join(",");
  };
  const report = (request) => {
    status.textContent = requestLabels.get(request) ?? "Browser Home is ready.";
    status.dataset.request = String(request);
  };
  const readFormOutput = () => JSON.parse(decoder.decode(new Uint8Array(
    api.memory.buffer, api.conduit_browser_form_output_ptr(), api.conduit_browser_form_output_len(),
  )));
  const runHello = () => {
    const source = encoder.encode(context.text("hello-form"));
    const host = encoder.encode(identity.host_id);
    const boot = encoder.encode(identity.boot_id);
    const pointer = api.conduit_browser_form_input_ptr();
    new Uint8Array(api.memory.buffer, pointer, source.length).set(source);
    if (api.conduit_browser_form_admit_source_interaction(source.length, 1n) < 0) throw new Error("Hello source admission refused");
    const inputBytes = new Uint8Array(api.memory.buffer, pointer, host.length + boot.length + source.length);
    inputBytes.set(host); inputBytes.set(boot, host.length); inputBytes.set(source, host.length + boot.length);
    if (api.conduit_browser_form_start(host.length, boot.length, source.length, 1n) < 0) throw new Error("Hello Plan/Play refused");
    const effect = readFormOutput();
    if (effect.effect_kind !== "manifestation" || effect.text !== "HELLO, WORLD.") throw new Error("Hello manifestation changed");
    if (api.conduit_browser_form_complete() < 0) throw new Error("Hello completion refused");
    const receipt = readFormOutput();
    if (receipt.disposition !== "completed") throw new Error("Hello Play did not complete");
    if (receipt.active_play_id !== effect.active_play_id) throw new Error("Hello Play identity changed before completion");
    formExecution = Object.freeze({ effect: Object.freeze(effect), receipt: Object.freeze(receipt) });
    status.textContent = effect.text;
    status.dataset.playDisposition = receipt.disposition;
  };
  const render = () => {
    if (api.conduit_home_view() < 0) throw new Error("portable Home presentation was refused");
    const encoded = new Uint8Array(api.memory.buffer, api.conduit_home_view_ptr(), api.conduit_home_view_len()).slice();
    context.presentation.present("home", encoded, {
      onEvent(event) {
        context.presentation.nextEvent("home");
        const request = api.conduit_home_apply_action(writeInput(api, event.action));
        if (request < 0) throw new Error("portable Home refused an application action");
        if (event.action === "home.open-forms") record("forms.opened");
        if (event.action.startsWith("home.open-form.")) record("form.selected");
        if (request === 2) {
          location.assign(new URL("../patchbay/", location.href));
          return;
        }
        if (request === 1) { location.assign(new URL("../tour/", location.href)); return; }
        if (request === 3) { location.assign(new URL("../creche/", location.href)); return; }
        report(request);
        render();
      },
    });
  };
  command.addEventListener("submit", (event) => {
    event.preventDefault();
    const text = input.value.trim();
    if (!text) return;
    const request = api.conduit_home_submit(writeInput(api, text));
    if (request < 0) throw new Error("portable Home refused the command");
    const normalized = text.toLowerCase();
    if (normalized === "open prompt") record("prompt.opened");
    if (normalized === "run hello" && request === 5) {
      runHello();
      record("form.run", "play.observed");
    } else {
      report(request);
    }
    input.value = "";
    render();
  });
  render();
  status.textContent = "Browser Home is ready.";
  globalThis.__conduitHome = Object.freeze({
    api,
    identity,
    steps: () => [...steps],
    evidence: () => formExecution,
  });
}
