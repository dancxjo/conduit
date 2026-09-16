const encoder = new TextEncoder();

function requireAbi(api) {
  const required = [
    "memory", "conduit_home_reset", "conduit_home_input_ptr", "conduit_home_submit",
    "conduit_home_apply_action", "conduit_home_view", "conduit_home_view_ptr", "conduit_home_view_len",
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
  [5, "Form run requested on this Browser Host."],
  [6, "Inspection requested on this Browser Host."],
  [7, "Wake requested for this Body."],
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
  document.documentElement.dataset.homeSteps = steps.join(",");
  const record = (...identities) => {
    for (const identity of identities) if (!steps.includes(identity)) steps.push(identity);
    document.documentElement.dataset.homeSteps = steps.join(",");
  };
  const report = (request) => {
    status.textContent = requestLabels.get(request) ?? "Browser Home is ready.";
    status.dataset.request = String(request);
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
        if (event.action === "home.open-patchbay") record("patchbay.opened");
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
    if (normalized.startsWith("run ") && request === 5) record("form.run", "play.observed");
    if (normalized === "home" && steps.includes("patchbay.opened")) record("home.returned");
    input.value = "";
    report(request);
    render();
  });
  render();
  status.textContent = "Browser Home is ready.";
  globalThis.__conduitHome = Object.freeze({ api, steps: () => [...steps] });
}
