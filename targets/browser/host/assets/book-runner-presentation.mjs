export function createBookStatus(presentation, slot, key, initialText) {
  let revision = 0;
  const render = (component, text) => presentation.present(slot, {
    revision: ++revision,
    actions: [],
    nodes: [
      { parent: null, component, key, text, action: null },
    ],
  });
  const status = Object.freeze({
    ordinary(text) { return render("status", text); },
    success(text) { return render("success-status", text); },
    failure(text) { return render("failure-status", text); },
  });
  queueMicrotask(() => status.ordinary(initialText));
  return status;
}

export function createBookRunnerStatus(presentation, slot, initialText) {
  return createBookStatus(presentation, slot, "play-status", initialText);
}

const RUN_IDENTITIES = Object.freeze([
  ["source_document_id", "Source document"], ["checked_form_id", "Checked Form"],
  ["expanded_form_id", "Expanded Form"], ["plan_id", "Plan"],
  ["fragment_id", "Plan fragment"], ["active_play_id", "Active Play"],
  ["presentation_id", "Presentation"], ["placement_id", "Placement"],
  ["host_id", "Host"], ["boot_id", "Boot"],
]);
const MAXIMUM_EVIDENCE_DEFINITIONS = 16;

function presentDefinitions(presentation, slot, revision, label, key, entries) {
  if (entries.length === 0 || entries.length > MAXIMUM_EVIDENCE_DEFINITIONS) {
    throw new Error("Book evidence definition count exceeds its admitted bound");
  }
  const nodes = [{ parent: null, component: "definition-table", key, text: label, action: null }];
  for (const [index, [name, value]] of entries.entries()) {
    nodes.push({
      parent: 0, component: "definition", key: `${key}-${index}`,
      text: name, value: String(value), valueCapacity: 4096, action: null,
    });
  }
  presentation.present(slot, { revision, actions: [], nodes });
}

export function createBookEvidenceTables(presentation, exactSlot, runSlot) {
  let exactRevision = 0;
  let runRevision = 0;
  let runEntries = [];
  return Object.freeze({
    projection(projection) {
      presentDefinitions(presentation, exactSlot, ++exactRevision, "Checked Form identities", "projection", [
        ["Source", projection.source_document_id],
        ["Checked Form", projection.checked_form_id],
        ["Visible expansion", projection.visible_expanded_form_id || "not available — source is invalid"],
        ["Realization expansion", projection.realization_expanded_form_id || "not available — source is invalid"],
        ["Realization", projection.realization],
        ["Opened Backs", projection.realization_backs.length],
      ]);
    },
    run(effect) {
      runEntries = RUN_IDENTITIES.map(([key, label]) => [label, effect[key]]);
      if (effect.source_interaction) runEntries.push(
        ["Source interaction proposal", effect.source_interaction.proposal_identity],
        ["Source interaction result", effect.source_interaction.result_identity],
      );
      presentDefinitions(presentation, runSlot, ++runRevision, "Latest run identities", "run", runEntries);
    },
    appendRun(entries) {
      runEntries.push(...entries);
      presentDefinitions(presentation, runSlot, ++runRevision, "Latest run identities", "run", runEntries);
    },
  });
}
