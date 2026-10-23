import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test from "node:test";
import { stopProcess } from "./process-stop.mjs";

function fixture(exited = false) {
  const child = Object.assign(new EventEmitter(), { pid: 42, exitCode: exited ? 0 : null, signalCode: null,
    stdout: { readableEnded: false }, stderr: { readableEnded: false }, signals: [] });
  child.kill = signal => child.signals.push(signal ?? "SIGTERM");
  const timers = new Map();
  const clock = { set: (fn, ms) => { timers.set(ms, fn); return ms; }, clear: id => timers.delete(id) };
  return { child, timers, clock };
}

test("exit alone does not retire output; close joins the pipes and removes timers", async () => {
  const { child, clock, timers } = fixture();
  let finished = false;
  const pending = stopProcess(child, clock).then(() => { finished = true; });
  child.exitCode = 0; child.emit("exit", 0);
  await Promise.resolve(); assert.equal(finished, false);
  assert.equal(timers.has(2000), false);
  child.emit("close", 0); await pending;
  assert.equal(timers.size, 0);
  assert.equal(child.listenerCount("exit") + child.listenerCount("close"), 0);
});

test("an already exited child still drains output without signalling a process", async () => {
  const { child, clock } = fixture(true);
  const pending = stopProcess(child, clock);
  assert.deepEqual(child.signals, []);
  child.emit("close", 0); await pending;
  child.stdout.readableEnded = true; child.stderr.readableEnded = true;
  await stopProcess(child, clock);
});

test("a live process is escalated once and a held pipe has the existing finite cleanup refusal", async () => {
  const { child, clock, timers } = fixture();
  const pending = stopProcess(child, clock);
  timers.get(2000)();
  assert.deepEqual(child.signals, ["SIGTERM", "SIGKILL"]);
  child.exitCode = 0; child.emit("exit", 0);
  timers.get(5000)();
  await assert.rejects(pending, /owned output did not close/);
  assert.equal(timers.size, 0);
  assert.equal(child.listenerCount("exit") + child.listenerCount("close"), 0);
});
