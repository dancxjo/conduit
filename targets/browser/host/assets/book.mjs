import { initializeBrowserHost } from "../browser-host-bootstrap.mjs";

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const chapter = document.querySelector("#chapter");
const hostState = document.querySelector("#host-state");
let host;
let generation = 0;
let running = false;

try {
  const [markdown, initialized] = await Promise.all([
    fetch("./chapter-1.md").then((response) => {
      if (!response.ok) throw new Error("chapter source is unavailable");
      return response.text();
    }),
    initializeBrowserHost(),
  ]);
  host = initialized;
  requireBookAbi(host.runtime);
  renderMarkdown(markdown);
  hostState.textContent = "Browser Host ready";
  globalThis.__conduitBookHost = host;
} catch (error) {
  hostState.textContent = "Browser Host unavailable";
  chapter.textContent = error instanceof Error ? error.message : String(error);
  chapter.classList.add("error");
}

function requireBookAbi(api) {
  const required = [
    "memory", "conduit_book_input_ptr", "conduit_book_input_capacity",
    "conduit_book_output_ptr", "conduit_book_output_len", "conduit_book_start",
    "conduit_book_complete", "conduit_book_cancel",
  ];
  if (required.some((name) => !(name in api))) throw new Error("executable-book ABI is incomplete");
}

function renderMarkdown(markdown) {
  const lines = markdown.replaceAll("\r\n", "\n").split("\n");
  let copy = appendCopy();
  let paragraph = [];
  const flush = () => {
    if (paragraph.length === 0) return;
    const element = document.createElement("p");
    element.textContent = paragraph.join(" ");
    copy.append(element);
    paragraph = [];
  };
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (line === "```conduit run") {
      flush();
      const source = [];
      index += 1;
      while (index < lines.length && lines[index] !== "```") source.push(lines[index++]);
      chapter.append(createRunner(source.join("\n")));
      copy = appendCopy();
    } else if (line.startsWith("# ")) {
      flush();
      const heading = document.createElement("h1");
      heading.textContent = line.slice(2);
      copy.append(heading);
    } else if (line.startsWith("## ")) {
      flush();
      const heading = document.createElement("h2");
      heading.textContent = line.slice(3);
      copy.append(heading);
    } else if (line.trim() === "") {
      flush();
    } else {
      paragraph.push(line.trim());
    }
  }
  flush();
}

function appendCopy() {
  const copy = document.createElement("div");
  copy.className = "chapter-copy";
  chapter.append(copy);
  return copy;
}

function createRunner(source) {
  const runner = document.createElement("section");
  runner.className = "runner";
  runner.innerHTML = `
    <div class="editor">
      <label class="editor-label" for="listing">Conduit · editable</label>
      <textarea id="listing" spellcheck="false" aria-label="Editable Conduit listing"></textarea>
      <div class="actions"><button class="run" type="button">Run</button><button class="stop" type="button" disabled>Stop</button></div>
    </div>
    <div class="result">
      <div class="indicator" role="img" aria-label="Indicator off"></div>
      <h2>Browser indicator</h2>
      <output class="morse" aria-label="Morse pattern">ready</output>
      <p class="play-status" role="status">Edit the message or timing, then run it.</p>
      <details><summary>Execution identities</summary><dl></dl></details>
    </div>`;
  const textarea = runner.querySelector("textarea");
  const run = runner.querySelector(".run");
  const stop = runner.querySelector(".stop");
  textarea.value = source;
  run.addEventListener("click", () => runListing(runner, textarea.value));
  stop.addEventListener("click", () => stopListing(runner));
  return runner;
}

async function runListing(runner, source) {
  if (running) stopListing(runner);
  const current = ++generation;
  const api = host.runtime;
  const sourceBytes = encoder.encode(source);
  const hostBytes = encoder.encode(host.hostId);
  const bootBytes = encoder.encode(host.bootId);
  const total = hostBytes.length + bootBytes.length + sourceBytes.length;
  const status = runner.querySelector(".play-status");
  if (total > api.conduit_book_input_capacity()) {
    status.textContent = "The listing exceeds the admitted input bound.";
    status.classList.add("error");
    return;
  }
  const input = new Uint8Array(api.memory.buffer, api.conduit_book_input_ptr(), total);
  input.set(hostBytes);
  input.set(bootBytes, hostBytes.length);
  input.set(sourceBytes, hostBytes.length + bootBytes.length);
  const code = api.conduit_book_start(hostBytes.length, bootBytes.length, sourceBytes.length, BigInt(current));
  if (code < 0) {
    status.textContent = `The Form was refused before Play (${code}).`;
    status.classList.add("error");
    return;
  }
  const effect = readOutput(api);
  running = true;
  status.classList.remove("error");
  status.textContent = "Playing through this browser Host…";
  runner.querySelector(".run").disabled = true;
  runner.querySelector(".stop").disabled = false;
  runner.querySelector(".morse").textContent = renderMorse(effect.segments);
  renderIdentities(runner, effect);
  try {
    for (const segment of effect.segments) {
      if (current !== generation) return;
      setIndicator(runner, segment.level);
      await delay(segment.units * effect.unit_millis);
    }
    if (current !== generation) return;
    setIndicator(runner, false);
    const completion = api.conduit_book_complete();
    if (completion < 0) throw new Error(`completion refused (${completion})`);
    const receipt = readOutput(api);
    status.textContent = "Completed — one bounded Play, one manifested pattern.";
    status.dataset.receipt = receipt.terminal_sign_id;
    running = false;
    runner.querySelector(".run").disabled = false;
    runner.querySelector(".stop").disabled = true;
  } catch (error) {
    status.textContent = error instanceof Error ? error.message : String(error);
    status.classList.add("error");
    running = false;
    runner.querySelector(".run").disabled = false;
    runner.querySelector(".stop").disabled = true;
  }
}

function stopListing(runner) {
  generation += 1;
  if (running) host.runtime.conduit_book_cancel();
  running = false;
  setIndicator(runner, false);
  runner.querySelector(".run").disabled = false;
  runner.querySelector(".stop").disabled = true;
  runner.querySelector(".play-status").textContent = "Stopped. The Play was cancelled.";
}

function readOutput(api) {
  const bytes = new Uint8Array(api.memory.buffer, api.conduit_book_output_ptr(), api.conduit_book_output_len());
  return JSON.parse(decoder.decode(bytes));
}

function setIndicator(runner, level) {
  const indicator = runner.querySelector(".indicator");
  indicator.classList.toggle("on", level);
  indicator.setAttribute("aria-label", level ? "Indicator on" : "Indicator off");
}

function renderMorse(segments) {
  return segments.map((segment) => {
    if (!segment.level) return segment.units === 7 ? "   " : segment.units === 3 ? " " : "";
    return segment.units === 1 ? "·" : "—";
  }).join("");
}

function renderIdentities(runner, effect) {
  const labels = {
    source_document_id: "Source document", checked_form_id: "Checked Form",
    expanded_form_id: "Expanded Form", plan_id: "Plan", fragment_id: "Plan fragment",
    active_play_id: "Active Play", presentation_id: "Presentation",
    placement_id: "Placement", host_id: "Host", boot_id: "Boot",
  };
  const list = runner.querySelector("details dl");
  list.replaceChildren();
  for (const [key, label] of Object.entries(labels)) {
    const term = document.createElement("dt");
    const value = document.createElement("dd");
    term.textContent = label;
    value.textContent = effect[key];
    list.append(term, value);
  }
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
