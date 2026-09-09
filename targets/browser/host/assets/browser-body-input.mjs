// Route acquired input to the foreground Form inside one immutable Body Plan.
// This only delivers Host observations; it never advances or replaces Play.
import { BrowserInputRefusal } from "./browser-human-input.mjs";
export function createBodyInputRouting({ forms, foreground }) {
  if (!Array.isArray(forms) || forms.length < 1 || forms.length > 16 || typeof foreground !== "function") {
    throw new BrowserInputRefusal("InvalidInventory", "invalid Body input routing inventory");
  }
  const placementForms = new Map();
  const placementInputs = new Map();
  const inputKinds = new Map([["input/keyboard", "keyboard"], ["input/button", "button"], ["input/pointer-source", "pointer"]]);
  const formIds = new Set();
  for (const partition of forms) {
    const form = partition.form?.checked_form_id;
    if (typeof form !== "string" || !form || form.length > 256 || formIds.has(form)) throw new BrowserInputRefusal("InvalidFormIdentity", "invalid Body input Form identity");
    formIds.add(form);
    for (const fragment of partition.plan.fragments) for (const placement of fragment.placements) {
      if (typeof placement.placement_id !== "string" || !placement.placement_id || placement.placement_id.length > 256 || placementForms.has(placement.placement_id) || placementForms.size === 16) throw new BrowserInputRefusal("PlacementBound", "Body input placement bound exceeded");
      placementForms.set(placement.placement_id, form);
      if (inputKinds.has(placement.kind_id)) placementInputs.set(placement.placement_id, inputKinds.get(placement.kind_id));
    }
  }
  const waiters = [], queued = [];
  const heldKeys = new Array(256).fill(null);
  const pumping = new Set();
  let heldButton = null, input = null, terminal = null, stopPointer = null;
  const fail = error => {
    if (terminal) return;
    terminal = error;
    stopPointer?.();
    queued.length = 0;
    for (const waiter of waiters.splice(0)) { waiter.dispose(); waiter.reject(error); }
  };
  const selected = () => {
    const form = foreground();
    if (!formIds.has(form)) throw new BrowserInputRefusal("StaleForm", "foreground Form is outside the admitted Body Plan");
    return form;
  };
  const accepts = (form, kind) => {
    for (const [placement, inputKind] of placementInputs) if (inputKind === kind && placementForms.get(placement) === form) return true;
    return false;
  };
  const deliver = (kind, event) => {
    if (!formIds.has(event.delivery_form)) throw new BrowserInputRefusal("StaleForm", "input lacks its captured Form identity");
    const index = waiters.findIndex(waiter => waiter.kind === kind && waiter.form === event.delivery_form);
    if (index >= 0) {
      const waiter = waiters.splice(index, 1)[0];
      waiter.dispose(); waiter.resolve(event);
    } else {
      if (queued.length === 8) throw new BrowserInputRefusal("Pressure", "foreground input queue capacity exhausted");
      queued.push({ kind, event });
    }
  };
  const pump = async kind => {
    if (pumping.has(kind)) return;
    pumping.add(kind);
    try {
      if (kind === "pointer") {
        stopPointer = input.observePointer((event, error) => {
          if (terminal) return;
          try { if (error) throw error; deliver(kind, event); }
          catch (failure) { fail(failure); }
        });
        return;
      }
      while (!terminal) {
        const event = await (kind === "keyboard" ? input.nextKeyboard() : input.nextButton());
        if (!terminal) deliver(kind, event);
      }
    } catch (error) { fail(error); }
  };
  return Object.freeze({
    // Called at physical capture, before a queued event can outlive focus.
    capture(kind, value) {
      if (terminal) throw terminal;
      if (kind === "keyboard") {
        if (!(value instanceof Uint8Array) || value.length !== 3 || value[1] > 1) throw new BrowserInputRefusal("InvalidInput", "invalid routed key event");
        const [usage, phase] = value;
        const form = heldKeys[usage] ?? selected();
        if (!accepts(form, kind)) return null;
        heldKeys[usage] = phase === 0 ? form : null;
        return form;
      }
      if (kind === "button") {
        const form = heldButton ?? selected();
        if (!accepts(form, kind)) return null;
        heldButton = value.pressed ? form : null;
        return form;
      }
      if (kind === "pointer") {
        const form = selected();
        return accepts(form, kind) ? form : null;
      }
      throw new BrowserInputRefusal("UnsupportedInput", "unsupported routed input kind");
    },
    attach(acquired) {
      if (input || terminal) throw new BrowserInputRefusal("InvalidState", "Body input routing already acquired or closed");
      input = acquired;
    },
    next(kind, placement, signal) {
      if (terminal) return Promise.reject(terminal);
      if (!input || placementInputs.get(placement) !== kind) {
        return Promise.reject(new BrowserInputRefusal("StalePlacement", "input request is outside the admitted Body Plan"));
      }
      if (signal.aborted) return Promise.reject(new BrowserInputRefusal("Cancelled", "Body input request cancelled"));
      if (waiters.some(waiter => waiter.placement === placement)) {
        return Promise.reject(new BrowserInputRefusal("DuplicateRequest", "Body input placement already has a pending request"));
      }
      const form = placementForms.get(placement);
      const index = queued.findIndex(item => item.kind === kind && item.event.delivery_form === form);
      if (index >= 0) return Promise.resolve(queued.splice(index, 1)[0].event);
      if (waiters.length === 16) {
        return Promise.reject(new BrowserInputRefusal("Pressure", "Body input pending request capacity exceeded"));
      }
      const pending = new Promise((resolve, reject) => {
        const abort = () => {
          const index = waiters.indexOf(waiter);
          if (index >= 0) waiters.splice(index, 1);
          waiter.dispose(); reject(new BrowserInputRefusal("Cancelled", "Body input request cancelled"));
        };
        const waiter = { kind, form, placement, resolve, reject, dispose: () => signal.removeEventListener("abort", abort) };
        waiters.push(waiter);
        signal.addEventListener("abort", abort, { once: true });
      });
      void pump(kind);
      return pending;
    },
    close() { fail(new BrowserInputRefusal("Cancelled", "Body input routing closed")); },
  });
}
