import { BrowserHostEffectRefusal } from "./browser-plot-effects.mjs";

const unsupported = () => new BrowserHostEffectRefusal("failed", 6, "Browser monotonic timer unavailable or lost its Boot/session basis");

export function createBrowserMonotonicTimer(window, hostId, bootId) {
  const performance = window.performance;
  if (!performance || typeof performance.now !== "function" ||
      !Number.isFinite(performance.timeOrigin) || typeof window.setTimeout !== "function" ||
      typeof window.clearTimeout !== "function") throw unsupported();
  const origin = performance.timeOrigin;
  let previous = -1;
  let closed = false;
  const now = () => {
    if (closed || window.performance !== performance || performance.timeOrigin !== origin) throw unsupported();
    let value;
    try { value = performance.now(); } catch { throw unsupported(); }
    if (!Number.isFinite(value) || value < 0 || value < previous) throw unsupported();
    previous = value;
    return value;
  };
  now();
  return Object.freeze({
    hostId, bootId,
    nowMicros() {
      const micros = Math.floor(now() * 1000);
      if (!Number.isSafeInteger(micros)) throw unsupported();
      return micros;
    },
    wait(duration, signal, slot) {
      if (!slot || slot.pending !== null || !Number.isSafeInteger(duration) || duration < 0 || duration > 60_000) {
        throw new Error("browser timer request exceeds acquisition");
      }
      return new Promise((resolve, reject) => {
        let deadline;
        try { deadline = now() + duration; } catch (error) { reject(error); return; }
        let settled = false;
        const finish = error => {
          if (settled) return;
          settled = true;
          if (slot.pending !== null) window.clearTimeout(slot.pending);
          slot.pending = null; slot.cancel = null;
          signal.removeEventListener("abort", abort);
          error ? reject(error) : resolve();
        };
        const abort = () => finish(new Error("browser timer cancelled"));
        const check = () => {
          if (settled) return;
          try {
            const remaining = deadline - now();
            if (remaining <= 0) finish();
            else slot.pending = window.setTimeout(check, Math.max(1, Math.ceil(remaining)));
          } catch (error) { finish(error); }
        };
        slot.cancel = abort;
        signal.addEventListener("abort", abort, { once: true });
        if (signal.aborted) { abort(); return; }
        try { slot.pending = window.setTimeout(check, duration); }
        catch { finish(unsupported()); }
      });
    },
    close() { closed = true; },
  });
}
