import { BrowserHostEffectRefusal } from "./browser-form-effects.mjs";

export const AUDIO_CUE_RESOURCE = "conduit.resource/browser-audio-cue-slot@1";
export const AUDIO_CUE_POOL = "browser/audio-cue";
const SCORE = "conduit.sound/startup-chime@1";
const FRAMES = 57_600, RATE = 48_000;
const failure = (denied, detail, message) => new BrowserHostEffectRefusal(denied ? "denied" : "failed", detail, message);

/** One prepared PCM buffer and at most one active platform source per admitted slot.
 * A slot admits an optional cue attempt, not permission or proof of audibility.
 */
export function acquireBrowserAudioCue({ api, window, placements, muted = false }) {
  if (!Array.isArray(placements) || placements.length < 1 || placements.length > 16 ||
      new Set(placements.map(item => item.placement_id)).size !== placements.length) throw new Error("invalid audio cue slot acquisition");
  const slots = new Map(placements.map(item => [item.placement_id, { cancel: null }]));
  let context = null, buffer = null, closed = false;
  let unavailable = null;
  try {
    if (typeof window.AudioContext !== "function") throw new Error("Audio output is unavailable");
    if (api.conduit_browser_startup_chime_frames() !== FRAMES) throw new Error("cue frame bound differs");
    const pointer = api.conduit_browser_startup_chime_prepare();
    const samples = new Int16Array(api.memory.buffer, pointer, FRAMES);
    context = new window.AudioContext({ sampleRate: RATE });
    buffer = context.createBuffer(1, FRAMES, RATE);
    const channel = buffer.getChannelData(0);
    for (let index = 0; index < FRAMES; index++) channel[index] = samples[index] / 32768;
  } catch (error) {
    unavailable = error.message;
    context?.close().catch(() => {});
    context = null;buffer = null;
  }
  return Object.freeze({
    capacity: slots.size,
    async perform(effect, signal) {
      const slot = slots.get(effect.placement_id);
      if (closed || !slot || slot.cancel || effect.score_id !== SCORE || effect.frames !== FRAMES || effect.sample_rate !== RATE || effect.channels !== 1) throw new Error("audio cue differs from acquired slot");
      if (signal.aborted) throw failure(false, 4, "Audio cue cancelled");
      if (muted) throw failure(true, 2, "Audio is muted");
      if (!context || !buffer) throw failure(false, 1, unavailable ?? "Audio output is unavailable");
      // Never queue audio until a later gesture. A suspended context is a real
      // denial for this wake; it is not retried or silently replayed afterward.
      if (context.state !== "running") throw failure(true, 2, "The browser has not permitted audio for this wake");
      await new Promise((resolve, reject) => {
        let source = null, timer = null, settled = false;
        const finish = error => {
          if (settled) return;
          settled = true;window.clearTimeout(timer);slot.cancel = null;
          signal.removeEventListener("abort", abort);
          context.removeEventListener("statechange", changed);
          if (source) {
            source.onended = null;
            try { source.stop(); } catch {}
            source.disconnect();source.buffer = null;
          }
          error ? reject(error) : resolve();
        };
        const abort = () => finish(failure(false, 4, "Audio cue cancelled"));
        const changed = () => { if (context.state !== "running") finish(failure(false, 4, "Audio output was interrupted")); };
        slot.cancel = abort;
        signal.addEventListener("abort", abort, { once: true });
        context.addEventListener("statechange", changed);
        try {
          source = context.createBufferSource();source.buffer = buffer;
          source.connect(context.destination);
          source.onended = () => finish();
          // One finite platform operation, including an explicit completion deadline.
          timer = window.setTimeout(() => finish(failure(false, 5, "Audio completion deadline expired")), 2500);
          source.start();
        } catch (error) { finish(failure(false, 3, `Audio rendering failed: ${error.message}`)); }
      });
    },
    close() {
      if (closed) return;
      closed = true;
      for (const slot of slots.values()) slot.cancel?.();
      context?.close().catch(() => {});buffer = null;
    },
  });
}
