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
      selected = new Set([...selected].filter((identity) => catalog.entries.some((entry) => entry.implementation_id === identity)));
    }
  }

  return Object.freeze({
    required: true,
    render() {
      const root = document.createElement("section");
      root.className = "browser-configuration";
      const presentation = presentationFor(root);
      if (checked) {
        const slot = presentationSlot(root, "browser-configuration-review");
        presentView(presentation, slot.dataset.applicationSlot, "review", null, (event) => {
          if (event.action === "configuration.edit") {
            checked = null;
            onChange();
          }
        });
        return root;
      }
      const groups = [...new Set(catalog.entries.map((entry) => entry.group))];
      const actionsSlot = presentationSlot(root, "browser-configuration-actions");
      presentView(presentation, actionsSlot.dataset.applicationSlot, "actions", null, (event) => {
        const presets = new Map([
          ["configuration.preset.minimal", ["browser/dom@1", "browser/dom-presentation@1"]],
          ["configuration.preset.interactive", catalog.defaults],
          ["configuration.preset.custom", [...selected]],
        ]);
        if (presets.has(event.action)) {
          selected = new Set(presets.get(event.action));
          checked = null;
          diagnostic = null;
          onChange();
        } else if (event.action === "configuration.review") {
          try { checked = review(host.runtime, selection()); diagnostic = null; }
          catch (error) { diagnostic = error instanceof Error ? error.message : String(error); }
          onChange();
        }
      });
      groups.forEach((group, index) => {
        const groupSlot = presentationSlot(root, `browser-configuration-group-${index}`);
        presentView(presentation, groupSlot.dataset.applicationSlot, "group", group, (event) => {
          const match = /^implementation\.change-(\d+)$/u.exec(event.action);
          const entry = match ? catalog.entries[Number(match[1])] : null;
          if (!entry) throw new Error("browser configuration event is not admitted");
          if (decoder.decode(event.value) === "true") selected.add(entry.implementation_id);
          else selected.delete(entry.implementation_id);
          diagnostic = null;
          onChange();
        });
      });
      return root;

      function presentView(view, slot, mode, group, onEvent) {
        const request = encoder.encode(JSON.stringify({
          revision: ++presentationRevision,
          mode,
          catalog_generation: mode === "review" ? checked.catalog_generation : catalog.generation,
          implementations: [...selected].sort(),
          group,
          diagnostic: mode === "actions" ? diagnostic : null,
        }));
        if (request.length > host.runtime.conduit_creche_input_capacity()) {
          throw new Error("browser configuration presentation request exceeds its bound");
        }
        new Uint8Array(host.runtime.memory.buffer, host.runtime.conduit_creche_input_ptr(), request.length).set(request);
        const code = host.runtime.conduit_creche_browser_configuration_view(request.length);
        if (code < 0) {
          throw outputError(host.runtime, "browser configuration presentation", code);
        }
        const encodedView = new Uint8Array(
          host.runtime.memory.buffer,
          host.runtime.conduit_creche_output_ptr(),
          host.runtime.conduit_creche_output_len(),
        ).slice();
        view.present(slot, encodedView, { onEvent(event) {
          view.nextEvent(slot);
          onEvent(event);
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

function presentationSlot(root, identity) {
  const slot = root.ownerDocument.createElement("div");
  slot.dataset.applicationSlot = identity;
  root.append(slot);
  return slot;
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
