// Route acquired input to the foreground Form inside one immutable Body Plan.
// This only delivers Host observations; it never advances or replaces Play.
export function createBodyInputRouting({ forms, foreground }) {
  if (!Array.isArray(forms) || forms.length < 1 || forms.length > 16 || typeof foreground !== "function") {
    throw new Error("invalid Body input routing inventory");
  }
  const placementForms = new Map();
  const formIds = new Set();
  for (const partition of forms) {
    const form = partition.form?.checked_form_id;
    if (typeof form !== "string" || !form || form.length > 256 || formIds.has(form)) throw new Error("invalid Body input Form identity");
    formIds.add(form);
    for (const fragment of partition.plan.fragments) for (const placement of fragment.placements) {
      if (typeof placement.placement_id !== "string" || !placement.placement_id || placement.placement_id.length > 256 || placementForms.has(placement.placement_id) || placementForms.size === 16) throw new Error("Body input placement bound exceeded");
      placementForms.set(placement.placement_id, form);
    }
  }
  const waiters = [], queued = [];
  const heldKeys = new Array(256).fill(null);
  const pumping = new Set();
  let heldButton = null, input = null, terminal = null;
  const fail = error => {
    if (terminal) return;
    terminal = error;
    queued.length = 0;
    for (const waiter of waiters.splice(0)) { waiter.dispose(); waiter.reject(error); }
  };
  const selected = () => {
    const form = foreground();
    if (!formIds.has(form)) throw new Error("foreground Form is outside the admitted Body Plan");
    return form;
  };
  const deliver = (kind, event) => {
    if (!formIds.has(event.delivery_form)) throw new Error("input lacks its captured Form identity");
    const index = waiters.findIndex(waiter => waiter.kind === kind && waiter.form === event.delivery_form);
    if (index >= 0) {
      const waiter = waiters.splice(index, 1)[0];
      waiter.dispose(); waiter.resolve(event);
    } else {
      if (queued.length === 8) throw new Error("foreground input queue capacity exhausted");
      queued.push({ kind, event });
    }
  };
  const pump = async kind => {
    if (pumping.has(kind)) return;
    pumping.add(kind);
    try {
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
        if (!(value instanceof Uint8Array) || value.length !== 3 || value[1] > 1) throw new Error("invalid routed key event");
        const [usage, phase] = value;
        const form = heldKeys[usage] ?? selected();
        heldKeys[usage] = phase === 0 ? form : null;
        return form;
      }
      if (kind === "button") {
        const form = heldButton ?? selected();
        heldButton = value.pressed ? form : null;
        return form;
      }
      throw new Error("unsupported routed input kind");
    },
    attach(acquired) {
      if (input || terminal) throw new Error("Body input routing already acquired or closed");
      input = acquired;
    },
    next(kind, placement, signal) {
      if (terminal) return Promise.reject(terminal);
      if (!input || !["keyboard", "button"].includes(kind) || !placementForms.has(placement)) {
        return Promise.reject(new Error("input request is outside the admitted Body Plan"));
      }
      if (signal.aborted) return Promise.reject(new Error("Body input request cancelled"));
      if (waiters.some(waiter => waiter.placement === placement)) {
        return Promise.reject(new Error("Body input placement already has a pending request"));
      }
      const form = placementForms.get(placement);
      const index = queued.findIndex(item => item.kind === kind && item.event.delivery_form === form);
      if (index >= 0) return Promise.resolve(queued.splice(index, 1)[0].event);
      if (waiters.length === 16) {
        return Promise.reject(new Error("Body input pending request capacity exceeded"));
      }
      const pending = new Promise((resolve, reject) => {
        const abort = () => {
          const index = waiters.indexOf(waiter);
          if (index >= 0) waiters.splice(index, 1);
          waiter.dispose(); reject(new Error("Body input request cancelled"));
        };
        const waiter = { kind, form, placement, resolve, reject, dispose: () => signal.removeEventListener("abort", abort) };
        waiters.push(waiter);
        signal.addEventListener("abort", abort, { once: true });
      });
      void pump(kind);
      return pending;
    },
    close() { fail(new Error("Body input routing closed")); },
  });
}
