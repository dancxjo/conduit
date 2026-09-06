import assert from "node:assert/strict";
import test from "node:test";
import { drainBrowserEffects } from "../../targets/browser/host/assets/browser-form-effects.mjs";

const effect = (placement) => ({ effect_kind: "timer", active_play_id: "body-play/one", placement_id: placement, request_sequence: 0 });
const waiting = { disposition: "waiting" };

function fixture(polls, completions, capacity = 2) {
  let output;
  const received = [];
  const api = {
    memory: new WebAssembly.Memory({ initial: 1 }),
    conduit_browser_form_pending_capacity: () => capacity,
    conduit_browser_form_input_capacity: () => 4096,
    conduit_browser_form_input_ptr: () => 0,
    conduit_browser_form_output_len: () => 0,
    conduit_browser_form_poll_effect: () => { output = polls.shift(); return 0; },
    conduit_browser_form_complete_effect: (playLength, placementLength, sequence, length) => {
      const bytes = new Uint8Array(api.memory.buffer);
      received.push({
        play: new TextDecoder().decode(bytes.slice(0, playLength)),
        placement: new TextDecoder().decode(bytes.slice(playLength, playLength + placementLength)),
        sequence, length,
      });
      output = completions.shift(); return 0;
    },
  };
  return { api, received, readOutput: () => output };
}

test("one Host dispatcher preserves cross-Form completion correlation", async () => {
  const host = fixture([effect("second"), waiting], [waiting, { disposition: "completed" }]);
  let releaseFirst;
  const result = await drainBrowserEffects({
    ...host, initialProgress: effect("first"),
    perform: async (value) => {
      if (value.placement_id === "first") await new Promise((resolve) => { releaseFirst = resolve; });
      return Uint8Array.of(7);
    },
    onWaiting: () => { if (host.received.length === 1) queueMicrotask(releaseFirst); },
  });
  assert.equal(result.disposition, "completed");
  assert.deepEqual(host.received, [
    { play: "body-play/one", placement: "second", sequence: 0, length: 1 },
    { play: "body-play/one", placement: "first", sequence: 0, length: 1 },
  ]);
});

test("duplicate or excessive pending effects refuse and abort admitted adapters", async () => {
  for (const [next, capacity] of [[effect("first"), 2], [effect("second"), 1]]) {
    const host = fixture([next], [], capacity);
    let signal;
    await assert.rejects(drainBrowserEffects({
      ...host, initialProgress: effect("first"),
      perform: async (_, admittedSignal) => { signal = admittedSignal; return new Promise(() => {}); },
    }), /identity or capacity violation/);
    assert.equal(signal.aborted, true);
    assert.deepEqual(host.received, []);
  }
});

test("an adapter failure is not converted into a successful completion", async () => {
  const host = fixture([waiting], []);
  await assert.rejects(drainBrowserEffects({
    ...host, initialProgress: effect("failed"),
    perform: async () => { throw new Error("resource lost"); },
  }), /resource lost/);
  assert.deepEqual(host.received, []);
});

test("kernel cancellation aborts its exact timer before acknowledgement", async () => {
  const host = fixture([{ ...effect("timer"), effect_kind: "cancel" }], []);
  let signal;
  host.api.conduit_browser_form_acknowledge_cancellation = (playLength, placementLength, sequence) => {
    assert.equal(signal.aborted, true);
    const bytes = new Uint8Array(host.api.memory.buffer);
    assert.equal(new TextDecoder().decode(bytes.slice(0, playLength)), "body-play/one");
    assert.equal(new TextDecoder().decode(bytes.slice(playLength, playLength + placementLength)), "timer");
    assert.equal(sequence, 0);
    host.readOutput = () => ({ disposition: "completed" });
    return 0;
  };
  const result = await drainBrowserEffects({
    api: host.api, readOutput: () => host.readOutput(), initialProgress: effect("timer"),
    perform: async (_, pendingSignal) => { signal = pendingSignal; return new Promise(() => {}); },
  });
  assert.equal(result.disposition, "completed");
  assert.deepEqual(host.received, []);
});
