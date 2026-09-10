import test from "node:test";
import assert from "node:assert/strict";
import { createBodyInputRouting } from "../../targets/browser/host/assets/browser-body-input.mjs";

function setup() {
  let selected = "notes", waiting;
  const forms = ["notes", "morse"].map(form => ({ form: { checked_form_id: form }, plan: { fragments: [{ placements: [{ placement_id: `${form}/keys`, kind_id: "input/keyboard" }] }] } }));
  const routing = createBodyInputRouting({ forms, foreground: () => selected });
  routing.attach({ nextKeyboard: () => new Promise((resolve, reject) => { waiting = { resolve, reject }; }) });
  const next = (form, signal = new AbortController().signal) => routing.next("keyboard", `${form}/keys`, signal);
  const capture = (usage, phase = 0) => {
    const canonical_bytes = Uint8Array.of(usage, phase, 0);
    return { canonical_bytes, delivery_form: routing.capture("keyboard", canonical_bytes) };
  };
  const send = async event => {
    assert.ok(waiting, "one acquired reader is armed");
    const pending = waiting; waiting = null; pending.resolve(event);
    await Promise.resolve();
  };
  const close = () => { routing.close(); waiting?.reject(new Error("closed")); };
  return { routing, next, capture, send, close, select(form) { selected = form; } };
}

test("foreground changes route future keys while preserving background pending requests", async () => {
  const f = setup();
  let notesDone = false;
  const notes = f.next("notes").then(value => { notesDone = true; return value; });
  const morse = f.next("morse");
  f.select("morse");
  await f.send(f.capture(4));
  assert.equal((await morse).delivery_form, "morse");
  assert.equal(notesDone, false);
  f.select("notes");
  await f.send(f.capture(5));
  assert.equal((await notes).delivery_form, "notes");
  f.close();
});

test("queued keys retain capture focus and held-key releases stay with their press", async () => {
  const f = setup();
  const first = f.next("notes");
  const down = f.capture(4);
  f.select("morse");
  await f.send(down);
  assert.equal((await first).delivery_form, "notes");
  await f.send(f.capture(4, 1));
  assert.equal((await f.next("notes")).canonical_bytes[1], 1);
  const next = f.next("morse");
  await f.send(f.capture(4));
  assert.equal((await next).delivery_form, "morse");
  f.close();
});

test("cancelling one input request does not cancel another Form's request", async () => {
  const f = setup();
  const controller = new AbortController();
  const cancelled = assert.rejects(f.next("notes", controller.signal), { code: "Cancelled" });
  const morse = f.next("morse");
  controller.abort();
  await cancelled;
  f.select("morse");
  await f.send(f.capture(4));
  assert.equal((await morse).delivery_form, "morse");
  f.close();
});

test("uninstalled identities and duplicate requests refuse without consuming input", async () => {
  const f = setup();
  const first = f.next("notes");
  await assert.rejects(f.next("notes"), /already has a pending/);
  await assert.rejects(f.next("unknown"), /outside the admitted/);
  f.select("unknown");
  assert.throws(() => f.capture(4), /outside the admitted/);
  f.select("notes");
  await f.send(f.capture(4));
  assert.equal((await first).delivery_form, "notes");
  f.close();
});

test("input pressure has a finite boundary and closes outstanding delivery", async () => {
  const f = setup();
  const failed = assert.rejects(f.next("morse"), { code: "Pressure" });
  for (let usage = 4; usage < 13; usage++) await f.send(f.capture(usage));
  await failed;
  await assert.rejects(f.next("notes"), { code: "Pressure" });
  f.close();
});

test('pointer delivery follows its captured Form and unrelated input cannot fill its queue', async () => {
  let selected = 'pointer', consume, stopped = false;
  const forms = ['pointer', 'notes'].map(form => ({ form: { checked_form_id: form }, plan: { fragments: [{ placements: [{ placement_id: `${form}/input`, kind_id: form === 'pointer' ? 'input/pointer-source' : 'input/keyboard' }] }] } }));
  const routing = createBodyInputRouting({ forms, foreground: () => selected });
  routing.attach({ observePointer(listener) { consume = listener; return () => { stopped = true; }; } });
  const signal = new AbortController().signal;
  const waiting = routing.next('pointer', 'pointer/input', signal);
  const event = { position_x: 250000, delivery_form: routing.capture('pointer', null) };
  selected = 'notes';
  for (let i = 0; i < 100; i++) assert.equal(routing.capture('pointer', null), null);
  assert.equal(routing.capture('button', { pressed: true }), null);
  consume(event);
  assert.equal((await waiting).position_x, 250000);
  await assert.rejects(routing.next('pointer', 'notes/input', signal), { code: 'StalePlacement' });
  routing.close();
  assert.equal(stopped, true);
});
