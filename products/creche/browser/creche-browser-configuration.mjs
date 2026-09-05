const CATALOG_SCHEMA = "conduit.creche/browser-configuration-catalog@1";
const REVIEW_SCHEMA = "conduit.creche/checked-browser-configuration@1";
const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function createBrowserConfigurationOutfitter({ host, presentationFor, restoredSelection = null, onChange }) {
  const catalog = call(host.runtime, "conduit_creche_browser_configuration_catalog");
  requireCatalog(catalog);
  let selected = new Set(restoredSelection?.implementations ?? catalog.defaults);
  let checked = null;
  let diagnostic = null;
  let presentationRevision = 0;

  if (restoredSelection) {
    try {
      checked = review(host.runtime, restoredSelection);
      selected = new Set(checked.selected_implementations);
    } catch (error) {
      diagnostic = error instanceof Error ? error.message : String(error);
    }
  }

  return Object.freeze({
    required: true,
    render() {
      const root = document.createElement("section");
      root.className = "browser-configuration";
      const presentation = presentationFor(root);
      if (checked) {
        root.innerHTML = '<div data-application-slot="browser-configuration-review"></div>';
        presentReview(presentation, checked, ++presentationRevision, () => {
          checked = null;
          onChange();
        });
        return root;
      }
      root.innerHTML = '<div data-application-slot="browser-configuration-actions"></div>';
      const groups = [...new Set(catalog.entries.map((entry) => entry.group))];
      groups.forEach((_, index) => root.insertAdjacentHTML("beforeend", `<div data-application-slot="browser-configuration-group-${index}"></div>`));
      presentActions(presentation, ++presentationRevision);
      groups.forEach((group, index) => presentGroup(presentation, group, index, ++presentationRevision));
      return root;

      function presentActions(view, viewRevision) {
        const presets = [
          ["Minimal", ["browser/dom@1", "browser/dom-presentation@1"]],
          ["Interactive", catalog.defaults],
          ["Custom", [...selected]],
        ];
        view.present("browser-configuration-actions", {
          revision: viewRevision,
          actions: [
            ...presets.map((_, index) => ({ id: `preset.${index}`, event: "activate" })),
            { id: "configuration.review", event: "activate" },
          ],
          nodes: [
            { parent: null, component: "stack", action: null, key: "browser-configuration", text: "" },
            { parent: 0, component: "heading", action: null, key: "configuration-heading", text: "Browser Host capabilities" },
            { parent: 0, component: "action-group", action: null, key: "configuration-presets", text: "Configuration presets" },
            ...presets.map(([label], index) => ({ parent: 2, component: "button", action: index, key: `preset-${index}`, text: label })),
            { parent: 0, component: "button", action: 3, key: "review-browser-configuration", text: "Review Host" },
            ...(diagnostic ? [{ parent: 0, component: "failure-status", action: null, key: "configuration-diagnostic", text: diagnostic }] : []),
          ],
        }, { onEvent(event) {
          view.nextEvent("browser-configuration-actions");
          if (event.action.startsWith("preset.")) {
            selected = new Set(presets[Number(event.action.slice(-1))][1]);
            checked = null;
            diagnostic = null;
            onChange();
          }
          if (event.action === "configuration.review") {
            try { checked = review(host.runtime, selection()); diagnostic = null; }
            catch (error) { diagnostic = error instanceof Error ? error.message : String(error); }
            onChange();
          }
        } });
      }

      function presentGroup(view, group, groupIndex, viewRevision) {
        const entries = catalog.entries.filter((entry) => entry.group === group);
        const slot = `browser-configuration-group-${groupIndex}`;
        const nodes = [
          { parent: null, component: "choice-group", action: null, key: `configuration-group-${groupIndex}`, text: `browser_capabilities_${groupIndex}` },
          { parent: 0, component: "choice-group-label", action: null, key: `configuration-group-${groupIndex}-legend`, text: group },
        ];
        entries.forEach((entry, index) => {
          const label = nodes.length;
          nodes.push({ parent: 0, component: "choice-option-label", action: null, key: `implementation-label-${index}`, text: `${entry.label} · ${entry.implementation_id}` });
          nodes.push({ parent: label, component: "independent-choice", action: index, key: `implementation-${index}`, text: entry.implementation_id, value: String(selected.has(entry.implementation_id)), valueCapacity: 5 });
          entry.runtime_prerequisites.forEach((item, prerequisiteIndex) => nodes.push({
            parent: 0,
            component: "paragraph",
            action: null,
            key: `prerequisite-${index}-${prerequisiteIndex}`,
            text: `Future runtime condition: ${item.detail}. Not claimed satisfied here.`,
          }));
        });
        view.present(slot, {
          revision: viewRevision,
          actions: entries.map((_, index) => ({ id: `implementation.change-${index}`, event: "change" })),
          nodes,
        }, { onEvent(event) {
          view.nextEvent(slot);
          const entry = entries[Number(event.action.split("-").at(-1))];
          if (decoder.decode(event.value) === "true") selected.add(entry.implementation_id);
          else selected.delete(entry.implementation_id);
          diagnostic = null;
          onChange();
        } });
      }
    },
    checked: () => checked,
    selection,
  });

  function selection() {
    return Object.freeze({
      catalog_generation: restoredSelection?.catalog_generation ?? catalog.generation,
      implementations: Object.freeze([...selected].sort()),
    });
  }
}

export function prepareCheckedBrowserSpore({ host, checked, selection, imageDigest, nowMillis, entropy }) {
  if (!checked || checked.schema !== REVIEW_SCHEMA) throw new TypeError("checked browser configuration is required");
  const digest = encoder.encode(imageDigest);
  const selectionBytes = encoder.encode(JSON.stringify(selection));
  const input = new Uint8Array(host.runtime.memory.buffer, host.runtime.conduit_creche_input_ptr(), 32 + digest.length + selectionBytes.length);
  input.set(entropy);
  input.set(digest, 32);
  input.set(selectionBytes, 32 + digest.length);
  const code = host.runtime.conduit_creche_prepare_selected_browser_spore(digest.length, selectionBytes.length, BigInt(nowMillis));
  if (code < 0) throw outputError(host.runtime, "browser configuration handoff", code);
  const prepared = readOutput(host.runtime);
  if (prepared.browser_configuration_id !== checked.configuration_id
    || prepared.browser_profile_id !== checked.profile_id
    || prepared.browser_configuration_source !== checked.canonical_source) {
    throw new Error("browser fabrication did not consume the exact reviewed configuration");
  }
  return prepared;
}

function review(runtime, selection) {
  const bytes = encoder.encode(JSON.stringify(selection));
  const input = new Uint8Array(runtime.memory.buffer, runtime.conduit_creche_input_ptr(), bytes.length);
  input.set(bytes);
  const code = runtime.conduit_creche_review_browser_configuration(bytes.length);
  if (code < 0) throw outputError(runtime, "browser Host configuration review", code);
  const value = readOutput(runtime);
  if (value.schema !== REVIEW_SCHEMA) throw new TypeError("browser Host review schema is incompatible");
  return Object.freeze(value);
}

function presentReview(presentation, checked, revision, edit) {
  const values = [
    ["Target", checked.target_id],
    ["Implementations", checked.selected_implementations.join(", ") || "none"],
    ["PROFILE", checked.profile_id],
    ["BrowserBundle output", checked.output],
    ["Body/Spore join", checked.join_mode],
  ];
  presentation.present("browser-configuration-review", {
    revision,
    actions: [{ id: "configuration.edit", event: "activate" }],
    nodes: [
      { parent: null, component: "successful-evidence", action: null, key: "configuration-review", text: "Reviewed browser Host configuration" },
      { parent: 0, component: "definition-table", action: null, key: "configuration-review-values", text: "Exact configuration identities" },
      ...values.map(([term, value], index) => ({ parent: 1, component: "definition", action: null, key: `review-${index}`, text: term, value, valueCapacity: 65_536 })),
      { parent: 0, component: "code-block", action: null, key: "configuration-source", text: "conduit", value: checked.canonical_source, valueCapacity: 65_536 },
      { parent: 0, component: "paragraph", action: null, key: "configuration-absent", text: `Configuration creates no ${checked.does_not_create.join(", ")}.` },
      { parent: 0, component: "button", action: 0, key: "edit-browser-configuration", text: "Back / Edit" },
    ],
  }, { onEvent() { presentation.nextEvent("browser-configuration-review"); edit(); } });
}

function requireCatalog(catalog) {
  if (catalog?.schema !== CATALOG_SCHEMA || !Number.isSafeInteger(catalog.generation)
    || !Array.isArray(catalog.entries) || !Array.isArray(catalog.defaults)) {
    throw new TypeError("browser configuration catalog contract is incomplete");
  }
}

function call(runtime, name) {
  const code = runtime[name]();
  if (code < 0) throw outputError(runtime, name, code);
  return readOutput(runtime);
}

function readOutput(runtime) {
  return JSON.parse(decoder.decode(new Uint8Array(runtime.memory.buffer, runtime.conduit_creche_output_ptr(), runtime.conduit_creche_output_len())));
}

function outputError(runtime, operation, code) {
  let message = `${operation} refused (${code})`;
  try { message = readOutput(runtime).message ?? message; } catch {}
  return new Error(message);
}
