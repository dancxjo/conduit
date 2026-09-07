// Shared page-Host dispatch for effects requested by the one WASM kernel.
// It does not plan work or schedule semantic operations.
export async function drainBrowserEffects({ api, initialProgress, readOutput, perform,
  isCurrent = () => true, onWaiting = () => {} }) {
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
          const input = new Uint8Array(api.memory.buffer, api.conduit_browser_form_input_ptr(), play.length + placement.length);
          input.set(play);
          input.set(placement, play.length);
          const result = api.conduit_browser_form_acknowledge_cancellation(play.length, placement.length, effect.request_sequence);
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
      if (completed.error) throw completed.error;
      const { effect, output = new Uint8Array() } = completed;
      const play = encoder.encode(effect.active_play_id);
      const placement = encoder.encode(effect.placement_id);
      const total = play.length + placement.length + output.length;
      if (total > api.conduit_browser_form_input_capacity()) {
        throw new Error("effect completion exceeds the admitted input bound");
      }
      const bytes = new Uint8Array(api.memory.buffer, api.conduit_browser_form_input_ptr(), total);
      bytes.set(play);
      bytes.set(placement, play.length);
      bytes.set(output, play.length + placement.length);
      const completion = api.conduit_browser_form_complete_effect(
        play.length, placement.length, effect.request_sequence ?? effect.observation_sequence,
        output.length,
      );
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
