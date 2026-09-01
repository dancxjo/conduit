import { initializeBrowserHost } from "./browser-host-bootstrap.mjs";
import { renderFlow, renderFlowRefusal } from "./assets/flow.js";

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
let pageRoutes = [];
let patchbaySequence = 0;
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
  guidedPages = parseBookPages(chapters);
  pageRoutes = guidedPages.map(pageRoute);
  const initialPage = pageIndexForLocation();
  renderPage(initialPage);
  if (isProductRoot()) replacePageRoute(initialPage);
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
    "conduit_book_project_patchbay", "conduit_book_project_patchbay_recursive",
    "conduit_book_multi_input_ptr", "conduit_book_multi_input_capacity",
    "conduit_book_multi_output_ptr", "conduit_book_multi_output_len",
    "conduit_book_multi_admit_source_interaction", "conduit_book_multi_start_source",
    "conduit_book_multi_start_sink",
    "conduit_book_multi_ingest", "conduit_book_multi_complete", "conduit_book_multi_cancel",
  ];
  if (required.some((name) => !(name in api))) throw new Error("executable-book ABI is incomplete");
}

function parseBookPages(chapters) {
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
  if (parsed.length === 0) throw new Error("the Book has no pages");
  return parsed;
}

function renderPage(index, routeChange = "none") {
  if (running) return;
  if (routeChange === "push") history.pushState(null, "", pageRoutes[index]);
  currentPage = index;
  runnerCount = 0;
  chapter.replaceChildren();
  renderMarkdown(guidedPages[index]);
  chapter.append(createNavigation());
  document.title = (chapter.querySelector("h1")?.textContent ?? "The Book") + " · The Book";
}

function createNavigation() {
  const navigation = document.createElement("nav");
  navigation.className = "book-navigation";
  navigation.setAttribute("aria-label", "Book pages");
  const progress = document.createElement("span");
  progress.className = "book-progress";
  progress.textContent = "Page " + (currentPage + 1) + " of " + guidedPages.length;
  const previous = navigationButton("Previous", currentPage === 0, () => renderPage(currentPage - 1, "push"));
  const next = navigationButton("Next", currentPage === guidedPages.length - 1, () => renderPage(currentPage + 1, "push"));
  navigation.append(progress, previous, next);
  return navigation;
}

function pageRoute(markdown) {
  const title = markdown.match(/^# (.+)$/m)?.[1];
  if (!title) throw new Error("a Book page has no title");
  const slug = title.toLowerCase().normalize("NFKD").replace(/\p{M}/gu, "").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
  if (!slug) throw new Error("a Book page has no route identity");
  return new URL(`${slug}/`, document.baseURI).pathname;
}

function pageIndexForLocation() {
  if (isProductRoot()) return 0;
  const index = pageRoutes.indexOf(location.pathname);
  if (index === -1) throw new Error("this Book page does not exist");
  return index;
}

function isProductRoot() {
  return location.pathname === new URL(".", document.baseURI).pathname
    || location.pathname === new URL("index.html", document.baseURI).pathname;
}

function replacePageRoute(index) {
  history.replaceState(null, "", pageRoutes[index]);
}

addEventListener("popstate", () => {
  const index = pageIndexForLocation();
  if (running) {
    replacePageRoute(currentPage);
    return;
  }
  renderPage(index);
});

function navigationButton(label, disabled, action) {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = label;
  button.disabled = disabled;
  button.addEventListener("click", action);
  return button;
}

function setNavigationDisabled(disabled) {
  for (const button of chapter.querySelectorAll(".book-navigation button")) {
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
    appendInlineMarkdown(element, paragraph.join(" "));
    copy.append(element);
    paragraph = [];
  };
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (line === "```conduit birth") {
      flush();
      index += 1;
      while (index < lines.length && lines[index] !== "```") index += 1;
      chapter.append(createCrecheCallToAction());
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
      chapter.append(createCrecheCallToAction("Add a physical Host in the Crèche"));
      copy = appendCopy();
    } else if (line === "<!-- conduit-first-host -->") {
      flush();
      chapter.append(createCrecheCallToAction("Admit a first Host in the Crèche"));
      copy = appendCopy();
    } else if (line === "<!-- conduit-graduation -->") {
      flush();
      chapter.append(createCrecheCallToAction("Open graduation in the Crèche"));
      copy = appendCopy();
    } else if (line.startsWith("# ")) {
      flush();
      const heading = document.createElement("h1");
      appendInlineMarkdown(heading, line.slice(2));
      copy.append(heading);
    } else if (line.startsWith("## ")) {
      flush();
      const heading = document.createElement("h2");
      appendInlineMarkdown(heading, line.slice(3));
      copy.append(heading);
    } else if (line.trim() === "") {
      flush();
    } else {
      paragraph.push(line.trim());
    }
  }
  flush();
}

function appendInlineMarkdown(parent, source) {
  const delimiters = [
    { opening: "**", closing: "**", tag: "strong" },
    { opening: "__", closing: "__", tag: "strong" },
    { opening: "`", closing: "`", tag: "code" },
    { opening: "*", closing: "*", tag: "em" },
    { opening: "_", closing: "_", tag: "em" },
  ];
  let offset = 0;
  while (offset < source.length) {
    let match = null;
    for (const delimiter of delimiters) {
      const openingIndex = source.indexOf(delimiter.opening, offset);
      if (openingIndex === -1) continue;
      const closingIndex = source.indexOf(
        delimiter.closing,
        openingIndex + delimiter.opening.length,
      );
      if (closingIndex === -1 || closingIndex === openingIndex + delimiter.opening.length) continue;
      if (!match || openingIndex < match.openingIndex) {
        match = { ...delimiter, openingIndex, closingIndex };
      }
    }
    if (!match) {
      parent.append(document.createTextNode(source.slice(offset)));
      return;
    }
    parent.append(document.createTextNode(source.slice(offset, match.openingIndex)));
    const element = document.createElement(match.tag);
    const content = source.slice(
      match.openingIndex + match.opening.length,
      match.closingIndex,
    );
    if (match.tag === "code") element.textContent = content;
    else appendInlineMarkdown(element, content);
    parent.append(element);
    offset = match.closingIndex + match.closing.length;
  }
}

function appendCopy() {
  const copy = document.createElement("div");
  copy.className = "chapter-copy";
  chapter.append(copy);
  return copy;
}

function createCrecheCallToAction(label = "Birth a Body") {
  const callout = document.createElement("aside");
  callout.className = "creche-handoff";
  const explanation = document.createElement("p");
  explanation.textContent = "The Book explains the idea. The Crèche owns the stateful birth and provisioning workflow.";
  const link = document.createElement("a");
  const configuredUrl = document.querySelector('meta[name="conduit-creche-url"]')?.content.trim();
  if (!configuredUrl) throw new Error("the Book has no configured Crèche entrance");
  link.href = configuredUrl;
  link.textContent = label;
  callout.append(explanation, link);
  return callout;
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
    refreshCompactPatchbay(recursive, recursiveSource.value, true);
  });
  recursiveSource.addEventListener("input", () => {
    directSource.value = recursiveSource.value;
    sourceDrafts.set(direct.dataset.sourceKey, recursiveSource.value);
    refreshCompactPatchbay(direct, directSource.value, false);
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
  runner.dataset.recursive = String(recursive);
  runner.innerHTML = `
    ${presentation.title ? `<header class="realization-heading"><span>${presentation.eyebrow}</span><h3>${presentation.title}</h3>${recursive ? '<button class="flip-back" type="button" aria-pressed="false">Flip Back</button>' : ""}</header>` : ""}
    ${recursive ? '<aside class="back-implementation" aria-hidden="true"><span>Implementation side</span><h4>Reviewed Form Back</h4><p>Run the projection to resolve the exact implementation identities.</p><dl></dl></aside>' : ""}
    <div class="editor">
      <div class="source-editor">
        <label class="editor-label" for="${listingId}">Conduit · editable</label>
        <textarea id="${listingId}" spellcheck="false" aria-label="Editable Conduit listing"></textarea>
        <div class="actions">
          <button class="run" type="button">${presentation.runLabel ?? "Run"}</button><button class="stop" type="button" disabled>Stop</button>
        </div>
      </div>
      ${compactPatchbayFrame()}
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
  const flip = runner.querySelector(".flip-back");
  textarea.value = sourceDrafts.get(sourceKey) ?? source;
  textarea.addEventListener("input", () => {
    sourceDrafts.set(sourceKey, textarea.value);
    refreshCompactPatchbay(runner, textarea.value, recursive);
  });
  run.addEventListener("click", () => runListing(runner, textarea.value, recursive));
  stop.addEventListener("click", () => stopListing(runner));
  flip?.addEventListener("click", () => {
    const flipped = runner.classList.toggle("back-flipped");
    flip.setAttribute("aria-pressed", String(flipped));
    flip.textContent = flipped ? "Flip to Face" : "Flip Back";
    runner.querySelector(".back-implementation").setAttribute("aria-hidden", String(!flipped));
  });
  refreshCompactPatchbay(runner, textarea.value, recursive);
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
      <div class="source-editor">
        <label class="editor-label" for="${listingId}">Conduit · editable · unchanged across Hosts</label>
        <textarea id="${listingId}" spellcheck="false" aria-label="Editable Conduit listing"></textarea>
        <div class="actions">
          <button class="run" type="button">Run across two Hosts</button><button class="stop" type="button" disabled>Stop</button>
        </div>
      </div>
      ${compactPatchbayFrame()}
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
  textarea.addEventListener("input", () => {
    sourceDrafts.set(sourceKey, textarea.value);
    refreshCompactPatchbay(runner, textarea.value, false);
  });
  runner.querySelector(".run").addEventListener("click", () => runMultiHostListing(runner, textarea.value));
  runner.querySelector(".stop").addEventListener("click", () => stopListing(runner));
  refreshCompactPatchbay(runner, textarea.value, false);
  return runner;
}

function compactPatchbayFrame() {
  return `<figure class="compact-patchbay" aria-label="Patchbay">
    <figcaption><span>Form · Patchbay</span><strong>Checking source…</strong></figcaption>
    <div class="book-flow-root" aria-label="Real Patchbay canvas"></div>
    <details class="compact-patchbay-text"><summary>Ordered textual equivalent</summary><ol></ol></details>
    <details class="compact-patchbay-exact"><summary>Exact projection identity</summary><dl></dl></details>
  </figure>`;
}

function refreshCompactPatchbay(runner, source, recursive) {
  const figure = runner.querySelector(".compact-patchbay");
  const expected = ++patchbaySequence;
  figure.dataset.sequence = String(expected);
  const sourceBytes = encoder.encode(source);
  const visual = figure.querySelector(".book-flow-root");
  const text = figure.querySelector(".compact-patchbay-text ol");
  const exact = figure.querySelector(".compact-patchbay-exact dl");
  text.replaceChildren();
  exact.replaceChildren();
  if (sourceBytes.length === 0 || sourceBytes.length > host.runtime.conduit_book_input_capacity()) {
    renderCompactPatchbayRefusal(figure, "Source exceeds the compact Patchbay input bound.");
    return false;
  }
  new Uint8Array(
    host.runtime.memory.buffer,
    host.runtime.conduit_book_input_ptr(),
    sourceBytes.length,
  ).set(sourceBytes);
  const project = recursive
    ? host.runtime.conduit_book_project_patchbay_recursive
    : host.runtime.conduit_book_project_patchbay;
  const code = project(sourceBytes.length, BigInt(expected));
  const output = host.runtime.conduit_book_output_len() > 0 ? readOutput(host.runtime) : null;
  if (code < 0) {
    renderCompactPatchbayRefusal(figure, output?.message ?? `Projection refused (${code}).`);
    return false;
  }
  if (!output || output.sequence !== expected || figure.dataset.sequence !== String(expected)) {
    renderCompactPatchbayRefusal(figure, "Stale compact Patchbay projection refused.");
    return false;
  }
  renderCompactPatchbayProjection(figure, output);
  return true;
}

function renderCompactPatchbayRefusal(figure, message) {
  figure.dataset.disposition = "refused";
  figure.querySelector("figcaption strong").textContent = "Source not checked";
  const visual = figure.querySelector(".book-flow-root");
  renderFlowRefusal(visual, message);
}

function renderCompactPatchbayProjection(figure, projection) {
  figure.dataset.disposition = "accepted";
  figure.dataset.sourceDocumentId = projection.source_document_id;
  figure.dataset.checkedFormId = projection.checked_form_id;
  figure.dataset.expandedFormId = projection.realization_expanded_form_id;
  figure.querySelector("figcaption strong").textContent = projection.form_name;
  const visual = figure.querySelector(".book-flow-root");
  renderFlow(patchbaySnapshot(projection), {
    target: visual,
    lens: "form",
    onSelect: () => {},
    onConnect: () => {},
    onClear: () => {},
  });

  const ordered = figure.querySelector(".compact-patchbay-text ol");
  for (const gear of projection.gears) {
    const item = document.createElement("li");
    const ports = [
      ...gear.inputs.map((port) => `input ${port.port_id}: ${port.info_kind} (${port.temporal})`),
      ...gear.outputs.map((port) => `output ${port.port_id}: ${port.info_kind} (${port.temporal})`),
    ];
    item.textContent = `Gear ${gear.gear_id}, Kind ${gear.kind_id}; ${ports.join("; ") || "no Ports"}.`;
    ordered.append(item);
  }
  for (const cord of projection.cords) {
    const item = document.createElement("li");
    item.textContent = `Cord from ${cord.source_gear_id} output ${cord.source_port_id} to ${cord.sink_gear_id} input ${cord.sink_port_id}; ${cord.info_kind}, ${cord.temporal}.`;
    ordered.append(item);
  }
  appendExactProjection(figure.querySelector(".compact-patchbay-exact dl"), projection);
  const implementation = figure.closest(".runner")?.querySelector(".back-implementation dl");
  if (implementation) renderBackImplementation(implementation, projection);
}

function patchbaySnapshot(projection) {
  const subjects = [];
  const relationships = [];
  const properties = [];
  const addProperty = (subject, name, value) => properties.push({ subject, name, value: { Text: value } });
  for (const gear of projection.gears) {
    subjects.push({ identity: gear.gear_id, role: "Gear", label: gear.gear_id, accessibility_name: `Gear ${gear.gear_id}` });
    addProperty(gear.gear_id, "kind-id", gear.kind_id);
    for (const [direction, ports] of [["receiving", gear.inputs], ["emitting", gear.outputs]]) {
      for (const port of ports) {
        const identity = `${gear.gear_id}.${port.port_id}`;
        subjects.push({ identity, role: "Port", label: port.port_id, accessibility_name: `${direction} Port ${identity}` });
        relationships.push({ source: gear.gear_id, target: identity, kind: "Contains" });
        addProperty(identity, "semantic-id", identity);
        addProperty(identity, "direction", direction);
        addProperty(identity, "value-kind", port.info_kind);
        addProperty(identity, "temporal", port.temporal);
      }
    }
  }
  for (const [index, cord] of projection.cords.entries()) {
    const identity = `cord:${index}:${cord.source_gear_id}.${cord.source_port_id}->${cord.sink_gear_id}.${cord.sink_port_id}`;
    subjects.push({ identity, role: "Cord", label: `Cord ${index + 1}`, accessibility_name: `Cord from ${cord.source_gear_id}.${cord.source_port_id} to ${cord.sink_gear_id}.${cord.sink_port_id}` });
    addProperty(identity, "source-port", `${cord.source_gear_id}.${cord.source_port_id}`);
    addProperty(identity, "sink-port", `${cord.sink_gear_id}.${cord.sink_port_id}`);
    addProperty(identity, "value-kind", cord.info_kind);
  }
  return {
    presentation: {
      identity: projection.visible_expanded_form_id,
      revision: projection.sequence,
      basis: { source_document_id: projection.source_document_id, checked_form_id: projection.checked_form_id },
      subjects, relationships, properties, text: [], actions: [], disclosures: [],
    },
    interaction: { revision: projection.sequence, selected_subject: null },
  };
}

function renderBackImplementation(list, projection) {
  list.replaceChildren();
  const backs = projection.realization_backs.length > 0
    ? projection.realization_backs
    : [{ invocation_path: "direct leaf", kind_id: "No reviewed Back opened", checked_form_id: projection.checked_form_id }];
  for (const back of backs) {
    for (const [name, value] of [
      ["Invocation", back.invocation_path],
      ["Implementation Kind", back.kind_id],
      ["Checked implementation Form", back.checked_form_id],
      ["Realization expansion", projection.realization_expanded_form_id],
    ]) {
      const term = document.createElement("dt");
      term.textContent = name;
      const description = document.createElement("dd");
      description.textContent = value;
      list.append(term, description);
    }
  }
}

function appendExactProjection(list, projection) {
  for (const [name, value] of [
    ["Source", projection.source_document_id],
    ["Checked Form", projection.checked_form_id],
    ["Visible expansion", projection.visible_expanded_form_id],
    ["Realization expansion", projection.realization_expanded_form_id],
    ["Realization", projection.realization],
    ["Opened Backs", projection.realization_backs.length],
  ]) {
    const term = document.createElement("dt");
    term.textContent = name;
    const description = document.createElement("dd");
    description.textContent = String(value);
    list.append(term, description);
  }
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
