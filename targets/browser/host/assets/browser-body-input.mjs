// Route acquired input to the foreground Form inside one immutable Body Plan.
// This only delivers Host observations; it never advances or replaces Play.
import { BrowserInputRefusal } from "./browser-human-input.mjs";
export function createBodyInputRouting({ forms, foreground, maximumPlacements }) {
  if (!Array.isArray(forms) || forms.length < 1 || forms.length > 16 || typeof foreground !== "function" ||
      !Number.isSafeInteger(maximumPlacements) || maximumPlacements < 1) {
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
      if (typeof placement.placement_id !== "string" || !placement.placement_id || placement.placement_id.length > 256 || placementForms.has(placement.placement_id) || placementForms.size === maximumPlacements) throw new BrowserInputRefusal("PlacementBound", "Body input placement bound exceeded");
      placementForms.set(placement.placement_id, form);
      if (inputKinds.has(placement.kind_id)) placementInputs.set(placement.placement_id, inputKinds.get(placement.kind_id));
    }
  }
  const waiters = [];
  const queues = new Map();
  const streamFailures = new Map();
  const pressure = new Map();
  const heldKeys = new Array(256).fill(null);
  const pumping = new Set();
  let heldButton = null, input = null, terminal = null, stopPointer = null;
  const fail = error => {
    if (terminal) return;
    terminal = error;
    stopPointer?.();
    queues.clear();
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
  const streamKey = (kind, form) => `${kind}\u0000${form}`;
  const streamPressure = (kind, form) => {
    const key = streamKey(kind, form);
    let state = pressure.get(key);
    if (!state) {
      state = { kind, form, capacity: kind === "pointer" ? 1 : 8, accepted: 0, delivered: 0, coalesced: 0, dropped: 0, refusals: 0 };
      pressure.set(key, state);
    }
    return state;
  };
  const queueFor = (kind, form) => {
    const key = streamKey(kind, form);
    let queue = queues.get(key);
    if (!queue) {
      const capacity = streamPressure(kind, form).capacity;
      queue = { items: new Array(capacity).fill(null), head: 0, length: 0 };
      queues.set(key, queue);
    }
    return queue;
  };
  const clearQueue = (queue) => {
    queue.items.fill(null);
    queue.head = 0;
    queue.length = 0;
  };
  const enqueue = (queue, event) => {
    queue.items[(queue.head + queue.length) % queue.items.length] = event;
    queue.length += 1;
  };
  const dequeue = (queue) => {
    const event = queue.items[queue.head];
    queue.items[queue.head] = null;
    queue.head = (queue.head + 1) % queue.items.length;
    queue.length -= 1;
    return event;
  };
  const increment = (state, field, amount = 1) => {
    const next = state[field] + amount;
    if (!Number.isSafeInteger(next)) throw new BrowserInputRefusal("SequenceOverflow", "input pressure evidence overflowed");
    state[field] = next;
  };
  const failStream = (kind, form, error) => {
    const key = streamKey(kind, form);
    streamFailures.set(key, error);
    const queue = queues.get(key);
    if (queue) clearQueue(queue);
    increment(streamPressure(kind, form), "refusals");
    for (let index = waiters.length - 1; index >= 0; index -= 1) {
      const waiter = waiters[index];
      if (waiter.kind !== kind || waiter.form !== form) continue;
      waiters.splice(index, 1);
      waiter.dispose();
      waiter.reject(error);
    }
  };
  const coalescePointer = (older, newer, state) => {
    const add = (left, right) => {
      const value = left + right;
      if (!Number.isSafeInteger(value)) throw new BrowserInputRefusal("SequenceOverflow", "coalesced pointer evidence overflowed");
      return value;
    };
    increment(state, "coalesced");
    return Object.freeze({
      ...newer,
      delta_x: add(older.delta_x ?? 0, newer.delta_x ?? 0),
      delta_y: add(older.delta_y ?? 0, newer.delta_y ?? 0),
      coalesced: add(add(older.coalesced ?? 0, newer.coalesced ?? 0), 1),
      dropped: add(older.dropped ?? 0, newer.dropped ?? 0),
      queue_capacity: 1,
    });
  };
  const deliver = (kind, event) => {
    if (!formIds.has(event.delivery_form)) throw new BrowserInputRefusal("StaleForm", "input lacks its captured Form identity");
    const form = event.delivery_form;
    const key = streamKey(kind, form);
    if (streamFailures.has(key)) return;
    const state = streamPressure(kind, form);
    increment(state, "accepted");
    if (kind === "pointer") {
      increment(state, "coalesced", event.coalesced ?? 0);
      increment(state, "dropped", event.dropped ?? 0);
    }
    const index = waiters.findIndex(waiter => waiter.kind === kind && waiter.form === event.delivery_form);
    if (index >= 0) {
      const waiter = waiters.splice(index, 1)[0];
      increment(state, "delivered");
      waiter.dispose(); waiter.resolve(event);
    } else {
      const queue = queueFor(kind, form);
      if (kind === "pointer" && queue.length === 1) {
        try { queue.items[queue.head] = coalescePointer(queue.items[queue.head], event, state); }
        catch (error) { failStream(kind, form, error); }
        return;
      }
      if (queue.length === state.capacity) {
        failStream(kind, form, new BrowserInputRefusal("Pressure", `ordered ${kind} stream capacity exhausted`));
        return;
      }
      enqueue(queue, event);
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
      const key = streamKey(kind, form);
      const failed = streamFailures.get(key);
      if (failed) return Promise.reject(failed);
      const queue = queues.get(key);
      if (queue?.length) {
        const event = dequeue(queue);
        increment(streamPressure(kind, form), "delivered");
        return Promise.resolve(event);
      }
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
    pressure() {
      return Object.freeze([...pressure.values()].map(state => Object.freeze({
        ...state,
        occupancy: queues.get(streamKey(state.kind, state.form))?.length ?? 0,
        terminal: streamFailures.get(streamKey(state.kind, state.form))?.code ?? null,
      })));
    },
    close() { fail(new BrowserInputRefusal("Cancelled", "Body input routing closed")); },
  });
}
