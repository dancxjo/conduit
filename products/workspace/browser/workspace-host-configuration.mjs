import { createBrowserConfigurationOutfitter as createExactOutfitter } from "../../creche/browser/creche-browser-configuration.mjs";

const CATALOG_SCHEMA = "conduit.host/browser-capability-intent-catalog@1";
const REVIEW_SCHEMA = "conduit.host/browser-capability-configuration-review@1";
const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function createBrowserConfigurationOutfitter({ host, presentationFor, onChange }) {
  const catalog = call(host.runtime, "conduit_browser_host_capability_intent_catalog");
  requireCatalog(catalog);
  const selected = new Set(catalog.defaults);
  let exact = null;

  return Object.freeze({
    required: true,
    render() {
      const root = document.createElement("section");
      root.className = "browser-capability-intent";
      if (exact) {
        const change = document.createElement("button");
        change.type = "button";
        change.textContent = "Change desired capabilities";
        change.addEventListener("click", () => { exact = null; onChange(); });
        root.append(exact.render(), change);
        return root;
      }
      const target = document.createElement("p");
      target.textContent = `Target: Browser page (${catalog.target_id}) · bind reviewed superset`;
      const fieldset = document.createElement("fieldset");
      const legend = document.createElement("legend"); legend.textContent = "What should this Host help with?";
      fieldset.append(legend);
      for (const entry of catalog.entries) {
        const label = document.createElement("label");
        const input = document.createElement("input"); input.type = "checkbox";
        input.checked = selected.has(entry.kind);
        input.addEventListener("change", () => input.checked ? selected.add(entry.kind) : selected.delete(entry.kind));
        label.append(input, document.createTextNode(entry.label));
        fieldset.append(label);
      }
      const resolve = document.createElement("button"); resolve.type = "button"; resolve.textContent = "Resolve reviewed Bases";
      resolve.addEventListener("click", () => {
        const review = reviewIntent(host.runtime, catalog.generation, [...selected]);
        exact = createExactOutfitter({ host, presentationFor, restoredSelection: review.configuration_selection, onChange });
        onChange();
      });
      root.append(target, fieldset, resolve);
      return root;
    },
    checked: () => exact?.checked() ?? null,
    selection: () => exact?.selection() ?? null,
  });
}

function reviewIntent(runtime, generation, kinds) {
  if (kinds.length < 1 || kinds.length > 32) throw new RangeError("Choose between one and 32 Host capabilities");
  const bytes = encoder.encode(JSON.stringify({ catalog_generation: generation, capabilities: kinds.sort().map(kind => ({ kind })) }));
  if (bytes.length > runtime.conduit_creche_input_capacity()) throw new RangeError("Host capability intent exceeds its admitted bound");
  new Uint8Array(runtime.memory.buffer, runtime.conduit_creche_input_ptr(), bytes.length).set(bytes);
  const code = runtime.conduit_browser_host_review_capability_intent(bytes.length);
  if (code < 0) throw outputError(runtime, "Host capability intent review", code);
  const review = readOutput(runtime);
  if (review?.schema !== REVIEW_SCHEMA || !review.configuration_selection) throw new TypeError("Host capability intent review is incompatible");
  return review;
}

function requireCatalog(catalog) {
  if (catalog?.schema !== CATALOG_SCHEMA || !Number.isSafeInteger(catalog.generation)
      || catalog.generation < 1 || typeof catalog.target_id !== "string"
      || !Array.isArray(catalog.defaults) || !Array.isArray(catalog.entries)
      || catalog.entries.length < 1 || catalog.entries.length > 32) {
    throw new TypeError("Host capability intent catalog is malformed or over capacity");
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
