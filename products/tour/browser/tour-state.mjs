const encoder = new TextEncoder();
const MAXIMUM_DRAFTS = 32;
const MAXIMUM_EXPANDED_BACKS = 16;
const MAXIMUM_STATE_KEY_BYTES = 128;
const MAXIMUM_DRAFT_BYTES = 4096;
const FORM_NAME = /^\s*form\s+([a-z][a-z0-9-]*)\s*\{/m;

export function identifyTourSpecimen(source) {
  const name = source.match(FORM_NAME)?.[1];
  if (!name) throw new Error("a runnable Tour specimen has no stable Form identity");
  return `canonical-form:${name}`;
}

export function createTourStage(source, mode) {
  const identity = identifyTourSpecimen(source);
  return Object.freeze({
    identity,
    label: source.match(FORM_NAME)[1],
    source,
    mode,
    recursive: mode === "recursive",
    faceBack: mode === "compare",
    multiHost: mode === "two-host" || mode === "two-host-plan",
    showPlan: mode === "two-host-plan",
  });
}

export function conceptualTourStage(pageTitle, companionIdentity) {
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(companionIdentity)) {
    throw new Error("a conceptual Tour lesson has no admitted companion identity");
  }
  return Object.freeze({ identity: `tour-companion:${companionIdentity}`, label: pageTitle, mode: "conceptual" });
}

function validKey(value) {
  return typeof value === "string" && value.length > 0 && encoder.encode(value).length <= MAXIMUM_STATE_KEY_BYTES;
}

export async function openTourReadingState(storage) {
  const drafts = new Map();
  const expandedBacks = new Set();
  let pending = Promise.resolve();
  let needsMigration = false;
  const workspace = { narrativePercent: 46, patchbayPercent: 55, sourcePercent: 60 };
  const state = await storage.readJson("reading-state");
  const storedWorkspace = await storage.readJson("workspace-layout");
  if (storedWorkspace !== null) {
    if ((storedWorkspace?.schema !== "conduit.tour/workspace-layout@1"
        && storedWorkspace?.schema !== "conduit.tour/workspace-layout@2")
      || !Number.isInteger(storedWorkspace.narrative_percent)
      || storedWorkspace.narrative_percent < 30 || storedWorkspace.narrative_percent > 65
      || (storedWorkspace.schema === "conduit.tour/workspace-layout@2"
        && (!Number.isInteger(storedWorkspace.patchbay_percent)
          || storedWorkspace.patchbay_percent < 35 || storedWorkspace.patchbay_percent > 70
          || !Number.isInteger(storedWorkspace.source_percent)
          || storedWorkspace.source_percent < 40 || storedWorkspace.source_percent > 75))) {
      throw new Error("persisted Tour workspace layout is malformed");
    }
    workspace.narrativePercent = storedWorkspace.narrative_percent;
    if (storedWorkspace.schema === "conduit.tour/workspace-layout@2") {
      workspace.patchbayPercent = storedWorkspace.patchbay_percent;
      workspace.sourcePercent = storedWorkspace.source_percent;
    }
  }
  if (state !== null) {
    if ((state?.schema !== "conduit.book/reading-state@1" && state?.schema !== "conduit.tour/reading-state@1")
      || !Array.isArray(state.drafts) || state.drafts.length > MAXIMUM_DRAFTS
      || !Array.isArray(state.expandedBacks) || state.expandedBacks.length > MAXIMUM_EXPANDED_BACKS) {
      throw new Error("persisted Tour state is malformed");
    }
    needsMigration = state.schema === "conduit.book/reading-state@1";
    for (const entry of state.drafts) {
      if (!Array.isArray(entry) || entry.length !== 2 || !validKey(entry[0])
        || typeof entry[1] !== "string" || encoder.encode(entry[1]).length > MAXIMUM_DRAFT_BYTES) {
        throw new Error("persisted Tour draft is outside its admitted bound");
      }
      drafts.set(entry[0], entry[1]);
    }
    for (const key of state.expandedBacks) {
      if (!validKey(key)) throw new Error("persisted Tour Back state is outside its admitted bound");
      expandedBacks.add(key);
    }
  }

  function persist() {
    const document = {
      schema: "conduit.tour/reading-state@1",
      drafts: [...drafts.entries()].slice(0, MAXIMUM_DRAFTS),
      expandedBacks: [...expandedBacks].slice(0, MAXIMUM_EXPANDED_BACKS),
    };
    pending = pending.then(() => storage.writeJson("reading-state", document));
    return pending;
  }

  function persistWorkspace() {
    pending = pending.then(() => storage.writeJson("workspace-layout", {
      schema: "conduit.tour/workspace-layout@2",
      narrative_percent: workspace.narrativePercent,
      patchbay_percent: workspace.patchbayPercent,
      source_percent: workspace.sourcePercent,
    }));
    return pending;
  }

  function setWorkspacePercent(key, value, minimum, maximum, label) {
    if (!Number.isInteger(value) || value < minimum || value > maximum) {
      throw new Error(`Tour ${label} is outside its admitted bound`);
    }
    workspace[key] = value;
    return persistWorkspace();
  }

  if (needsMigration) persist();

  return Object.freeze({
    schema: "conduit.tour/reading-state-handle@1",
    drafts,
    expandedBacks,
    workspace,
    setNarrativePercent: (value) => setWorkspacePercent("narrativePercent", value, 30, 65, "narrative width"),
    setPatchbayPercent: (value) => setWorkspacePercent("patchbayPercent", value, 35, 70, "Patchbay height"),
    setSourcePercent: (value) => setWorkspacePercent("sourcePercent", value, 40, 75, "source width"),
    persist,
    flush: () => pending,
  });
}
