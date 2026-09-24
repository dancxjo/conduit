import assert from "node:assert/strict";
import test from "node:test";
import { drainBrowserEffects, BrowserHostEffectRefusal } from "../../targets/browser/host/assets/browser-form-effects.mjs";
import { bindBrowserRuntimeBridge } from "../../targets/browser/host/assets/browser-runtime-bridge.mjs";

const effect = (placement) => ({ effect_kind: "timer", active_play_id: "body-play/one", placement_id: placement, request_sequence: 0 });
const waiting = { disposition: "waiting" };

function fixture(polls, completions, capacity = 2) {
  let output;
  const outputPointer = 4096;
  let outputLength = 0;
  const received = [];
  const setOutput = (value) => {
    output = value;
    if (value === undefined) {
      outputLength = 0;
      return;
    }
    const bytes = new TextEncoder().encode(JSON.stringify(value));
    new Uint8Array(api.memory.buffer, outputPointer, bytes.length).set(bytes);
    outputLength = bytes.length;
  };
  const identity = new TextEncoder().encode("conduit.browser/runtime-abi");
  const api = {
    memory: new WebAssembly.Memory({ initial: 1 }),
    conduit_browser_runtime_abi_revision: () => 1,
    conduit_browser_runtime_abi_identity_ptr: () => 1024,
    conduit_browser_runtime_abi_identity_len: () => identity.length,
    conduit_browser_form_output_ptr: () => outputPointer,
    conduit_browser_form_pending_capacity: () => capacity,
    conduit_browser_form_input_capacity: () => 4096,
    conduit_browser_form_input_ptr: () => 0,
    conduit_browser_form_output_len: () => outputLength,
    conduit_browser_form_poll_effect: () => { setOutput(polls.shift()); return 0; },
    conduit_browser_form_complete_effect: (playLength, placementLength, sequence, length) => {
      const bytes = new Uint8Array(api.memory.buffer);
      received.push({
        play: new TextDecoder().decode(bytes.slice(0, playLength)),
        placement: new TextDecoder().decode(bytes.slice(playLength, playLength + placementLength)),
        sequence, length,
      });
      setOutput(completions.shift()); return 0;
    },
  };
  new Uint8Array(api.memory.buffer, 1024, identity.length).set(identity);
  const bridge = bindBrowserRuntimeBridge(api, { context: "effects proof runtime" });
  return { api, bridge, received, readOutput: () => output, setOutput };
}

test("one host dispatcher preserves cross-form completion correlation", async () => {
  const host = fixture([effect("second"), waiting], [waiting, { disposition: "completed" }]);
  let releaseFirst;
  const result = await drainBrowserEffects({
    ...host, initialProgress: effect("first"),
    bridge: host.bridge,
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
      bridge: host.bridge,
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
    bridge: host.bridge,
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
    host.setOutput({ disposition: "completed" });
    return 0;
  };
  const result = await drainBrowserEffects({
    api: host.api, bridge: host.bridge, readOutput: () => host.readOutput(), initialProgress: effect("timer"),
    perform: async (_, pendingSignal) => { signal = pendingSignal; return new Promise(() => {}); },
  });
  assert.equal(result.disposition, "completed");
  assert.deepEqual(host.received, []);
});


test("typed Host denial is correlated without stopping unrelated pending work", async () => {
  const host = fixture([effect("keyboard"), waiting], [{ disposition: "completed" }]);
  const refusals = [];
  let releaseKeyboard;
  host.api.conduit_browser_form_refuse_effect = (playLength, placementLength, sequence, disposition, detail) => {
    const bytes = new Uint8Array(host.api.memory.buffer);
    refusals.push({ play: new TextDecoder().decode(bytes.slice(0, playLength)), placement: new TextDecoder().decode(bytes.slice(playLength, playLength + placementLength)), sequence, disposition, detail });
    host.setOutput({ disposition: "waiting" });
    queueMicrotask(releaseKeyboard);
    return 0;
  };
  const result = await drainBrowserEffects({ ...host, initialProgress: effect("audio"),
    bridge: host.bridge,
    perform: async value => {
      if (value.placement_id === "audio") throw new BrowserHostEffectRefusal("denied", 2, "audio policy denied");
      await new Promise(resolve => { releaseKeyboard = resolve; });
      return Uint8Array.of(1);
    },
  });
  assert.equal(result.disposition, "completed");
  assert.deepEqual(refusals, [{ play: "body-play/one", placement: "audio", sequence: 0, disposition: 1, detail: 2 }]);
  assert.deepEqual(host.received, [{ play: "body-play/one", placement: "keyboard", sequence: 0, length: 1 }]);
});
