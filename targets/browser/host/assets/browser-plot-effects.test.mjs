import test from "node:test";
import assert from "node:assert/strict";
import { drainBrowserEffects } from "./browser-plot-effects.mjs";

test("Stop aborts admitted PCM after its early Host completion", async () => {
  const controller = new AbortController();
  const observed = [];
  const pcm = { effect_kind: "pcm-playback", active_play_id: "play", placement_id: "audio", request_sequence: 0 };
  const timer = { effect_kind: "timer", active_play_id: "play", placement_id: "clock", request_sequence: 0 };
  let progress = pcm, completed = 0;
  await drainBrowserEffects({
    api: {
      conduit_browser_plot_pending_capacity: () => 2,
      conduit_browser_plot_poll_effect: () => { progress = { disposition: "waiting" }; return 0; },
    },
    initialProgress: pcm,
    readOutput: () => progress,
    bridge: { browserPlotCompleteEffect() { completed++; progress = timer; return { status: 0 }; } },
    async perform(effect, signal) {
      observed.push({ effect, signal });
      if (effect.effect_kind === "timer") return new Promise(() => {});
    },
    signal: controller.signal,
    onWaiting(effects) {
      if (completed && effects.some(effect => effect.effect_kind === "timer")) queueMicrotask(() => controller.abort());
    },
  });
  assert.equal(completed, 1);
  assert.equal(observed.length, 2);
  assert.ok(observed.every(({ signal }) => signal.aborted));
});
