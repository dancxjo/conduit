import "./patchbay-components.js";
import init, {
  explain_panel,
  panel_language_metadata,
  panel_source_metadata,
  parse_panel,
  patchbay_apply_transaction,
  patchbay_open_session,
} from "./conduit_web.js";
import { PatchbayReactFlowRenderer } from "./patchbay-renderer.js";
import { patchbayFeatures } from "./patchbay-features.js";
import { autoArrangeOperations } from "./patchbay-layout.js";
import { PatchbayWorkspaceController } from "./patchbay-workspace.js";
import {
  attachPanelSourceHighlighting,
  configurePanelLanguage,
  configurePanelSourceMetadata,
} from "./panel-highlighter.js";

const source = document.querySelector("#source");
const syncSourceHighlight = attachPanelSourceHighlighting(source);
const result = document.querySelector("#result");
const runButton = document.querySelector("#run");
const stopButton = document.querySelector("#stop");
const undoResetButton = document.querySelector("#undo-reset");
const arrangeButton = document.querySelector("#arrange");
const consoleBadge = document.querySelector("#console-status-badge");
const selectedNodeLabel = document.querySelector("#selected-node-label");
const moveLeftBtn = document.querySelector("#move-left");
const moveRightBtn = document.querySelector("#move-right");
const runnabilityState = document.querySelector("#runnability-state");
const executionStory = document.querySelector("#execution-story");
const scenarioSelect = document.querySelector("#scenario");
const timelinePosition = document.querySelector("#timeline-position");
const timelinePositionLabel = document.querySelector("#timeline-position-label");
const timelineLanes = document.querySelector("#timeline-lanes");
const timelineExplanation = document.querySelector("#timeline-explanation");
const timelineTableBody = document.querySelector("#timeline-table tbody");
const watchValue = document.querySelector("#watch-value");
const watchAccounting = document.querySelector("#watch-accounting");
const watchToggle = document.querySelector("#watch-toggle");
const freezeDisplay = document.querySelector("#freeze-display");
const displayFreezeStatus = document.querySelector("#display-freeze-status");
const watchObservationLead = document.querySelector("#watch-observation-lead");
const instrumentResult = document.querySelector("#instrument-result");
const instrumentResultText = document.querySelector("#instrument-result-text");
const liveFlowStatus = document.querySelector("#live-flow-status");
const liveFlowTableBody = document.querySelector("#live-flow-table tbody");
const workspace = document.querySelector("#workspace");
const workspaceKicker = document.querySelector("#workspace-kicker");
const readerContent = document.querySelector("#reader-content");
const bookCover = document.querySelector("#book-cover");
const readerSection = document.querySelector("#reader-section");
const directoryView = document.querySelector("#directory-view");
const narrativeBeforeLab = document.querySelector("#narrative-before-lab");
const narrativeAfterLab = document.querySelector("#narrative-after-lab");
const chapterOpening = document.querySelector("#chapter-opening");
const readerPager = document.querySelector(".reader-pager");
const expandLabButton = document.querySelector("#expand-lab");

const lessons = await (await fetch("../lessons/current.json", { cache: "no-store" })).json();
const book = await (await fetch("../book/current.json", { cache: "no-store" })).json();
const migrationLedger = await (
  await fetch("../book/migration.json", { cache: "no-store" })
).json();
const freshReaderStudy = await (
  await fetch("../book/fresh-reader-study.json", { cache: "no-store" })
).json();
const browserPlan = await (await fetch("./browser-plan.json", { cache: "no-store" })).json();
const referenceManifest = await (
  await fetch("../reference-panels/current.json", { cache: "no-store" })
).json();
if (referenceManifest.schema !== "conduit.tour-reference-panels") {
  throw new Error("unsupported Tour reference-panel manifest");
}
if (book.schema !== "conduit.tour-book" || book.schema_version !== 0) {
  throw new Error("unsupported Tour book manifest");
}
if (
  migrationLedger.schema !== "conduit.tour-migration-ledger" ||
  migrationLedger.schema_version !== 0
) {
  throw new Error("unsupported Tour migration ledger");
}
if (
  freshReaderStudy.schema !== "conduit.tour-fresh-reader-study" ||
  freshReaderStudy.schema_version !== 0
) {
  throw new Error("unsupported Tour fresh-reader study");
}
const projectArtifactSources = new Map(await Promise.all(
  book.projects.filter((project) => project.artifact.source_path).map(async (project) => {
    const response = await fetch(new URL(project.artifact.source_path, import.meta.url), {
      cache: "no-store",
    });
    if (!response.ok) {
      throw new Error(`project-artifact-fetch:${project.id}:${response.status}`);
    }
    return [project.id, await response.text()];
  }),
));
const referencePanels = await Promise.all(referenceManifest.panels.map(async (panel) => {
  const response = await fetch(new URL(panel.source_path, import.meta.url), {
    cache: "no-store",
  });
  if (!response.ok) {
    throw new Error(`reference-panel-fetch:${panel.id}:${response.status}`);
  }
  return { ...panel, source: await response.text() };
}));

async function fetchArtifact(artifact) {
  const url = new URL(artifact.path, import.meta.url);
  const response = await fetch(url, { cache: "no-store" });
  if (!response.ok) throw new Error(`artifact-fetch:${artifact.id}:${response.status}`);
  const bytes = await response.arrayBuffer();
  if (bytes.byteLength !== artifact.bytes) {
    throw new Error(`artifact-size:${artifact.id}`);
  }
  return { artifact, bytes, url };
}

async function sha256Hex(bytes) {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)]
    .map((value) => value.toString(16).padStart(2, "0"))
    .join("");
}

if (browserPlan.schema !== "conduit.tour-browser-plan") {
  throw new Error("unsupported Tour browser plan");
}
const adapterArtifact = browserPlan.artifacts.find(
  (artifact) => artifact.id === "browser-host-adapter",
);
const loadedAdapter = await fetchArtifact(adapterArtifact);
if (await sha256Hex(loadedAdapter.bytes) !== adapterArtifact.sha256) {
  throw new Error("browser host adapter integrity mismatch");
}
const {
  BrowserHostReason,
  DedicatedWorkerExecutionAdapter,
  Placement,
  observeBrowserHost,
  resolveBrowserPlacement,
  verifyExactArtifact,
} = await import(loadedAdapter.url);

const loadedArtifacts = new Map([[adapterArtifact.id, loadedAdapter]]);
for (const artifact of browserPlan.artifacts) {
  if (loadedArtifacts.has(artifact.id)) continue;
  const loaded = await fetchArtifact(artifact);
  const verified = await verifyExactArtifact(loaded.bytes, artifact.sha256);
  if (!verified.ok) {
    throw new Error(`${BrowserHostReason.ArtifactIntegrity}:${artifact.id}`);
  }
  loadedArtifacts.set(artifact.id, loaded);
}
await init({
  module_or_path: loadedArtifacts.get("conduit-web-wasm").bytes,
});
configurePanelLanguage(JSON.parse(panel_language_metadata()));
configurePanelSourceMetadata(panel_source_metadata);
syncSourceHighlight();

const placementFact = (id, available, lifetime, scheduling, transfer, terminalRisks) => ({
  id,
  available,
  lifetime,
  scheduling,
  transfer,
  limits: { queueBytes: browserPlan.bounds.maximum_message_bytes },
  terminalRisks,
});
const hostReport = observeBrowserHost({
  hostId: "conduit/tour-browser-host",
  observationId: browserPlan.observation_id,
  reporter: {
    realmId: "conduit/tour-static-realm",
    entityId: "conduit/tour-browser-workload",
    passportIdentity: "conduit/tour-browser-passport",
    statusObservation: {
      realmId: "conduit/tour-static-realm",
      entityId: "conduit/tour-browser-workload",
      passportIdentity: "conduit/tour-browser-passport",
      reporterIdentity: "conduit/tour-static-status-reporter",
      timeBasis: "conduit/tour-fixture-clock",
      observedAtTick: 9,
      validUntilTick: 100,
      status: "active",
    },
  },
  tick: 10,
  validUntilTick: 100,
  context: {
    secureContext: globalThis.isSecureContext,
    origin: globalThis.location.origin,
    crossOriginIsolated: globalThis.crossOriginIsolated,
  },
  placements: [
    placementFact(
      Placement.DedicatedWorker,
      typeof Worker === "function",
      "worker",
      "event-loop",
      "structured-clone",
      ["worker-death", "page-close"],
    ),
    placementFact(
      Placement.Wasm,
      typeof WebAssembly === "object",
      "worker",
      "placement-owned",
      "linear-memory",
      ["trap", "worker-death"],
    ),
  ],
  permissions: [],
  activation: false,
  resources: {
    queueBytes: browserPlan.bounds.maximum_message_bytes,
    pendingMessages: browserPlan.bounds.maximum_pending,
  },
});
if (hostReport.ok === false) {
  throw new Error(`${hostReport.code}:${hostReport.detail}`);
}

let current = lessons.lessons.find((lesson) => lesson.id === "book.origin-hidden-program")
  || lessons.lessons[0];
let pendingSourceEditFrame = null;
let acceptedSource = "";
let selectedNode = null;
let selectedCord = null;
let positions = {};
let patchbaySessionId = "";
let patchbaySourceRevision = 0;
let patchbayPresentationRevision = 0;
let patchbayView = null;
let activeAdapter = null;
let activeWorkerSessionId = null;
let activeWorkerRunIdentity = null;
let runEpoch = 0;
let liveWakeTimer = null;
let activeWatchControl = null;
let activeRunProjection = null;
let displayIsFrozen = false;
let deferredLivePresentation = null;
let deferredLiveDeltaCount = 0;
let liveEvidenceSequence = -1;
let liveFlowRows = [];
let topologyView = "logical";
const evidence = [];
let timelineRecords = [];
let timelineCursor = -1;
let timelineTimer = null;
const draftKey = (id) => `conduit-tour-draft/${id}`;
const recoveryKey = (id) => `conduit-tour-reset-recovery/${id}`;
const layoutKey = (id) => `conduit-tour-layout/${id}`;
const readingPositionKey = "conduit-tour-reader/0/reading-position";
const checkpointKey = (projectId) =>
  `conduit-tour-reader/0/project-checkpoint/${projectId}`;
const projectStateKey = (project) => project.artifact.state_key;
const projectRecoveryKey = (project) => `${project.artifact.state_key}/recovery`;
const bookSections = book.projects.flatMap((project) =>
  project.chapters.flatMap((chapter) =>
    chapter.sections.map((section) => ({ project, chapter, section })),
  ),
);
const sectionById = new Map(bookSections.map((entry) => [entry.section.id, entry]));
for (const entry of bookSections) {
  const lab = entry.section.blocks.find((block) => block.kind === "lab");
  const lesson = lessons.lessons.find((candidate) => candidate.id === lab?.lesson_id);
  if (!lab || !lesson) {
    throw new Error(`unresolved reader lab reference in ${entry.section.id}`);
  }
}
const migrationByLessonId = new Map(
  migrationLedger.entries.map((entry) => [entry.lesson_id, entry]),
);
const referenceLessonById = new Map(
  book.reference.lessons.map((lessonId) => {
    const lesson = lessons.lessons.find((candidate) => candidate.id === lessonId);
    if (!lesson) throw new Error(`unresolved Reference lesson ${lessonId}`);
    return [lessonId, lesson];
  }),
);
const retiredByLessonId = new Map(
  book.retired.lessons.map((entry) => {
    const lesson = lessons.lessons.find((candidate) => candidate.id === entry.lesson_id);
    if (!lesson || !sectionById.has(entry.replacement_section)) {
      throw new Error(`unresolved retired lesson ${entry.lesson_id}`);
    }
    return [entry.lesson_id, { ...entry, lesson }];
  }),
);
let activeReaderSection = null;
let activeReaderDestination = "cover";
let activeDirectoryKind = null;
const MIN_I32 = -2_147_483_648;
const MAX_I32 = 2_147_483_647;
const MAXIMUM_LAYOUT_OPERATIONS_PER_TRANSACTION = 32;
const LIVE_WATCH_PRESENTATION_INTERVAL_MS = 750;
const MAXIMUM_LIVE_FLOW_ROWS = 12;

const narrativeLabels = {
  invitation: "Invitation",
  need: "The need",
  idea: "The idea",
  action: "Try it",
  witness: "What to witness",
  explanation: "Why it works",
  reflection: "Reflect",
  "next-hook": "What comes next",
};

function setDestination(destination) {
  activeReaderDestination = destination;
  for (const kind of ["book", "reference", "cookbook"]) {
    const button = document.querySelector(`#show-${kind}`);
    const active = destination === kind || (destination === "cover" && kind === "book");
    button.classList.toggle("active", active);
    button.setAttribute("aria-pressed", String(active));
  }
  document.querySelector("#book-navigation").hidden =
    !["book", "cover"].includes(destination);
}

function updateRoute(parameters = {}, { replace = false, hash = "" } = {}) {
  const url = new URL(location.href);
  url.search = "";
  for (const [key, value] of Object.entries(parameters)) {
    if (value !== undefined && value !== null && value !== "") {
      url.searchParams.set(key, value);
    }
  }
  url.hash = hash;
  history[replace ? "replaceState" : "pushState"]({}, "", url);
}

function storedCheckpoint(projectId) {
  try {
    const value = JSON.parse(localStorage.getItem(checkpointKey(projectId)) || "[]");
    return Array.isArray(value) ? value : [];
  } catch {
    localStorage.removeItem(checkpointKey(projectId));
    return [];
  }
}

function recordCheckpoint(entry) {
  const visited = new Set(storedCheckpoint(entry.project.id));
  visited.add(entry.section.id);
  localStorage.setItem(checkpointKey(entry.project.id), JSON.stringify([...visited]));
  return visited;
}

function projectSections(project) {
  return project.chapters.flatMap((chapter) => chapter.sections);
}

function projectRevisions(project) {
  return [
    project.artifact.initial_revision,
    ...projectSections(project).map((section) => section.state.produces),
  ].filter((revision, index, revisions) => revisions.indexOf(revision) === index);
}

function initialProjectState(project) {
  return {
    revision: project.artifact.initial_revision,
    completed_sections: [],
  };
}

function storedProjectState(project) {
  try {
    const value = JSON.parse(
      localStorage.getItem(projectStateKey(project)) || "null",
    );
    if (
      !value ||
      !projectRevisions(project).includes(value.revision) ||
      !Array.isArray(value.completed_sections)
    ) {
      return initialProjectState(project);
    }
    return value;
  } catch {
    localStorage.removeItem(projectStateKey(project));
    return initialProjectState(project);
  }
}

function saveProjectState(project, state) {
  localStorage.setItem(projectStateKey(project), JSON.stringify(state));
}

function advanceProjectState(entry) {
  const state = storedProjectState(entry.project);
  const revisions = projectRevisions(entry.project);
  const producedRevision = entry.section.state.produces;
  if (revisions.indexOf(producedRevision) >= revisions.indexOf(state.revision)) {
    state.revision = producedRevision;
  }
  if (!state.completed_sections.includes(entry.section.id)) {
    state.completed_sections.push(entry.section.id);
  }
  saveProjectState(entry.project, state);
  renderProjectArtifact(entry);
  return state;
}

function projectLessonIds(project) {
  const artifactDraftId = project.artifact.source_path
    ? `project-artifact/${project.id}`
    : null;
  return [...new Set([artifactDraftId, ...projectSections(project).map((section) =>
    section.blocks.find((block) => block.kind === "lab")?.lesson_id
  )].filter(Boolean))];
}

function resetProjectState(entry) {
  if (current?.id) {
    localStorage.setItem(draftKey(current.id), source.value);
  }
  const drafts = Object.fromEntries(projectLessonIds(entry.project).flatMap((lessonId) => {
    const draft = localStorage.getItem(draftKey(lessonId));
    return draft === null ? [] : [[lessonId, draft]];
  }));
  localStorage.setItem(projectRecoveryKey(entry.project), JSON.stringify({
    state: storedProjectState(entry.project),
    checkpoints: storedCheckpoint(entry.project.id),
    drafts,
  }));
  saveProjectState(entry.project, initialProjectState(entry.project));
  localStorage.removeItem(checkpointKey(entry.project.id));
  for (const lessonId of projectLessonIds(entry.project)) {
    localStorage.removeItem(draftKey(lessonId));
    localStorage.removeItem(recoveryKey(lessonId));
  }
  const first = sectionById.get(projectSections(entry.project)[0].id);
  openReaderSection(first);
}

function recoverProjectState(entry) {
  try {
    const recovery = JSON.parse(
      localStorage.getItem(projectRecoveryKey(entry.project)) || "null",
    );
    if (!recovery?.state || !Array.isArray(recovery.checkpoints)) return;
    saveProjectState(entry.project, recovery.state);
    localStorage.setItem(
      checkpointKey(entry.project.id),
      JSON.stringify(recovery.checkpoints),
    );
    for (const [lessonId, draft] of Object.entries(recovery.drafts || {})) {
      if (projectLessonIds(entry.project).includes(lessonId)) {
        localStorage.setItem(draftKey(lessonId), draft);
      }
    }
    localStorage.removeItem(projectRecoveryKey(entry.project));
    openReaderSection(entry);
  } catch {
    localStorage.removeItem(projectRecoveryKey(entry.project));
  }
}

function renderProjectArtifact(entry) {
  const card = document.querySelector("#project-artifact");
  card.hidden = false;
  document.querySelector("#opening-result").textContent = entry.section.opening_result;
  document.querySelector("#artifact-id").textContent = entry.project.artifact.id;
  document.querySelector("#artifact-inherits").textContent =
    entry.section.state.inherits;
  document.querySelector("#artifact-produces").textContent =
    entry.section.state.produces;
  document.querySelector("#section-non-audio").textContent =
    entry.section.accessibility.non_audio;
  document.querySelector("#section-reduced-motion").textContent =
    entry.section.accessibility.reduced_motion;
  document.querySelector("#section-screen-reader").textContent =
    entry.section.accessibility.screen_reader;
  const state = storedProjectState(entry.project);
  const revisions = projectRevisions(entry.project);
  document.querySelector("#artifact-status").textContent =
    `Local project state: ${state.revision} · ${state.completed_sections.length} ` +
    `of ${projectSections(entry.project).length} sections carried forward. ` +
    "This is reader state, not a live-run claim.";
  document.querySelector("#reset-project").onclick = () => resetProjectState(entry);
  const recover = document.querySelector("#recover-project");
  recover.disabled = localStorage.getItem(projectRecoveryKey(entry.project)) === null;
  recover.onclick = () => recoverProjectState(entry);
  card.dataset.revision = state.revision;
  card.dataset.revisionIndex = String(revisions.indexOf(state.revision));
  instrumentResult.hidden = !(
    entry.project.id === "living-instrument" &&
    projectArtifactSources.has(entry.project.id)
  );
  if (instrumentResult && !instrumentResult.hidden) resetInstrumentResult();
}

function saveReadingPosition() {
  if (!activeReaderSection) return;
  localStorage.setItem(readingPositionKey, JSON.stringify({
    section_id: activeReaderSection.section.id,
    scroll_top: readerContent.scrollTop,
  }));
}

function storedReadingPosition() {
  try {
    const value = JSON.parse(localStorage.getItem(readingPositionKey) || "null");
    return value && sectionById.has(value.section_id) ? value : null;
  } catch {
    localStorage.removeItem(readingPositionKey);
    return null;
  }
}

function renderNarrativeBlock(block) {
  const element = document.createElement(block.kind === "reflection" ? "aside" : "section");
  element.id = `narrative-${block.id}`;
  element.className = `narrative-block narrative-${block.kind}`;
  const label = document.createElement("p");
  label.className = "book-kicker";
  label.textContent = narrativeLabels[block.kind];
  const heading = document.createElement("h3");
  heading.textContent = narrativeLabels[block.kind];
  heading.className = "sr-only";
  const body = document.createElement("p");
  body.textContent = block.body;
  element.append(label, heading, body);
  return element;
}

function setLabExpanded(expanded) {
  workspace.dataset.mode = expanded ? "expanded" : "compact";
  expandLabButton.setAttribute("aria-expanded", String(expanded));
  expandLabButton.textContent = expanded ? "Collapse inline lab" : "Expand full workspace";
  requestAnimationFrame(() => window.dispatchEvent(new Event("resize")));
}

function closeTechnicalDrawers() {
  document.querySelectorAll("#workspace details").forEach((details) => {
    details.open = false;
  });
}

function revealHashTarget() {
  const id = decodeURIComponent(location.hash.slice(1));
  if (!id) return;
  const target = document.getElementById(id);
  if (!target) return;
  if (target instanceof HTMLDetailsElement) target.open = true;
  target.scrollIntoView({ block: "start" });
}

function renderBookToc() {
  const toc = document.querySelector("#project-toc");
  toc.replaceChildren();
  for (const project of book.projects) {
    const projectGroup = document.createElement("section");
    projectGroup.className = "toc-project";
    const heading = document.createElement("h3");
    heading.textContent = project.title;
    projectGroup.append(heading);
    for (const chapter of project.chapters) {
      const details = document.createElement("details");
      details.className = "toc-chapter";
      details.open = true;
      const summary = document.createElement("summary");
      summary.textContent = `Chapter ${chapter.number}: ${chapter.title}`;
      const list = document.createElement("ol");
      list.className = "nav-list";
      for (const section of chapter.sections) {
        const item = document.createElement("li");
        const button = document.createElement("button");
        button.type = "button";
        button.textContent = section.title;
        button.dataset.sectionId = section.id;
        button.onclick = () => openReaderSection(sectionById.get(section.id));
        item.append(button);
        list.append(item);
      }
      details.append(summary, list);
      projectGroup.append(details);
    }
    toc.append(projectGroup);
  }
}

function renderCover({ updateHistory = true } = {}) {
  saveReadingPosition();
  void stopExactSession("reader-cover");
  activeReaderSection = null;
  setDestination("cover");
  bookCover.hidden = false;
  directoryView.hidden = true;
  readerSection.hidden = true;
  workspace.hidden = true;
  workspace.dataset.mode = "compact";
  document.querySelector("#cover-kicker").textContent = book.cover.kicker;
  document.querySelector("#cover-title").textContent = book.title;
  document.querySelector("#cover-subtitle").textContent = book.subtitle;
  document.querySelector("#cover-invitation").textContent = book.cover.invitation;
  const projects = document.querySelector("#cover-projects");
  projects.replaceChildren();
  for (const project of book.projects) {
    const card = document.createElement("article");
    const title = document.createElement("h3");
    title.textContent = project.title;
    const description = document.createElement("p");
    description.textContent = project.description;
    const count = document.createElement("p");
    count.className = "section-progress";
    const sections = projectSections(project);
    const state = storedProjectState(project);
    count.textContent = `${storedCheckpoint(project.id).length} of ${sections.length} sections visited · ` +
      `${state.completed_sections.length} carried forward · ${state.revision}`;
    const result = document.createElement("p");
    result.textContent = project.artifact.non_audio_result;
    card.append(title, description, result, count);
    projects.append(card);
  }
  readerContent.scrollTop = 0;
  if (updateHistory) updateRoute({});
}

function openReaderSection(entry, { updateHistory = true, scrollTop = 0 } = {}) {
  if (!entry) return;
  const preserveExpandedWorkspace = workspace.dataset.mode === "expanded";
  saveReadingPosition();
  activeReaderSection = entry;
  activeDirectoryKind = null;
  setDestination("book");
  bookCover.hidden = true;
  directoryView.hidden = true;
  readerSection.hidden = false;
  workspace.hidden = false;
  readerPager.hidden = false;
  chapterOpening.hidden = false;

  const allProjectSections = entry.project.chapters.flatMap((chapter) => chapter.sections);
  const visited = recordCheckpoint(entry);
  document.querySelector("#project-progress").textContent = entry.project.title;
  document.querySelector("#section-progress").textContent =
    `${visited.size} of ${allProjectSections.length} project sections visited`;
  document.querySelector("#reader-section-title").textContent = entry.section.title;
  document.querySelector("#reader-section-summary").textContent = entry.section.summary;
  document.querySelector("#chapter-number").textContent = entry.chapter.kind === "interlude"
    ? "Optional interlude"
    : `Chapter ${entry.chapter.number}`;
  document.querySelector("#chapter-opening-title").textContent = entry.chapter.title;
  document.querySelector("#chapter-description").textContent = entry.chapter.description;
  document.querySelector("#chapter-opening-copy").textContent = entry.chapter.opening;
  renderProjectArtifact(entry);

  const labIndex = entry.section.blocks.findIndex((block) => block.kind === "lab");
  narrativeBeforeLab.replaceChildren(
    ...entry.section.blocks.slice(0, labIndex).map(renderNarrativeBlock),
  );
  narrativeAfterLab.replaceChildren(
    ...entry.section.blocks.slice(labIndex + 1).map(renderNarrativeBlock),
  );
  const lab = entry.section.blocks[labIndex];
  const lesson = lessons.lessons.find((candidate) => candidate.id === lab.lesson_id);
  const artifactSource = projectArtifactSources.get(entry.project.id);
  const presentedLesson = artifactSource
    ? {
        ...lesson,
        id: `project-artifact/${entry.project.id}`,
        title: `${entry.project.title} — ${entry.section.title}`,
        objective: entry.section.summary,
        prose: `${entry.section.state.carry_forward} ${lesson.prose || ""}`.trim(),
        source: artifactSource,
        supporting_source: lesson.source,
        supporting_execution: lesson.execution || "run-to-completion",
        supporting_validation: lesson.validation,
        supporting_watch_target: lesson.watch_target,
        supporting_runnability: lesson.runnability,
        execution: entry.project.artifact.execution,
        validation: entry.project.artifact.validation,
        watch_target: entry.project.artifact.watch_target,
        runnability: entry.project.artifact.runnability || {
          state: "runnable",
          profile: "browser",
          proof: "browser-worker-exact-plan",
        },
      }
    : lesson;
  show(presentedLesson);
  setLabExpanded(lab.presentation === "expanded" || preserveExpandedWorkspace);
  closeTechnicalDrawers();

  const index = bookSections.indexOf(entry);
  const previous = document.querySelector("#previous-section");
  const next = document.querySelector("#next-section");
  previous.hidden = false;
  next.hidden = false;
  previous.disabled = index === 0;
  previous.textContent = index === 0
    ? "This is the first section"
    : `Previous: ${bookSections[index - 1].section.title}`;
  previous.setAttribute("aria-label", "Previous section");
  previous.onclick = () => openReaderSection(bookSections[index - 1]);
  next.disabled = false;
  next.textContent = index === bookSections.length - 1
    ? "Complete final project"
    : `Next: ${bookSections[index + 1].section.title}`;
  next.setAttribute("aria-label", "Next section");
  next.onclick = () => {
    advanceProjectState(entry);
    if (index < bookSections.length - 1) {
      openReaderSection(bookSections[index + 1]);
    }
  };
  const permalink = document.querySelector("#section-permalink");
  permalink.href = `?section=${encodeURIComponent(entry.section.id)}`;

  document.querySelectorAll("[data-section-id]").forEach((button) => {
    const active = button.dataset.sectionId === entry.section.id;
    button.classList.toggle("active", active);
    if (active) button.closest("details").open = true;
  });
  if (updateHistory) updateRoute({ section: entry.section.id });
  requestAnimationFrame(() => {
    readerContent.scrollTop = Math.max(0, Number(scrollTop) || 0);
    revealHashTarget();
  });
}

function directoryEntries(kind) {
  if (kind === "reference") {
    return [
      ...referencePanels.map((panel) => ({
        id: panel.id,
        title: panel.title,
        description: panel.objective || panel.prose || "Canonical checked panel",
        search: [panel.id, panel.title, panel.objective, panel.prose]
          .filter(Boolean).join(" "),
        target: panel,
        referenceKind: "panel",
      })),
      ...[...referenceLessonById.values()].map((lesson) => ({
        id: lesson.id,
        title: lesson.title,
        description: lesson.objective,
        search: [
          lesson.id,
          lesson.title,
          lesson.objective,
          lesson.prose,
          ...(lesson.vocabulary || []),
        ].filter(Boolean).join(" "),
        target: lesson,
        referenceKind: "lesson",
      })),
    ];
  }
  return book.cookbook.recipes.map((recipe) => {
    const lesson = lessons.lessons.find((candidate) => candidate.id === recipe.lesson_id);
    return {
      id: recipe.id,
      title: recipe.title,
      description: lesson.objective,
      search: [recipe.id, recipe.title, ...recipe.tags, lesson.title, lesson.objective].join(" "),
      target: lesson,
      recipe,
    };
  });
}

function renderDirectoryResults(kind, query = "") {
  const normalized = query.trim().toLocaleLowerCase();
  const results = document.querySelector("#directory-results");
  results.replaceChildren();
  for (const entry of directoryEntries(kind).filter((candidate) =>
    candidate.search.toLocaleLowerCase().includes(normalized))) {
    const item = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    const title = document.createElement("strong");
    title.textContent = entry.title;
    const description = document.createElement("span");
    description.textContent = entry.description;
    button.append(title, description);
    button.onclick = () => openDirectoryLab(kind, entry);
    item.append(button);
    results.append(item);
  }
}

function openDirectory(kind, { updateHistory = true, query = "" } = {}) {
  saveReadingPosition();
  void stopExactSession(`${kind}-directory`);
  activeReaderSection = null;
  activeDirectoryKind = kind;
  setDestination(kind);
  bookCover.hidden = true;
  readerSection.hidden = true;
  workspace.hidden = true;
  workspace.dataset.mode = "compact";
  directoryView.hidden = false;
  const descriptor = book[kind];
  document.querySelector("#directory-kicker").textContent = "Independent directory";
  document.querySelector("#directory-title").textContent = descriptor.title;
  document.querySelector("#directory-description").textContent = descriptor.description;
  const input = document.querySelector("#directory-query");
  input.value = query;
  input.oninput = () => renderDirectoryResults(kind, input.value);
  renderDirectoryResults(kind, query);
  readerContent.scrollTop = 0;
  if (updateHistory) updateRoute({ view: kind, q: query });
}

function openDirectoryLab(kind, entry, { updateHistory = true } = {}) {
  const preserveExpandedWorkspace = workspace.dataset.mode === "expanded";
  activeReaderSection = null;
  activeDirectoryKind = kind;
  setDestination(kind);
  bookCover.hidden = true;
  directoryView.hidden = true;
  readerSection.hidden = false;
  workspace.hidden = false;
  document.querySelector("#project-artifact").hidden = true;
  chapterOpening.hidden = true;
  readerPager.hidden = false;
  document.querySelector("#project-progress").textContent = kind === "reference"
    ? `Reference ${entry.referenceKind || "panel"}`
    : "Cookbook recipe";
  document.querySelector("#section-progress").textContent = "Outside sequential book progress";
  document.querySelector("#reader-section-title").textContent = entry.title;
  document.querySelector("#reader-section-summary").textContent = entry.description;
  narrativeBeforeLab.replaceChildren(renderNarrativeBlock({
    id: "directory-action",
    kind: "action",
    body: kind === "reference"
      ? "Inspect this canonical checked panel without changing your place in the book."
      : "Run or inspect this exact recipe without adding it to Previous/Next navigation.",
  }));
  narrativeAfterLab.replaceChildren();
  show(entry.target);
  setLabExpanded(preserveExpandedWorkspace);
  closeTechnicalDrawers();
  const previous = document.querySelector("#previous-section");
  const next = document.querySelector("#next-section");
  previous.disabled = false;
  previous.textContent = `Back to ${book[kind].title}`;
  previous.onclick = () => openDirectory(kind);
  next.hidden = true;
  const permalink = document.querySelector("#section-permalink");
  permalink.href = `?view=${kind}&item=${encodeURIComponent(entry.id)}`;
  if (updateHistory) updateRoute({ view: kind, item: entry.id });
  readerContent.scrollTop = 0;
}

function openRetiredLesson(entry, { updateHistory = true } = {}) {
  const preserveExpandedWorkspace = workspace.dataset.mode === "expanded";
  saveReadingPosition();
  activeReaderSection = null;
  activeDirectoryKind = "retired";
  setDestination("reference");
  bookCover.hidden = true;
  directoryView.hidden = true;
  readerSection.hidden = false;
  workspace.hidden = false;
  document.querySelector("#project-artifact").hidden = true;
  chapterOpening.hidden = true;
  readerPager.hidden = false;
  document.querySelector("#project-progress").textContent = "Retired opening fixture";
  document.querySelector("#section-progress").textContent =
    "Outside sequential book progress · exact fixture retained";
  document.querySelector("#reader-section-title").textContent =
    `Retired: ${entry.lesson.title}`;
  document.querySelector("#reader-section-summary").textContent = entry.reason;
  narrativeBeforeLab.replaceChildren(
    renderNarrativeBlock({
      id: "retired-need",
      kind: "need",
      body: entry.reason,
    }),
    renderNarrativeBlock({
      id: "retired-action",
      kind: "action",
      body: "Inspect the retained production fixture without returning it to the curriculum.",
    }),
  );
  narrativeAfterLab.replaceChildren(renderNarrativeBlock({
    id: "retired-explanation",
    kind: "explanation",
    body: "Git history is not a second Tour schema. This current route keeps the useful exact fixture and points reading progress to its project-driven replacement.",
  }));
  show(entry.lesson);
  setLabExpanded(preserveExpandedWorkspace);
  closeTechnicalDrawers();
  const replacement = sectionById.get(entry.replacement_section);
  const previous = document.querySelector("#previous-section");
  const next = document.querySelector("#next-section");
  previous.disabled = false;
  previous.hidden = false;
  previous.textContent = `Continue to replacement: ${replacement.section.title}`;
  previous.onclick = () => openReaderSection(replacement);
  next.hidden = true;
  document.querySelector("#section-permalink").href =
    `?lesson=${encodeURIComponent(entry.lesson_id)}`;
  if (updateHistory) updateRoute({ lesson: entry.lesson_id });
  readerContent.scrollTop = 0;
}

function clearLiveWakeTimer() {
  if (liveWakeTimer !== null) {
    clearTimeout(liveWakeTimer);
    liveWakeTimer = null;
  }
}

function resetWatchPresentation(message = "No Watch is attached.") {
  disableWatchControl();
  watchValue.textContent = message;
  watchAccounting.textContent = "No Watch accounting yet.";
  activeRunProjection = null;
  displayIsFrozen = false;
  deferredLivePresentation = null;
  deferredLiveDeltaCount = 0;
  liveEvidenceSequence = -1;
  liveFlowRows = [];
  freezeDisplay.disabled = true;
  freezeDisplay.setAttribute("aria-pressed", "false");
  freezeDisplay.textContent = "Freeze Display (F)";
  displayFreezeStatus.textContent = "Display follows authoritative live deltas.";
  liveFlowStatus.textContent = "No authoritative live-flow delta yet.";
  liveFlowTableBody.replaceChildren();
  if (instrumentResult && !instrumentResult.hidden) resetInstrumentResult();
}

function resetInstrumentResult(message =
  "Ready. Start the exact run to produce the first beat; audio remains off.") {
  if (!instrumentResult || !instrumentResultText) return;
  instrumentResult.dataset.state = "ready";
  instrumentResult.dataset.phase = "";
  instrumentResult.dataset.tick = "";
  instrumentResult.dataset.accent = "false";
  instrumentResult.style.setProperty("--instrument-level", "0%");
  instrumentResultText.textContent = message;
}

function renderInstrumentWatch(record) {
  if (!instrumentResult || !instrumentResultText || instrumentResult.hidden) return;
  const text = record?.material?.kind === "preview"
    ? record.material.text
    : null;
  const scoped = text?.match(/^tick=(\d+) level=(\d+)\s*$/);
  const beat = Number.parseInt(scoped?.[1] ?? text ?? "", 10);
  if (!Number.isSafeInteger(beat) || beat < 0) {
    instrumentResult.dataset.state = "observed";
    instrumentResultText.textContent = text
      ? `Exact Watch value: ${text.trim() || "empty"}. Audio remains off.`
      : "The exact Watch has not produced a numeric beat yet. Audio remains off.";
    return;
  }
  const phase = beat % 8;
  const exactLevel = scoped ? Number.parseInt(scoped[2], 10) : null;
  const intensity = Number.isSafeInteger(exactLevel)
    ? Math.min(4, Math.round(exactLevel / 256))
    : (phase <= 4 ? phase : 8 - phase);
  const accent = phase === 0 || phase === 4;
  instrumentResult.dataset.state = "live";
  instrumentResult.dataset.tick = String(beat);
  instrumentResult.dataset.phase = String(phase);
  instrumentResult.dataset.accent = String(accent);
  instrumentResult.style.setProperty("--instrument-level", `${25 + intensity * 18.75}%`);
  instrumentResultText.textContent =
    `Exact Watch beat ${beat}. Step ${phase + 1} of 8; ` +
    `${accent ? "accent on" : "accent off"}; intensity ${intensity} of 4. ` +
    "Audio remains off.";
}

function disableWatchControl() {
  activeWatchControl = null;
  watchToggle.disabled = true;
  watchToggle.textContent = "Attach Watch (W)";
  watchToggle.setAttribute("aria-pressed", "false");
  watchObservationLead.dataset.attached = "false";
  watchObservationLead.textContent =
    "Observation lead detached. This dashed lead is presentation only and is never a graph cord.";
  renderStructuredTopology();
}

function setWatchControl(control) {
  activeWatchControl = control;
  watchToggle.disabled = Boolean(control.pending);
  watchToggle.textContent = control.attached ? "Remove Watch (W)" : "Attach Watch (W)";
  watchToggle.setAttribute("aria-pressed", String(control.attached));
  watchObservationLead.dataset.attached = String(control.attached);
  watchObservationLead.textContent = control.attached
    ? `Presentation-only observation lead attached to ${control.subjectLabel || control.watchId}; it cannot carry demand or pressure.`
    : `Observation lead detached from ${control.subjectLabel || control.watchId}; the exact dataflow continues.`;
  renderStructuredTopology();
}

function setWatchControlBusy(control, busy) {
  if (!control || activeWatchControl !== control) return;
  control.pending = busy;
  watchToggle.disabled = busy;
  for (const button of document.querySelectorAll(".structured-watch-button")) {
    button.disabled = busy;
  }
}

function setLivePresentationActive() {
  freezeDisplay.disabled = false;
  displayFreezeStatus.textContent = displayIsFrozen
    ? "Display is frozen; the exact executor and bounded observation cursor continue."
    : "Display follows authoritative live deltas.";
}

function authoritativeEvidenceDelta(records) {
  const delta = (records || []).filter((record) =>
    Number.isSafeInteger(record.sequence) && record.sequence > liveEvidenceSequence
  );
  for (const record of delta) {
    liveEvidenceSequence = Math.max(liveEvidenceSequence, record.sequence);
  }
  return delta;
}

function renderLiveFlowRows(projection, delta, watchRecord, runtime = projection) {
  if (delta.length > 0) {
    liveFlowRows.push(...delta);
    if (liveFlowRows.length > MAXIMUM_LIVE_FLOW_ROWS) {
      liveFlowRows.splice(0, liveFlowRows.length - MAXIMUM_LIVE_FLOW_ROWS);
    }
  }
  liveFlowTableBody.replaceChildren();
  for (const record of liveFlowRows) {
    const row = document.createElement("tr");
    row.dataset.sequence = String(record.sequence);
    const pressure = record.pressure
      ? `${record.pressure}; ${record.occupancy_items} items / ${record.occupancy_bytes} bytes`
      : "—";
    for (const value of [
      record.sequence,
      record.tick,
      `${record.subject_kind}: ${record.subject_id}`,
      record.event_detail ? `${record.event_kind}: ${record.event_detail}` : record.event_kind,
      pressure,
    ]) {
      const cell = document.createElement("td");
      cell.textContent = String(value);
      row.append(cell);
    }
    liveFlowTableBody.append(row);
  }
  const cordDeltas = delta.filter((record) => record.subject_kind === "cord");
  const latest = cordDeltas.at(-1) || delta.at(-1);
  const dropped = runtime.evidence_store?.dropped_events ?? 0;
  liveFlowStatus.textContent = latest
    ? `Batch rate: ${delta.length} authoritative event${delta.length === 1 ? "" : "s"} per bounded presentation update; ` +
      `${cordDeltas.length} cord event${cordDeltas.length === 1 ? "" : "s"}. ` +
      `Latest: ${latest.subject_kind} ${latest.subject_id}, ${latest.event_kind}, ` +
      `${latest.occupancy_items} items / ${latest.occupancy_bytes} bytes. ` +
      `Rolling evidence gaps: ${dropped}. Watch cursor: ${watchRecord?.cursor ?? "detached"}.`
    : `No new authoritative event in this update. Rolling evidence gaps: ${dropped}.`;
  patchbayRenderer?.presentLiveEvidence(patchbayView || projection, delta, watchRecord);
}

function applyLiveRunProjection(projection) {
  if (!patchbayView) return;
  patchbayView = {
    ...patchbayView,
    run: projection.run,
    evidence: projection.evidence,
  };
  evidence.splice(0, evidence.length, ...projection.evidence);
  document.querySelector("#evidence").textContent = JSON.stringify(evidence, null, 2);
  patchbayRenderer?.updateRunPresentation(patchbayView);
}

function presentContinuousUpdate(presentation) {
  activeRunProjection = presentation.projection;
  if (displayIsFrozen) {
    deferredLivePresentation = presentation;
    deferredLiveDeltaCount += presentation.delta.length;
    displayFreezeStatus.textContent =
      `Display frozen; ${deferredLiveDeltaCount} authoritative delta${deferredLiveDeltaCount === 1 ? "" : "s"} deferred while the exact executor remains live.`;
    return;
  }
  applyLiveRunProjection(presentation.projection);
  if (presentation.watched) {
    renderLatestWatch(presentation.watched, presentation.result);
  } else if (presentation.control) {
    renderDetachedWatch(presentation.result, presentation.control);
  }
  renderExactResultTimeline(presentation.result);
  renderLiveFlowRows(
    presentation.projection,
    presentation.delta,
    presentation.watched?.records?.at(-1),
    presentation.result,
  );
  result.textContent = presentation.message;
}

function toggleDisplayFreeze() {
  if (freezeDisplay.disabled) return;
  displayIsFrozen = !displayIsFrozen;
  freezeDisplay.setAttribute("aria-pressed", String(displayIsFrozen));
  freezeDisplay.textContent = displayIsFrozen ? "Resume Display (F)" : "Freeze Display (F)";
  if (displayIsFrozen) {
    displayFreezeStatus.textContent =
      "Display frozen; the exact executor and bounded observation cursor continue.";
    return;
  }
  const deferred = deferredLivePresentation;
  deferredLivePresentation = null;
  deferredLiveDeltaCount = 0;
  displayFreezeStatus.textContent = "Display resumed at the latest bounded authoritative state.";
  if (deferred) presentContinuousUpdate(deferred);
}

async function toggleWatch() {
  const control = activeWatchControl;
  if (!control || watchToggle.disabled) return;
  const wasAttached = control.attached;
  const nextAttached = !wasAttached;
  // Stop issuing observation reads before the detach request reaches the
  // worker. The ticker pump remains independent and continues below.
  if (wasAttached) control.attached = false;
  setWatchControlBusy(control, true);
  watchToggle.setAttribute("aria-pressed", String(control.attached));
  const operation = wasAttached
    ? "patchbay-detach-exact-watch"
    : "patchbay-attach-exact-watch";
  const response = await control.adapter.request(operation, {
    sessionId: control.sessionId,
    ...control.runIdentity,
    operatorId: control.operatorId,
    watchId: control.watchId,
  });
  if (activeWatchControl !== control) return;
  if (!response.ok || !response.value?.ok) {
    control.attached = wasAttached;
    control.pending = false;
    setWatchControl(control);
    result.textContent = response.value?.diagnostic || response.code || "Watch control failed";
    return;
  }
  control.attached = nextAttached;
  control.pending = false;
  setWatchControl(control);
  if (!control.attached) {
    watchValue.textContent = "Watch detached; the exact ticker continues without observation pressure.";
    if (instrumentResult && instrumentResultText && !instrumentResult.hidden) {
      instrumentResult.dataset.state = "detached";
      instrumentResultText.textContent =
        "Watch detached. The exact ticker continues, but this presentation no longer receives beat values; audio remains off.";
    }
  }
  recordEvidence({
    kind: control.attached ? "watch-attached" : "watch-detached",
    lesson: current.id,
    watch_id: control.watchId,
  });
}

function renderLatestWatch(batch, run) {
  const record = batch?.records?.at(-1);
  const text = record?.material?.kind === "preview"
    ? record.material.text
    : null;
  if (record) {
    watchValue.textContent = text ?? record.material?.kind ?? "Observed non-preview value.";
    renderInstrumentWatch(record);
  }
  watchAccounting.textContent = JSON.stringify({
    run_id: run.run_id,
    plan_identity: run.plan_identity,
    source_semantic_hash: run.source_semantic_hash,
    state: run.state,
    next_timer_deadline: run.next_timer_deadline,
    watch_id: batch.watch_id,
    retention: "latest",
    cursor: batch.next_cursor,
    representation: record?.representation,
    sensitivity: record?.sensitivity,
    timestamp: {
      tick: record?.tick,
      time_basis: record?.time_basis,
      uncertainty_ticks: record?.clock_uncertainty_ticks,
      value_timestamps: record?.value_timestamps || [],
    },
    renderer: record?.material?.renderer || { status: record?.material?.kind || "absent" },
    material_kind: record?.material?.kind || "absent",
    truncated: Boolean(record?.truncated),
    recent_history: batch.records.slice(-1).map((item) => ({
      cursor: item.cursor,
      tick: item.tick,
      material_kind: item.material?.kind,
      truncated: Boolean(item.truncated),
    })),
    gap_before: record?.gap_before ?? 0,
    value_storage: run.value_storage,
    evidence_store: run.evidence_store,
  }, null, 2);
}

function renderDetachedWatch(run, control) {
  watchAccounting.textContent = JSON.stringify({
    run_id: run.run_id,
    plan_identity: run.plan_identity,
    source_semantic_hash: run.source_semantic_hash,
    state: run.state,
    next_timer_deadline: run.next_timer_deadline,
    watch_id: control.watchId,
    attached: false,
    retention: "latest",
    cursor: control.cursor,
    representation: { id: "std/text" },
    sensitivity: "public",
    value_storage: run.value_storage,
    evidence_store: run.evidence_store,
  }, null, 2);
}

function validPosition(position) {
  return position &&
    Number.isInteger(position.x) &&
    Number.isInteger(position.y) &&
    position.x >= MIN_I32 &&
    position.x <= MAX_I32 &&
    position.y >= MIN_I32 &&
    position.y <= MAX_I32;
}

function rememberLayout(lessonId, nodePositions, view) {
  const movableNodeIds = new Set(
    (view.topology?.logical_nodes || []).map((node) => node.id),
  );
  const boundedPositions = {};
  for (const nodeId of movableNodeIds) {
    const position = nodePositions[nodeId];
    if (validPosition(position)) boundedPositions[nodeId] = position;
  }
  localStorage.setItem(layoutKey(lessonId), JSON.stringify(boundedPositions));
}

function rememberedLayoutOperations(lessonId, view) {
  let storedPositions;
  try {
    storedPositions = JSON.parse(localStorage.getItem(layoutKey(lessonId)) || "{}");
  } catch {
    localStorage.removeItem(layoutKey(lessonId));
    return [];
  }
  if (!storedPositions || typeof storedPositions !== "object" ||
      Array.isArray(storedPositions)) {
    localStorage.removeItem(layoutKey(lessonId));
    return [];
  }

  const movableNodeIds = new Set(
    (view.topology?.logical_nodes || []).map((node) => node.id),
  );
  const maximumNodes = Math.min(
    view.bounds?.maximum_nodes || 0,
    movableNodeIds.size,
  );
  return Object.entries(storedPositions)
    .filter(([nodeId, position]) =>
      movableNodeIds.has(nodeId) && validPosition(position)
    )
    .slice(0, maximumNodes)
    .map(([nodeId, position]) => ({
      MoveNode: {
        node_id: nodeId,
        position,
      },
    }));
}

// Initialize React Flow Patchbay Renderer
let patchbayRenderer = null;
let workspaceController = null;
const cyContainer = document.getElementById("cy");
document.querySelector(".node-controls").hidden =
  !patchbayFeatures.legacyLinePlacement;
if (cyContainer) {
  patchbayRenderer = new PatchbayReactFlowRenderer(cyContainer, {
    onTransaction: (operation, options) => applyPatchbayOperations(
      Array.isArray(operation) ? operation : [operation],
      options,
    ),
    onNodeSelect: (nodeId) => {
      selectNode(nodeId);
    },
    onCordSelect: (cordId) => {
      selectCord(cordId);
    },
    onPortSelect: (nodeId, port) => {
      selectPort(nodeId, port);
    },
    onCordWatch: (cordId) => toggleWatchForSubject({ cordId }),
    onPortWatch: (nodeId, port) => toggleWatchForSubject({ nodeId, port }),
    onSelectionClear: () => {
      clearTopologySelection();
    },
    onNotification: (msg) => {
      result.textContent = msg;
    }
  });
  patchbayRenderer.init();
  workspaceController = new PatchbayWorkspaceController({
    canvasCard: document.querySelector(".canvas-card"),
    sourceCard: document.querySelector(".source-card"),
    actionBar: document.querySelector(".primary-actions"),
    consoleCard: document.querySelector(".console-card"),
    source,
    renderer: patchbayRenderer,
    getContext: () => ({
      documentName: current?.id || "untitled.panel",
      revision: patchbayView?.source?.revision || 0,
      identity: patchbayView?.source?.identity,
      dirty: Boolean(current && source.value !== current.source),
      diagnostics: patchbayView?.diagnostics?.length || 0,
    }),
  });
  workspaceController.init();
}

function updateCytoscapeGraph() {
  syncDiagnosticSourceHighlights();
  if (patchbayRenderer) {
    patchbayRenderer.setViewModel(patchbayView, current.id, topologyView);
  }
  renderStructuredTopology();
  renderDiagnosticConsole();
  workspaceController?.updateStatus();
}

function syncDiagnosticSourceHighlights() {
  const projectionIsCurrent = patchbayView &&
    patchbayView.source.revision === patchbaySourceRevision &&
    patchbayView.source.source === source.value;
  const ranges = projectionIsCurrent
    ? (patchbayView.diagnostics || [])
      .map((diagnostic) => diagnostic.primary_range)
      .filter((range) => range?.source_revision === patchbaySourceRevision)
    : [];
  source.setSourceDiagnosticRanges?.(ranges);
}

function renderDiagnosticConsole() {
  const consoleList = document.querySelector("#diagnostic-console");
  if (!consoleList) return;
  consoleList.replaceChildren();
  for (const diagnostic of patchbayView?.diagnostics || []) {
    const item = document.createElement("li");
    item.dataset.diagnosticId = diagnostic.id;
    item.dataset.diagnosticState = diagnostic.state;
    const button = document.createElement("button");
    button.type = "button";
    button.className = `diagnostic-console-button diagnostic-${diagnostic.state}`;
    button.textContent = `${diagnostic.code}: ${diagnostic.explanation}`;
    button.setAttribute(
      "aria-label",
      `${diagnostic.severity} ${diagnostic.code}; ${diagnostic.explanation}`,
    );
    button.onclick = () => selectDiagnostic(diagnostic);
    item.append(button);
    consoleList.append(item);
  }
}

function watchAdmissionForSubject({ cordId = null, nodeId = null, port = null }) {
  const admissions = activeRunProjection?.plan?.watch_admissions ||
    patchbayView?.plan?.watch_admissions || [];
  const direct = admissions.find((admission) =>
    (cordId && admission.subject_kind === "cord" && admission.cord === cordId) ||
    (nodeId && port && admission.subject_kind === "node-port" &&
      admission.node === nodeId && admission.port === port.id)
  );
  if (direct || !nodeId || !port) return direct;
  const connectedCord = (patchbayView?.topology?.cords || []).find((cord) =>
    (cord.from_node === nodeId && cord.from_port === port.id) ||
    (cord.to_node === nodeId && cord.to_port === port.id)
  );
  return connectedCord
    ? admissions.find((admission) =>
        admission.subject_kind === "cord" && admission.cord === connectedCord.id
      )
    : null;
}

function toggleWatchForSubject(subject) {
  const admission = watchAdmissionForSubject(subject);
  if (!admission) {
    result.textContent = "No exact-plan Watch admission exists for this selected subject.";
    return;
  }
  if (activeWatchControl?.watchId !== admission.id) {
    result.textContent =
      `Watch ${admission.id} is admitted but is not the active bounded instrument control.`;
    return;
  }
  void toggleWatch();
}

function watchSubjectButton(admission, label) {
  if (!admission) return null;
  const button = document.createElement("button");
  button.type = "button";
  button.className = "structured-watch-button";
  const attached = activeWatchControl?.watchId === admission.id && activeWatchControl.attached;
  button.textContent = attached ? `Remove Watch from ${label}` : `Watch ${label}`;
  button.setAttribute("aria-label", button.textContent);
  button.onclick = () => toggleWatchForSubject({
    cordId: admission.cord,
    nodeId: admission.node,
    port: admission.port ? { id: admission.port } : null,
  });
  return button;
}

function selectDiagnostic(diagnostic) {
  const cord = diagnostic.targets.find((target) => target.kind === "cord");
  const node = diagnostic.targets.find((target) => target.kind === "node");
  if (cord) {
    selectCord(cord.id);
  } else if (node) {
    selectNode(node.id);
  } else {
    selectSourceRange(
      "diagnostic",
      diagnostic.id,
      diagnostic.primary_range,
    );
  }
  result.textContent =
    `${diagnostic.code}: ${diagnostic.message}\n${diagnostic.explanation}`;
}

function plannedNodesForPresentation(view = patchbayView) {
  return (view?.topology?.planned_realization?.nodes || []).map((node) => ({
    id: node.instance,
    contract_id: node.binding.contract_id,
    source_range: node.source_origin_range,
    inputs: node.inputs || [],
    outputs: node.outputs || [],
    logical_origin: node.logical_origin,
    planned_binding: node.binding,
  }));
}

function projectedNodesForTopologyView() {
  return topologyView === "logical"
    ? patchbayView?.topology?.logical_nodes || []
    : plannedNodesForPresentation();
}

function projectedCordsForTopologyView() {
  return topologyView === "logical"
    ? patchbayView?.topology?.cords || []
    : patchbayView?.topology?.planned_realization?.cords || [];
}

function renderStructuredTopology() {
  const portList = document.querySelector("#panel-port-list");
  const connectionList = document.querySelector("#panel-connection-list");
  if (!portList || !connectionList) return;
  portList.replaceChildren();
  connectionList.replaceChildren();
  const nodes = projectedNodesForTopologyView();
  for (const node of nodes) {
    for (const port of [...node.inputs, ...node.outputs]) {
      const item = document.createElement("li");
      item.dataset.semanticPath = port.semantic_path;
      item.dataset.portDirection =
        port.direction === "input" ? "receiving" : "outgoing";
      const button = document.createElement("button");
      button.type = "button";
      button.className = "structured-topology-button";
      button.textContent =
        `${node.id}: ${port.display_label} — ${port.type_id}; ` +
        `${port.delivery}; ${port.connections}`;
      button.setAttribute(
        "aria-label",
        `${node.id}, ${port.accessible_label}, type ${port.type_id}, ` +
        `${port.delivery}, ${port.connections}`,
      );
      button.onclick = () => selectPort(node.id, port);
      item.append(button);
      const admission = watchAdmissionForSubject({ nodeId: node.id, port });
      const watchButton = watchSubjectButton(admission, `${node.id}.${port.id}`);
      if (watchButton) item.append(watchButton);
      portList.append(item);
    }
  }
  for (const cord of projectedCordsForTopologyView()) {
    const diagnostic = topologyView === "logical"
      ? (patchbayView?.diagnostics || []).find((candidate) =>
          candidate.targets.some(
            (target) => target.kind === "cord" && target.id === cord.id,
          )
        )
      : null;
    const item = document.createElement("li");
    item.dataset.fromPortPath = cord.from_port_path;
    item.dataset.toPortPath = cord.to_port_path;
    const button = document.createElement("button");
    button.type = "button";
    button.className = "structured-topology-button";
    const from = `${cord.from_node || "unfinished"}.${cord.from_port || "…"}`;
    const to = `${cord.to_node || "unfinished"}.${cord.to_port || "…"}`;
    button.textContent =
      `${from} > → > ${to} — ${diagnostic?.code || "valid"}; ` +
      `${diagnostic?.explanation || `${cord.value_type}; ${cord.pressure}`}`;
    button.setAttribute(
      "aria-label",
      `${from}, ${topologyView === "logical" ? "authored" : "planned"} source endpoint, ` +
      `to ${to}, ${topologyView === "logical" ? "authored" : "planned"} destination endpoint; ` +
      `${diagnostic?.code || "valid"}; ` +
      `${diagnostic?.explanation || `${cord.value_type}; ${cord.pressure}`}`,
    );
    button.onclick = () => selectCord(cord.id);
    item.append(button);
    const admission = watchAdmissionForSubject({ cordId: cord.id });
    const watchButton = watchSubjectButton(admission, `cord ${cord.id}`);
    if (watchButton) item.append(watchButton);
    connectionList.append(item);
  }
}

function openPatchbaySession() {
  patchbaySessionId = `tour/${current.id}`;
  const opened = JSON.parse(patchbay_open_session(patchbaySessionId, acceptedSource));
  if (!opened.ok) {
    patchbayView = null;
    updateCytoscapeGraph();
    result.textContent = opened.diagnostic;
    return false;
  }
  patchbayView = opened.view;
  patchbaySourceRevision = opened.view.source.revision;
  patchbayPresentationRevision = opened.view.presentation.revision;
  positions = opened.view.presentation.node_positions;
  const rememberedOperations = rememberedLayoutOperations(current.id, opened.view);
  if (rememberedOperations.length > 0) {
    for (let offset = 0; offset < rememberedOperations.length;
      offset += MAXIMUM_LAYOUT_OPERATIONS_PER_TRANSACTION) {
      const restored = applyPatchbayOperations(
        rememberedOperations.slice(
          offset,
          offset + MAXIMUM_LAYOUT_OPERATIONS_PER_TRANSACTION,
        ),
      );
      if (!restored.ok) {
        localStorage.removeItem(layoutKey(current.id));
        updateCytoscapeGraph();
        break;
      }
    }
    return true;
  }
  updateCytoscapeGraph();
  autoArrangePatchbay();
  return true;
}

function autoArrangePatchbay() {
  if (!patchbayView) return false;
  const operations = autoArrangeOperations(patchbayView, topologyView);
  for (let offset = 0; offset < operations.length;
    offset += MAXIMUM_LAYOUT_OPERATIONS_PER_TRANSACTION) {
    const arranged = applyPatchbayOperations(
      operations.slice(
        offset,
        offset + MAXIMUM_LAYOUT_OPERATIONS_PER_TRANSACTION,
      ),
      { skipRender: true },
    );
    if (!arranged.ok) return false;
  }
  updateCytoscapeGraph();
  if (cyContainer) cyContainer.dataset.layoutAlgorithm = "layered";
  result.textContent = operations.length === 0
    ? "No topology items were available to arrange."
    : `Arranged ${operations.length} topology item${operations.length === 1 ? "" : "s"} by dataflow.`;
  return true;
}

function applyPatchbayOperations(operations, options = {}) {
  const request = {
    protocol_version: 0,
    document_id: patchbaySessionId,
    expected_source_revision: patchbaySourceRevision,
    expected_presentation_revision: patchbayPresentationRevision,
    operations
  };
  const transaction = JSON.parse(
    patchbay_apply_transaction(patchbaySessionId, JSON.stringify(request)),
  );
  if (!transaction.ok) {
    result.textContent = transaction.diagnostic;
    return transaction;
  }
  patchbayView = activeRunProjection?.run
    ? {
        ...transaction.view,
        run: activeRunProjection.run,
        evidence: activeRunProjection.evidence,
      }
    : transaction.view;
  patchbaySourceRevision = transaction.result.source.revision;
  patchbayPresentationRevision = transaction.result.presentation.revision;
  acceptedSource = transaction.result.source.source;
  if (options.preserveFaceplateFocus || options.syncSource) {
    source.value = acceptedSource;
    syncSourceHighlight();
  }
  positions = transaction.result.presentation.node_positions;
  syncDiagnosticSourceHighlights();
  runButton.disabled =
    activeRunnability()?.state !== "runnable" || !patchbayView.plan || Boolean(activeAdapter);
  rememberLayout(current.id, positions, patchbayView);
  if (!options.preserveFaceplateFocus && !options.skipRender) {
    updateCytoscapeGraph();
  }
  return transaction;
}

function recordEvidence(event) {
  evidence.push(event);
  const maximum = Math.min(
    browserPlan.bounds.maximum_evidence_events,
    current.budgets?.evidence_events || browserPlan.bounds.maximum_evidence_events,
  );
  if (evidence.length > maximum) evidence.splice(0, evidence.length - maximum);
  document.querySelector("#evidence").textContent =
    evidence.length === 0 ? "No run evidence yet." : JSON.stringify(evidence, null, 2);
}

function activeScenario() {
  if (!scenarioSelect) return null;
  const libraryScenario = current.library?.scenarios?.find(
    (scenario) => scenario.id === scenarioSelect.value,
  );
  if (libraryScenario) {
    if (libraryScenario.validation?.kind === "diagnostic" &&
        libraryScenario.validation.value === "CND-IMP-001") {
      return {
        ...libraryScenario,
        runnability: {
          state: "unsupported",
          profile: current.runnability?.profile || "browser",
          code: libraryScenario.validation.value,
          reason: libraryScenario.semantics,
        },
      };
    }
    return libraryScenario;
  }
  const platformProfile = current.platform?.profiles?.find(
    (profile) => profile.id === scenarioSelect.value,
  );
  if (!platformProfile) return null;
  const outcome = platformProfile.admission === "accepted"
    ? "admitted by the checked contract"
    : `rejected before execution with ${platformProfile.code}`;
  const runnability = platformProfile.admission !== "accepted"
    ? {
        state: "illustrative/unavailable",
        profile: current.runnability?.profile || "browser",
        code: platformProfile.code,
        reason: `${platformProfile.id} has no admitted implementation on the current browser host.`,
      }
    : platformProfile.browser_execution === "fixture-only"
      ? {
          state: "illustrative/unavailable",
          profile: "browser",
          code: "CND-HST-002",
          reason: `${platformProfile.id} is proven by the checked host fixture and native conformance run, not executed by this browser host.`,
        }
      : current.supporting_runnability || current.runnability;
  return {
    ...platformProfile,
    source: current.supporting_source || current.source,
    execution: current.supporting_execution || current.execution,
    validation: current.supporting_validation || current.validation,
    watch_target: current.supporting_watch_target || current.watch_target,
    runnability,
    explanation: `${platformProfile.id}: ${outcome}. The editable representative panel remains real source and reruns independently.`,
  };
}

function activeRunnability() {
  return activeScenario()?.runnability || current?.runnability;
}

function authoredSource() {
  return activeScenario()?.source || current.source;
}

function stopTimelinePlayback() {
  if (timelineTimer !== null) {
    clearInterval(timelineTimer);
    timelineTimer = null;
  }
}

function explainTimelineRecord(record) {
  if (record.source === "exact-run-result") {
    return `The exact browser run was rejected before evidence could be emitted: ${record.event_detail}`;
  }
  const time = `At deterministic tick ${record.tick}, event ${record.sequence}`;
  const subject = `${record.subject_kind} ${record.subject_id}`;
  const action = record.event_detail
    ? `${record.event_kind} (${record.event_detail})`
    : record.event_kind;
  const pressure = record.subject_kind === "cord"
    ? ` The cord used ${record.pressure} pressure with occupancy ${record.occupancy_items} items / ${record.occupancy_bytes} bytes.`
    : "";
  const terminal = record.terminal_cause
    ? ` The run became terminal: ${record.terminal_cause}.`
    : "";
  return `${time} records ${action} for ${subject}.${pressure}${terminal}`;
}

function highlightTimelineSubject(record) {
  document.querySelectorAll(".timeline-linked").forEach(
    (element) => element.classList.remove("timeline-linked"),
  );
  const targetId = record?.node_id || record?.cord_id;
  if (!targetId) return;
  if (record.node_id) {
    selectNode(record.node_id);
  } else if (record.cord_id) {
    selectCord(record.cord_id);
  }
  requestAnimationFrame(() => {
    document.querySelectorAll("[data-id]").forEach((element) => {
      if (element.dataset.id === targetId) element.classList.add("timeline-linked");
    });
  });
}

function selectTimelineRecord(index) {
  if (timelineRecords.length === 0) {
    timelineCursor = -1;
    timelinePosition.disabled = true;
    timelinePositionLabel.textContent = "No exact run evidence yet.";
    timelineExplanation.textContent =
      "Run a scenario to inspect its exact ordered evidence.";
    highlightTimelineSubject(null);
    return;
  }
  timelineCursor = Math.max(0, Math.min(index, timelineRecords.length - 1));
  timelinePosition.disabled = false;
  timelinePosition.max = String(timelineRecords.length - 1);
  timelinePosition.value = String(timelineCursor);
  const record = timelineRecords[timelineCursor];
  timelinePositionLabel.textContent =
    `${timelineCursor + 1} of ${timelineRecords.length}: ${record.event_kind}`;
  timelineExplanation.textContent = explainTimelineRecord(record);
  timelineLanes.querySelectorAll(".timeline-event").forEach((marker) => {
    const markerIndex = Number(marker.dataset.index);
    marker.classList.toggle("current", markerIndex === timelineCursor);
    marker.classList.toggle("future", markerIndex > timelineCursor);
    marker.setAttribute("aria-current", markerIndex === timelineCursor ? "true" : "false");
  });
  timelineTableBody.querySelectorAll("tr").forEach((row) => {
    row.classList.toggle("selected", Number(row.dataset.index) === timelineCursor);
  });
  highlightTimelineSubject(record);
}

function renderTimeline(records) {
  stopTimelinePlayback();
  timelineRecords = records;
  timelineLanes.replaceChildren();
  timelineTableBody.replaceChildren();
  const lanes = [...new Set(records.map((record) => record.subject_id))];
  for (const subjectId of lanes) {
    const lane = document.createElement("div");
    lane.className = "timeline-lane";
    const label = document.createElement("span");
    label.className = "timeline-lane-label";
    label.textContent = subjectId;
    label.title = subjectId;
    const track = document.createElement("div");
    track.className = "timeline-track";
    records.forEach((record, index) => {
      const slot = document.createElement("span");
      if (record.subject_id === subjectId) {
        const marker = document.createElement("button");
        marker.type = "button";
        marker.className = "timeline-event";
        marker.dataset.index = String(index);
        marker.dataset.subjectKind = record.subject_kind;
        marker.dataset.terminal = String(Boolean(record.terminal_cause));
        marker.textContent = String(record.sequence ?? index);
        marker.title = explainTimelineRecord(record);
        marker.setAttribute(
          "aria-label",
          `Select event ${record.sequence ?? index}: ${record.event_kind} for ${record.subject_id}`,
        );
        marker.onclick = () => selectTimelineRecord(index);
        slot.append(marker);
      }
      track.append(slot);
    });
    lane.append(label, track);
    timelineLanes.append(lane);
  }
  records.forEach((record, index) => {
    const row = document.createElement("tr");
    row.dataset.index = String(index);
    const pressure = record.pressure
      ? `${record.pressure}; ${record.occupancy_items} items / ${record.occupancy_bytes} bytes`
      : "—";
    for (const value of [
      record.sequence ?? "—",
      record.tick ?? "before execution",
      `${record.subject_kind}: ${record.subject_id}`,
      record.event_detail
        ? `${record.event_kind}: ${record.event_detail}`
        : record.event_kind,
      pressure,
      record.terminal_cause || "—",
    ]) {
      const cell = document.createElement("td");
      cell.textContent = String(value);
      row.append(cell);
    }
    timelineTableBody.append(row);
  });
  selectTimelineRecord(records.length - 1);
}

function renderExactResultTimeline(value) {
  if (!executionStory) return;
  const values = document.querySelector("#timeline-values");
  if (value.ok) {
    values.textContent =
      `Exact terminal state: ${value.terminal}\n` +
      `Exact stdout: ${JSON.stringify(value.stdout || "")}\n` +
      `Exact display: ${JSON.stringify(value.display || "")}\n` +
      `Exact stderr: ${JSON.stringify(value.stderr || "")}`;
  } else {
    values.textContent =
      `Exact run rejection: ${value.code || "unknown"}\n${value.diagnostic || ""}`;
  }
  if (Array.isArray(value.evidence) && value.evidence.length > 0) {
    renderTimeline(value.evidence);
    return;
  }
  if (!value.ok) {
    renderTimeline([{
      source: "exact-run-result",
      sequence: "—",
      tick: null,
      subject_kind: "run-result",
      subject_id: value.code || "rejected",
      event_kind: "run-rejected",
      event_detail: value.diagnostic || value.code || "unknown rejection",
      terminal_cause: "rejected",
    }]);
    return;
  }
  renderTimeline([]);
}

function configureExecutionStory() {
  stopTimelinePlayback();
  if (!executionStory) return;
  renderTimeline([]);
  document.querySelector("#timeline-values").textContent =
    "No exact run values yet.";
  const story = current.library || current.platform;
  executionStory.hidden = !story;
  if (!story) return;
  const platform = Boolean(current.platform);
  document.querySelector("#story-kind").textContent =
    platform ? "Platform contract lesson" : "Library lesson";
  document.querySelector("#story-selectable-title").textContent =
    platform ? "Checked plan profiles" : "Selectable contracts";
  document.querySelector("#library-summary").textContent =
    story.summary || current.objective;
  document.querySelector("#library-what").textContent = story.what;
  document.querySelector("#library-when").textContent = story.when;
  document.querySelector("#library-wrong").textContent = story.wrong;
  const contractList = document.querySelector("#library-contracts");
  contractList.replaceChildren();
  for (const contract of story.contracts || story.profiles) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "btn small";
    button.textContent = contract.id;
    button.onclick = () => {
      if (platform) {
        scenarioSelect.value = contract.id;
        scenarioSelect.dispatchEvent(new Event("change"));
      } else {
        selectNode(contract.instance);
        result.textContent =
          `${contract.id}: selected ${contract.instance} in the authoritative Patchbay projection.`;
      }
    };
    contractList.append(button);
  }
  const docs = document.querySelector("#library-docs");
  docs.replaceChildren();
  const references = [
    ...(story.docs || []),
    ...(platform ? [story.fixture, story.panel].filter(Boolean) : []),
  ];
  for (const path of references) {
    const item = document.createElement("li");
    const link = document.createElement("a");
    link.href = path;
    link.textContent = path.split("/").at(-1);
    item.append(link);
    docs.append(item);
  }
  scenarioSelect.replaceChildren();
  for (const scenario of platform ? story.profiles : (story.scenarios || story.profiles)) {
    const option = document.createElement("option");
    option.value = scenario.id;
    option.textContent = scenario.title || scenario.id;
    scenarioSelect.append(option);
  }
}

function renderPlan(projection = null) {
  document.querySelector("#plan").textContent = projection
    ? JSON.stringify(projection, null, 2)
    : "No Rust-resolved plan for this source yet.";
}

function renderRustProjection(projection) {
  if (!projection?.source || !projection.semantic || !projection.plan ||
      !projection.presentation || !projection.run || !Array.isArray(projection.evidence)) {
    throw new Error("CND-PBY-009: incomplete Rust Patchbay projection");
  }
  if (!patchbayView) {
    throw new Error("CND-PBY-009: run projection has no Patchbay workspace");
  }
  projection = {
    ...projection,
    run: {
      ...projection.run,
      source_revision: activeWorkerRunIdentity?.sourceRevision ?? null,
    },
  };
  activeRunProjection = projection;
  if (projection.source.semantic_hash !== patchbayView.source.semantic_hash) {
    applyLiveRunProjection(projection);
    return;
  }
  const sourceRevision = patchbayView.source.revision;
  const rebaseRange = (item) => ({
    ...item,
    source_range: item.source_range
      ? { ...item.source_range, source_revision: sourceRevision }
      : null,
  });
  const topology = {
    ...projection.topology,
    logical_nodes: projection.topology.logical_nodes.map(rebaseRange),
    cords: projection.topology.cords.map(rebaseRange),
    planned_realization: projection.topology.planned_realization
      ? {
          ...projection.topology.planned_realization,
          nodes: projection.topology.planned_realization.nodes.map((node) => ({
            ...node,
            source_origin_range: node.source_origin_range
              ? { ...node.source_origin_range, source_revision: sourceRevision }
              : null,
          })),
        }
      : null,
  };
  const survivingNodes = new Set(
    topology.logical_nodes.map((node) => node.id),
  );
  const retainedPositions = Object.fromEntries(
    Object.entries(patchbayView.presentation.node_positions)
      .filter(([nodeId]) => survivingNodes.has(nodeId)),
  );
  renderPlan(projection);
  patchbayView = {
    ...projection,
    source: patchbayView.source,
    semantic: patchbayView.semantic,
    presentation: {
      ...patchbayView.presentation,
      node_positions: retainedPositions,
    },
    topology,
  };
  positions = retainedPositions;
  updateCytoscapeGraph();
  evidence.splice(0, evidence.length, ...projection.evidence);
  document.querySelector("#evidence").textContent = JSON.stringify(evidence, null, 2);
}

function renderLiveRunProjection(projection) {
  if (!projection?.source || !projection.run || !Array.isArray(projection.evidence) ||
      !patchbayView) {
    throw new Error("CND-PBY-009: incomplete live run projection");
  }
  projection = {
    ...projection,
    run: {
      ...projection.run,
      source_revision: activeWorkerRunIdentity?.sourceRevision ?? null,
    },
  };
  activeRunProjection = projection;
  return authoritativeEvidenceDelta(projection.evidence);
}

async function stopExactSession(cause, message) {
  runEpoch += 1;
  clearLiveWakeTimer();
  stopTimelinePlayback();
  const adapter = activeAdapter;
  const sessionId = activeWorkerSessionId;
  const runIdentity = activeWorkerRunIdentity;
  activeAdapter = null;
  activeWorkerSessionId = null;
  activeWorkerRunIdentity = null;
  disableWatchControl();
  if (adapter && sessionId) {
    try {
      const cancelled = await adapter.request("patchbay-cancel-exact-run", {
        sessionId,
        ...runIdentity,
        disposition: "abort",
      });
      if (cancelled.ok && cancelled.value?.ok) {
        renderRustProjection(cancelled.value.view);
        renderExactResultTimeline(cancelled.value);
        recordEvidence({ kind: "run-cancelled", state: cancelled.value.state });
        await adapter.request("patchbay-dispose-exact-run", {
          sessionId,
          ...runIdentity,
        });
      } else {
        patchbayRenderer?.presentPlacementLoss(
          cancelled.value?.diagnostic || cancelled.code || "exact cancellation failed",
        );
      }
    } finally {
      adapter.terminate(cause);
    }
  } else if (adapter) {
    adapter.terminate(cause);
  }
  activeRunProjection = null;
  deferredLivePresentation = null;
  deferredLiveDeltaCount = 0;
  displayIsFrozen = false;
  freezeDisplay.disabled = true;
  freezeDisplay.setAttribute("aria-pressed", "false");
  freezeDisplay.textContent = "Freeze Display (F)";
  displayFreezeStatus.textContent = "Display follows authoritative live deltas.";
  runButton.disabled = activeRunnability()?.state !== "runnable";
  stopButton.disabled = true;
  consoleBadge.textContent = "Ready";
  consoleBadge.className = "badge status-badge idle";
  if (message) result.textContent = message;
}

function show(lesson) {
  if (pendingSourceEditFrame !== null) {
    cancelAnimationFrame(pendingSourceEditFrame);
    pendingSourceEditFrame = null;
  }
  void stopExactSession("lesson-changed");
  resetWatchPresentation();
  current = lesson;
  patchbayView = null;
  patchbaySourceRevision = 0;
  patchbayPresentationRevision = 0;
  positions = {};
  updateCytoscapeGraph();
  workspaceController?.setDocument(lesson.id);
  document.querySelector("#title").textContent = lesson.title;
  document.querySelector("#goal").textContent = lesson.objective || lesson.title;
  document.querySelector("#prose").textContent = lesson.prose || "";
  if (workspaceKicker) workspaceKicker.textContent = "Embedded real lab";
  const availability = lesson.runnability;
  if (!availability) {
    throw new Error(`missing runnability declaration for ${lesson.id}`);
  }
  runnabilityState.textContent =
    `${availability.state} · ${availability.profile}`;
  runnabilityState.dataset.state = availability.state;
  document.querySelector("#execution-note").textContent = availability.state === "runnable"
    ? `${lesson.profile || availability.profile}: exact ${browserPlan.placement} placement on ${hostReport.hostId}.`
    : `${availability.code}: ${availability.reason}`;
  document.querySelector("#command").textContent =
    (lesson.commands ?? [lesson.command || "conduct inspect"]).join("  ·  ");

  // Active state highlighting in nav
  document.querySelectorAll("[data-lesson-id]").forEach((button) => {
    button.classList.toggle("active", button.dataset.lessonId === lesson.id);
  });

  const draft = localStorage.getItem(draftKey(lesson.id));
  source.value = draft ?? lesson.source;
  source.setSourceDiagnosticRanges?.([]);
  syncSourceHighlight();
  const parsedDraft = JSON.parse(parse_panel(source.value));
  acceptedSource = parsedDraft.ok ? source.value : lesson.source;
  selectedNode = null;
  selectedCord = null;
  topologyView = "logical";
  undoResetButton.disabled = localStorage.getItem(recoveryKey(lesson.id)) === null;
  closeTechnicalDrawers();
  evidence.length = 0;
  recordEvidence({ kind: "lesson-selected", lesson: lesson.id });
  configureExecutionStory();
  renderPlan();
  openPatchbaySession();
  check();
  runButton.disabled = availability.state !== "runnable";
}

function selectNode(node) {
  const logicalProjection = (patchbayView?.topology?.logical_nodes || [])
    .find((candidate) => candidate.id === node);
  const plannedProjection = plannedNodesForPresentation()
    .find((candidate) => candidate.id === node);
  const projection = logicalProjection || plannedProjection;
  if (!selectSourceRange("node", node, projection?.source_range)) return;
  selectedNode = node;
  selectedCord = null;
  selectedNodeLabel.textContent = plannedProjection
    ? `Selected planned instance: ${node}; source origin: ${plannedProjection.logical_origin}`
    : `Selected semantic node: ${node}`;
  moveLeftBtn.disabled = Boolean(plannedProjection);
  moveRightBtn.disabled = Boolean(plannedProjection);
  if (patchbayRenderer) {
    patchbayRenderer.selectNode(node);
  }
}

function selectCord(cordId) {
  const projection = projectedCordsForTopologyView()
    .find((candidate) => candidate.id === cordId);
  if (topologyView === "expanded") {
    selectedNode = null;
    selectedCord = cordId;
    moveLeftBtn.disabled = true;
    moveRightBtn.disabled = true;
    selectedNodeLabel.textContent = `Selected exact planned cord: ${cordId}`;
    result.textContent =
      "Planned cords are immutable plan facts and have no independent authored source declaration.";
    patchbayRenderer?.selectCord(cordId);
    return;
  }
  if (!selectSourceRange("cord", cordId, projection?.source_range)) return;
  source.setSourceRelatedRanges?.([
    projection.from_port_range,
    projection.to_port_range,
  ]);
  selectedNode = null;
  selectedCord = cordId;
  moveLeftBtn.disabled = true;
  moveRightBtn.disabled = true;
  const provenance = projection.source_range?.provenance === "authored"
    ? ""
    : " (derived edge; revealing authored owner)";
  selectedNodeLabel.textContent = `Selected cord: ${cordId}${provenance}`;
  patchbayRenderer?.selectCord(cordId);
}

function selectPort(nodeId, port) {
  if (topologyView === "expanded") {
    const plannedNode = plannedNodesForPresentation()
      .find((candidate) => candidate.id === nodeId);
    selectSourceRange(
      "planned instance source origin",
      plannedNode?.logical_origin || nodeId,
      plannedNode?.source_range,
    );
    selectedNode = nodeId;
    selectedCord = null;
    moveLeftBtn.disabled = true;
    moveRightBtn.disabled = true;
    selectedNodeLabel.textContent =
      `Selected planned port ${nodeId}.${port.id}; source origin: ` +
      `${plannedNode?.logical_origin || "unavailable"}`;
    return;
  }
  const cord = (patchbayView?.topology?.cords || []).find((candidate) =>
    port.direction === "input"
      ? candidate.to_port_path === port.semantic_path
      : candidate.from_port_path === port.semantic_path
  );
  if (cord) {
    const range = port.direction === "input"
      ? cord.to_port_range
      : cord.from_port_range;
    const projectedPath = port.direction === "input"
      ? cord.to_port_path
      : cord.from_port_path;
    if (projectedPath !== port.semantic_path ||
        !selectSourceRange("port", port.semantic_path, range)) {
      result.textContent =
        `CND-PBY-STALE: port selection path ${port.semantic_path} ` +
        "does not match its authoritative cord projection.";
      return;
    }
    selectedNode = null;
    selectedCord = cord.id;
    patchbayRenderer?.selectCord(cord.id);
    selectedNodeLabel.textContent =
      `Selected ${port.accessible_label}: ${port.semantic_path}`;
    return;
  }
  const projection = [
    ...(patchbayView?.topology?.logical_nodes || []),
    ...plannedNodesForPresentation(),
  ].find((candidate) => candidate.id === nodeId);
  if (!selectSourceRange("port owner", port.semantic_path, projection?.source_range)) {
    return;
  }
  selectedNode = nodeId;
  selectedCord = null;
  selectedNodeLabel.textContent =
    `Selected ${port.accessible_label}: ${port.semantic_path}`;
}

function selectSourceRange(kind, id, range) {
  if (!range) {
    result.textContent =
      `Selected ${kind} ${id} has no direct authored source range.`;
    return false;
  }
  if (!patchbayView || range.source_revision !== patchbaySourceRevision ||
      patchbayView.source.revision !== patchbaySourceRevision ||
      patchbayView.source.source !== source.value) {
    result.textContent =
      `CND-PBY-STALE: ${kind} selection was rejected because the source projection is stale.`;
    return false;
  }
  source.setSourceRelatedRanges?.([]);
  source.setSourceHighlightRange?.(range.start_utf16, range.end_utf16);
  source.setSelectionRange(range.start_utf16, range.end_utf16);
  const line = source.value.slice(0, range.start_utf16).split("\n").length - 1;
  const computedLineHeight = Number.parseFloat(getComputedStyle(source).lineHeight);
  const lineHeight = Number.isFinite(computedLineHeight) ? computedLineHeight : 20;
  source.scrollTop = Math.max(0, line * lineHeight - source.clientHeight / 3);
  source.syncHighlight?.();
  return true;
}

function clearTopologySelection() {
  selectedNode = null;
  selectedCord = null;
  selectedNodeLabel.textContent = "No topology item selected";
  moveLeftBtn.disabled = true;
  moveRightBtn.disabled = true;
  source.setSourceHighlightRange?.(null, null);
  source.setSourceRelatedRanges?.([]);
  patchbayRenderer?.highlightCordEndpoints(null);
}

function check() {
  const parsed = JSON.parse(parse_panel(source.value));
  const availability = activeRunnability();
  if (!parsed.ok) {
    result.textContent = parsed.diagnostic;
  } else if (availability.state === "runnable") {
    result.textContent =
      `Valid runnable panel: ${parsed.nodes} nodes, ${parsed.cords} cords.`;
  } else {
    const resolved = JSON.parse(explain_panel(source.value));
    const diagnostic = resolved.ok
      ? `${availability.code}: declared ${availability.state}; execution remains disabled`
      : resolved.diagnostic;
    const checkComplete = current.validation?.kind === "diagnostic"
      && diagnostic.includes(current.validation.value);
    result.textContent = checkComplete
      ? `✓ Lesson check complete (not execution evidence).\n${diagnostic}`
      : `${availability.code}: ${availability.reason}\n${diagnostic}`;
    if (checkComplete) {
      recordEvidence({
        kind: "lesson-check-completed",
        lesson: current.id,
        executionEvidence: false,
      });
    }
  }
  updateCytoscapeGraph();
  renderTopology();
  runButton.disabled =
    activeRunnability()?.state !== "runnable" || !patchbayView?.plan;
}

function renderTopology() {
  const explanation = JSON.parse(explain_panel(source.value));
  const logicalButton = document.querySelector("#logical-view");
  const expandedButton = document.querySelector("#expanded-view");
  const realization = patchbayView?.topology?.planned_realization || null;
  const realizationStatus = patchbayView?.topology?.planned_realization_status ||
    "no-exact-plan";
  if (!realization && topologyView === "expanded") topologyView = "logical";
  expandedButton.disabled = !realization;
  expandedButton.title = realization
    ? "Show the read-only exact planned realization"
    : "No exact plan has been resolved";
  logicalButton.classList.toggle("active", topologyView === "logical");
  logicalButton.setAttribute("aria-pressed", String(topologyView === "logical"));
  expandedButton.classList.toggle("active", topologyView === "expanded");
  expandedButton.setAttribute("aria-pressed", String(topologyView === "expanded"));
  const notice = document.querySelector("#plan-view-notice");
  notice.textContent = realization
    ? topologyView === "expanded"
      ? realization.notice
      : "Logical shows editable semantic promises; provider, device, artifact, host, allocation, and authority facts are plan-only."
    : realizationStatus === "active-plan-mismatch"
      ? "The active run plan snapshot is missing or mismatched. Expanded is unavailable and candidate facts will not be blended into the run."
    : "No exact plan has been resolved. Expanded is unavailable; no realization is manufactured from registry defaults.";
  document.querySelector("#canvas-help").textContent = topologyView === "expanded"
    ? "Read-only exact plan. Select an instance to reveal its authored semantic origin; resolve source changes into a new plan."
    : "Drag nodes to adjust presentation layout. Drag from an outgoing jack to a receiving jack to connect; select a cord and drag either end to rewire it.";
  if (arrangeButton) arrangeButton.disabled = topologyView === "expanded";
  document.querySelector("#topology").textContent = topologyView === "expanded"
    ? JSON.stringify(realization, null, 2)
    : explanation.ok ? explanation.logical : explanation.diagnostic;
  renderStructuredTopology();
}

source.addEventListener("input", () => {
  localStorage.setItem(draftKey(current.id), source.value);
  // Browser editing APIs may deliver one logical replacement as a burst of
  // input events. Fail closed immediately, then resolve only the newest value
  // once the browser has finished that frame instead of manufacturing an
  // intermediate plan and lesson-check record for every chunk.
  runButton.disabled = true;
  const lessonId = current.id;
  const sourceValue = source.value;
  if (pendingSourceEditFrame !== null) {
    cancelAnimationFrame(pendingSourceEditFrame);
  }
  pendingSourceEditFrame = requestAnimationFrame(() => {
    pendingSourceEditFrame = null;
    if (current.id !== lessonId || source.value !== sourceValue) return;
    const transaction = applyPatchbayOperations(
      [{ ReplaceSource: { source: sourceValue } }],
      { skipRender: true },
    );
    if (!transaction.ok) {
      runButton.disabled = true;
      return;
    }
    check();
  });
});

scenarioSelect?.addEventListener("change", () => {
  void stopExactSession("scenario-changed");
  const scenario = activeScenario();
  if (!scenario) return;
  patchbayView = null;
  patchbaySourceRevision = 0;
  patchbayPresentationRevision = 0;
  positions = {};
  updateCytoscapeGraph();
  source.value = scenario.source;
  source.setSourceDiagnosticRanges?.([]);
  syncSourceHighlight();
  localStorage.removeItem(draftKey(current.id));
  acceptedSource = scenario.source;
  selectedNode = null;
  selectedCord = null;
  evidence.length = 0;
  recordEvidence({ kind: "scenario-selected", scenario: scenario.id });
  renderPlan();
  renderTimeline([]);
  openPatchbaySession();
  check();
  const runnability = activeRunnability();
  const readiness = runnability?.state === "runnable"
    ? "Ready for an exact deterministic run."
    : `${runnability.code}: ${runnability.reason}`;
  result.textContent = `${scenario.explanation}\n${readiness}`;
});

document.querySelector("#check").onclick = check;
if (arrangeButton) {
  arrangeButton.onclick = () => {
    localStorage.removeItem(layoutKey(current.id));
    autoArrangePatchbay();
  };
}
document.querySelector("#logical-view").onclick = () => {
  topologyView = "logical";
  updateCytoscapeGraph();
  renderTopology();
};
document.querySelector("#expanded-view").onclick = () => {
  if (!patchbayView?.topology?.planned_realization) {
    result.textContent = "No exact plan has been resolved; Expanded remains unavailable.";
    return;
  }
  topologyView = "expanded";
  updateCytoscapeGraph();
  renderTopology();
};

function moveSelected(delta) {
  if (!selectedNode) return;
  const currentPos = positions[selectedNode]
    || { x: 100, y: 80 };
  const newX = currentPos.x + delta;
  const newY = currentPos.y;
  const transaction = applyPatchbayOperations([{
    MoveNode: {
      node_id: selectedNode,
      position: { x: newX, y: newY }
    }
  }]);
  if (!transaction.ok) {
    return;
  }
  result.textContent =
    `Presentation moved; semantic hash remains ${transaction.result.semantic.source_semantic_hash}.`;
}

moveLeftBtn.onclick = () => moveSelected(-20);
moveRightBtn.onclick = () => moveSelected(20);

const EXACT_PUMP_TURN_DECISIONS = 32;
const MAXIMUM_EXACT_PUMP_TURNS = 8;

async function pumpExactRunCooperatively(adapter, sessionId, runIdentity) {
  let response = null;
  for (let turn = 0; turn < MAXIMUM_EXACT_PUMP_TURNS; turn += 1) {
    response = await adapter.request("patchbay-pump-exact-run", {
      sessionId,
      ...runIdentity,
      quantum: EXACT_PUMP_TURN_DECISIONS,
    });
    if (!response.ok || !response.value?.ok || response.value.state !== "active") {
      break;
    }
  }
  return response;
}

function scheduleContinuousWatch({
  adapter, sessionId, runIdentity, watchId, epoch, cursor, deadline,
}) {
  if (epoch !== runEpoch || activeAdapter !== adapter || !Number.isSafeInteger(deadline)) return;
  clearLiveWakeTimer();
  liveWakeTimer = setTimeout(async () => {
    liveWakeTimer = null;
    if (epoch !== runEpoch || activeAdapter !== adapter) return;
    const cycleControl = activeWatchControl?.watchId === watchId
      ? activeWatchControl
      : null;
    if (cycleControl?.pending) {
      scheduleContinuousWatch({
        adapter,
        sessionId,
        runIdentity,
        watchId,
        epoch,
        cursor,
        deadline,
      });
      return;
    }
    setWatchControlBusy(cycleControl, true);
    try {
      const advanced = await adapter.request("patchbay-advance-exact-run", {
        sessionId,
        ...runIdentity,
        tick: deadline,
      });
      if (!advanced.ok || !advanced.value?.ok) {
        throw new Error(advanced.value?.diagnostic || advanced.code || "timer wake failed");
      }
      const pumped = await pumpExactRunCooperatively(adapter, sessionId, runIdentity);
      if (!pumped.ok || !pumped.value?.ok) {
        throw new Error(pumped.value?.diagnostic || pumped.code || "ticker pump failed");
      }
      if (epoch !== runEpoch || activeAdapter !== adapter) return;
      // The exact topology is immutable for this epoch. Rebuilding React Flow
      // for every value would spend the observation budget on static work and
      // can starve Stop/Watch input on slower browser hosts.
      const liveDelta = renderLiveRunProjection(pumped.value.view);
      let nextCursor = cursor;
      let watched = null;
      if (cycleControl?.attached) {
        watched = await adapter.request("patchbay-read-exact-watch", {
          sessionId,
          ...runIdentity,
          operatorId: cycleControl.operatorId,
          watchId,
          cursor,
          maximumRecords: 1,
        });
        if (!watched.ok || !watched.value?.ok) {
          throw new Error(watched.value?.diagnostic || watched.code || "Watch read failed");
        }
        if (epoch !== runEpoch || activeAdapter !== adapter) return;
        nextCursor = watched.value.next_cursor;
        cycleControl.cursor = nextCursor;
      }
      const message =
        `✓ Live exact run remains ${pumped.value.state}.\n` +
        (watched
          ? `Latest public text: ${JSON.stringify(watched.value.records.at(-1)?.material?.text ?? "")}\n`
          : "Watch detached; the ticker continues without observation pressure.\n") +
        `Next admitted timer deadline: ${pumped.value.next_timer_deadline}.`;
      presentContinuousUpdate({
        projection: activeRunProjection,
        result: pumped.value,
        watched: watched?.value || null,
        control: cycleControl,
        delta: liveDelta,
        message,
      });
      recordEvidence({
        kind: watched ? "watch-latest" : "ticker-without-watch",
        lesson: current.id,
        cursor: nextCursor,
        state: pumped.value.state,
      });
      if (!pumped.value.terminal) {
        scheduleContinuousWatch({
          adapter,
          sessionId,
          runIdentity,
          watchId,
          epoch,
          cursor: nextCursor,
          deadline: pumped.value.next_timer_deadline,
        });
      }
    } catch (error) {
      if (epoch === runEpoch) {
        patchbayRenderer?.presentPlacementLoss(String(error));
        liveFlowStatus.textContent =
          `Abrupt browser placement loss: ${error}. This is not graceful cancellation.`;
        result.textContent = `Live ticker stopped: ${error}`;
        consoleBadge.textContent = "Failed";
        consoleBadge.className = "badge status-badge failed";
      }
    } finally {
      setWatchControlBusy(cycleControl, false);
    }
  }, LIVE_WATCH_PRESENTATION_INTERVAL_MS);
}

async function run() {
  const runnability = activeRunnability();
  if (runnability?.state !== "runnable") {
    result.textContent =
      `${runnability.code}: ${runnability.reason}`;
    return;
  }
  if (!patchbayView?.plan) {
    result.textContent =
      "CND-PBY-NO-PLAN: no exact plan exists for the current source revision.";
    return;
  }
  await stopExactSession("superseded");
  const epoch = ++runEpoch;
  const binding = resolveBrowserPlacement(hostReport, {
    tick: 11,
    placement: Placement.DedicatedWorker,
    minimumResources: {
      queueBytes: browserPlan.bounds.maximum_message_bytes,
      pendingMessages: browserPlan.bounds.maximum_pending,
    },
  });
  if (!binding.ok) {
    result.textContent = `${binding.code}: ${binding.detail}`;
    recordEvidence({ kind: "placement-rejected", code: binding.code });
    return;
  }
  const workerArtifact = loadedArtifacts.get("tour-worker");
  const wasmArtifact = loadedArtifacts.get("conduit-web-wasm");
  const adapter = new DedicatedWorkerExecutionAdapter({
    ...binding,
    planIdentity: browserPlan.plan_identity,
    artifactUrl: workerArtifact.url,
    maximumPending: browserPlan.bounds.maximum_pending,
    maximumMessageBytes: browserPlan.bounds.maximum_message_bytes,
    responseTimeoutMs: browserPlan.bounds.response_timeout_ms,
  }, recordEvidence);
  activeAdapter = adapter;
  const started = adapter.start();
  if (!started.ok) {
    activeAdapter = null;
    result.textContent = `${started.code}: ${started.detail}`;
    return;
  }
  runButton.disabled = true;
  stopButton.disabled = false;
  consoleBadge.textContent = "Running";
  consoleBadge.className = "badge status-badge running";
  result.textContent = "Executing graph in browser placement worker…";

  let terminal = false;
  let watchId = null;
  let watchOperatorId = null;
  let watchCursor = 0;
  let sessionId = null;
  let runIdentity = null;
  try {
    const configured = await adapter.request("configure", {
      wasmUrl: wasmArtifact.url.href,
      wasmSha256: wasmArtifact.artifact.sha256,
    });
    if (!configured.ok) throw new Error(configured.code);
    const opened = await adapter.request("patchbay-open-session", {
      documentId: `tour/worker-run/${epoch}`,
      source: source.value,
    });
    if (!opened.ok || !opened.value?.ok) {
      throw new Error(`${opened.code || opened.value?.code || "open-failed"}: ${opened.value?.diagnostic || ""}`);
    }
    sessionId = opened.value.session_id;
    activeWorkerSessionId = sessionId;
    const started = await adapter.request("patchbay-start-exact-run", { sessionId });
    if (!started.ok || !started.value?.ok) {
      const rejection = {
        ok: false,
        code: started.code || started.value?.code || "start-failed",
        diagnostic: started.value?.diagnostic || "exact session start was rejected",
      };
      terminal = true;
      renderExactResultTimeline(rejection);
      result.textContent = rejection.diagnostic;
      recordEvidence({ kind: "run-rejected", lesson: current.id, code: rejection.code });
      return;
    }
    runIdentity = {
      runId: started.value.run_id,
      sourceRevision: started.value.source_revision,
      planIdentity: started.value.plan_identity,
    };
    activeWorkerRunIdentity = runIdentity;
    setLivePresentationActive();
    if ((activeScenario()?.execution || current.execution) === "continuous-watch") {
      const compatibleAdmissions = started.value.view.plan.watch_admissions.filter(
        (watch) => watch.representation_id === "std/text" &&
          watch.retention === "latest" &&
          watch.sensitivity_ceiling === "public",
      );
      const targetCord = current.watch_target
        ? started.value.view.topology.cords.find((cord) =>
            cord.from_node === current.watch_target.from_node &&
            cord.from_port === current.watch_target.from_port)
        : null;
      const admission = current.watch_target
        ? compatibleAdmissions.find((watch) => watch.cord === targetCord?.id)
        : compatibleAdmissions[0];
      if (!admission) throw new Error("exact plan has no public latest-value text Watch");
      watchId = admission.id;
      watchOperatorId = admission.operator;
      const attached = await adapter.request("patchbay-attach-exact-watch", {
        sessionId,
        ...runIdentity,
        operatorId: watchOperatorId,
        watchId,
      });
      if (!attached.ok || !attached.value?.ok) {
        throw new Error(attached.value?.diagnostic || attached.code || "Watch attach failed");
      }
      setWatchControl({
        adapter,
        sessionId,
        runIdentity,
        watchId,
        operatorId: watchOperatorId,
        subjectLabel: admission.subject_kind === "cord"
          ? `cord ${admission.cord}`
          : `${admission.node}.${admission.port}`,
        attached: true,
        cursor: 0,
      });
    }
    const operation = activeScenario()?.execution === "cancel-before-first-step"
      ? "patchbay-cancel-exact-run"
      : "patchbay-pump-exact-run";
    const executed = operation === "patchbay-cancel-exact-run"
      ? await adapter.request(operation, { sessionId, ...runIdentity, disposition: "abort" })
      : await pumpExactRunCooperatively(adapter, sessionId, runIdentity);
    if (epoch !== runEpoch) return;
    if (!executed.ok || !executed.value?.ok) {
      const rejection = {
        ok: false,
        code: executed.code || executed.value?.code || "pump-failed",
        diagnostic: executed.value?.diagnostic || "exact session pump was rejected",
      };
      terminal = true;
      renderExactResultTimeline(rejection);
      result.textContent = rejection.diagnostic;
      recordEvidence({ kind: "run-rejected", lesson: current.id, code: rejection.code });
      return;
    }
    const value = executed.value;
    terminal = Boolean(value.terminal);
    renderRustProjection(value.view);
    renderExactResultTimeline(value);
    const initialLiveDelta = authoritativeEvidenceDelta(value.view.evidence);
    let watched = null;
    if (watchId) {
      const read = await adapter.request("patchbay-read-exact-watch", {
        sessionId,
        ...runIdentity,
        operatorId: watchOperatorId,
        watchId,
        cursor: watchCursor,
        maximumRecords: 1,
      });
      if (!read.ok || !read.value?.ok) {
        throw new Error(read.value?.diagnostic || read.code || "Watch read failed");
      }
      watched = read.value;
      watchCursor = watched.next_cursor;
      if (activeWatchControl?.watchId === watchId) {
        activeWatchControl.cursor = watchCursor;
      }
      renderLatestWatch(watched, value);
    }
    renderLiveFlowRows(value.view, initialLiveDelta, watched?.records?.at(-1), value);
    const counts = Number.isInteger(value.completed_nodes)
      ? `\nEvidence: ${value.completed_nodes} nodes, ${value.cords_conducted} cords conducted.`
      : "";
    const validation = activeScenario()?.validation || current.validation;
    const visibleValue = validation?.kind === "display"
      ? value.display
      : validation?.kind === "stdout"
        ? value.stdout
        : value.stdout || value.display;
    const visibleResult = value.ok
      ? `${visibleValue || (value.terminal
        ? `Run completed: ${value.terminal}.`
        : `Run remains ${value.state}; awaiting an admitted wake.`)}${counts}`
      : value.diagnostic;
    const lessonComplete = validation?.kind === "stdout"
      ? value.ok && Boolean(value.terminal) && value.stdout === validation.value
      : validation?.kind === "display"
        ? value.ok && Boolean(value.terminal) && value.display === validation.value
      : validation?.kind === "terminal"
        ? value.ok && value.terminal === validation.value
      : validation?.kind === "diagnostic"
          ? (!value.ok && value.diagnostic.includes(validation.value))
            || (value.stderr || "").includes(validation.value)
          : validation?.kind === "watch"
            ? value.ok && watched?.records?.at(-1)?.material?.text === validation.value
          : validation?.kind === "watch-prefix"
            ? value.ok && watched?.records?.at(-1)?.material?.text?.startsWith(validation.value)
          : value.ok;

    result.textContent = lessonComplete
      ? `✓ Lesson complete!\n${visibleResult}`
      : visibleResult;

    recordEvidence({
      kind: lessonComplete ? "lesson-completed" : (value.ok ? "run-completed" : "run-rejected"),
      lesson: current.id,
      completedNodes: value.completed_nodes,
      cordsConducted: value.cords_conducted,
      state: value.state,
    });
    if (watchId && !terminal) {
      scheduleContinuousWatch({
        adapter,
        sessionId,
        runIdentity,
        watchId,
        epoch,
        cursor: watchCursor,
        deadline: value.next_timer_deadline,
      });
    }
  } catch (error) {
    if (epoch === runEpoch) {
      patchbayRenderer?.presentPlacementLoss(String(error));
      liveFlowStatus.textContent =
        `Abrupt browser placement loss: ${error}. This is not graceful cancellation.`;
      result.textContent = `Run failed: ${error}`;
      await stopExactSession("run-failed");
    }
  } finally {
    if (epoch === runEpoch && terminal) {
      if (sessionId && runIdentity) {
        await adapter.request("patchbay-dispose-exact-run", {
          sessionId,
          ...runIdentity,
        });
      }
      adapter.terminate("completed");
      activeAdapter = null;
      activeWorkerSessionId = null;
      activeWorkerRunIdentity = null;
      activeRunProjection = null;
      displayIsFrozen = false;
      deferredLivePresentation = null;
      freezeDisplay.disabled = true;
      freezeDisplay.setAttribute("aria-pressed", "false");
      freezeDisplay.textContent = "Freeze Display (F)";
      disableWatchControl();
      runButton.disabled = activeRunnability()?.state !== "runnable";
      stopButton.disabled = true;
      consoleBadge.textContent = "Idle";
      consoleBadge.className = "badge status-badge idle";
    } else if (epoch === runEpoch) {
      consoleBadge.textContent = "Live";
      consoleBadge.className = "badge status-badge running";
    }
  }
}

runButton.onclick = run;
watchToggle.onclick = () => void toggleWatch();
freezeDisplay.onclick = toggleDisplayFreeze;
stopButton.onclick = () => void stopExactSession(
  "learner-cancelled",
  "Run cancelled; exact worker placement is terminal.",
);

document.addEventListener("keydown", (event) => {
  const target = event.target;
  const isEditing = target instanceof HTMLElement && (
    ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName) ||
    target.isContentEditable
  );
  const isWatchShortcut = event.key.toLowerCase() === "w" &&
    !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
  if (isWatchShortcut && !isEditing && !event.repeat && !event.isComposing) {
    event.preventDefault();
    if (!watchToggle.disabled) void toggleWatch();
    return;
  }
  const isFreezeShortcut = event.key.toLowerCase() === "f" &&
    !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
  if (isFreezeShortcut && !isEditing && !event.repeat && !event.isComposing) {
    event.preventDefault();
    toggleDisplayFreeze();
    return;
  }
  const isRunShortcut = event.shiftKey
    && event.key === "Enter"
    && !event.altKey
    && !event.ctrlKey
    && !event.metaKey;
  if (!isRunShortcut || event.repeat || event.isComposing) return;

  event.preventDefault();
  if (!runButton.disabled) void run();
});

document.querySelector("#reset").onclick = () => {
  void stopExactSession("reset");
  resetWatchPresentation();
  localStorage.setItem(recoveryKey(current.id), source.value);
  localStorage.removeItem(layoutKey(current.id));
  source.value = authoredSource();
  syncSourceHighlight();
  acceptedSource = source.value;
  selectedNode = null;
  selectedCord = null;
  positions = {};
  selectedNodeLabel.textContent = "No topology item selected";
  moveLeftBtn.disabled = true;
  moveRightBtn.disabled = true;
  localStorage.removeItem(draftKey(current.id));
  undoResetButton.disabled = false;
  openPatchbaySession();
  check();
};

if (executionStory) {
  timelinePosition.addEventListener("input", () => {
    stopTimelinePlayback();
    selectTimelineRecord(Number(timelinePosition.value));
  });

  document.querySelector("#timeline-play").onclick = () => {
    if (timelineRecords.length === 0 || timelineTimer !== null) return;
    if (timelineCursor >= timelineRecords.length - 1) selectTimelineRecord(0);
    timelineTimer = setInterval(() => {
      if (timelineCursor >= timelineRecords.length - 1) {
        stopTimelinePlayback();
        return;
      }
      selectTimelineRecord(timelineCursor + 1);
    }, 650);
  };
  document.querySelector("#timeline-pause").onclick = stopTimelinePlayback;
  document.querySelector("#timeline-step").onclick = () => {
    stopTimelinePlayback();
    if (timelineRecords.length > 0) selectTimelineRecord(timelineCursor + 1);
  };
  document.querySelector("#timeline-reset").onclick = () => {
    stopTimelinePlayback();
    if (timelineRecords.length > 0) selectTimelineRecord(0);
  };
  document.querySelector("#timeline-replay").onclick = () => {
    stopTimelinePlayback();
    if (timelineRecords.length === 0) return;
    selectTimelineRecord(0);
    document.querySelector("#timeline-play").click();
  };
  executionStory.addEventListener("keydown", (event) => {
    if (["INPUT", "SELECT", "BUTTON"].includes(event.target.tagName)) return;
    if (event.key === " ") {
      event.preventDefault();
      document.querySelector("#timeline-play").click();
    } else if (event.key === "ArrowRight") {
      event.preventDefault();
      document.querySelector("#timeline-step").click();
    }
  });
}

undoResetButton.onclick = () => {
  const recovered = localStorage.getItem(recoveryKey(current.id));
  if (recovered === null) return;
  source.value = recovered;
  syncSourceHighlight();
  localStorage.setItem(draftKey(current.id), recovered);
  localStorage.removeItem(recoveryKey(current.id));
  undoResetButton.disabled = true;
  const parsed = JSON.parse(parse_panel(recovered));
  acceptedSource = parsed.ok ? recovered : current.source;
  openPatchbaySession();
  check();
};

document.querySelector("#download").onclick = () => {
  const link = document.createElement("a");
  link.href = URL.createObjectURL(new Blob([source.value], { type: "text/plain" }));
  link.download = `${current.id}.panel`;
  link.click();
  URL.revokeObjectURL(link.href);
};

function applyReaderRoute({ restoreReading = false } = {}) {
  const parameters = new URLSearchParams(location.search);
  const requestedSection = parameters.get("section");
  if (requestedSection && sectionById.has(requestedSection)) {
    const position = storedReadingPosition();
    openReaderSection(sectionById.get(requestedSection), {
      updateHistory: false,
      scrollTop: position?.section_id === requestedSection ? position.scroll_top : 0,
    });
    return;
  }

  const requestedLesson = parameters.get("lesson");
  if (requestedLesson) {
    const disposition = migrationByLessonId.get(requestedLesson);
    if (disposition) {
      workspace.dataset.mode = "expanded";
      const destination = disposition.destination;
      if (["Book", "Interlude"].includes(disposition.classification)) {
        openReaderSection(sectionById.get(destination.id), { updateHistory: false });
        return;
      }
      if (disposition.classification === "Cookbook") {
        const entry = directoryEntries("cookbook")
          .find((candidate) => candidate.id === destination.id);
        openDirectoryLab("cookbook", entry, { updateHistory: false });
        return;
      }
      if (disposition.classification === "Reference") {
        const entry = directoryEntries("reference")
          .find((candidate) => candidate.id === destination.id);
        openDirectoryLab("reference", entry, { updateHistory: false });
        return;
      }
      if (disposition.classification === "Retire/Replace") {
        openRetiredLesson(retiredByLessonId.get(requestedLesson), {
          updateHistory: false,
        });
        return;
      }
    }
  }

  const view = parameters.get("view");
  if (["reference", "cookbook"].includes(view)) {
    const item = parameters.get("item");
    const entry = item && directoryEntries(view).find((candidate) => candidate.id === item);
    if (entry) {
      openDirectoryLab(view, entry, { updateHistory: false });
    } else {
      openDirectory(view, {
        updateHistory: false,
        query: parameters.get("q") || "",
      });
    }
    return;
  }

  const position = restoreReading ? storedReadingPosition() : null;
  if (position) {
    openReaderSection(sectionById.get(position.section_id), {
      updateHistory: false,
      scrollTop: position.scroll_top,
    });
  } else {
    renderCover({ updateHistory: false });
  }
}

const pageParameters = new URLSearchParams(location.search);
if (readerContent) {
  expandLabButton.onclick = () => {
    setLabExpanded(expandLabButton.getAttribute("aria-expanded") !== "true");
  };
  document.querySelector("#show-book").onclick = () => renderCover();
  document.querySelector("#show-reference").onclick = () => openDirectory("reference");
  document.querySelector("#show-cookbook").onclick = () => openDirectory("cookbook");
  document.querySelector("#begin-book").onclick = () =>
    openReaderSection(sectionById.get(book.cover.start_section));
  readerContent.addEventListener("scroll", saveReadingPosition, { passive: true });
  window.addEventListener("beforeunload", saveReadingPosition);
  window.addEventListener("hashchange", revealHashTarget);
  for (const details of document.querySelectorAll("#workspace details[id]")) {
    details.addEventListener("toggle", () => {
      if (!details.open || readerSection.hidden) return;
      const url = new URL(location.href);
      url.hash = details.id;
      history.replaceState({}, "", url);
    });
  }
  renderBookToc();
  applyReaderRoute({ restoreReading: location.search === "" });
  window.addEventListener("popstate", () => applyReaderRoute());
} else {
  for (const lesson of lessons.lessons) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = lesson.title;
    button.dataset.lessonId = lesson.id;
    button.onclick = () => show(lesson);
    const item = document.createElement("li");
    item.append(button);
    document.querySelector("#lessons").append(item);
  }
  for (const panel of referencePanels) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = panel.title;
    button.dataset.lessonId = panel.id;
    button.onclick = () => show(panel);
    const item = document.createElement("li");
    item.append(button);
    document.querySelector("#reference-panels").append(item);
  }
  const requestedLesson = pageParameters.get("lesson");
  current = lessons.lessons.find((lesson) => lesson.id === requestedLesson)
    || referencePanels.find((panel) => panel.id === requestedLesson)
    || current;
  show(current);
}

const requestedScenario = pageParameters.get("scenario");
if (requestedScenario && scenarioSelect &&
    [...(current.library?.scenarios || []), ...(current.platform?.profiles || [])]
      .some((scenario) => scenario.id === requestedScenario)) {
  scenarioSelect.value = requestedScenario;
  scenarioSelect.dispatchEvent(new Event("change"));
}
document.documentElement.dataset.tourReady = "true";
if (pageParameters.has("autorun") && !workspace.hidden) await run();
