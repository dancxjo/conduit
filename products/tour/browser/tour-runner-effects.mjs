// Adapts effects admitted by the production kernel to this browser laboratory.
export function createTourEffectPerformer({ api, runner, humanInput, openHumanInput, isCurrent,
  delay, renderIdentities, renderMorse, setIndicator }) {
  return async (progress, signal) => {
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
      runner.querySelector(".input-button").textContent = "Choose a horizontal position";
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
