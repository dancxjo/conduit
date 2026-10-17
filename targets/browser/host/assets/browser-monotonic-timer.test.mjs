import test from "node:test";
import assert from "node:assert/strict";
import { createBrowserMonotonicTimer } from "./browser-monotonic-timer.mjs";

function fixture() {
  let elapsed = 0;
  let wall = 100_000;
  let nextId = 0;
  const callbacks = new Map();
  const performance = { timeOrigin: 1234, now: () => elapsed };
  let nonce = 0;
  const window = {
    performance,
    crypto: { getRandomValues(bytes) { bytes.fill(++nonce); return bytes; } },
    setTimeout(callback, delay) { const id = ++nextId; callbacks.set(id, { callback, delay }); return id; },
    clearTimeout(id) { callbacks.delete(id); },
  };
  return {
    window,
    advance(millis) { elapsed += millis; },
    stepWall(millis) { wall += millis; },
    wall: () => wall,
    fire() { const [id, entry] = callbacks.entries().next().value; callbacks.delete(id); entry.callback(); },
    pending: () => [...callbacks.values()].map(({ delay }) => delay),
  };
}

test("elapsed timer ignores forward and backward wall steps and early callbacks", async () => {
  const clock = fixture();
  const timer = createBrowserMonotonicTimer(clock.window, "host", "boot");
  const slot = { pending: null, cancel: null };
  let completed = false;
  const wait = timer.wait(5000, new AbortController().signal, slot).then(() => { completed = true; });
  clock.stepWall(60_000); clock.advance(1000); clock.fire();
  await Promise.resolve();
  assert.equal(completed, false);
  assert.deepEqual(clock.pending(), [4000]);
  clock.stepWall(-120_000); clock.advance(4000); clock.fire();
  await wait;
  assert.equal(completed, true);
  assert.equal(slot.pending, null);
});

test("missing or changed performance basis refuses instead of using wall time", async () => {
  const clock = fixture();
  assert.throws(() => createBrowserMonotonicTimer({ ...clock.window, performance: null }, "host", "boot"), /monotonic timer unavailable/);
  assert.throws(() => createBrowserMonotonicTimer({ ...clock.window, crypto: null }, "host", "boot"), /monotonic timer unavailable/);
  const timer = createBrowserMonotonicTimer(clock.window, "host", "boot");
  const wait = timer.wait(5000, new AbortController().signal, { pending: null, cancel: null });
  clock.window.performance = { timeOrigin: 1234, now: () => 5000 };
  clock.fire();
  await assert.rejects(wait, /monotonic timer unavailable/);
});

test("new timer sessions use distinct stable clock identities", () => {
  const clock = fixture();
  const first = createBrowserMonotonicTimer(clock.window, "host", "boot");
  const second = createBrowserMonotonicTimer(clock.window, "host", "boot");
  const firstBasis = first.basisId;
  assert.match(first.basisId, /^browser\/performance-[0-9a-f]{32}$/);
  assert.notEqual(first.basisId, second.basisId);
  clock.advance(1);
  first.nowMicros();
  assert.equal(first.basisId, firstBasis);
});

test("a paused counter stays pending and a late wake completes only the original wait", async () => {
  const clock = fixture();
  const timer = createBrowserMonotonicTimer(clock.window, "host", "boot");
  const slot = { pending: null, cancel: null };
  let completions = 0;
  const wait = timer.wait(5000, new AbortController().signal, slot).then(() => { completions += 1; });
  // A callback after a pause is not evidence of elapsed provider time.
  clock.stepWall(60_000);
  clock.fire();
  await Promise.resolve();
  assert.equal(completions, 0);
  assert.deepEqual(clock.pending(), [5000]);
  // A continuous provider can instead advance while delivery is delayed.
  clock.advance(6000);
  clock.fire();
  await wait;
  assert.equal(completions, 1);
  assert.equal(slot.pending, null);
  assert.deepEqual(clock.pending(), []);
});

test("regression, cancellation, and closed session cannot complete a stale timer", async () => {
  const clock = fixture();
  const timer = createBrowserMonotonicTimer(clock.window, "host", "boot");
  const controller = new AbortController();
  const slot = { pending: null, cancel: null };
  const cancelled = timer.wait(5000, controller.signal, slot);
  controller.abort();
  await assert.rejects(cancelled, /cancelled/);
  assert.deepEqual(clock.pending(), []);
  clock.advance(100);
  const wait = timer.wait(5000, new AbortController().signal, slot);
  clock.advance(-1); clock.fire();
  await assert.rejects(wait, /monotonic timer unavailable/);
  timer.close();
  assert.throws(() => timer.nowMicros(), /monotonic timer unavailable/);
  await assert.rejects(timer.wait(1, new AbortController().signal, slot), /monotonic timer unavailable/);
});
