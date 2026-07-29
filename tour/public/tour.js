import init, { parse_panel, run_panel } from "./conduit_web.js";

const source = document.querySelector("#source"), result = document.querySelector("#result");
const lessons = await (await fetch("../lessons/v1.json")).json();
await init();
let current = lessons.lessons[0];
const key = id => `conduit-tour-draft/${id}`;
function show(lesson) {
  current = lesson; document.querySelector("#title").textContent = lesson.title;
  document.querySelector("#goal").textContent = `Production parser check; ${lesson.profile}.`;
  source.value = localStorage.getItem(key(lesson.id)) ?? lesson.source; check();
}
function check() { const value = JSON.parse(parse_panel(source.value)); result.textContent = value.ok ? `Valid panel: ${value.nodes} nodes, ${value.cords} cords.` : value.diagnostic; }
for (const lesson of lessons.lessons) { const button = document.createElement("button"); button.textContent = lesson.title; button.onclick = () => show(lesson); const item = document.createElement("li"); item.append(button); document.querySelector("#lessons").append(item); }
source.addEventListener("input", () => { localStorage.setItem(key(current.id), source.value); check(); });
document.querySelector("#check").onclick = check;
function run() { const value = JSON.parse(run_panel(source.value)); result.textContent = value.ok ? value.stdout || "Run completed." : value.diagnostic; }
document.querySelector("#run").onclick = run;
document.querySelector("#reset").onclick = () => { source.value = current.source; localStorage.removeItem(key(current.id)); check(); };
document.querySelector("#download").onclick = () => { const link = document.createElement("a"); link.href = URL.createObjectURL(new Blob([source.value], {type:"text/plain"})); link.download = "lesson.panel"; link.click(); URL.revokeObjectURL(link.href); };
show(current);
if (new URLSearchParams(location.search).has("autorun")) run();
