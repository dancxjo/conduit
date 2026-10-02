const encoder = new TextEncoder();
const decoder = new TextDecoder();
const MAXIMUM_SEARCH_BYTES = 128;

export function readReviewedPlotInventory(runtime, source) {
  const bytes = encoder.encode(source);
  if (bytes.length === 0 || bytes.length > runtime.conduit_creche_input_capacity()) {
    throw new Error("reviewed plot inventory is outside the admitted runtime bound");
  }
  new Uint8Array(runtime.memory.buffer, runtime.conduit_creche_input_ptr(), bytes.length).set(bytes);
  const code = runtime.conduit_creche_reviewed_inventory(bytes.length);
  const output = JSON.parse(decoder.decode(new Uint8Array(
    runtime.memory.buffer,
    runtime.conduit_creche_output_ptr(),
    runtime.conduit_creche_output_len(),
  )));
  if (code < 0) throw new Error(output.message ?? `reviewed plot inventory refused (${code})`);
  return validateInventory(output);
}

export function reviewInitialWorkload(runtime, hostId, bootId, source, selected) {
  const segments = [hostId, bootId, encodedPlotSelection(selected), source].map((value) => encoder.encode(value));
  const total = segments.reduce((sum, bytes) => sum + bytes.length, 0);
  if (segments.some((bytes) => bytes.length === 0) || total > runtime.conduit_creche_input_capacity()) {
    throw new Error("initial workload review input is outside its admitted runtime bound");
  }
  const input = new Uint8Array(runtime.memory.buffer, runtime.conduit_creche_input_ptr(), total);
  let offset = 0;
  for (const bytes of segments) { input.set(bytes, offset); offset += bytes.length; }
  const code = runtime.conduit_creche_review_initial_workload(...segments.map((bytes) => bytes.length));
  const output = JSON.parse(decoder.decode(new Uint8Array(
    runtime.memory.buffer,
    runtime.conduit_creche_output_ptr(),
    runtime.conduit_creche_output_len(),
  )));
  if (code < 0) throw new Error(output.message ?? `initial workload review refused (${code})`);
  return output;
}

export function openPlotSelection(inventory, persisted = null, handoff = null) {
  validateInventory(inventory);
  const candidates = [];
  if (persisted !== null) {
    if (persisted?.schema !== "conduit.creche/plot-selection@1"
      || !Array.isArray(persisted.plots)
      || persisted.plots.length > inventory.maximum_selection) {
      throw new Error("persisted Crèche Plot selection is malformed or over capacity");
    }
    candidates.push(...persisted.plots.map((candidate) => ({ origin: "restored", candidate })));
  }
  if (handoff !== null) candidates.push({ origin: "gallery-handoff", candidate: handoff });
  const selected = [];
  const refusals = [];
  let acceptedHandoff = null;
  for (const { origin, candidate } of candidates) {
    const current = exactCurrent(inventory, candidate);
    if (!current) {
      refusals.push({ disposition: "stale-plot-identity", origin, candidate });
      continue;
    }
    if (origin === "gallery-handoff") acceptedHandoff = current;
    if (selected.some((plot) => plot.checked_plot_id === current.checked_plot_id)) continue;
    if (selected.length === inventory.maximum_selection) {
      refusals.push({ disposition: "selection-capacity-exhausted", origin, candidate });
      continue;
    }
    selected.push(current);
  }
  return Object.freeze({ selected, refusals, acceptedHandoff });
}

export function togglePlot(inventory, selected, name) {
  const plot = reviewedPlot(inventory, name);
  return setPlotSelected(inventory, selected, name, !selected.some((candidate) => candidate.checked_plot_id === plot.checked_plot_id));
}

export function setPlotSelected(inventory, selected, name, desired) {
  if (typeof desired !== "boolean") throw new Error("reviewed plot selection must be boolean");
  const plot = reviewedPlot(inventory, name);
  const present = selected.some((candidate) => candidate.checked_plot_id === plot.checked_plot_id);
  if (present === desired) return selected;
  if (!desired) return selected.filter((candidate) => candidate.checked_plot_id !== plot.checked_plot_id);
  if (selected.length >= inventory.maximum_selection) {
    throw new Error("initial Plot selection capacity is exhausted");
  }
  return [...selected, plot];
}

function reviewedPlot(inventory, name) {
  const plot = inventory.plots.find((candidate) => candidate.name === name);
  if (!plot) throw new Error(`reviewed plot ${JSON.stringify(name)} is absent`);
  return plot;
}

export function searchPlots(inventory, query) {
  if (typeof query !== "string" || encoder.encode(query).length > MAXIMUM_SEARCH_BYTES) {
    throw new Error("Plot search is outside its admitted bound");
  }
  const terms = query.trim().toLocaleLowerCase().split(/\s+/u).filter(Boolean);
  return inventory.plots.filter((plot) => {
    const text = `${plot.title} ${plot.name} ${plot.required_kinds.join(" ")}`.toLocaleLowerCase();
    return terms.every((term) => text.includes(term));
  });
}

export function persistedPlotSelection(inventory, selected) {
  return {
    schema: "conduit.creche/plot-selection@1",
    inventory_source_document_id: inventory.source_document_id,
    plots: selected.map(exactIdentity),
  };
}

export function encodedPlotSelection(selected) {
  return JSON.stringify(selected.map(exactIdentity));
}

// Preserve each selected document verbatim; concatenation would invent new
// source identities. Unselected catalog entries need no authoring admission.
export function selectedReviewedSource(source, selected) {
  let bundle;
  try { bundle = JSON.parse(source); } catch { return source; }
  if (bundle?.schema !== 'conduit.creche/reviewed-plot-bundle@2' || !Array.isArray(bundle.plots)) return source;
  const names = new Set(selected.map(plot => plot.name));
  const plots = bundle.plots.filter(plot => names.has(plot.entry ?? plot.slug.replaceAll('-', '_')));
  if (plots.length !== names.size) throw new Error('The selection is absent from its reviewed inventory');
  // An empty Body still records a checked source interaction, with no selected
  // Plots or execution. Retain one existing document as that bounded context.
  return JSON.stringify({ ...bundle, plots: plots.length ? plots : bundle.plots.slice(0, 1) });
}

function exactCurrent(inventory, candidate) {
  if (!candidate || typeof candidate !== "object") return null;
  return inventory.plots.find((plot) => plot.name === candidate.name
    && plot.source_document_id === candidate.source_document_id
    && plot.checked_plot_id === candidate.checked_plot_id) ?? null;
}

function exactIdentity(plot) {
  return {
    name: plot.name,
    source_document_id: plot.source_document_id,
    checked_plot_id: plot.checked_plot_id,
  };
}

function validateInventory(inventory) {
  if (inventory?.schema !== "conduit.creche/reviewed-plot-inventory@2"
    || typeof inventory.source_document_id !== "string"
    || !Number.isSafeInteger(inventory.maximum_selection)
    || inventory.maximum_selection < 0
    || !Array.isArray(inventory.plots)
    || inventory.plots.length > inventory.maximum_selection) {
    throw new Error("reviewed plot inventory is malformed or over capacity");
  }
  const names = new Set();
  const identities = new Set();
  for (const plot of inventory.plots) {
    if (typeof plot?.name !== "string" || typeof plot.title !== "string"
      || typeof plot.source !== "string" || plot.source.length === 0
      || typeof plot.source_document_id !== "string"
      || typeof plot.checked_plot_id !== "string" || !Array.isArray(plot.required_kinds)
      || names.has(plot.name) || identities.has(plot.checked_plot_id)) {
      throw new Error("reviewed plot inventory contains an invalid or duplicate entry");
    }
    names.add(plot.name);
    identities.add(plot.checked_plot_id);
  }
  return inventory;
}
