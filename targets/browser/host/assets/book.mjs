import { initializeBrowserHost } from "../browser-host-bootstrap.mjs";

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const chapter = document.querySelector("#chapter");
const hostState = document.querySelector("#host-state");
let host;
let generation = 0;
let running = false;
let runnerCount = 0;
let activeRunner = null;
let currentStep = 0;
let steps = [];
const sourceDrafts = new Map();

try {
  const [chapters, initialized] = await Promise.all([
    Promise.all(["chapter-1.md", "chapter-2.md", "chapter-3.md"].map((name) =>
      fetch(`./${name}`).then((response) => {
        if (!response.ok) throw new Error(`${name} is unavailable`);
        return response.text();
      }),
    )),
    initializeBrowserHost(),
  ]);
  host = initialized;
  requireBookAbi(host.runtime);
  steps = parseTourSteps(chapters);
  renderStep(0);
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
    "conduit_book_inventory", "conduit_book_admit_source_interaction",
  ];
  if (required.some((name) => !(name in api))) throw new Error("executable-book ABI is incomplete");
}

function parseTourSteps(chapters) {
  const parsed = [];
  let current = [];
  for (const line of chapters.join("\n").replaceAll("\r\n", "\n").split("\n")) {
    if (line.startsWith("# Step ") && current.length > 0) {
      parsed.push(current.join("\n"));
      current = [];
    }
    if (line.startsWith("# Step ") || current.length > 0) current.push(line);
  }
  if (current.length > 0) parsed.push(current.join("\n"));
  if (parsed.length !== 7) {
    throw new Error("the first Tour slice must contain exactly seven steps, received " + parsed.length);
  }
  return parsed;
}

function renderStep(index) {
  if (running) return;
  currentStep = index;
  runnerCount = 0;
  chapter.replaceChildren();
  renderMarkdown(steps[index]);
  chapter.append(createNavigation());
  document.title = (chapter.querySelector("h1")?.textContent ?? "Conduit Tour") + " · The Conduit Tour";
}

function createNavigation() {
  const navigation = document.createElement("nav");
  navigation.className = "tour-navigation";
  navigation.setAttribute("aria-label", "Tour steps");
  const progress = document.createElement("span");
  progress.className = "tour-progress";
  progress.textContent = "Step " + (currentStep + 1) + " of " + steps.length;
  const previous = navigationButton("Previous", currentStep === 0, () => renderStep(currentStep - 1));
  const reset = navigationButton("Reset this step", false, () => {
    for (const key of sourceDrafts.keys()) {
      if (key.startsWith(currentStep + ":")) sourceDrafts.delete(key);
    }
    renderStep(currentStep);
  });
  const restart = navigationButton("Restart Tour", false, () => {
    sourceDrafts.clear();
    renderStep(0);
  });
  const next = navigationButton("Next", currentStep === steps.length - 1, () => renderStep(currentStep + 1));
  navigation.append(progress, previous, reset, restart, next);
  return navigation;
}

function navigationButton(label, disabled, action) {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = label;
  button.disabled = disabled;
  button.addEventListener("click", action);
  return button;
}

function setNavigationDisabled(disabled) {
  for (const button of chapter.querySelectorAll(".tour-navigation button")) {
    button.disabled = disabled || (
      (button.textContent === "Previous" && currentStep === 0)
      || (button.textContent === "Next" && currentStep === steps.length - 1)
    );
  }
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
    if (line === "```conduit run" || line === "```conduit run recursive" || line === "```conduit compare") {
      flush();
      const recursive = line.endsWith(" recursive");
      const comparison = line.endsWith(" compare");
      const source = [];
      index += 1;
      while (index < lines.length && lines[index] !== "```") source.push(lines[index++]);
      chapter.append(comparison
        ? createRealizationComparison(source.join("\n"))
        : createRunner(source.join("\n"), recursive));
      copy = appendCopy();
    } else if (line === "```text") {
      flush();
      const source = [];
      index += 1;
      while (index < lines.length && lines[index] !== "```") source.push(lines[index++]);
      const diagram = document.createElement("pre");
      diagram.className = "concept-diagram";
      const code = document.createElement("code");
      code.textContent = source.join("\n");
      diagram.append(code);
      copy.append(diagram);
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

function createRealizationComparison(source) {
  const comparison = document.createElement("div");
  comparison.className = "realization-comparison";
  const direct = createRunner(source, false, {
    eyebrow: "Realization A",
    title: "Host leaf",
    runLabel: "Run Host leaf",
  });
  const recursive = createRunner(source, true, {
    eyebrow: "Realization B",
    title: "Open reviewed Back",
    runLabel: "Run open Back",
  });
  const directSource = direct.querySelector("textarea");
  const recursiveSource = recursive.querySelector("textarea");
  recursiveSource.value = directSource.value;
  sourceDrafts.set(recursive.dataset.sourceKey, directSource.value);
  directSource.addEventListener("input", () => {
    recursiveSource.value = directSource.value;
    sourceDrafts.set(recursive.dataset.sourceKey, directSource.value);
  });
  recursiveSource.addEventListener("input", () => {
    directSource.value = recursiveSource.value;
    sourceDrafts.set(direct.dataset.sourceKey, recursiveSource.value);
  });
  comparison.append(direct, recursive);
  return comparison;
}

function createRunner(source, recursive = false, presentation = {}) {
  runnerCount += 1;
  const sourceKey = currentStep + ":" + runnerCount;
  const listingId = runnerCount === 1 ? "listing" : `listing-${runnerCount}`;
  const runner = document.createElement("section");
  runner.className = "runner";
  runner.dataset.sourceKey = sourceKey;
  runner.innerHTML = `
    ${presentation.title ? `<header class="realization-heading"><span>${presentation.eyebrow}</span><h3>${presentation.title}</h3></header>` : ""}
    <div class="editor">
      <label class="editor-label" for="${listingId}">Conduit · editable</label>
      <textarea id="${listingId}" spellcheck="false" aria-label="Editable Conduit listing"></textarea>
      <div class="actions">
        <button class="run" type="button">${presentation.runLabel ?? "Run"}</button><button class="stop" type="button" disabled>Stop</button>
      </div>
    </div>
    <div class="result">
      <div class="indicator" role="img" aria-label="Indicator off"></div>
      <h2>Planned result</h2>
      <output class="morse" aria-label="Planned result">ready</output>
      <p class="play-status" role="status">Edit the message or timing, then run it.</p>
      <details><summary>What happened?</summary><dl></dl><div class="expansion"></div></details>
    </div>`;
  const textarea = runner.querySelector("textarea");
  const run = runner.querySelector(".run");
  const stop = runner.querySelector(".stop");
  textarea.value = sourceDrafts.get(sourceKey) ?? source;
  textarea.addEventListener("input", () => sourceDrafts.set(sourceKey, textarea.value));
  run.addEventListener("click", () => runListing(runner, textarea.value, recursive));
  stop.addEventListener("click", () => stopListing(runner));
  return runner;
}

async function runListing(runner, source, recursive) {
  if (running && activeRunner) stopListing(activeRunner);
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
  const interactionInput = new Uint8Array(
    api.memory.buffer,
    api.conduit_book_input_ptr(),
    sourceBytes.length,
  );
  interactionInput.set(sourceBytes);
  const interaction = api.conduit_book_admit_source_interaction(
    sourceBytes.length,
    BigInt(current),
  );
  if (interaction < 0) {
    const refusal = api.conduit_book_output_len() > 0 ? readOutput(api) : null;
    status.textContent = refusal?.message
      ? `The edit was refused · ${refusal.category}: ${refusal.message}`
      : `The edit was refused (${interaction}).`;
    status.classList.add("error");
    return;
  }
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
  activeRunner = runner;
  setNavigationDisabled(true);
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
    activeRunner = null;
    setNavigationDisabled(false);
    runner.querySelector(".run").disabled = false;
    runner.querySelector(".stop").disabled = true;
  } catch (error) {
    status.textContent = error instanceof Error ? error.message : String(error);
    status.classList.add("error");
    running = false;
    activeRunner = null;
    setNavigationDisabled(false);
    runner.querySelector(".run").disabled = false;
    runner.querySelector(".stop").disabled = true;
  }
}

function stopListing(runner) {
  generation += 1;
  if (running) host.runtime.conduit_book_cancel();
  running = false;
  activeRunner = null;
  setNavigationDisabled(false);
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
  if (effect.source_interaction) {
    for (const [label, value] of [
      ["Source interaction proposal", effect.source_interaction.proposal_identity],
      ["Source interaction result", effect.source_interaction.result_identity],
    ]) {
      const term = document.createElement("dt");
      const identity = document.createElement("dd");
      term.textContent = label;
      identity.textContent = value;
      list.append(term, identity);
    }
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
