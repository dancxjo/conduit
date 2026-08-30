import { initializeBrowserHost } from "../browser-host-bootstrap.mjs";
import { createBodyBirthRunner, createFirstHostRunner, readBodyProjection } from "./creche-lifecycle.mjs";
import { createPhysicalHostRunner } from "./creche-physical.mjs";
import { createGraduationRunner, renderBiography } from "./creche-graduation.mjs";

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const chapter = document.querySelector("#chapter");
const hostState = document.querySelector("#host-state");
let host;
let peerHost = null;
let generation = 0;
let running = false;
let runnerCount = 0;
let activeRunner = null;
let activeDelay = null;
let cancelActiveKeyEvent = null;
let currentPage = 0;
let guidedPages = [];
const sourceDrafts = new Map();

try {
  const [chapters, initialized] = await Promise.all([
    Promise.all(["chapter-1.md", "chapter-2.md", "chapter-3.md", "chapter-4.md", "chapter-5.md", "chapter-6.md", "chapter-8.md"].map((name) =>
      fetch(`./${name}`).then((response) => {
        if (!response.ok) throw new Error(`${name} is unavailable`);
        return response.text();
      }),
    )),
    initializeBrowserHost(),
  ]);
  host = initialized;
  requireBookAbi(host.runtime);
  guidedPages = parseCrechePages(chapters);
  renderPage(0);
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
    "conduit_book_start_recursive", "conduit_book_complete", "conduit_book_complete_with_output", "conduit_book_cancel",
    "conduit_book_inventory", "conduit_book_admit_source_interaction",
    "conduit_book_multi_input_ptr", "conduit_book_multi_input_capacity",
    "conduit_book_multi_output_ptr", "conduit_book_multi_output_len",
    "conduit_book_multi_admit_source_interaction", "conduit_book_multi_start_source",
    "conduit_book_multi_start_sink",
    "conduit_book_multi_ingest", "conduit_book_multi_complete", "conduit_book_multi_cancel",
    "conduit_creche_input_ptr", "conduit_creche_input_capacity",
    "conduit_creche_output_ptr", "conduit_creche_output_len",
    "conduit_creche_admit_source_interaction", "conduit_creche_birth",
    "conduit_creche_current", "conduit_creche_biography", "conduit_creche_attach_here",
    "conduit_creche_graduation_readiness", "conduit_creche_graduate",
    "conduit_creche_prepare_selected_physical_spore",
    "conduit_creche_admit_physical_spore",
  ];
  if (required.some((name) => !(name in api))) throw new Error("executable-book ABI is incomplete");
}

function parseCrechePages(chapters) {
  const parsed = [];
  let current = [];
  for (const line of chapters.join("\n").replaceAll("\r\n", "\n").split("\n")) {
    if (line.startsWith("# ") && current.length > 0) {
      parsed.push(current.join("\n"));
      current = [];
    }
    if (line.startsWith("# ") || current.length > 0) current.push(line);
  }
  if (current.length > 0) parsed.push(current.join("\n"));
  if (parsed.length === 0) throw new Error("the Crèche has no guided pages");
  return parsed;
}

function renderPage(index) {
  if (running) return;
  currentPage = index;
  runnerCount = 0;
  chapter.replaceChildren();
  chapter.append(createCrecheBodyContext());
  renderMarkdown(guidedPages[index]);
  chapter.append(createNavigation());
  document.title = (chapter.querySelector("h1")?.textContent ?? "The Crèche") + " · The Crèche";
}

function createCrecheBodyContext() {
  const context = document.createElement("aside");
  context.className = "creche-body-context";
  context.setAttribute("aria-label", "Current Body in the Crèche");
  const body = readBodyProjection(host.runtime);
  if (!body) {
    context.innerHTML = "<strong>Crèche</strong><span>No Body has been born yet.</span>";
    return context;
  }
  const name = document.createElement("strong");
  name.textContent = body.friendly_name;
  const identity = document.createElement("code");
  identity.textContent = body.body_id;
  const state = document.createElement("span");
  state.textContent = `${body.state} · ${body.raw_membership.parts.length} admitted Part${body.raw_membership.parts.length === 1 ? "" : "s"}`;
  context.append(name, identity, state);
  return context;
}

function createNavigation() {
  const navigation = document.createElement("nav");
  navigation.className = "creche-navigation";
  navigation.setAttribute("aria-label", "Guided Crèche pages");
  const progress = document.createElement("span");
  progress.className = "creche-progress";
  progress.textContent = "Page " + (currentPage + 1) + " of " + guidedPages.length;
  const previous = navigationButton("Previous", currentPage === 0, () => renderPage(currentPage - 1));
  const reset = navigationButton("Reset this page", false, () => {
    for (const key of sourceDrafts.keys()) {
      if (key.startsWith(currentPage + ":")) sourceDrafts.delete(key);
    }
    renderPage(currentPage);
  });
  const revisitBirth = navigationButton("Revisit birth page", false, () => {
    sourceDrafts.clear();
    renderPage(0);
  });
  const next = navigationButton("Next", currentPage === guidedPages.length - 1, () => renderPage(currentPage + 1));
  navigation.append(progress, previous, reset, revisitBirth, next);
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
  for (const button of chapter.querySelectorAll(".creche-navigation button")) {
    button.disabled = disabled || (
      (button.textContent === "Previous" && currentPage === 0)
      || (button.textContent === "Next" && currentPage === guidedPages.length - 1)
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
    if (line === "```conduit birth") {
      flush();
      const source = [];
      index += 1;
      while (index < lines.length && lines[index] !== "```") source.push(lines[index++]);
      runnerCount += 1;
      const sourceKey = currentPage + ":" + runnerCount;
      chapter.append(createBodyBirthRunner({
        source: source.join("\n"),
        sourceKey,
        listingId: runnerCount === 1 ? "listing" : `listing-${runnerCount}`,
        host,
        draft: sourceDrafts.get(sourceKey),
        onDraft: (value) => sourceDrafts.set(sourceKey, value),
        nextSequence: () => ++generation,
        onBodyChanged: refreshCrecheBodyContext,
      }));
      copy = appendCopy();
    } else if (line === "```conduit run two-host" || line === "```conduit run two-host plan") {
      flush();
      const showPlan = line.endsWith(" plan");
      const source = [];
      index += 1;
      while (index < lines.length && lines[index] !== "```") source.push(lines[index++]);
      chapter.append(createMultiHostRunner(source.join("\n"), showPlan));
      copy = appendCopy();
    } else if (line === "```conduit run" || line === "```conduit run recursive" || line === "```conduit compare") {
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
    } else if (line === "<!-- conduit-host-inventory -->") {
      flush();
      renderInventory(readInventory(host.runtime));
      copy = appendCopy();
    } else if (line === "<!-- conduit-physical-host -->") {
      flush();
      chapter.append(createPhysicalHostRunner({ host }));
      copy = appendCopy();
    } else if (line === "<!-- conduit-first-host -->") {
      flush();
      chapter.append(createFirstHostRunner({
        host,
        nextSequence: () => {
          const admissionSequence = ++generation;
          generation += 1;
          return admissionSequence;
        },
        onBodyChanged: refreshCrecheBodyContext,
      }));
      copy = appendCopy();
    } else if (line === "<!-- conduit-graduation -->") {
      flush();
      chapter.append(createGraduationRunner({
        host,
        nextSequence: () => ++generation,
        onBodyChanged: refreshCrecheBodyContext,
        onEnd: renderCrecheComplete,
      }));
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

function refreshCrecheBodyContext() {
  chapter.querySelector(".creche-body-context")?.replaceWith(createCrecheBodyContext());
}

function renderCrecheComplete(receipt, biography) {
  chapter.replaceChildren();
  const complete = document.createElement("section");
  complete.className = "creche-complete";
  const heading = document.createElement("h1");
  heading.textContent = "The Body continues";
  const copy = document.createElement("p");
  copy.textContent = "The Crèche has ended. Its presentation is gone; the same Body and its graduation evidence remain in the runtime.";
  const identity = document.createElement("code");
  identity.textContent = receipt.body_id;
  const durable = document.createElement("section");
  durable.className = "body-biography compatible-reader";
  durable.setAttribute("aria-label", "Body biography");
  durable.innerHTML = "<h2>Body biography · compatible reader</h2><ol></ol>";
  renderBiography(durable, biography);
  complete.append(heading, copy, identity, durable);
  chapter.append(complete);
}

function createRealizationComparison(source) {
  const comparison = document.createElement("div");
  comparison.className = "realization-comparison";
  const face = document.createElement("div");
  face.className = "shared-face";
  const faceLabel = document.createElement("span");
  faceLabel.textContent = "Same requested Face";
  const faceContract = document.createElement("code");
  faceContract.textContent = "text/morse · text: value/text@1 → pattern: value/morse-pattern@1";
  face.append(faceLabel, faceContract);
  const direct = createRunner(source, false, {
    eyebrow: "Realization A",
    title: "Direct leaf",
    runLabel: "Run direct leaf",
  });
  const recursive = createRunner(source, true, {
    eyebrow: "Realization B",
    title: "Recursive Form Back",
    runLabel: "Run recursive Back",
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
  comparison.append(face, direct, recursive);
  return comparison;
}

function createRunner(source, recursive = false, presentation = {}) {
  runnerCount += 1;
  const sourceKey = currentPage + ":" + runnerCount;
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

function createMultiHostRunner(source, showPlan) {
  runnerCount += 1;
  const sourceKey = currentPage + ":" + runnerCount;
  const listingId = runnerCount === 1 ? "listing" : `listing-${runnerCount}`;
  const runner = document.createElement("section");
  runner.className = "runner multi-host-runner";
  runner.dataset.sourceKey = sourceKey;
  runner.dataset.mode = "multi";
  runner.innerHTML = `
    <div class="editor">
      <label class="editor-label" for="${listingId}">Conduit · editable · unchanged across Hosts</label>
      <textarea id="${listingId}" spellcheck="false" aria-label="Editable Conduit listing"></textarea>
      <div class="actions">
        <button class="run" type="button">Run across two Hosts</button><button class="stop" type="button" disabled>Stop</button>
      </div>
    </div>
    <div class="result multi-host-result">
      <div class="host-map" aria-label="Two independent browser Hosts">
        <article class="host-card host-a"><span>Host A · source</span><strong>waiting</strong><code class="host-id"></code><code class="boot-id"></code></article>
        <div class="planned-line" aria-label="One planned cross-Host Cord"><span>typed Cord</span><b>→</b><small>1 item · finite bytes</small></div>
        <article class="host-card host-b"><span>Host B · presentation</span><strong>waiting</strong><code class="host-id"></code><code class="boot-id"></code></article>
      </div>
      <h2>Planned result on Host B</h2>
      <output class="morse" aria-label="Planned result">ready</output>
      <p class="play-status" role="status">Run the Form to start two independent browser Hosts.</p>
      <details class="evidence"><summary>What happened?</summary><dl></dl><div class="expansion"></div></details>
      <details class="plan-view-details"><summary>Exact Plan for this Play</summary><div class="plan-view"></div><details class="raw-plan"><summary>Raw Plan evidence</summary><pre><code></code></pre></details></details>
    </div>`;
  runner.querySelector(".plan-view-details").open = showPlan;
  const textarea = runner.querySelector("textarea");
  textarea.value = sourceDrafts.get(sourceKey) ?? source;
  textarea.addEventListener("input", () => sourceDrafts.set(sourceKey, textarea.value));
  runner.querySelector(".run").addEventListener("click", () => runMultiHostListing(runner, textarea.value));
  runner.querySelector(".stop").addEventListener("click", () => stopListing(runner));
  return runner;
}

class BrowserMemoryLine {
  constructor(maximumFrameBytes, maximumPayloadBytes) {
    this.maximumFrameBytes = maximumFrameBytes;
    this.maximumPayloadBytes = maximumPayloadBytes;
    this.pending = null;
  }

  transfer(frame, targetApi) {
    if (this.pending !== null) throw new Error("browser-memory Line pressure: one item is already in flight");
    if (!Array.isArray(frame.payload) || frame.payload.length > this.maximumPayloadBytes) {
      throw new Error("browser-memory Line payload exceeds its exact Plan bound");
    }
    const encoded = encoder.encode(JSON.stringify(frame));
    if (encoded.length > this.maximumFrameBytes || encoded.length > targetApi.conduit_book_multi_input_capacity()) {
      throw new Error("browser-memory Line frame exceeds its exact admitted bound");
    }
    this.pending = encoded;
    const input = new Uint8Array(
      targetApi.memory.buffer,
      targetApi.conduit_book_multi_input_ptr(),
      encoded.length,
    );
    input.set(this.pending);
    this.pending = null;
    const code = targetApi.conduit_book_multi_ingest(encoded.length);
    if (code < 0) throw new Error(`browser-memory Line ingest refused (${code})`);
    return readMultiOutput(targetApi);
  }

  cancel() {
    this.pending = null;
  }
}

let activeMemoryLine = null;

async function ensurePeerHost() {
  if (peerHost !== null) return peerHost;
  const initialized = await initializeBrowserHost();
  requireBookAbi(initialized.runtime);
  if (initialized.hostId === host.hostId || initialized.bootId === host.bootId) {
    throw new Error("second browser Host did not receive independent Host and Boot identity");
  }
  peerHost = initialized;
  globalThis.__conduitBookPeerHost = peerHost;
  return peerHost;
}

async function runMultiHostListing(runner, source) {
  if (running && activeRunner) stopListing(activeRunner);
  const current = ++generation;
  const status = runner.querySelector(".play-status");
  running = true;
  activeRunner = runner;
  setNavigationDisabled(true);
  runner.querySelector(".run").disabled = true;
  runner.querySelector(".stop").disabled = false;
  status.classList.remove("error");
  status.textContent = "Starting an independent second browser Host…";
  try {
    const peer = await ensurePeerHost();
    if (current !== generation) return;
    renderHostCard(runner, "a", host, "planning source fragment");
    renderHostCard(runner, "b", peer, "waiting for planned Cord");
    const sourceBytes = encoder.encode(source);
    admitMultiSource(host.runtime, sourceBytes, current);
    admitMultiSource(peer.runtime, sourceBytes, current);
    const sourceProgress = startMultiSource(host.runtime, host, peer, sourceBytes, current);
    const sinkProgress = startMultiSink(
      peer.runtime,
      peer,
      sourceProgress.plan_projection.raw_plan,
      current,
    );
    if (sourceProgress.effect_kind !== "line" || sinkProgress.effect_kind !== "waiting") {
      throw new Error("two-Host runner did not start at the exact planned Line boundary");
    }
    const plan = sourceProgress.plan_projection;
    renderPlanProjection(runner, plan);
    const line = new BrowserMemoryLine(
      plan.raw_plan.fragments[0].connections[0].selected_line.binding.limits.maximum_frame_bytes,
      plan.cord.maximum_payload_bytes,
    );
    activeMemoryLine = line;
    renderHostCard(runner, "a", host, "offered one typed value");
    status.textContent = "Host A offered one value on the exact planned Cord…";
    if (!await nextPaint(current)) return;
    const presentation = line.transfer(sourceProgress.frame, peer.runtime);
    if (presentation.effect_kind !== "manifestation") {
      throw new Error("Host B did not request its planned presentation");
    }
    const accepted = line.transfer(presentation.accepted_frame, host.runtime);
    if (accepted.effect_kind !== "waiting") {
      throw new Error("Host A did not retain exact remote acceptance");
    }
    renderHostCard(runner, "a", host, "accepted · awaiting delivery");
    renderHostCard(runner, "b", peer, "presenting exact value");
    runner.querySelector(".morse").textContent = presentation.manifestation.text;
    renderIdentities(runner, presentation.manifestation);
    renderPlanProjection(runner, presentation.plan_projection);
    status.textContent = "Host B observed the planned presentation; acknowledging delivery…";
    if (!await nextPaint(current)) return;
    const completion = peer.runtime.conduit_book_multi_complete();
    if (completion < 0) throw new Error(`Host B presentation completion refused (${completion})`);
    const delivered = readMultiOutput(peer.runtime);
    const close = line.transfer(delivered.frame, host.runtime);
    const terminal = line.transfer(close.frame, peer.runtime);
    const sourceReceipt = line.transfer(terminal.frame, host.runtime);
    if (terminal.receipt?.disposition !== "completed" || sourceReceipt.receipt?.disposition !== "completed") {
      throw new Error("two-Host Play did not retain reciprocal terminal receipts");
    }
    renderHostCard(runner, "a", host, "completed");
    renderHostCard(runner, "b", peer, "completed");
    status.textContent = "Completed — one immutable Plan, two independent Plays, one delivered cross-Host value.";
    status.dataset.planId = plan.plan_id;
    status.dataset.sourceReceipt = sourceReceipt.receipt.terminal_sign_id;
    status.dataset.sinkReceipt = terminal.receipt.terminal_sign_id;
    finishRun(runner);
  } catch (error) {
    cancelMultiSessions();
    status.textContent = error instanceof Error ? error.message : String(error);
    status.classList.add("error");
    finishRun(runner);
  }
}

function admitMultiSource(api, sourceBytes, sequence) {
  if (sourceBytes.length > api.conduit_book_multi_input_capacity()) {
    throw new Error("The listing exceeds the admitted multi-Host input bound.");
  }
  new Uint8Array(api.memory.buffer, api.conduit_book_multi_input_ptr(), sourceBytes.length).set(sourceBytes);
  const code = api.conduit_book_multi_admit_source_interaction(sourceBytes.length, BigInt(sequence));
  if (code < 0) {
    const refusal = api.conduit_book_multi_output_len() > 0 ? readMultiOutput(api) : null;
    throw new Error(refusal?.message ?? `multi-Host source interaction refused (${code})`);
  }
}

function startMultiSource(api, sourceHost, sinkHost, sourceBytes, sequence) {
  const fields = [sourceHost.hostId, sourceHost.bootId, sinkHost.hostId, sinkHost.bootId]
    .map((value) => encoder.encode(value));
  const total = fields.reduce((sum, field) => sum + field.length, sourceBytes.length);
  if (total > api.conduit_book_multi_input_capacity()) {
    throw new Error("multi-Host start frame exceeds its admitted input bound");
  }
  const input = new Uint8Array(api.memory.buffer, api.conduit_book_multi_input_ptr(), total);
  let offset = 0;
  for (const field of fields) {
    input.set(field, offset);
    offset += field.length;
  }
  input.set(sourceBytes, offset);
  const code = api.conduit_book_multi_start_source(
    fields[0].length,
    fields[1].length,
    fields[2].length,
    fields[3].length,
    sourceBytes.length,
    BigInt(sequence),
  );
  if (code < 0) {
    const refusal = api.conduit_book_multi_output_len() > 0 ? readMultiOutput(api) : null;
    throw new Error(refusal?.message
      ? `The Form was refused before multi-Host Play · ${refusal.category}: ${refusal.message}`
      : `multi-Host Play start refused (${code})`);
  }
  return readMultiOutput(api);
}

function startMultiSink(api, sinkHost, plan, sequence) {
  const fields = [sinkHost.hostId, sinkHost.bootId, JSON.stringify(plan)]
    .map((value) => encoder.encode(value));
  const total = fields.reduce((sum, field) => sum + field.length, 0);
  if (total > api.conduit_book_multi_input_capacity()) {
    throw new Error("exact multi-Host Plan exceeds its admitted sink input bound");
  }
  const input = new Uint8Array(api.memory.buffer, api.conduit_book_multi_input_ptr(), total);
  let offset = 0;
  for (const field of fields) {
    input.set(field, offset);
    offset += field.length;
  }
  const code = api.conduit_book_multi_start_sink(
    fields[0].length,
    fields[1].length,
    fields[2].length,
    BigInt(sequence),
  );
  if (code < 0) {
    const refusal = api.conduit_book_multi_output_len() > 0 ? readMultiOutput(api) : null;
    throw new Error(refusal?.message
      ? `Host B refused the exact Plan before Play · ${refusal.message}`
      : `multi-Host sink Plan admission refused (${code})`);
  }
  return readMultiOutput(api);
}

function readMultiOutput(api) {
  const bytes = new Uint8Array(
    api.memory.buffer,
    api.conduit_book_multi_output_ptr(),
    api.conduit_book_multi_output_len(),
  );
  return JSON.parse(decoder.decode(bytes));
}

function renderHostCard(runner, suffix, identity, state) {
  const card = runner.querySelector(`.host-${suffix}`);
  card.querySelector("strong").textContent = state;
  card.querySelector(".host-id").textContent = identity.hostId;
  card.querySelector(".boot-id").textContent = identity.bootId;
  card.dataset.hostId = identity.hostId;
  card.dataset.bootId = identity.bootId;
}

function renderPlanProjection(runner, plan) {
  const view = runner.querySelector(".plan-view");
  view.replaceChildren();
  const explanation = document.createElement("p");
  explanation.textContent = plan.explanation;
  const planId = document.createElement("code");
  planId.className = "projected-plan-id";
  planId.textContent = plan.plan_id;
  const hosts = document.createElement("div");
  hosts.className = "projected-hosts";
  for (const projected of plan.hosts) {
    const card = document.createElement("article");
    const title = document.createElement("strong");
    title.textContent = `${projected.label} · one Play`;
    const identity = document.createElement("code");
    identity.textContent = projected.host_id;
    const gears = document.createElement("ul");
    for (const gear of projected.gears) {
      const item = document.createElement("li");
      item.textContent = `${gear.kind_id} → ${gear.implementation_id}`;
      gears.append(item);
    }
    card.append(title, identity, gears);
    hosts.append(card);
  }
  const cord = document.createElement("p");
  cord.className = "projected-cord";
  cord.textContent = `Cross-Host ${plan.cord.value_kind} Cord · ${plan.cord.line_id} · ${plan.cord.maximum_in_flight_items} item / ${plan.cord.maximum_payload_bytes} bytes`;
  view.append(explanation, planId, hosts, cord);
  runner.querySelector(".raw-plan code").textContent = JSON.stringify(plan.raw_plan, null, 2);
  runner.querySelector(".plan-view-details").dataset.planId = plan.plan_id;
}

function nextPaint(expectedGeneration) {
  return new Promise((resolve) => requestAnimationFrame(() => resolve(expectedGeneration === generation)));
}

function nextKeyEvent(expectedGeneration) {
  return new Promise((resolve) => {
    cancelActiveKeyEvent?.();
    const onKeyDown = (event) => {
      const usage = browserKeyboardUsage(event.code);
      if (usage === null) return;
      event.preventDefault();
      globalThis.removeEventListener("keydown", onKeyDown, true);
      cancelActiveKeyEvent = null;
      if (expectedGeneration !== generation) return resolve(null);
      const modifiers = (event.ctrlKey ? 1 : 0)
        | (event.shiftKey ? 2 : 0)
        | (event.altKey ? 4 : 0)
        | (event.metaKey ? 8 : 0);
      resolve(Uint8Array.of(usage, 0, modifiers));
    };
    cancelActiveKeyEvent = () => {
      globalThis.removeEventListener("keydown", onKeyDown, true);
      cancelActiveKeyEvent = null;
      resolve(null);
    };
    globalThis.addEventListener("keydown", onKeyDown, { capture: true, once: false });
  });
}

function browserKeyboardUsage(code) {
  if (/^Key[A-Z]$/.test(code)) return 0x04 + code.charCodeAt(3) - 65;
  if (/^Digit[1-9]$/.test(code)) return 0x1e + Number(code.slice(5)) - 1;
  if (code === "Digit0") return 0x27;
  return ({ Enter: 0x28, Escape: 0x29, Backspace: 0x2a, Tab: 0x2b, Space: 0x2c })[code] ?? null;
}

function finishRun(runner) {
  activeMemoryLine = null;
  running = false;
  activeRunner = null;
  setNavigationDisabled(false);
  runner.querySelector(".run").disabled = false;
  runner.querySelector(".stop").disabled = true;
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
  let progress = readOutput(api);
  running = true;
  activeRunner = runner;
  setNavigationDisabled(true);
  status.classList.remove("error");
  status.textContent = "Playing through this browser Host…";
  runner.querySelector(".run").disabled = true;
  runner.querySelector(".stop").disabled = false;
  try {
    while (progress.effect_kind) {
      if (progress.effect_kind === "timer") {
        status.textContent = `Waiting for planned tick · ${progress.duration_millis} ms`;
        if (!await delay(progress.duration_millis, current)) return;
      } else if (progress.effect_kind === "key-event") {
        status.textContent = "Waiting for one admitted keyboard transition…";
        const encoded = await nextKeyEvent(current);
        if (!encoded) return;
        new Uint8Array(api.memory.buffer, api.conduit_book_input_ptr(), encoded.length).set(encoded);
        const completion = api.conduit_book_complete_with_output(encoded.length);
        if (completion < 0) throw new Error(`keyboard completion refused (${completion})`);
        progress = readOutput(api);
        continue;
      } else if (progress.effect_kind === "manifestation") {
        runner.querySelector(".morse").textContent =
          progress.text ?? renderMorse(progress.segments);
        renderIdentities(runner, progress);
        for (const segment of progress.segments) {
          if (current !== generation) return;
          setIndicator(runner, segment.level);
          if (!await delay(segment.units * progress.unit_millis, current)) return;
        }
        setIndicator(runner, false);
        status.textContent = "Observed planned presentation; continuing the same Play…";
      } else {
        throw new Error(`unsupported browser Host effect ${progress.effect_kind}`);
      }
      if (current !== generation) return;
      const completion = api.conduit_book_complete();
      if (completion < 0) throw new Error(`completion refused (${completion})`);
      progress = readOutput(api);
    }
    status.textContent = progress.timer_completions > 0
      ? `Completed — one bounded Play, ${progress.timer_completions} planned ticks, ${progress.manifestation_completions} presentations.`
      : "Completed — one bounded Play, one planned manifestation.";
    status.dataset.receipt = progress.terminal_sign_id;
    status.dataset.timerCompletions = String(progress.timer_completions);
    status.dataset.manifestationCompletions = String(progress.manifestation_completions);
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
  cancelDelay();
  cancelActiveKeyEvent?.();
  if (running && runner.dataset.mode === "multi") cancelMultiSessions();
  else if (running) host.runtime.conduit_book_cancel();
  running = false;
  activeRunner = null;
  setNavigationDisabled(false);
  setIndicator(runner, false);
  runner.querySelector(".run").disabled = false;
  runner.querySelector(".stop").disabled = true;
  runner.querySelector(".play-status").textContent = "Stopped. The Play was cancelled.";
}

function cancelMultiSessions() {
  activeMemoryLine?.cancel();
  activeMemoryLine = null;
  host?.runtime.conduit_book_multi_cancel();
  peerHost?.runtime.conduit_book_multi_cancel();
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
  if (!indicator) return;
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
  const list = runner.querySelector("details.evidence dl, details:not(.plan-view-details) dl");
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
  const expansion = runner.querySelector("details.evidence .expansion, details:not(.plan-view-details) .expansion");
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

function delay(milliseconds, expectedGeneration) {
  return new Promise((resolve) => {
    const pending = {
      resolve,
      timeout: setTimeout(() => {
        if (activeDelay === pending) activeDelay = null;
        resolve(expectedGeneration === generation);
      }, milliseconds),
    };
    activeDelay = pending;
  });
}

function cancelDelay() {
  if (!activeDelay) return;
  clearTimeout(activeDelay.timeout);
  const { resolve } = activeDelay;
  activeDelay = null;
  resolve(false);
}
