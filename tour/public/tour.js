import init, { parse_panel, patchbay_move_node, patchbay_replace_source, run_panel } from "./conduit_web.js";

const source = document.querySelector("#source"), result = document.querySelector("#result");
const lessons = await (await fetch("../lessons/v1.json")).json();
await init();
let current = lessons.lessons[0];
let acceptedSource = "", selectedNode = null, positions = {};
const key = id => `conduit-tour-draft/${id}`;
function show(lesson) {
  current = lesson; document.querySelector("#title").textContent = lesson.title;
  document.querySelector("#goal").textContent = `Production parser check; ${lesson.profile}.`;
  source.value = localStorage.getItem(key(lesson.id)) ?? lesson.source;
  acceptedSource = source.value; selectedNode = null; positions = {}; check();
}
function selectNode(node) {
  selectedNode = node;
  const start = source.value.indexOf(`node ${node} `);
  if (start >= 0) { source.focus(); source.setSelectionRange(start, start + node.length + 5); }
  renderPanel(JSON.parse(parse_panel(source.value)));
}
function renderPanel(value) {
  const panel = document.querySelector("#panel");
  panel.replaceChildren(...(value.node_labels ?? []).map(label => {
    const node = label.split(" : ")[0], button = document.createElement("button"), item = document.createElement("li");
    button.textContent = label; button.setAttribute("aria-pressed", String(node === selectedNode));
    button.style.transform = `translateX(${positions[node]?.x ?? 0}px)`;
    button.onclick = () => selectNode(node); item.append(button); return item;
  }));
  document.querySelector("#move-left").disabled = !selectedNode;
  document.querySelector("#move-right").disabled = !selectedNode;
}
function check() { const value = JSON.parse(parse_panel(source.value)); result.textContent = value.ok ? `Valid panel: ${value.nodes} nodes, ${value.cords} cords.` : value.diagnostic; renderPanel(value); }
for (const lesson of lessons.lessons) { const button = document.createElement("button"); button.textContent = lesson.title; button.onclick = () => show(lesson); const item = document.createElement("li"); item.append(button); document.querySelector("#lessons").append(item); }
source.addEventListener("input", () => {
  const transaction = JSON.parse(patchbay_replace_source(acceptedSource, source.value));
  if (transaction.ok) { acceptedSource = source.value; localStorage.setItem(key(current.id), source.value); }
  check();
});
source.addEventListener("select", () => {
  const before = source.value.slice(0, source.selectionStart);
  const match = before.match(/node\s+([A-Za-z][A-Za-z0-9_-]*)\s*$/);
  if (match) { selectedNode = match[1]; renderPanel(JSON.parse(parse_panel(source.value))); }
});
document.querySelector("#check").onclick = check;
function moveSelected(delta) {
  if (!selectedNode) return;
  const position = positions[selectedNode] ?? {x: 0, y: 0};
  const transaction = JSON.parse(patchbay_move_node(source.value, selectedNode, position.x + delta, position.y));
  if (!transaction.ok) { result.textContent = transaction.diagnostic; return; }
  positions = transaction.positions; result.textContent = `Presentation moved; semantic hash remains ${transaction.semantic_hash}.`; check();
}
document.querySelector("#move-left").onclick = () => moveSelected(-16);
document.querySelector("#move-right").onclick = () => moveSelected(16);
function run() { const value = JSON.parse(run_panel(source.value)); result.textContent = value.ok ? `${value.stdout || "Run completed."}\nEvidence: ${value.completed_nodes} nodes, ${value.cords_conducted} cords conducted.` : value.diagnostic; }
document.querySelector("#run").onclick = run;
document.querySelector("#stop").onclick = () => { result.textContent = "No live run: this lesson uses the finite production runtime."; };
source.addEventListener("keydown", event => { if (event.shiftKey && event.key === "Enter") { event.preventDefault(); run(); } });
document.querySelector("#reset").onclick = () => { source.value = current.source; acceptedSource = source.value; selectedNode = null; positions = {}; localStorage.removeItem(key(current.id)); check(); };
document.querySelector("#download").onclick = () => { const link = document.createElement("a"); link.href = URL.createObjectURL(new Blob([source.value], {type:"text/plain"})); link.download = "lesson.panel"; link.click(); URL.revokeObjectURL(link.href); };
show(current);
if (new URLSearchParams(location.search).has("autorun")) run();
