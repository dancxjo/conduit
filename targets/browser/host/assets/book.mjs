import { initializeBrowserHost } from "../browser-host-bootstrap.mjs";

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const chapter = document.querySelector("#chapter");
const hostState = document.querySelector("#host-state");
let host;
let generation = 0;
let running = false;
let runnerCount = 0;

try {
  const [chapters, initialized] = await Promise.all([
    Promise.all(["chapter-1.md", "chapter-2.md"].map((name) =>
      fetch(`./${name}`).then((response) => {
        if (!response.ok) throw new Error(`${name} is unavailable`);
        return response.text();
      }),
    )),
    initializeBrowserHost(),
  ]);
  host = initialized;
  requireBookAbi(host.runtime);
  renderMarkdown(chapters[0]);
  renderInventory(readInventory(host.runtime));
  renderMarkdown(chapters[1]);
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
    "conduit_book_start_recursive", "conduit_book_complete", "conduit_book_cancel",
    "conduit_book_inventory",
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
    if (line === "```conduit run" || line === "```conduit run recursive") {
      flush();
      const recursive = line.endsWith(" recursive");
      const source = [];
      index += 1;
      while (index < lines.length && lines[index] !== "```") source.push(lines[index++]);
      chapter.append(createRunner(source.join("\n"), recursive));
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

function createRunner(source, recursive = false) {
  runnerCount += 1;
  const listingId = runnerCount === 1 ? "listing" : `listing-${runnerCount}`;
  const runner = document.createElement("section");
  runner.className = "runner";
  runner.innerHTML = `
    <div class="editor">
      <label class="editor-label" for="${listingId}">Conduit · editable</label>
      <textarea id="${listingId}" spellcheck="false" aria-label="Editable Conduit listing"></textarea>
      <div class="actions">
        <button class="run" type="button">Run</button><button class="stop" type="button" disabled>Stop</button>
        <label class="realization-control">Realization <select aria-label="Morse realization"><option value="direct">direct leaf</option><option value="recursive">recursive Form Back</option></select></label>
      </div>
    </div>
    <div class="result">
      <div class="indicator" role="img" aria-label="Indicator off"></div>
      <h2>Planned result</h2>
      <output class="morse" aria-label="Planned result">ready</output>
      <p class="play-status" role="status">Edit the message or timing, then run it.</p>
      <details><summary>Execution identities</summary><dl></dl><div class="expansion"></div></details>
    </div>`;
  const textarea = runner.querySelector("textarea");
  const run = runner.querySelector(".run");
  const stop = runner.querySelector(".stop");
  runner.querySelector("select").value = recursive ? "recursive" : "direct";
  textarea.value = source;
  run.addEventListener("click", () => runListing(
    runner,
    textarea.value,
    runner.querySelector("select").value === "recursive",
  ));
  stop.addEventListener("click", () => stopListing(runner));
  return runner;
}

async function runListing(runner, source, recursive) {
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
  const start = recursive ? api.conduit_book_start_recursive : api.conduit_book_start;
  const code = start(hostBytes.length, bootBytes.length, sourceBytes.length, BigInt(current));
  if (code < 0) {
    const refusal = api.conduit_book_output_len() > 0 ? readOutput(api) : null;
    status.textContent = refusal?.message
      ? `The Form was refused before Play · ${refusal.category}: ${refusal.message}`
      : `The Form was refused before Play (${code}).`;
    status.classList.add("error");
    return;
  }
  const effect = readOutput(api);
  running = true;
  status.classList.remove("error");
  status.textContent = "Playing through this browser Host…";
  runner.querySelector(".run").disabled = true;
  runner.querySelector(".stop").disabled = false;
  runner.querySelector(".morse").textContent = effect.text ?? renderMorse(effect.segments);
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
    status.textContent = "Completed — one bounded Play, one planned manifestation.";
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

function readInventory(api) {
  const code = api.conduit_book_inventory();
  if (code < 0) throw new Error(`browser Gear inventory refused (${code})`);
  return readOutput(api);
}

function renderInventory(inventory) {
  const entries = inventory.entries;
  const copy = appendCopy();
  const details = document.createElement("details");
  details.className = "gear-inventory";
  const summary = document.createElement("summary");
  const installed = entries.filter((entry) => entry.implementation_id !== null);
  summary.textContent = `Available gears · ${installed.length} exact browser implementations · ${inventory.limits.maximum_gears} Gear / ${inventory.limits.maximum_cords} Cord bound`;
  const list = document.createElement("ul");
  for (const entry of entries) {
    const item = document.createElement("li");
    item.className = entry.implementation_id ? "available" : "unavailable";
    const kind = document.createElement("code");
    kind.textContent = entry.kind_id;
    const status = document.createElement("span");
    status.textContent = `${entry.family} · ${entry.classification}`;
    item.title = entry.reason;
    item.append(kind, status);
    list.append(item);
  }
  details.append(summary, list);
  copy.append(details);
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
  const expansion = runner.querySelector(".expansion");
  expansion.replaceChildren();
  const mode = document.createElement("p");
  mode.textContent = `Selected realization: ${effect.realization}`;
  expansion.append(mode);
  if (effect.realization_backs.length > 0) {
    const heading = document.createElement("strong");
    heading.textContent = "Opened reusable Forms";
    const backs = document.createElement("ul");
    for (const back of effect.realization_backs) {
      const item = document.createElement("li");
      item.textContent = `${back.invocation_path} → ${back.kind_id}`;
      item.title = back.checked_form_id;
      backs.append(item);
    }
    expansion.append(heading, backs);
  }
  const heading = document.createElement("strong");
  heading.textContent = "Planned leaves";
  const gears = document.createElement("ul");
  for (const gear of effect.expanded_gears) {
    const item = document.createElement("li");
    item.textContent = `${gear.kind_id} · ${gear.implementation_id}`;
    item.title = gear.gear_id;
    gears.append(item);
  }
  expansion.append(heading, gears);
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
