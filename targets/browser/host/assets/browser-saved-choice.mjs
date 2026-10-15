import { manifestApplicationView } from "./application-face.mjs";
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const SCHEMA = "input/saved-choice@1";

export class BrowserSavedChoiceRefusal extends Error {
  constructor(code, message, cause) {
    super(message, cause ? { cause } : undefined);
    this.name = "BrowserSavedChoiceRefusal";
    this.code = code;
  }
}
const refusal = (code, message, cause) => new BrowserSavedChoiceRefusal(code, message, cause);
function exactText(value, maximum) {
  return typeof value === "string" && value.length > 0 && !value.includes("\0") && encoder.encode(value).length <= maximum;
}
function validate(configuration) {
  if (!configuration || !exactText(configuration.label, 128) || !exactText(configuration.key, 128) ||
      !exactText(configuration.initial, 64) || !Array.isArray(configuration.options) ||
      configuration.options.length < 1 || configuration.options.length > 16) {
    throw refusal("InvalidChoiceConfiguration", "Saved choice configuration exceeds its exact bounds");
  }
  const identities = new Set();
  for (const option of configuration.options) {
    if (!exactText(option?.identity, 64) || !exactText(option?.label, 128) || identities.has(option.identity)) {
      throw refusal("InvalidChoiceConfiguration", "Saved choice options must have unique bounded identities and labels");
    }
    identities.add(option.identity);
  }
  if (!identities.has(configuration.initial)) throw refusal("InvalidChoiceConfiguration", "Initial choice is absent from options");
  return identities;
}
function manifest(configuration, container, onChoose) {
  container.dataset.savedChoice = configuration.key;
  let face;
  face = manifestApplicationView(configuration.view, container, {
    eventCapacity: 1,
    onEvent(event) {
      face.nextEvent();
      onChoose(decoder.decode(event.value));
    },
  });
  const select = container.querySelector("select");
  if (!select) throw refusal("InvalidChoiceView", "Saved choice has no semantic select manifestation");
  const status = container.ownerDocument.createElement("output");
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");
  status.dataset.savedChoiceStatus = configuration.key;
  container.append(status);
  return {
    state(identity, enabled, message) {
      select.value = identity;
      select.disabled = !enabled;
      const label = configuration.options.find(option => option.identity === identity)?.label;
      status.textContent = label ? `${message} — ${label}` : message;
    },
    close() { select.disabled = true; },
  };
}

/** One admitted choice source. Storage is the selected browser application's
 * existing readJson/writeJson interface. The native select is manifested from
 * the Rust semantic NamedSelect view; no application-specific options live here.
 * The optional renderer supports deterministic Host-boundary conformance.
 */
export function openBrowserSavedChoice({ configuration, storage, container, render = manifest }) {
  const identities = validate(configuration);
  if (typeof storage?.readJson !== "function" || typeof storage?.writeJson !== "function") {
    throw refusal("ChoiceStorageUnavailable", "Saved choice requires admitted durable browser storage");
  }
  let closed = false;
  let initialized = false;
  let selected = configuration.initial;
  let pending = null;
  let failure = null;
  let saving = false;
  const storageKey = `saved-choice/${configuration.key}`;
  const view = render(configuration, container, choose);
  view.state(selected, false, "Restoring saved choice…");
  function settle(error, identity) {
    if (!pending) return;
    const current = pending;
    pending = null;
    current.signal?.removeEventListener("abort", current.abort);
    if (error) current.reject(error);
    else current.resolve(encoder.encode(identity));
  }
  function fail(error) {
    if (closed) return;
    failure = error;
    view.state(selected, false, `Not saved: ${error.code}`);
    settle(error);
  }
  async function save(identity) {
    if (!identities.has(identity)) throw refusal("UnknownChoice", "Selected identity is absent from the authored options");
    saving = true;
    view.state(identity, false, "Saving…");
    try { await storage.writeJson(storageKey, { schema: SCHEMA, identity }); }
    catch (error) { throw refusal("ChoiceSaveFailed", "Browser storage did not confirm the saved choice", error); }
    finally { saving = false; }
    if (closed || !pending) return;
    selected = identity;
    view.state(selected, false, "Saved in this browser");
    settle(null, selected);
  }
  async function choose(identity) {
    if (closed || !pending || !initialized || failure || saving) return;
    try { await save(identity); }
    catch (error) { fail(error); }
  }
  async function restore() {
    let record;
    try { record = await storage.readJson(storageKey); }
    catch (error) { throw refusal("ChoiceRestoreFailed", "Saved choice could not be read", error); }
    if (closed || !pending) return;
    if (record === null) {
      initialized = true;
      await save(configuration.initial);
      return;
    }
    if (!record || record.schema !== SCHEMA || !exactText(record.identity, 64) ||
        Object.keys(record).length !== 2) throw refusal("CorruptSavedChoice", "Saved choice record is malformed");
    if (!identities.has(record.identity)) throw refusal("UnknownSavedChoice", "Saved identity is absent from the current authored options");
    initialized = true;
    selected = record.identity;
    view.state(selected, false, "Restored from this browser");
    settle(null, selected);
  }
  function close() {
    if (closed) return;
    closed = true;
    settle(refusal("ChoiceCancelled", "Saved choice request was cancelled"));
    view.close();
  }
  return Object.freeze({
    next(signal) {
      if (closed) return Promise.reject(refusal("ChoiceCancelled", "Saved choice adapter is closed"));
      if (failure) return Promise.reject(failure);
      if (pending) return Promise.reject(refusal("ChoicePressure", "Saved choice already has a pending request"));
      if (signal?.aborted) return Promise.reject(refusal("ChoiceCancelled", "Saved choice request was cancelled"));
      const result = new Promise((resolve, reject) => {
        pending = { resolve, reject, signal, abort: close };
        signal?.addEventListener("abort", close, { once: true });
      });
      if (!initialized) restore().catch(fail);
      else view.state(selected, true, "Saved in this browser");
      return result;
    },
    close,
  });
}
