// Route acquired input to the foreground Plot inside one immutable Body Plan.
// This only delivers Host observations; it never advances or replaces Play.
import { BrowserInputRefusal } from "./browser-human-input.mjs";
export function createBodyInputRouting({ plots, foreground, maximumPlacements }) {
  if (!Array.isArray(plots) || plots.length < 1 || plots.length > 16 || typeof foreground !== "function" ||
      !Number.isSafeInteger(maximumPlacements) || maximumPlacements < 1) {
    throw new BrowserInputRefusal("InvalidInventory", "invalid Body input routing inventory");
  }
  const placementPlots = new Map();
  const placementInputs = new Map();
  const inputKinds = new Map([["input/keyboard", "keyboard"], ["input/button", "button"], ["input/pointer-source", "pointer"]]);
  const formIds = new Set();
  for (const partition of plots) {
    const plot = partition.plot?.checked_plot_id;
    if (typeof plot !== "string" || !plot || plot.length > 256 || formIds.has(plot)) throw new BrowserInputRefusal("InvalidPlotIdentity", "invalid Body input Plot identity");
    formIds.add(plot);
    for (const fragment of partition.plan.fragments) for (const placement of fragment.placements) {
      if (typeof placement.placement_id !== "string" || !placement.placement_id || placement.placement_id.length > 256 || placementPlots.has(placement.placement_id) || placementPlots.size === maximumPlacements) throw new BrowserInputRefusal("PlacementBound", "Body input placement bound exceeded");
      placementPlots.set(placement.placement_id, plot);
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
    const plot = foreground();
    if (!formIds.has(plot)) throw new BrowserInputRefusal("StalePlot", "foreground Plot is outside the admitted body Plan");
    return plot;
  };
  const accepts = (plot, kind) => {
    for (const [placement, inputKind] of placementInputs) if (inputKind === kind && placementPlots.get(placement) === plot) return true;
    return false;
  };
  const streamKey = (kind, plot) => `${kind}\u0000${plot}`;
  const streamPressure = (kind, plot) => {
    const key = streamKey(kind, plot);
    let state = pressure.get(key);
    if (!state) {
      state = { kind, plot, capacity: kind === "pointer" ? 1 : 8, accepted: 0, delivered: 0, coalesced: 0, dropped: 0, refusals: 0 };
      pressure.set(key, state);
    }
    return state;
  };
  const queueFor = (kind, plot) => {
    const key = streamKey(kind, plot);
    let queue = queues.get(key);
    if (!queue) {
      const capacity = streamPressure(kind, plot).capacity;
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
  const failStream = (kind, plot, error) => {
    const key = streamKey(kind, plot);
    streamFailures.set(key, error);
    const queue = queues.get(key);
    if (queue) clearQueue(queue);
    increment(streamPressure(kind, plot), "refusals");
    for (let index = waiters.length - 1; index >= 0; index -= 1) {
      const waiter = waiters[index];
      if (waiter.kind !== kind || waiter.plot !== plot) continue;
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
    const boundedDelta = (value) => Math.max(-1_000_000, Math.min(1_000_000, value));
    increment(state, "coalesced");
    return Object.freeze({
      ...newer,
      delta_x: boundedDelta(add(older.delta_x ?? 0, newer.delta_x ?? 0)),
      delta_y: boundedDelta(add(older.delta_y ?? 0, newer.delta_y ?? 0)),
      coalesced: add(add(older.coalesced ?? 0, newer.coalesced ?? 0), 1),
      dropped: add(older.dropped ?? 0, newer.dropped ?? 0),
      queue_capacity: 1,
    });
  };
  const deliver = (kind, event) => {
    if (!formIds.has(event.delivery_plot)) throw new BrowserInputRefusal("StalePlot", "input lacks its captured Plot identity");
    const plot = event.delivery_plot;
    const key = streamKey(kind, plot);
    if (streamFailures.has(key)) return;
    const state = streamPressure(kind, plot);
    increment(state, "accepted");
    if (kind === "pointer") {
      increment(state, "coalesced", event.coalesced ?? 0);
      increment(state, "dropped", event.dropped ?? 0);
    }
    const index = waiters.findIndex(waiter => waiter.kind === kind && waiter.plot === event.delivery_plot);
    if (index >= 0) {
      const waiter = waiters.splice(index, 1)[0];
      increment(state, "delivered");
      waiter.dispose(); waiter.resolve(event);
    } else {
      const queue = queueFor(kind, plot);
      if (kind === "pointer" && queue.length === 1) {
        try { queue.items[queue.head] = coalescePointer(queue.items[queue.head], event, state); }
        catch (error) { failStream(kind, plot, error); }
        return;
      }
      if (queue.length === state.capacity) {
        failStream(kind, plot, new BrowserInputRefusal("Pressure", `ordered ${kind} stream capacity exhausted`));
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
        const plot = heldKeys[usage] ?? selected();
        if (!accepts(plot, kind)) return null;
        heldKeys[usage] = phase === 0 ? plot : null;
        return plot;
      }
      if (kind === "button") {
        const plot = heldButton ?? selected();
        if (!accepts(plot, kind)) return null;
        heldButton = value.pressed ? plot : null;
        return plot;
      }
      if (kind === "pointer") {
        const plot = selected();
        return accepts(plot, kind) ? plot : null;
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
        return Promise.reject(new BrowserInputRefusal("StalePlacement", "input request is outside the admitted body Plan"));
      }
      if (signal.aborted) return Promise.reject(new BrowserInputRefusal("Cancelled", "Body input request cancelled"));
      if (waiters.some(waiter => waiter.placement === placement)) {
        return Promise.reject(new BrowserInputRefusal("DuplicateRequest", "Body input placement already has a pending request"));
      }
      const plot = placementPlots.get(placement);
      const key = streamKey(kind, plot);
      const failed = streamFailures.get(key);
      if (failed) return Promise.reject(failed);
      const queue = queues.get(key);
      if (queue?.length) {
        const event = dequeue(queue);
        increment(streamPressure(kind, plot), "delivered");
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
        const waiter = { kind, plot, placement, resolve, reject, dispose: () => signal.removeEventListener("abort", abort) };
        waiters.push(waiter);
        signal.addEventListener("abort", abort, { once: true });
      });
      void pump(kind);
      return pending;
    },
    pressure() {
      return Object.freeze([...pressure.values()].map(state => Object.freeze({
        ...state,
        occupancy: queues.get(streamKey(state.kind, state.plot))?.length ?? 0,
        terminal: streamFailures.get(streamKey(state.kind, state.plot))?.code ?? null,
      })));
    },
    close() { fail(new BrowserInputRefusal("Cancelled", "Body input routing closed")); },
  });
}
