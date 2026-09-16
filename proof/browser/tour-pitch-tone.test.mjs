import assert from "node:assert/strict";
import test from "node:test";
import { createPitchTonePerformer } from "../../targets/browser/host/assets/browser-form-effects.mjs";

class Parameter {
  calls = [];
  setValueAtTime(value, time) { this.calls.push(["set", value, time]); }
  linearRampToValueAtTime(value, time) { this.calls.push(["ramp", value, time]); }
}

function harness(state = "running") {
  const frequency = new Parameter(), level = new Parameter();
  const listeners = new Map();
  const oscillator = {
    frequency, onended: null, connect() {}, disconnect() {},
    start() {}, stop() { queueMicrotask(() => oscillator.onended?.()); },
  };
  const gain = { gain: level, connect() {}, disconnect() {} };
  const context = {
    state, currentTime: 4, destination: {},
    createOscillator: () => oscillator,
    createGain: () => gain,
    addEventListener: (name, listener) => listeners.set(name, listener),
    removeEventListener: (name) => listeners.delete(name),
  };
  const window = {
    AudioContext: class { constructor() { return context; } },
    setTimeout, clearTimeout,
  };
  return { window, frequency, level };
}

const effect = Object.freeze({
  oscillator: "sine", hertz: 440, duration_millis: 180, gain_millionths: 25_000,
});

test("the admitted pitch is sent to a short conservatively gained oscillator", async () => {
  const { window, frequency, level } = harness();
  await createPitchTonePerformer(window)(effect, new AbortController().signal);
  assert.deepEqual(frequency.calls, [["set", 440, 4]]);
  assert.deepEqual(level.calls.map(([kind, value]) => [kind, value]), [
    ["set", 0], ["ramp", 0.025], ["set", 0.025], ["ramp", 0],
  ]);
  assert.deepEqual(level.calls.map(([, , time]) => Math.round(time * 1000)), [4000, 4012, 4155, 4180]);
});

test("audio policy denial stays distinct from an invalid or unavailable effect", async () => {
  const suspended = harness("suspended");
  await assert.rejects(
    createPitchTonePerformer(suspended.window)(effect, new AbortController().signal),
    error => error.disposition === "denied" && error.detail === 2,
  );
  await assert.rejects(
    createPitchTonePerformer({ setTimeout, clearTimeout })
      (effect, new AbortController().signal),
    error => error.disposition === "failed" && error.detail === 1,
  );
  await assert.rejects(
    createPitchTonePerformer(harness().window)
      ({ ...effect, gain_millionths: 25_001 }, new AbortController().signal),
    /safety envelope/,
  );
});
