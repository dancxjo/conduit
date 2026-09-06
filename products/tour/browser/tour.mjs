import { initializeBrowserHost } from "../../../targets/browser/host/assets/browser-host-membership.mjs";
import { configureFlowStorage, renderFlow, renderFlowRefusal } from "../../patchbay/html/assets/flow.js";
import { conceptualTourStage, createTourStage, openTourReadingState } from "./tour-state.mjs";
import { createTourNavigation, createTourRunnerActions, createTourWorkspace } from "./tour-navigation.mjs";
import { createTourEvidenceTables, createTourPlanPresentation, createTourRunnerField, createTourRunnerStatus, restoreTourRunnerDraft } from "./tour-runner-presentation.mjs";
import { createProductMasthead } from "../../../semantics/presentation/assets/product-masthead.mjs";
import { attachConduitSyntaxEditor, createConduitSyntaxExample } from "../../../targets/browser/host/assets/application-syntax-presentation.mjs";
import { createTourRouting, parseTourPages } from "./tour-routing.mjs";
import { createReviewedFormGallery, presentTourInventory, readReviewedGallery, reviewedFormStage } from "./tour-inventory-presentation.mjs";
import { openBrowserHumanInput } from "../../../targets/browser/host/assets/browser-human-input.mjs";

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const chapter = document.querySelector("#chapter");
let host;
let peerHost = null;
let generation = 0;
let running = false;
let runnerSlotSequence = 0;
let activeRunner = null;
const activeDelays = new Set();
let humanInput = null;
let currentPage = 0;
let guidedPages = [];
let patchbaySequence = 0;
let readingState;
let admittedRuntimeBytes;
let navigation;
let hostPresentation, hostPresentationFor, hostStatus;
let routing;
let workspace;
let gallery;
let laboratorySelectionSequence = 0;
const laboratory = document.createElement("div");
laboratory.className = "tour-workbench";
laboratory.dataset.applicationComponent = "tour-laboratory";

export async function startApplication(application) {
try {
  hostPresentation = application.presentation;
  hostPresentationFor = application.presentationFor;
  hostStatus = createProductMasthead(hostPresentation, "product-masthead", "tour");
  hostStatus.ordinary("Starting browser Host…");
  readingState = await openTourReadingState(application.storage);
  workspace = createTourWorkspace(document, readingState);
  configureFlowStorage(application.storage);
  navigation = createTourNavigation(hostPresentation, (offset) => {
    renderPage(currentPage + offset, "push").catch(showTourFailure);
  });
  admittedRuntimeBytes = application.bytes("runtime");
  const [chapters, initialized] = await Promise.all([
    Promise.resolve([1, 2, 3, 4, 5, 6, 8].map((number) => application.text(`chapter-${number}`))),
    initializeBrowserHost(admittedRuntimeBytes),
  ]);
  host = initialized;
  requireTourAbi(host.runtime);
  gallery = readReviewedGallery(host.runtime);
  if (host.runtime.conduit_browser_form_human_machinery() < 0) {
    throw new Error("browser Host selected machinery is unavailable");
  }
  const selectedMachinery = readOutput(host.runtime);
  if (selectedMachinery.schema !== "conduit.browser/selected-human-machinery@1") {
    throw new Error("browser Host selected machinery is malformed");
  }
  humanInput = openBrowserHumanInput({
    target: document,
    boot: {
      host_id: host.hostId,
      boot_id: host.bootId,
      offer_generation: 1,
      implementation_registry: selectedMachinery.implementations,
    },
  });
  routing = createTourRouting({
    host,
    applicationId: application.manifest.applicationId,
    render: (index) => renderPage(index),
    onFailure: showTourFailure,
  });
  guidedPages = parseTourPages(chapters);
  setupTourModes();
  const initialRoute = routing.admitPages(guidedPages);
  await renderPage(initialRoute.index);
  if (initialRoute.normalize) await routing.move(initialRoute.index, "replace");
  hostStatus.success("Browser Host ready");
  globalThis.__conduitTourHost = host;
  globalThis.__conduitTourLaboratory = laboratory;
  globalThis.__conduitTourPersistence = Object.freeze({
    schema: "conduit.tour/persistence@1",
    flush: readingState.flush,
  });
} catch (error) {
  hostStatus.failure("Browser Host unavailable");
  chapter.textContent = error instanceof Error ? error.message : String(error);
  chapter.classList.add("error");
}
}

function showTourFailure(error) {
  hostStatus.failure("Browser Host unavailable");
  chapter.textContent = error instanceof Error ? error.message : String(error);
  chapter.classList.add("error");
}

function persistTourState() {
  return readingState.persist();
}

function requireTourAbi(api) {
  const required = [
    "memory", "conduit_browser_form_input_ptr", "conduit_browser_form_input_capacity",
    "conduit_browser_form_output_ptr", "conduit_browser_form_output_len", "conduit_browser_form_start",
    "conduit_browser_form_acknowledge_cancellation", "conduit_browser_form_poll_effect", "conduit_browser_form_complete_effect", "conduit_browser_form_pending_capacity",
    "conduit_browser_form_start_recursive", "conduit_browser_form_complete", "conduit_browser_form_complete_with_output", "conduit_browser_form_cancel",
    "conduit_browser_form_inventory", "conduit_browser_form_human_machinery", "conduit_browser_form_admit_source_interaction",
    "conduit_browser_form_reviewed_gallery",
    "conduit_tour_encode_button_transition",
    "conduit_tour_project_patchbay", "conduit_tour_project_patchbay_recursive",
    "conduit_syntax_input_ptr", "conduit_syntax_input_capacity",
    "conduit_syntax_output_ptr", "conduit_syntax_output_len", "conduit_syntax_project",
    "conduit_tour_multi_input_ptr", "conduit_tour_multi_input_capacity",
    "conduit_tour_multi_output_ptr", "conduit_tour_multi_output_len",
    "conduit_tour_multi_admit_source_interaction", "conduit_tour_multi_start_source",
    "conduit_tour_multi_start_sink",
    "conduit_tour_multi_ingest", "conduit_tour_multi_complete", "conduit_tour_multi_cancel",
  ];
  if (required.some((name) => !(name in api))) throw new Error("executable-tour ABI is incomplete");
}

async function renderPage(index, routeChange = "none") {
  retireActiveLaboratory();
  if (routeChange === "push") await routing.move(index, "push");
  currentPage = index;
  setTourMode("guided");
  workspace.showLesson();
  chapter.replaceChildren();
  renderMarkdown(guidedPages[index]);
  chapter.scrollTop = 0;
  if (routeChange === "push") chapter.querySelector("h1")?.focus({ preventScroll: true });
  navigation.render(currentPage, guidedPages.length, running);
  document.title = guidedPages[index].title + " · Tour";
}

function setupTourModes() {
  const guided = document.querySelector('[data-tour-mode="guided"]');
  const galleryButton = document.querySelector('[data-tour-mode="gallery"]');
  if (!guided || !galleryButton) throw new Error("Tour entrances are incomplete");
  guided.addEventListener("click", () => renderPage(currentPage).catch(showTourFailure));
  galleryButton.addEventListener("click", () => renderGallery());
}

function setTourMode(mode) {
  if (mode !== "guided" && mode !== "gallery") throw new Error("Tour entrance is not admitted");
  document.body.dataset.tourMode = mode;
  for (const button of document.querySelectorAll("[data-tour-mode]")) {
    button.setAttribute("aria-pressed", String(button.dataset.tourMode === mode));
  }
}

function renderGallery() {
  retireActiveLaboratory();
  setTourMode("gallery");
  workspace.showLesson();
  chapter.replaceChildren();
  const crecheUrl = document.querySelector('meta[name="conduit-creche-url"]')?.content;
  if (!crecheUrl) throw new Error("Crèche product handoff is unavailable");
  const gallerySurface = createReviewedFormGallery(document, gallery, crecheUrl, (form, action) => {
    selectLaboratoryStage(reviewedFormStage(form), [], true);
    gallerySurface.select(form.checked_form_id);
    if (action === "inspect") {
      const patchbay = laboratory.querySelector(".compact-patchbay");
      patchbay.tabIndex = -1;
      patchbay.focus({ preventScroll: true });
    }
  });
  const { surface, heading } = gallerySurface;
  chapter.append(surface);
  selectLaboratoryStage(reviewedFormStage(gallery.forms[0]), []);
  gallerySurface.select(gallery.forms[0].checked_form_id);
  document.querySelector("#laboratory-slot").replaceChildren(laboratory);
  chapter.scrollTop = 0;
  heading.focus({ preventScroll: true });
  document.title = "Form Gallery · Tour";
}

function setNavigationDisabled(disabled) {
  if (disabled !== running) throw new Error("Tour navigation state is inconsistent");
  navigation.render(currentPage, guidedPages.length, running);
}

function renderMarkdown(page) {
  const lines = page.markdown.replaceAll("\r\n", "\n").split("\n");
  let copy = appendCopy();
  let paragraph = [];
  const stages = [];
  let declaredStageIndex = 0;
  const admitStage = (stage) => {
    const declared = page.stages[declaredStageIndex++];
    if (!declared || declared.identity !== stage.identity || declared.mode !== stage.mode) {
      throw new Error("Tour runnable source does not match its admitted page stage");
    }
    return stage;
  };
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
      const source = [];
      index += 1;
      while (index < lines.length && lines[index] !== "```") source.push(lines[index++]);
      copy.append(createConduitSyntaxExample(source.join("\n"), host.runtime));
      chapter.append(createCrecheCallToAction());
      copy = appendCopy();
    } else if (line === "```conduit run two-host" || line === "```conduit run two-host plan") {
      flush();
      const showPlan = line.endsWith(" plan");
      const source = [];
      index += 1;
      while (index < lines.length && lines[index] !== "```") source.push(lines[index++]);
      appendStageSelector(copy, stages, admitStage(createTourStage(source.join("\n"), showPlan ? "two-host-plan" : "two-host")));
      copy = appendCopy();
    } else if (line === "```conduit run" || line === "```conduit run recursive" || line === "```conduit compare") {
      flush();
      const recursive = line.endsWith(" recursive");
      const comparison = line.endsWith(" compare");
      const source = [];
      index += 1;
      while (index < lines.length && lines[index] !== "```") source.push(lines[index++]);
      appendStageSelector(copy, stages, admitStage(createTourStage(
        source.join("\n"), comparison ? "compare" : recursive ? "recursive" : "run",
      )));
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
      heading.tabIndex = -1;
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
  if (declaredStageIndex !== page.stages.length) throw new Error("Tour page declares a stage with no runnable source");
  selectLaboratoryStage(stages[0] ?? conceptualTourStage(page.title, page.companion), stages);
  document.querySelector("#laboratory-slot").replaceChildren(laboratory);
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

function appendStageSelector(copy, stages, stage) {
  stages.push(stage);
  const button = document.createElement("button");
  button.type = "button";
  button.className = "tour-stage-selector";
  button.dataset.specimenId = stage.identity;
  button.textContent = `Load ${stage.label} in the laboratory`;
  button.addEventListener("click", () => selectLaboratoryStage(stage, stages, true));
  copy.append(button);
}

function selectLaboratoryStage(stage, stages, reveal = false) {
  retireActiveLaboratory();
  laboratory.dataset.specimenId = stage.identity;
  laboratory.dataset.mode = stage.mode;
  laboratory.dataset.selectionSequence = String(++laboratorySelectionSequence);
  if (reveal) workspace.showLaboratory(true);
  for (const button of chapter.querySelectorAll(".tour-stage-selector")) {
    button.setAttribute("aria-pressed", String(button.dataset.specimenId === stage.identity
      && stages.indexOf(stage) === [...chapter.querySelectorAll(".tour-stage-selector")].indexOf(button)));
  }
  if (stage.mode === "conceptual") {
    const companion = document.createElement("section");
    companion.className = "conceptual-laboratory";
    companion.innerHTML = `<figure class="compact-patchbay" aria-label="Patchbay"><figcaption><span>Lesson companion · Patchbay</span><strong>No editable specimen selected</strong></figcaption><div class="conceptual-patchbay">This lesson describes Body state without inventing an executable Form.</div></figure><div class="result"><h2>Lesson state</h2><output aria-label="Planned result">No Play requested</output><p>Choose another lesson to load an executable specimen.</p></div>`;
    laboratory.replaceChildren(companion);
    return;
  }
  const runner = stage.multiHost
    ? createMultiHostRunner(stage.source, stage.showPlan, stage.identity)
    : createRunner(stage.source, stage.recursive, {
      faceBack: stage.faceBack,
      runLabel: stage.faceBack ? "Run this Form" : "Run",
      sourceKey: stage.identity,
    });
  laboratory.replaceChildren(runner);
  if (stage.checkedFormId) {
    const patchbay = runner.querySelector(".compact-patchbay");
    if (patchbay.dataset.sourceDocumentId !== stage.sourceDocumentId
      || patchbay.dataset.checkedFormId !== stage.checkedFormId) {
      throw new Error("Gallery source does not project its exact reviewed Form identity");
    }
  }
}

function retireActiveLaboratory() {
  if (!running || !activeRunner) return;
  const retiredIdentity = activeRunner.dataset.sourceKey;
  stopListing(activeRunner);
  laboratory.dataset.retiredSpecimenId = retiredIdentity;
  laboratory.dataset.retirementDisposition = "cancelled";
}

function createCrecheCallToAction(label = "Birth a Body") {
  const callout = document.createElement("aside");
  callout.className = "creche-handoff";
  const explanation = document.createElement("p");
  explanation.textContent = "The Tour explains the idea. The Crèche owns the stateful birth and provisioning workflow.";
  const link = document.createElement("a");
  const configuredUrl = document.querySelector('meta[name="conduit-creche-url"]')?.content.trim();
  if (!configuredUrl) throw new Error("Tour has no configured Crèche entrance");
  link.href = configuredUrl;
  link.textContent = label;
  callout.append(explanation, link);
  return callout;
}

function createRunner(source, recursive = false, presentation = {}) {
  const sourceKey = presentation.sourceKey;
  const listingId = "listing";
  const actionsSlot = `tour-runner-actions-${++runnerSlotSequence}`;
  const fieldSlot = `tour-runner-field-${runnerSlotSequence}`;
  const statusSlot = `tour-runner-status-${runnerSlotSequence}`;
  const exactSlot = `tour-exact-evidence-${runnerSlotSequence}`;
  const runSlot = `tour-run-evidence-${runnerSlotSequence}`;
  const runner = document.createElement("section");
  runner.className = "runner";
  runner.dataset.sourceKey = sourceKey;
  runner.dataset.recursive = String(recursive);
  runner.innerHTML = `
    ${compactPatchbayFrame()}
    <div class="editor">
      <div data-application-slot="${fieldSlot}"></div>
      <div data-application-slot="${actionsSlot}"></div>
    </div>
    <div class="result">
      <div class="indicator" role="img" aria-label="Indicator off"></div>
      <button type="button" class="input-button" hidden>Hold to control indicator</button>
      <h2>Planned result</h2>
      <output class="morse" aria-label="Planned result">ready</output>
      <div data-application-slot="${statusSlot}"></div>
      <details class="exact-evidence"><summary>Inspect exact evidence</summary>
        <h3>Checked Form</h3><div class="exact-projection" data-application-slot="${exactSlot}"></div>
        <h3>Latest run</h3><div class="run-identities" data-application-slot="${runSlot}"></div><div class="expansion"></div>
      </details>
    </div>`;
  runner.dataset.faceBack = String(presentation.faceBack === true);
  const runnerPresentation = hostPresentationFor(runner);
  const initialSource = readingState.drafts.get(sourceKey) ?? source;
  createTourRunnerField(runnerPresentation, fieldSlot, listingId, "Conduit · editable", initialSource, (value) => {
    readingState.drafts.set(sourceKey, value);
    persistTourState();
    refreshCompactPatchbay(runner, value, recursive);
  });
  const textarea = runner.querySelector(`[data-application-key="${listingId}"]`);
  const syntaxEditor = attachConduitSyntaxEditor(textarea, host.runtime);
  runner.actionControls = createTourRunnerActions(
    runnerPresentation, actionsSlot, presentation.runLabel ?? "Run",
    () => runListing(runner, textarea.value, recursive), () => stopListing(runner),
    () => restoreTourRunnerDraft({
      runner, textarea, source, sourceKey, readingState, syntaxEditor,
      cancel: () => { if (running && activeRunner === runner) stopListing(runner); },
      refresh: (value) => refreshCompactPatchbay(runner, value, recursive),
    }),
  );
  runner.playStatus = createTourRunnerStatus(
    runnerPresentation, statusSlot, "Edit the message or timing, then run it.",
  );
  runner.evidence = createTourEvidenceTables(runnerPresentation, exactSlot, runSlot);
  queueMicrotask(() => runner.actionControls.render(false));
  refreshCompactPatchbay(runner, textarea.value, recursive);
  return runner;
}

function createMultiHostRunner(source, showPlan, sourceKey) {
  const listingId = "listing";
  const actionsSlot = `tour-runner-actions-${++runnerSlotSequence}`;
  const fieldSlot = `tour-runner-field-${runnerSlotSequence}`;
  const statusSlot = `tour-runner-status-${runnerSlotSequence}`;
  const exactSlot = `tour-exact-evidence-${runnerSlotSequence}`;
  const runSlot = `tour-run-evidence-${runnerSlotSequence}`;
  const planSlot = `tour-plan-evidence-${runnerSlotSequence}`;
  const runner = document.createElement("section");
  runner.className = "runner multi-host-runner";
  runner.dataset.sourceKey = sourceKey;
  runner.dataset.mode = "multi";
  runner.innerHTML = `
    ${compactPatchbayFrame()}
    <div class="editor">
      <div data-application-slot="${fieldSlot}"></div>
      <div data-application-slot="${actionsSlot}"></div>
    </div>
    <div class="result multi-host-result">
      <div class="host-map" aria-label="Two independent browser Hosts">
        <article class="host-card host-a"><span>Host A · source</span><strong>waiting</strong><code class="host-id"></code><code class="boot-id"></code></article>
        <div class="planned-line" aria-label="One planned cross-Host Cord"><span>typed Cord</span><b>→</b><small>1 item · finite bytes</small></div>
        <article class="host-card host-b"><span>Host B · presentation</span><strong>waiting</strong><code class="host-id"></code><code class="boot-id"></code></article>
      </div>
      <h2>Planned result on Host B</h2>
      <output class="morse" aria-label="Planned result">ready</output>
      <div data-application-slot="${statusSlot}"></div>
      <details class="exact-evidence plan-view-details"><summary>Inspect exact evidence</summary>
        <h3>Checked Form</h3><div class="exact-projection" data-application-slot="${exactSlot}"></div>
        <h3>Latest run</h3><div class="run-identities" data-application-slot="${runSlot}"></div><div class="expansion"></div>
        <h3>Exact Plan for this Play</h3><div class="plan-view" data-application-slot="${planSlot}"></div>
      </details>
    </div>`;
  runner.querySelector(".plan-view-details").dataset.includesPlan = String(showPlan);
  const runnerPresentation = hostPresentationFor(runner);
  const initialSource = readingState.drafts.get(sourceKey) ?? source;
  createTourRunnerField(
    runnerPresentation, fieldSlot, listingId, "Conduit · editable · unchanged across Hosts", initialSource, (value) => {
      readingState.drafts.set(sourceKey, value);
      persistTourState();
      refreshCompactPatchbay(runner, value, false);
    },
  );
  const textarea = runner.querySelector(`[data-application-key="${listingId}"]`);
  const syntaxEditor = attachConduitSyntaxEditor(textarea, host.runtime);
  runner.actionControls = createTourRunnerActions(
    runnerPresentation, actionsSlot, "Run across two Hosts",
    () => runMultiHostListing(runner, textarea.value), () => stopListing(runner),
    () => restoreTourRunnerDraft({
      runner, textarea, source, sourceKey, readingState, syntaxEditor,
      cancel: () => { if (running && activeRunner === runner) stopListing(runner); },
      refresh: (value) => refreshCompactPatchbay(runner, value, false),
    }),
  );
  runner.playStatus = createTourRunnerStatus(
    runnerPresentation, statusSlot, "Run the Form to start two independent browser Hosts.",
  );
  runner.evidence = createTourEvidenceTables(runnerPresentation, exactSlot, runSlot);
  runner.planEvidence = createTourPlanPresentation(runnerPresentation, planSlot);
  queueMicrotask(() => runner.actionControls.render(false));
  refreshCompactPatchbay(runner, textarea.value, false);
  return runner;
}

function compactPatchbayFrame() {
  return `<figure class="compact-patchbay" aria-label="Patchbay">
    <figcaption><span>Form · Patchbay</span><strong>Checking source…</strong></figcaption>
    <div class="tour-flow-root" aria-label="Real Patchbay canvas"></div>
    <ol class="compact-patchbay-text" aria-label="Ordered textual equivalent" hidden></ol>
    <section class="gear-back-expansion" hidden aria-label="Reviewed Form Back topology">
      <header><strong>Inside this Gear</strong><button type="button" class="close-gear-back">Return to Face</button></header>
      <div class="tour-flow-root gear-back-flow" aria-label="Reviewed Form Back Patchbay topology"></div>
    </section>
  </figure>`;
}

function refreshCompactPatchbay(runner, source, recursive) {
  const figure = runner.querySelector(".compact-patchbay");
  figure.dataset.backExpanded = "false";
  figure.querySelector(".gear-back-expansion").hidden = true;
  const expected = ++patchbaySequence;
  figure.dataset.sequence = String(expected);
  const sourceBytes = encoder.encode(source);
  const visual = figure.querySelector(".tour-flow-root");
  const text = figure.querySelector(".compact-patchbay-text");
  text.replaceChildren();
  if (sourceBytes.length === 0 || sourceBytes.length > host.runtime.conduit_browser_form_input_capacity()) {
    renderCompactPatchbayRefusal(figure, "Source exceeds the compact Patchbay input bound.");
    return false;
  }
  new Uint8Array(
    host.runtime.memory.buffer,
    host.runtime.conduit_browser_form_input_ptr(),
    sourceBytes.length,
  ).set(sourceBytes);
  const project = recursive
    ? host.runtime.conduit_tour_project_patchbay_recursive
    : host.runtime.conduit_tour_project_patchbay;
  const code = project(sourceBytes.length, BigInt(expected));
  const output = host.runtime.conduit_browser_form_output_len() > 0 ? readOutput(host.runtime) : null;
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
  const visual = figure.querySelector(".tour-flow-root");
  renderFlowRefusal(visual, message);
}

function renderCompactPatchbayProjection(figure, projection) {
  figure.faceProjection = projection;
  const invalid = projection.diagnostics.length > 0;
  figure.dataset.disposition = invalid ? "invalid" : "accepted";
  figure.dataset.sourceDocumentId = projection.source_document_id;
  figure.dataset.checkedFormId = projection.checked_form_id;
  figure.dataset.expandedFormId = projection.realization_expanded_form_id;
  figure.querySelector("figcaption strong").textContent = invalid ? `${projection.form_name} · needs repair` : projection.form_name;
  const visual = figure.querySelector(".tour-flow-root");
  figure.querySelector(".compact-patchbay-diagnostic")?.remove();
  if (invalid) {
    const panel = document.createElement("section");
    panel.className = "compact-patchbay-diagnostic";
    panel.setAttribute("role", "alert");
    const diagnostic = projection.diagnostics[0];
    const heading = document.createElement("strong");
    heading.textContent = `${diagnostic.code} · ${diagnostic.message}`;
    const fix = document.createElement("p");
    fix.textContent = `How to fix: ${diagnostic.fix}`;
    panel.append(heading, fix);
    visual.before(panel);
  }
  const runner = figure.closest(".runner");
  const expanded = figure.dataset.backExpanded === "true";
  renderFlow(patchbaySnapshot(projection, {
    reviewedBack: runner?.dataset.faceBack === "true",
    backExpanded: expanded,
  }), {
    target: visual,
    lens: "form",
    onSelect: () => {},
    onConnect: () => {},
    onClear: () => {},
    onOpenBack: (subjectIdentity) => toggleGearBack(figure, projection, subjectIdentity),
  });

  const ordered = figure.querySelector(".compact-patchbay-text");
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
  runner?.evidence.projection(projection);
  const sourceKey = runner?.dataset.sourceKey;
  if (runner?.dataset.faceBack === "true" && sourceKey && readingState.expandedBacks.has(sourceKey)
    && figure.dataset.backExpanded !== "true" && figure.dataset.backRestoreApplied !== "true") {
    figure.dataset.backRestoreApplied = "true";
    const subject = projection.gears.find((gear) => gear.kind_id === "text/morse")?.gear_id;
    if (subject) queueMicrotask(() => toggleGearBack(figure, projection, subject));
  }
}

function toggleGearBack(figure, faceProjection, subjectIdentity) {
  const expansion = figure.querySelector(".gear-back-expansion");
  const opening = figure.dataset.backExpanded !== "true";
  figure.dataset.backExpanded = String(opening);
  expansion.hidden = !opening;
  const sourceKey = figure.closest(".runner")?.dataset.sourceKey;
  if (sourceKey) {
    if (opening) readingState.expandedBacks.add(sourceKey);
    else readingState.expandedBacks.delete(sourceKey);
    persistTourState();
  }
  renderCompactPatchbayProjection(figure, faceProjection);
  if (!opening) return;

  const runner = figure.closest(".runner");
  const source = runner.querySelector("textarea").value;
  const sourceBytes = encoder.encode(source);
  const expected = ++patchbaySequence;
  new Uint8Array(
    host.runtime.memory.buffer,
    host.runtime.conduit_browser_form_input_ptr(),
    sourceBytes.length,
  ).set(sourceBytes);
  const code = host.runtime.conduit_tour_project_patchbay_recursive(sourceBytes.length, BigInt(expected));
  const back = host.runtime.conduit_browser_form_output_len() > 0 ? readOutput(host.runtime) : null;
  if (code < 0 || !back || back.sequence !== expected) {
    renderFlowRefusal(expansion.querySelector(".gear-back-flow"), back?.message ?? "Reviewed Back unavailable.");
    return;
  }
  if (back.source_document_id !== faceProjection.source_document_id
    || back.checked_form_id !== faceProjection.checked_form_id) {
    renderFlowRefusal(expansion.querySelector(".gear-back-flow"), "Reviewed Back changed the requested Face.");
    return;
  }
  expansion.dataset.subjectIdentity = subjectIdentity;
  expansion.dataset.sourceDocumentId = back.source_document_id;
  expansion.dataset.checkedFormId = back.checked_form_id;
  expansion.dataset.expandedFormId = back.realization_expanded_form_id;
  renderFlow(patchbaySnapshot(back, { realizationTopology: true }), {
    target: expansion.querySelector(".gear-back-flow"),
    lens: "form",
    onSelect: () => {},
    onConnect: () => {},
    onClear: () => {},
    onOpenBack: () => {},
  });
  expansion.querySelector(".close-gear-back").onclick = () => {
    if (figure.dataset.backExpanded === "true") toggleGearBack(figure, faceProjection, subjectIdentity);
  };
}

function patchbaySnapshot(projection, options = {}) {
  const subjects = [];
  const relationships = [];
  const properties = [];
  const gears = options.realizationTopology ? projection.realization_gears : projection.gears;
  const cords = options.realizationTopology ? projection.realization_cords : projection.cords;
  const addProperty = (subject, name, value) => properties.push({ subject, name, value: { Text: value } });
  const portIdentity = (gearId, direction, portId) => `${gearId}.${direction}:${portId}`;
  const diagnosticSubjects = new Set(projection.diagnostics.flatMap((diagnostic) => diagnostic.subjects));
  for (const gear of gears) {
    subjects.push({ identity: gear.gear_id, role: "Gear", label: gear.gear_id, accessibility_name: `Gear ${gear.gear_id}` });
    addProperty(gear.gear_id, "kind-id", gear.kind_id);
    if (diagnosticSubjects.has(gear.gear_id)) addProperty(gear.gear_id, "diagnostic-state", "error");
    if (options.reviewedBack && gear.kind_id === "text/morse") {
      addProperty(gear.gear_id, "reviewed-back", "available");
      addProperty(gear.gear_id, "back-expanded", String(options.backExpanded === true));
    }
    for (const [direction, ports] of [["receiving", gear.inputs], ["emitting", gear.outputs]]) {
      for (const port of ports) {
        // A Gear may intentionally use the same authored name for its input
        // and output. Direction is therefore part of the presentation subject
        // identity even though the authored Port label remains unchanged.
        const identity = portIdentity(gear.gear_id, direction, port.port_id);
        subjects.push({ identity, role: "Port", label: port.port_id, accessibility_name: `${direction} Port ${identity}` });
        relationships.push({ source: gear.gear_id, target: identity, kind: "Contains" });
        addProperty(identity, "semantic-id", identity);
        addProperty(identity, "direction", direction);
        addProperty(identity, "value-kind", port.info_kind);
        addProperty(identity, "temporal", port.temporal);
        if (diagnosticSubjects.has(identity)) addProperty(identity, "diagnostic-state", "error");
      }
    }
  }
  for (const [index, cord] of cords.entries()) {
    const identity = `cord:${index}:${cord.source_gear_id}.${cord.source_port_id}->${cord.sink_gear_id}.${cord.sink_port_id}`;
    subjects.push({ identity, role: "Cord", label: `Cord ${index + 1}`, accessibility_name: `Cord from ${cord.source_gear_id}.${cord.source_port_id} to ${cord.sink_gear_id}.${cord.sink_port_id}` });
    addProperty(identity, "source-port", portIdentity(cord.source_gear_id, "emitting", cord.source_port_id));
    addProperty(identity, "sink-port", portIdentity(cord.sink_gear_id, "receiving", cord.sink_port_id));
    addProperty(identity, "value-kind", cord.info_kind);
    // Motion on the authored Face communicates Cord direction only. It is not
    // presented as evidence that a Play delivered an item.
    addProperty(identity, "flow-animation", "directional");
    addProperty(identity, "flow-label", "");
    if (cord.invalid || diagnosticSubjects.has(identity)) addProperty(identity, "diagnostic-state", "error");
  }
  return {
    presentation: {
      identity: projection.visible_expanded_form_id || projection.source_proposal_id,
      revision: projection.sequence,
      basis: { source_document_id: projection.source_document_id, checked_form_id: projection.checked_form_id },
      subjects, relationships, properties, text: [], actions: [], disclosures: [],
    },
    interaction: { revision: projection.sequence, selected_subject: null },
  };
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
    if (encoded.length > this.maximumFrameBytes || encoded.length > targetApi.conduit_tour_multi_input_capacity()) {
      throw new Error("browser-memory Line frame exceeds its exact admitted bound");
    }
    this.pending = encoded;
    const input = new Uint8Array(
      targetApi.memory.buffer,
      targetApi.conduit_tour_multi_input_ptr(),
      encoded.length,
    );
    input.set(this.pending);
    this.pending = null;
    const code = targetApi.conduit_tour_multi_ingest(encoded.length);
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
  const initialized = await initializeBrowserHost(admittedRuntimeBytes, { durable: false });
  requireTourAbi(initialized.runtime);
  if (initialized.hostId === host.hostId || initialized.bootId === host.bootId) {
    throw new Error("second browser Host did not receive independent Host and Boot identity");
  }
  peerHost = initialized;
  globalThis.__conduitTourPeerHost = peerHost;
  return peerHost;
}

async function runMultiHostListing(runner, source) {
  if (running && activeRunner) stopListing(activeRunner);
  const current = ++generation;
  running = true;
  activeRunner = runner;
  setNavigationDisabled(true);
  runner.actionControls.render(true);
  runner.playStatus.ordinary("Starting an independent second browser Host…");
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
    runner.playStatus.ordinary("Host A offered one value on the exact planned Cord…");
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
    runner.playStatus.ordinary("Host B observed the planned presentation; acknowledging delivery…");
    if (!await nextPaint(current)) return;
    const completion = peer.runtime.conduit_tour_multi_complete();
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
    runner.playStatus.success("Completed — one immutable Plan, two independent Plays, one delivered cross-Host value.");
    appendRunEvidence(runner, [
      ["Terminal source receipt", sourceReceipt.receipt.terminal_sign_id],
      ["Terminal sink receipt", terminal.receipt.terminal_sign_id],
    ]);
    finishRun(runner);
  } catch (error) {
    cancelMultiSessions();
    runner.playStatus.failure(error instanceof Error ? error.message : String(error));
    finishRun(runner);
  }
}

function admitMultiSource(api, sourceBytes, sequence) {
  if (sourceBytes.length > api.conduit_tour_multi_input_capacity()) {
    throw new Error("The listing exceeds the admitted multi-Host input bound.");
  }
  new Uint8Array(api.memory.buffer, api.conduit_tour_multi_input_ptr(), sourceBytes.length).set(sourceBytes);
  const code = api.conduit_tour_multi_admit_source_interaction(sourceBytes.length, BigInt(sequence));
  if (code < 0) {
    const refusal = api.conduit_tour_multi_output_len() > 0 ? readMultiOutput(api) : null;
    throw new Error(refusal?.message ?? `multi-Host source interaction refused (${code})`);
  }
}

function startMultiSource(api, sourceHost, sinkHost, sourceBytes, sequence) {
  const fields = [sourceHost.hostId, sourceHost.bootId, sinkHost.hostId, sinkHost.bootId]
    .map((value) => encoder.encode(value));
  const total = fields.reduce((sum, field) => sum + field.length, sourceBytes.length);
  if (total > api.conduit_tour_multi_input_capacity()) {
    throw new Error("multi-Host start frame exceeds its admitted input bound");
  }
  const input = new Uint8Array(api.memory.buffer, api.conduit_tour_multi_input_ptr(), total);
  let offset = 0;
  for (const field of fields) {
    input.set(field, offset);
    offset += field.length;
  }
  input.set(sourceBytes, offset);
  const code = api.conduit_tour_multi_start_source(
    fields[0].length,
    fields[1].length,
    fields[2].length,
    fields[3].length,
    sourceBytes.length,
    BigInt(sequence),
  );
  if (code < 0) {
    const refusal = api.conduit_tour_multi_output_len() > 0 ? readMultiOutput(api) : null;
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
  if (total > api.conduit_tour_multi_input_capacity()) {
    throw new Error("exact multi-Host Plan exceeds its admitted sink input bound");
  }
  const input = new Uint8Array(api.memory.buffer, api.conduit_tour_multi_input_ptr(), total);
  let offset = 0;
  for (const field of fields) {
    input.set(field, offset);
    offset += field.length;
  }
  const code = api.conduit_tour_multi_start_sink(
    fields[0].length,
    fields[1].length,
    fields[2].length,
    BigInt(sequence),
  );
  if (code < 0) {
    const refusal = api.conduit_tour_multi_output_len() > 0 ? readMultiOutput(api) : null;
    throw new Error(refusal?.message
      ? `Host B refused the exact Plan before Play · ${refusal.message}`
      : `multi-Host sink Plan admission refused (${code})`);
  }
  return readMultiOutput(api);
}

function readMultiOutput(api) {
  const bytes = new Uint8Array(
    api.memory.buffer,
    api.conduit_tour_multi_output_ptr(),
    api.conduit_tour_multi_output_len(),
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
  runner.planEvidence.render(plan);
  runner.querySelector(".plan-view-details").dataset.planId = plan.plan_id;
}

function nextPaint(expectedGeneration) {
  return new Promise((resolve) => requestAnimationFrame(() => resolve(expectedGeneration === generation)));
}

function finishRun(runner) {
  activeMemoryLine = null;
  running = false;
  activeRunner = null;
  setNavigationDisabled(false);
  runner.actionControls.render(false);
}

async function runListing(runner, source, recursive) {
  if (running && activeRunner) stopListing(activeRunner);
  const current = ++generation;
  const api = host.runtime;
  const sourceBytes = encoder.encode(source);
  const hostBytes = encoder.encode(host.hostId);
  const bootBytes = encoder.encode(host.bootId);
  const total = hostBytes.length + bootBytes.length + sourceBytes.length;
  if (total > api.conduit_browser_form_input_capacity()) {
    runner.playStatus.failure("The listing exceeds the admitted input bound.");
    return;
  }
  const input = new Uint8Array(api.memory.buffer, api.conduit_browser_form_input_ptr(), total);
  const interactionInput = new Uint8Array(
    api.memory.buffer,
    api.conduit_browser_form_input_ptr(),
    sourceBytes.length,
  );
  interactionInput.set(sourceBytes);
  const interaction = api.conduit_browser_form_admit_source_interaction(
    sourceBytes.length,
    BigInt(current),
  );
  if (interaction < 0) {
    const refusal = api.conduit_browser_form_output_len() > 0 ? readOutput(api) : null;
    runner.playStatus.failure(refusal?.message
      ? `The edit was refused · ${refusal.category}: ${refusal.message}`
      : `The edit was refused (${interaction}).`);
    return;
  }
  input.set(hostBytes);
  input.set(bootBytes, hostBytes.length);
  input.set(sourceBytes, hostBytes.length + bootBytes.length);
  const start = recursive ? api.conduit_browser_form_start_recursive : api.conduit_browser_form_start;
  const code = start(hostBytes.length, bootBytes.length, sourceBytes.length, BigInt(current));
  if (code < 0) {
    const refusal = api.conduit_browser_form_output_len() > 0 ? readOutput(api) : null;
    runner.playStatus.failure(refusal?.message
      ? `The Form was refused before Play · ${refusal.category}: ${refusal.message}`
      : `The Form was refused before Play (${code}).`);
    return;
  }
  let progress = readOutput(api);
  running = true;
  activeRunner = runner;
  setNavigationDisabled(true);
  runner.playStatus.ordinary("Playing through this browser Host…");
  runner.actionControls.render(true);
  try {
    const effects = new Map();
    let wake = null;
    const capacity = api.conduit_browser_form_pending_capacity();
    const perform = async (progress, signal) => {
      if (progress.effect_kind === "clock-observation") {
        const bytes = new Uint8Array(8);
        new DataView(bytes.buffer).setBigUint64(0, BigInt(Math.floor(performance.now() * 1000)), true);
        return bytes;
      } else if (progress.effect_kind === "timer") {
        runner.playStatus.ordinary(`Waiting for planned tick · ${progress.duration_millis} ms`);
        if (!await delay(progress.duration_millis, current, signal)) return;
      } else if (progress.effect_kind === "key-event") {
        runner.playStatus.ordinary("Waiting for one admitted keyboard transition…");
        const event = await humanInput.nextKeyboard();
        if (current !== generation) return;
        const encoded = event.canonical_bytes;
        return encoded;
      } else if (progress.effect_kind === "button-transition") {
        runner.querySelector(".input-button").hidden = false;
        runner.playStatus.ordinary("Waiting for one admitted button transition…");
        const event = await humanInput.nextButton();
        if (current !== generation) return;
        const encodedCode = api.conduit_tour_encode_button_transition(
          event.pressed ? 1 : 0,
          BigInt(event.sequence),
        );
        if (encodedCode < 0) throw new Error(`button transition encoding refused (${encodedCode})`);
        const encoded = new Uint8Array(
          api.memory.buffer,
          api.conduit_browser_form_output_ptr(),
          api.conduit_browser_form_output_len(),
        ).slice();
        return encoded;
      } else if (progress.effect_kind === "manifestation") {
        runner.querySelector(".morse").textContent =
          progress.text ?? renderMorse(progress.segments);
        renderIdentities(runner, progress);
        if (progress.presentation_kind === "presentation/indicator-state") {
          setIndicator(runner, progress.text === "true");
        } else {
          for (const segment of progress.segments) {
            if (current !== generation) return;
            setIndicator(runner, segment.level);
            if (!await delay(segment.units * progress.unit_millis, current)) return;
          }
          setIndicator(runner, false);
        }
        runner.playStatus.ordinary("Observed planned presentation; continuing the same Play…");
      } else {
        throw new Error(`unsupported browser Host effect ${progress.effect_kind}`);
      }
    };
    while (current === generation) {
      while (progress.effect_kind) {
        const effect = progress;
        const key = JSON.stringify([effect.active_play_id, effect.placement_id,
          effect.request_sequence ?? effect.observation_sequence]);
        if (effect.effect_kind === "cancel") {
          const pending = effects.get(key);
          if (!pending || pending.effect.effect_kind !== "timer") {
            throw new Error("kernel cancellation does not name a pending timer");
          }
          pending.controller.abort();
          effects.delete(key);
          const play = encoder.encode(effect.active_play_id);
          const placement = encoder.encode(effect.placement_id);
          const input = new Uint8Array(api.memory.buffer, api.conduit_browser_form_input_ptr(), play.length + placement.length);
          input.set(play);
          input.set(placement, play.length);
          const result = api.conduit_browser_form_acknowledge_cancellation(play.length, placement.length, effect.request_sequence);
          if (result < 0) throw new Error(`cancellation acknowledgement refused (${result})`);
          progress = readOutput(api);
          continue;
        }
        if (effects.has(key) || effects.size >= capacity) {
          throw new Error("browser Host effect identity or capacity violation");
        }
        const pending = { key, effect, ready: false, controller: new AbortController() };
        effects.set(key, pending);
        const settle = (result) => {
          Object.assign(pending, result, { ready: true });
          wake?.();
          wake = null;
        };
        perform(effect, pending.controller.signal).then(
          (output) => settle({ output }),
          (error) => settle({ error }),
        );
        const poll = api.conduit_browser_form_poll_effect();
        if (poll < 0) throw new Error(`effect poll refused (${poll})`);
        progress = readOutput(api);
      }
      if (progress.disposition !== "waiting") {
        if (effects.size) throw new Error("Play completed with platform effects pending");
        break;
      }
      if (!effects.size) throw new Error("Play awaits an absent platform effect");
      let completed = [...effects.values()].find((effect) => effect.ready);
      if (!completed) {
        await new Promise((resolve) => { wake = resolve; });
        if (current !== generation) return;
        completed = [...effects.values()].find((effect) => effect.ready);
      }
      if (current !== generation) return;
      effects.delete(completed.key);
      if (completed.error) throw completed.error;
      const { effect, output = new Uint8Array() } = completed;
      const play = encoder.encode(effect.active_play_id);
      const placement = encoder.encode(effect.placement_id);
      const total = play.length + placement.length + output.length;
      if (total > api.conduit_browser_form_input_capacity()) {
        throw new Error("effect completion exceeds the admitted input bound");
      }
      const bytes = new Uint8Array(api.memory.buffer, api.conduit_browser_form_input_ptr(), total);
      bytes.set(play);
      bytes.set(placement, play.length);
      bytes.set(output, play.length + placement.length);
      const completion = api.conduit_browser_form_complete_effect(
        play.length, placement.length, effect.request_sequence ?? effect.observation_sequence,
        output.length,
      );
      if (completion < 0) throw new Error(`effect completion refused (${completion})`);
      progress = readOutput(api);
    }
    if (current !== generation) return;
    runner.playStatus.success(progress.timer_completions > 0
      ? `Completed — one bounded Play, ${progress.timer_completions} planned ticks, ${progress.manifestation_completions} presentations.`
      : `Completed — one bounded Play, ${progress.manifestation_completions} planned manifestations.`);
    appendRunEvidence(runner, [
      ["Terminal Sign", progress.terminal_sign_id],
      ["Timer completions", String(progress.timer_completions)],
      ["Manifestation completions", String(progress.manifestation_completions)],
    ]);
    running = false;
    activeRunner = null;
    setNavigationDisabled(false);
    runner.actionControls.render(false);
  } catch (error) {
    if (current !== generation) return;
    generation += 1;
    cancelDelay();
    humanInput?.cancelPending();
    api.conduit_browser_form_cancel();
    setIndicator(runner, false);
    runner.playStatus.failure(error instanceof Error ? error.message : String(error));
    running = false;
    activeRunner = null;
    setNavigationDisabled(false);
    runner.actionControls.render(false);
  }
}

function stopListing(runner) {
  generation += 1;
  cancelDelay();
  humanInput?.cancelPending();
  if (running && runner.dataset.mode === "multi") cancelMultiSessions();
  else if (running) host.runtime.conduit_browser_form_cancel();
  running = false;
  activeRunner = null;
  setNavigationDisabled(false);
  setIndicator(runner, false);
  runner.actionControls.render(false);
  runner.playStatus.ordinary("Stopped. The Play was cancelled.");
}

function cancelMultiSessions() {
  activeMemoryLine?.cancel();
  activeMemoryLine = null;
  host?.runtime.conduit_tour_multi_cancel();
  peerHost?.runtime.conduit_tour_multi_cancel();
}

function readOutput(api) {
  const bytes = new Uint8Array(api.memory.buffer, api.conduit_browser_form_output_ptr(), api.conduit_browser_form_output_len());
  return JSON.parse(decoder.decode(bytes));
}

function readInventory(api) {
  const code = api.conduit_browser_form_inventory();
  if (code < 0) throw new Error(`browser Gear inventory refused (${code})`);
  return readOutput(api);
}

function renderInventory(inventory) {
  const copy = appendCopy();
  const slot = document.createElement("div");
  slot.dataset.applicationSlot = "tour-inventory";
  copy.append(slot);
  presentTourInventory(hostPresentation, inventory);
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
  runner.evidence.run(effect);
  const expansion = runner.querySelector(".exact-evidence .expansion");
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

function appendRunEvidence(runner, entries) {
  runner.evidence.appendRun(entries);
}

function delay(milliseconds, expectedGeneration, signal) {
  return new Promise((resolve) => {
    const finish = (accepted) => {
      clearTimeout(pending.timeout);
      activeDelays.delete(pending);
      signal?.removeEventListener("abort", abort);
      resolve(accepted);
    };
    const abort = () => finish(false);
    const pending = {
      resolve: finish,
      timeout: setTimeout(() => finish(expectedGeneration === generation), milliseconds),
    };
    activeDelays.add(pending);
    signal?.addEventListener("abort", abort, { once: true });
    if (signal?.aborted) abort();
  });
}

function cancelDelay() {
  for (const pending of activeDelays) {
    clearTimeout(pending.timeout);
    pending.resolve(false);
  }
  activeDelays.clear();
}
