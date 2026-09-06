const decoder = new TextDecoder("utf-8", { fatal: true });

export function createTourStatus(presentation, slot, key, initialText) {
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

export function createTourRunnerStatus(presentation, slot, initialText) {
  return createTourStatus(presentation, slot, "play-status", initialText);
}

export function restoreTourRunnerDraft({ runner, textarea, source, sourceKey, readingState, syntaxEditor, cancel, refresh }) {
  cancel();
  readingState.drafts.delete(sourceKey);
  readingState.persist();
  textarea.value = source;
  syntaxEditor.render();
  refresh(source);
  runner.querySelector(".morse").textContent = "ready";
  runner.querySelector(".indicator")?.setAttribute("aria-label", "Indicator off");
  runner.evidence.clearRun();
  runner.playStatus.ordinary("Canonical source restored. No Play is active.");
  textarea.focus({ preventScroll: true });
}

export function createTourRunnerField(presentation, slot, listingId, label, source, onInput) {
  let revision = 0;
  presentation.present(slot, {
    revision: ++revision,
    actions: [{ id: "tour.source.input", event: "input" }],
    nodes: [
      { parent: null, component: "stack", key: "source-editor", text: "", action: null },
      { parent: 0, component: "form-field", key: "source-field", text: "", action: null },
      {
        parent: 1, component: "textarea", key: listingId, text: label,
        value: source, valueCapacity: 65_536, action: 0,
      },
      { parent: 1, component: "field-label", key: "source-label", text: label, action: null },
      {
        parent: 1, component: "field-help", key: "source-help",
        text: "Editing checks this Form without starting a Play.", action: null,
      },
    ],
  }, {
    onEvent(event) {
      presentation.nextEvent(slot);
      if (event.action === "tour.source.input") onInput(decoder.decode(event.value));
    },
  });
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
    throw new Error("Tour evidence definition count exceeds its admitted bound");
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

export function createTourEvidenceTables(presentation, exactSlot, runSlot) {
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
    clearRun() {
      runEntries = [];
      presentDefinitions(presentation, runSlot, ++runRevision, "Latest run identities", "run", [
        ["Latest run", "none since canonical source was restored"],
      ]);
    },
  });
}

const MAXIMUM_PROJECTED_HOSTS = 4;
const MAXIMUM_GEARS_PER_HOST = 4;
const MAXIMUM_RAW_PLAN_BYTES = 65_536;

export function createTourPlanPresentation(presentation, slot) {
  let revision = 0;
  return Object.freeze({
    render(plan) {
      if (!Array.isArray(plan.hosts) || plan.hosts.length === 0 || plan.hosts.length > MAXIMUM_PROJECTED_HOSTS
        || plan.hosts.some((host) => !Array.isArray(host.gears)
          || host.gears.length === 0 || host.gears.length > MAXIMUM_GEARS_PER_HOST)) {
        throw new Error("Tour Plan projection exceeds its admitted Host or Gear bound");
      }
      const rawPlan = JSON.stringify(plan.raw_plan, null, 2);
      if (new TextEncoder().encode(rawPlan).length > MAXIMUM_RAW_PLAN_BYTES) {
        throw new Error("Tour raw Plan evidence exceeds its admitted byte bound");
      }

      const nodes = [
        { parent: null, component: "panel", key: "plan", text: "", action: null },
        { parent: 0, component: "paragraph", key: "explanation", text: plan.explanation, action: null },
        { parent: 0, component: "code", key: "plan-id", text: plan.plan_id, action: null },
        { parent: 0, component: "grid", key: "hosts", text: "", action: null },
      ];
      for (const [hostIndex, projected] of plan.hosts.entries()) {
        const card = nodes.length;
        nodes.push({
          parent: 3, component: "artifact", key: `host-${hostIndex}`,
          text: `${projected.label} · one Play`, action: null,
        });
        nodes.push({
          parent: card, component: "code", key: `host-${hostIndex}-identity`,
          text: projected.host_id, action: null,
        });
        const gears = nodes.length;
        nodes.push({
          parent: card, component: "definition-table", key: `host-${hostIndex}-gears`,
          text: `${projected.label} selected Gears`, action: null,
        });
        for (const [gearIndex, gear] of projected.gears.entries()) {
          nodes.push({
            parent: gears, component: "definition", key: `host-${hostIndex}-gear-${gearIndex}`,
            text: gear.kind_id, value: gear.implementation_id, valueCapacity: 4096, action: null,
          });
        }
      }
      nodes.push({
        parent: 0, component: "paragraph", key: "cord",
        text: `Cross-Host ${plan.cord.value_kind} Cord · ${plan.cord.line_id} · ${plan.cord.maximum_in_flight_items} item / ${plan.cord.maximum_payload_bytes} bytes`,
        action: null,
      });
      const raw = nodes.length;
      nodes.push({ parent: 0, component: "artifact", key: "raw-plan", text: "Raw Plan evidence", action: null });
      nodes.push({
        parent: raw, component: "code-block", key: "raw-plan-json", text: "json",
        value: rawPlan, valueCapacity: MAXIMUM_RAW_PLAN_BYTES, action: null,
      });
      presentation.present(slot, { revision: ++revision, actions: [], nodes });
    },
  });
}
