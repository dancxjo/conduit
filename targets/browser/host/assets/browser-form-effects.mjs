// A typed result of one real platform attempt. Other errors remain programming/resource failures.
export class BrowserHostEffectRefusal extends Error {
  constructor(disposition, detail, message) {
    super(message);this.name = "BrowserHostEffectRefusal";
    if (!["denied", "failed"].includes(disposition) || !Number.isInteger(detail) || detail < 0 || detail > 65535) throw new Error("invalid Host effect refusal");
    this.disposition = disposition;this.detail = detail;
  }
}

// Performs one bounded browser-local tone. The kernel owns the request and
// completion; this adapter owns only the admitted Web Audio platform effect.
export function createPitchTonePerformer(window) {
  let context = null;
  const refusal = (denied, detail, message) =>
    new BrowserHostEffectRefusal(denied ? "denied" : "failed", detail, message);
  return async (effect, signal) => {
    if (effect.oscillator !== "sine" || !Number.isInteger(effect.hertz) || effect.hertz < 20 ||
        effect.hertz > 20_000 || !Number.isInteger(effect.duration_millis) ||
        effect.duration_millis < 20 || effect.duration_millis > 250 ||
        !Number.isInteger(effect.gain_millionths) || effect.gain_millionths < 1 ||
        effect.gain_millionths > 25_000) throw new Error("pitch tone exceeds its admitted safety envelope");
    if (signal.aborted) throw refusal(false, 4, "Pitch tone cancelled");
    if (!context) {
      if (typeof window.AudioContext !== "function") {
        throw refusal(false, 1, "Audio output is unavailable");
      }
      try { context = new window.AudioContext(); }
      catch (error) { throw refusal(false, 1, `Audio output is unavailable: ${error.message}`); }
    }
    if (context.state !== "running") {
      throw refusal(true, 2, "The browser has not permitted audio for this interaction");
    }
    await new Promise((resolve, reject) => {
      let oscillator = null, gain = null, deadline = null, settled = false;
      const finish = error => {
        if (settled) return;
        settled = true;
        window.clearTimeout(deadline);
        signal.removeEventListener("abort", abort);
        context.removeEventListener("statechange", changed);
        if (oscillator) {
          oscillator.onended = null;
          try { oscillator.stop(); } catch {}
          oscillator.disconnect();
        }
        gain?.disconnect();
        error ? reject(error) : resolve();
      };
      const abort = () => finish(refusal(false, 4, "Pitch tone cancelled"));
      const changed = () => {
        if (context.state !== "running") finish(refusal(false, 4, "Audio output was interrupted"));
      };
      signal.addEventListener("abort", abort, { once: true });
      context.addEventListener("statechange", changed);
      try {
        const now = context.currentTime;
        const end = now + effect.duration_millis / 1000;
        const peak = effect.gain_millionths / 1_000_000;
        oscillator = context.createOscillator();
        gain = context.createGain();
        oscillator.type = effect.oscillator;
        oscillator.frequency.setValueAtTime(effect.hertz, now);
        gain.gain.setValueAtTime(0, now);
        gain.gain.linearRampToValueAtTime(peak, now + 0.012);
        gain.gain.setValueAtTime(peak, Math.max(now + 0.012, end - 0.025));
        gain.gain.linearRampToValueAtTime(0, end);
        oscillator.connect(gain);gain.connect(context.destination);
        oscillator.onended = () => finish();
        deadline = window.setTimeout(() => finish(refusal(false, 5, "Pitch tone completion deadline expired")), effect.duration_millis + 250);
        oscillator.start(now);oscillator.stop(end);
      } catch (error) {
        finish(refusal(false, 3, `Audio rendering failed: ${error.message}`));
      }
    });
  };
}

// Shared page-Host dispatch for effects requested by the one WASM kernel.
// It does not plan work or schedule semantic operations.
export async function drainBrowserEffects({ api, initialProgress, readOutput, perform,
  isCurrent = () => true, onWaiting = () => {}, bridge = null }) {
  const encoder = new TextEncoder();
  const effects = new Map();
  let wake = null;
  let progress = initialProgress;
  const capacity = api.conduit_browser_form_pending_capacity();
  try {
    while (isCurrent()) {
      while (progress.effect_kind) {
        const effect = progress;
        const key = JSON.stringify([effect.active_play_id, effect.placement_id,
          effect.request_sequence ?? effect.observation_sequence]);
        if (effect.effect_kind === "cancel") {
          const pending = effects.get(key);
          if (!pending || pending.effect.effect_kind !== "timer") {
            throw new Error("kernel cancellation does not name a pending timer");
          }
          pending.controller.abort();
          effects.delete(key);
          const play = encoder.encode(effect.active_play_id);
          const placement = encoder.encode(effect.placement_id);
          const bytes = new Uint8Array(play.length + placement.length);
          bytes.set(play);
          bytes.set(placement, play.length);
          const input = bridge
            ? bridge.writeInput(bytes, {
                pointerExport: "conduit_browser_form_input_ptr",
                capacityExport: "conduit_browser_form_input_capacity",
                label: "cancellation acknowledgement input",
              })
            : new Uint8Array(api.memory.buffer, api.conduit_browser_form_input_ptr(), bytes.length);
          if (!bridge) input.set(bytes);
          let result;
          try {
            result = api.conduit_browser_form_acknowledge_cancellation(play.length, placement.length, effect.request_sequence);
          } finally {
            input.fill(0);
          }
          if (result < 0) throw new Error(`cancellation acknowledgement refused (${result})`);
          progress = readOutput(api);
          continue;
        }
        if (effects.has(key) || effects.size >= capacity) {
          throw new Error("browser Host effect identity or capacity violation");
        }
        const pending = { key, effect, ready: false, controller: new AbortController() };
        effects.set(key, pending);
        const settle = (result) => {
          Object.assign(pending, result, { ready: true });
          wake?.();
          wake = null;
        };
        perform(effect, pending.controller.signal).then(
          (output) => settle({ output }),
          (error) => settle({ error }),
        );
        const poll = api.conduit_browser_form_poll_effect();
        if (poll < 0) throw new Error(`effect poll refused (${poll})`);
        progress = readOutput(api);
      }
      if (progress.disposition !== "waiting") {
        if (effects.size) throw new Error("Play completed with platform effects pending");
        break;
      }
      if (!effects.size) throw new Error("Play awaits an absent platform effect");
      let completed = [...effects.values()].find((effect) => effect.ready);
      if (!completed) {
        onWaiting([...effects.values()].map(({ effect }) => effect));
        await new Promise((resolve) => { wake = resolve; });
        if (!isCurrent()) return;
        completed = [...effects.values()].find((effect) => effect.ready);
      }
      if (!isCurrent()) return;
      effects.delete(completed.key);
      if (completed.error && !(completed.error instanceof BrowserHostEffectRefusal)) throw completed.error;
      const { effect, output = new Uint8Array() } = completed;
      const play = encoder.encode(effect.active_play_id);
      const placement = encoder.encode(effect.placement_id);
      const bytes = new Uint8Array(play.length + placement.length + output.length);
      bytes.set(play);
      bytes.set(placement, play.length);
      bytes.set(output, play.length + placement.length);
      const input = bridge
        ? bridge.writeInput(bytes, {
            pointerExport: "conduit_browser_form_input_ptr",
            capacityExport: "conduit_browser_form_input_capacity",
            label: "effect completion input",
          })
        : (() => {
            if (bytes.length > api.conduit_browser_form_input_capacity()) {
              throw new Error("effect completion exceeds the admitted input bound");
            }
            const view = new Uint8Array(api.memory.buffer, api.conduit_browser_form_input_ptr(), bytes.length);
            view.set(bytes);
            return view;
          })();
      let completion;
      try {
        completion = completed.error
          ? api.conduit_browser_form_refuse_effect(play.length, placement.length,
              effect.request_sequence ?? effect.observation_sequence,
              completed.error.disposition === "denied" ? 1 : 2, completed.error.detail)
          : api.conduit_browser_form_complete_effect(play.length, placement.length,
              effect.request_sequence ?? effect.observation_sequence, output.length);
      } finally {
        input.fill(0);
      }
      if (completion < 0) {
        const refusal = api.conduit_browser_form_output_len() > 0 ? readOutput(api) : null;
        throw new Error(`effect completion refused (${completion})${refusal?.message ? `: ${refusal.message}` : ""}`);
      }
      progress = readOutput(api);
    }
    return isCurrent() ? progress : undefined;
  } finally {
    for (const pending of effects.values()) pending.controller.abort();
    effects.clear();
  }
}
