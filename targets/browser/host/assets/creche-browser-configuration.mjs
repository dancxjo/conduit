const CATALOG_SCHEMA = "conduit.creche/browser-configuration-catalog@1";
const REVIEW_SCHEMA = "conduit.creche/checked-browser-configuration@1";
const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function createBrowserConfigurationOutfitter({ host, restoredSelection = null, onChange }) {
  const catalog = call(host.runtime, "conduit_creche_browser_configuration_catalog");
  requireCatalog(catalog);
  let selected = new Set(restoredSelection?.implementations ?? catalog.defaults);
  let checked = null;
  let diagnostic = null;

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
      root.innerHTML = "<h3>Browser Host capabilities</h3>";
      if (checked) {
        renderReview(root, checked, () => {
          checked = null;
          onChange();
        });
        return root;
      }
      const presets = document.createElement("div");
      presets.className = "browser-configuration-presets";
      presets.append(
        preset("Minimal", ["browser/dom@1", "browser/dom-presentation@1"]),
        preset("Interactive", catalog.defaults),
        preset("Custom", [...selected]),
      );
      root.append(presets);
      for (const group of [...new Set(catalog.entries.map((entry) => entry.group))]) {
        const fieldset = document.createElement("fieldset");
        const legend = document.createElement("legend");
        legend.textContent = group;
        fieldset.append(legend);
        for (const entry of catalog.entries.filter((candidate) => candidate.group === group)) {
          const label = document.createElement("label");
          const input = document.createElement("input");
          input.type = "checkbox";
          input.value = entry.implementation_id;
          input.checked = selected.has(entry.implementation_id);
          input.addEventListener("change", () => {
            if (input.checked) selected.add(input.value); else selected.delete(input.value);
            diagnostic = null;
          });
          label.append(input, ` ${entry.label} · ${entry.implementation_id}`);
          if (entry.runtime_prerequisites.length) {
            const note = document.createElement("small");
            note.textContent = ` Future runtime conditions: ${entry.runtime_prerequisites.map((item) => item.detail).join("; ")}. None is claimed satisfied here.`;
            label.append(note);
          }
          fieldset.append(label);
        }
        root.append(fieldset);
      }
      const button = document.createElement("button");
      button.type = "button";
      button.className = "review-browser-configuration";
      button.textContent = "Review Host";
      button.addEventListener("click", () => {
        try {
          checked = review(host.runtime, selection());
          diagnostic = null;
          onChange();
        } catch (error) {
          diagnostic = error instanceof Error ? error.message : String(error);
          onChange();
        }
      });
      root.append(button);
      if (diagnostic) {
        const failure = document.createElement("p");
        failure.className = "browser-configuration-diagnostic";
        failure.setAttribute("role", "alert");
        failure.textContent = diagnostic;
        root.append(failure);
      }
      return root;
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

  function preset(label, implementations) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = label;
    button.addEventListener("click", () => {
      selected = new Set(implementations);
      checked = null;
      diagnostic = null;
      onChange();
    });
    return button;
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

function renderReview(root, checked, edit) {
  const summary = document.createElement("dl");
  summary.className = "browser-configuration-review";
  for (const [term, value] of [
    ["Target", checked.target_id],
    ["Implementations", checked.selected_implementations.join(", ") || "none"],
    ["PROFILE", checked.profile_id],
    ["BrowserBundle output", checked.output],
    ["Body/Spore join", checked.join_mode],
  ]) {
    const dt = document.createElement("dt"); dt.textContent = term;
    const dd = document.createElement("dd"); dd.textContent = value;
    summary.append(dt, dd);
  }
  const source = document.createElement("pre"); source.textContent = checked.canonical_source;
  const absent = document.createElement("p");
  absent.textContent = `Configuration creates no ${checked.does_not_create.join(", ")}.`;
  const button = document.createElement("button");
  button.type = "button"; button.className = "edit-browser-configuration"; button.textContent = "Back / Edit";
  button.addEventListener("click", edit);
  root.append(summary, source, absent, button);
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
