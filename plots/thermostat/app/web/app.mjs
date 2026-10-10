const $ = (id) => document.getElementById(id);
let face;
let busy = false;
const subject = "thermostat/device";
const groups = [
  {id: "mode-controls", prefix: "thermostat.mode.", property: "mode", options: [["off", "○"], ["heat", "☀"], ["cool", "❄"], ["auto", "↻"]]},
  {id: "fan-controls", prefix: "thermostat.fan.", property: "fan", options: [["auto", ""], ["on", ""]]},
  {id: "preset-controls", prefix: "thermostat.preset.", property: "preset", options: [["comfort", "☀"], ["eco", "♧"], ["sleep", "☾"]]},
];
function property(name) {
  const value = face?.properties?.find((p) => p.subject === subject && p.name === name)?.value;
  return value && typeof value === "object" ? Object.values(value)[0] : undefined;
}
function semanticText(identity, fallback) {
  return face?.text?.filter((t) => t.subject === identity).map((t) => t.text).join(" ") || fallback;
}
function action(identity) { return face?.actions?.find((a) => a.identity === identity); }
function availabilityReason(a) {
  const state = a?.availability;
  return state && typeof state === "object" ? Object.values(state)[0]?.explanation : "This control is not available.";
}
function bind(button, a) {
  button.disabled = busy || !a || a.availability !== "Available";
  button.title = a?.availability === "Available" ? a.name : availabilityReason(a);
  button.onclick = () => invoke(a.identity);
}
function render() {
  document.querySelector("main").setAttribute("aria-busy", String(busy));
  const target = property("target-decicelsius");
  $("target").textContent = Number.isFinite(target) ? (target / 10).toFixed(1) : "—";
  $("room-name").textContent = face?.subjects?.find((s) => s.identity === subject)?.name || "Thermostat";
  $("current").textContent = semanticText("thermostat/current", "Sensor unavailable");
  $("status").textContent = semanticText("thermostat/status", "Control demo");
  const mode = property("mode");
  document.querySelector(".temperature-card").dataset.mode = mode || "off";
  for (const id of ["lower", "raise"]) {
    const a = action(`thermostat.${id}`);
    bind($(id), a);
    if (a) $(id).setAttribute("aria-label", a.name);
  }
  for (const group of groups) {
    for (const [key] of group.options) {
      const button = $(`${group.property}-${key}`);
      const a = action(group.prefix + key);
      button.querySelector(".control-name").textContent = a?.name || key[0].toUpperCase() + key.slice(1);
      button.setAttribute("aria-pressed", String(property(group.property) === key));
      bind(button, a);
    }
  }
  $("revision").textContent = face ? `Revision ${face.revision}` : "Disconnected";
}
function notice(message, error = false) {
  $("notice").textContent = message;
  $("notice").classList.toggle("error", error);
}
async function readFace() {
  const response = await fetch("/face", {cache: "no-store", headers: {Accept: "application/json"}});
  if (!response.ok) throw new Error("Could not load thermostat controls.");
  return response.json();
}
async function connect() {
  busy = true;
  render();
  try {
    face = await readFace();
    $("retry").hidden = true;
    notice("Controls connected.");
  } catch (error) {
    face = undefined;
    $("retry").hidden = false;
    notice(`${error.message} Reconnect to try again.`, true);
  } finally { busy = false; render(); }
}
async function invoke(identity) {
  if (busy || !face || action(identity)?.availability !== "Available") return;
  const label = action(identity).name;
  busy = true;
  render();
  try {
    const response = await fetch("/action", {method: "POST", headers: {"Content-Type": "application/json", Accept: "application/json"}, body: JSON.stringify({revision: face.revision, action_id: identity})});
    const result = await response.json();
    if (!response.ok) {
      face = await readFace();
      notice(typeof result.error === "string" ? result.error : "The controls changed. Review the current setting and try again.", true);
    } else {
      face = result;
      notice(`${label}. Target ${(property("target-decicelsius") / 10).toFixed(1)} degrees Celsius. ${semanticText("thermostat/status", "Setting saved.")}`);
    }
  } catch {
    face = undefined;
    $("retry").hidden = false;
    notice("Connection lost. Reconnect to see the current settings before making another change.", true);
  } finally { busy = false; render(); }
}
for (const group of groups) {
  for (const [key, icon] of group.options) {
    const button = document.createElement("button");
    button.id = `${group.property}-${key}`;
    button.type = "button";
    button.disabled = true;
    const symbol = document.createElement("span");
    symbol.className = "control-icon";
    symbol.setAttribute("aria-hidden", "true");
    symbol.textContent = icon;
    const name = document.createElement("span");
    name.className = "control-name";
    name.textContent = key[0].toUpperCase() + key.slice(1);
    button.append(symbol, name);
    $(group.id).append(button);
  }
}
$("retry").onclick = connect;
connect();
