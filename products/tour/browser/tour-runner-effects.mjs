import { BrowserHostEffectRefusal } from "../../../targets/browser/host/assets/browser-form-effects.mjs";

// Adapts effects admitted by the production kernel to this browser laboratory.
export function createTourEffectPerformer({ api, runner, humanInput, openHumanInput, isCurrent,
  delay, renderIdentities, renderRunIdentities, renderMorse, setIndicator }) {
  const tone = createPitchTonePerformer(globalThis);
  return async (progress, signal) => {
    if (typeof progress?.active_play_id === "string") {
      renderRunIdentities(runner, progress);
    }
    if (progress.effect_kind === "clock-observation") {
      const bytes = new Uint8Array(8);
      new DataView(bytes.buffer).setBigUint64(0, BigInt(Math.floor(performance.now() * 1000)), true);
      return bytes;
    } else if (progress.effect_kind === "timer") {
      runner.playStatus.ordinary(`Waiting for planned tick · ${progress.duration_millis} ms`);
      if (!await delay(progress.duration_millis, signal)) return;
    } else if (progress.effect_kind === "key-event") {
      runner.playStatus.ordinary("Waiting for one admitted keyboard transition…");
      const event = await humanInput.nextKeyboard();
      if (!isCurrent()) return;
      const encoded = event.canonical_bytes;
      return encoded;
    } else if (progress.effect_kind === "pointer-event") {
      runner.querySelector(".input-button").hidden = false;
      if (!runner.querySelector(".input-button").matches("input[type=range]")) {
        runner.querySelector(".input-button").textContent = "Choose a horizontal position";
      }
      runner.playStatus.ordinary("Click a horizontal position on the controller to choose a pitch.");
      const event = await nextPointer(openHumanInput, runner, signal);
      if (!isCurrent()) return;
      const code = api.conduit_browser_form_encode_pointer(
        event.position_x, event.position_y, event.delta_x, event.delta_y,
        event.primary_pressed ? 1 : 0, event.coalesced, event.dropped,
        event.queue_capacity, event.sequence,
      );
      if (code < 0) throw new Error(`pointer encoding refused (${code})`);
      return new Uint8Array(api.memory.buffer, api.conduit_browser_form_output_ptr(),
        api.conduit_browser_form_output_len()).slice();
    } else if (progress.effect_kind === "button-transition") {
      runner.querySelector(".input-button").hidden = false;
      runner.playStatus.ordinary("Waiting for one admitted button transition…");
      const event = await humanInput.nextButton();
      if (!isCurrent()) return;
      const encodedCode = api.conduit_tour_encode_button_transition(
        event.pressed ? 1 : 0,
        BigInt(event.sequence),
      );
      if (encodedCode < 0) throw new Error(`button transition encoding refused (${encodedCode})`);
      const encoded = new Uint8Array(
        api.memory.buffer,
        api.conduit_browser_form_output_ptr(),
        api.conduit_browser_form_output_len(),
      ).slice();
      return encoded;
    } else if (progress.effect_kind === "pitch-tone") {
      await tone(progress, signal);
      runner.playStatus.ordinary(`Played admitted ${progress.hertz} Hz tone; continuing the same Play…`);
    } else if (progress.effect_kind === "manifestation") {
      runner.querySelector(".morse").textContent =
        progress.text ?? renderMorse(progress.segments);
      renderIdentities(runner, progress);
      if (progress.presentation_kind === "presentation/indicator-state") {
        setIndicator(runner, progress.text === "true");
      } else if (progress.presentation_kind === "presentation/pulse") {
        const indicator = runner.querySelector(".indicator");
        for (const animation of indicator.getAnimations()) animation.cancel();
        if (!globalThis.matchMedia("(prefers-reduced-motion: reduce)").matches) {
          indicator.animate([
            { backgroundColor: "#d1ffc3", boxShadow: "0 0 48px #96f5a8", transform: "scale(1.08)" },
            { backgroundColor: "#19372b", boxShadow: "0 0 0 transparent", transform: "scale(1)" },
          ], { duration: 180 });
        }
      } else {
        for (const segment of progress.segments) {
          if (!isCurrent()) return;
          setIndicator(runner, segment.level);
          if (!await delay(segment.units * progress.unit_millis, signal)) return;
        }
        setIndicator(runner, false);
      }
      runner.playStatus.ordinary("Observed planned presentation; continuing the same Play…");
    } else {
      throw new Error(`unsupported browser Host effect ${progress.effect_kind}`);
    }
  };
}

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

function nextPointer(openInput, runner, signal) {
  const input = openInput(runner.querySelector(".input-button"));
  return new Promise((resolve, reject) => {
    let stop;
    const cleanup = () => {
      stop?.();
      input.close();
      signal.removeEventListener("abort", cancel);
      runner.cancelPointer = null;
    };
    const cancel = () => { cleanup(); reject(new Error("Pointer request cancelled")); };
    stop = input.observePointer((event, error) => {
      cleanup();
      if (error) reject(error);
      else resolve(event);
    });
    runner.cancelPointer = cancel;
    signal.addEventListener("abort", cancel, { once: true });
    if (signal.aborted) cancel();
  });
}
